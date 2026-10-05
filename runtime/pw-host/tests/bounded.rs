//! **The host holds a value to the invariants its contract states**
//! (ADR-0179): each at a path into the value, as the component names it. The
//! walk itself, on values built by hand: each kind of step, the places it
//! does not reach, and the value of another shape than the path says.

#![cfg(feature = "engine")]

use pw_host::engine::holds;
use pw_host::{Bounded, Measure};
use wasmtime::component::Val;

fn positive(argument: usize, path: &[&str]) -> Bounded {
    Bounded {
        argument,
        path: path.iter().map(|s| s.to_string()).collect(),
        ty: "domain.PositiveInt".into(),
        holds: "value >= 1".into(),
        measure: Measure::Value,
        at_least: Some(1),
        at_most: None,
    }
}

/// A post's text (ADR-0225): from 1 to 280 code points.
fn post_text(argument: usize) -> Bounded {
    Bounded {
        argument,
        path: Vec::new(),
        ty: "feed.app.PostText".into(),
        holds: "String.length(value) >= 1 & String.length(value) <= 280".into(),
        measure: Measure::Length,
        at_least: Some(1),
        at_most: Some(280),
    }
}

fn line(quantity: i64) -> Val {
    Val::Record(vec![
        ("item-id".into(), Val::String("espresso".into())),
        ("quantity".into(), Val::S64(quantity)),
    ])
}

fn cart(quantities: &[i64]) -> Val {
    Val::Result(Ok(Some(Box::new(Val::Record(vec![(
        "lines".into(),
        Val::List(quantities.iter().map(|q| line(*q)).collect()),
    )])))))
}

#[test]
fn an_argument_is_held_to_its_bounds_at_both_ends() {
    let percent = Bounded {
        at_least: Some(0),
        at_most: Some(100),
        holds: "value >= 0 & value <= 100".into(),
        ..positive(0, &[])
    };
    assert!(holds(std::slice::from_ref(&percent), &[Val::S64(0)]).is_ok());
    assert!(holds(std::slice::from_ref(&percent), &[Val::S64(100)]).is_ok());
    for outside in [-1, 101] {
        let why = holds(std::slice::from_ref(&percent), &[Val::S64(outside)]).expect_err("outside");
        assert!(why.starts_with(&format!("value 1 is {outside}")), "{why}");
    }
    // Each check reads the value at its own position.
    let second = positive(1, &[]);
    assert!(holds(std::slice::from_ref(&second), &[Val::S64(0), Val::S64(1)]).is_ok());
    assert!(holds(std::slice::from_ref(&second), &[Val::S64(1), Val::S64(0)]).is_err());
}

#[test]
fn each_item_of_a_list_and_each_field_is_reached() {
    let checks = [positive(0, &["ok", "lines", "*", "quantity"])];
    assert!(holds(&checks, &[cart(&[1, 2, 3])]).is_ok());
    assert!(
        holds(&checks, &[cart(&[])]).is_ok(),
        "no line, nothing to hold"
    );
    let why = holds(&checks, &[cart(&[1, 0, 3])]).expect_err("the second line");
    assert_eq!(
        why,
        "value 1's `ok.lines[1].quantity` is 0, and `domain.PositiveInt` holds `value >= 1`"
    );
}

#[test]
fn a_case_the_path_does_not_name_holds_nothing_to_check() {
    let checks = [positive(0, &["ok", "lines", "*", "quantity"])];
    // A declared error is not the cart.
    let refused = Val::Result(Err(Some(Box::new(Val::Variant(
        "cart-expired".into(),
        None,
    )))));
    assert!(holds(&checks, &[refused]).is_ok());
    let absent = [positive(0, &["some"])];
    assert!(holds(&absent, &[Val::Option(None)]).is_ok());
    assert!(holds(&absent, &[Val::Option(Some(Box::new(Val::S64(0))))]).is_err());
    // A variant's case, by its name, and a payload's field by its position.
    let case = [positive(0, &["unavailable", "1"])];
    let payload = |n: i64| {
        Val::Variant(
            "unavailable".into(),
            Some(Box::new(Val::Tuple(vec![
                Val::String("espresso".into()),
                Val::S64(n),
            ]))),
        )
    };
    assert!(holds(&case, &[payload(2)]).is_ok());
    assert!(holds(&case, &[payload(0)]).is_err());
    assert!(holds(&case, &[Val::Variant("expired".into(), None)]).is_ok());
}

#[test]
fn a_value_of_another_shape_than_the_contract_says_is_refused() {
    // The contract and the value disagree: refused, never passed over.
    let checks = [positive(0, &["quantity"])];
    let why = holds(&checks, &[Val::Record(vec![])]).expect_err("no field");
    assert!(why.contains("has no field `quantity`"), "{why}");
    let why = holds(&[positive(0, &[])], &[Val::String("1".into())]).expect_err("no Int");
    assert!(why.contains("is no `Int`"), "{why}");
    let why = holds(&[positive(3, &[])], &[Val::S64(1)]).expect_err("no value 4");
    assert!(why.contains("no value 4"), "{why}");
}

/// **A `String`'s length is held at both ends, in code points** (ADR-0225),
/// as the language's `String.length` counts it: an emoji is one, whatever
/// its bytes, and its UTF-16 units.
#[test]
fn a_strings_length_is_held_at_both_ends_in_code_points() {
    let check = [post_text(0)];
    let text = |s: &str| [Val::String(s.into())];
    assert!(holds(&check, &text("a")).is_ok());
    assert!(holds(&check, &text(&"a".repeat(280))).is_ok());
    // 280 emoji: 1,120 bytes, 560 UTF-16 units, 280 code points.
    assert!(holds(&check, &text(&"\u{1F600}".repeat(280))).is_ok());
    let why = holds(&check, &text("")).expect_err("empty");
    assert!(why.starts_with("value 1 is 0 code points long"), "{why}");
    let why = holds(&check, &text(&"\u{1F600}".repeat(281))).expect_err("too long");
    assert!(
        why.starts_with("value 1 is 281 code points long, and `feed.app.PostText` holds"),
        "{why}"
    );
    // A length is a `String`'s: another value is refused, as the contract
    // and the value disagree.
    let why = holds(&check, &[Val::S64(3)]).expect_err("no String");
    assert!(why.contains("is no `String`"), "{why}");
}
