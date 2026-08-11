//! **What each unresolved call is currently worth, frozen before it is
//! repaired.**
//!
//! Architect ruling, 2026-08-10:
//!
//! > Freeze one matrix before repairing them. […] The StorePage discovery tells
//! > us why this matters: resolving a previously nonexistent operation can
//! > change much more than name checking. So when you regenerate E8 evidence,
//! > regenerate it against ALL semantic movements caused by this resolution
//! > repair, not just `current_session`.
//!
//! Four **accepted** programs call names that resolve to nothing:
//!
//! ```text
//! A-005   current_consumer()          in a command body
//! A-014   add_to_cart(..)             in a resumable handler
//! A-013   include_markdown("..")      in a build-time page
//! store   current_session()           in a page and two commands
//! ```
//!
//! Each has been checking clean since it was written. Not because a rule was
//! wrong, but because **no rule asked**: every analysis produces an answer for a
//! call it cannot resolve, and each of those answers is indistinguishable from
//! *this call is harmless*.
//!
//! ```text
//! inference   contributes no effects
//! privacy     contributes no label
//! placement   contributes no constraint
//! contract    contributes no capability
//! ```
//!
//! This file records what those four programs claim TODAY. After the repair,
//! each row moves or it does not, and every movement gets classified — which is
//! the only way "we fixed an import" can be told apart from "the declared
//! authority of an accepted program changed".
//!
//! # It asserts today's values, so it fails on the repair
//!
//! Deliberately. `docs/RISK_QUEUE.md`'s admissibility rule wants the instrument
//! to predate the change; a matrix that merely *printed* would let a repair land
//! with nobody comparing. This goes red, and the diff is the classification.

use std::collections::BTreeSet;

use pw_core::contract::{ComponentContract, contracts};
use pw_core::hir::Hir;
use pw_core::lower::lower_file;
use pw_core::resolve::Workspace;
use pw_core::signatures::Signatures;
use pw_syntax::parse_tree;

/// The shared library every accepted fixture is checked against.
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
    out.push((
        "domain.pw".to_string(),
        std::fs::read_to_string(root.join("examples/domain.pw")).expect("domain.pw"),
    ));
    out
}

fn contracts_for(extra: &[&str]) -> Vec<ComponentContract> {
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    let mut files = library();
    for rel in extra {
        files.push((
            rel.rsplit('/').next().unwrap().to_string(),
            std::fs::read_to_string(root.join(rel)).unwrap_or_else(|e| panic!("{rel}: {e}")),
        ));
    }
    let hirs: Vec<Hir> = files
        .iter()
        .map(|(_, s)| lower_file(s, &parse_tree(s).green))
        .collect();
    let refs: Vec<&Hir> = hirs.iter().collect();
    let ws = Workspace::build(&refs);
    let sigs = Signatures::build(&ws, &refs);
    contracts(&refs, &sigs, &ws)
}

fn of<'a>(cs: &'a [ComponentContract], id: &str) -> &'a ComponentContract {
    cs.iter().find(|c| c.component_id == id).unwrap_or_else(|| {
        panic!(
            "no contract for `{id}`; have {:?}",
            cs.iter().map(|c| &c.component_id).collect::<Vec<_>>()
        )
    })
}

fn caps(c: &ComponentContract) -> BTreeSet<String> {
    c.required_capabilities.iter().map(|k| k.name()).collect()
}

// --- the four rows -----------------------------------------------------------

/// **A-005 — `current_consumer()` in a command body.**
///
/// The fixture's subject is idempotency, and its body reads a consumer identity
/// that names nothing. The command's declared authority today is whatever
/// `Carts.add` contributes and nothing from the unresolved call.
#[test]
fn a005_idempotent_command_before_repair() {
    let cs = contracts_for(&["examples/accepted/A-005-idempotent-command.pw"]);
    let c = of(&cs, "cart.commands.add_to_cart");
    assert_eq!(
        caps(c),
        BTreeSet::from(["database.write<Carts>".to_string()]),
        "TODAY. `current_consumer()` resolves to nothing and contributes \
         nothing. If it turns out to be a platform operation with a session or \
         user effect, this row moves and the movement is the finding."
    );
    assert_eq!(c.allowed_placements, ["origin"]);
}

/// **A-014 — `add_to_cart(..)` inside a resumable handler.**
///
/// The row with no capability column, and the reason is worth recording: A-014
/// declares a `view`, and a view produces **no contract at all** —
/// `component_kind` covers query, command, page and component. So the
/// before-state here is not "requires nothing"; it is "there is nothing to ask".
///
/// That makes the fixture's own subject unmeasurable from the contract side.
/// It is a content-addressed resumable handler, and the handler calls a name
/// that resolves to nothing — so whatever authority `add_to_cart` would carry
/// is attributed to no one, and the E8-0 property it looks like evidence for
/// (a page does not inherit its handler's authority) is not being exercised.
#[test]
fn a014_content_addressed_handler_before_repair() {
    let cs = contracts_for(&["examples/accepted/A-014-content-addressed-resumable-handler.pw"]);
    assert!(
        !cs.iter()
            .any(|c| c.component_id.starts_with("store.add_button")),
        "TODAY: a `view` produces no contract, so this fixture contributes no          capability claim in either direction. If `view` ever becomes a          component kind, this row gains a capability column and the handler's          unresolved `add_to_cart` becomes visible as an absence. Got {:?}",
        cs.iter().map(|c| &c.component_id).collect::<Vec<_>>()
    );

    // And the call itself. Asserted on the source, because there is no derived
    // artifact to read it from — which is the finding.
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    let src = std::fs::read_to_string(
        root.join("examples/accepted/A-014-content-addressed-resumable-handler.pw"),
    )
    .expect("A-014");
    assert!(src.contains("add_to_cart("), "the handler calls it");
    assert!(
        !src.contains("import cart") && !src.contains("{ add_to_cart }"),
        "and imports it from nowhere"
    );
}

/// **A-013 — `include_markdown("..")` in a build-time page.**
///
/// Reading a file at build time is plausibly a real platform operation with a
/// real effect. Today it contributes nothing, so the page claims build-time
/// determinism without anything checking what it reads.
#[test]
fn a013_build_time_page_before_repair() {
    let cs = contracts_for(&["examples/accepted/A-013-build-time-deterministic-page.pw"]);
    let c = cs
        .iter()
        .find(|c| c.exports.iter().any(|e| e.kind == "page"))
        .unwrap_or_else(|| {
            panic!(
                "{:?}",
                cs.iter().map(|c| &c.component_id).collect::<Vec<_>>()
            )
        });
    assert!(
        caps(c).is_empty(),
        "TODAY: nothing. If `include_markdown` becomes a build-time platform \
         operation it will contribute an effect, and a page declaring \
         `placement build` will have to satisfy the determinism rule against a \
         real read rather than against silence. Got {:?}",
        caps(c)
    );
    // MOVED, 2026-08-10, and not by the resolution repair this file is about.
    //
    // It read `["build", "browser", "edge", "origin"]` — placeable everywhere,
    // including the browser, for a page whose subject is build-time
    // determinism. The evidence-reachability audit found why: `contract.rs`
    // discarded the author's pinned world when building its placement demand,
    // so `placement build` reached the checker and never reached the artifact.
    //
    // Classification: this row moved because the CONTRACT was repaired, not
    // because the program changed. `include_markdown` still resolves to
    // nothing, and the capability assertion above is still the before-state of
    // the repair this file exists for.
    assert_eq!(
        c.allowed_placements,
        ["build"],
        "the pin now reaches the artifact — but nothing yet checks that what \
         the page READS is build-known, which is the `include_markdown` ruling"
    );
}

/// **The store — `current_session()` in a page and two commands. REPAIRED.**
///
/// The row the architect ruled on first, and the classification of its
/// movement:
///
/// ```text
/// StorePage     {}                        -> {session.read}
/// add_to_cart   {database.write<Carts>}   -> {+ session.read}
/// clear_cart    {database.write<Carts>}   -> {+ session.read}
/// placements    origin                       unchanged
/// ```
///
/// The repair is one line — `import context.{ current_session }` — and
/// `packages/pw-platform-web/context.pw` had declared the function since E2C.
/// Nothing was invented; a name that already existed was brought into scope.
///
/// **What moved is a claim, not a spelling.** `StorePage` renders by building
/// a query key from the session, so it reads the session to render. The E8
/// evidence saying a page renders without authority described a page whose
/// call named nothing. That is the whole reason the matrix was frozen first.
#[test]
fn store_page_after_repair() {
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    let mut extra: Vec<String> = Vec::new();
    for e in std::fs::read_dir(root.join("examples/store")).expect("store") {
        let p = e.expect("entry").path();
        if p.extension().is_some_and(|x| x == "pw") {
            extra.push(format!(
                "examples/store/{}",
                p.file_name().unwrap().to_string_lossy()
            ));
        }
    }
    extra.sort();
    let refs: Vec<&str> = extra.iter().map(String::as_str).collect();
    let cs = contracts_for(&refs);

    let page = of(&cs, "store.page.StorePage");
    assert_eq!(
        caps(page),
        BTreeSet::from(["session.read".to_string()]),
        "the page reads the session to render, and now says so"
    );
    assert_eq!(page.allowed_placements, ["origin"]);

    let add = of(&cs, "store.page.add_to_cart");
    assert_eq!(
        caps(add),
        BTreeSet::from([
            "database.write<Carts>".to_string(),
            "session.read".to_string()
        ]),
        "the write AND the session read — which is what the backend saw as two \
         host calls before the import was applied"
    );

    // The discriminator. `Menu` is on the same page, in the same module, and
    // calls no context operation — so the movement above is attributable to
    // the repair rather than to something that moved every contract.
    assert_eq!(
        caps(of(&cs, "store.page.Menu")),
        BTreeSet::from(["database.read<Menus>".to_string()]),
        "an unrelated query in the same module did not move"
    );
}

// --- the property that makes the matrix worth freezing ------------------------

/// **Nothing above is a statement that these programs are correct.**
///
/// Every row is a value derived, in part, from a call that names nothing — and
/// each analysis that contributed to it did so by contributing NOTHING. The
/// matrix exists so that after the repair, a row that did not move is as
/// interesting as one that did.
#[test]
fn every_frozen_row_is_derived_from_a_call_that_resolves_to_nothing() {
    // The four programs, and the name each one calls into the void. Listed here
    // rather than re-derived, so this file states its own subject: if a repair
    // lands and this list is stale, the test that reads it fails.
    // `examples/store/app.pw` / `current_session` was the fourth entry and is
    // REPAIRED — see `store_page_after_repair` for the classification. It is
    // named here rather than deleted so the list reads as a work-list with one
    // item struck through, not as a list that was always three long.
    const UNRESOLVED: &[(&str, &str)] = &[
        (
            "examples/accepted/A-005-idempotent-command.pw",
            "current_consumer",
        ),
        (
            "examples/accepted/A-014-content-addressed-resumable-handler.pw",
            "add_to_cart",
        ),
        (
            "examples/accepted/A-013-build-time-deterministic-page.pw",
            "include_markdown",
        ),
    ];
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    for (file, name) in UNRESOLVED {
        let src =
            std::fs::read_to_string(root.join(file)).unwrap_or_else(|e| panic!("{file}: {e}"));
        assert!(
            src.contains(&format!("{name}(")),
            "{file} no longer calls `{name}` — the matrix above is describing a \
             program that has changed, and its rows are no longer the \
             before-state of anything"
        );
        // And it is still unimported. A repair adds an import or a declaration;
        // either way this stops holding, and the row above has to be
        // reclassified rather than silently kept.
        assert!(
            !src.contains(&format!("{{ {name} }}")) && !src.contains(&format!("{name} }}")),
            "{file} now imports `{name}`. Reclassify the frozen row: what did \
             its effects, capabilities, label and placement become?"
        );
    }
}
