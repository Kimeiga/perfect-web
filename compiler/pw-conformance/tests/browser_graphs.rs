//! **A value of a type that contains itself, from a browser, as its nodes**
//! (ADR-0205).
//!
//! A browser sends such a value as `{ "$graph": [node, ...] }`: node 0 the
//! value, each value of the type inside a node `{ "$node": k }`. The host
//! types it by the component's own parameter, each `$node` the index its node
//! holds there, and the component checks the nodes as it checks a host's
//! (ADR-0194). The graphs here are written by a model, independently of the
//! browser's encoder, and each query's answer is the model's. A host writes
//! one for a browser as the model does (ADR-0233).

use std::collections::BTreeMap;

use pw_conformance::{Runnable, compile, units};
use pw_host::engine::{HostFn, Val};
use serde_json::json;

const CASES: usize = 100;

const PROGRAM: &str = r#"module thread

import List

type Comment = Comment { text: String, likes: Int, replies: List<Comment> }

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

public query Size(c: Comment) -> Int { count(c) }

public query Liked(c: Comment) -> Int { liked(c) }

public query Deepest(c: Comment) -> Int { deepest(c) }

public query Root(c: Comment) -> String { c.text }

public query Echo(c: Comment) -> Comment { c }

type Tag =
    | Plain
    | Named(String)
    | Held(Comment)

type Wrapped = Wrapped {
    first: Option<Comment>,
    none: Option<Comment>,
    tag: Tag,
    plain: Tag,
    held: Tag,
    all: List<Comment>,
    count: Result<Int, String>,
}

public query Wrap(c: Comment) -> Wrapped {
    Wrapped {
        first: Some(c),
        none: None,
        tag: Named(c.text),
        plain: Plain,
        held: Held(c),
        all: [c, c],
        count: Ok(List.length(c.replies)),
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
    fn text(&mut self) -> String {
        const POOL: [&str; 6] = ["", "a", "Root", "人", "\u{1F600}", "quote\"d"];
        POOL[self.below(POOL.len() as u64) as usize].to_string()
    }
}

/// The model's comment.
#[derive(Debug, Clone)]
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

/// A comment as a browser sends it: its nodes in level order, each node's
/// replies the next run, written here independently of the browser's
/// encoder.
fn graph(c: &Comment) -> serde_json::Value {
    let mut queue = std::collections::VecDeque::from([c]);
    let mut nodes = Vec::new();
    let mut next = 1u64;
    while let Some(c) = queue.pop_front() {
        let k = c.replies.len() as u64;
        let refs: Vec<serde_json::Value> =
            (next..next + k).map(|i| json!({ "$node": i })).collect();
        nodes.push(json!({ "text": c.text, "likes": c.likes, "replies": refs }));
        next += k;
        queue.extend(c.replies.iter());
    }
    json!({ "$graph": nodes })
}

fn compiled(id: &str) -> Runnable {
    Runnable::new(compile(&units(&[("thread.pw", PROGRAM)]), id))
}

fn host() -> BTreeMap<String, HostFn> {
    BTreeMap::new()
}

/// The query's answer to what a browser sent, or why there is none: the
/// host's refusal, or the component's trap.
fn sent(r: &Runnable, json: serde_json::Value) -> Result<Val, String> {
    let args = r.arguments(&[json])?;
    r.call(&host(), &args).map(|mut v| v.remove(0))
}

#[test]
fn a_browsers_graph_is_the_tree_its_nodes_make() {
    let (size, liked_, deep, root) = (
        compiled("thread.Size"),
        compiled("thread.Liked"),
        compiled("thread.Deepest"),
        compiled("thread.Root"),
    );
    let mut rng = Rng(0x0DD_BA11);
    for _ in 0..CASES {
        let c = comment(&mut rng, 5);
        let g = graph(&c);
        assert_eq!(sent(&size, g.clone()), Ok(Val::S64(count(&c))), "{g}");
        assert_eq!(sent(&liked_, g.clone()), Ok(Val::S64(liked(&c))), "{g}");
        assert_eq!(sent(&deep, g.clone()), Ok(Val::S64(deepest(&c))), "{g}");
        assert_eq!(sent(&root, g), Ok(Val::String(c.text.clone())));
    }
}

/// **A host writes such a value for a browser as its nodes** (ADR-0233),
/// in the level order the browser's module decodes and the host reads: the
/// model's graph of the same tree, written independently. A comment the
/// component returns, made nested, is written back as the graph it was sent
/// as, and the host reads that graph again.
#[test]
fn a_hosts_graph_for_a_browser_is_the_trees_nodes_in_level_order() {
    let echo = compiled("thread.Echo");
    let mut rng = Rng(0xFEED_FACE);
    for _ in 0..CASES {
        let c = comment(&mut rng, 5);
        let g = graph(&c);
        let args = echo.arguments(std::slice::from_ref(&g)).expect("a graph");
        let nested = echo
            .call_untangled(&host(), &args)
            .expect("answered")
            .remove(0);
        let written = echo.browser_value(nested).expect("written");
        assert_eq!(written, g);
        echo.arguments(&[written]).expect("read again");
    }
}

/// **Each such value inside another is a graph of its own** (ADR-0205,
/// ADR-0233): an option's, a case's payload, each item of a list's; and
/// around them, a case by its name and payload, a result's `Ok`.
#[test]
fn a_hosts_graphs_inside_another_value_are_each_its_own() {
    let wrap = compiled("thread.Wrap");
    let mut rng = Rng(0xC0FF_EE00);
    for _ in 0..CASES / 10 {
        let c = comment(&mut rng, 4);
        let g = graph(&c);
        let args = wrap.arguments(std::slice::from_ref(&g)).expect("a graph");
        let nested = wrap
            .call_untangled(&host(), &args)
            .expect("answered")
            .remove(0);
        assert_eq!(
            wrap.browser_value(nested).expect("written"),
            json!({
                "first": { "$case": "some", "value": g.clone() },
                "none": { "$case": "none" },
                "tag": { "$case": "named", "value": c.text.clone() },
                "plain": { "$case": "plain" },
                "held": { "$case": "held", "value": g.clone() },
                "all": [g.clone(), g.clone()],
                "count": { "$case": "ok", "value": c.replies.len() },
            })
        );
    }
}

#[test]
fn a_chain_as_deep_as_its_data_crosses_from_a_browser() {
    // 50,000 nodes, as JSON three values deep: no parser's limit and no
    // stack is near it. The component decodes them in place and reads the
    // root; a function of its own that recursed down the chain would trap,
    // as ADR-0050 records.
    const DEEP: usize = 50_000;
    let nodes: Vec<serde_json::Value> = (0..DEEP)
        .map(|k| {
            let next: Vec<serde_json::Value> = if k + 1 < DEEP {
                vec![json!({ "$node": k + 1 })]
            } else {
                Vec::new()
            };
            json!({ "text": if k == 0 { "top" } else { "" }, "likes": 1, "replies": next })
        })
        .collect();
    let g = json!({ "$graph": nodes });
    // Through text, as a server reads a request.
    let read: serde_json::Value = serde_json::from_str(&g.to_string()).expect("parses");
    assert_eq!(
        sent(&compiled("thread.Root"), read),
        Ok(Val::String("top".into()))
    );
}

#[test]
fn a_graph_that_is_not_a_tree_traps_in_the_component() {
    let size = compiled("thread.Size");
    let node = |replies: &[u64]| {
        let refs: Vec<serde_json::Value> = replies.iter().map(|i| json!({ "$node": i })).collect();
        json!({ "text": "", "likes": 0, "replies": refs })
    };
    // Held by one before it, held twice, held by none, past the graph, and
    // out of level order: each the component's decoder refuses.
    for nodes in [
        vec![node(&[1]), node(&[0])],
        vec![node(&[1, 1]), node(&[])],
        vec![node(&[]), node(&[])],
        vec![node(&[3])],
        vec![node(&[2, 1]), node(&[]), node(&[])],
    ] {
        let g = json!({ "$graph": nodes });
        assert!(sent(&size, g.clone()).is_err(), "{g}");
    }
    // Control: the same nodes, a tree in level order.
    let tree = json!({ "$graph": [node(&[1, 2]), node(&[]), node(&[])] });
    assert_eq!(sent(&size, tree), Ok(Val::S64(3)));
}

#[test]
fn what_is_not_a_graph_is_refused_by_the_host() {
    let size = compiled("thread.Size");
    let refused = |j: serde_json::Value| size.arguments(&[j]).expect_err("refused");
    // Nested, as a value of no such type is written.
    assert!(refused(json!({ "text": "", "likes": 0, "replies": [] })).contains("`$graph`"));
    assert!(refused(json!({ "$graph": [3] })).contains("expected an object"));
    assert!(refused(json!({ "$graph": [{ "text": "", "replies": [] }] })).contains("`likes`"));
    assert!(
        refused(json!({ "$graph": [{ "text": "", "likes": 0, "replies": [1] }] }))
            .contains("`$node`")
    );
    assert!(
        refused(json!({ "$graph": [{ "text": "", "likes": 0, "replies": [{ "$node": -1 }] }] }))
            .contains("`$node`")
    );
}
