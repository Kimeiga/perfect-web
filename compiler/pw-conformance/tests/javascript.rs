//! **Pure computation compiled to JavaScript agrees with the component**
//! (ADR-0044).
//!
//! Each query below is compiled twice from one backend IR: to a Wasm
//! component, run through the E8 host, and to an ES module, run under Node.
//! Both get the same generated arguments, and must return the same value or
//! both trap. The inputs are chosen where JavaScript and Pleris differ:
//! integers near the 64-bit bounds, negative division, strings whose UTF-16
//! order is not their code point order (U+FF61 and U+1F600), and the
//! characters ECMAScript's `trim` treats differently (U+FEFF and U+0085).

use std::collections::BTreeMap;

use pw_conformance::{Runnable, compile, units};
use pw_host::engine::{HostFn, Val};
use wasmtime::component::types::{ComponentItem, Type};

const PROGRAM: &str = "module j

import List
import String
import Float
import Map
import Set

type Word = Word { text: String, score: Int }

public query Arith(a: Int, b: Int) -> Int { a * 3 - b + a % 5 }

public query Divide(a: Int, b: Int) -> Int { a / b }

public query Remainder(a: Int, b: Int) -> Int { a % b }

public query Negate(a: Int) -> Int { -a }

public query Floats(x: Float, y: Float) -> Float { x * 2.5 - y / 3.0 + x }

public query FloatOrder(x: Float, y: Float) -> Bool { x < y | x == y }

public query Mean(xs: List<Int>) -> Float {
    if List.length(xs) == 0 { 0.0 } else { Float.from_int(List.fold(xs, 0, (t, x) => t + x)) / Float.from_int(List.length(xs)) }
}

public query Before(a: String, b: String) -> Bool { a < b }

public query Order(a: String, b: String) -> Int {
    if a < b { -1 } else { if a > b { 1 } else { 0 } }
}

public query Branch(n: Int, s: String) -> String {
    if n > 0 & String.length(s) > 2 { \"big {n}\" } else { \"small {s} {n > 5}\" }
}

public query Chars(text: String) -> Int { String.length(text) }

public query Points(text: String) -> List<Int> { String.codepoints(text) }

public query Rebuilt(points: List<Int>) -> String { String.from_codepoints(points) }

public query Trimmed(text: String) -> String { String.trim(text) }

public query Lower(text: String) -> String { String.to_lower_ascii(text) }

public query Inside(text: String, part: String) -> Bool {
    String.contains(text, part) & String.starts_with(text, part) | String.ends_with(text, part)
}

public query Joined(parts: List<String>, separator: String) -> String { String.join(parts, separator) }

public query Evens(xs: List<Int>) -> List<Int> { List.filter(xs, x => x % 2 == 0) }

public query Sum(xs: List<Int>) -> Int { List.fold(xs, 0, (t, x) => t + x) }

public query Doubled(xs: List<Int>) -> List<Int> { List.map(xs, x => x * 2) }

public query AnyNegative(xs: List<Int>) -> Bool { List.any(xs, x => x < 0) }

public query AllSmall(xs: List<Int>) -> Bool { List.all(xs, x => x < 100) }

public query Sorted(words: List<Word>) -> List<Word> {
    List.sort_by(words, (a, b) => if a.score < b.score { 1 } else { if a.score > b.score { -1 } else { 0 } })
}

public query Runs(words: List<Word>) -> List<List<Word>> { List.group_by(words, w => w.text) }

public query Nth(xs: List<Int>, i: Int) -> Option<Int> { List.get(xs, i) }

public query FirstBig(xs: List<Int>) -> Option<Int> { List.find(xs, x => x > 100) }

public query Front(xs: List<Int>, n: Int) -> List<Int> { List.take(xs, n) }

public query Both(xs: List<Int>, ys: List<Int>) -> List<Int> { List.concat(xs, ys) }

public query Build(n: Int, text: String) -> Word { Word { text: \"{text}!\", score: n + 1 } }

public query Describe(w: Word) -> String { \"{w.text}: {w.score}\" }

public query Head(xs: List<Int>) -> Int {
    match List.get(xs, 0) {
        Some(x) => x,
        None => -1,
    }
}

public query AsFloat(n: Int) -> Float { Float.from_int(n) }

// ADR-0050: recursion, compiled beside the query in both. The inputs are
// bounded, so a generated `Int` does not ask for a depth nothing has.
fn fib(n: Int) -> Int { if n < 2 { n } else { fib(n - 1) + fib(n - 2) } }

public query Fib(n: Int) -> Int { fib(n % 20) }

fn is_even(n: Int) -> Bool { if n == 0 { true } else { is_odd(n - 1) } }

fn is_odd(n: Int) -> Bool { if n == 0 { false } else { is_even(n - 1) } }

public query Even(n: Int) -> Bool { is_even(n % 64) }

fn count_from<T>(xs: List<T>, i: Int) -> Int {
    match List.get(xs, i) {
        Some(x) => 1 + count_from(xs, i + 1),
        None => 0,
    }
}

public query CountWords(words: List<Word>) -> Int { count_from(words, 0) }

public query CountInts(xs: List<Int>) -> Int { count_from(xs, 0) }

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

// ADR-0051: an early `return`, `?`, and `for` loops with `mut` bindings.
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

public query Stitched(words: List<Word>) -> String {
    let mut out = \"\"
    for w in words {
        out = \"{out}{w.text}/\"
    }
    out
}

fn half(n: Int) -> Result<Int, String> {
    if n % 2 == 0 { Ok(n / 2) } else { Err(\"odd: {n}\") }
}

public query Quarter(n: Int) -> Result<Int, String> {
    let h = half(n)?
    let q = half(h)?
    Ok(q)
}

// ADR-0052: a function is a value. Closures capture, are passed, returned,
// and called through, in both.
fn apply_to(f: fn(Int) -> Int, x: Int) -> Int { f(x) }

public query Tripled(x: Int) -> Int { apply_to(y => y * 3, x) }

fn adder(k: Int) -> fn(Int) -> Int {
    x => x + k
}

public query Added(k: Int, xs: List<Int>) -> List<Int> {
    let add = adder(k)
    List.map(xs, add)
}

public query Labelled(words: List<Word>, tag: String) -> List<String> {
    let label: fn(Word) -> String = w => \"{tag}:{w.text}:{w.score}\"
    List.map(words, label)
}

public query Picked(flag: Bool, x: Int) -> Int {
    let f: fn(Int) -> Int = if flag { y => y * 10 } else { y => y - 10 }
    f(x)
}

public query SumOf(xs: List<Int>, i: Int) -> Option<Int> {
    let a = List.get(xs, i)?
    let b = List.get(xs, i + 1)?
    Some(a + b)
}

// ADR-0049: a string's escapes are the language's, the same in both.
public query Escapes() -> String { \"tab\\there \\\"q\\\" \\\\ \\{x\\} \\u{1F600}\\n\" }

public query Framed(text: String) -> String { \"[\\t{text}\\n]\" }

// ADR-0054: an opaque value is its representation, in both.
opaque type Count = Int

opaque type Tag = String

fn counted(n: Int) -> Count { Count(n) }

public query Counts(xs: List<Int>) -> List<Count> { List.map(xs, x => counted(x * 2)) }

public query CountTotal(cs: List<Count>) -> Int { List.fold(cs, 0, (t, c) => t + c.value) }

public query Tagged(s: String) -> Tag { Tag(\"#{s}\") }

public query Untagged(t: Tag) -> String { t.value }

// ADR-0055: slicing, and `sum` and `maximum` computed.
public query Rest(xs: List<Int>, n: Int) -> List<Int> { List.drop(xs, n) }

public query Part(xs: List<Int>, a: Int, b: Int) -> List<Int> { List.slice(xs, a, b) }

public query Backwards(words: List<Word>) -> List<Word> { List.reverse(words) }

public query TextPart(text: String, a: Int, b: Int) -> String { String.slice(text, a, b) }

public query FloatTotal(xs: List<Float>) -> Float { List.sum(xs) }

public query FloatMax(xs: List<Float>) -> Option<Float> { List.maximum(xs) }

// ADR-0056: Unicode case mapping, from one set of tables in both.
public query CaseLower(text: String) -> String { String.to_lower(text) }

public query CaseUpper(text: String) -> String { String.to_upper(text) }

// ADR-0057: maps and sets, arrays in ascending key order in both.
public query Tally(words: List<String>) -> List<Int> {
    let mut counts: Map<String, Int> = Map.empty()
    for w in words {
        let n = match Map.get(counts, w) {
            Some(c) => c + 1,
            None => 1,
        }
        counts = Map.insert(counts, w, n)
    }
    Map.values(counts)
}

public query ByText(words: List<Word>) -> List<String> {
    Map.keys(Map.from_lists(List.map(words, w => w.text), words))
}

public query ScoreOf(words: List<Word>, k: String) -> Option<Word> {
    Map.get(Map.remove(Map.from_lists(List.map(words, w => w.text), words), \"\"), k)
}

public query Unique(xs: List<Int>) -> List<Int> { Set.to_list(Set.from_list(xs)) }

public query Overlap(xs: List<Int>, ys: List<Int>) -> List<Int> {
    Set.to_list(Set.intersection(Set.from_list(xs), Set.from_list(ys)))
}

public query Either(xs: List<String>, ys: List<String>) -> List<String> {
    Set.to_list(Set.union(Set.from_list(xs), Set.from_list(ys)))
}

public query Only(xs: List<Int>, ys: List<Int>) -> List<Int> {
    Set.to_list(Set.difference(Set.insert(Set.from_list(xs), 0), Set.from_list(ys)))
}

public query Given(m: Map<String, Int>) -> Int { Map.size(m) }

// Keyed by a text's length, so keys repeat, and the last of each is kept.
public query LastOf(words: List<Word>) -> List<Int> {
    List.map(Map.values(Map.from_lists(List.map(words, w => String.length(w.text)), words)), w => w.score)
}

// ADR-0059: a declared sum type, built and matched, in both: a case as
// `{ $case, value }`, its fields an array when there are several.
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

fn shape_area(s: Shape) -> Int {
    match s {
        Circle(r) => r * r,
        Rect(w, h) => w * h,
        Label(t) => String.length(t),
        Empty => -1,
    }
}

public query ShapeArea(s: Shape) -> Int { shape_area(s) }

public query ShapeKind(s: Shape) -> String {
    match s {
        Shape.Circle(_) | Shape.Rect(_, _) => \"figure\",
        Label(t) => \"text {t}\",
        other => \"nothing\",
    }
}

public query ShapeMade(kind: Int, n: Int, text: String) -> Shape {
    if kind % 4 == 0 {
        Shape.Circle(n)
    } else {
        if kind % 4 == 1 { Shape.Rect(n, kind) } else { if kind % 4 == 2 { Shape.Label(text) } else { Empty } }
    }
}

public query ShapeSame(s: Shape) -> Shape { s }

public query ShapeCircles(radii: List<Int>) -> List<Shape> { List.map(radii, Shape.Circle) }

public query ShapeBuilt(n: Int) -> Shape {
    let make: fn(Int) -> Shape = Shape.Circle
    make(n)
}

public query ShapeTotal(shapes: List<Shape>) -> Int { List.fold(shapes, 0, (t, s) => t + shape_area(s)) }

public query Mixing(m: Mixed) -> Mixed {
    match m {
        I(n) => Mixed.S(\"{n}\"),
        F(x) => Mixed.F(x * 2.0),
        S(t) => Mixed.Pair(1.5, t),
        B(b) => Mixed.B(!b),
        Pair(x, t) => Mixed.I(String.length(t)),
        Blank => Mixed.Blank,
    }
}

// ADR-0062: generic records and sum types, a layout per instance in both.
type Holder<T> = Holder { item: T, note: String }

type Either<A, B> =
    | Left(A)
    | Right(B)

public query GenHeld(n: Int, s: String) -> String {
    let a = Holder { item: n, note: s }
    let b = Holder { item: s, note: \"x\" }
    \"{a.item}{a.note}{b.item}{b.note}\"
}

fn parity(x: Int) -> Either<Int, String> {
    if x % 2 == 0 { Either.Left(x) } else { Either.Right(\"{x}\") }
}

public query GenEither(xs: List<Int>) -> Int {
    List.fold(List.map(xs, parity), 0, (t, e) => match e {
        Left(n) => t + n,
        Right(s) => t + String.length(s),
    })
}

// ADR-0060: nested and literal patterns, compiled to a decision tree in
// both.
public query PatNested(o: Option<Option<Int>>) -> Int {
    match o {
        Some(Some(x)) => x,
        Some(None) => 0,
        None => -1,
    }
}

public query PatInner(o: Option<Shape>) -> String {
    match o {
        Some(Circle(0)) => \"dot\",
        Some(Circle(r)) => \"circle {r}\",
        Some(Rect(w, 1)) => \"thin {w}\",
        Some(Empty) => \"empty\",
        Some(_) => \"other\",
        None => \"none\",
    }
}

public query PatFlag(b: Bool, n: Int) -> Int {
    match b {
        true => n,
        false => 0 - n,
    }
}

public query PatWords(n: Int) -> String {
    match n {
        -1 => \"minus one\",
        0 => \"zero\",
        1 | 2 => \"small\",
        _ => \"many\",
    }
}

public query PatGreet(s: String) -> String {
    match s {
        \"a\" => \"letter\",
        \"\" => \"nothing\",
        other => \"{other}?\",
    }
}

public query MaybeShape(s: Option<Shape>) -> Int {
    match s {
        Some(x) => shape_area(x),
        None => -2,
    }
}
";

const CASES: usize = 200;

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
    /// Integers where Pleris and JavaScript differ: the 64-bit bounds, 2^53,
    /// zero, small negatives.
    fn int(&mut self) -> i64 {
        match self.below(10) {
            0 => i64::MIN,
            1 => i64::MAX,
            2 => (1 << 53) + self.below(3) as i64,
            3 => 0,
            4..=6 => self.below(21) as i64 - 10,
            7 => self.below(2000) as i64 - 1000,
            _ => self.next() as i64,
        }
    }
    fn string(&mut self) -> String {
        const POOLS: [&str; 6] = [
            "abcXYZ ",
            "人水ひと",
            "\u{FF61}\u{1F600}\u{10000}\u{FFFD}",
            " \t\n\u{85}\u{A0}\u{3000}\u{FEFF}\u{2028}",
            "aAbB_-.",
            // Cased beyond ASCII (ADR-0056): each grows, shrinks or is one
            // Unicode maps specially.
            "ßẞİıſΣςΐДЖǅﬃ𐐀𐐨",
        ];
        let pool: Vec<char> = POOLS[self.below(POOLS.len() as u64) as usize]
            .chars()
            .collect();
        (0..self.below(8))
            .map(|_| pool[self.below(pool.len() as u64) as usize])
            .collect()
    }
}

/// A random value of a component type.
fn val(rng: &mut Rng, ty: &Type, depth: u32) -> Val {
    let len = |rng: &mut Rng| if depth > 2 { 0 } else { rng.below(6) as usize };
    match ty {
        Type::Bool => Val::Bool(rng.below(2) == 1),
        Type::S64 => Val::S64(rng.int()),
        Type::Float64 => Val::Float64(match rng.below(8) {
            0 => 0.0,
            1 => -0.0,
            2 => 1e308,
            _ => (rng.next() as i64 as f64) / 7.0,
        }),
        Type::String => Val::String(rng.string()),
        Type::List(l) => {
            let element = l.ty();
            // Code points, valid or not: a list of `Int`s is sometimes one.
            if matches!(element, Type::S64) && rng.below(2) == 0 {
                return Val::List(
                    (0..len(rng))
                        .map(|_| {
                            Val::S64(match rng.below(6) {
                                0 => 0xD800 + rng.below(0x800) as i64,
                                1 => 0x110000,
                                2 => -1,
                                _ => rng.below(0x3000) as i64,
                            })
                        })
                        .collect(),
                );
            }
            Val::List(
                (0..len(rng))
                    .map(|_| val(rng, &element, depth + 1))
                    .collect(),
            )
        }
        Type::Record(r) => Val::Record(
            r.fields()
                .map(|f| (f.name.to_string(), val(rng, &f.ty, depth + 1)))
                .collect(),
        ),
        // A map's entry (ADR-0057). A generated map is rarely in order, so
        // the entry check is what the two are held to on most of them.
        Type::Tuple(t) => Val::Tuple(t.types().map(|ty| val(rng, &ty, depth + 1)).collect()),
        // A sum type's case (ADR-0059), and an option's.
        Type::Variant(v) => {
            let case = v
                .cases()
                .nth(rng.below(v.cases().len() as u64) as usize)
                .expect("a variant has a case");
            Val::Variant(
                case.name.to_string(),
                case.ty.map(|ty| Box::new(val(rng, &ty, depth + 1))),
            )
        }
        Type::Option(o) => {
            Val::Option((rng.below(3) > 0).then(|| Box::new(val(rng, &o.ty(), depth + 1))))
        }
        other => panic!("not generated: {other:?}"),
    }
}

/// The export's parameter types, read from the component.
fn params(r: &Runnable) -> Vec<Type> {
    let located = r.compiled().contract.exports[0]
        .component
        .clone()
        .expect("located");
    let mut config = wasmtime::Config::new();
    config.wasm_component_model(true);
    let engine = wasmtime::Engine::new(&config).expect("engine");
    let component = wasmtime::component::Component::new(&engine, &r.compiled().component.bytes)
        .expect("component");
    for (name, item) in component.component_type().exports(&engine) {
        let ComponentItem::ComponentInstance(i) = item.ty else {
            continue;
        };
        if name != located.interface {
            continue;
        }
        for (f, item) in i.exports(&engine) {
            if let ComponentItem::ComponentFunc(func) = item.ty
                && f == located.function
            {
                return func.params().map(|(_, t)| t).collect();
            }
        }
    }
    panic!("no export")
}

/// A value as JavaScript source, in the module's representation.
fn js(v: &Val) -> String {
    match v {
        Val::S64(n) => format!("{n}n"),
        Val::Float64(f) => {
            if f.is_nan() {
                "NaN".into()
            } else if f.is_infinite() {
                if *f > 0.0 { "Infinity" } else { "-Infinity" }.into()
            } else {
                format!("{f:?}")
            }
        }
        Val::Bool(b) => b.to_string(),
        Val::String(s) => serde_json::to_string(s).unwrap(),
        Val::List(xs) | Val::Tuple(xs) => {
            format!("[{}]", xs.iter().map(js).collect::<Vec<_>>().join(", "))
        }
        Val::Record(fs) => format!(
            "{{ {} }}",
            fs.iter()
                .map(|(k, v)| format!(
                    "{}: {}",
                    serde_json::to_string(&k.replace('-', "_")).unwrap(),
                    js(v)
                ))
                .collect::<Vec<_>>()
                .join(", ")
        ),
        // `{ $case, value }` (ADR-0044, ADR-0059).
        Val::Variant(case, payload) => match payload {
            Some(p) => format!(
                "{{ $case: {}, value: {} }}",
                serde_json::to_string(case).unwrap(),
                js(p)
            ),
            None => format!("{{ $case: {} }}", serde_json::to_string(case).unwrap()),
        },
        Val::Option(Some(x)) => format!("{{ $case: \"some\", value: {} }}", js(x)),
        Val::Option(None) => "{ $case: \"none\" }".to_string(),
        other => panic!("not written as JavaScript: {other:?}"),
    }
}

/// A value in the canonical form both sides are compared in. A float is its
/// bits, and every NaN one NaN.
fn canonical(v: &Val) -> serde_json::Value {
    use serde_json::json;
    match v {
        Val::S64(n) => json!({ "i": n.to_string() }),
        Val::Float64(f) if f.is_nan() => json!({ "f": "NaN" }),
        Val::Float64(f) => json!({ "f": format!("{:016x}", f.to_bits()) }),
        Val::Bool(b) => json!({ "b": b }),
        Val::String(s) => json!({ "s": s }),
        // A tuple is a JavaScript array, as a list is (ADR-0057).
        Val::List(xs) | Val::Tuple(xs) => {
            json!({ "l": xs.iter().map(canonical).collect::<Vec<_>>() })
        }
        Val::Record(fs) => json!({
            "r": fs.iter().map(|(k, v)| (k.replace('-', "_"), canonical(v))).collect::<serde_json::Map<_, _>>()
        }),
        Val::Option(Some(x)) => json!({ "some": canonical(x) }),
        Val::Option(None) => json!({ "none": null }),
        // `{ $case: "ok", value }` and `{ $case: "err", value }` (ADR-0044).
        Val::Result(Ok(Some(x))) => json!({ "ok": canonical(x) }),
        Val::Result(Err(Some(e))) => json!({ "err": canonical(e) }),
        // A declared case, by its WIT name (ADR-0059).
        Val::Variant(case, Some(p)) => json!({ case: canonical(p) }),
        Val::Variant(case, None) => json!({ case: null }),
        other => panic!("not compared: {other:?}"),
    }
}

/// Node's side of `canonical`, and a runner that calls each module on its
/// cases and prints one line per call.
const RUNNER: &str = r#"import { cases } from "./cases.mjs";
function bits(x) {
  const d = new DataView(new ArrayBuffer(8));
  d.setFloat64(0, x);
  return d.getBigUint64(0).toString(16).padStart(16, "0");
}
function enc(v) {
  if (typeof v === "bigint") return { i: v.toString() };
  if (typeof v === "number") return { f: Number.isNaN(v) ? "NaN" : bits(v) };
  if (typeof v === "boolean") return { b: v };
  if (typeof v === "string") return { s: v };
  if (Array.isArray(v)) return { l: v.map(enc) };
  if (v !== null && typeof v === "object" && "$case" in v)
    return "value" in v ? { [v.$case]: enc(v.value) } : { [v.$case]: null };
  if (v !== null && typeof v === "object")
    return { r: Object.fromEntries(Object.entries(v).map(([k, x]) => [k, enc(x)])) };
  return { unknown: String(v) };
}
for (const [id, list] of Object.entries(cases)) {
  const m = await import("./" + id + ".mjs");
  for (const args of list) {
    let out;
    try {
      out = enc(m.run(...args));
    } catch (e) {
      out = String(e.message).startsWith("trap:") ? { trap: true } : { error: String(e) };
    }
    console.log(JSON.stringify(out));
  }
}
"#;

/// Every query in `ids`, compiled both ways from `us`, called with the same
/// generated arguments. Returns (queries, calls, trapped in both).
fn agree(us: &[pw_core::check::Unit], ids: &[String], seed: u64) -> (usize, usize, usize) {
    let mut rng = Rng(seed);
    let cases: Vec<(String, Vec<Vec<Val>>)> = ids
        .iter()
        .map(|id| {
            let tys = params(&Runnable::new(compile(us, id)));
            let calls = (0..CASES)
                .map(|_| tys.iter().map(|t| val(&mut rng, t, 0)).collect())
                .collect();
            (id.clone(), calls)
        })
        .collect();
    agree_on(us, &cases, seed)
}

/// Each query, compiled both ways from `us`, called with the arguments
/// given for it. Returns (queries, calls, trapped in both).
fn agree_on(
    us: &[pw_core::check::Unit],
    cases: &[(String, Vec<Vec<Val>>)],
    tag: u64,
) -> (usize, usize, usize) {
    let dir = std::env::temp_dir().join(format!("pw-js-{}-{tag:x}", std::process::id()));
    std::fs::create_dir_all(&dir).expect("temp");
    let ops: BTreeMap<String, HostFn> = BTreeMap::new();
    let mut cases_js = Vec::new();
    let mut expected: Vec<(String, String, serde_json::Value)> = Vec::new();
    for (id, calls) in cases {
        let module = pw_core::backend::js_pure::module(us, id).unwrap_or_else(|e| panic!("{e}"));
        std::fs::write(dir.join(format!("{id}.mjs")), &module).expect("write");
        let r = Runnable::new(compile(us, id));
        let mut list = Vec::new();
        for args in calls {
            let want = match r.call(&ops, args) {
                Ok(v) => canonical(&v[0]),
                Err(_) => serde_json::json!({ "trap": true }),
            };
            list.push(format!(
                "[{}]",
                args.iter().map(js).collect::<Vec<_>>().join(", ")
            ));
            expected.push((id.clone(), format!("{args:?}"), want));
        }
        cases_js.push(format!(
            "  {}: [\n    {}\n  ]",
            serde_json::to_string(id).unwrap(),
            list.join(",\n    ")
        ));
    }
    std::fs::write(
        dir.join("cases.mjs"),
        format!("export const cases = {{\n{}\n}};\n", cases_js.join(",\n")),
    )
    .expect("write");
    std::fs::write(dir.join("runner.mjs"), RUNNER).expect("write");
    let out = std::process::Command::new("node")
        .arg("runner.mjs")
        .current_dir(&dir)
        .output()
        .expect("node runs: ADR-0044's modules are tested under Node");
    assert!(
        out.status.success(),
        "{}",
        String::from_utf8_lossy(&out.stderr)
    );
    let lines: Vec<serde_json::Value> = String::from_utf8(out.stdout)
        .unwrap()
        .lines()
        .map(|l| serde_json::from_str(l).expect("a line of JSON"))
        .collect();
    assert_eq!(lines.len(), expected.len());
    let mut traps = 0;
    for ((id, args, want), got) in expected.iter().zip(&lines) {
        assert_eq!(got, want, "{id}{args}");
        traps += usize::from(want.get("trap").is_some());
    }
    std::fs::remove_dir_all(&dir).ok();
    (cases.len(), expected.len(), traps)
}

#[test]
fn every_query_agrees_with_its_component_under_node() {
    let us = units(&[("j.pw", PROGRAM)]);
    let ids: Vec<String> = PROGRAM
        .lines()
        .filter_map(|l| l.strip_prefix("public query "))
        .map(|l| format!("j.{}", l.split('(').next().unwrap()))
        .collect();
    let (queries, calls, traps) = agree(&us, &ids, 0x15);
    println!(
        "javascript: {queries} queries, {calls} calls, component and module agree \
         ({traps} trapped in both)"
    );
}

/// **kiokun's shard rule, as the browser would run it.** kiokun.com's app
/// keeps the rule in TypeScript (`shard-utils.ts`); here it is the Pleris
/// rule `pw build` writes to `modules/`, held to its component.
#[test]
fn kiokuns_shard_rule_as_a_module_agrees_with_its_component() {
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    let mut paths: Vec<String> = std::fs::read_dir(root.join("examples/kiokun"))
        .expect("examples/kiokun")
        .map(|e| e.expect("entry").path())
        .filter(|p| p.extension().is_some_and(|x| x == "pw"))
        .map(|p| {
            format!(
                "examples/kiokun/{}",
                p.file_name().unwrap().to_string_lossy()
            )
        })
        .collect();
    paths.sort();
    let paths: Vec<&str> = paths.iter().map(String::as_str).collect();
    let program = pw_conformance::files(&paths);
    let program: Vec<(&str, &str)> = program
        .iter()
        .map(|(p, s)| (p.as_str(), s.as_str()))
        .collect();
    let us = units(&program);
    let ids = ["shards.Place".to_string(), "shards.Places".to_string()];
    let (queries, calls, _) = agree(&us, &ids, 0x16);
    println!(
        "javascript: kiokun's {queries} shard queries, {calls} calls, component and module agree"
    );
}

/// **The module's case mapping is Rust's, code point by code point**
/// (ADR-0056). `tests/case_mapping.rs` holds the component to the same, so
/// the two agree on every code point Unicode maps, not only on the ones a
/// generated string happens to contain.
#[test]
fn the_modules_case_mapping_is_rusts_for_every_code_point() {
    let us = units(&[("j.pw", PROGRAM)]);
    let dir = std::env::temp_dir().join(format!("pw-js-case-{}", std::process::id()));
    std::fs::create_dir_all(&dir).expect("temp");
    for id in ["j.CaseLower", "j.CaseUpper"] {
        let module = pw_core::backend::js_pure::module(&us, id).unwrap_or_else(|e| panic!("{e}"));
        std::fs::write(dir.join(format!("{id}.mjs")), module).expect("write");
    }
    let changed: Vec<char> = (0..=char::MAX as u32)
        .filter_map(char::from_u32)
        .filter(|c| {
            c.to_lowercase().ne(std::iter::once(*c)) || c.to_uppercase().ne(std::iter::once(*c))
        })
        .collect();
    let texts: Vec<String> = changed.chunks(400).map(|c| c.iter().collect()).collect();
    std::fs::write(
        dir.join("texts.json"),
        serde_json::to_string(&texts).expect("json"),
    )
    .expect("write");
    std::fs::write(
        dir.join("case.mjs"),
        "import { readFileSync } from \"node:fs\";\n\
         const texts = JSON.parse(readFileSync(\"texts.json\", \"utf8\"));\n\
         const lower = await import(\"./j.CaseLower.mjs\");\n\
         const upper = await import(\"./j.CaseUpper.mjs\");\n\
         console.log(JSON.stringify(texts.map((t) => [lower.run(t), upper.run(t)])));\n",
    )
    .expect("write");
    let out = std::process::Command::new("node")
        .arg("case.mjs")
        .current_dir(&dir)
        .output()
        .expect("node runs");
    assert!(
        out.status.success(),
        "{}",
        String::from_utf8_lossy(&out.stderr)
    );
    let got: Vec<(String, String)> = serde_json::from_slice(&out.stdout).expect("json");
    assert_eq!(got.len(), texts.len());
    for (t, (lo, up)) in texts.iter().zip(&got) {
        let want_lo: String = t.chars().flat_map(char::to_lowercase).collect();
        let want_up: String = t.chars().flat_map(char::to_uppercase).collect();
        assert_eq!(lo, &want_lo, "lower {t:?}");
        assert_eq!(up, &want_up, "upper {t:?}");
    }
    std::fs::remove_dir_all(&dir).ok();
}

/// **A map the module is given is checked as the component checks it**
/// (ADR-0057): in order, out of order, a key twice, and in code point order
/// that UTF-16's order reverses. A generated map is rarely any of these.
#[test]
fn the_modules_entry_check_is_the_components() {
    let us = units(&[("j.pw", PROGRAM)]);
    let entry = |k: &str, v: i64| Val::Tuple(vec![Val::String(k.to_string()), Val::S64(v)]);
    let maps = [
        vec![entry("a", 1), entry("b", 2)],
        vec![entry("b", 1), entry("a", 2)],
        vec![entry("a", 1), entry("a", 2)],
        vec![entry("\u{FF61}", 1), entry("\u{1F600}", 2)],
        vec![entry("\u{1F600}", 1), entry("\u{FF61}", 2)],
        vec![],
    ];
    let calls = maps.into_iter().map(|m| vec![Val::List(m)]).collect();
    let (queries, calls, traps) = agree_on(&us, &[("j.Given".to_string(), calls)], 0x57);
    assert_eq!((queries, calls, traps), (1, 6, 3));
}
