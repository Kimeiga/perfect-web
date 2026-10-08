//! **A component's trap says why it stopped** (ADR-0267). A trap of the
//! backend's own calls a function the compiler names by its cause, `pw-trap:
//! <its words>`, and the host reads the cause from the frame it stopped in;
//! Wasm's own traps it names by their code. Until ADR-0267 every one read
//! as Wasm's "`unreachable` instruction executed", whatever stopped it, and
//! ruling 0057-c's "by name" held in the browser's module alone. Each test
//! states its causes, through the host, with its controls.

use std::collections::BTreeMap;

use pw_conformance::{Runnable, compile, units};
use pw_host::engine::Val;

const PROGRAM: &str = r#"module t

import List
import Map
import String

public query Plus(x: Int) -> Int { x + 1 }

public query Times(x: Int) -> Int { x * 2 }

public query Negated(x: Int) -> Int { -x }

public query Quotient(a: Int, b: Int) -> Int { a / b }

public query Size(m: Map<String, Int>) -> Int { Map.size(m) }

public query Built(keys: List<String>, values: List<Int>) -> Map<String, Int> {
    Map.from_lists(keys, values)
}

public query Text(points: List<Int>) -> String { String.from_codepoints(points) }

fn grown(s: String, n: Int) -> String !{} {
    match n {
        0 => s,
        _ => grown("{s}{s}", n - 1),
    }
}

public query Grown(n: Int) -> Int { String.length(grown("x", n)) }

fn spin(n: Int) -> Int !{} { spin(n + 1) }

public query Spun(n: Int) -> Int { spin(n) }

public query Product(a: Int, b: Int) -> Int { a * b }

fn copies(xs: List<String>, n: Int) -> List<String> !{} {
    match n {
        0 => xs,
        _ => copies(List.concat(xs, xs), n - 1),
    }
}

public query Joined(copied: Int, doubled: Int) -> Int {
    String.length(String.join(copies([grown("x", doubled)], copied), ""))
}

fn fib(n: Int) -> Int !{} {
    match n {
        0 => 0,
        1 => 1,
        _ => fib(n - 1) + fib(n - 2),
    }
}

public query Fib(n: Int) -> Int { fib(n) }
"#;

fn run(id: &str, args: &[Val]) -> Result<Vec<Val>, String> {
    Runnable::new(compile(&units(&[("t.pw", PROGRAM)]), id)).call(&BTreeMap::new(), args)
}

fn entry(k: &str, v: i64) -> Val {
    Val::Tuple(vec![Val::String(k.to_string()), Val::S64(v)])
}

/// What the host says stopped `id` given `args`.
fn stopped(id: &str, args: &[Val]) -> String {
    match run(id, args) {
        Ok(out) => panic!("{id} answered {out:?}"),
        Err(why) => why,
    }
}

#[test]
fn a_trap_of_the_components_own_says_its_cause() {
    let texts = |ks: &[&str]| Val::List(ks.iter().map(|k| Val::String(k.to_string())).collect());
    for (id, args, cause) in [
        ("t.Plus", vec![Val::S64(i64::MAX)], "Int overflow"),
        ("t.Times", vec![Val::S64(i64::MAX)], "Int overflow"),
        ("t.Negated", vec![Val::S64(i64::MIN)], "Int overflow"),
        (
            "t.Size",
            vec![Val::List(vec![entry("a", 1), entry("a", 2)])],
            "a map or set from outside repeats a key",
        ),
        (
            "t.Built",
            vec![texts(&["a", "b"]), Val::List(vec![Val::S64(1)])],
            "lists of two lengths",
        ),
        (
            "t.Text",
            vec![Val::List(vec![Val::S64(0xD800)])],
            "not a Unicode scalar value",
        ),
        // Doubled past the 64 MiB the harness gives a call.
        ("t.Grown", vec![Val::S64(40)], "memory exhausted"),
        // 2048 copies of one string of one MiB, the list holding each once:
        // joined, 2 GiB, more than one region holds, refused before
        // anything is allocated.
        (
            "t.Joined",
            vec![Val::S64(11), Val::S64(20)],
            "a value too long for memory",
        ),
        // 1024 of them, 1 GiB: what a region holds, and past the harness's.
        (
            "t.Joined",
            vec![Val::S64(10), Val::S64(20)],
            "memory exhausted",
        ),
    ] {
        let why = stopped(id, &args);
        assert!(
            why.starts_with(&format!("stopped: {cause}: ")),
            "{id}: {why}"
        );
    }
    // The controls: each answers where nothing stops it.
    assert_eq!(run("t.Plus", &[Val::S64(1)]), Ok(vec![Val::S64(2)]));
    assert_eq!(
        run("t.Negated", &[Val::S64(i64::MAX)]),
        Ok(vec![Val::S64(-i64::MAX)])
    );
    assert_eq!(
        run("t.Size", &[Val::List(vec![entry("a", 1), entry("b", 2)])]),
        Ok(vec![Val::S64(2)])
    );
    assert_eq!(
        run("t.Text", &[Val::List(vec![Val::S64(0x1F600)])]),
        Ok(vec![Val::String("\u{1F600}".to_string())])
    );
    assert_eq!(run("t.Grown", &[Val::S64(3)]), Ok(vec![Val::S64(8)]));
    // Four copies of eight bytes: a join of a megabyte's copies spends the
    // fuel the harness gives a call.
    assert_eq!(
        run("t.Joined", &[Val::S64(2), Val::S64(3)]),
        Ok(vec![Val::S64(32)])
    );
}

#[test]
fn wasms_own_traps_say_theirs() {
    let why = stopped("t.Quotient", &[Val::S64(7), Val::S64(0)]);
    assert!(why.starts_with("stopped: division by zero: "), "{why}");
    let why = stopped("t.Spun", &[Val::S64(0)]);
    assert!(why.starts_with("stopped: calls nested too deep: "), "{why}");
    // The least `Int` divided by -1: `i64.div_s` traps by itself.
    let why = stopped("t.Quotient", &[Val::S64(i64::MIN), Val::S64(-1)]);
    assert!(why.starts_with("stopped: Int overflow: "), "{why}");
    // And the multiplication's check, which divides back: `MIN / -1`.
    let why = stopped("t.Product", &[Val::S64(-1), Val::S64(i64::MIN)]);
    assert!(why.starts_with("stopped: Int overflow: "), "{why}");
    // 331 million calls, none deep: past the fuel the harness gives a call.
    let why = stopped("t.Fib", &[Val::S64(40)]);
    assert!(why.starts_with("stopped: out of fuel: "), "{why}");
    // The controls.
    assert_eq!(
        run("t.Quotient", &[Val::S64(7), Val::S64(2)]),
        Ok(vec![Val::S64(3)])
    );
    assert_eq!(
        run("t.Product", &[Val::S64(-1), Val::S64(i64::MAX)]),
        Ok(vec![Val::S64(-i64::MAX)])
    );
    assert_eq!(run("t.Fib", &[Val::S64(10)]), Ok(vec![Val::S64(55)]));
}

#[test]
fn the_compiler_and_the_host_name_a_trap_alike() {
    assert_eq!(
        pw_core::backend::wasm::TRAP_PREFIX,
        pw_host::engine::TRAP_PREFIX
    );
}
