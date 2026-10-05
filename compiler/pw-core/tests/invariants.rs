//! **An opaque type states its invariant, and every construction holds it**
//! (ADR-0179).
//!
//! `opaque type PositiveInt = Int where value >= 1`. A construction the build
//! cannot show holds it is refused (PW0622), and an invariant the language
//! cannot read is refused where it is written (PW0623). Every refusal has an
//! accepted neighbour in the same shape: a rule that refused everything would
//! pass every "is refused" assertion.

use pw_core::check::check_sources;
use pw_core::hir::Hir;
use pw_core::resolve::Workspace;
use pw_core::signatures::Signatures;

const POSITIVE: &str = "opaque type PositiveInt = Int where value >= 1\n\n";

/// The diagnostics for one file, `code message` each.
fn diagnostics(src: &str) -> Vec<String> {
    check_sources(&[("t.pw".to_string(), src.to_string())])
        .into_iter()
        .flat_map(|(_, ds)| ds.into_iter().map(|d| format!("{} {}", d.code, d.message)))
        .collect()
}

fn codes(src: &str) -> Vec<String> {
    diagnostics(src)
        .into_iter()
        .map(|d| d.split(' ').next().unwrap_or_default().to_string())
        .collect()
}

/// One function, under `PositiveInt`.
fn with(f: &str) -> String {
    format!("module t\n\n{POSITIVE}{f}\n")
}

#[test]
fn a_literal_is_itself() {
    assert!(codes(&with("fn one() -> PositiveInt { PositiveInt(1) }")).is_empty());
    let zero = diagnostics(&with("fn zero() -> PositiveInt { PositiveInt(0) }"));
    assert_eq!(zero.len(), 1, "{zero:?}");
    assert!(
        zero[0].starts_with("PW0622")
            && zero[0].contains("value >= 1")
            && zero[0].ends_with("it is 0"),
        "{zero:?}"
    );
    assert_eq!(
        codes(&with("fn below() -> PositiveInt { PositiveInt(-5) }")),
        ["PW0622"]
    );
}

#[test]
fn arithmetic_on_bounded_values_is_exact() {
    // Two counts of at least one sum to at least two.
    assert!(
        codes(&with(
            "fn added(a: PositiveInt, b: PositiveInt) -> PositiveInt { PositiveInt(a.value + b.value) }"
        ))
        .is_empty()
    );
    assert!(
        codes(&with(
            "fn doubled(a: PositiveInt) -> PositiveInt { PositiveInt(a.value * 2) }"
        ))
        .is_empty()
    );
    // One fewer than a count may be 0.
    let fewer = diagnostics(&with(
        "fn fewer(n: PositiveInt) -> PositiveInt { PositiveInt(n.value - 1) }",
    ));
    assert_eq!(fewer.len(), 1, "{fewer:?}");
    assert!(fewer[0].ends_with("it is at least 0"), "{fewer:?}");
    // An `Int` nothing bounds is any `Int`.
    let any = diagnostics(&with("fn any(n: Int) -> PositiveInt { PositiveInt(n) }"));
    assert!(any[0].ends_with("it is any `Int`"), "{any:?}");
}

#[test]
fn a_test_narrows_what_it_tests() {
    // In the branch the test holds in ...
    assert!(
        codes(&with(
            "fn positive(n: Int) -> Option<PositiveInt> {\n    if n >= 1 {\n        Some(PositiveInt(n))\n    } else {\n        None\n    }\n}"
        ))
        .is_empty()
    );
    // ... and not in the other.
    let wrong = diagnostics(&with(
        "fn wrong(n: Int) -> Option<PositiveInt> {\n    if n >= 1 {\n        None\n    } else {\n        Some(PositiveInt(n))\n    }\n}",
    ));
    assert_eq!(wrong.len(), 1, "{wrong:?}");
    assert!(wrong[0].ends_with("it is at most 0"), "{wrong:?}");
    // The negation of a failed test holds in the `else`.
    assert!(
        codes(&with(
            "fn negated(n: Int) -> Option<PositiveInt> {\n    if n < 1 {\n        None\n    } else {\n        Some(PositiveInt(n))\n    }\n}"
        ))
        .is_empty()
    );
    // A field read through a local: `n.value`.
    assert!(
        codes(&with(
            "fn fewer(n: PositiveInt) -> Option<PositiveInt> {\n    if n.value > 1 {\n        Some(PositiveInt(n.value - 1))\n    } else {\n        None\n    }\n}"
        ))
        .is_empty()
    );
    // A test of another value narrows nothing of this one.
    assert_eq!(
        codes(&with(
            "fn other(n: Int, m: Int) -> Option<PositiveInt> {\n    if m >= 1 {\n        Some(PositiveInt(n))\n    } else {\n        None\n    }\n}"
        )),
        ["PW0622"]
    );
}

#[test]
fn a_conjunction_narrows_where_it_holds_and_only_there() {
    // Both, written either way round, and the right of `&` reads the left.
    assert!(
        codes(&with(
            "fn small(n: Int) -> Option<PositiveInt> {\n    if 0 < n & n < 100 {\n        Some(PositiveInt(n))\n    } else {\n        None\n    }\n}"
        ))
        .is_empty()
    );
    // Where a conjunction fails, either part may have: nothing is known,
    // and what it would have said, had it held, is not taken.
    assert_eq!(
        codes(&with(
            "fn either(n: Int) -> Option<PositiveInt> {\n    if n < 1 & n > -5 {\n        None\n    } else {\n        Some(PositiveInt(n))\n    }\n}"
        )),
        ["PW0622"]
    );
    assert_eq!(
        codes(&with(
            "fn outside(n: Int) -> Option<PositiveInt> {\n    if n >= 1 & n <= 100 {\n        None\n    } else {\n        Some(PositiveInt(n))\n    }\n}"
        )),
        ["PW0622"]
    );
    // Where a disjunction fails, both parts did.
    assert!(
        codes(&with(
            "fn neither(n: Int) -> Option<PositiveInt> {\n    if n < 1 | n > 100 {\n        None\n    } else {\n        Some(PositiveInt(n))\n    }\n}"
        ))
        .is_empty()
    );
}

#[test]
fn a_let_is_its_initializer_where_the_let_is() {
    assert!(
        codes(&with(
            "fn more(n: PositiveInt) -> PositiveInt {\n    let m = n.value + 2\n    PositiveInt(m)\n}"
        ))
        .is_empty()
    );
    assert_eq!(
        codes(&with(
            "fn less(n: PositiveInt) -> PositiveInt {\n    let m = n.value - 2\n    PositiveInt(m)\n}"
        )),
        ["PW0622"]
    );
}

#[test]
fn a_branch_is_either_of_its_values() {
    assert!(
        codes(&with(
            "fn chosen(b: Bool) -> PositiveInt {\n    PositiveInt(if b { 1 } else { 2 })\n}"
        ))
        .is_empty()
    );
    assert_eq!(
        codes(&with(
            "fn chosen(b: Bool) -> PositiveInt {\n    PositiveInt(if b { 1 } else { 0 })\n}"
        )),
        ["PW0622"]
    );
}

#[test]
fn a_piped_value_is_held_too() {
    assert!(
        codes(&with(
            "fn piped(n: Int) -> Option<PositiveInt> {\n    if n >= 1 {\n        Some(n |> PositiveInt())\n    } else {\n        None\n    }\n}"
        ))
        .is_empty()
    );
    assert_eq!(
        codes(&with(
            "fn piped(n: Int) -> PositiveInt { n |> PositiveInt() }"
        )),
        ["PW0622"]
    );
}

#[test]
fn an_upper_bound_is_held_as_the_lower_is() {
    let percent = "module t\n\nopaque type Percent = Int where value >= 0 & value <= 100\n\n";
    assert!(
        codes(&format!(
            "{percent}fn full() -> Percent {{ Percent(100) }}\n"
        ))
        .is_empty()
    );
    assert_eq!(
        codes(&format!(
            "{percent}fn over() -> Percent {{ Percent(101) }}\n"
        )),
        ["PW0622"]
    );
    // `value < 101` is `value <= 100`.
    let strict = "module t\n\nopaque type Small = Int where value < 101\n\n";
    assert!(codes(&format!("{strict}fn full() -> Small {{ Small(100) }}\n")).is_empty());
    assert_eq!(
        codes(&format!("{strict}fn over() -> Small {{ Small(101) }}\n")),
        ["PW0622"]
    );
    // `10 < value` is `value > 10`, read the other way round.
    let flipped = "module t\n\nopaque type Big = Int where 10 < value\n\n";
    assert!(codes(&format!("{flipped}fn big() -> Big {{ Big(11) }}\n")).is_empty());
    assert_eq!(
        codes(&format!("{flipped}fn small() -> Big {{ Big(10) }}\n")),
        ["PW0622"]
    );
    // `value > 0` is `value >= 1`.
    let above = "module t\n\nopaque type Count = Int where value > 0\n\n";
    assert!(codes(&format!("{above}fn one() -> Count {{ Count(1) }}\n")).is_empty());
    assert_eq!(
        codes(&format!("{above}fn zero() -> Count {{ Count(0) }}\n")),
        ["PW0622"]
    );
}

#[test]
fn an_opaque_type_that_states_nothing_checks_nothing() {
    // ADR-0054's opaque values, as they were.
    assert!(
        codes("module t\n\nopaque type Count = Int\n\nfn zero() -> Count { Count(0) }\n")
            .is_empty()
    );
}

#[test]
fn an_invariant_the_language_cannot_read_is_refused_where_it_is_written() {
    let refused = |inv: &str| codes(&format!("module t\n\nopaque type X = {inv}\n"));
    assert!(refused("Int where value >= 1").is_empty());
    assert!(refused("Int where 1 <= value & value <= 9").is_empty());
    for (inv, why) in [
        // A `String`'s length is read since ADR-0225, and its value is not.
        ("String where value >= 1", "which only an `Int` has"),
        ("Int where value == 3", "compares `value` with `<`"),
        (
            "Int where size(value) > 0",
            "compares `value`, or `String.length(value)`, with an integer",
        ),
        (
            "Int where value >= other",
            "compares `value`, or `String.length(value)`, with an integer",
        ),
        ("Int where value >= 5 & value <= 2", "holds of no value"),
        (
            "Int where value > 9223372036854775807",
            "beyond what an `Int` holds",
        ),
        (
            "Int where value >= -9999999999999999999",
            "beyond what an `Int` holds",
        ),
    ] {
        let found = diagnostics(&format!("module t\n\nopaque type X = {inv}\n"));
        assert!(
            found
                .iter()
                .any(|d| d.starts_with("PW0623") && d.contains(why)),
            "{inv}: {found:?}"
        );
    }
}

#[test]
fn the_store_builds_every_count_it_can_show() {
    // `PositiveInt(1)` in the page and the command, `added` in the cart's
    // transitions, and `fewer` behind its test: the store checks.
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    let mut files = Vec::new();
    for dir in [
        "packages/pw-std",
        "packages/pw-platform-web",
        "examples/lib",
        "examples/store",
    ] {
        for e in std::fs::read_dir(root.join(dir)).expect("dir") {
            let p = e.expect("entry").path();
            if p.extension().is_some_and(|x| x == "pw") {
                files.push((
                    p.to_string_lossy().to_string(),
                    std::fs::read_to_string(&p).expect("read"),
                ));
            }
        }
    }
    let domain = root.join("examples/domain.pw");
    let src = std::fs::read_to_string(&domain).expect("domain");
    assert!(src.contains("opaque type PositiveInt = Int where value >= 1"));
    files.push((domain.to_string_lossy().to_string(), src));
    let found: Vec<String> = check_sources(&files)
        .into_iter()
        .flat_map(|(_, ds)| ds.into_iter().map(|d| format!("{} {}", d.code, d.message)))
        .filter(|d| d.starts_with("PW062"))
        .collect();
    assert!(found.is_empty(), "{found:?}");
}

/// **The contract states where a value from outside must hold it**
/// (ADR-0179): `add_to_cart`'s quantity, and every cart a data layer answers,
/// down to each line's quantity, as the component names them. And an
/// argument's invariant is part of the export's identity, as its
/// authorization is.
#[test]
fn the_store_s_contracts_state_where_a_count_comes_from_outside() {
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    let mut paths = Vec::new();
    for dir in [
        "packages/pw-std",
        "packages/pw-platform-web",
        "examples/lib",
        "examples/store",
    ] {
        let mut ps: Vec<std::path::PathBuf> = std::fs::read_dir(root.join(dir))
            .expect("dir")
            .map(|e| e.expect("entry").path())
            .filter(|p| p.extension().is_some_and(|x| x == "pw"))
            .collect();
        ps.sort();
        paths.extend(ps);
    }
    paths.push(root.join("examples/domain.pw"));
    let hirs: Vec<Hir> = paths
        .iter()
        .map(|p| {
            let src = std::fs::read_to_string(p).expect("read");
            pw_core::lower::lower_file(&src, &pw_syntax::parse_tree(&src).green)
        })
        .collect();
    let refs: Vec<&Hir> = hirs.iter().collect();
    let ws = Workspace::build(&refs);
    let sigs = Signatures::build(&ws, &refs);
    let contracts = pw_core::contract::contracts(&refs, &sigs, &ws);
    let add = contracts
        .iter()
        .find(|c| c.component_id == "store.page.add_to_cart")
        .expect("add_to_cart's contract");
    let export = add.exports[0].component.as_ref().expect("located");
    assert_eq!(export.bounded.len(), 1, "{:?}", export.bounded);
    let quantity = &export.bounded[0];
    assert_eq!(
        (
            quantity.argument,
            quantity.path.is_empty(),
            quantity.ty.as_str()
        ),
        (1, true, "domain.PositiveInt")
    );
    assert_eq!((quantity.at_least, quantity.at_most), (Some(1), None));
    assert_eq!(quantity.holds, "value >= 1");
    // The data layer's answer: a cart, each line's quantity.
    let answer = add
        .imports
        .iter()
        .find(|i| i.name == "add")
        .expect("carts#add");
    assert_eq!(answer.bounded.len(), 1, "{:?}", answer.bounded);
    assert_eq!(answer.bounded[0].path, ["ok", "lines", "*", "quantity"]);
    // A command whose arguments hold nothing states nothing.
    let clear = contracts
        .iter()
        .find(|c| c.component_id == "store.page.clear_cart")
        .expect("clear_cart's contract");
    assert!(
        clear.exports[0]
            .component
            .as_ref()
            .expect("located")
            .bounded
            .is_empty()
    );
}
