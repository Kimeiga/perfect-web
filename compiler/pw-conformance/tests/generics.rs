//! **Generic records, sum types and opaque types, compiled** (ADR-0062).
//!
//! A `Type::Nominal` carried no type arguments until 2026-09-26, so
//! `Box<Int>` had no layout and every generic declaration was refused by the
//! backend. Each query here builds, reads or matches an instance inside a
//! component, and runs through the E8 host beside the answer it must give.

use std::collections::BTreeMap;

use pw_conformance::{Runnable, compile, units};
use pw_host::engine::Val;

const PROGRAM: &str = r#"module g

import List

type Box<T> = Box { value: T, label: String }

type Pair<A, B> = Pair { first: A, second: B }

type Maybe<T> =
    | Nothing
    | Just(T)

opaque type Tagged<T> = List<T>

fn boxed(n: Int) -> Box<Int> { Box { value: n, label: "n" } }

public query Unboxed(n: Int) -> Int { boxed(n).value + 1 }

public query Swapped(a: Int, b: String) -> String {
    let p = Pair { first: a, second: b }
    "{p.second}:{p.first}"
}

public query Two(a: Int, b: String) -> String {
    let ints = Pair { first: a, second: a + 1 }
    let texts = Pair { first: b, second: "{b}{b}" }
    "{ints.first + ints.second} {texts.second}"
}

fn first_of(xs: List<Int>) -> Maybe<Int> {
    match List.get(xs, 0) {
        Some(x) => Maybe.Just(x),
        None => Maybe.Nothing,
    }
}

public query First(xs: List<Int>) -> Int {
    match first_of(xs) {
        Just(x) => x,
        Nothing => -1,
    }
}

public query Nested(n: Int) -> Int {
    let b = Box { value: Box { value: n, label: "in" }, label: "out" }
    b.value.value
}

public query Tags(xs: List<Int>) -> Int {
    let t = Tagged(xs)
    List.length(t.value)
}

public query Justs(xs: List<Int>) -> Int {
    let ms = List.map(xs, Maybe.Just)
    List.fold(ms, 0, (t, m) => match m {
        Just(x) => t + x,
        Nothing => t,
    })
}

public query Labels(xs: List<String>) -> String {
    let boxes = List.map(xs, x => Box { value: x, label: "{x}!" })
    List.fold(boxes, "", (t, b) => "{t}{b.label}")
}
"#;

fn call(id: &str, args: &[Val]) -> Val {
    Runnable::new(compile(&units(&[("g.pw", PROGRAM)]), id))
        .call(&BTreeMap::new(), args)
        .expect("runs")
        .remove(0)
}

fn text(s: &str) -> Val {
    Val::String(s.to_string())
}

fn ints(xs: &[i64]) -> Val {
    Val::List(xs.iter().map(|x| Val::S64(*x)).collect())
}

#[test]
fn a_generic_record_is_built_and_read() {
    assert_eq!(call("g.Unboxed", &[Val::S64(41)]), Val::S64(42));
    assert_eq!(
        call("g.Swapped", &[Val::S64(7), text("seven")]),
        text("seven:7")
    );
}

#[test]
fn two_instances_of_one_record_are_two_layouts() {
    // `Pair<Int, Int>` and `Pair<String, String>` in one body.
    assert_eq!(call("g.Two", &[Val::S64(2), text("ab")]), text("5 abab"));
}

#[test]
fn a_generic_sum_type_is_built_and_matched() {
    assert_eq!(call("g.First", &[ints(&[9, 1])]), Val::S64(9));
    assert_eq!(call("g.First", &[ints(&[])]), Val::S64(-1));
}

#[test]
fn an_instance_inside_an_instance() {
    // `Box<Box<Int>>`: the outer layout holds the inner's.
    assert_eq!(call("g.Nested", &[Val::S64(-3)]), Val::S64(-3));
}

#[test]
fn a_generic_opaque_type_is_built_and_read() {
    assert_eq!(call("g.Tags", &[ints(&[1, 2, 3])]), Val::S64(3));
}

#[test]
fn a_generic_case_as_a_function_and_a_record_in_a_lambda() {
    assert_eq!(call("g.Justs", &[ints(&[1, 2, 3])]), Val::S64(6));
    assert_eq!(
        call("g.Labels", &[Val::List(vec![text("a"), text("b")])]),
        text("a!b!")
    );
}

#[test]
fn a_type_parameter_nothing_fixes_is_refused_by_name() {
    // `Maybe.Nothing` alone, where nothing says what it is a `Maybe` of.
    let err = pw_core::backend::component::compile(
        &units(&[(
            "m.pw",
            "module m\n\ntype Maybe<T> =\n    | Nothing\n    | Just(T)\n\npublic query Q(n: Int) -> Int {\n    let m = Maybe.Nothing\n    n\n}\n",
        )]),
        "m.Q",
    )
    .map(|_| ())
    .expect_err("refused");
    assert!(
        err.contains("a type parameter no use instantiates"),
        "{err}"
    );
}
