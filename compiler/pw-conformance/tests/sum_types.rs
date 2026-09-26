//! **Declared sum types, built and matched, run** (ADR-0059).
//!
//! Each query runs through the E8 host, over generated values, beside a model
//! written here in Rust. A sum type crosses as a WIT `variant`: a parameter
//! arrives in the joined flat slots of the Canonical ABI's flattening, and a
//! result leaves through memory, so both directions are exercised, and so is
//! every slot a case of an `Int`, a `Float`, a `String` or a `Bool` shares.

use std::collections::BTreeMap;

use pw_conformance::{Calls, Runnable, compile, operation, units};
use pw_host::engine::Val;

const CASES: usize = 150;

const PROGRAM: &str = r#"module shapes

import List

type Shape =
    | Circle(Int)
    | Rect(Int, Int)
    | Label(String)
    | Empty

type Mixed =
    | I(Int)
    | F(Float)
    | S(String)
    | B(Bool)
    | Pair(Float, String)
    | Blank

type Holder = Holder { shape: Shape, name: String }

type Failure =
    | Missing
    | Broken(String)

fn fetch(key: String) -> Result<String, Failure> !{ database.read<Holder> }
    host "shapes:data/store#fetch"

fn area(s: Shape) -> Int {
    match s {
        Circle(r) => 3 * r * r,
        Rect(w, h) => w * h,
        Label(_) => 0,
        Empty => -1,
    }
}

public query Make(kind: Int, n: Int, text: String) -> Shape {
    if kind == 0 {
        Shape.Circle(n)
    } else {
        if kind == 1 {
            Shape.Rect(n, n + 1)
        } else {
            if kind == 2 { Shape.Label(text) } else { Shape.Empty }
        }
    }
}

public query Area(s: Shape) -> Int { area(s) }

public query Describe(s: Shape) -> String {
    match s {
        Shape.Circle(r) => "circle {r}",
        Shape.Label(t) => "label {t}",
        _ => "other",
    }
}

public query Kind(s: Shape) -> String {
    match s {
        Circle(_) | Rect(_, _) => "figure",
        other => match other {
            Label(t) => "text {t}",
            _ => "nothing",
        },
    }
}

public query Same(s: Shape) -> Shape { s }

public query Circles(radii: List<Int>) -> List<Shape> { List.map(radii, Shape.Circle) }

public query Nothing() -> Shape { Empty }

public query Total(hs: List<Holder>) -> Int { List.fold(hs, 0, (t, h) => t + area(h.shape)) }

public query Renamed(h: Holder, name: String) -> Holder { Holder { shape: h.shape, name: name } }

public query Mix(m: Mixed) -> Mixed {
    match m {
        I(n) => Mixed.I(n + 1),
        F(x) => Mixed.F(x * 2.0),
        S(t) => Mixed.S("{t}!"),
        B(b) => Mixed.B(!b),
        Pair(x, t) => Mixed.Pair(x + 1.0, "<{t}>"),
        Blank => Mixed.Blank,
    }
}

public query Maybe(s: Option<Shape>) -> Int {
    match s {
        Some(x) => area(x),
        None => -2,
    }
}

public query Fetched(key: String) -> String {
    match fetch(key) {
        Ok(v) => v,
        Err(e) => match e {
            Missing => "missing",
            Broken(why) => "broken: {why}",
        },
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
    /// Small, so `3 * r * r` does not overflow; the bounds are a trap's
    /// business, which `computation.rs` tests.
    fn int(&mut self) -> i64 {
        self.below(2001) as i64 - 1000
    }
    fn float(&mut self) -> f64 {
        match self.below(6) {
            0 => 0.0,
            1 => -0.0,
            2 => 1e300,
            _ => (self.next() as i64 as f64) / 7.0,
        }
    }
    fn text(&mut self) -> String {
        const POOL: [&str; 7] = ["", "a", "ab", "人", "\u{1F600}", "tab\there", "Z"];
        POOL[self.below(POOL.len() as u64) as usize].to_string()
    }
}

/// The model's shape.
#[derive(Debug, Clone, PartialEq)]
enum Shape {
    Circle(i64),
    Rect(i64, i64),
    Label(String),
    Empty,
}

fn shape(rng: &mut Rng) -> Shape {
    match rng.below(4) {
        0 => Shape::Circle(rng.int()),
        1 => Shape::Rect(rng.int(), rng.int()),
        2 => Shape::Label(rng.text()),
        _ => Shape::Empty,
    }
}

fn shape_val(s: &Shape) -> Val {
    let case =
        |name: &str, payload: Option<Val>| Val::Variant(name.to_string(), payload.map(Box::new));
    match s {
        Shape::Circle(r) => case("circle", Some(Val::S64(*r))),
        Shape::Rect(w, h) => case("rect", Some(Val::Tuple(vec![Val::S64(*w), Val::S64(*h)]))),
        Shape::Label(t) => case("label", Some(Val::String(t.clone()))),
        Shape::Empty => case("empty", None),
    }
}

fn area(s: &Shape) -> i64 {
    match s {
        Shape::Circle(r) => 3 * r * r,
        Shape::Rect(w, h) => w * h,
        Shape::Label(_) => 0,
        Shape::Empty => -1,
    }
}

fn compiled(id: &str) -> Runnable {
    Runnable::new(compile(&units(&[("shapes.pw", PROGRAM)]), id))
}

fn call(r: &Runnable, args: &[Val]) -> Val {
    r.call(&BTreeMap::new(), args).expect("runs").remove(0)
}

fn text(s: &str) -> Val {
    Val::String(s.to_string())
}

#[test]
fn each_case_is_built() {
    let make = compiled("shapes.Make");
    let mut rng = Rng(0x5A11);
    for _ in 0..CASES {
        let (kind, n, t) = (rng.below(5) as i64, rng.int(), rng.text());
        let expected = match kind {
            0 => Shape::Circle(n),
            1 => Shape::Rect(n, n + 1),
            2 => Shape::Label(t.clone()),
            _ => Shape::Empty,
        };
        let out = call(&make, &[Val::S64(kind), Val::S64(n), text(&t)]);
        assert_eq!(out, shape_val(&expected), "Make({kind}, {n}, {t:?})");
    }
}

#[test]
fn a_parameter_is_matched_case_by_case() {
    // A `Shape` parameter arrives flat: a discriminant, then slots its
    // cases share (an `i64`, then two `i32`s). The match reads it from
    // memory, after writing it there.
    let area_q = compiled("shapes.Area");
    let describe = compiled("shapes.Describe");
    let kind = compiled("shapes.Kind");
    let mut rng = Rng(0xA2EA);
    for _ in 0..CASES {
        let s = shape(&mut rng);
        let v = shape_val(&s);
        assert_eq!(
            call(&area_q, std::slice::from_ref(&v)),
            Val::S64(area(&s)),
            "Area({s:?})"
        );
        let described = match &s {
            Shape::Circle(r) => format!("circle {r}"),
            Shape::Label(t) => format!("label {t}"),
            _ => "other".to_string(),
        };
        assert_eq!(
            call(&describe, std::slice::from_ref(&v)),
            text(&described),
            "Describe({s:?})"
        );
        let kinded = match &s {
            Shape::Circle(_) | Shape::Rect(..) => "figure".to_string(),
            Shape::Label(t) => format!("text {t}"),
            Shape::Empty => "nothing".to_string(),
        };
        assert_eq!(call(&kind, &[v]), text(&kinded), "Kind({s:?})");
    }
}

#[test]
fn a_parameter_is_returned_as_itself() {
    // Flat in, through memory out: the joined slots written back as each
    // case lays its payload out.
    let same = compiled("shapes.Same");
    let mut rng = Rng(0x5A3E);
    for _ in 0..CASES {
        let v = shape_val(&shape(&mut rng));
        assert_eq!(call(&same, std::slice::from_ref(&v)), v);
    }
}

#[test]
fn a_case_is_a_function_value_and_a_bare_case_a_value() {
    let circles = compiled("shapes.Circles");
    let radii = [3, -1, 0, 7];
    let out = call(
        &circles,
        &[Val::List(radii.iter().map(|r| Val::S64(*r)).collect())],
    );
    assert_eq!(
        out,
        Val::List(
            radii
                .iter()
                .map(|r| shape_val(&Shape::Circle(*r)))
                .collect()
        )
    );
    assert_eq!(
        call(&compiled("shapes.Nothing"), &[]),
        shape_val(&Shape::Empty)
    );
}

#[test]
fn a_case_inside_a_record_and_a_list() {
    let total = compiled("shapes.Total");
    let renamed = compiled("shapes.Renamed");
    let mut rng = Rng(0x7074);
    for _ in 0..CASES {
        let shapes: Vec<Shape> = (0..rng.below(6)).map(|_| shape(&mut rng)).collect();
        let holders: Vec<Val> = shapes
            .iter()
            .map(|s| {
                Val::Record(vec![
                    ("shape".into(), shape_val(s)),
                    ("name".into(), text("n")),
                ])
            })
            .collect();
        let expected: i64 = shapes.iter().map(area).sum();
        assert_eq!(
            call(&total, &[Val::List(holders.clone())]),
            Val::S64(expected)
        );
        if let Some(h) = holders.first() {
            let Val::Record(fs) = h else { unreachable!() };
            let out = call(&renamed, &[h.clone(), text("m")]);
            assert_eq!(
                out,
                Val::Record(vec![
                    ("shape".into(), fs[0].1.clone()),
                    ("name".into(), text("m"))
                ])
            );
        }
    }
}

#[test]
fn every_joined_slot_is_read_and_written() {
    // `Mixed`'s first slot is shared by an `Int` (i64), a `Float` (f64,
    // reinterpreted), a `String`'s pointer and a `Bool` (i32, extended); its
    // second by a `String`'s length and a pair's pointer.
    let mix = compiled("shapes.Mix");
    let mut rng = Rng(0x313C);
    let case = |name: &str, payload: Option<Val>| Val::Variant(name.into(), payload.map(Box::new));
    for _ in 0..CASES {
        let (input, expected) = match rng.below(6) {
            0 => {
                let n = rng.int();
                (
                    case("i", Some(Val::S64(n))),
                    case("i", Some(Val::S64(n + 1))),
                )
            }
            1 => {
                let x = rng.float();
                (
                    case("f", Some(Val::Float64(x))),
                    case("f", Some(Val::Float64(x * 2.0))),
                )
            }
            2 => {
                let t = rng.text();
                (
                    case("s", Some(text(&t))),
                    case("s", Some(text(&format!("{t}!")))),
                )
            }
            3 => {
                let b = rng.below(2) == 1;
                (
                    case("b", Some(Val::Bool(b))),
                    case("b", Some(Val::Bool(!b))),
                )
            }
            4 => {
                let (x, t) = (rng.float(), rng.text());
                (
                    case("pair", Some(Val::Tuple(vec![Val::Float64(x), text(&t)]))),
                    case(
                        "pair",
                        Some(Val::Tuple(vec![
                            Val::Float64(x + 1.0),
                            text(&format!("<{t}>")),
                        ])),
                    ),
                )
            }
            _ => (case("blank", None), case("blank", None)),
        };
        assert_eq!(
            call(&mix, std::slice::from_ref(&input)),
            expected,
            "Mix({input:?})"
        );
    }
}

#[test]
fn a_case_inside_an_option() {
    let maybe = compiled("shapes.Maybe");
    let mut rng = Rng(0x0971);
    for _ in 0..CASES {
        let s = (rng.below(3) > 0).then(|| shape(&mut rng));
        let arg = Val::Option(s.as_ref().map(|s| Box::new(shape_val(s))));
        let expected = s.as_ref().map(area).unwrap_or(-2);
        assert_eq!(call(&maybe, &[arg]), Val::S64(expected), "Maybe({s:?})");
    }
}

#[test]
fn a_hosts_answer_is_matched_by_its_case() {
    // `Failure` crosses the boundary in `fetch`'s result, so the world's own
    // `variant` is what the match reads.
    let calls = Calls::default();
    let ops = BTreeMap::from([operation(&calls, "shapes:data/store#fetch", |args| {
        let [Val::String(key)] = args else {
            return Val::Result(Err(None));
        };
        let failure = |name: &str, payload: Option<Val>| {
            Val::Result(Err(Some(Box::new(Val::Variant(
                name.into(),
                payload.map(Box::new),
            )))))
        };
        match key.as_str() {
            "gone" => failure("missing", None),
            "bad" => failure("broken", Some(Val::String("torn page".into()))),
            other => Val::Result(Ok(Some(Box::new(Val::String(format!("value of {other}")))))),
        }
    })]);
    let fetched = compiled("shapes.Fetched");
    for (key, expected) in [
        ("gone", "missing"),
        ("bad", "broken: torn page"),
        ("ok", "value of ok"),
    ] {
        let out = fetched.call(&ops, &[text(key)]).expect("runs").remove(0);
        assert_eq!(out, text(expected), "Fetched({key})");
    }
    assert_eq!(calls.lock().unwrap().len(), 3);
}

/// A sum type of more cases than an 8-bit discriminant holds: its
/// discriminant is 16 bits, as the Canonical ABI gives 257 to 65,536 cases.
#[test]
fn a_type_of_three_hundred_cases_has_a_wider_discriminant() {
    let cases: Vec<String> = (0..300)
        .map(|i| match i % 3 {
            0 => format!("C{i}"),
            1 => format!("C{i}(Int)"),
            _ => format!("C{i}(String)"),
        })
        .collect();
    let arms: String = (0..300)
        .map(|i| match i % 3 {
            0 => format!("        C{i} => {i},\n"),
            1 => format!("        C{i}(n) => {i} + n,\n"),
            _ => format!("        C{i}(t) => {i} + String.length(t),\n"),
        })
        .collect();
    let program = format!(
        "module wide\n\nimport String\n\ntype Wide =\n    | {}\n\npublic query Which(w: Wide) -> Int {{\n    match w {{\n{arms}    }}\n}}\n\npublic query Last(n: Int) -> Wide {{ Wide.C299(\"{{n}}\") }}\n",
        cases.join("\n    | ")
    );
    let which = Runnable::new(compile(&units(&[("wide.pw", &program)]), "wide.Which"));
    for i in [0usize, 1, 2, 255, 256, 257, 298, 299] {
        let name = format!("c{i}");
        let (arg, expected) = match i % 3 {
            0 => (Val::Variant(name, None), i as i64),
            1 => (
                Val::Variant(name, Some(Box::new(Val::S64(5)))),
                i as i64 + 5,
            ),
            _ => (
                Val::Variant(name, Some(Box::new(Val::String("人人".into())))),
                i as i64 + 2,
            ),
        };
        assert_eq!(call(&which, &[arg]), Val::S64(expected), "case {i}");
    }
    let last = Runnable::new(compile(&units(&[("wide.pw", &program)]), "wide.Last"));
    assert_eq!(
        call(&last, &[Val::S64(7)]),
        Val::Variant("c299".into(), Some(Box::new(text("7"))))
    );
}

fn refused(program: &str, id: &str) -> String {
    let err = pw_core::backend::component::compile(&units(&[("m.pw", program)]), id)
        .map(|_| ())
        .expect_err("refused");
    println!("{id}: {err}");
    err
}

#[test]
fn a_union_of_one_case_is_a_variant() {
    // Until 2026-09-26 `wit.rs` wrote a union of one case as a record of
    // the declaration's fields, which a union has none of: `tuple<>`.
    let program = "module one\n\ntype Only =\n    | Only(Int)\n\npublic query Wrap(n: Int) -> Only { Only.Only(n) }\n\npublic query Unwrapped(o: Only) -> Int {\n    match o {\n        Only(n) => n,\n    }\n}\n";
    let wrap = Runnable::new(compile(&units(&[("one.pw", program)]), "one.Wrap"));
    let only = Val::Variant("only".into(), Some(Box::new(Val::S64(42))));
    assert_eq!(call(&wrap, &[Val::S64(42)]), only);
    let unwrapped = Runnable::new(compile(&units(&[("one.pw", program)]), "one.Unwrapped"));
    assert_eq!(call(&unwrapped, &[only]), Val::S64(42));
}

#[test]
fn a_type_that_contains_itself_is_refused_by_name() {
    let err = refused(
        "module m\n\ntype Chain =\n    | End\n    | Link(Int, Chain)\n\npublic query Length(c: Chain) -> Int {\n    match c {\n        End => 0,\n        Link(_, rest) => 1,\n    }\n}\n",
        "m.Length",
    );
    assert!(err.contains("a type that contains itself"), "{err}");
}

#[test]
fn a_generic_sum_type_is_refused_by_name() {
    // A `Type::Nominal` carries no type arguments, so a generic type's
    // layout is not known to the backend (KNOWN_LIMITATIONS).
    let err = refused(
        "module m\n\ntype Maybe<T> =\n    | Nothing\n    | Just(T)\n\npublic query Unwrap(n: Int) -> Int {\n    match Maybe.Just(n) {\n        Just(x) => x,\n        Nothing => 0,\n    }\n}\n",
        "m.Unwrap",
    );
    assert!(err.contains("a generic sum type"), "{err}");
}
