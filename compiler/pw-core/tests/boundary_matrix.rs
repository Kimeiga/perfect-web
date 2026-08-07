//! **What a typed value costs to cross a boundary, frozen before there is one
//! analysis answering it.**
//!
//! Architect ruling, 2026-08-07:
//!
//! > Remote-capable is not equivalent to resume-serializable. But they are two
//! > policies over the same underlying semantic fact: whether a typed value can
//! > safely cross a boundary. Don't build a second `is_remote_capable_type()`
//! > beside `is_serializable_capture()`.
//!
//! ```text
//!                   boundary-transfer analysis
//!                      /                 \
//!           resume-capture policy      remote-call policy
//! ```
//!
//! The resume-capture policy exists and decides today. The remote-call policy
//! does not. So the risk in building the shared analysis is the ordinary one:
//! the refactor quietly changes what the existing policy decides, and the only
//! witness is a corpus that still passes because both the old and the new
//! answer happen to be "reject this file".
//!
//! `docs/RISK_QUEUE.md`'s admissibility rule is what this file answers — the
//! instrument comes before the change it measures. Every row below is a
//! capture, a boundary, and the verdict as of the day the shared analysis was
//! built. `contract_matrix.rs` caught a live security hole on its first run for
//! exactly this reason.
//!
//! # Why the rows are captures rather than types
//!
//! Two questions are answered by two different machines, and a value can pass
//! one and fail the other:
//!
//! ```text
//! can this value be serialized at all?   its TYPE — is it a resource
//! may it cross THIS boundary?            its LABEL — is it private
//! ```
//!
//! A matrix over types alone would freeze half the analysis and would have
//! nothing to say about `R-030`, whose captured `Cart` is a perfectly ordinary
//! serializable record.

use std::collections::BTreeSet;

use pw_core::check::check_sources;

mod support;

/// The library every corpus program is checked against.
fn library() -> Vec<(String, String)> {
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    let mut out = Vec::new();
    for dir in [
        "examples/lib",
        "packages/pw-std",
        "packages/pw-platform-web",
    ] {
        let mut files: Vec<std::path::PathBuf> = std::fs::read_dir(root.join(dir))
            .unwrap_or_else(|e| panic!("{dir}: {e}"))
            .map(|e| e.expect("entry").path())
            .filter(|p| p.extension().is_some_and(|x| x == "pw"))
            .collect();
        files.sort();
        for p in files {
            out.push((
                p.file_name().unwrap().to_string_lossy().to_string(),
                std::fs::read_to_string(&p).expect("read"),
            ));
        }
    }
    let domain = root.join("examples/domain.pw");
    out.push((
        "domain.pw".to_string(),
        std::fs::read_to_string(&domain).expect("domain.pw"),
    ));
    out
}

/// The codes one rejected fixture produces, checked as its own program.
fn codes_for(fixture: &str) -> BTreeSet<&'static str> {
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    let path = root.join("examples/rejected").join(fixture);
    let src = std::fs::read_to_string(&path).unwrap_or_else(|e| panic!("{fixture}: {e}"));
    let mut files = library();
    files.push((fixture.to_string(), src));
    check_sources(&files)
        .into_iter()
        .filter(|(n, _)| n == fixture)
        .flat_map(|(_, ds)| ds.into_iter().map(|d| d.code))
        .collect()
}

/// The codes a synthetic program produces, with the effect vocabulary in scope.
fn codes(src: &str) -> BTreeSet<&'static str> {
    check_sources(&[
        ("effects.pw".to_string(), support::VOCABULARY.to_string()),
        ("t.pw".to_string(), src.to_string()),
    ])
    .into_iter()
    .flat_map(|(_, ds)| ds.into_iter().map(|d| d.code))
    .collect()
}

// --- the corpus rows ---------------------------------------------------------

#[test]
fn the_resume_capture_verdicts_the_corpus_carries() {
    // The two fixtures the policy exists for, and the exact code each produces.
    // A refactor that moved either one to the other's code, or to no code at
    // all, would still leave both files rejected and the corpus green.
    assert!(
        codes_for("R-010-nonserializable-handler-capture.pw").contains("PW5008"),
        "R-010 captures a live connection: the value IS the open socket"
    );
    assert!(
        codes_for("R-030-private-data-in-resume-manifest.pw").contains("PW5007"),
        "R-030 captures a session-scoped value, which the manifest ships publicly"
    );

    // And the discrimination: neither fixture produces the OTHER's code. The
    // two rules answer different questions from different facts, and a shared
    // analysis that collapsed them would pass a test asserting only presence.
    assert!(
        !codes_for("R-010-nonserializable-handler-capture.pw").contains("PW5007"),
        "a resource is not a privacy failure"
    );
    assert!(
        !codes_for("R-030-private-data-in-resume-manifest.pw").contains("PW5008"),
        "a session-scoped record is perfectly serializable"
    );
}

// --- the synthetic neighbours ------------------------------------------------
//
// Each is one step from a rejected case, and each must stay on its own side of
// the line. Together they say the analysis discriminates rather than that it
// fires.

const DOMAIN: &str = "\
module boundary.domain

type Store = Store { id: Int, name: String }
type StoreError = StoreError { why: String }
opaque type StoreId = String
opaque type SessionId = String
type Cart = Cart { line_count: Int }
type CartError = CartError { why: String }
opaque type OpenTransaction = String

fn begin() -> OpenTransaction !{ resource.acquire<OpenTransaction> } { todo }
fn finish(t: OpenTransaction) -> Int !{ resource.release<OpenTransaction> } { 0 }
";

fn program(rest: &str) -> String {
    format!("{DOMAIN}\n{rest}")
}

#[test]
fn a_resource_capture_is_refused_and_a_key_capture_is_not() {
    // The distinction R-010 turns on, in both directions. The repair the
    // diagnostic suggests — capture the key, acquire again on the other side —
    // has to actually be accepted, or the rule is unactionable.
    let refused = codes(&program(
        "\
page Checkout() {
    let t = begin()
    view { <button on:press={resumable(captures = { t }) => finish(t)}>go</button> }
}
",
    ));
    assert!(refused.contains("PW5008"), "{refused:?}");

    let accepted = codes(&program(
        "\
page Checkout(id: StoreId) {
    view { <button on:press={resumable(captures = { id }) => 1}>go</button> }
}
",
    ));
    assert!(
        !accepted.contains("PW5008"),
        "an opaque key is exactly what the repair asks for: {accepted:?}"
    );
}

#[test]
fn a_scoped_producer_makes_its_type_private_and_an_unscoped_one_does_not() {
    // R-030's fact: `Cart` is not private in its own declaration. It is private
    // because only a `session query` produces one. The neighbour is the same
    // record produced by an ordinary query, and it must cross freely — or the
    // rule is "records may not be captured", which is not the rule.
    // R-030's shape: an annotated parameter, so the capture's type is nameable
    // and the only question left is whether the value may cross.
    let refused = codes(&program(
        "\
session query Basket(s: SessionId) -> Result<Cart, CartError>
    cache private
{
    todo
}

view Summary(cart: Cart) !{} {
    <button on:press={resumable(captures = { cart }) => cart.line_count}>go</button>
}
",
    ));
    assert!(refused.contains("PW5007"), "{refused:?}");

    // The same record, the same capture, an unscoped producer. Everything
    // about the view is identical; only the declaration that makes a `Cart`
    // differs, and it is in the other half of the file.
    let accepted = codes(&program(
        "\
query Basket(id: StoreId) -> Result<Cart, CartError>
    cache shared
{
    todo
}

view Summary(cart: Cart) !{} {
    <button on:press={resumable(captures = { cart }) => cart.line_count}>go</button>
}
",
    ));
    assert!(
        !accepted.contains("PW5007"),
        "the same record, produced by an unscoped query: {accepted:?}"
    );
}

#[test]
fn a_capture_with_no_nameable_type_is_its_own_verdict() {
    // The third answer, and the one a two-valued analysis cannot express: the
    // build cannot determine the type, so it has no schema, so there is no
    // decision to make — as distinct from deciding it may cross.
    let found = codes(&program(
        "\
page P() {
    let x = todo
    view { <button on:press={resumable(captures = { x }) => 1}>go</button> }
}
",
    ));
    assert!(found.contains("PW5016"), "{found:?}");
    assert!(
        !found.contains("PW5008") && !found.contains("PW5007"),
        "an undetermined type is not a resource and not private: {found:?}"
    );
}

#[test]
fn a_handler_that_captures_nothing_is_not_a_boundary_question() {
    // The control that keeps every assertion above meaningful. If the analysis
    // reported something for every handler, three of the four tests here would
    // pass for a reason unrelated to what they name.
    let found = codes(&program(
        "\
page P() {
    view { <button on:press={resumable() => 1}>go</button> }
}
",
    ));
    assert!(
        !found
            .iter()
            .any(|c| ["PW5007", "PW5008", "PW5016"].contains(c)),
        "{found:?}"
    );
}
