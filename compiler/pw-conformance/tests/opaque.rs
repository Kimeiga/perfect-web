//! **An opaque value, built and read inside a component** (ADR-0054).
//!
//! Until 2026-09-25 the backend refused both: `PositiveInt(1)` named no
//! declaration it could call, and `.value` was not a record's field. An
//! opaque type is its representation, so both are the same value under
//! another type. Each query runs as a component through the E8 host.

use std::collections::BTreeMap;

use pw_conformance::{Runnable, compile, units};
use pw_host::engine::Val;

const PROGRAM: &str = r#"module o

import List
import String

opaque type Count = Int

opaque type Tag = String

fn count(n: Int) -> Count { Count(n) }

fn raw(c: Count) -> Int { c.value }

public query Roundtrip(n: Int) -> Int { raw(count(n)) + 1 }

public query Piped(n: Int) -> Int { raw(n * 3 |> Count()) }

public query Wrap(s: String) -> Tag { Tag(s) }

public query Unwrap(t: Tag) -> String { t.value }

public query Longer(a: Tag, b: Tag) -> Tag {
    if String.length(a.value) >= String.length(b.value) { a } else { b }
}

public query Doubled(xs: List<Int>) -> List<Count> { List.map(xs, x => Count(x * 2)) }

public query Total(cs: List<Count>) -> Int { List.fold(cs, 0, (t, c) => t + c.value) }

type Point = Point { x: Int, y: Int }

opaque type Place = Point

opaque type Names = List<String>

opaque type Maybe = Option<Int>

public query Moved(x: Int, y: Int) -> Int {
    let p = Place(Point { x: x, y: y })
    p.value.x * 10 + p.value.y
}

public query Named(names: List<String>) -> Int { List.length(Names(names).value) }

public query Chosen(n: Int) -> Int {
    let m = if n > 0 { Maybe(Some(n)) } else { Maybe(None) }
    match m.value {
        Some(v) => v,
        None => -1,
    }
}

fn down(c: Count) -> Int {
    if c.value <= 0 { 0 } else { 1 + down(Count(c.value - 1)) }
}

public query Steps(n: Int) -> Int { down(Count(n % 50)) }

public query Scaled(k: Int, xs: List<Int>) -> List<Int> {
    let by = Count(k)
    List.map(xs, x => x * by.value)
}
"#;

fn call(id: &str, args: &[Val]) -> Result<Val, String> {
    Runnable::new(compile(&units(&[("o.pw", PROGRAM)]), id))
        .call(&BTreeMap::new(), args)
        .map(|mut out| out.remove(0))
}

fn text(s: &str) -> Val {
    Val::String(s.to_string())
}

#[test]
fn an_opaque_value_is_built_and_read_back() {
    assert_eq!(call("o.Roundtrip", &[Val::S64(41)]), Ok(Val::S64(42)));
    assert_eq!(call("o.Piped", &[Val::S64(5)]), Ok(Val::S64(15)));
}

#[test]
fn an_opaque_value_crosses_the_boundary_as_its_representation() {
    assert_eq!(call("o.Wrap", &[text("人")]), Ok(text("人")));
    assert_eq!(call("o.Unwrap", &[text("tag")]), Ok(text("tag")));
    assert_eq!(
        call("o.Longer", &[text("ab"), text("abc")]),
        Ok(text("abc"))
    );
    assert_eq!(
        call("o.Longer", &[text("abc"), text("ab")]),
        Ok(text("abc"))
    );
}

#[test]
fn an_opaque_value_is_built_in_a_loop_and_folded() {
    let xs = Val::List([1, 2, 3].into_iter().map(Val::S64).collect());
    let doubled = Val::List([2, 4, 6].into_iter().map(Val::S64).collect());
    assert_eq!(call("o.Doubled", &[xs]), Ok(doubled.clone()));
    assert_eq!(call("o.Total", &[doubled]), Ok(Val::S64(12)));
}

#[test]
fn a_record_a_list_and_an_option_are_each_an_opaque_types_representation() {
    assert_eq!(
        call("o.Moved", &[Val::S64(3), Val::S64(4)]),
        Ok(Val::S64(34))
    );
    let names = Val::List(vec![text("a"), text("人")]);
    assert_eq!(call("o.Named", &[names]), Ok(Val::S64(2)));
    assert_eq!(call("o.Chosen", &[Val::S64(7)]), Ok(Val::S64(7)));
    assert_eq!(call("o.Chosen", &[Val::S64(-7)]), Ok(Val::S64(-1)));
}

#[test]
fn an_opaque_value_is_passed_to_a_recursion_and_captured() {
    assert_eq!(call("o.Steps", &[Val::S64(7)]), Ok(Val::S64(7)));
    let xs = Val::List([1, 2].into_iter().map(Val::S64).collect());
    let scaled = Val::List([3, 6].into_iter().map(Val::S64).collect());
    assert_eq!(call("o.Scaled", &[Val::S64(3), xs]), Ok(scaled));
}
