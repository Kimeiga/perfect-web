//! **An early `return`, `?`, and `for` loops, compiled** (ADR-0051).
//!
//! Until 2026-09-25 the backend refused all three: a body's value was its
//! last expression, and a compiled body bound each name once. Each query
//! runs as a component through the E8 host and is compared with Rust doing
//! the same.

use std::collections::BTreeMap;

use pw_conformance::{Runnable, compile, units};
use pw_host::engine::Val;

const PROGRAM: &str = r#"module c

import List

public query Clamp(n: Int) -> Int {
    if n < 0 {
        return 0
    }
    if n > 100 {
        return 100
    }
    n
}

public query FirstEven(xs: List<Int>) -> Option<Int> {
    for x in xs {
        if x % 2 == 0 {
            return Some(x)
        }
    }
    None
}

public query Total(xs: List<Int>) -> Int {
    let mut total = 0
    for x in xs {
        total = total + x
    }
    total
}

public query Occurrences(words: List<String>, word: String) -> Int {
    let mut n = 0
    for w in words {
        if w == word {
            n = n + 1
        }
    }
    n
}

public query Ordered(xs: List<Int>) -> Int {
    let mut count = 0
    for a in xs {
        for b in xs {
            if a < b {
                count = count + 1
            }
        }
    }
    count
}

public query Joined(words: List<String>) -> String {
    let mut out = ""
    let mut first = true
    for w in words {
        if first {
            out = w
        } else {
            out = "{out}, {w}"
        }
        first = false
    }
    out
}

fn half(n: Int) -> Result<Int, String> {
    if n % 2 == 0 { Ok(n / 2) } else { Err("odd: {n}") }
}

public query Quarter(n: Int) -> Result<Int, String> {
    let h = half(n)?
    let q = half(h)?
    Ok(q)
}

public query SumOf(xs: List<Int>, i: Int, j: Int) -> Option<Int> {
    let a = List.get(xs, i)?
    let b = List.get(xs, j)?
    Some(a + b)
}

fn sign(n: Int) -> Int {
    if n < 0 {
        return -1
    }
    if n == 0 {
        return 0
    }
    1
}

public query Signs(xs: List<Int>) -> List<Int> { List.map(xs, sign) }

public query SignOf(n: Int) -> Int { sign(n) * 10 }

public query Doubled(n: Int) -> Int {
    let d = if n < 0 { return -1 } else { n * 2 }
    d + 1
}

public query Head(xs: List<Int>) -> Int {
    let first = match List.get(xs, 0) {
        None => return 0,
        Some(x) => x,
    }
    first * 100
}

public query Swapped(a: Int, b: Int) -> Int {
    let mut x = a
    let mut y = b
    let t = x
    x = y
    y = t
    x * 10 + y
}
"#;

fn call(id: &str, args: &[Val]) -> Result<Val, String> {
    Runnable::new(compile(&units(&[("c.pw", PROGRAM)]), id))
        .call(&BTreeMap::new(), args)
        .map(|mut out| out.remove(0))
}

fn int(n: i64) -> Val {
    Val::S64(n)
}

fn ints(xs: &[i64]) -> Val {
    Val::List(xs.iter().map(|x| int(*x)).collect())
}

fn words(ws: &[&str]) -> Val {
    Val::List(ws.iter().map(|w| Val::String(w.to_string())).collect())
}

fn some(v: Val) -> Val {
    Val::Option(Some(Box::new(v)))
}

#[test]
fn a_return_leaves_the_body_early() {
    for (n, want) in [
        (-5, 0),
        (0, 0),
        (42, 42),
        (100, 100),
        (101, 100),
        (i64::MAX, 100),
    ] {
        assert_eq!(call("c.Clamp", &[int(n)]), Ok(int(want)), "clamp({n})");
    }
}

#[test]
fn a_return_leaves_a_loop_and_the_body() {
    assert_eq!(
        call("c.FirstEven", &[ints(&[1, 3, 4, 6])]),
        Ok(some(int(4)))
    );
    assert_eq!(
        call("c.FirstEven", &[ints(&[1, 3, 5])]),
        Ok(Val::Option(None))
    );
    assert_eq!(call("c.FirstEven", &[ints(&[])]), Ok(Val::Option(None)));
}

#[test]
fn a_loop_updates_a_mutable_binding() {
    assert_eq!(call("c.Total", &[ints(&[1, 2, 3, 4])]), Ok(int(10)));
    assert_eq!(call("c.Total", &[ints(&[])]), Ok(int(0)));
    assert_eq!(
        call(
            "c.Occurrences",
            &[words(&["a", "b", "a", "人"]), Val::String("a".into())]
        ),
        Ok(int(2))
    );
    // Two loops, one inside the other: every ordered pair.
    assert_eq!(call("c.Ordered", &[ints(&[3, 1, 2])]), Ok(int(3)));
    assert_eq!(
        call("c.Joined", &[words(&["x", "y", "z"])]),
        Ok(Val::String("x, y, z".into()))
    );
    assert_eq!(call("c.Joined", &[words(&[])]), Ok(Val::String("".into())));
}

#[test]
fn a_branch_that_returns_takes_the_other_branchs_type() {
    // The `if`'s and the `match`'s values are bound, and one side returns:
    // it has no value, and the binding is the other side's type.
    assert_eq!(call("c.Doubled", &[int(5)]), Ok(int(11)));
    assert_eq!(call("c.Doubled", &[int(-5)]), Ok(int(-1)));
    assert_eq!(call("c.Head", &[ints(&[3, 4])]), Ok(int(300)));
    assert_eq!(call("c.Head", &[ints(&[])]), Ok(int(0)));
}

#[test]
fn a_read_is_a_copy_a_later_assignment_does_not_change() {
    // `t` is what `x` held when it was read. Were a read the variable
    // itself, `y = t` would give `y` the value `x` was just assigned.
    assert_eq!(call("c.Swapped", &[int(1), int(2)]), Ok(int(21)));
    assert_eq!(call("c.Swapped", &[int(7), int(-3)]), Ok(int(-23)));
}

#[test]
fn a_question_mark_returns_the_failure() {
    let ok = |n: i64| Val::Result(Ok(Some(Box::new(int(n)))));
    let err = |s: &str| Val::Result(Err(Some(Box::new(Val::String(s.into())))));
    assert_eq!(call("c.Quarter", &[int(12)]), Ok(ok(3)));
    assert_eq!(call("c.Quarter", &[int(6)]), Ok(err("odd: 3")));
    assert_eq!(call("c.Quarter", &[int(7)]), Ok(err("odd: 7")));

    let xs = ints(&[10, 20, 30]);
    assert_eq!(
        call("c.SumOf", &[xs.clone(), int(0), int(2)]),
        Ok(some(int(40)))
    );
    assert_eq!(
        call("c.SumOf", &[xs.clone(), int(0), int(9)]),
        Ok(Val::Option(None))
    );
    assert_eq!(
        call("c.SumOf", &[xs, int(-1), int(0)]),
        Ok(Val::Option(None))
    );
}

#[test]
fn a_callee_that_returns_early_returns_from_itself() {
    // `sign` is compiled beside the export rather than inlined, so its
    // `return` leaves `sign` and not the query.
    assert_eq!(call("c.SignOf", &[int(-7)]), Ok(int(-10)));
    assert_eq!(call("c.SignOf", &[int(0)]), Ok(int(0)));
    assert_eq!(call("c.SignOf", &[int(9)]), Ok(int(10)));
    assert_eq!(call("c.Signs", &[ints(&[-2, 0, 5])]), Ok(ints(&[-1, 0, 1])));
}
