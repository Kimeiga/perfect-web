//! **E9-K — Koka effect-oracle conformance.**
//!
//! The gate item that replaced "differential tests match Koka for the common
//! semantic subset" on 2026-08-08, after that one was measured and found
//! obsolete: the common subset is empty, because ADR-0011 made Koka an
//! effects-only oracle and ADR-0015 scoped `emit-koka` to a pure subset the
//! application language does not live in.
//!
//! > **E9-K.** For the effect-language subset intentionally shared with Koka,
//! > Pleris agrees on higher-order effect propagation, row polymorphism,
//! > pure/total computations, and selective effect discharge. Every recorded
//! > intentional divergence remains explicitly tested as a divergence.
//!
//! ```text
//! ordinary Pleris corpus          no Koka parity claim
//! effect-semantics oracle cases   differential against real Koka 3.2.3
//! known intentional differences   must remain different, for the reason
//! ```
//!
//! # What is on each side
//!
//! The Koka half is `just rq-row-polymorphism`, whose evidence at
//! `docs/evidence/E0/spike-koka-row-polymorphism.txt` records OUTCOME 1: a
//! clean pass on higher-order effect propagation, with propagation automatic
//! through unannotated generic helpers. That spike enumerates the failure modes
//! it was pre-registered to detect, and none occurred.
//!
//! This file is the Pleris half: **the same four properties, asserted on Pleris
//! programs.** Conformance is the pair. A property this project cannot yet
//! express is recorded as such rather than claimed — see
//! `selective_discharge_is_not_yet_a_pleris_property`.
//!
//! The divergence half is `tests/differential_vs_koka.rs`, unchanged and still
//! required: those four are the reasons Koka is an effects-only oracle, and a
//! divergence that quietly became an agreement would mean the project had
//! regressed to Koka's guarantee.

use std::collections::BTreeSet;

use pw_core::check::check_sources;

mod support;

/// A program checked WITH the shared library, which is where `List` and
/// `ElementRef` come from.
fn codes_with_library(src: &str) -> BTreeSet<&'static str> {
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    let mut files: Vec<(String, String)> = Vec::new();
    for dir in [
        "examples/lib",
        "packages/pw-std",
        "packages/pw-platform-web",
    ] {
        let mut paths: Vec<std::path::PathBuf> = std::fs::read_dir(root.join(dir))
            .unwrap_or_else(|e| panic!("{dir}: {e}"))
            .map(|e| e.expect("entry").path())
            .filter(|p| p.extension().is_some_and(|x| x == "pw"))
            .collect();
        paths.sort();
        for p in paths {
            files.push((
                p.file_name().unwrap().to_string_lossy().to_string(),
                std::fs::read_to_string(&p).expect("read"),
            ));
        }
    }
    files.push((
        "domain.pw".to_string(),
        std::fs::read_to_string(root.join("examples/domain.pw")).expect("domain.pw"),
    ));
    files.push(("t.pw".to_string(), src.to_string()));
    check_sources(&files)
        .into_iter()
        .filter(|(n, _)| n == "t.pw")
        .flat_map(|(_, ds)| ds.into_iter().map(|d| d.code))
        .collect()
}

fn codes(src: &str) -> BTreeSet<&'static str> {
    check_sources(&[
        ("effects.pw".to_string(), support::VOCABULARY.to_string()),
        ("t.pw".to_string(), src.to_string()),
    ])
    .into_iter()
    .flat_map(|(_, ds)| ds.into_iter().map(|d| d.code))
    .collect()
}

const DOMAIN: &str = "\
module oracle.domain

type Store = Store { id: Int, name: String }
type StoreError = StoreError { why: String }
opaque type StoreId = String
";

fn program(rest: &str) -> String {
    format!("{DOMAIN}\n{rest}")
}

// --- 1. higher-order effect propagation --------------------------------------

/// **An effect crosses a generic callback, without an annotation saying so.**
///
/// The Koka side is the row-polymorphism spike's central finding: *propagation
/// is AUTOMATIC through unannotated generic helpers*. The failure it was
/// pre-registered to detect — "higher-order effect disappears" — did not occur.
///
/// The Pleris side is here, in the shape the language actually has: a callback
/// handed to `List.map`, which declares no effect row of its own. R-037 is the
/// corpus fixture; this asserts the property beside its control so the pair is
/// readable as a pair.
///
/// **A function-typed PARAMETER is not that shape.** `f: fn(StoreId) -> Store`
/// does not parse — Pleris has lambdas and higher-order platform functions, and
/// not first-class function types in a user signature. My first version of this
/// test wrote one and read the parse failure as a propagation gap, which would
/// have been a false finding about the effect system.
#[test]
fn an_effect_crosses_an_unannotated_generic_helper() {
    let found = codes_with_library(
        "\
module oracle.higher_order

import List

view Badge(items: List<ElementRef>) !{} {
    let widths = items |> List.map(fn(el) el.getBoundingClientRect().width)
    <ul>{#each widths as w (w)}<li style:width={w} />{/each}</ul>
}
",
    );
    assert!(
        found.contains("PW0401") || found.contains("PW5001"),
        "`layout.measure` reaches the view through a helper that never mentions \
         it: {found:?}"
    );
}

/// The discriminating control. Same shape, nothing effectful inside the lambda.
///
/// Without it the test above passes for a checker that reports every view
/// calling a higher-order function — which is not propagation, it is noise.
#[test]
fn the_same_helper_with_a_pure_callback_is_clean() {
    let found = codes_with_library(
        "\
module oracle.higher_order_pure

import List

view Badge(items: List<Int>) !{} {
    let doubled = items |> List.map(fn(n) n)
    <ul>{#each doubled as w (w)}<li>{w}</li>{/each}</ul>
}
",
    );
    assert!(
        !found.contains("PW0401") && !found.contains("PW5001"),
        "a pure callback through the same helper is not an effect: {found:?}"
    );
}

// --- 2. row polymorphism ------------------------------------------------------

/// **One helper carries whichever effects its callback has, not a fixed set.**
///
/// The property that makes the row a variable rather than a constant. `List.map`
/// is used at two different rows in one program and each caller sees its own — a
/// helper with a FIXED row would have to carry both everywhere, and the pure
/// view would be rejected for a measurement it never performs.
#[test]
fn one_generic_helper_serves_two_different_effect_rows() {
    let pure = codes_with_library(
        "\
module oracle.two_rows_pure

import List

view Pure(items: List<Int>) !{} {
    let doubled = items |> List.map(fn(n) n)
    <ul>{#each doubled as w (w)}<li>{w}</li>{/each}</ul>
}
",
    );
    assert!(
        !pure.contains("PW0401") && !pure.contains("PW5001"),
        "the pure use of `List.map` carries no row from anyone else's use: {pure:?}"
    );

    // The SAME helper, a measuring callback, and the view is rejected. Two
    // callers, two answers, one helper with no row of its own.
    let measuring = codes_with_library(
        "\
module oracle.two_rows_measuring

import List

view Measuring(items: List<ElementRef>) !{} {
    let widths = items |> List.map(fn(el) el.getBoundingClientRect().width)
    <ul>{#each widths as w (w)}<li style:width={w} />{/each}</ul>
}
",
    );
    assert!(
        measuring.contains("PW0401") || measuring.contains("PW5001"),
        "{measuring:?}"
    );
}

// --- 3. pure / total computations ---------------------------------------------

/// **`!{}` means nothing, and is checked.**
///
/// Koka's `total` is what makes its exhaustiveness check non-vacuous — E0
/// finding F-8, and the reason `emit-koka` writes `total` on every function it
/// generates. Pleris's equivalent is an empty row, and it must be a claim the
/// compiler enforces rather than a comment.
#[test]
fn an_empty_row_is_enforced_and_an_omitted_one_is_not_a_loophole() {
    let declared_pure = codes(&program(
        "\
fn read(id: StoreId) -> Store !{ database.read<Store> } { todo }

view Badge(id: StoreId) !{} {
    <p>{read(id).name}</p>
}
",
    ));
    assert!(
        declared_pure.contains("PW0401") || declared_pure.contains("PW5001"),
        "`!{{}}` is a claim, not a decoration: {declared_pure:?}"
    );

    // And omitting the row is not a way around it. A view that writes no row at
    // all still may not reach the database — the effects are INFERRED, so
    // silence is not permission.
    let no_row = codes(&program(
        "\
fn read(id: StoreId) -> Store !{ database.read<Store> } { todo }

view Badge(id: StoreId) {
    <p>{read(id).name}</p>
}
",
    ));
    assert!(
        no_row.contains("PW0401") || no_row.contains("PW5001") || no_row.contains("PW5002"),
        "omitting the row does not grant the effect: {no_row:?}"
    );
}

// --- 4. selective effect discharge --------------------------------------------

/// **Pleris does not have this yet, and saying so is the point.**
///
/// The Koka spike proves selective discharge — handle one effect while
/// preserving the rest of the row — and pre-registered its absence as an E1
/// FAIL that did not occur. Pleris has no effect handler: `handler` in the
/// grammar is a resumable-handler POLICY, which is a different thing entirely
/// (E7's `resumable(captures = ..)`, not an algebraic effect handler).
///
/// E9-K asks for agreement on the shared subset. Discharge is not in the shared
/// subset today, because one side does not implement it — and a conformance
/// suite that quietly omitted the property it cannot demonstrate would be
/// claiming a subset larger than the one it tests.
///
/// **This test asserts the absence**, so that implementing discharge makes it
/// fail and forces the conformance pair to be written rather than forgotten.
#[test]
fn selective_discharge_is_not_yet_a_pleris_property() {
    // **The concrete assertion.** `packages/pw-platform-web/effects.pw` declares
    // the effect vocabulary; nothing in it declares a handler, and there is no
    // discharge operation for a program to write.
    //
    // An earlier version of this test also ran a program containing the word
    // `handle` and asserted it produced no particular code. That asserted
    // nothing: the word parses as an ordinary name, so the program was checking
    // that an identifier is an identifier.
    let platform = std::fs::read_to_string(
        std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../../packages/pw-platform-web/effects.pw"),
    )
    .expect("the platform effects");
    assert!(
        !platform.contains("handler ") && !platform.contains("handle "),
        "the platform declares an effect handler — selective discharge may now \
         be expressible, and E9-K's fourth property needs a real conformance \
         pair instead of this test"
    );
}

// --- the divergences stay divergences -----------------------------------------

/// **E9-K's second half**, asserted as a fact about the test suite rather than
/// re-derived here.
///
/// > Every recorded intentional divergence remains explicitly tested as a
/// > divergence.
///
/// `tests/differential_vs_koka.rs` owns the four. This asserts they are still
/// there and still named — a conformance suite that grew while the divergence
/// suite was quietly deleted would read as increasing rigour while removing it.
#[test]
fn every_recorded_divergence_is_still_tested_as_one() {
    let text = std::fs::read_to_string(
        std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/differential_vs_koka.rs"),
    )
    .expect("the divergence suite");
    for named in [
        "nominal_wrappers_stay_distinct_where_koka_erases_them",
        "option_and_list_stay_distinct_where_koka_conflates_them",
        "pw_rejects_the_match_koka_accepts_under_exn",
        "the_result_does_not_depend_on_any_effect_row",
    ] {
        assert!(
            text.contains(named),
            "`{named}` is gone from the divergence suite. ADR-0011 records why \
             it exists; deleting it would let the project regress to Koka's \
             guarantee with nothing saying so."
        );
    }
}
