//! **E10 gate item 2: compiled semantics, against a reference.**
//!
//! Charter §14 M10 task 9: "Differentially compare server behavior with
//! generated Koka/Rust reference implementations."
//!
//! Every compiled server declaration (the store's five and the kiokun slice's
//! two) runs beside an independent Rust statement of what it means, against the
//! same generated data layer. The calls each makes, in order and with their
//! arguments, and the result must be identical, over 300 generated cases per
//! declaration.
//!
//! Inputs and the data layer's answers are generated from the COMPONENT's own
//! types, read from the artifact by Wasmtime, not from a description written
//! here that could drift from the world. Strings include the empty string,
//! CJK, kana, emoji and long runs; integers span `s64`. A pointer, a length, a
//! field offset or a discriminant that the encoder got wrong changes a call or
//! a result, or traps.

use std::collections::BTreeMap;
use std::sync::{Arc, Mutex};

use pw_conformance::{Runnable, compile, files, units};
use pw_host::engine::{HostFn, Val};
use wasmtime::component::types::{ComponentItem, Type};

const CASES: usize = 300;

/// xorshift64*: deterministic, so a failing case is reproducible from its seed.
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
}

fn string(rng: &mut Rng) -> String {
    const POOLS: [&str; 6] = [
        "abcdefghijklmnopqrstuvwxyz",
        "人市魚酒時夢空青上多面色聲樂錢園諺貪",
        "ひとみずかぜそらアイウエオ",
        "🀄🍣😀🇯🇵",
        " -_.:/?#&=%",
        "ÀéîõüÇñß",
    ];
    match rng.below(12) {
        0 => String::new(),
        1 => "語".repeat(1 + rng.below(3_000) as usize),
        _ => {
            let pool: Vec<char> = POOLS[rng.below(POOLS.len() as u64) as usize]
                .chars()
                .collect();
            (0..1 + rng.below(12))
                .map(|_| pool[rng.below(pool.len() as u64) as usize])
                .collect()
        }
    }
}

/// A random value of a component type.
fn val(rng: &mut Rng, ty: &Type, depth: u32) -> Val {
    let list_len = |rng: &mut Rng| if depth > 3 { 0 } else { rng.below(5) as usize };
    match ty {
        Type::Bool => Val::Bool(rng.below(2) == 1),
        Type::S8 => Val::S8(rng.next() as i8),
        Type::U8 => Val::U8(rng.next() as u8),
        Type::S16 => Val::S16(rng.next() as i16),
        Type::U16 => Val::U16(rng.next() as u16),
        Type::S32 => Val::S32(rng.next() as i32),
        Type::U32 => Val::U32(rng.next() as u32),
        Type::S64 => Val::S64(match rng.below(4) {
            0 => i64::MIN,
            1 => i64::MAX,
            _ => rng.next() as i64,
        }),
        Type::U64 => Val::U64(rng.next()),
        Type::Float64 => Val::Float64((rng.next() as i64 as f64) / 7.0),
        Type::Float32 => Val::Float32((rng.next() as i32 as f32) / 7.0),
        Type::Char => Val::Char(string(rng).chars().next().unwrap_or('x')),
        Type::String => Val::String(string(rng)),
        Type::List(l) => Val::List(
            (0..list_len(rng))
                .map(|_| val(rng, &l.ty(), depth + 1))
                .collect(),
        ),
        Type::Record(r) => Val::Record(
            r.fields()
                .map(|f| (f.name.to_string(), val(rng, &f.ty, depth + 1)))
                .collect(),
        ),
        Type::Tuple(t) => Val::Tuple(t.types().map(|t| val(rng, &t, depth + 1)).collect()),
        Type::Option(o) => {
            Val::Option((rng.below(3) != 0).then(|| Box::new(val(rng, &o.ty(), depth + 1))))
        }
        Type::Result(r) => {
            if rng.below(3) != 0 {
                Val::Result(Ok(r.ok().map(|t| Box::new(val(rng, &t, depth + 1)))))
            } else {
                Val::Result(Err(r.err().map(|t| Box::new(val(rng, &t, depth + 1)))))
            }
        }
        Type::Variant(v) => {
            let cases: Vec<_> = v.cases().collect();
            let c = &cases[rng.below(cases.len() as u64) as usize];
            Val::Variant(
                c.name.to_string(),
                c.ty.as_ref().map(|t| Box::new(val(rng, t, depth + 1))),
            )
        }
        Type::Enum(e) => {
            let names: Vec<_> = e.names().collect();
            Val::Enum(names[rng.below(names.len() as u64) as usize].to_string())
        }
        other => panic!("the oracle does not generate {other:?}"),
    }
}

/// The artifact's own types: each imported function's result, and the export's
/// parameters.
struct Types {
    results: BTreeMap<String, Type>,
    params: Vec<Type>,
}

fn types(bytes: &[u8], export: &[String; 2]) -> Types {
    let mut config = wasmtime::Config::new();
    config.wasm_component_model(true);
    let engine = wasmtime::Engine::new(&config).expect("engine");
    let component = wasmtime::component::Component::new(&engine, bytes).expect("component");
    let ty = component.component_type();
    let mut results = BTreeMap::new();
    for (name, item) in ty.imports(&engine) {
        let ComponentItem::ComponentInstance(i) = item.ty else {
            continue;
        };
        for (f, item) in i.exports(&engine) {
            if let ComponentItem::ComponentFunc(func) = item.ty
                && let Some(r) = func.results().next()
            {
                results.insert(format!("{name}#{f}"), r);
            }
        }
    }
    let mut params = Vec::new();
    for (name, item) in ty.exports(&engine) {
        let ComponentItem::ComponentInstance(i) = item.ty else {
            continue;
        };
        if name != export[0] {
            continue;
        }
        for (f, item) in i.exports(&engine) {
            if let ComponentItem::ComponentFunc(func) = item.ty
                && f == export[1]
            {
                params = func.params().map(|(_, t)| t).collect();
            }
        }
    }
    Types { results, params }
}

/// One call to a host operation: its name and arguments.
type Call = (String, Vec<Val>);

/// A generated data layer: it answers a call by its name and arguments.
type Answer = dyn Fn(&str, &[Val]) -> Val + Send + Sync;

/// What a declaration means, stated independently: given its arguments and a
/// host to call, its result.
type Reference = fn(&[Val], &mut dyn FnMut(&str, Vec<Val>) -> Val) -> Val;

fn field(v: &Val, name: &str) -> Val {
    let Val::Record(fields) = v else {
        panic!("not a record")
    };
    fields
        .iter()
        .find(|(k, _)| k == name)
        .expect(name)
        .1
        .clone()
}

/// **Run one declaration beside its reference, CASES times.**
///
/// `answer` is the data layer for one case: it answers a call by its name and
/// arguments, the same way for the component and for the reference.
fn differential(
    what: &str,
    runnable: &Runnable,
    reference: Reference,
    mut layer: impl FnMut(&mut Rng, &Types) -> Box<Answer>,
) {
    let contract = &runnable.compiled().contract;
    let located = contract.exports[0].component.clone().expect("located");
    let export = [located.interface, located.function];
    let t = types(&runnable.compiled().component.bytes, &export);
    let ops: Vec<String> = t.results.keys().cloned().collect();
    let mut rng = Rng(0x9E37_79B9_7F4A_7C15 ^ what.len() as u64);
    for case in 0..CASES {
        let seed = rng.0;
        let args: Vec<Val> = t.params.iter().map(|p| val(&mut rng, p, 0)).collect();
        let answer: Arc<Answer> = layer(&mut rng, &t).into();

        // The compiled component, through the host.
        let seen: Arc<Mutex<Vec<Call>>> = Arc::default();
        let host: BTreeMap<String, HostFn> = ops
            .iter()
            .map(|op| {
                let (seen, answer, name) = (seen.clone(), answer.clone(), op.clone());
                let f: HostFn = Arc::new(move |a: &[Val]| {
                    seen.lock().unwrap().push((name.clone(), a.to_vec()));
                    Ok(vec![answer(&name, a)])
                });
                (op.clone(), f)
            })
            .collect();
        let got = runnable
            .call(&host, &args)
            .unwrap_or_else(|e| panic!("{what} case {case} (seed {seed:#x}) failed: {e}"));

        // The reference, against the same answers.
        let mut expected_calls: Vec<Call> = Vec::new();
        let expected = reference(&args, &mut |name, a| {
            expected_calls.push((name.to_string(), a.clone()));
            answer(name, &a)
        });

        assert_eq!(
            *seen.lock().unwrap(),
            expected_calls,
            "{what} case {case} (seed {seed:#x}): the calls differ for {args:?}"
        );
        assert_eq!(
            got,
            vec![expected],
            "{what} case {case} (seed {seed:#x}): the results differ for {args:?}"
        );
    }
    println!("oracle: {what}: {CASES} generated cases, the component and the reference agree");
}

/// Answer every operation with one generated value of its result type.
fn fixed(rng: &mut Rng, t: &Types) -> Box<Answer> {
    let answers: BTreeMap<String, Val> = t
        .results
        .iter()
        .map(|(op, ty)| (op.clone(), val(rng, ty, 0)))
        .collect();
    Box::new(move |op, _| answers[op].clone())
}

fn store() -> Vec<pw_core::check::Unit> {
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    let mut paths = vec!["examples/domain.pw".to_string()];
    for dir in ["examples/lib", "examples/store"] {
        let mut ps: Vec<String> = std::fs::read_dir(root.join(dir))
            .unwrap()
            .map(|e| e.unwrap().path())
            .filter(|p| p.extension().is_some_and(|x| x == "pw"))
            .map(|p| p.strip_prefix(&root).unwrap().display().to_string())
            .collect();
        ps.sort();
        paths.extend(ps);
    }
    let refs: Vec<&str> = paths.iter().map(String::as_str).collect();
    let program = files(&refs);
    let borrowed: Vec<(&str, &str)> = program
        .iter()
        .map(|(p, s)| (p.as_str(), s.as_str()))
        .collect();
    units(&borrowed)
}

fn kiokun() -> Vec<pw_core::check::Unit> {
    let program = files(&[
        "examples/kiokun/dictionary.pw",
        "examples/kiokun/Entries.pw",
        "examples/kiokun/Index.pw",
        "examples/kiokun/app.pw",
    ]);
    let borrowed: Vec<(&str, &str)> = program
        .iter()
        .map(|(p, s)| (p.as_str(), s.as_str()))
        .collect();
    units(&borrowed)
}

#[test]
fn the_stores_commands_agree_with_their_reference() {
    let u = store();
    differential(
        "store.page.add_to_cart",
        &Runnable::new(compile(&u, "store.page.add_to_cart")),
        |args, host| {
            let session = host("pw:host/session#read", vec![]);
            host(
                "store:data/carts#add",
                vec![session, args[0].clone(), args[1].clone()],
            )
        },
        fixed,
    );
    differential(
        "store.page.clear_cart",
        &Runnable::new(compile(&u, "store.page.clear_cart")),
        |_, host| {
            let session = host("pw:host/session#read", vec![]);
            host("store:data/carts#clear", vec![session])
        },
        fixed,
    );
}

#[test]
fn the_stores_queries_agree_with_their_reference() {
    let u = store();
    for (id, op) in [
        ("store.page.Cart", "store:data/carts#current"),
        ("store.page.Menu", "store:data/menus#for-store"),
        ("store.page.Store", "store:data/stores#get"),
    ] {
        let op: &'static str = op;
        differential(
            id,
            &Runnable::new(compile(&u, id)),
            match op {
                "store:data/carts#current" => {
                    |args, host| host("store:data/carts#current", vec![args[0].clone()])
                }
                "store:data/menus#for-store" => {
                    |args, host| host("store:data/menus#for-store", vec![args[0].clone()])
                }
                _ => |args, host| host("store:data/stores#get", vec![args[0].clone()]),
            },
            fixed,
        );
    }
}

#[test]
fn the_kiokun_lookup_agrees_with_its_reference() {
    // The reference: one redirect, followed. Written from the declaration's
    // meaning, not from its lowering.
    fn lookup(args: &[Val], host: &mut dyn FnMut(&str, Vec<Val>) -> Val) -> Val {
        match host("kiokun:data/entries#get", vec![args[0].clone()]) {
            Val::Option(Some(entry)) => match field(&entry, "redirect") {
                Val::Option(Some(target)) => host("kiokun:data/entries#get", vec![*target]),
                _ => Val::Option(Some(entry)),
            },
            _ => Val::Option(None),
        }
    }
    differential(
        "kiokun.page.Lookup",
        &Runnable::new(compile(&kiokun(), "kiokun.page.Lookup")),
        lookup,
        |rng, t| {
            // A table of generated entries whose redirects name each other,
            // themselves, or nothing, so chains, stubs to stubs and stubs to
            // missing words all occur.
            let entry_ty = &t.results["kiokun:data/entries#get"];
            let words: Vec<String> = (0..1 + rng.below(5)).map(|_| string(rng)).collect();
            let mut table: BTreeMap<String, Val> = BTreeMap::new();
            for w in &words {
                let Val::Option(Some(e)) = val(rng, entry_ty, 0) else {
                    continue;
                };
                let Val::Record(mut fields) = *e else {
                    unreachable!()
                };
                let redirect = match rng.below(3) {
                    0 => Val::Option(None),
                    1 => Val::Option(Some(Box::new(Val::String(
                        words[rng.below(words.len() as u64) as usize].clone(),
                    )))),
                    _ => Val::Option(Some(Box::new(Val::String(string(rng))))),
                };
                for (k, v) in fields.iter_mut() {
                    if k == "redirect" {
                        *v = redirect.clone();
                    }
                }
                table.insert(w.clone(), Val::Record(fields));
            }
            Box::new(move |_, a| match a {
                [Val::String(w)] => Val::Option(table.get(w).cloned().map(Box::new)),
                _ => Val::Option(None),
            })
        },
    );
}

#[test]
fn the_kiokun_search_agrees_with_its_reference() {
    differential(
        "kiokun.page.Search",
        &Runnable::new(compile(&kiokun(), "kiokun.page.Search")),
        |args, host| {
            host(
                "kiokun:data/index#search",
                vec![args[0].clone(), Val::S64(20)],
            )
        },
        fixed,
    );
}

/// **The oracle can disagree.** Three references, each wrong in one detail a
/// miscompilation could share: the limit the query passes, whether the
/// redirect is followed, and whether the session is read first. Each must be
/// caught, or the agreement above measures nothing.
#[test]
fn the_oracle_notices_a_reference_that_is_wrong() {
    let quiet = std::panic::take_hook();
    std::panic::set_hook(Box::new(|_| {}));
    let caught = |what: &str, runnable: &Runnable, wrong: Reference| {
        std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            differential(what, runnable, wrong, fixed_or_table)
        }))
        .is_err()
    };
    let search = Runnable::new(compile(&kiokun(), "kiokun.page.Search"));
    let lookup = Runnable::new(compile(&kiokun(), "kiokun.page.Lookup"));
    let add = Runnable::new(compile(&store(), "store.page.add_to_cart"));
    let results = [
        caught("Search, limit 21", &search, |args, host| {
            host(
                "kiokun:data/index#search",
                vec![args[0].clone(), Val::S64(21)],
            )
        }),
        caught("Lookup, no redirect", &lookup, |args, host| {
            host("kiokun:data/entries#get", vec![args[0].clone()])
        }),
        caught("add_to_cart, no session", &add, |args, host| {
            host(
                "store:data/carts#add",
                vec![Val::String(String::new()), args[0].clone(), args[1].clone()],
            )
        }),
    ];
    std::panic::set_hook(quiet);
    println!("oracle: wrong references caught: {results:?}");
    assert_eq!(results, [true, true, true]);
}

/// The data layer the mutation controls run against: a table for `Lookup`,
/// a fixed answer for everything else.
fn fixed_or_table(rng: &mut Rng, t: &Types) -> Box<Answer> {
    if let Some(entry_ty) = t.results.get("kiokun:data/entries#get") {
        // One entry that redirects to another: the redirect-ignoring reference
        // then makes one call where the component makes two.
        let Val::Option(Some(e)) = std::iter::repeat_with(|| val(rng, entry_ty, 0))
            .find(|v| matches!(v, Val::Option(Some(_))))
            .expect("an entry")
        else {
            unreachable!()
        };
        let Val::Record(mut fields) = *e else {
            unreachable!()
        };
        let target = fields.clone();
        for (k, v) in fields.iter_mut() {
            if k == "redirect" {
                *v = Val::Option(Some(Box::new(Val::String("target".into()))));
            }
        }
        let stub = Val::Record(fields);
        return Box::new(move |_, a| match a {
            [Val::String(w)] if w == "target" => {
                Val::Option(Some(Box::new(Val::Record(target.clone()))))
            }
            [Val::String(_)] => Val::Option(Some(Box::new(stub.clone()))),
            _ => Val::Option(None),
        });
    }
    fixed(rng, t)
}
