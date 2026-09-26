//! **Nested and literal patterns, run** (ADR-0060).
//!
//! Each query runs through the E8 host over generated values, beside a model
//! written here in Rust. A match with a pattern nested in another, a literal,
//! or a `Bool`, an `Int` or a `String` for a scrutinee compiles to a decision
//! tree: each node tests one value, and each leaf is the first arm every test
//! on its path agrees with.

use std::collections::BTreeMap;

use pw_conformance::{Runnable, compile, units};
use pw_host::engine::Val;

const CASES: usize = 200;

const PROGRAM: &str = r#"module pats

type Shape =
    | Circle(Int)
    | Label(String)
    | Empty

public query Nested(o: Option<Option<Int>>) -> Int {
    match o {
        Some(Some(x)) => x,
        Some(None) => 0,
        None => -1,
    }
}

public query Inner(o: Option<Shape>) -> String {
    match o {
        Some(Circle(0)) => "dot",
        Some(Circle(r)) => "circle {r}",
        Some(Empty) => "empty",
        Some(_) => "other",
        None => "none",
    }
}

public query EmptyOnly(o: Option<Shape>) -> Int {
    match o {
        Some(Empty) => 1,
        _ => 0,
    }
}

public query Flag(b: Bool) -> Int {
    match b {
        true => 1,
        false => 0,
    }
}

public query Words(n: Int) -> String {
    match n {
        -1 => "minus one",
        0 => "zero",
        1 | 2 => "small",
        _ => "many",
    }
}

public query Greet(s: String) -> String {
    match s {
        "hi" => "hello",
        "" => "nothing",
        other => "{other}?",
    }
}

public query Picked(a: Option<Int>, b: Bool) -> Int {
    match a {
        Some(1) => if b { 10 } else { 11 },
        Some(n) => n,
        None => 0,
    }
}

public query Results(r: Result<Option<Shape>, String>) -> String {
    match r {
        Ok(Some(Label(t))) => "label {t}",
        Ok(Some(Circle(r))) => "circle {r}",
        Ok(_) => "nothing",
        Err("") => "no reason",
        Err(why) => "failed: {why}",
    }
}
"#;

struct Rng(u64);

impl Rng {
    fn next(&mut self) -> u64 {
        self.0 ^= self.0 >> 12;
        self.0 ^= self.0 << 25;
        self.0 ^= self.0 >> 27;
        self.0.wrapping_mul(0x2545_F491_4F6C_DD1D)
    }
    fn below(&mut self, n: u64) -> u64 {
        self.next() % n.max(1)
    }
    /// Small, so the literals the arms name come up often.
    fn int(&mut self) -> i64 {
        match self.below(4) {
            0 => [i64::MIN, i64::MAX][self.below(2) as usize],
            _ => self.below(7) as i64 - 3,
        }
    }
    fn text(&mut self) -> String {
        const POOL: [&str; 5] = ["", "hi", "HI", "人", "hi "];
        POOL[self.below(POOL.len() as u64) as usize].to_string()
    }
}

#[derive(Debug, Clone)]
enum Shape {
    Circle(i64),
    Label(String),
    Empty,
}

fn shape(rng: &mut Rng) -> Shape {
    match rng.below(3) {
        0 => Shape::Circle(rng.int()),
        1 => Shape::Label(rng.text()),
        _ => Shape::Empty,
    }
}

fn shape_val(s: &Shape) -> Val {
    match s {
        Shape::Circle(r) => Val::Variant("circle".into(), Some(Box::new(Val::S64(*r)))),
        Shape::Label(t) => Val::Variant("label".into(), Some(Box::new(Val::String(t.clone())))),
        Shape::Empty => Val::Variant("empty".into(), None),
    }
}

fn option(v: Option<Val>) -> Val {
    Val::Option(v.map(Box::new))
}

fn call(id: &str, args: &[Val]) -> Val {
    Runnable::new(compile(&units(&[("pats.pw", PROGRAM)]), id))
        .call(&BTreeMap::new(), args)
        .expect("runs")
        .remove(0)
}

fn text(s: &str) -> Val {
    Val::String(s.to_string())
}

#[test]
fn a_pattern_nested_in_an_option_takes_its_payload_apart() {
    let mut rng = Rng(0x0E57);
    let runnable = Runnable::new(compile(&units(&[("pats.pw", PROGRAM)]), "pats.Nested"));
    for _ in 0..CASES {
        let o: Option<Option<i64>> = match rng.below(3) {
            0 => None,
            1 => Some(None),
            _ => Some(Some(rng.int())),
        };
        let expected = match o {
            Some(Some(x)) => x,
            Some(None) => 0,
            None => -1,
        };
        let arg = option(o.map(|i| option(i.map(Val::S64))));
        let out = runnable
            .call(&BTreeMap::new(), &[arg])
            .expect("runs")
            .remove(0);
        assert_eq!(out, Val::S64(expected), "Nested({o:?})");
    }
}

#[test]
fn a_case_and_a_literal_nested_in_an_option() {
    let mut rng = Rng(0x1AAE);
    let runnable = Runnable::new(compile(&units(&[("pats.pw", PROGRAM)]), "pats.Inner"));
    for _ in 0..CASES {
        let o = (rng.below(4) > 0).then(|| shape(&mut rng));
        let expected = match &o {
            Some(Shape::Circle(0)) => "dot".to_string(),
            Some(Shape::Circle(r)) => format!("circle {r}"),
            Some(Shape::Empty) => "empty".to_string(),
            Some(_) => "other".to_string(),
            None => "none".to_string(),
        };
        let arg = option(o.as_ref().map(shape_val));
        let out = runnable
            .call(&BTreeMap::new(), &[arg])
            .expect("runs")
            .remove(0);
        assert_eq!(out, text(&expected), "Inner({o:?})");
    }
}

#[test]
fn a_case_named_alone_under_another_is_that_case_not_a_binding() {
    // Until ADR-0060 `Empty` here bound every payload, so `Some(Circle(3))`
    // took the first arm.
    let runnable = Runnable::new(compile(&units(&[("pats.pw", PROGRAM)]), "pats.EmptyOnly"));
    for (o, expected) in [
        (Some(Shape::Empty), 1),
        (Some(Shape::Circle(3)), 0),
        (Some(Shape::Label("x".into())), 0),
        (None, 0),
    ] {
        let arg = option(o.as_ref().map(shape_val));
        let out = runnable
            .call(&BTreeMap::new(), &[arg])
            .expect("runs")
            .remove(0);
        assert_eq!(out, Val::S64(expected), "EmptyOnly({o:?})");
    }
}

#[test]
fn a_bool_is_matched_by_its_two_values() {
    assert_eq!(call("pats.Flag", &[Val::Bool(true)]), Val::S64(1));
    assert_eq!(call("pats.Flag", &[Val::Bool(false)]), Val::S64(0));
}

#[test]
fn an_int_is_matched_by_its_literals_negative_ones_too() {
    let runnable = Runnable::new(compile(&units(&[("pats.pw", PROGRAM)]), "pats.Words"));
    for n in [-2, -1, 0, 1, 2, 3, i64::MIN, i64::MAX] {
        let expected = match n {
            -1 => "minus one",
            0 => "zero",
            1 | 2 => "small",
            _ => "many",
        };
        let out = runnable
            .call(&BTreeMap::new(), &[Val::S64(n)])
            .expect("runs")
            .remove(0);
        assert_eq!(out, text(expected), "Words({n})");
    }
}

#[test]
fn a_string_is_matched_by_its_literals_and_bound_otherwise() {
    let runnable = Runnable::new(compile(&units(&[("pats.pw", PROGRAM)]), "pats.Greet"));
    for s in ["hi", "", "HI", "hi ", "人"] {
        let expected = match s {
            "hi" => "hello".to_string(),
            "" => "nothing".to_string(),
            other => format!("{other}?"),
        };
        let out = runnable
            .call(&BTreeMap::new(), &[text(s)])
            .expect("runs")
            .remove(0);
        assert_eq!(out, text(&expected), "Greet({s:?})");
    }
}

#[test]
fn a_literal_arm_falls_through_to_the_arm_after_it() {
    let runnable = Runnable::new(compile(&units(&[("pats.pw", PROGRAM)]), "pats.Picked"));
    for (a, b, expected) in [
        (Some(1), true, 10),
        (Some(1), false, 11),
        (Some(5), true, 5),
        (None, true, 0),
    ] {
        let out = runnable
            .call(&BTreeMap::new(), &[option(a.map(Val::S64)), Val::Bool(b)])
            .expect("runs")
            .remove(0);
        assert_eq!(out, Val::S64(expected), "Picked({a:?}, {b})");
    }
}

#[test]
fn a_result_of_an_option_of_a_case_three_deep() {
    let mut rng = Rng(0x7E5E);
    let runnable = Runnable::new(compile(&units(&[("pats.pw", PROGRAM)]), "pats.Results"));
    for _ in 0..CASES {
        let r: Result<Option<Shape>, String> = match rng.below(3) {
            0 => Err(rng.text()),
            _ => Ok((rng.below(3) > 0).then(|| shape(&mut rng))),
        };
        let expected = match &r {
            Ok(Some(Shape::Label(t))) => format!("label {t}"),
            Ok(Some(Shape::Circle(c))) => format!("circle {c}"),
            Ok(_) => "nothing".to_string(),
            Err(e) if e.is_empty() => "no reason".to_string(),
            Err(why) => format!("failed: {why}"),
        };
        let arg = match &r {
            Ok(o) => Val::Result(Ok(Some(Box::new(option(o.as_ref().map(shape_val)))))),
            Err(e) => Val::Result(Err(Some(Box::new(text(e))))),
        };
        let out = runnable
            .call(&BTreeMap::new(), &[arg])
            .expect("runs")
            .remove(0);
        assert_eq!(out, text(&expected), "Results({r:?})");
    }
}

fn refused(program: &str, id: &str) -> String {
    let err = pw_core::backend::component::compile(&units(&[("m.pw", program)]), id)
        .map(|_| ())
        .expect_err("refused");
    println!("{id}: {err}");
    err
}

#[test]
fn a_float_literal_pattern_is_refused_by_name() {
    let err = refused(
        "module m\n\npublic query Q(x: Float) -> Int {\n    match x {\n        0.5 => 1,\n        _ => 0,\n    }\n}\n",
        "m.Q",
    );
    assert!(err.contains("a `Float` literal pattern"), "{err}");
}

#[test]
fn an_or_pattern_that_binds_is_refused_by_name() {
    let err = refused(
        "module m\n\npublic query Q(o: Result<Int, Int>) -> Int {\n    match o {\n        Ok(x) | Err(x) => x,\n    }\n}\n",
        "m.Q",
    );
    assert!(err.contains("an or-pattern that binds a name"), "{err}");
}
