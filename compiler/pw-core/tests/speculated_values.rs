//! **What a speculated value computes, the page's module computes** wherever
//! the page shows it (ADR-0235).
//!
//! A text part computed from a speculated value whole was the module's
//! (ADR-0228). A block's subject computed from one was refused, and a value
//! computed from one inside a block was skipped: no part, no region, and
//! nothing refused it, so a count inside a block kept the server's beside one
//! a press changed. A block whose subject is computed from the value is a
//! region now, its subject and each value computed inside it the module's,
//! given to the renderer with the value. One inside a block no region renders
//! is refused by name. The module runs here under Node.

use pw_core::backend::wasm::Encoding;
use pw_core::check::Unit;
use pw_core::lower::lower_file;
use pw_syntax::parse_tree;
use serde_json::json;

/// A thread a session replies to, `markup` beside its count.
fn program(markup: &str) -> String {
    format!(
        r#"module m

import List
import context.{{ current_session }}
import capability.{{ Session, SessionId }}

opaque type InteractionId = String

type Comment = Comment {{ id: Int, text: String, replies: List<Comment> }}

event Replied(id: Int)

fn thread_of(s: Session<SessionId>) -> Comment !{{ database.read<Comment> }}
    host "m:data/comments#thread"

fn flag_of(s: Session<SessionId>) -> Bool !{{ database.read<Comment> }}
    host "m:data/comments#flag"

fn add(s: Session<SessionId>, text: String) -> Comment !{{ database.write<Comment> }}
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
{{
    thread_of(session)
}}

session query Flag(session: Session<SessionId>) -> Bool
    freshness      0.seconds
    consistency    read_your_writes
    cache          private
    key            session
    invalidates_on Replied(_)
    concurrency    one_per_key
    on_key_change  cancel
    timeout        2.seconds
{{
    flag_of(session)
}}

command reply(text: String) -> Comment
    requires      SignedIn
    idempotent_by InteractionId
    emits         Replied(1)
    optimistic    Thread(current_session()) as t => replied(t, text)
{{
    add(current_session(), text)
}}

fn replied(t: Comment, text: String) -> Comment !{{}} {{
    Comment {{ id: t.id, text: t.text, replies: List.concat(t.replies, [Comment {{ id: 0, text: text, replies: [] }}]) }}
}}

session page P() {{
    route     "/"
    placement origin
    cache     private

    let thread = query Thread(current_session())
    let flag = query Flag(current_session())

    view {{
        <title>P</title>
        <main>
            <p id="count">{{List.length(thread.replies)}}</p>
            {markup}
            <button type="button" on:press|refusable={{() => reply("new")}}>Reply</button>
        </main>
    }}
}}
"#
    )
}

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

/// The page's speculation, or why it was refused.
fn speculation(program: &str) -> Result<pw_core::backend::speculation::Speculation, String> {
    pw_core::backend::speculation::compile(&units(program))
        .expect("the program checks")
        .into_iter()
        .find(|c| c.page == "m.P")
        .map(|c| match c.module {
            Encoding::Encoded(m) => Ok(m),
            other => Err(format!("{other:?}")),
        })
        .expect("the page's speculation")
}

/// What the module computes for its regions from `sent`, and after the
/// reply a press speculates, each by its path.
fn computed(source: &str, sent: &serde_json::Value) -> serde_json::Value {
    static RUNS: std::sync::atomic::AtomicUsize = std::sync::atomic::AtomicUsize::new(0);
    let dir = std::env::temp_dir().join(format!(
        "pw-speculated-values-{}-{}",
        std::process::id(),
        RUNS.fetch_add(1, std::sync::atomic::Ordering::SeqCst)
    ));
    std::fs::create_dir_all(&dir).expect("temp");
    std::fs::write(dir.join("module.mjs"), source).expect("write");
    std::fs::write(dir.join("sent.json"), sent.to_string()).expect("write");
    std::fs::write(
        dir.join("run.mjs"),
        "import { decode, computed, commands } from \"./module.mjs\";\n\
         import { readFileSync } from \"node:fs\";\n\
         const sent = JSON.parse(readFileSync(\"sent.json\", \"utf8\"));\n\
         const at = (v) => Object.fromEntries(Object.entries(computed.thread).map(([k, f]) => [k, String(f(v))]));\n\
         const thread = decode.thread(sent);\n\
         const replied = commands[\"m.reply\"][0].transition(thread, [\"new\"]);\n\
         console.log(JSON.stringify({ before: at(thread), after: at(replied) }));\n",
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
fn a_subject_and_a_value_in_its_block_are_the_modules() {
    let s = speculation(&program(
        "{#if List.length(thread.replies) == 0}<p id=\"quiet\">None yet, of {List.length(thread.replies)}.</p>{/if}",
    ))
    .unwrap_or_else(|e| panic!("{e}"));
    // The block is a region of `thread`, rendered as a block is.
    assert!(
        s.source.contains("\"thread\": [{ kind: \"block\", part: "),
        "{}",
        s.source
    );
    // Its subject and the value inside it, each the module's, by its path:
    // false and 1 for a thread with a reply, and after another, false and 2.
    let sent = json!({ "$graph": [
        { "id": 1, "text": "a", "replies": [{ "$node": 1 }] },
        { "id": 2, "text": "b", "replies": [] },
    ]});
    let got = computed(&s.source, &sent);
    let values = |at: &str| -> Vec<String> {
        let mut v: Vec<String> = got[at]
            .as_object()
            .expect("by path")
            .iter()
            .map(|(k, v)| {
                format!(
                    "{}={}",
                    k.rsplit('~').next().unwrap_or_default(),
                    v.as_str().unwrap_or_default()
                )
            })
            .collect();
        v.sort();
        v
    };
    assert_eq!(values("before").len(), 2, "{got}");
    assert!(
        values("before").iter().any(|v| v.ends_with("=false")),
        "{got}"
    );
    assert!(values("before").iter().any(|v| v.ends_with("=1")), "{got}");
    assert!(values("after").iter().any(|v| v.ends_with("=2")), "{got}");
}

#[test]
fn a_value_no_region_renders_is_refused_by_name() {
    // Inside a block a query's value the press does not change decides.
    match speculation(&program(
        "{#if flag}<p id=\"inner\">{List.length(thread.replies)}</p>{/if}",
    )) {
        Ok(_) => panic!("compiled"),
        Err(e) => assert!(
            e.contains(
                "computes a value from `thread` inside a block, which a speculation would not reach"
            ),
            "{e}"
        ),
    }
    // Control: the same value inside the block its own subject decides.
    speculation(&program(
        "{#if List.length(thread.replies) > 0}<p>{List.length(thread.replies)}</p>{/if}",
    ))
    .unwrap_or_else(|e| panic!("{e}"));
}
