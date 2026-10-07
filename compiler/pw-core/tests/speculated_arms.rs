//! **A command speculates on several entries, and a page on the ones it
//! shows** (ADR-0238).
//!
//! A command's `optimistic` clause named one entry, and a page that called the
//! command without showing it was refused. The feed's `like` changes a post on
//! the home page's timeline and on its own thread page. Its clause names both
//! now, an arm each, and each page speculates on the arm whose target it
//! shows. An arm whose target a page shows under another key is still
//! refused, and each arm's name is its own target's value.

use pw_core::backend::wasm::Encoding;
use pw_core::check::{Unit, check_sources};
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

/// What `program` reports at check, its own file's diagnostics.
fn reported(program: &str) -> Vec<String> {
    let files: Vec<(String, String)> = units(program)
        .into_iter()
        .map(|u| (u.path, u.src))
        .collect();
    check_sources(&files)
        .into_iter()
        .filter(|(n, _)| n == "app.pw")
        .flat_map(|(_, ds)| ds.into_iter().map(|d| format!("{} {}", d.code, d.message)))
        .collect()
}

/// The page's speculation: none, compiled, or why it was refused.
fn speculation(
    program: &str,
    page: &str,
) -> Option<Result<pw_core::backend::speculation::Speculation, String>> {
    pw_core::backend::speculation::compile(&units(program))
        .expect("the program checks")
        .into_iter()
        .find(|c| c.page == page)
        .map(|c| match c.module {
            Encoding::Encoded(m) => Ok(m),
            other => Err(format!("{other:?}")),
        })
}

const LIKE: &str = "match like(id) {";

#[test]
fn each_page_speculates_on_the_arm_whose_target_it_shows() {
    let program = feed(&|s| s.to_string());
    assert_eq!(reported(&program), Vec::<String>::new());
    // The home page shows the timeline: `like`'s first arm.
    let home = speculation(&program, "feed.app.Home")
        .expect("the home page's")
        .unwrap_or_else(|e| panic!("{e}"));
    assert!(
        home.source
            .contains("\"feed.app.like\": [{ binding: \"feed\","),
        "{}",
        home.source
    );
    assert!(
        !home.source.contains("binding: \"thread\""),
        "{}",
        home.source
    );
    // The thread page shows the thread its `id` keys: the second.
    let thread = speculation(&program, "feed.app.PostPage")
        .expect("the thread page's")
        .unwrap_or_else(|e| panic!("{e}"));
    assert!(
        thread
            .source
            .contains("\"feed.app.like\": [{ binding: \"thread\","),
        "{}",
        thread.source
    );
    assert!(
        !thread.source.contains("binding: \"feed\""),
        "{}",
        thread.source
    );
}

#[test]
fn an_arm_whose_target_a_page_shows_under_another_key_is_refused() {
    // The thread page's like given another post's id: the thread it shows is
    // not the entry the arm names.
    let program = feed(&|s| {
        s.replace(LIKE, "match like(PostId(\"p1\")) {").replace(
            "<button id=\"like\" type=\"button\" on:press={resumable(captures = { id }) =>",
            "<button id=\"like\" type=\"button\" on:press={() =>",
        )
    });
    match speculation(&program, "feed.app.PostPage").expect("the thread page's") {
        Ok(s) => panic!("speculated {:?}", s.bindings),
        Err(e) => assert!(
            e.contains("reads by no binding whose key resolves to the same invocation context"),
            "{e}"
        ),
    }
}

#[test]
fn a_page_that_shows_no_target_has_no_speculation() {
    // A page that likes a post and shows neither the timeline nor a thread.
    let program = feed(&|s| {
        format!(
            "{s}\npage Likes() {{\n    route \"/likes\"\n    cache private\n\n    signal n: Int = 0\n\n    \
             view {{\n        <title>Likes</title>\n        <main>\n            \
             <button type=\"button\" on:press={{() => match like(PostId(\"p1\")) {{\n                \
             Ok(_) => n = n + 1,\n                Err(_) => n = n,\n            }}}}>Like</button>\n        \
             </main>\n    }}\n}}\n"
        )
    });
    assert_eq!(reported(&program), Vec::<String>::new());
    assert!(
        speculation(&program, "feed.app.Likes").is_none(),
        "a page that speculates on nothing"
    );
}

#[test]
fn each_arms_name_is_its_own_targets_value() {
    // The thread arm given the timeline's transition: its `thread` is a
    // `Post`, which `liked` does not take.
    let program = feed(&|s| {
        s.replace(
            "Thread(post) as thread => liked_thread(thread, post)",
            "Thread(post) as thread => liked(thread, post)",
        )
    });
    let found = reported(&program);
    assert!(!found.is_empty(), "checked clean");
    assert!(
        found.iter().any(|d| d.contains("Post")),
        "the arm's value is a `Post`: {found:#?}"
    );
}

#[test]
fn arms_are_separated_by_commas() {
    // Without its comma the thread's arm vanished, and the program checked
    // (ADR-0237). It is the one error now, and the arm is read.
    let program = feed(&|s| {
        s.replace(
            "liked(feed, post),\n                  Thread(post)",
            "liked(feed, post)\n                  Thread(post)",
        )
    });
    assert_eq!(
        reported(&program),
        ["PW0016 an optimistic clause's arms are separated by commas"]
    );
    let hir = lower_file(&program, &parse_tree(&program).green);
    let (_, like) = hir
        .all_decls()
        .find(|(_, d)| d.name == "like")
        .expect("the like command");
    // The timeline's, the following timeline's (ADR-0257) and the thread's.
    assert_eq!(like.optimistic_clauses().len(), 3);
}
