//! **A type that contains itself, built, walked and crossing, run**
//! (ADR-0194).
//!
//! Each query runs through the E8 host beside a model written here in Rust.
//! A value of a type that contains itself crosses a component boundary as its
//! nodes, in level order (`wit.rs`): the host makes a nested argument its
//! nodes, the component checks and rebuilds them in place, and its result
//! leaves as nodes again, which the host makes nested. The model's own
//! encoding, written here, says what the nodes must be, and malformed nodes
//! must trap.

use std::collections::BTreeMap;

use pw_conformance::{Runnable, compile, operation, units};
use pw_host::engine::{HostFn, Val};

const CASES: usize = 120;

const PROGRAM: &str = r#"module thread

import List

type Comment = Comment { text: String, likes: Int, replies: List<Comment> }

type Json =
    | Null
    | Num(Float)
    | Text(String)
    | Array(List<Json>)
    | Keyed(String, List<Json>)

type Page = Page { title: String, thread: Comment }

type Missing =
    | NotFound

fn fetch(id: Int) -> Comment !{ database.read<Comment> }
    host "thread:data/comments#fetch"

fn weigh(c: Comment) -> Int !{ database.read<Comment> }
    host "thread:data/comments#weigh"

fn count(c: Comment) -> Int {
    List.fold(c.replies, 1, (n, r) => n + count(r))
}

fn liked(c: Comment) -> Int {
    List.fold(c.replies, c.likes, (n, r) => n + liked(r))
}

fn larger(a: Int, b: Int) -> Int {
    if a > b { a } else { b }
}

fn deepest(c: Comment) -> Int {
    1 + List.fold(c.replies, 0, (d, r) => larger(d, deepest(r)))
}

fn bump(c: Comment) -> Comment {
    Comment { text: "{c.text}+", likes: c.likes + 1, replies: List.map(c.replies, bump) }
}

fn total(j: Json) -> Float {
    match j {
        Null => 0.0,
        Num(x) => x,
        Text(_) => 1.0,
        Array(items) => List.fold(items, 0.0, (t, x) => t + total(x)),
        Keyed(_, items) => List.fold(items, 100.0, (t, x) => t + total(x)),
    }
}

public query Size(c: Comment) -> Int { count(c) }

public query Root(c: Comment) -> String { c.text }

public query Liked(c: Comment) -> Int { liked(c) }

public query Deepest(c: Comment) -> Int { deepest(c) }

public query Same(c: Comment) -> Comment { c }

public query Bumped(c: Comment) -> Comment { bump(c) }

public query Built() -> Int {
    count(Comment { text: "Root", likes: 0, replies: [Comment { text: "A reply", likes: 1, replies: [] }] })
}

public query Replies(c: Comment) -> List<Comment> { c.replies }

public query Sizes(cs: List<Comment>) -> List<Int> { List.map(cs, count) }

public query First(cs: List<Comment>) -> Option<Comment> { List.get(cs, 0) }

public query Found(c: Comment, text: String) -> Result<Comment, Missing> {
    match List.find(c.replies, (r) => r.text == text) {
        Some(r) => Ok(r),
        None => Err(Missing.NotFound),
    }
}

public query Paged(c: Comment) -> Page { Page { title: c.text, thread: c } }

public query Counted(p: Page) -> Int { count(p.thread) }

public query Chain(steps: List<Int>) -> Comment {
    let mut c = Comment { text: "leaf", likes: 0, replies: [] }
    for s in steps {
        c = Comment { text: "x", likes: s, replies: [c] }
    }
    c
}

public query Total(j: Json) -> Float { total(j) }

public query Echo(j: Json) -> Json { j }

public query Fetched(id: Int) -> Int { count(fetch(id)) }

public query Weighed(c: Comment) -> Int { weigh(bump(c)) }

type Folder = Folder { name: String, items: List<Item> }

type Item =
    | File(Int)
    | Sub(Folder)

fn folder(depth: Int) -> Folder {
    if depth <= 0 {
        Folder { name: "leaf", items: [Item.File(1)] }
    } else {
        Folder { name: "dir", items: [Item.File(depth), Item.Sub(folder(depth - 1)), Item.Sub(folder(depth - 1))] }
    }
}

fn size(f: Folder) -> Int {
    List.fold(f.items, 0, (t, i) => t + item_size(i))
}

fn item_size(i: Item) -> Int {
    match i {
        File(n) => n,
        Sub(f) => size(f),
    }
}

public query Tree(depth: Int) -> Int { size(folder(depth)) }
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
    fn text(&mut self) -> String {
        const POOL: [&str; 6] = ["", "a", "Root", "人", "\u{1F600}", "tab\there"];
        POOL[self.below(POOL.len() as u64) as usize].to_string()
    }
}

/// The model's comment.
#[derive(Debug, Clone, PartialEq)]
struct Comment {
    text: String,
    likes: i64,
    replies: Vec<Comment>,
}

fn comment(rng: &mut Rng, depth: u32) -> Comment {
    let replies = match depth {
        0 => 0,
        _ => rng.below(4),
    };
    Comment {
        text: rng.text(),
        likes: rng.below(1000) as i64 - 500,
        replies: (0..replies).map(|_| comment(rng, depth - 1)).collect(),
    }
}

fn count(c: &Comment) -> i64 {
    1 + c.replies.iter().map(count).sum::<i64>()
}

fn liked(c: &Comment) -> i64 {
    c.likes + c.replies.iter().map(liked).sum::<i64>()
}

fn deepest(c: &Comment) -> i64 {
    1 + c.replies.iter().map(deepest).max().unwrap_or(0)
}

fn bump(c: &Comment) -> Comment {
    Comment {
        text: format!("{}+", c.text),
        likes: c.likes + 1,
        replies: c.replies.iter().map(bump).collect(),
    }
}

/// A comment as the program holds it: nested.
fn nested(c: &Comment) -> Val {
    Val::Record(vec![
        ("text".into(), Val::String(c.text.clone())),
        ("likes".into(), Val::S64(c.likes)),
        (
            "replies".into(),
            Val::List(c.replies.iter().map(nested).collect()),
        ),
    ])
}

/// A comment as a component passes it: its nodes in level order, written
/// here independently of the host's and the component's encoders.
fn nodes(c: &Comment) -> Val {
    let mut queue = std::collections::VecDeque::from([c]);
    let mut out = Vec::new();
    let mut next = 1u32;
    while let Some(c) = queue.pop_front() {
        let k = c.replies.len() as u32;
        out.push(Val::Record(vec![
            ("text".into(), Val::String(c.text.clone())),
            ("likes".into(), Val::S64(c.likes)),
            (
                "replies".into(),
                Val::List((next..next + k).map(Val::U32).collect()),
            ),
        ]));
        next += k;
        queue.extend(c.replies.iter());
    }
    Val::List(out)
}

/// One node, written out: a text, its likes, its children's indices.
fn node(text: &str, likes: i64, replies: &[u32]) -> Val {
    Val::Record(vec![
        ("text".into(), Val::String(text.into())),
        ("likes".into(), Val::S64(likes)),
        (
            "replies".into(),
            Val::List(replies.iter().map(|i| Val::U32(*i)).collect()),
        ),
    ])
}

/// The model's JSON-like value.
#[derive(Debug, Clone, PartialEq)]
enum Json {
    Null,
    Num(f64),
    Text(String),
    Array(Vec<Json>),
    Keyed(String, Vec<Json>),
}

fn json(rng: &mut Rng, depth: u32) -> Json {
    let pick = match depth {
        0 => rng.below(3),
        _ => rng.below(5),
    };
    let items = |rng: &mut Rng| (0..rng.below(4)).map(|_| json(rng, depth - 1)).collect();
    match pick {
        0 => Json::Null,
        1 => Json::Num((rng.below(2000) as f64 - 1000.0) / 4.0),
        2 => Json::Text(rng.text()),
        3 => Json::Array(items(rng)),
        _ => Json::Keyed(rng.text(), items(rng)),
    }
}

fn json_val(j: &Json) -> Val {
    let case = |name: &str, p: Option<Val>| Val::Variant(name.into(), p.map(Box::new));
    match j {
        Json::Null => case("null", None),
        Json::Num(x) => case("num", Some(Val::Float64(*x))),
        Json::Text(t) => case("text", Some(Val::String(t.clone()))),
        Json::Array(items) => case(
            "array",
            Some(Val::List(items.iter().map(json_val).collect())),
        ),
        Json::Keyed(k, items) => case(
            "keyed",
            Some(Val::Tuple(vec![
                Val::String(k.clone()),
                Val::List(items.iter().map(json_val).collect()),
            ])),
        ),
    }
}

fn total(j: &Json) -> f64 {
    match j {
        Json::Null => 0.0,
        Json::Num(x) => *x,
        Json::Text(_) => 1.0,
        Json::Array(items) => items.iter().fold(0.0, |t, x| t + total(x)),
        Json::Keyed(_, items) => items.iter().fold(100.0, |t, x| t + total(x)),
    }
}

fn compiled(id: &str) -> Runnable {
    Runnable::new(compile(&units(&[("thread.pw", PROGRAM)]), id))
}

fn host() -> BTreeMap<String, HostFn> {
    BTreeMap::new()
}

/// The call's result, as the component passed it.
fn raw(r: &Runnable, args: &[Val]) -> Result<Val, String> {
    r.call(&host(), args).map(|mut v| v.remove(0))
}

/// The call's result, as the program holds it.
fn call(r: &Runnable, args: &[Val]) -> Val {
    r.call_untangled(&host(), args).expect("runs").remove(0)
}

#[test]
fn a_tree_is_built_and_walked_in_the_component() {
    assert_eq!(call(&compiled("thread.Built"), &[]), Val::S64(2));
    let (size, likes, deep) = (
        compiled("thread.Size"),
        compiled("thread.Liked"),
        compiled("thread.Deepest"),
    );
    let mut rng = Rng(0x5eed_0194);
    for _ in 0..CASES {
        let c = comment(&mut rng, 5);
        let arg = [nested(&c)];
        assert_eq!(call(&size, &arg), Val::S64(count(&c)), "{c:?}");
        assert_eq!(call(&likes, &arg), Val::S64(liked(&c)), "{c:?}");
        assert_eq!(call(&deep, &arg), Val::S64(deepest(&c)), "{c:?}");
    }
}

#[test]
fn two_types_that_hold_each_other_through_a_list_compile_inside_a_component() {
    // A folder's items, an item a file or a folder: the cycle runs through
    // a list and a case, and neither type crosses the boundary.
    fn size(depth: i64) -> i64 {
        match depth {
            0 => 1,
            d => d + 2 * size(d - 1),
        }
    }
    let tree = compiled("thread.Tree");
    for depth in 0..8 {
        assert_eq!(
            call(&tree, &[Val::S64(depth)]),
            Val::S64(size(depth)),
            "{depth}"
        );
    }
}

#[test]
fn a_tree_crosses_as_its_nodes_in_level_order_and_back_unchanged() {
    let same = compiled("thread.Same");
    // Written out once, so the encoding the model checks is the one the WIT
    // describes, and not merely one the model and the host share.
    let small = Comment {
        text: "a".into(),
        likes: 0,
        replies: vec![
            Comment {
                text: "b".into(),
                likes: 1,
                replies: vec![Comment {
                    text: "d".into(),
                    likes: 3,
                    replies: vec![],
                }],
            },
            Comment {
                text: "c".into(),
                likes: 2,
                replies: vec![],
            },
        ],
    };
    assert_eq!(
        raw(&same, &[nested(&small)]),
        Ok(Val::List(vec![
            node("a", 0, &[1, 2]),
            node("b", 1, &[3]),
            node("c", 2, &[]),
            node("d", 3, &[]),
        ]))
    );
    let mut rng = Rng(0x0194_0001);
    for _ in 0..CASES {
        let c = comment(&mut rng, 5);
        // Nodes in, nodes out: the component's decoder and encoder.
        assert_eq!(raw(&same, &[nodes(&c)]), Ok(nodes(&c)), "{c:?}");
        // Nested in and out: the host's too.
        assert_eq!(call(&same, &[nested(&c)]), nested(&c), "{c:?}");
    }
}

#[test]
fn a_tree_is_rebuilt_in_the_component() {
    let bumped = compiled("thread.Bumped");
    let mut rng = Rng(0x0194_0002);
    for _ in 0..CASES {
        let c = comment(&mut rng, 5);
        assert_eq!(call(&bumped, &[nested(&c)]), nested(&bump(&c)), "{c:?}");
    }
}

#[test]
fn trees_held_by_other_types_cross_both_ways() {
    let (replies, sizes, first, found, paged, counted) = (
        compiled("thread.Replies"),
        compiled("thread.Sizes"),
        compiled("thread.First"),
        compiled("thread.Found"),
        compiled("thread.Paged"),
        compiled("thread.Counted"),
    );
    let mut rng = Rng(0x0194_0003);
    for _ in 0..CASES / 2 {
        let c = comment(&mut rng, 4);
        let cs: Vec<Comment> = (0..rng.below(4)).map(|_| comment(&mut rng, 3)).collect();
        // A list of trees, out and in.
        assert_eq!(
            call(&replies, &[nested(&c)]),
            Val::List(c.replies.iter().map(nested).collect())
        );
        let list = Val::List(cs.iter().map(nested).collect());
        assert_eq!(
            call(&sizes, std::slice::from_ref(&list)),
            Val::List(cs.iter().map(|c| Val::S64(count(c))).collect())
        );
        // An option of one.
        assert_eq!(
            call(&first, &[list]),
            Val::Option(cs.first().map(|c| Box::new(nested(c))))
        );
        // A result of one, either way.
        let wanted = match rng.below(2) {
            0 => c
                .replies
                .first()
                .map(|r| r.text.clone())
                .unwrap_or_default(),
            _ => "nobody".to_string(),
        };
        let expected = match c.replies.iter().find(|r| r.text == wanted) {
            Some(r) => Val::Result(Ok(Some(Box::new(nested(r))))),
            None => Val::Result(Err(Some(Box::new(Val::Variant("not-found".into(), None))))),
        };
        assert_eq!(call(&found, &[nested(&c), Val::String(wanted)]), expected);
        // A record holding one, out and in.
        let page = Val::Record(vec![
            ("title".into(), Val::String(c.text.clone())),
            ("thread".into(), nested(&c)),
        ]);
        assert_eq!(call(&paged, &[nested(&c)]), page);
        assert_eq!(call(&counted, &[page]), Val::S64(count(&c)));
    }
}

#[test]
fn a_sum_type_that_contains_itself_crosses_both_ways() {
    let (sum, echo) = (compiled("thread.Total"), compiled("thread.Echo"));
    let mut rng = Rng(0x0194_0004);
    for _ in 0..CASES {
        let j = json(&mut rng, 4);
        assert_eq!(
            call(&sum, &[json_val(&j)]),
            Val::Float64(total(&j)),
            "{j:?}"
        );
        assert_eq!(call(&echo, &[json_val(&j)]), json_val(&j), "{j:?}");
    }
}

#[test]
fn a_host_answers_with_a_tree_and_is_given_one() {
    let (fetched, weighed) = (compiled("thread.Fetched"), compiled("thread.Weighed"));
    let mut rng = Rng(0x0194_0005);
    for _ in 0..CASES / 4 {
        let c = comment(&mut rng, 4);
        // The data layer answers nested; the host passes it as nodes.
        let answer = nested(&c);
        let calls = pw_conformance::Calls::default();
        let ops: BTreeMap<String, HostFn> =
            [operation(&calls, "thread:data/comments#fetch", move |_| {
                answer.clone()
            })]
            .into_iter()
            .collect();
        assert_eq!(
            fetched.call_untangled(&ops, &[Val::S64(7)]),
            Ok(vec![Val::S64(count(&c))])
        );
        // The component passes nodes; the data layer is given them nested.
        let calls = pw_conformance::Calls::default();
        let ops: BTreeMap<String, HostFn> =
            [operation(&calls, "thread:data/comments#weigh", |args| {
                Val::S64(args.len() as i64)
            })]
            .into_iter()
            .collect();
        assert_eq!(
            weighed.call_untangled(&ops, &[nested(&c)]),
            Ok(vec![Val::S64(1)])
        );
        let given = calls.lock().expect("calls")[0].1.clone();
        assert_eq!(given, vec![nested(&bump(&c))]);
    }
}

#[test]
fn a_chain_deeper_than_any_stack_crosses_both_ways() {
    // Built by a loop, so nothing recursed to build it; encoded, decoded and
    // encoded again by code that does not recurse.
    const DEPTH: usize = 50_000;
    let steps = Val::List((0..DEPTH as i64).map(Val::S64).collect());
    let chain = raw(&compiled("thread.Chain"), &[steps]).expect("a deep chain encodes");
    let Val::List(ns) = &chain else {
        panic!("not nodes: {chain:?}");
    };
    assert_eq!(ns.len(), DEPTH + 1);
    assert_eq!(ns[0], node("x", DEPTH as i64 - 1, &[1]));
    assert_eq!(ns[DEPTH], node("leaf", 0, &[]));
    let same = compiled("thread.Same");
    assert_eq!(raw(&same, std::slice::from_ref(&chain)), Ok(chain.clone()));
    // Nested, it is deeper than a host nests.
    let refused = same.call_untangled(&host(), &[chain]);
    assert!(
        refused.as_ref().is_err_and(|e| e.contains(&format!(
            "deeper than {}",
            pw_host::engine::graph::NESTED_DEPTH
        ))),
        "{refused:?}"
    );
}

#[test]
fn nodes_that_are_not_one_tree_in_level_order_trap() {
    // `Root` reads the first node alone, so nodes it accepted would answer:
    // only the check refuses them.
    let (size, root) = (compiled("thread.Size"), compiled("thread.Root"));
    let cases: Vec<(&str, Vec<Val>)> = vec![
        ("no node at all", vec![]),
        ("a node holding itself", vec![node("a", 0, &[0])]),
        (
            "a node holding itself, its index continuing the run",
            vec![node("a", 0, &[]), node("b", 0, &[1])],
        ),
        (
            "a node holding one before it",
            vec![node("a", 0, &[1]), node("b", 0, &[0])],
        ),
        ("an index past the last node", vec![node("a", 0, &[1])]),
        (
            "a node no list holds",
            vec![node("a", 0, &[1]), node("b", 0, &[]), node("c", 0, &[])],
        ),
        (
            "a node two lists hold",
            vec![node("a", 0, &[1, 1]), node("b", 0, &[])],
        ),
        (
            "children out of level order",
            vec![node("a", 0, &[2, 1]), node("b", 0, &[]), node("c", 0, &[])],
        ),
        (
            "a run that skips an index",
            vec![node("a", 0, &[2]), node("b", 0, &[]), node("c", 0, &[])],
        ),
        (
            "a later node's children before an earlier one's",
            vec![
                node("a", 0, &[1, 2]),
                node("b", 0, &[4]),
                node("c", 0, &[3]),
                node("d", 0, &[]),
                node("e", 0, &[]),
            ],
        ),
    ];
    // Control: a well-formed tree is read.
    assert_eq!(
        raw(
            &root,
            &[Val::List(vec![node("a", 0, &[1]), node("b", 0, &[])])]
        ),
        Ok(Val::String("a".into()))
    );
    for (what, ns) in cases {
        let result = raw(&root, &[Val::List(ns.clone())]);
        assert!(result.is_err(), "{what}: {result:?}");
        let result = raw(&size, &[Val::List(ns.clone())]);
        assert!(result.is_err(), "{what}: {result:?}");
        // The host refuses the same nodes from a component, by name.
        let untangled = pw_host::engine::graph::untangle(Val::List(ns), &size_param_type(&size));
        assert!(untangled.is_err(), "{what}: the host took {untangled:?}");
    }
}

/// `Size`'s parameter type, as the component declares it.
fn size_param_type(r: &Runnable) -> wasmtime::component::types::Type {
    let engine = wasmtime::Engine::default();
    let component =
        wasmtime::component::Component::new(&engine, &r.compiled().component.bytes).expect("loads");
    let ty = component.component_type();
    for (_, item) in ty.exports(&engine) {
        if let wasmtime::component::types::ComponentItem::ComponentInstance(i) = item.ty {
            for (_, f) in i.exports(&engine) {
                if let wasmtime::component::types::ComponentItem::ComponentFunc(f) = f.ty {
                    return f.params().next().expect("a parameter").1;
                }
            }
        }
    }
    panic!("no function")
}

#[test]
fn a_value_as_deep_as_a_host_nests_stays_within_half_a_thread_stack() {
    // The bound is `NESTED_DEPTH`: a nested value is cloned, compared,
    // printed and dropped by recursion, and each must fit in half the 2 MiB a
    // Rust thread starts with, so a host holding one has room to spare.
    use pw_host::engine::graph::{NESTED_DEPTH, untangle};
    let ty = size_param_type(&compiled("thread.Size"));
    let depth = NESTED_DEPTH as u32;
    let chain: Vec<Val> = (0..depth)
        .map(|i| match i + 1 == depth {
            true => node("leaf", 0, &[]),
            false => node("x", i as i64, &[i + 1]),
        })
        .collect();
    let worker = std::thread::Builder::new()
        .stack_size(1024 * 1024)
        .spawn(move || {
            let nested = untangle(Val::List(chain), &ty).expect("within the bound");
            let copied = nested.clone();
            assert!(copied == nested);
            let printed = format!("{nested:?}");
            assert!(printed.len() > NESTED_DEPTH);
            drop(copied);
            drop(nested);
        })
        .expect("a thread");
    worker.join().expect("no overflow");
}
