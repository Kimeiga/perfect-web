//! **Pure computation → JavaScript modules** (ADR-0044).
//!
//! Charter §14 M10 task 2, in its stated order:
//!
//! > first: generated modern JavaScript modules for handlers and pure
//! > computation; then: Wasm for pure compute-heavy modules.
//!
//! Handlers compile to modules since ADR-0033. This is the other half: a query
//! that computes and reaches no host becomes an ES module exporting `run`,
//! encoded from the same backend IR the component encoder reads
//! (`backend::ir`). The lowering is shared, so the two backends cannot type,
//! inline or resolve one declaration differently. Only the encoding differs,
//! and a differential test runs both on the same inputs
//! (`pw-conformance/tests/javascript.rs`).
//!
//! # Pleris semantics, not JavaScript's
//!
//! Every place the two languages differ is written out here rather than left
//! to the host language:
//! - an `Int` is a `BigInt`, checked into 64 bits after every `+`, `-`, `*`
//!   and negation, and `/` and `%` are Euclidean and trap on a zero divisor
//!   (ADR-0039 §1);
//! - a `String` is ordered by code point, where JavaScript's `<` orders UTF-16
//!   units and puts U+1F600 before U+FF61; its length counts code points;
//! - `String.trim` strips Unicode `White_Space`, which is not ECMAScript's
//!   set: JavaScript strips U+FEFF and keeps U+0085;
//! - a trap is a thrown `Error` whose message begins `trap:`.
//!
//! # Values
//!
//! An `Int` is a `BigInt`, a `Float` a number, a `Bool` a boolean, a `String`
//! a string, a list an array, and a record an object keyed by its Pleris field
//! names. `Some(v)` is `{ $case: "some", value: v }`, `None` is
//! `{ $case: "none" }`, and `Ok` and `Err` the same; `$` begins no Pleris
//! name, so a case can never be read as a record. A declared sum type's case
//! is named as its WIT case is, `Shape.Circle(r)` being
//! `{ $case: "circle", value: r }`, and a case of several fields holds them
//! in an array, as a tuple is held (ADR-0059). An opaque type is its
//! representation, as it is on a wire.

use std::collections::{BTreeMap, BTreeSet};

use crate::resolve::DefId;

use super::case::{self, Case};
use super::ir::{
    BinaryOp, BuiltinCase, Case as VariantCase, Const, EachKind, Function, Instr, Intrinsic,
    Program, Region, Shape, Terminator, Type, UnaryOp, ValueId,
};

/// **A query's pure computation, as an ES module.** `Err` names what the
/// encoding does not take, as the Wasm encoder's refusals do.
pub fn module(units: &[crate::check::Unit], component_id: &str) -> Result<String, String> {
    super::component::lowered(units, component_id, |function, program| {
        encode(component_id, function, program)
    })
}

/// **Every query of a program that reaches no host, as a module**, by
/// component id: what `pw build` writes to `modules/`.
pub fn modules(units: &[crate::check::Unit]) -> Result<Vec<(String, String)>, String> {
    super::component::pure_queries(units, |pure, program| {
        pure.iter()
            .map(|(id, function)| Ok((id.clone(), encode(id, function, program)?)))
            .collect()
    })
}

fn encode(component_id: &str, function: &Function, program: &Program) -> Result<String, String> {
    let refused = |why: String| format!("`{component_id}` is not a JavaScript module: {why}");
    // The functions compiled beside the query (ADR-0050), each a function
    // of the module, named by its position.
    let callees: BTreeMap<(DefId, Vec<Type>), usize> = function
        .callees
        .iter()
        .enumerate()
        .map(|(n, c)| ((c.def, c.instance.clone()), n))
        .collect();
    let mut helpers = BTreeSet::new();
    let mut sources = Vec::new();
    // The function values' code (ADR-0052): its captures, then its
    // parameters, as a function of the module.
    for (n, c) in function.closures.iter().enumerate() {
        let mut e = Emitter::new(program, &callees, &function.closures);
        e.function(&c.function).map_err(refused)?;
        helpers.extend(e.helpers.iter().copied());
        let params: Vec<String> = c.function.params.iter().map(|(v, _)| val(*v)).collect();
        sources.push(format!(
            "function {}({}) {{\n{}}}",
            closure_name(n),
            params.join(", "),
            e.body
        ));
    }
    for (n, c) in function.callees.iter().enumerate() {
        let mut e = Emitter::new(program, &callees, &function.closures);
        e.function(c).map_err(refused)?;
        helpers.extend(e.helpers.iter().copied());
        let params: Vec<String> = c.params.iter().map(|(v, _)| val(*v)).collect();
        sources.push(format!(
            "function {}({}) {{\n{}}}",
            callee_name(n),
            params.join(", "),
            e.body
        ));
    }
    let mut e = Emitter::new(program, &callees, &function.closures);
    e.function(function).map_err(refused)?;
    e.helpers.extend(helpers);
    Ok(e.finish(component_id, function, &sources))
}

/// **A resumable handler, as its ES module** (ADR-0058). `paths` are the
/// captured paths its parameters are, in order.
pub(crate) fn handler_source(
    function: &Function,
    program: &Program,
    identity: &str,
    name: &str,
    paths: &[String],
) -> Result<String, String> {
    let callees: BTreeMap<(DefId, Vec<Type>), usize> = function
        .callees
        .iter()
        .enumerate()
        .map(|(n, c)| ((c.def, c.instance.clone()), n))
        .collect();
    let mut helpers = BTreeSet::new();
    let mut sources = Vec::new();
    for (n, c) in function.closures.iter().enumerate() {
        let mut e = Emitter::new(program, &callees, &function.closures);
        e.function(&c.function)?;
        helpers.extend(e.helpers.iter().copied());
        let params: Vec<String> = c.function.params.iter().map(|(v, _)| val(*v)).collect();
        sources.push(format!(
            "function {}({}) {{\n{}}}",
            closure_name(n),
            params.join(", "),
            e.body
        ));
    }
    for (n, c) in function.callees.iter().enumerate() {
        let mut e = Emitter::new(program, &callees, &function.closures);
        e.function(c)?;
        helpers.extend(e.helpers.iter().copied());
        let params: Vec<String> = c.params.iter().map(|(v, _)| val(*v)).collect();
        sources.push(format!(
            "function {}({}) {{\n{}}}",
            callee_name(n),
            params.join(", "),
            e.body
        ));
    }
    let mut e = Emitter::new(program, &callees, &function.closures);
    e.handler = true;
    e.handler_function(function, paths)?;
    e.helpers.extend(helpers);
    Ok(e.finish_handler(identity, name, &sources))
}

fn json(s: &str) -> String {
    serde_json::Value::String(s.to_string()).to_string()
}

/// A function compiled beside the query, in the module (ADR-0050).
fn callee_name(n: usize) -> String {
    format!("callee{n}")
}

/// A function value's code, in the module (ADR-0052).
fn closure_name(n: usize) -> String {
    format!("closure{n}")
}

struct Emitter<'p> {
    program: &'p Program,
    /// The functions compiled beside the query, by instance (ADR-0050).
    callees: &'p BTreeMap<(DefId, Vec<Type>), usize>,
    /// The function values' code (ADR-0052).
    closures: &'p [super::ir::Closure],
    /// Each value's type, as the IR states it.
    types: BTreeMap<ValueId, Type>,
    /// The prelude functions the body calls.
    helpers: BTreeSet<&'static str>,
    /// A resumable handler's body (ADR-0058): a command it calls is awaited.
    /// Off in a function value's body, which is not async.
    handler: bool,
    body: String,
    depth: usize,
}

/// Each prelude function, by name, and what it calls.
const HELPERS: &[(&str, &[&str], &str)] = &[
    (
        "trap",
        &[],
        "function trap(what) {\n  throw new Error(\"trap: \" + what);\n}",
    ),
    (
        "int",
        &["trap"],
        "// An Int is 64 bits, and a result that does not fit traps (ADR-0039).\n\
         function int(v) {\n  if (v < -(2n ** 63n) || v > 2n ** 63n - 1n) trap(\"Int overflow\");\n  return v;\n}",
    ),
    (
        "div",
        &["trap", "int"],
        "// Euclidean: the remainder is in [0, |b|), so -7 / 2 is -4.\n\
         function div(a, b) {\n  if (b === 0n) trap(\"division by zero\");\n  let q = a / b;\n  \
         if (a % b < 0n) q = b > 0n ? q - 1n : q + 1n;\n  return int(q);\n}",
    ),
    (
        "rem",
        &["trap"],
        "function rem(a, b) {\n  if (b === 0n) trap(\"division by zero\");\n  const r = a % b;\n  \
         return r < 0n ? (b > 0n ? r + b : r - b) : r;\n}",
    ),
    (
        "compare",
        &[],
        "// By code point, as UTF-8's bytes order them. `<` on two JavaScript\n\
         // strings orders UTF-16 units, and puts U+1F600 before U+FF61.\n\
         function compare(a, b) {\n  const x = a[Symbol.iterator](), y = b[Symbol.iterator]();\n  \
         for (;;) {\n    const p = x.next(), q = y.next();\n    if (p.done || q.done) return p.done ? (q.done ? 0 : -1) : 1;\n    \
         const c = p.value.codePointAt(0) - q.value.codePointAt(0);\n    if (c !== 0) return c < 0 ? -1 : 1;\n  }\n}",
    ),
    (
        "sign",
        &[],
        "function sign(n) {\n  return n > 0n ? 1 : n < 0n ? -1 : 0;\n}",
    ),
    (
        "some",
        &[],
        "function some(value) {\n  return { $case: \"some\", value };\n}",
    ),
    (
        "none",
        &[],
        "const none = Object.freeze({ $case: \"none\" });",
    ),
    (
        "codepoints",
        &[],
        "function codepoints(s) {\n  return Array.from(s, (c) => BigInt(c.codePointAt(0)));\n}",
    ),
    (
        "from_codepoints",
        &["trap"],
        "// A value that is not a Unicode scalar value traps (ADR-0040).\n\
         function from_codepoints(points) {\n  let s = \"\";\n  for (const p of points) {\n    \
         if (p < 0n || p > 0x10FFFFn || (p >= 0xD800n && p <= 0xDFFFn)) trap(\"not a Unicode scalar value\");\n    \
         s += String.fromCodePoint(Number(p));\n  }\n  return s;\n}",
    ),
    (
        "trim",
        &[],
        "// Unicode White_Space, as Rust's `str::trim` and the component's\n\
         // `String.trim`: not ECMAScript's `trim`, which strips U+FEFF and keeps\n\
         // U+0085.\n\
         const WHITE = new Set([0x9, 0xA, 0xB, 0xC, 0xD, 0x20, 0x85, 0xA0, 0x1680, 0x2000, 0x2001,\n  \
         0x2002, 0x2003, 0x2004, 0x2005, 0x2006, 0x2007, 0x2008, 0x2009, 0x200A, 0x2028, 0x2029,\n  \
         0x202F, 0x205F, 0x3000]);\n\
         function trim(s) {\n  const cs = Array.from(s);\n  let a = 0, b = cs.length;\n  \
         while (a < b && WHITE.has(cs[a].codePointAt(0))) a++;\n  \
         while (b > a && WHITE.has(cs[b - 1].codePointAt(0))) b--;\n  return cs.slice(a, b).join(\"\");\n}",
    ),
    (
        "slice",
        &[],
        "// `start` up to `end`, each clamped to the array, as `List.slice` and\n\
         // `String.slice` (ADR-0055).\n\
         function slice(xs, start, end) {\n  const n = BigInt(xs.length);\n  \
         const clamp = (i) => (i < 0n ? 0n : i > n ? n : i);\n  const a = clamp(start), b = clamp(end);\n  \
         return b > a ? xs.slice(Number(a), Number(b)) : [];\n}",
    ),
    (
        "exact",
        &["trap"],
        "// An Int sent to a command as a JSON number (ADR-0058): exact within\n\
         // ±2^53, and a trap past it rather than a different number.\n\
         function exact(v) {\n  \
         if (v < -(2n ** 53n) || v > 2n ** 53n) trap(\"an Int a JavaScript number cannot carry exactly\");\n  \
         return Number(v);\n}",
    ),
    (
        "key_order",
        &["compare"],
        "// The order of two keys (ADR-0057): an Int by value, a String by code\n\
         // point, as the component orders them.\n\
         function key_order(a, b) {\n  return typeof a === \"bigint\" ? (a < b ? -1 : a > b ? 1 : 0) : compare(a, b);\n}",
    ),
    (
        "place",
        &["key_order"],
        "// Where `key` goes among entries sorted by `of`: the first not below it,\n\
         // and whether it is there.\n\
         function place(xs, key, of) {\n  let lo = 0, hi = xs.length;\n  while (lo < hi) {\n    \
         const mid = (lo + hi) >> 1;\n    if (key_order(of(xs[mid]), key) < 0) lo = mid + 1;\n    else hi = mid;\n  }\n  \
         return [lo, lo < xs.length && key_order(of(xs[lo]), key) === 0];\n}",
    ),
    (
        "by_key",
        &["key_order"],
        "// Sorted by key, stably, and of each run of equal keys the last.\n\
         function by_key(xs, of) {\n  const s = [...xs].sort((a, b) => key_order(of(a), of(b)));\n  \
         return s.filter((x, i) => i + 1 === s.length || key_order(of(x), of(s[i + 1])) !== 0);\n}",
    ),
    (
        "checked",
        &["key_order", "trap"],
        "// A map or set from outside: each key below the next, or a trap.\n\
         function checked(xs, of) {\n  for (let i = 1; i < xs.length; i++)\n    \
         if (key_order(of(xs[i - 1]), of(xs[i])) >= 0) trap(\"a map or set out of order\");\n  return xs;\n}",
    ),
    (
        "merged",
        &["key_order"],
        "// Two ascending sets in one pass: what to keep of an element only the\n\
         // first has, only the second has, and both have.\n\
         function merged(a, b, first, second, both) {\n  const out = [];\n  let i = 0, j = 0;\n  \
         while (i < a.length || j < b.length) {\n    \
         const o = j >= b.length ? -1 : i >= a.length ? 1 : key_order(a[i], b[j]);\n    \
         if (o < 0) { if (first) out.push(a[i]); i++; }\n    \
         else if (o > 0) { if (second) out.push(b[j]); j++; }\n    \
         else { if (both) out.push(a[i]); i++; j++; }\n  }\n  return out;\n}",
    ),
    (
        "case_map",
        &[],
        "// Each code point mapped by a case table the component's data segment\n\
         // holds too (ADR-0056): not `toLowerCase`, whose Unicode is the\n\
         // engine's. A table is its ranges and its multi entries, four numbers\n\
         // each, sorted by their first.\n\
         function case_map(s, { ranges, multi }) {\n  \
         // How many entries come before cp: first below it, or not above it.\n  \
         const before = (t, cp, inclusive) => {\n    let lo = 0, hi = t.length / 4;\n    \
         while (lo < hi) {\n      const mid = (lo + hi) >> 1;\n      \
         if (inclusive ? t[mid * 4] <= cp : t[mid * 4] < cp) lo = mid + 1;\n      else hi = mid;\n    }\n    \
         return lo;\n  };\n  let out = \"\";\n  for (const ch of s) {\n    const cp = ch.codePointAt(0);\n    \
         const m = before(multi, cp, false);\n    if (m < multi.length / 4 && multi[m * 4] === cp) {\n      \
         for (let k = 1; k <= 3 && multi[m * 4 + k] !== 0; k++) out += String.fromCodePoint(multi[m * 4 + k]);\n      \
         continue;\n    }\n    const r = before(ranges, cp, true) - 1;\n    \
         const on = r >= 0 && cp <= ranges[r * 4 + 1] && (cp - ranges[r * 4]) % ranges[r * 4 + 3] === 0;\n    \
         out += String.fromCodePoint(on ? cp + ranges[r * 4 + 2] : cp);\n  }\n  return out;\n}",
    ),
    (
        "lower_ascii",
        &[],
        "// `A`-`Z` only (ADR-0040).\n\
         function lower_ascii(s) {\n  return s.replace(/[A-Z]/g, (c) => String.fromCharCode(c.charCodeAt(0) + 32));\n}",
    ),
    (
        "group_by",
        &[],
        "// Runs of adjacent elements with equal keys, as `List.group_by`.\n\
         function group_by(items, key) {\n  const out = [];\n  let last;\n  for (const x of items) {\n    \
         const k = key(x);\n    if (out.length > 0 && k === last) out[out.length - 1].push(x);\n    \
         else out.push([x]);\n    last = k;\n  }\n  return out;\n}",
    ),
];

impl<'p> Emitter<'p> {
    fn new(
        program: &'p Program,
        callees: &'p BTreeMap<(DefId, Vec<Type>), usize>,
        closures: &'p [super::ir::Closure],
    ) -> Emitter<'p> {
        Emitter {
            program,
            callees,
            closures,
            types: BTreeMap::new(),
            helpers: BTreeSet::new(),
            handler: false,
            body: String::new(),
            depth: 1,
        }
    }

    /// The prelude this module's functions call: every helper a used helper
    /// calls, in the table's order, then the case tables used (ADR-0056).
    fn prelude(&self) -> Vec<String> {
        let mut used = self.helpers.clone();
        loop {
            let before = used.len();
            for (name, deps, _) in HELPERS {
                if used.contains(name) {
                    used.extend(deps.iter().copied());
                }
            }
            if used.len() == before {
                break;
            }
        }
        let mut out: Vec<String> = HELPERS
            .iter()
            .filter(|(name, _, _)| used.contains(name))
            .map(|(_, _, source)| source.to_string())
            .collect();
        out.extend(
            [("case_lower", Case::Lower), ("case_upper", Case::Upper)]
                .into_iter()
                .filter(|(name, _)| used.contains(name))
                .map(|(name, c)| case_table(name, c)),
        );
        out
    }

    fn finish(self, component_id: &str, function: &Function, callees: &[String]) -> String {
        let prelude = self.prelude();
        let prelude: Vec<&str> = prelude.iter().map(String::as_str).collect();
        let params: Vec<String> = function.params.iter().map(|(v, _)| val(*v)).collect();
        let mut before: Vec<&str> = prelude;
        before.extend(callees.iter().map(String::as_str));
        format!(
            "// Generated by pw: pure computation `{component_id}` (ADR-0044). Do not edit.\n\n\
             {}{}export function run({}) {{\n{}}}\n",
            before.join("\n\n"),
            if before.is_empty() { "" } else { "\n\n" },
            params.join(", "),
            self.body
        )
    }

    /// **A handler's module** (ADR-0058): its prelude and functions, the names
    /// the runtime reads, and `run(context)`.
    fn finish_handler(self, identity: &str, name: &str, sources: &[String]) -> String {
        let prelude = self.prelude();
        let mut before: Vec<&str> = prelude.iter().map(String::as_str).collect();
        before.extend(sources.iter().map(String::as_str));
        format!(
            "// Generated by pw: resumable handler {identity}. Do not edit.\n\n\
             {}{}export const name = {};\nexport const handler = {};\n\
             export async function run(context) {{\n{}}}\n",
            before.join("\n\n"),
            if before.is_empty() { "" } else { "\n\n" },
            json(name),
            json(identity),
            self.body
        )
    }

    /// **A handler's body** (ADR-0058): each captured path read from the
    /// document and decoded by its type, then the body, its commands awaited.
    fn handler_function(&mut self, f: &Function, paths: &[String]) -> Result<(), String> {
        let [entry] = f.blocks.as_slice() else {
            return Err(format!("{} blocks; a handler is one", f.blocks.len()));
        };
        let Terminator::Return(result) = &entry.terminator else {
            return Err("its body does not return a value".into());
        };
        for ((v, t), path) in f.params.iter().zip(paths) {
            let at: String = path.split('.').map(|s| format!("[{}]", json(s))).collect();
            let decoded = self.decode(&format!("context.captures{at}"), t)?;
            self.line(&format!("const {} = {decoded};", val(*v)));
            self.types.insert(*v, t.clone());
        }
        for i in &entry.instrs {
            self.instr(i)?;
        }
        self.line(&format!("return {};", val(*result)));
        Ok(())
    }

    /// An instance's shape: its declaration's, under its arguments
    /// (ADR-0062).
    fn shape(&self, def: crate::resolve::DefId, args: &[Type]) -> Option<&Shape> {
        self.program
            .types
            .iter()
            .find(|t| t.def == def && t.args == args)
            .map(|t| &t.shape)
    }

    /// **A captured value, from the JSON the renderer wrote** (ADR-0058). An
    /// `Int` is a JavaScript number there, exact within ±2^53 (ADR-0033 §7),
    /// and a `BigInt` here (ADR-0044); a record is an object by field name in
    /// both.
    fn decode(&self, expr: &str, ty: &Type) -> Result<String, String> {
        let refused = || format!("a captured {ty:?}, which the document does not carry");
        Ok(match ty {
            Type::Int => format!("BigInt({expr})"),
            Type::Float | Type::Str | Type::Bool => expr.to_string(),
            Type::List(t) => format!("{expr}.map((x) => {})", self.decode("x", t)?),
            Type::Nominal(def, args) => match self.shape(*def, args) {
                Some(Shape::Alias(of)) => self.decode(expr, of)?,
                Some(Shape::Record { fields }) => {
                    let fields: Vec<String> = fields
                        .iter()
                        .map(|(n, t)| {
                            Ok(format!(
                                "{}: {}",
                                json(n),
                                self.decode(&format!("o[{}]", json(n)), t)?
                            ))
                        })
                        .collect::<Result<_, String>>()?;
                    format!("((o) => ({{ {} }}))({expr})", fields.join(", "))
                }
                _ => return Err(refused()),
            },
            _ => return Err(refused()),
        })
    }

    /// **A value sent to a command, as JSON carries it** (ADR-0058): an `Int`
    /// as a number, exact within ±2^53 or a trap; an opaque type as its
    /// representation (ADR-0033 §4).
    fn wire(&mut self, v: &str, ty: &Type) -> Result<String, String> {
        match ty {
            Type::Int => Ok(format!("{}({v})", self.uses("exact"))),
            Type::Float | Type::Str | Type::Bool => Ok(v.to_string()),
            Type::Nominal(def, args) => match self.shape(*def, args).cloned() {
                Some(Shape::Alias(of)) => self.wire(v, &of),
                _ => Err(format!(
                    "a command argument of type {ty:?}, which a browser cannot send"
                )),
            },
            other => Err(format!(
                "a command argument of type {other:?}, which a browser cannot send"
            )),
        }
    }

    fn function(&mut self, f: &Function) -> Result<(), String> {
        if !f.capabilities.is_empty() {
            return Err("it reaches the host, so it is not pure computation".into());
        }
        let [entry] = f.blocks.as_slice() else {
            return Err(format!("{} blocks; a pure query is one", f.blocks.len()));
        };
        let Terminator::Return(result) = &entry.terminator else {
            return Err("its body does not return a value".into());
        };
        for (v, t) in &f.params {
            self.types.insert(*v, t.clone());
        }
        for i in &entry.instrs {
            self.instr(i)?;
        }
        self.line(&format!("return {};", val(*result)));
        Ok(())
    }

    fn line(&mut self, s: &str) {
        for _ in 0..self.depth {
            self.body.push_str("  ");
        }
        self.body.push_str(s);
        self.body.push('\n');
    }

    fn uses(&mut self, helper: &'static str) -> &'static str {
        self.helpers.insert(helper);
        helper
    }

    fn type_of(&self, v: ValueId) -> Result<&Type, String> {
        self.types
            .get(&v)
            .ok_or_else(|| format!("{} has no type", val(v)))
    }

    /// A region's instructions, then its value assigned to `target`.
    fn region(&mut self, r: &Region, target: ValueId) -> Result<(), String> {
        for i in &r.instrs {
            self.instr(i)?;
        }
        self.line(&format!("{} = {};", val(target), val(r.value)));
        Ok(())
    }

    /// A region as an arrow function of `params`.
    fn lambda(&mut self, params: &[ValueId], r: &Region) -> Result<String, String> {
        let outer = std::mem::take(&mut self.body);
        let depth = self.depth;
        self.depth = 1;
        // A function value is not async: no command is awaited in it.
        let handler = std::mem::replace(&mut self.handler, false);
        for i in &r.instrs {
            if let Err(e) = self.instr(i) {
                self.handler = handler;
                return Err(e);
            }
        }
        self.handler = handler;
        self.line(&format!("return {};", val(r.value)));
        let inner = std::mem::replace(&mut self.body, outer);
        self.depth = depth;
        let pad = "  ".repeat(depth);
        let inner: String = inner.lines().map(|l| format!("{pad}{l}\n")).collect();
        let names: Vec<String> = params.iter().map(|p| val(*p)).collect();
        Ok(format!("({}) => {{\n{inner}{pad}}}", names.join(", ")))
    }

    fn fields(
        &self,
        def: crate::resolve::DefId,
        args: &[Type],
    ) -> Result<&[(String, Type)], String> {
        match self.shape(def, args) {
            Some(Shape::Record { fields }) => Ok(fields),
            Some(Shape::Alias(_)) => Ok(&[]),
            Some(Shape::Variant { .. }) => Err("a sum type, which has cases, not fields".into()),
            None => Err("a nominal type with no definition".into()),
        }
    }

    /// A sum type's cases, each with its payload's fields (ADR-0059).
    fn cases(
        &self,
        def: crate::resolve::DefId,
        args: &[Type],
    ) -> Result<&[(String, Vec<Type>)], String> {
        match self.shape(def, args) {
            Some(Shape::Variant { cases }) => Ok(cases),
            _ => Err("a case of a type that is not a sum type".into()),
        }
    }

    /// A case's name as a module value holds it, and its payload's fields'
    /// types, in the scrutinee's type.
    fn case_of(&self, scrutinee: &Type, case: VariantCase) -> Result<(String, Vec<Type>), String> {
        Ok(match (scrutinee, case) {
            (_, VariantCase::Builtin(c)) => {
                let payload = match (scrutinee, c) {
                    (Type::Option(t), BuiltinCase::Some) => vec![(**t).clone()],
                    (Type::Result(t, _), BuiltinCase::Ok) => vec![(**t).clone()],
                    (Type::Result(_, e), BuiltinCase::Err) => vec![(**e).clone()],
                    (Type::Option(_), BuiltinCase::None) => Vec::new(),
                    _ => return Err(format!("`{c:?}` is not a case of a {scrutinee:?}")),
                };
                (case_name(c).to_string(), payload)
            }
            (Type::Nominal(def, args), VariantCase::Declared(i)) => {
                let (name, fields) = self
                    .cases(*def, args)?
                    .get(i as usize)
                    .ok_or("a case past the type's cases")?;
                (crate::wit::ident(name), fields.clone())
            }
            _ => return Err(format!("a declared case of a {scrutinee:?}")),
        })
    }

    fn instr(&mut self, i: &Instr) -> Result<(), String> {
        let r = val(i.result());
        self.types.insert(i.result(), i.ty().clone());
        match i {
            Instr::Const { value, .. } => {
                let lit = match value {
                    Const::Int(n) => format!("{n}n"),
                    Const::Float(f) => number(*f),
                    Const::Bool(b) => b.to_string(),
                    Const::Str(s) => serde_json::to_string(s).map_err(|e| e.to_string())?,
                    Const::Unit => "undefined".into(),
                };
                self.line(&format!("const {r} = {lit};"));
            }
            // A function value (ADR-0052): a JavaScript function that calls
            // its code with what it captured, then what it is given.
            Instr::Closure {
                index, captures, ..
            } => {
                let Some(c) = self.closures.get(*index as usize) else {
                    return Err("a function value nothing compiled".into());
                };
                let arity = c.function.params.len() - c.captures;
                let params: Vec<String> = (0..arity).map(|k| format!("a{k}")).collect();
                let mut given: Vec<String> = captures.iter().map(|v| val(*v)).collect();
                given.extend(params.iter().cloned());
                self.line(&format!(
                    "const {r} = ({}) => {}({});",
                    params.join(", "),
                    closure_name(*index as usize),
                    given.join(", ")
                ));
            }
            // A command a handler calls (ADR-0058): awaited through its
            // context, each argument as JSON can carry it.
            Instr::Command { command, args, .. } => {
                if !self.handler {
                    return Err(format!(
                        "a call to `{command}`, which only a handler's own body makes"
                    ));
                }
                let mut sent = Vec::new();
                for a in args {
                    let t = self.type_of(*a)?.clone();
                    sent.push(self.wire(&val(*a), &t)?);
                }
                self.line(&format!(
                    "const {r} = await context.command({}, [{}]);",
                    json(command),
                    sent.join(", ")
                ));
            }
            // An opaque type is its representation (ADR-0054).
            Instr::Retype { value, .. } => {
                self.line(&format!("const {r} = {};", val(*value)));
            }
            Instr::Apply { function, args, .. } => {
                let args: Vec<String> = args.iter().map(|a| val(*a)).collect();
                self.line(&format!(
                    "const {r} = {}({});",
                    val(*function),
                    args.join(", ")
                ));
            }
            // `return e` and `?`'s failure (ADR-0051). The placeholder is
            // declared, never read.
            Instr::Return { value, .. } => {
                self.line(&format!("return {};", val(*value)));
                self.line(&format!("let {r};"));
            }
            // A mutable binding: a JavaScript variable (ADR-0051).
            Instr::Local { init, .. } => {
                self.line(&format!("let {r} = {};", val(*init)));
            }
            Instr::Set { local, value, .. } => {
                self.line(&format!("{} = {};", val(*local), val(*value)));
                self.line(&format!("const {r} = undefined;"));
            }
            Instr::Get { local, .. } => {
                self.line(&format!("const {r} = {};", val(*local)));
            }
            // A function compiled beside the query (ADR-0050).
            Instr::Call {
                callee,
                instance,
                args,
                ..
            } => {
                let Some(n) = self.callees.get(&(*callee, instance.clone())) else {
                    return Err(format!("a call to {callee:?}, which nothing compiled"));
                };
                let args: Vec<String> = args.iter().map(|a| val(*a)).collect();
                self.line(&format!(
                    "const {r} = {}({});",
                    callee_name(*n),
                    args.join(", ")
                ));
            }
            Instr::ImportCall { import, .. } => {
                return Err(format!("a host call, `{}`", import.qualified()));
            }
            Instr::Construct { ctor, args, ty, .. } => {
                let instance: &[Type] = match ty {
                    Type::Nominal(_, a) => a,
                    _ => &[],
                };
                let fields = self.fields(*ctor, instance)?;
                let expr = if fields.is_empty() {
                    // An opaque type is its representation.
                    match args.as_slice() {
                        [a] => val(*a),
                        _ => return Err("an opaque construction of several values".into()),
                    }
                } else {
                    let parts: Result<Vec<String>, String> = fields
                        .iter()
                        .zip(args)
                        .map(|((name, _), a)| {
                            Ok(format!(
                                "{}: {}",
                                serde_json::to_string(name).map_err(|e| e.to_string())?,
                                val(*a)
                            ))
                        })
                        .collect();
                    format!("{{ {} }}", parts?.join(", "))
                };
                self.line(&format!("const {r} = {expr};"));
            }
            Instr::Project { of, field, .. } => {
                let Type::Nominal(def, instance) = self.type_of(*of)?.clone() else {
                    return Err("a field read from a value that is not a record".into());
                };
                let name = self
                    .fields(def, &instance)?
                    .get(*field as usize)
                    .map(|(n, _)| n.clone())
                    .ok_or("a field index past the record's fields")?;
                let key = serde_json::to_string(&name).map_err(|e| e.to_string())?;
                self.line(&format!("const {r} = {}[{key}];", val(*of)));
            }
            Instr::Variant { case, payload, .. } => {
                let expr = match (case, payload) {
                    (BuiltinCase::None, _) => self.uses("none").to_string(),
                    (BuiltinCase::Some, Some(p)) => format!("{}({})", self.uses("some"), val(*p)),
                    (c, Some(p)) => {
                        format!("{{ $case: \"{}\", value: {} }}", case_name(*c), val(*p))
                    }
                    (c, None) => format!("{{ $case: \"{}\" }}", case_name(*c)),
                };
                self.line(&format!("const {r} = {expr};"));
            }
            // `Shape.Circle(r)` (ADR-0059): its WIT case's name, and its
            // fields, one as the value, several as an array of them.
            Instr::Case {
                case, fields, ty, ..
            } => {
                let (name, _) = self.case_of(ty, VariantCase::Declared(*case))?;
                let name = json(&name);
                let expr = match fields.as_slice() {
                    [] => format!("{{ $case: {name} }}"),
                    [one] => format!("{{ $case: {name}, value: {} }}", val(*one)),
                    many => format!(
                        "{{ $case: {name}, value: [{}] }}",
                        many.iter().map(|f| val(*f)).collect::<Vec<_>>().join(", ")
                    ),
                };
                self.line(&format!("const {r} = {expr};"));
            }
            Instr::Match {
                scrutinee, arms, ..
            } => {
                self.line(&format!("let {r};"));
                self.line(&format!("switch ({}.$case) {{", val(*scrutinee)));
                self.depth += 1;
                let st = self.type_of(*scrutinee)?.clone();
                for arm in arms {
                    let labels: Vec<String> = arm
                        .cases
                        .iter()
                        .map(|c| self.case_of(&st, *c).map(|(n, _)| n))
                        .collect::<Result<_, _>>()?;
                    let Some((last, rest)) = labels.split_last() else {
                        return Err("a match arm that takes no case".into());
                    };
                    for l in rest {
                        self.line(&format!("case {}:", json(l)));
                    }
                    self.line(&format!("case {}: {{", json(last)));
                    self.depth += 1;
                    if let [case] = arm.cases.as_slice() {
                        let (_, types) = self.case_of(&st, *case)?;
                        let many = types.len() > 1;
                        for (k, (b, t)) in arm.bindings.iter().zip(types).enumerate() {
                            let Some(b) = b else { continue };
                            self.types.insert(*b, t);
                            let read = match many {
                                true => format!("{}.value[{k}]", val(*scrutinee)),
                                false => format!("{}.value", val(*scrutinee)),
                            };
                            self.line(&format!("const {} = {read};", val(*b)));
                        }
                    }
                    self.region(&arm.body, i.result())?;
                    self.line("break;");
                    self.depth -= 1;
                    self.line("}");
                }
                let trap = self.uses("trap");
                self.line(&format!("default: {trap}(\"no arm takes this case\");"));
                self.depth -= 1;
                self.line("}");
            }
            Instr::Binary { op, lhs, rhs, .. } => {
                let (a, b) = (val(*lhs), val(*rhs));
                let expr = match (self.type_of(*lhs)?.clone(), op) {
                    (Type::Int, BinaryOp::Add | BinaryOp::Sub | BinaryOp::Mul) => {
                        format!("{}({a} {} {b})", self.uses("int"), symbol(*op))
                    }
                    (Type::Int, BinaryOp::Div) => format!("{}({a}, {b})", self.uses("div")),
                    (Type::Int, BinaryOp::Rem) => format!("{}({a}, {b})", self.uses("rem")),
                    (Type::Float, BinaryOp::Rem) => {
                        return Err("`%` on a Float, whose semantics are not decided".into());
                    }
                    (Type::Str, o) if o.compares() && !matches!(o, BinaryOp::Eq | BinaryOp::Ne) => {
                        format!("{}({a}, {b}) {} 0", self.uses("compare"), symbol(*op))
                    }
                    (Type::Int | Type::Float | Type::Bool | Type::Str, o) => {
                        format!("{a} {} {b}", symbol(*o))
                    }
                    (t, o) => return Err(format!("`{o:?}` on a {t:?}")),
                };
                self.line(&format!("const {r} = {expr};"));
            }
            Instr::Unary { op, operand, .. } => {
                let a = val(*operand);
                let expr = match (op, self.type_of(*operand)?) {
                    (UnaryOp::Neg, Type::Int) => format!("{}(-{a})", self.uses("int")),
                    (UnaryOp::Neg, _) => format!("-{a}"),
                    (UnaryOp::Not, _) => format!("!{a}"),
                };
                self.line(&format!("const {r} = {expr};"));
            }
            Instr::If {
                cond, then, els, ..
            } => {
                self.line(&format!("let {r};"));
                self.line(&format!("if ({}) {{", val(*cond)));
                self.depth += 1;
                self.region(then, i.result())?;
                self.depth -= 1;
                self.line("} else {");
                self.depth += 1;
                self.region(els, i.result())?;
                self.depth -= 1;
                self.line("}");
            }
            Instr::Concat { parts, .. } => {
                let ps: Vec<String> = parts.iter().map(|p| val(*p)).collect();
                self.line(&format!("const {r} = \"\" + {};", ps.join(" + ")));
            }
            Instr::Format { value, .. } => {
                let a = val(*value);
                let expr = match self.type_of(*value)? {
                    Type::Int => format!("{a}.toString()"),
                    Type::Bool => format!("String({a})"),
                    Type::Str => a,
                    t => return Err(format!("a {t:?} interpolated")),
                };
                self.line(&format!("const {r} = {expr};"));
            }
            Instr::Each {
                kind,
                list,
                seed,
                params,
                body,
                ..
            } => {
                let element = match self.type_of(*list)? {
                    Type::List(t) => (**t).clone(),
                    t => return Err(format!("a list operation over a {t:?}")),
                };
                for (k, p) in params.iter().enumerate() {
                    // A fold's first parameter is its accumulator; every other
                    // parameter is an element.
                    let t = match (kind, k, seed) {
                        (EachKind::Fold, 0, Some(s)) => self.type_of(*s)?.clone(),
                        _ => element.clone(),
                    };
                    self.types.insert(*p, t);
                }
                // `for x in xs` (ADR-0051): a loop in the function's own body,
                // so a `return` in it leaves the function.
                if *kind == EachKind::For {
                    let xs = val(*list);
                    self.line(&format!("for (const {} of {xs}) {{", val(params[0])));
                    self.depth += 1;
                    for i in &body.instrs {
                        self.instr(i)?;
                    }
                    self.depth -= 1;
                    self.line("}");
                    self.line(&format!("const {r} = undefined;"));
                    return Ok(());
                }
                let f = self.lambda(params, body)?;
                let xs = val(*list);
                let expr = match kind {
                    EachKind::Map => format!("{xs}.map({f})"),
                    EachKind::Filter => format!("{xs}.filter({f})"),
                    EachKind::Any => format!("{xs}.some({f})"),
                    EachKind::All => format!("{xs}.every({f})"),
                    EachKind::Fold => {
                        let s = seed.ok_or("a fold with no seed")?;
                        format!("{xs}.reduce({f}, {})", val(s))
                    }
                    EachKind::Find => {
                        let (some, none) = (self.uses("some"), self.uses("none"));
                        format!(
                            "((f) => {{ for (const x of {xs}) if (f(x)) return {some}(x); return {none}; }})({f})"
                        )
                    }
                    // Stable, as the component's merge sort is: `compare(a, b)
                    // > 0` puts `b` first, and a tie keeps the list's order.
                    EachKind::SortBy => {
                        let sign = self.uses("sign");
                        format!("((f) => [...{xs}].sort((a, b) => {sign}(f(a, b))))({f})")
                    }
                    EachKind::GroupBy => format!("{}({xs}, {f})", self.uses("group_by")),
                    EachKind::For => unreachable!("a loop is emitted above"),
                };
                self.line(&format!("const {r} = {expr};"));
            }
            Instr::Intrinsic { op, args, .. } => {
                let a: Vec<String> = args.iter().map(|v| val(*v)).collect();
                let arg = |k: usize| a.get(k).cloned().unwrap_or_default();
                let expr = match op {
                    Intrinsic::ListLength => format!("BigInt({}.length)", arg(0)),
                    Intrinsic::ListGet => {
                        let (some, none) = (self.uses("some"), self.uses("none"));
                        format!(
                            "{i} >= 0n && {i} < BigInt({xs}.length) ? {some}({xs}[Number({i})]) : {none}",
                            xs = arg(0),
                            i = arg(1)
                        )
                    }
                    Intrinsic::ListTake => format!(
                        "{xs}.slice(0, {n} < 0n ? 0 : {n} > BigInt({xs}.length) ? {xs}.length : Number({n}))",
                        xs = arg(0),
                        n = arg(1)
                    ),
                    Intrinsic::ListDrop => format!(
                        "{}({xs}, {n}, BigInt({xs}.length))",
                        self.uses("slice"),
                        xs = arg(0),
                        n = arg(1)
                    ),
                    Intrinsic::ListSlice => {
                        format!("{}({}, {}, {})", self.uses("slice"), arg(0), arg(1), arg(2))
                    }
                    Intrinsic::ListReverse => format!("[...{}].reverse()", arg(0)),
                    Intrinsic::StrSlice => format!(
                        "{}(Array.from({}), {}, {}).join(\"\")",
                        self.uses("slice"),
                        arg(0),
                        arg(1),
                        arg(2)
                    ),
                    Intrinsic::ListConcat => format!("[...{}, ...{}]", arg(0), arg(1)),
                    Intrinsic::StrLength => format!("BigInt(Array.from({}).length)", arg(0)),
                    Intrinsic::StrCodepoints => format!("{}({})", self.uses("codepoints"), arg(0)),
                    Intrinsic::StrFromCodepoints => {
                        format!("{}({})", self.uses("from_codepoints"), arg(0))
                    }
                    Intrinsic::StrStartsWith => format!("{}.startsWith({})", arg(0), arg(1)),
                    Intrinsic::StrEndsWith => format!("{}.endsWith({})", arg(0), arg(1)),
                    Intrinsic::StrContains => format!("{}.includes({})", arg(0), arg(1)),
                    Intrinsic::StrJoin => format!("{}.join({})", arg(0), arg(1)),
                    Intrinsic::StrTrim => format!("{}({})", self.uses("trim"), arg(0)),
                    Intrinsic::StrToLowerAscii => {
                        format!("{}({})", self.uses("lower_ascii"), arg(0))
                    }
                    Intrinsic::StrToLower => format!(
                        "{}({}, {})",
                        self.uses("case_map"),
                        arg(0),
                        self.uses("case_lower")
                    ),
                    Intrinsic::StrToUpper => format!(
                        "{}({}, {})",
                        self.uses("case_map"),
                        arg(0),
                        self.uses("case_upper")
                    ),
                    // `Number(bigint)` is the nearest value, ties to even, as
                    // `f64.convert_i64_s` is.
                    Intrinsic::FloatFromInt => format!("Number({})", arg(0)),
                    // Maps and sets (ADR-0057): arrays in ascending key
                    // order, a map's entries `[key, value]`.
                    Intrinsic::MapEmpty | Intrinsic::SetEmpty => "[]".to_string(),
                    Intrinsic::MapSize | Intrinsic::SetSize => format!("BigInt({}.length)", arg(0)),
                    Intrinsic::MapGet => {
                        let (some, none) = (self.uses("some"), self.uses("none"));
                        format!(
                            "((p) => p[1] ? {some}({m}[p[0]][1]) : {none})({}({m}, {k}, (e) => e[0]))",
                            self.uses("place"),
                            m = arg(0),
                            k = arg(1)
                        )
                    }
                    Intrinsic::MapContains => {
                        format!(
                            "{}({}, {}, (e) => e[0])[1]",
                            self.uses("place"),
                            arg(0),
                            arg(1)
                        )
                    }
                    Intrinsic::SetContains => {
                        format!(
                            "{}({}, {}, (e) => e)[1]",
                            self.uses("place"),
                            arg(0),
                            arg(1)
                        )
                    }
                    Intrinsic::MapInsert => format!(
                        "((p) => [...{m}.slice(0, p[0]), [{k}, {v}], ...{m}.slice(p[0] + (p[1] ? 1 : 0))])({}({m}, {k}, (e) => e[0]))",
                        self.uses("place"),
                        m = arg(0),
                        k = arg(1),
                        v = arg(2)
                    ),
                    Intrinsic::SetInsert => format!(
                        "((p) => [...{s}.slice(0, p[0]), {x}, ...{s}.slice(p[0] + (p[1] ? 1 : 0))])({}({s}, {x}, (e) => e))",
                        self.uses("place"),
                        s = arg(0),
                        x = arg(1)
                    ),
                    Intrinsic::MapRemove | Intrinsic::SetRemove => format!(
                        "((p) => p[1] ? [...{xs}.slice(0, p[0]), ...{xs}.slice(p[0] + 1)] : {xs})({}({xs}, {k}, {of}))",
                        self.uses("place"),
                        xs = arg(0),
                        k = arg(1),
                        of = if matches!(op, Intrinsic::MapRemove) {
                            "(e) => e[0]"
                        } else {
                            "(e) => e"
                        }
                    ),
                    Intrinsic::MapKeys => format!("{}.map((e) => e[0])", arg(0)),
                    Intrinsic::MapValues => format!("{}.map((e) => e[1])", arg(0)),
                    Intrinsic::MapFromLists => format!(
                        "((ks, vs) => {{ if (ks.length !== vs.length) {}(\"lists of two lengths\"); return {}(ks.map((k, i) => [k, vs[i]]), (e) => e[0]); }})({}, {})",
                        self.uses("trap"),
                        self.uses("by_key"),
                        arg(0),
                        arg(1)
                    ),
                    Intrinsic::SetFromList => {
                        format!("{}({}, (e) => e)", self.uses("by_key"), arg(0))
                    }
                    Intrinsic::SetToList => arg(0),
                    Intrinsic::SetUnion | Intrinsic::SetIntersection | Intrinsic::SetDifference => {
                        let keep = match op {
                            Intrinsic::SetUnion => "true, true, true",
                            Intrinsic::SetIntersection => "false, false, true",
                            _ => "true, false, false",
                        };
                        format!("{}({}, {}, {keep})", self.uses("merged"), arg(0), arg(1))
                    }
                    Intrinsic::MapCheck => {
                        format!("{}({}, (e) => e[0])", self.uses("checked"), arg(0))
                    }
                    Intrinsic::SetCheck => {
                        format!("{}({}, (e) => e)", self.uses("checked"), arg(0))
                    }
                };
                self.line(&format!("const {r} = {expr};"));
            }
            Instr::MakeList { items, .. } => {
                let xs: Vec<String> = items.iter().map(|v| val(*v)).collect();
                self.line(&format!("const {r} = [{}];", xs.join(", ")));
            }
        }
        Ok(())
    }
}

fn val(v: ValueId) -> String {
    format!("v{}", v.0)
}

/// A case table as a module constant: the numbers the component's data
/// segment holds, eight entries to a line.
fn case_table(name: &str, c: Case) -> String {
    let t = case::table(c);
    let lines = |words: Vec<[i64; 4]>| -> String {
        words
            .chunks(8)
            .map(|row| {
                let row: Vec<String> = row
                    .iter()
                    .map(|w| w.map(|x| x.to_string()).join(", "))
                    .collect();
                format!("    {},", row.join(", "))
            })
            .collect::<Vec<_>>()
            .join("\n")
    };
    let ranges = lines(
        t.ranges
            .iter()
            .map(|r| [r.lo as i64, r.hi as i64, r.delta as i64, r.stride as i64])
            .collect(),
    );
    let multi = lines(
        t.multi
            .iter()
            .map(|m| {
                [
                    m.from as i64,
                    m.to[0] as i64,
                    m.to[1] as i64,
                    m.to[2] as i64,
                ]
            })
            .collect(),
    );
    let (major, minor, update) = char::UNICODE_VERSION;
    format!(
        "// Unicode {major}.{minor}.{update}'s {} case, per code point (ADR-0056).\n\
         const {name} = {{\n  ranges: [\n{ranges}\n  ],\n  multi: [\n{multi}\n  ],\n}};",
        match c {
            Case::Lower => "lower",
            Case::Upper => "upper",
        }
    )
}

/// A float as JavaScript reads it back exactly.
fn number(f: f64) -> String {
    if f.is_nan() {
        "NaN".into()
    } else if f.is_infinite() {
        if f > 0.0 { "Infinity" } else { "-Infinity" }.into()
    } else {
        format!("{f:?}")
    }
}

fn symbol(op: BinaryOp) -> &'static str {
    match op {
        BinaryOp::Add => "+",
        BinaryOp::Sub => "-",
        BinaryOp::Mul => "*",
        BinaryOp::Div => "/",
        BinaryOp::Rem => "%",
        BinaryOp::Eq => "===",
        BinaryOp::Ne => "!==",
        BinaryOp::Lt => "<",
        BinaryOp::Le => "<=",
        BinaryOp::Gt => ">",
        BinaryOp::Ge => ">=",
    }
}

fn case_name(c: BuiltinCase) -> &'static str {
    match c {
        BuiltinCase::Some => "some",
        BuiltinCase::None => "none",
        BuiltinCase::Ok => "ok",
        BuiltinCase::Err => "err",
    }
}
