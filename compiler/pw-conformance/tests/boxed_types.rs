//! **A type that holds itself in place, built, walked and crossing, run**
//! (ADR-0202).
//!
//! `next: Option<Node>` and `Add(Expr, Expr)` hold a value of their own type
//! with no list between, which no layout by type holds. The encoder boxes each
//! value of such a type: it is the address of its cell. Each query runs
//! through the E8 host beside a model written here in Rust.

use std::collections::BTreeMap;

use pw_conformance::{Runnable, compile, operation, units};
use pw_host::engine::{HostFn, Val};

const CASES: i64 = 40;

const PROGRAM: &str = r#"module chain

import List

type Node = Node { value: Int, next: Option<Node> }

type Expr =
    | Num(Int)
    | Neg(Expr)
    | Add(Expr, Expr)
    | Mul(Expr, Expr)

fn build(n: Int) -> Option<Node> {
    if n <= 0 { None } else { Some(Node { value: n, next: build(n - 1) }) }
}

fn total(l: Option<Node>) -> Int {
    match l {
        None => 0,
        Some(node) => node.value + total(node.next),
    }
}

fn eval(e: Expr) -> Int {
    match e {
        Num(n) => n,
        Neg(x) => 0 - eval(x),
        Add(a, b) => eval(a) + eval(b),
        Mul(a, b) => eval(a) * eval(b),
    }
}

fn tower(n: Int) -> Expr {
    if n <= 0 { Num(1) } else { Add(Num(n), Mul(tower(n - 1), Neg(Num(-2)))) }
}

public query Sum(n: Int) -> Int { total(build(n)) }

public query Eval(n: Int) -> Int { eval(tower(n)) }

public query Chain(n: Int) -> Option<Node> { build(n) }

public query Total(l: Option<Node>) -> Int { total(l) }

public query Tower(n: Int) -> Expr { tower(n) }

public query Value(e: Expr) -> Int { eval(e) }

public query Echo(e: Expr) -> Expr { e }

public query Long(steps: List<Int>) -> Node {
    let mut c = Node { value: 0, next: None }
    for s in steps {
        c = Node { value: s, next: Some(c) }
    }
    c
}

public query Same(n: Node) -> Node { n }

public query Head(n: Node) -> Int { n.value }

public query Length(n: Node) -> Int { 1 + total_length(n.next) }

fn total_length(l: Option<Node>) -> Int {
    match l {
        None => 0,
        Some(node) => 1 + total_length(node.next),
    }
}

public query Root(e: Expr) -> Int {
    match e {
        Num(n) => n,
        _ => -1,
    }
}

type Even = Even { n: Int, next: Option<Odd> }

type Odd = Odd { n: Int, next: Even }

fn evens(k: Int) -> Even {
    if k <= 0 { Even { n: 0, next: None } } else { Even { n: k, next: Some(Odd { n: k, next: evens(k - 1) }) } }
}

fn count_even(e: Even) -> Int {
    match e.next {
        None => e.n,
        Some(o) => e.n + o.n + count_even(o.next),
    }
}

public query Mutual(k: Int) -> Int { count_even(evens(k)) }

type Tree<T> =
    | Leaf(T)
    | Branch(Tree<T>, Tree<T>)

fn leaves(t: Tree<Int>) -> Int {
    match t {
        Leaf(x) => x,
        Branch(a, b) => leaves(a) + leaves(b),
    }
}

fn grow(n: Int) -> Tree<Int> {
    if n <= 0 { Leaf(1) } else { Branch(grow(n - 1), Leaf(n)) }
}

public query Grown(n: Int) -> Int { leaves(grow(n)) }

type Rose = Rose { label: Int, kids: List<Rose>, best: Option<Rose> }

fn labels(r: Rose) -> Int {
    let below = List.fold(r.kids, r.label, (t, k) => t + labels(k))
    match r.best {
        None => below,
        Some(b) => below + labels(b),
    }
}

fn rose(n: Int) -> Rose {
    if n <= 0 {
        Rose { label: 1, kids: [], best: None }
    } else {
        Rose { label: n, kids: [rose(n - 1), rose(n - 2)], best: Some(rose(n - 1)) }
    }
}

public query Roses(n: Int) -> Rose { rose(n) }

public query Labels(r: Rose) -> Int { labels(r) }

fn fetch(id: Int) -> Option<Node> !{ database.read<Node> }
    host "chain:data/nodes#fetch"

fn weigh(n: Option<Node>) -> Int !{ database.read<Node> }
    host "chain:data/nodes#weigh"

public query Fetched(id: Int) -> Int { total(fetch(id)) }

public query Weighed(n: Int) -> Int { weigh(build(n)) }
"#;

fn compiled(id: &str) -> Runnable {
    Runnable::new(compile(&units(&[("chain.pw", PROGRAM)]), id))
}

fn call(r: &Runnable, args: &[Val]) -> Val {
    r.call_untangled(&BTreeMap::new(), args)
        .expect("runs")
        .remove(0)
}

/// `Option<Node>`, nested, holding `values` in order.
fn chain(values: &[i64]) -> Val {
    values.iter().rev().fold(Val::Option(None), |next, v| {
        Val::Option(Some(Box::new(Val::Record(vec![
            ("value".into(), Val::S64(*v)),
            ("next".into(), next),
        ]))))
    })
}

/// An `Expr`, in Rust.
#[derive(Debug, Clone)]
enum Expr {
    Num(i64),
    Neg(Box<Expr>),
    Add(Box<Expr>, Box<Expr>),
    Mul(Box<Expr>, Box<Expr>),
}

impl Expr {
    fn eval(&self) -> Option<i64> {
        Some(match self {
            Expr::Num(n) => *n,
            Expr::Neg(x) => 0i64.checked_sub(x.eval()?)?,
            Expr::Add(a, b) => a.eval()?.checked_add(b.eval()?)?,
            Expr::Mul(a, b) => a.eval()?.checked_mul(b.eval()?)?,
        })
    }

    fn val(&self) -> Val {
        let case = |name: &str, v: Val| Val::Variant(name.into(), Some(Box::new(v)));
        match self {
            Expr::Num(n) => case("num", Val::S64(*n)),
            Expr::Neg(x) => case("neg", x.val()),
            Expr::Add(a, b) => case("add", Val::Tuple(vec![a.val(), b.val()])),
            Expr::Mul(a, b) => case("mul", Val::Tuple(vec![a.val(), b.val()])),
        }
    }
}

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
    fn expr(&mut self, depth: u32) -> Expr {
        match if depth == 0 { 0 } else { self.below(4) } {
            0 => Expr::Num(self.below(21) as i64 - 10),
            1 => Expr::Neg(Box::new(self.expr(depth - 1))),
            2 => Expr::Add(
                Box::new(self.expr(depth - 1)),
                Box::new(self.expr(depth - 1)),
            ),
            _ => Expr::Mul(
                Box::new(self.expr(depth - 1)),
                Box::new(self.expr(depth - 1)),
            ),
        }
    }
}

#[test]
fn a_value_held_in_place_is_built_and_walked_in_the_component() {
    let (sum, eval) = (compiled("chain.Sum"), compiled("chain.Eval"));
    let tower = |n: i64| (0..=n).fold(0, |t, k| if k == 0 { 1 } else { k + 2 * t });
    for n in 0..CASES {
        assert_eq!(
            call(&sum, &[Val::S64(n)]),
            Val::S64(n * (n + 1) / 2),
            "Sum({n})"
        );
        assert_eq!(call(&eval, &[Val::S64(n)]), Val::S64(tower(n)), "Eval({n})");
    }
}

#[test]
fn a_value_held_in_place_crosses_as_its_nodes() {
    // Out: a chain the component builds, nested on the host.
    let made = compiled("chain.Chain");
    for n in 0..CASES {
        let values: Vec<i64> = (1..=n).rev().collect();
        assert_eq!(call(&made, &[Val::S64(n)]), chain(&values), "Chain({n})");
    }
    // In: a chain the host gives, walked.
    let total = compiled("chain.Total");
    let mut rng = Rng(0x0202);
    for _ in 0..CASES {
        let values: Vec<i64> = (0..rng.below(30)).map(|_| rng.below(1000) as i64).collect();
        let want: i64 = values.iter().sum();
        assert_eq!(call(&total, &[chain(&values)]), Val::S64(want));
    }
    // A tree, both ways: built, evaluated, and given back unchanged.
    let tower = compiled("chain.Tower");
    let value = compiled("chain.Value");
    let echo = compiled("chain.Echo");
    let model = |n: i64| {
        (1..=n).fold(Expr::Num(1), |t, k| {
            Expr::Add(
                Box::new(Expr::Num(k)),
                Box::new(Expr::Mul(
                    Box::new(t),
                    Box::new(Expr::Neg(Box::new(Expr::Num(-2)))),
                )),
            )
        })
    };
    for n in 0..12 {
        assert_eq!(call(&tower, &[Val::S64(n)]), model(n).val(), "Tower({n})");
    }
    for _ in 0..CASES {
        let e = rng.expr(6);
        assert_eq!(call(&echo, &[e.val()]), e.val(), "{e:?}");
        if let Some(want) = e.eval() {
            assert_eq!(call(&value, &[e.val()]), Val::S64(want), "{e:?}");
        }
    }
}

/// A `Node`'s node: its value, and its next's index.
fn node(value: i64, next: Option<u32>) -> Val {
    Val::Record(vec![
        ("value".into(), Val::S64(value)),
        (
            "next".into(),
            Val::Option(next.map(|i| Box::new(Val::U32(i)))),
        ),
    ])
}

/// The call's result, as the component passed it.
fn raw(r: &Runnable, args: &[Val]) -> Result<Val, String> {
    r.call(&BTreeMap::new(), args).map(|mut v| v.remove(0))
}

#[test]
fn a_chain_deeper_than_any_stack_crosses_both_ways() {
    // Built by a loop, so nothing recursed to build it; encoded, decoded and
    // encoded again by code that does not recurse.
    const DEPTH: usize = 50_000;
    let steps = Val::List((1..=DEPTH as i64).map(Val::S64).collect());
    let chain = raw(&compiled("chain.Long"), &[steps]).expect("a deep chain encodes");
    let Val::List(ns) = &chain else {
        panic!("not nodes: {chain:?}");
    };
    assert_eq!(ns.len(), DEPTH + 1);
    assert_eq!(ns[0], node(DEPTH as i64, Some(1)));
    assert_eq!(ns[DEPTH], node(0, None));
    let same = compiled("chain.Same");
    assert_eq!(raw(&same, std::slice::from_ref(&chain)), Ok(chain.clone()));
    // Nested, it is deeper than a host nests.
    let refused = same.call_untangled(&BTreeMap::new(), &[chain]);
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
    // `Head` reads the first node alone, so nodes it accepted would answer:
    // only the check refuses them.
    let (head, length) = (compiled("chain.Head"), compiled("chain.Length"));
    let cases: Vec<(&str, Vec<Val>)> = vec![
        ("no node at all", vec![]),
        ("a node holding itself", vec![node(1, Some(0))]),
        (
            "a node holding itself, its index continuing the run",
            vec![node(1, None), node(2, Some(1))],
        ),
        (
            "a node holding one before it",
            vec![node(1, Some(1)), node(2, Some(0))],
        ),
        ("an index past the last node", vec![node(1, Some(1))]),
        (
            "a node no box holds",
            vec![node(1, Some(1)), node(2, None), node(3, None)],
        ),
        (
            "a run that skips an index",
            vec![node(1, Some(2)), node(2, None), node(3, None)],
        ),
    ];
    // Control: a well-formed chain is read.
    assert_eq!(
        raw(&head, &[Val::List(vec![node(7, Some(1)), node(8, None)])]),
        Ok(Val::S64(7))
    );
    for (what, ns) in cases {
        for r in [&head, &length] {
            let result = raw(r, &[Val::List(ns.clone())]);
            assert!(result.is_err(), "{what}: {result:?}");
        }
        // The host refuses the same nodes, by name.
        let untangled = length.call_untangled(&BTreeMap::new(), &[Val::List(ns)]);
        assert!(untangled.is_err(), "{what}: the host took {untangled:?}");
    }
    // A variant's boxes: two children, each its own index, in order.
    let root = compiled("chain.Root");
    let num = |n: i64| Val::Variant("num".into(), Some(Box::new(Val::S64(n))));
    let add = |a: u32, b: u32| {
        Val::Variant(
            "add".into(),
            Some(Box::new(Val::Tuple(vec![Val::U32(a), Val::U32(b)]))),
        )
    };
    assert_eq!(
        raw(&root, &[Val::List(vec![add(1, 2), num(1), num(2)])]),
        Ok(Val::S64(-1))
    );
    for (what, ns) in [
        ("one node two boxes hold", vec![add(1, 1), num(1)]),
        (
            "children out of level order",
            vec![add(2, 1), num(1), num(2)],
        ),
    ] {
        let result = raw(&root, &[Val::List(ns)]);
        assert!(result.is_err(), "{what}: {result:?}");
    }
}

#[test]
fn types_that_hold_each_other_in_place_compile_inside_a_component() {
    // `Even` holds an `Option<Odd>`, `Odd` an `Even`: both are boxed. And a
    // generic one, `Tree<Int>`, is boxed for its instance.
    let (mutual, grown) = (compiled("chain.Mutual"), compiled("chain.Grown"));
    for k in 0..CASES {
        assert_eq!(
            call(&mutual, &[Val::S64(k)]),
            Val::S64(k * (k + 1)),
            "Mutual({k})"
        );
        assert_eq!(
            call(&grown, &[Val::S64(k)]),
            Val::S64(1 + k * (k + 1) / 2),
            "Grown({k})"
        );
    }
}

/// A `Rose`, in Rust.
#[derive(Debug, Clone, PartialEq)]
struct Rose {
    label: i64,
    kids: Vec<Rose>,
    best: Option<Box<Rose>>,
}

impl Rose {
    fn grown(n: i64) -> Rose {
        if n <= 0 {
            Rose {
                label: 1,
                kids: vec![],
                best: None,
            }
        } else {
            Rose {
                label: n,
                kids: vec![Rose::grown(n - 1), Rose::grown(n - 2)],
                best: Some(Box::new(Rose::grown(n - 1))),
            }
        }
    }

    fn labels(&self) -> i64 {
        self.label
            + self.kids.iter().map(Rose::labels).sum::<i64>()
            + self.best.as_ref().map_or(0, |b| b.labels())
    }

    fn val(&self) -> Val {
        Val::Record(vec![
            ("label".into(), Val::S64(self.label)),
            (
                "kids".into(),
                Val::List(self.kids.iter().map(Rose::val).collect()),
            ),
            (
                "best".into(),
                Val::Option(self.best.as_ref().map(|b| Box::new(b.val()))),
            ),
        ])
    }
}

#[test]
fn a_list_of_boxes_crosses_as_its_nodes() {
    // `kids: List<Rose>` in a type boxed for its `best: Option<Rose>`: a list
    // of its cells' addresses, crossing as a list of indices.
    let (roses, labels) = (compiled("chain.Roses"), compiled("chain.Labels"));
    for n in 0..7 {
        let model = Rose::grown(n);
        assert_eq!(call(&roses, &[Val::S64(n)]), model.val(), "Roses({n})");
        assert_eq!(
            call(&labels, &[model.val()]),
            Val::S64(model.labels()),
            "Labels({n})"
        );
    }
}

#[test]
fn a_host_answers_with_a_value_held_in_place_and_is_given_one() {
    let (fetched, weighed) = (compiled("chain.Fetched"), compiled("chain.Weighed"));
    for n in 0..CASES / 4 {
        let values: Vec<i64> = (1..=n).collect();
        // The data layer answers nested; the host passes it as nodes.
        let answer = chain(&values);
        let calls = pw_conformance::Calls::default();
        let ops: BTreeMap<String, HostFn> =
            [operation(&calls, "chain:data/nodes#fetch", move |_| {
                answer.clone()
            })]
            .into_iter()
            .collect();
        assert_eq!(
            fetched.call_untangled(&ops, &[Val::S64(7)]),
            Ok(vec![Val::S64(values.iter().sum())])
        );
        // The component passes nodes; the data layer is given them nested.
        let calls = pw_conformance::Calls::default();
        let ops: BTreeMap<String, HostFn> = [operation(&calls, "chain:data/nodes#weigh", |args| {
            Val::S64(args.len() as i64)
        })]
        .into_iter()
        .collect();
        assert_eq!(
            weighed.call_untangled(&ops, &[Val::S64(n)]),
            Ok(vec![Val::S64(1)])
        );
        let given = calls.lock().expect("calls")[0].1.clone();
        let built: Vec<i64> = (1..=n).rev().collect();
        assert_eq!(given, vec![chain(&built)]);
    }
}
