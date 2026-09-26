//! **A function is a value, compiled** (ADR-0052).
//!
//! Until 2026-09-25 a lambda or a declaration's name compiled only where a
//! list operation runs it: stored, returned, or passed to any other
//! declaration, it was refused. Each query here runs as a component through
//! the E8 host and is compared with Rust doing the same.

use std::collections::BTreeMap;

use pw_conformance::{Runnable, compile, units};
use pw_host::engine::Val;

const PROGRAM: &str = r#"module f

import List

fn apply(f: fn(Int) -> Int, x: Int) -> Int { f(x) }

public query Applied(x: Int) -> Int { apply(y => y * 3, x) }

fn twice(f: fn(Int) -> Int, x: Int) -> Int { f(f(x)) }

public query Twice(x: Int) -> Int { twice(y => y + 1, x) }

public query Captured(xs: List<Int>, k: Int) -> List<Int> {
    let add: fn(Int) -> Int = x => x + k
    List.map(xs, add)
}

fn compose(f: fn(Int) -> Int, g: fn(Int) -> Int) -> fn(Int) -> Int {
    x => g(f(x))
}

public query Composed(x: Int) -> Int {
    let h = compose(y => y * 2, y => y + 1)
    h(x)
}

fn double(n: Int) -> Int { n * 2 }

public query Named(x: Int) -> Int { apply(double, x) }

fn make_adder(k: Int) -> fn(Int) -> Int {
    x => x + k
}

public query Adder(k: Int, x: Int) -> Int {
    let add = make_adder(k)
    add(x)
}

public query Chosen(flag: Bool, x: Int) -> Int {
    let f: fn(Int) -> Int = if flag { y => y * 10 } else { y => y - 10 }
    f(x)
}

public query Tagged(words: List<String>, tag: String) -> List<String> {
    let mark: fn(String) -> String = w => "{tag}:{w}"
    List.map(words, mark)
}

fn fold_with(xs: List<Int>, i: Int, acc: Int, f: fn(Int, Int) -> Int) -> Int {
    match List.get(xs, i) {
        None => acc,
        Some(x) => fold_with(xs, i + 1, f(acc, x), f),
    }
}

public query Product(xs: List<Int>) -> Int { fold_with(xs, 0, 1, (a, b) => a * b) }
"#;

fn call(id: &str, args: &[Val]) -> Result<Val, String> {
    Runnable::new(compile(&units(&[("f.pw", PROGRAM)]), id))
        .call(&BTreeMap::new(), args)
        .map(|mut out| out.remove(0))
}

fn int(n: i64) -> Val {
    Val::S64(n)
}

fn ints(xs: &[i64]) -> Val {
    Val::List(xs.iter().map(|x| int(*x)).collect())
}

#[test]
fn a_lambda_passed_to_a_declaration_is_called_there() {
    assert_eq!(call("f.Applied", &[int(7)]), Ok(int(21)));
    assert_eq!(call("f.Twice", &[int(7)]), Ok(int(9)));
}

#[test]
fn a_closure_carries_what_it_captures() {
    assert_eq!(
        call("f.Captured", &[ints(&[1, 2, 3]), int(10)]),
        Ok(ints(&[11, 12, 13]))
    );
    assert_eq!(
        call(
            "f.Tagged",
            &[
                Val::List(vec![Val::String("a".into()), Val::String("人".into())]),
                Val::String("t".into())
            ]
        ),
        Ok(Val::List(vec![
            Val::String("t:a".into()),
            Val::String("t:人".into())
        ]))
    );
}

#[test]
fn a_function_is_returned_and_called() {
    // compose(y * 2, y + 1)(x) is x * 2 + 1.
    assert_eq!(call("f.Composed", &[int(5)]), Ok(int(11)));
    assert_eq!(call("f.Adder", &[int(4), int(5)]), Ok(int(9)));
}

#[test]
fn a_declarations_name_is_a_value() {
    assert_eq!(call("f.Named", &[int(8)]), Ok(int(16)));
}

#[test]
fn a_function_is_chosen_by_a_branch() {
    assert_eq!(call("f.Chosen", &[Val::Bool(true), int(3)]), Ok(int(30)));
    assert_eq!(call("f.Chosen", &[Val::Bool(false), int(3)]), Ok(int(-7)));
}

#[test]
fn a_recursion_carries_a_function() {
    assert_eq!(call("f.Product", &[ints(&[2, 3, 4])]), Ok(int(24)));
    assert_eq!(call("f.Product", &[ints(&[])]), Ok(int(1)));
}
