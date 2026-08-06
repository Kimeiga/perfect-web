//! Generating Koka from `.pw` source (ADR-0015).
//!
//! These tests are hermetic: they assert what is generated, never that it
//! compiles. Compiling and *executing* it needs the pinned toolchain and lives
//! in `spikes/pw-to-koka/run.sh`, whose evidence is
//! `docs/evidence/E2/spike-pw-to-koka.txt`. Splitting them this way keeps
//! `just ci` runnable without Koka while the gate claim still rests on a real
//! execution.

use pw_core::koka::lower_module;
use pw_core::lower::lower_file;
use pw_syntax::parse_tree;

fn generate(src: &str) -> pw_core::koka::Lowered {
    let p = parse_tree(src);
    assert!(p.ok(), "fixture must parse: {:?}", p.errors);
    let hir = lower_file(src, &p.green);
    let name = hir
        .modules
        .iter()
        .next()
        .map(|(_, m, _)| m.name.clone())
        .unwrap_or_default();
    lower_module(&hir, &name)
}

fn fixture() -> String {
    let p = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../examples/koka/pricing.pw");
    std::fs::read_to_string(p).expect("the Koka fixture exists")
}

#[test]
fn the_fixture_lowers_completely() {
    // If the fixture ever stops lowering in full, the spike's execution claim
    // silently narrows. This is the tripwire for that.
    let out = generate(&fixture());
    assert!(
        out.skipped.is_empty(),
        "the fixture must lower with nothing skipped, got {:?}",
        out.skipped
    );
    assert_eq!(
        out.emitted,
        [
            "CartLine",
            "OrderState",
            "line_total",
            "with_delivery",
            "label"
        ],
        "every declaration must reach the output"
    );
}

#[test]
fn every_generated_function_declares_total() {
    // The load-bearing annotation. Under any other effect row Koka's
    // exhaustiveness check passes vacuously (E0 finding F-8), so generated code
    // without `total` would compile while proving nothing.
    let out = generate(&fixture());
    let funs: Vec<&str> = out
        .source
        .lines()
        .filter(|l| l.starts_with("pub fun "))
        .collect();
    assert_eq!(funs.len(), 3, "expected three functions, got {funs:?}");
    for f in &funs {
        assert!(f.contains(" : total "), "not total: {f}");
    }
}

#[test]
fn names_are_mapped_the_way_koka_requires() {
    // Verified against Koka 3.2.3 by compiling a probe, not read from docs:
    // type names lowercase, a record's constructor is the struct name with its
    // first letter capitalised.
    let out = generate(&fixture()).source;
    assert!(out.contains("module store_pricing"), "{out}");
    assert!(out.contains("pub value struct cart_line"), "{out}");
    assert!(out.contains("pub type order_state"), "{out}");
    assert!(out.contains("  quantity : int"), "{out}");
    assert!(
        out.contains("pub fun line_total( line : cart_line ) : total int"),
        "{out}"
    );
    // Union constructors keep their names; they are already capitalised.
    assert!(out.contains("  Confirmed( field0 : string )"), "{out}");
}

#[test]
fn a_record_constructor_call_is_renamed_but_a_union_constructor_is_not() {
    let out = generate(
        "module m\n\
         type Point = Point { x: Int, y: Int }\n\
         type CartLine = CartLine { qty: Int }\n\
         type Dir = | North | South\n\
         fn origin() -> Point !{} { Point(0, 0) }\n\
         fn one() -> CartLine !{} { CartLine(1) }\n\
         fn up() -> Dir !{} { North }\n",
    )
    .source;
    // A record's constructor is its Koka struct name with the first letter
    // capitalised, so a multi-word name is NOT left as written.
    assert!(out.contains("Point( 0, 0 )"), "{out}");
    assert!(out.contains("Cart_line( 1 )"), "{out}");
    assert!(
        !out.contains("CartLine("),
        "the pw spelling must not survive:\n{out}"
    );
    // A union constructor is already in Koka's shape and is left alone.
    assert!(out.contains("  North\n"), "{out}");
}

#[test]
fn a_declaration_outside_the_subset_is_skipped_with_a_reason() {
    // A backend that covers a subset must say which subset. Silently omitting a
    // declaration would make the output look like a complete translation.
    let out = generate(
        "module m\n\
         fn pure_one() -> Int !{} { 1 }\n\
         fn impure() -> Int !{ database.read } { 2 }\n\
         view V() !{} { <p>hi</p> }\n",
    );
    assert_eq!(out.emitted, ["pure_one"]);
    let reasons: Vec<(&str, &str)> = out
        .skipped
        .iter()
        .map(|s| (s.name.as_str(), s.reason))
        .collect();
    assert!(
        reasons.contains(&("impure", "a non-empty effect row is outside the subset")),
        "{reasons:?}"
    );
    assert!(reasons.iter().any(|(n, _)| *n == "V"), "{reasons:?}");
}

#[test]
fn a_body_the_subset_cannot_express_skips_the_whole_function() {
    // Partial output would be worse than none: a function whose body lost a
    // statement would still compile and would compute the wrong thing.
    let out = generate(
        "module m\n\
         fn piped(c: Int) -> Int !{} {\n    c |> double\n}\n",
    );
    assert!(out.emitted.is_empty(), "{:?}", out.emitted);
    assert_eq!(out.skipped.len(), 1);
    assert_eq!(
        out.skipped[0].reason,
        "a pipeline, transition or assignment"
    );
    assert!(
        !out.source.contains("piped"),
        "a skipped function must not appear at all:\n{}",
        out.source
    );
}

#[test]
fn the_generated_source_matches_what_the_spike_compiled() {
    // The spike proves the output *executes*. This pins the output it executed,
    // so a generator change that would break the spike fails here first —
    // in `just ci`, without needing the Koka toolchain.
    let out = generate(&fixture()).source;
    for want in [
        "pub fun line_total( line : cart_line ) : total int",
        "  (line.quantity * line.unit_price)",
        "pub fun with_delivery( subtotal : int, distance_m : int ) : total int",
        "  val base = 299",
        "pub fun label( state : order_state ) : total string",
        "  match state",
        "    Draft -> \"draft\"",
        "    Confirmed( reference ) -> reference",
    ] {
        assert!(out.contains(want), "missing {want:?} from:\n{out}");
    }
}

#[test]
fn the_corpus_reports_honestly_how_little_of_it_lowers() {
    // The number that keeps the gate claim from being overread: most of the
    // corpus is outside the pure subset, by design, and the backend says so.
    let dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../examples/accepted");
    let mut emitted = 0;
    let mut skipped = 0;
    for e in std::fs::read_dir(dir).expect("accepted/") {
        let p = e.expect("entry").path();
        if p.extension().is_none_or(|x| x != "pw") {
            continue;
        }
        let out = generate(&std::fs::read_to_string(&p).expect("read"));
        emitted += out.emitted.len();
        skipped += out.skipped.len();
    }
    assert!(emitted > 0, "some accepted declarations must lower");
    assert!(
        skipped > emitted,
        "the subset is meant to be small: {emitted} emitted, {skipped} skipped"
    );
}
