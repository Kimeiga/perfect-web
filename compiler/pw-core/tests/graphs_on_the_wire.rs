//! **A value of a type that contains itself on the browser's wire, as its
//! nodes** (ADR-0205).
//!
//! `{ "$graph": [node, ...] }`: node 0 the value, each value of the type
//! inside a node `{ "$node": k }`, in level order, each node's own in the
//! order its fields are declared, as a component reads nodes (ADR-0194). The
//! build writes a signal's first value so; a handler reads a signal's value
//! from its nodes and writes one, and sends one to a command, so. Each
//! module runs under Node against a context that holds signals and records
//! what is set and sent.

use pw_core::backend::wasm::Encoding;
use pw_core::check::Unit;
use pw_core::lower::lower_file;
use pw_syntax::parse_tree;
use serde_json::json;

const PROGRAM: &str = r#"module m

import List

opaque type InteractionId = String

type Comment = Comment { id: Int, text: String, replies: List<Comment> }

type Oops = Oops { why: String }

command save(c: Comment) -> Result<Int, Oops>
    requires      SignedIn
    idempotent_by InteractionId
{
    Ok(c.id)
}

page P() {
    cache private

    signal draft: Comment = Comment {
        id: 1,
        text: "a",
        replies: [
            Comment { id: 2, text: "b", replies: [Comment { id: 4, text: "d", replies: [] }] },
            Comment { id: 3, text: "c", replies: [] }
        ]
    }

    view {
        <main>
            <button id="nest" type="button" on:press|refusable={() => draft = Comment { id: draft.id + 10, text: "wrapped", replies: [draft] }}>Nest</button>
            <button id="send" type="button" on:press|refusable={() => { let _saved = save(draft) }}>Send</button>
        </main>
    }
}
"#;

fn units(program: &str) -> Vec<Unit> {
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    let mut files = Vec::new();
    for dir in ["packages/pw-std", "packages/pw-platform-web"] {
        let mut paths: Vec<std::path::PathBuf> = std::fs::read_dir(root.join(dir))
            .expect(dir)
            .map(|e| e.expect("entry").path())
            .filter(|p| p.extension().is_some_and(|x| x == "pw"))
            .collect();
        paths.sort();
        files.extend(paths);
    }
    let mut out: Vec<Unit> = files
        .into_iter()
        .map(|p| {
            let src = std::fs::read_to_string(&p).expect("read");
            Unit {
                path: p.display().to_string(),
                hir: lower_file(&src, &parse_tree(&src).green),
                src,
            }
        })
        .collect();
    out.push(Unit {
        path: "m.pw".into(),
        hir: lower_file(program, &parse_tree(program).green),
        src: program.to_string(),
    });
    out
}

/// Each handler's module source, or why it was refused.
fn handlers(program: &str) -> Vec<Result<String, String>> {
    pw_core::backend::js::compile(&units(program))
        .expect("the program checks")
        .into_iter()
        .map(|c| match c.module {
            Encoding::Encoded(m) => Ok(m.source),
            other => Err(format!("{other:?}")),
        })
        .collect()
}

/// The module whose source writes `marker`.
fn handler(program: &str, marker: &str) -> String {
    handlers(program)
        .into_iter()
        .filter_map(Result::ok)
        .find(|s| s.contains(marker))
        .unwrap_or_else(|| panic!("no handler writes `{marker}`"))
}

/// What a module sets and sends, run under Node with `signals` held.
fn run(source: &str, signals: &serde_json::Value) -> serde_json::Value {
    // One directory each: tests run the same module at once.
    static RUNS: std::sync::atomic::AtomicUsize = std::sync::atomic::AtomicUsize::new(0);
    let dir = std::env::temp_dir().join(format!(
        "pw-graph-{}-{}",
        std::process::id(),
        RUNS.fetch_add(1, std::sync::atomic::Ordering::SeqCst)
    ));
    std::fs::create_dir_all(&dir).expect("temp");
    std::fs::write(dir.join("handler.mjs"), source).expect("write");
    std::fs::write(dir.join("signals.json"), signals.to_string()).expect("write");
    std::fs::write(
        dir.join("run.mjs"),
        "import { run } from \"./handler.mjs\";\n\
         import { readFileSync } from \"node:fs\";\n\
         const signals = JSON.parse(readFileSync(\"signals.json\", \"utf8\"));\n\
         const sent = [];\n\
         const set = [];\n\
         const context = {\n  captures: {},\n  get: (name) => signals[name],\n  \
         set: (name, value) => set.push([name, value]),\n  \
         command: async (id, args) => { sent.push([id, args]); return { $case: \"ok\" }; },\n};\n\
         try {\n  await run(context);\n  console.log(JSON.stringify({ sent, set }));\n} catch (e) {\n  \
         console.log(JSON.stringify({ sent, set, trap: String(e.message) }));\n}\n",
    )
    .expect("write");
    let out = std::process::Command::new("node")
        .arg("run.mjs")
        .current_dir(&dir)
        .output()
        .expect("node runs");
    std::fs::remove_dir_all(&dir).ok();
    assert!(
        out.status.success(),
        "{}",
        String::from_utf8_lossy(&out.stderr)
    );
    serde_json::from_slice(&out.stdout).expect("one line of JSON")
}

/// The draft's first value, as its nodes: level order, each node's replies
/// the next run.
fn first_draft() -> serde_json::Value {
    json!({ "$graph": [
        { "id": 1, "text": "a", "replies": [{ "$node": 1 }, { "$node": 2 }] },
        { "id": 2, "text": "b", "replies": [{ "$node": 3 }] },
        { "id": 3, "text": "c", "replies": [] },
        { "id": 4, "text": "d", "replies": [] },
    ]})
}

#[test]
fn the_build_writes_a_first_value_as_its_nodes() {
    let compiled = pw_core::backend::signals::compile(&units(PROGRAM)).expect("checks");
    let page = compiled.iter().find(|c| c.page == "m.P").expect("the page");
    let signals = page.signals.as_ref().expect("its first values are data");
    let draft = signals.iter().find(|s| s.name == "draft").expect("draft");
    assert_eq!(draft.initial, first_draft());
    // The renderer reads it as the tree it is.
    let nested = json!({ "id": 1, "text": "a", "replies": [
        { "id": 2, "text": "b", "replies": [{ "id": 4, "text": "d", "replies": [] }] },
        { "id": 3, "text": "c", "replies": [] },
    ]});
    assert_eq!(
        pw_render::Value::from_wire(&draft.initial).expect("a tree"),
        pw_render::Value::from_wire(&nested).expect("a value")
    );
}

#[test]
fn a_handler_reads_a_signal_from_its_nodes_and_writes_one_as_its_nodes() {
    let nest = handler(PROGRAM, "context.set(\"draft\"");
    // The first value, wrapped: its nodes one further down.
    let out = run(&nest, &json!({ "draft": first_draft() }));
    assert_eq!(
        out["set"],
        json!([["draft", { "$graph": [
            { "id": 11, "text": "wrapped", "replies": [{ "$node": 1 }] },
            { "id": 1, "text": "a", "replies": [{ "$node": 2 }, { "$node": 3 }] },
            { "id": 2, "text": "b", "replies": [{ "$node": 4 }] },
            { "id": 3, "text": "c", "replies": [] },
            { "id": 4, "text": "d", "replies": [] },
        ]}]]),
        "{out}"
    );
    // A chain 20,000 deep: read, wrapped and written without recursion.
    const DEEP: usize = 20_000;
    let nodes: Vec<serde_json::Value> = (0..DEEP)
        .map(|k| {
            let next: Vec<serde_json::Value> = if k + 1 < DEEP {
                vec![json!({ "$node": k + 1 })]
            } else {
                Vec::new()
            };
            json!({ "id": k, "text": "", "replies": next })
        })
        .collect();
    let out = run(&nest, &json!({ "draft": { "$graph": nodes } }));
    let written = out["set"][0][1]["$graph"].as_array().expect("nodes");
    assert_eq!(written.len(), DEEP + 1, "{}", out["trap"]);
    assert_eq!(written[0]["id"], 10);
    for (k, node) in written.iter().enumerate().skip(1) {
        assert_eq!(node["id"], k - 1);
        let held: Vec<u64> = node["replies"]
            .as_array()
            .expect("replies")
            .iter()
            .map(|r| r["$node"].as_u64().expect("a node"))
            .collect();
        let expected: Vec<u64> = if k < DEEP {
            vec![k as u64 + 1]
        } else {
            Vec::new()
        };
        assert_eq!(held, expected);
    }
}

#[test]
fn a_graph_that_is_not_a_tree_traps_in_the_handler() {
    let nest = handler(PROGRAM, "context.set(\"draft\"");
    let node = |replies: &[u64]| {
        let refs: Vec<serde_json::Value> = replies.iter().map(|i| json!({ "$node": i })).collect();
        json!({ "id": 1, "text": "", "replies": refs })
    };
    for nodes in [
        vec![node(&[1]), node(&[0])],
        vec![node(&[1, 1]), node(&[])],
        vec![node(&[]), node(&[])],
        vec![node(&[5])],
    ] {
        let out = run(&nest, &json!({ "draft": { "$graph": nodes } }));
        assert!(
            out["trap"]
                .as_str()
                .is_some_and(|t| t.contains("a `$graph` that is not a tree")),
            "{out}"
        );
        assert_eq!(out["set"], json!([]));
    }
    // Nested, as a value of no such type is written: no graph.
    let out = run(
        &nest,
        &json!({ "draft": { "id": 1, "text": "", "replies": [] } }),
    );
    assert!(
        out["trap"].as_str().is_some_and(|t| t.contains("no node")),
        "{out}"
    );
}

#[test]
fn a_handler_sends_a_command_a_value_as_its_nodes() {
    let send = handler(PROGRAM, "context.command(\"m.save\"");
    let out = run(&send, &json!({ "draft": first_draft() }));
    // Read and written again: the same nodes, as a component reads them.
    assert_eq!(out["sent"], json!([["m.save", [first_draft()]]]), "{out}");
}

#[test]
fn what_a_host_writes_of_one_and_two_types_that_hold_each_other_are_refused() {
    // A capture: the document's, which the renderer writes knowing no type.
    let captured = PROGRAM.replace(
        "page P() {",
        "public query Thread() -> Comment\n    freshness 30.seconds\n    consistency snapshot\n    \
         cache shared\n    concurrency one_per_key\n    timeout 2.seconds\n{\n    \
         Comment { id: 1, text: \"t\", replies: [] }\n}\n\n\
         page Q() {\n    cache private\n\n    let thread = query Thread()\n\n    view {\n        \
         <main><button type=\"button\" on:press|refusable={() => { let _s = save(thread) }}>Go</button></main>\n    \
         }\n}\n\npage P() {",
    );
    let refused: Vec<String> = handlers(&captured)
        .into_iter()
        .filter_map(Result::err)
        .collect();
    assert!(
        refused
            .iter()
            .any(|r| r.contains("holds a type that contains itself")),
        "{refused:?}"
    );
    // A command's declared error: written by its host, knowing no type.
    let errs = PROGRAM
        .replace("-> Result<Int, Oops>", "-> Result<Int, Comment>")
        .replace("    Ok(c.id)\n", "    Err(c)\n");
    let refused: Vec<String> = handlers(&errs)
        .into_iter()
        .filter_map(Result::err)
        .collect();
    assert!(
        refused
            .iter()
            .any(|r| r.contains("answer cannot be read by the page")
                && r.contains("contains itself")),
        "{refused:?}"
    );
    // Two types that hold each other: nodes of either.
    let mutual = "module m\n\ntype A = A { n: Int, bs: List<B> }\n\ntype B = B { as: List<A> }\n\n\
        page P() {\n    cache private\n\n    signal a: A = A { n: 1, bs: [] }\n\n    view {\n        \
        <main><button type=\"button\" on:press|refusable={() => a = A { n: a.n + 1, bs: [] }}>Go</button></main>\n    \
        }\n}\n";
    let refused: Vec<String> = handlers(mutual)
        .into_iter()
        .filter_map(Result::err)
        .collect();
    assert!(
        refused
            .iter()
            .any(|r| r.contains("two types that hold each other")),
        "{refused:?}"
    );
}

#[test]
fn each_node_holds_the_next_run_in_the_order_its_fields_are_declared() {
    // Two fields that hold the type: `left`'s nodes come before `right`'s,
    // as a component's decoder reads them (ADR-0194), at build and in the
    // browser alike.
    let program = "module m\n\nopaque type InteractionId = String\n\n\
        type Node = Node { n: Int, left: List<Node>, right: List<Node> }\n\n\
        page P() {\n    cache private\n\n    \
        signal tree: Node = Node { n: 1, left: [Node { n: 2, left: [], right: [] }], \
        right: [Node { n: 3, left: [Node { n: 4, left: [], right: [] }], right: [] }] }\n\n    \
        view {\n        <main><button type=\"button\" on:press|refusable={() => tree = \
        Node { n: 0, left: [tree], right: [tree] }}>Twice</button></main>\n    }\n}\n";
    let first = json!({ "$graph": [
        { "n": 1, "left": [{ "$node": 1 }], "right": [{ "$node": 2 }] },
        { "n": 2, "left": [], "right": [] },
        { "n": 3, "left": [{ "$node": 3 }], "right": [] },
        { "n": 4, "left": [], "right": [] },
    ]});
    let compiled = pw_core::backend::signals::compile(&units(program)).expect("checks");
    let page = compiled.iter().find(|c| c.page == "m.P").expect("the page");
    let tree = &page.signals.as_ref().expect("data")[0];
    assert_eq!(tree.initial, first);
    // The handler writes the tree twice under a new root, each copy's nodes
    // after the other's, in level order.
    let twice = handler(program, "context.set(\"tree\"");
    let out = run(&twice, &json!({ "tree": first }));
    assert_eq!(
        out["set"],
        json!([["tree", { "$graph": [
            { "n": 0, "left": [{ "$node": 1 }], "right": [{ "$node": 2 }] },
            { "n": 1, "left": [{ "$node": 3 }], "right": [{ "$node": 4 }] },
            { "n": 1, "left": [{ "$node": 5 }], "right": [{ "$node": 6 }] },
            { "n": 2, "left": [], "right": [] },
            { "n": 3, "left": [{ "$node": 7 }], "right": [] },
            { "n": 2, "left": [], "right": [] },
            { "n": 3, "left": [{ "$node": 8 }], "right": [] },
            { "n": 4, "left": [], "right": [] },
            { "n": 4, "left": [], "right": [] },
        ]}]]),
        "{out}"
    );
}
