//! **Recursion, compiled** (ADR-0050).
//!
//! Calls are inlined (ADR-0039 §4), and until 2026-09-25 a recursive one was
//! refused: an inlined recursion has no end. A recursive callee is compiled
//! once per instance, beside the export, and called. Each query runs as a
//! component through the E8 host and is compared with Rust doing the same.

use std::collections::BTreeMap;

use pw_conformance::{Runnable, compile, units};
use pw_host::engine::Val;

const PROGRAM: &str = r#"module r

import List

fn fact(n: Int) -> Int {
    if n <= 1 { 1 } else { n * fact(n - 1) }
}

public query Factorial(n: Int) -> Int { fact(n) }

fn fib(n: Int) -> Int {
    if n < 2 { n } else { fib(n - 1) + fib(n - 2) }
}

public query Fibonacci(n: Int) -> Int { fib(n) }

fn is_even(n: Int) -> Bool {
    if n == 0 { true } else { is_odd(n - 1) }
}

fn is_odd(n: Int) -> Bool {
    if n == 0 { false } else { is_even(n - 1) }
}

public query Even(n: Int) -> Bool { is_even(n) }

fn count_from<T>(xs: List<T>, i: Int) -> Int {
    match List.get(xs, i) {
        Some(x) => 1 + count_from(xs, i + 1),
        None => 0,
    }
}

public query CountInts(xs: List<Int>) -> Int { count_from(xs, 0) }

public query CountWords(xs: List<String>) -> Int { count_from(xs, 0) }

public query Triangle(n: Int) -> Int {
    if n <= 0 { 0 } else { n + Triangle(n - 1) }
}

fn repeat(s: String, n: Int) -> String {
    if n <= 0 { "" } else { "{s}{repeat(s, n - 1)}" }
}

public query Repeat(s: String, n: Int) -> String { repeat(s, n) }

fn largest(xs: List<Int>, i: Int, best: Option<Int>) -> Option<Int> {
    match List.get(xs, i) {
        None => best,
        Some(x) => match best {
            None => largest(xs, i + 1, Some(x)),
            Some(b) => largest(xs, i + 1, if x > b { Some(x) } else { Some(b) }),
        },
    }
}

public query Largest(xs: List<Int>) -> Option<Int> { largest(xs, 0, None) }
"#;

fn call(id: &str, args: &[Val]) -> Result<Val, String> {
    Runnable::new(compile(&units(&[("r.pw", PROGRAM)]), id))
        .call(&BTreeMap::new(), args)
        .map(|mut out| out.remove(0))
}

fn int(n: i64) -> Val {
    Val::S64(n)
}

fn fib(n: i64) -> i64 {
    if n < 2 { n } else { fib(n - 1) + fib(n - 2) }
}

#[test]
fn a_function_calls_itself() {
    for n in [0, 1, 5, 20] {
        let want: i64 = (1..=n.max(1)).product();
        assert_eq!(call("r.Factorial", &[int(n)]), Ok(int(want)), "fact({n})");
    }
    for n in [0, 1, 2, 10, 20] {
        assert_eq!(call("r.Fibonacci", &[int(n)]), Ok(int(fib(n))), "fib({n})");
    }
}

#[test]
fn two_functions_call_each_other() {
    for n in [0, 1, 2, 7, 100] {
        assert_eq!(
            call("r.Even", &[int(n)]),
            Ok(Val::Bool(n % 2 == 0)),
            "is_even({n})"
        );
    }
}

#[test]
fn a_generic_recursion_is_compiled_once_per_instance() {
    let ints = Val::List((0..17).map(int).collect());
    assert_eq!(call("r.CountInts", &[ints]), Ok(int(17)));
    let words = Val::List(
        ["a", "人", "😀"]
            .iter()
            .map(|w| Val::String(w.to_string()))
            .collect(),
    );
    assert_eq!(call("r.CountWords", &[words]), Ok(int(3)));
    assert_eq!(call("r.CountWords", &[Val::List(vec![])]), Ok(int(0)));
}

#[test]
fn an_exported_query_calls_itself() {
    for n in [0, 1, 10, 100] {
        assert_eq!(call("r.Triangle", &[int(n)]), Ok(int(n * (n + 1) / 2)));
    }
}

#[test]
fn a_recursion_builds_a_string_and_an_option() {
    assert_eq!(
        call("r.Repeat", &[Val::String("ab".into()), int(3)]),
        Ok(Val::String("ababab".into()))
    );
    let xs = Val::List([3, -1, 9, 4].into_iter().map(int).collect());
    assert_eq!(
        call("r.Largest", &[xs]),
        Ok(Val::Option(Some(Box::new(int(9)))))
    );
    assert_eq!(
        call("r.Largest", &[Val::List(vec![])]),
        Ok(Val::Option(None))
    );
}

#[test]
fn an_overflow_inside_a_recursion_traps() {
    // 21! does not fit in 64 bits; the multiplication traps, as any `Int`
    // arithmetic does (ADR-0039 §1), at whatever depth it happens.
    assert!(call("r.Factorial", &[int(21)]).is_err());
}
