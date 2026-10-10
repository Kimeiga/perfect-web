//! **A speculation on a value of a type that contains itself** (ADR-0233).
//!
//! A page's speculation module decodes the value the server sends it, and a
//! value of a type that contains itself was refused there: the server wrote
//! it nested, knowing no type, and a decoder written out by the type's shape
//! never ends (ADR-0205 §5). The server writes it by its query's type now,
//! as its nodes, and the module reads it so, as a handler reads a signal's.
//! The module runs here under Node: it decodes a thread from its nodes,
//! applies the reply a press speculates, and computes what the page shows.

use pw_core::backend::wasm::Encoding;
use pw_core::check::Unit;
use pw_core::lower::lower_file;
use pw_syntax::parse_tree;
use serde_json::json;

const PROGRAM: &str = r#"module m

import List
import context.{ current_session }
import capability.{ Session, SessionId }

opaque type InteractionId = String

type Comment = Comment { id: Int, text: String, replies: List<Comment> }

event Replied(id: Int)

fn thread_of(s: Session<SessionId>) -> Comment !{ database.read<Comment> }
    host "m:data/comments#thread"

fn add(s: Session<SessionId>, text: String) -> Comment !{ database.write<Comment> }
    host "m:data/comments#add"

session query Thread(session: Session<SessionId>) -> Comment
    freshness      0.seconds
    consistency    read_your_writes
    cache          private
    key            session
    invalidates_on Replied(_)
    concurrency    one_per_key
    on_key_change  cancel
    timeout        2.seconds
{
    thread_of(session)
}

command reply(text: String) -> Comment
    requires      SignedIn
    idempotent_by InteractionId
    emits         Replied(1)
    optimistic    Thread(current_session()) as t => replied(t, text)
{
    add(current_session(), text)
}

fn replied(t: Comment, text: String) -> Comment !{} {
    Comment { id: t.id, text: t.text, replies: List.concat(t.replies, [Comment { id: 0, text: text, replies: [] }]) }
}

session page P() {
    route     "/"
    placement origin
    cache     private

    let thread = query Thread(current_session())

    view {
        <title>P</title>
        <main>
            <p id="text">{thread.text}</p>
            <p id="count">{List.length(thread.replies)}</p>
            <button type="button" on:press|refusable={() => reply("new")}>Reply</button>
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

/// The page's speculation module, or why it was refused.
fn module(program: &str) -> Result<String, String> {
    pw_core::backend::speculation::compile(&units(program))
        .expect("the program checks")
        .into_iter()
        .find(|c| c.page == "m.P")
        .map(|c| match c.module {
            Encoding::Encoded(m) => Ok(m.source),
            other => Err(format!("{other:?}")),
        })
        .expect("the page's module")
}

/// What the module makes of `sent`: the thread it decodes, the parts it
/// computes from it, and the same after the reply a press speculates.
fn run(source: &str, sent: &serde_json::Value) -> serde_json::Value {
    // One directory each: the tests run at once.
    static RUNS: std::sync::atomic::AtomicUsize = std::sync::atomic::AtomicUsize::new(0);
    let dir = std::env::temp_dir().join(format!(
        "pw-speculated-graph-{}-{}",
        std::process::id(),
        RUNS.fetch_add(1, std::sync::atomic::Ordering::SeqCst)
    ));
    std::fs::create_dir_all(&dir).expect("temp");
    std::fs::write(dir.join("module.mjs"), source).expect("write");
    std::fs::write(dir.join("sent.json"), sent.to_string()).expect("write");
    std::fs::write(
        dir.join("run.mjs"),
        "import { decode, parts, commands } from \"./module.mjs\";\n\
         import { readFileSync } from \"node:fs\";\n\
         const sent = JSON.parse(readFileSync(\"sent.json\", \"utf8\"));\n\
         const plain = (v) => JSON.parse(JSON.stringify(v, (_, x) => typeof x === \"bigint\" ? Number(x) : x));\n\
         const shown = (v) => Object.fromEntries(Object.entries(parts.thread).map(([k, f]) => [k, String(f(v))]));\n\
         try {\n  const thread = decode.thread(sent);\n  \
         const replied = commands[\"m.reply\"][0].transition(thread, [\"new\"]);\n  \
         console.log(JSON.stringify({ thread: plain(thread), shown: shown(thread), replied: plain(replied), after: shown(replied) }));\n\
         } catch (e) {\n  console.log(JSON.stringify({ trap: String(e.message) }));\n}\n",
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

#[test]
fn a_speculated_thread_is_read_from_its_nodes_and_replied_to() {
    let source = module(PROGRAM).unwrap_or_else(|e| panic!("{e}"));
    // As the server writes it (ADR-0233): the thread's nodes in level order.
    let sent = json!({ "$graph": [
        { "id": 1, "text": "a", "replies": [{ "$node": 1 }, { "$node": 2 }] },
        { "id": 2, "text": "b", "replies": [{ "$node": 3 }] },
        { "id": 3, "text": "c", "replies": [] },
        { "id": 4, "text": "d", "replies": [] },
    ]});
    let got = run(&source, &sent);
    assert_eq!(
        got["thread"],
        json!({ "id": 1, "text": "a", "replies": [
            { "id": 2, "text": "b", "replies": [{ "id": 4, "text": "d", "replies": [] }] },
            { "id": 3, "text": "c", "replies": [] },
        ]}),
        "{got}"
    );
    let shown: Vec<&str> = got["shown"]
        .as_object()
        .expect("the parts")
        .values()
        .filter_map(|v| v.as_str())
        .collect();
    assert!(
        shown.contains(&"a") && shown.contains(&"2"),
        "the text and the count: {got}"
    );
    // The reply a press speculates, last among the thread's replies, and
    // counted.
    assert_eq!(got["replied"]["replies"][2]["text"], "new", "{got}");
    let after: Vec<&str> = got["after"]
        .as_object()
        .expect("the parts")
        .values()
        .filter_map(|v| v.as_str())
        .collect();
    assert!(after.contains(&"3"), "{got}");
}

#[test]
fn a_thread_sent_nested_or_not_a_tree_traps() {
    let source = module(PROGRAM).unwrap_or_else(|e| panic!("{e}"));
    // Nested, as the server wrote it knowing no type: no graph.
    let nested = run(&source, &json!({ "id": 1, "text": "a", "replies": [] }));
    assert!(
        nested["trap"]
            .as_str()
            .is_some_and(|t| t.contains("`$graph`")),
        "{nested}"
    );
    // A node held twice.
    let twice = run(
        &source,
        &json!({ "$graph": [
            { "id": 1, "text": "a", "replies": [{ "$node": 1 }, { "$node": 1 }] },
            { "id": 2, "text": "b", "replies": [] },
        ]}),
    );
    assert!(
        twice["trap"]
            .as_str()
            .is_some_and(|t| t.contains("not a tree")),
        "{twice}"
    );
}
