//! **`elif` and `else if` chains, compiled** (ADR-0068).
//!
//! `if a { x } elif b { y } else { z }` is one syntax node holding `a`,
//! `{ x }`, `b`, `{ y }` and `{ z }`, and `else if` the same. Until
//! 2026-09-26 the lowering took its third child for the `else`, so the chain
//! compiled as `if a { x } else b`: when `a` was false, the value was the
//! next condition, and `{ y }` and `{ z }` were never run. A chain of `Bool`s
//! checked and compiled and answered wrong. Each query here runs through the
//! E8 host beside the answer it must give.

use std::collections::BTreeMap;

use pw_conformance::{Runnable, compile, units};
use pw_host::engine::Val;

/// Chains of `Bool`s, alone: lowered as `if a { x } else b`, each still
/// checked, compiled and ran, with the wrong answer.
const BOOLS: &str = r#"module ch

public query Pick(n: Int) -> Bool {
    if n > 10 { true } elif n > 5 { false } else { true }
}

public query PickElse(n: Int) -> Bool {
    if n > 10 { true } else if n > 5 { false } else { true }
}
"#;

const PROGRAM: &str = r#"module ch

public query Grade(n: Int) -> Int {
    if n > 90 { 3 } elif n > 60 { 2 } elif n > 30 { 1 } else { 0 }
}

public query Steps(n: Int) -> Int {
    let mut t = 0
    if n > 10 { t = 2 } elif n > 5 { t = 1 }
    t
}
"#;

fn run(program: &str, id: &str, n: i64) -> Val {
    Runnable::new(compile(&units(&[("ch.pw", program)]), id))
        .call(&BTreeMap::new(), &[Val::S64(n)])
        .expect("runs")
        .remove(0)
}

fn call(id: &str, n: i64) -> Val {
    run(PROGRAM, id, n)
}

#[test]
fn an_elif_chain_of_bools_answers_each_branch() {
    for n in [0, 3, 5, 6, 8, 10, 11, 40] {
        let want = if n > 10 { true } else { n <= 5 };
        assert_eq!(run(BOOLS, "ch.Pick", n), Val::Bool(want), "Pick({n})");
        assert_eq!(
            run(BOOLS, "ch.PickElse", n),
            Val::Bool(want),
            "PickElse({n})"
        );
    }
}

#[test]
fn a_chain_of_four_branches_reaches_the_last() {
    for n in [0, 30, 31, 60, 61, 90, 91, 100] {
        let want = match n {
            91.. => 3,
            61..=90 => 2,
            31..=60 => 1,
            _ => 0,
        };
        assert_eq!(call("ch.Grade", n), Val::S64(want), "Grade({n})");
    }
}

#[test]
fn a_chain_without_else_is_a_statement() {
    for n in [0, 5, 6, 10, 11] {
        let want = match n {
            11.. => 2,
            6..=10 => 1,
            _ => 0,
        };
        assert_eq!(call("ch.Steps", n), Val::S64(want), "Steps({n})");
    }
}
