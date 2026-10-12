//! **A speculation on the entry a page's parameter keys** (ADR-0236, ruling
//! 0122-d).
//!
//! A speculation's target names a resource and its key, and a page shows it
//! when one of its bindings reads the same entry. A key was matched by an
//! invocation-context call, `current_session()`, or by `_` (ADR-0222). The
//! feed's `reply(to, text)` speculates on `Thread(to)`, and its thread page
//! binds `Thread(id)` and passes `id` to `to`. The key matches now when every
//! handler on the page passes the command's parameter the page's parameter
//! the binding reads, unchanged. Any other flow is refused by name.

use pw_core::backend::wasm::Encoding;
use pw_core::check::Unit;
use pw_core::lower::lower_file;
use pw_syntax::parse_tree;

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
        path: "app.pw".into(),
        hir: lower_file(program, &parse_tree(program).green),
        src: program.to_string(),
    });
    out
}

/// The feed, its `app.pw` changed by `change`.
fn feed(change: &dyn Fn(&str) -> String) -> String {
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    change(&std::fs::read_to_string(root.join("examples/feed/app.pw")).expect("the feed"))
}

/// The thread page's speculation, or why it was refused.
fn thread_page(program: &str) -> Result<pw_core::backend::speculation::Speculation, String> {
    pw_core::backend::speculation::compile(&units(program))
        .expect("the program checks")
        .into_iter()
        .find(|c| c.page == "feed.app.PostPage")
        .map(|c| match c.module {
            Encoding::Encoded(m) => Ok(m),
            other => Err(format!("{other:?}")),
        })
        .expect("the thread page's speculation")
}

const PASSED: &str = "Some(text) => match reply(id, text) {";

#[test]
fn the_thread_page_speculates_on_the_thread_its_parameter_keys() {
    let s = thread_page(&feed(&|s| s.to_string())).unwrap_or_else(|e| panic!("{e}"));
    // And `like`, on the same thread, since ADR-0238.
    assert_eq!(s.commands, ["feed.app.like", "feed.app.reply"]);
    let [binding] = s.bindings.as_slice() else {
        panic!("{:?}", s.bindings);
    };
    assert_eq!(
        (binding.binding.as_str(), binding.resource.as_str()),
        ("thread", "feed.app.Thread")
    );
    // Its key, as the page writes it: the page's parameter, which the host
    // reads from the document's address.
    assert_eq!(binding.key, ["id"]);
}

#[test]
fn any_other_flow_to_the_key_is_refused_by_name() {
    let why = "reads by no binding whose key resolves to the same invocation context, or to the \
               page's parameter each of its handlers passes the command unchanged (ruling 0122-d)";
    for (what, change) in [
        // The page's parameter through a name of the handler's own, which
        // the rule does not follow.
        (
            "a name bound to it",
            Box::new(|s: &str| {
                s.replace(
                    PASSED,
                    "Some(text) => match { let to = id\n    reply(to, text) } {",
                )
            }) as Box<dyn Fn(&str) -> String>,
        ),
        // Another post's.
        (
            "another post",
            Box::new(|s: &str| {
                s.replace(PASSED, "Some(text) => match reply(PostId(\"p1\"), text) {")
                    .replace(
                        "resumable(captures = { id }) => match post_text(answer)",
                        "() => match post_text(answer)",
                    )
            }),
        ),
        // A view the page composes, which its use may give anything.
        (
            "a view's call",
            Box::new(|s: &str| {
                s.replace(
                    "// A post and its replies, each a post with its own, as deep as they go",
                    "view Plus(to: PostId) {\n    <button type=\"button\" on:press|refusable={resumable(captures = { to }) => match post_text(\"+1\") {\n        Some(t) => match reply(to, t) {\n            Ok(_) => (),\n            Err(_) => (),\n        },\n        None => (),\n    }}>+1</button>\n}\n\n// A post and its replies, each a post with its own, as deep as they go",
                )
                .replace(
                    "            <Replies post={thread} />\n",
                    "            <Replies post={thread} />\n            <Plus to={id} />\n",
                )
            }),
        ),
    ] {
        match thread_page(&feed(&*change)) {
            Ok(s) => panic!("{what}: speculated {:?}", s.bindings),
            Err(e) => assert!(e.contains(why), "{what}: {e}"),
        }
    }
}

#[test]
fn a_region_a_speculation_renders_reads_the_pages_parameter() {
    // The page's parameters are its document's, which gives them to the
    // browser: a block the thread decides reads the thread's `id`.
    let s = thread_page(&feed(&|s| {
        s.replace(
            "<p id=\"quiet\">No replies yet.</p>",
            "<p id=\"quiet\">No replies to {id} yet.</p>",
        )
    }))
    .unwrap_or_else(|e| panic!("{e}"));
    // The block is a region, and its `id` is read from the document.
    assert!(
        s.source.contains("\"thread\": [{ kind: \"block\", part: "),
        "{}",
        s.source
    );
}
