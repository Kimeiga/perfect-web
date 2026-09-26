//! **Pure computation, compiled, against exact references** (ADR-0039).
//!
//! Each operator runs through the E8 host over generated inputs, beside a Rust
//! statement of what it means computed in `i128`, where no `i64` operation can
//! overflow. So the reference shares nothing with the encoder's overflow
//! checks: an `Int` result is the exact value when it fits in 64 bits, and a
//! trap when it does not; `/` and `%` are Euclidean; a zero divisor traps.

use std::collections::BTreeMap;

use pw_conformance::{Runnable, compile, operation, units};
use pw_host::engine::Val;

const CASES: usize = 300;

/// xorshift64*: deterministic, so a failing case is reproducible.
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
    /// Mostly the edges: zero, ±1, the extremes and their neighbours, small
    /// values, and values near where a product or sum overflows.
    fn int(&mut self) -> i64 {
        match self.below(10) {
            0 => [0, 1, -1, 2, -2][self.below(5) as usize],
            1 => [i64::MIN, i64::MAX, i64::MIN + 1, i64::MAX - 1][self.below(4) as usize],
            2 => (self.next() as i64) >> 32,
            3 => 3_037_000_499 + self.below(3) as i64, // near sqrt(i64::MAX)
            4 => -(self.below(1000) as i64),
            5 => self.below(1000) as i64,
            _ => self.next() as i64,
        }
    }
    fn string(&mut self) -> String {
        const POOLS: [&str; 5] = [
            "abcdefghijklmnopqrstuvwxyz",
            "人市魚酒時夢空青上多面",
            "ひとみずかぜそらアイウエオ",
            "🀄🍣😀🇯🇵",
            "ÀéîõüÇñß",
        ];
        match self.below(8) {
            0 => String::new(),
            1 => "a".repeat(self.below(3) as usize),
            _ => {
                let pool: Vec<char> = POOLS[self.below(POOLS.len() as u64) as usize]
                    .chars()
                    .collect();
                (0..self.below(6))
                    .map(|_| pool[self.below(pool.len() as u64) as usize])
                    .collect()
            }
        }
    }
}

const PROGRAM: &str = "module m

type Pair = Pair {
    n: Int,
    text: String,
}

type Doc = Doc {
    s: String,
}

fn get(w: String) -> Option<String> !{ database.read<Doc> }
    host \"m:d/e#get\"

fn double(n: Int) -> Int {
    n * 2
}

fn describe(n: Int) -> String {
    if n < 0 { \"negative\" } else { if n == 0 { \"zero\" } else { \"positive\" } }
}

public query Add(a: Int, b: Int) -> Int { a + b }

public query Sub(a: Int, b: Int) -> Int { a - b }

public query Mul(a: Int, b: Int) -> Int { a * b }

public query Div(a: Int, b: Int) -> Int { a / b }

public query Rem(a: Int, b: Int) -> Int { a % b }

public query Neg(a: Int) -> Int { -a }

public query Order(a: Int, b: Int) -> Int {
    if a < b { -1 } else { if a > b { 1 } else { 0 } }
}

public query Before(a: String, b: String) -> Bool { a < b }

public query Same(a: String, b: String) -> Bool { a == b }

public query Text(n: Int, b: Bool, s: String) -> String { \"n={n} b={b} s={s}!\" }

public query Shape(n: Int) -> Pair { Pair { n: double(n), text: describe(n) } }

public query Guarded(a: Int, b: Int) -> Bool { b != 0 & a / b > 1 }

public query Either(a: Int, b: Int) -> Bool { b == 0 | a / b > 1 }

public query Not(b: Bool) -> Bool { !b }

public query Floats(a: Float, b: Float) -> Float { a * b - a / b }

public query Branch(w: String, look: Bool) -> Option<String> {
    if look { get(w) } else { None }
}
";

fn compiled(id: &str) -> Runnable {
    Runnable::new(compile(&units(&[("m.pw", PROGRAM)]), id))
}

fn call(r: &Runnable, args: &[Val]) -> Result<Val, String> {
    r.call(&BTreeMap::new(), args).map(|mut out| out.remove(0))
}

/// The compiled result must be the reference's value, or a trap where the
/// reference has none.
fn agree(name: &str, got: Result<Val, String>, want: Option<Val>, case: &str) {
    match (got, want) {
        (Ok(g), Some(w)) => assert_eq!(g, w, "{name}({case})"),
        (Err(_), None) => {}
        (Ok(g), None) => panic!("{name}({case}) = {g:?}, and the exact result has no Int"),
        (Err(e), Some(w)) => panic!("{name}({case}) trapped ({e}), and should be {w:?}"),
    }
}

/// An exact `i128` result as an `Int`, or no value when it does not fit.
fn int(exact: i128) -> Option<Val> {
    i64::try_from(exact).ok().map(Val::S64)
}

fn euclid_div(a: i64, b: i64) -> Option<Val> {
    (b != 0)
        .then(|| (a as i128).div_euclid(b as i128))
        .and_then(int)
}

fn euclid_rem(a: i64, b: i64) -> Option<Val> {
    (b != 0)
        .then(|| (a as i128).rem_euclid(b as i128))
        .and_then(int)
}

#[test]
fn int_arithmetic_is_exact_or_traps() {
    type Reference = fn(i64, i64) -> Option<Val>;
    let ops: [(&str, Reference); 5] = [
        ("m.Add", |a, b| int(a as i128 + b as i128)),
        ("m.Sub", |a, b| int(a as i128 - b as i128)),
        ("m.Mul", |a, b| int(a as i128 * b as i128)),
        ("m.Div", euclid_div),
        ("m.Rem", euclid_rem),
    ];
    for (i, (id, reference)) in ops.iter().enumerate() {
        let r = compiled(id);
        let mut rng = Rng(0x5eed_0000 + i as u64);
        let (mut values, mut traps) = (0, 0);
        for _ in 0..CASES {
            let (a, b) = (rng.int(), rng.int());
            let want = reference(a, b);
            if want.is_some() {
                values += 1;
            } else {
                traps += 1;
            }
            agree(
                id,
                call(&r, &[Val::S64(a), Val::S64(b)]),
                want,
                &format!("{a}, {b}"),
            );
        }
        println!("{id}: {CASES} cases, {values} values, {traps} traps, all as the reference");
    }
}

#[test]
fn euclidean_division_is_kokas() {
    // The four sign combinations, checked against Koka 3.2.3's answers
    // (ADR-0039 §1), and the two edges that are not errors.
    let (div, rem) = (compiled("m.Div"), compiled("m.Rem"));
    for (a, b, q, m) in [
        (-7, 2, -4, 1),
        (7, -2, -3, 1),
        (-7, -2, 4, 1),
        (7, 2, 3, 1),
        (i64::MIN, 1, i64::MIN, 0),
        (i64::MIN, 2, i64::MIN / 2, 0),
        (i64::MAX, -1, -i64::MAX, 0),
        (-1, i64::MIN, 1, i64::MAX),
    ] {
        assert_eq!(
            call(&div, &[Val::S64(a), Val::S64(b)]),
            Ok(Val::S64(q)),
            "{a} / {b}"
        );
        assert_eq!(
            call(&rem, &[Val::S64(a), Val::S64(b)]),
            Ok(Val::S64(m)),
            "{a} % {b}"
        );
    }
    // `MIN % -1` is 0, which fits; `MIN / -1` does not.
    assert_eq!(
        call(&rem, &[Val::S64(i64::MIN), Val::S64(-1)]),
        Ok(Val::S64(0))
    );
    assert!(call(&div, &[Val::S64(i64::MIN), Val::S64(-1)]).is_err());
    // A zero divisor traps, where Koka answers 0 and the dividend.
    assert!(call(&div, &[Val::S64(7), Val::S64(0)]).is_err());
    assert!(call(&rem, &[Val::S64(7), Val::S64(0)]).is_err());
}

#[test]
fn negation_and_comparison() {
    let (neg, order) = (compiled("m.Neg"), compiled("m.Order"));
    let mut rng = Rng(0x0e9);
    for _ in 0..CASES {
        let (a, b) = (rng.int(), rng.int());
        agree(
            "m.Neg",
            call(&neg, &[Val::S64(a)]),
            int(-(a as i128)),
            &a.to_string(),
        );
        assert_eq!(
            call(&order, &[Val::S64(a), Val::S64(b)]),
            Ok(Val::S64(a.cmp(&b) as i64)),
            "order({a}, {b})"
        );
    }
}

#[test]
fn strings_compare_by_their_bytes() {
    let (before, same) = (compiled("m.Before"), compiled("m.Same"));
    let mut rng = Rng(0x57);
    for _ in 0..CASES {
        let (a, b) = (rng.string(), rng.string());
        // One string compared with itself, a third of the time.
        let b = if rng.below(3) == 0 { a.clone() } else { b };
        let args = [Val::String(a.clone()), Val::String(b.clone())];
        assert_eq!(call(&before, &args), Ok(Val::Bool(a < b)), "{a:?} < {b:?}");
        assert_eq!(call(&same, &args), Ok(Val::Bool(a == b)), "{a:?} == {b:?}");
    }
}

#[test]
fn an_interpolation_writes_each_part_as_text() {
    let text = compiled("m.Text");
    let mut rng = Rng(0x7e);
    for n in [0, 1, -1, 42, i64::MIN, i64::MAX, 1_000_000_007] {
        let s = rng.string();
        for b in [true, false] {
            assert_eq!(
                call(&text, &[Val::S64(n), Val::Bool(b), Val::String(s.clone())]),
                Ok(Val::String(format!("n={n} b={b} s={s}!"))),
            );
        }
    }
    for _ in 0..CASES {
        let (n, s, b) = (rng.int(), rng.string(), rng.below(2) == 1);
        assert_eq!(
            call(&text, &[Val::S64(n), Val::Bool(b), Val::String(s.clone())]),
            Ok(Val::String(format!("n={n} b={b} s={s}!"))),
        );
    }
}

#[test]
fn a_record_built_from_inlined_calls() {
    let shape = compiled("m.Shape");
    let mut rng = Rng(0x5a);
    for _ in 0..CASES {
        let n = rng.int();
        let text = match n {
            n if n < 0 => "negative",
            0 => "zero",
            _ => "positive",
        };
        let want = int(2 * n as i128).map(|v| {
            Val::Record(vec![
                ("n".to_string(), v),
                ("text".to_string(), Val::String(text.to_string())),
            ])
        });
        agree(
            "m.Shape",
            call(&shape, &[Val::S64(n)]),
            want,
            &n.to_string(),
        );
    }
}

#[test]
fn logic_evaluates_its_right_side_only_when_it_decides() {
    // `b != 0 & a / b > 1` with `b == 0` would trap if `a / b` ran: `&` and
    // `|` are the language's logical operators, and they short-circuit.
    let (guarded, either, not) = (
        compiled("m.Guarded"),
        compiled("m.Either"),
        compiled("m.Not"),
    );
    let mut rng = Rng(0x10);
    for _ in 0..CASES {
        let a = rng.int();
        let b = if rng.below(3) == 0 { 0 } else { rng.int() };
        let quotient = euclid_div(a, b);
        let gt1 = |q: &Option<Val>| matches!(q, Some(Val::S64(q)) if *q > 1);
        let want_guarded = if b == 0 {
            Some(Val::Bool(false))
        } else {
            quotient.as_ref().map(|_| Val::Bool(gt1(&quotient)))
        };
        let want_either = if b == 0 {
            Some(Val::Bool(true))
        } else {
            quotient.as_ref().map(|_| Val::Bool(gt1(&quotient)))
        };
        let args = [Val::S64(a), Val::S64(b)];
        agree(
            "m.Guarded",
            call(&guarded, &args),
            want_guarded,
            &format!("{a}, {b}"),
        );
        agree(
            "m.Either",
            call(&either, &args),
            want_either,
            &format!("{a}, {b}"),
        );
    }
    for b in [true, false] {
        assert_eq!(call(&not, &[Val::Bool(b)]), Ok(Val::Bool(!b)));
    }
}

#[test]
fn floats_are_ieee() {
    let floats = compiled("m.Floats");
    let mut rng = Rng(0xf1);
    for _ in 0..CASES {
        let (a, b) = (
            (rng.int() as f64) / 1e9,
            if rng.below(8) == 0 {
                0.0
            } else {
                (rng.int() as f64) / 1e12
            },
        );
        let want = a * b - a / b;
        match call(&floats, &[Val::Float64(a), Val::Float64(b)]) {
            Ok(Val::Float64(g)) => assert!(
                g == want || (g.is_nan() && want.is_nan()),
                "floats({a}, {b}) = {g}, want {want}"
            ),
            other => panic!("floats({a}, {b}) = {other:?}"),
        }
    }
}

#[test]
fn an_import_called_only_in_a_branch_is_imported() {
    // Until 2026-09-25 the encoder read only the top of a body for the
    // imports it needed, so this component had no `get` to call.
    let r = compiled("m.Branch");
    assert!(
        r.compiled()
            .component
            .imports
            .iter()
            .any(|i| i.ends_with("#get")),
        "{:?}",
        r.compiled().component.imports
    );
    let calls = pw_conformance::Calls::default();
    let ops = BTreeMap::from([operation(&calls, "m:d/e#get", |args| match args {
        [Val::String(w)] => Val::Option(Some(Box::new(Val::String(format!("<{w}>"))))),
        _ => Val::Option(None),
    })]);
    let looked = r.call(&ops, &[Val::String("人".into()), Val::Bool(true)]);
    assert_eq!(
        looked,
        Ok(vec![Val::Option(Some(Box::new(Val::String(
            "<人>".into()
        ))))])
    );
    let skipped = r.call(&ops, &[Val::String("人".into()), Val::Bool(false)]);
    assert_eq!(skipped, Ok(vec![Val::Option(None)]));
    assert_eq!(
        calls.lock().unwrap().len(),
        1,
        "only the branch that ran called"
    );
}

/// The refusals, each by its reason.
fn refused(program: &str, id: &str) -> String {
    let err = pw_core::backend::component::compile(&units(&[("r.pw", program)]), id)
        .map(|_| ())
        .expect_err("refused");
    println!("{id}: {err}");
    err
}

#[test]
fn what_stays_outside_is_refused_by_name() {
    // A recursion and a generic callee were refused here until 2026-09-25.
    // Both compile now (ADR-0050): the recursion beside the export, the
    // generic callee at the types its arguments give it.
    let recursion = Runnable::new(compile(
        &units(&[(
            "r.pw",
            "module r\n\nfn down(n: Int) -> Int {\n    if n == 0 { 0 } else { down(n - 1) }\n}\n\npublic query Q(n: Int) -> Int { down(n) }\n",
        )]),
        "r.Q",
    ));
    assert_eq!(
        recursion.call(&BTreeMap::new(), &[Val::S64(5)]),
        Ok(vec![Val::S64(0)])
    );
    let generic = Runnable::new(compile(
        &units(&[(
            "r.pw",
            "module r\n\nfn same<T>(x: T) -> T { x }\n\npublic query Q(n: Int) -> Int { same(n) }\n",
        )]),
        "r.Q",
    ));
    assert_eq!(
        generic.call(&BTreeMap::new(), &[Val::S64(7)]),
        Ok(vec![Val::S64(7)])
    );

    // `pw check` refuses it now, before the backend sees it (PW0609,
    // ADR-0043). The backend's own refusal stays for operands the checker
    // cannot type.
    let mixed = refused(
        "module r\n\npublic query Q(a: Int, b: String) -> Bool { a == b }\n",
        "r.Q",
    );
    assert!(mixed.contains("PW0609"), "{mixed}");

    let float_rem = refused(
        "module r\n\npublic query Q(a: Float, b: Float) -> Float { a % b }\n",
        "r.Q",
    );
    assert!(float_rem.contains("`%` on a Float"), "{float_rem}");

    // An early `return` was refused here until 2026-09-25 (ADR-0051).
    let early = Runnable::new(compile(
        &units(&[(
            "r.pw",
            "module r\n\npublic query Q(a: Int) -> Int {\n    if a > 0 { return 1 } else { 0 }\n}\n",
        )]),
        "r.Q",
    ));
    assert_eq!(
        early.call(&BTreeMap::new(), &[Val::S64(5)]),
        Ok(vec![Val::S64(1)])
    );
    assert_eq!(
        early.call(&BTreeMap::new(), &[Val::S64(-5)]),
        Ok(vec![Val::S64(0)])
    );

    // A lambda a list operation runs is compiled into its loop, so a
    // `return` or an assignment there would leave or change the function
    // around it (ADR-0051).
    let lambda_return = refused(
        "module r\n\nimport List\n\npublic query Q(xs: List<Int>) -> List<Int> {\n    List.map(xs, x => {\n        return 1\n    })\n}\n",
        "r.Q",
    );
    assert!(
        lambda_return.contains("a `return` inside a function value"),
        "{lambda_return}"
    );
    let lambda_assign = refused(
        "module r\n\nimport List\n\npublic query Q(xs: List<Int>) -> Int {\n    let mut n = 0\n    let ys = List.map(xs, x => {\n        n = n + x\n        x\n    })\n    n\n}\n",
        "r.Q",
    );
    assert!(
        lambda_assign.contains("an assignment inside a function value"),
        "{lambda_assign}"
    );

    let no_else = refused(
        "module r\n\npublic query Q(a: Int) -> Int {\n    if a > 0 { 1 }\n}\n",
        "r.Q",
    );
    assert!(
        no_else.contains("without `else`") || no_else.contains("does not check"),
        "{no_else}"
    );
}

#[test]
fn a_program_that_does_not_parse_is_not_compiled() {
    // `&&` is not the language's operator: `a & (& b)` does not parse. The
    // backend used to compile the tree that survived a syntax error, because
    // a checked `Unit` carries its source and not its parse errors
    // (`backend::lower::Checked::of`, 2026-09-25).
    let err = refused(
        "module r\n\npublic query Q(a: Bool, b: Bool) -> Bool { a && b }\n",
        "r.Q",
    );
    assert!(
        err.contains("does not check") && err.contains("PW0009"),
        "{err}"
    );
}
