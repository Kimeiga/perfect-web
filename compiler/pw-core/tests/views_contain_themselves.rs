//! **A view that contains itself** (ADR-0203, ADR-0130's ruling 2).
//!
//! A reply thread is a view of a comment holding the same view for each
//! reply. Composed in place it would never end, so it is refused where no
//! block is on the way back to it, and otherwise each use is an instance of
//! the view's own template, made at run time in a frame of its own, as a
//! loop's row is. The renderer bounds how deep the instances take a page, at
//! what a browser's HTML parser nests.

use pw_core::check::check_sources;
use pw_core::lower::lower_file;
use pw_syntax::parse_tree;

const THREAD: &str = r#"module t

type Comment = Comment { id: Int, text: String, replies: List<Comment> }

view Thread(c: Comment) !{} {
    <li>
        <p>{c.text}</p>
        <ul>
            {#each c.replies as r (r.id)}
                <Thread c={r} />
            {/each}
        </ul>
    </li>
}

view Root(c: Comment) !{} {
    <ul>
        <Thread c={c} />
    </ul>
}
"#;

fn reported(src: &str) -> Vec<String> {
    check_sources(&[("t.pw".to_string(), src.to_string())])
        .into_iter()
        .flat_map(|(_, ds)| ds.into_iter().map(|d| format!("{} {}", d.code, d.message)))
        .collect()
}

/// Each template, as the renderer reads it.
fn templates(src: &str) -> Vec<pw_render::Template> {
    let hir = lower_file(src, &parse_tree(src).green);
    pw_core::template_ir::build(&[&hir])
        .into_iter()
        .map(|t| serde_json::from_value(serde_json::to_value(t).expect("writes")).expect("reads"))
        .collect()
}

/// A comment whose replies are `replies`, as the wire gives one.
fn comment(id: i64, replies: Vec<serde_json::Value>) -> serde_json::Value {
    serde_json::json!({ "id": id, "text": format!("c{id}"), "replies": replies })
}

/// The HTML without the renderer's markers.
fn visible(html: &str) -> String {
    let mut out = String::new();
    let mut rest = html;
    while let Some(i) = rest.find("<!--pw:") {
        out.push_str(&rest[..i]);
        let j = rest[i..].find("-->").expect("closed");
        rest = &rest[i + j + 3..];
    }
    out.push_str(rest);
    out
}

#[test]
fn a_view_that_contains_itself_through_a_block_checks() {
    assert_eq!(reported(THREAD), Vec::<String>::new());
}

#[test]
fn one_that_contains_itself_with_no_block_on_the_way_back_is_refused() {
    let endless = "module t\n\ntype C = C { n: Int }\n\nview Loop(c: C) !{} {\n    <div><Loop c={c} /></div>\n}\n";
    let found = reported(endless);
    assert!(
        found.iter().any(|d| d.starts_with("PW5020")
            && d.contains("`<Loop>` contains itself with no `{#if}`, `{#match}` or `{#each}`")),
        "{found:?}"
    );
    // Through another view, with no block either way: refused.
    let mutual = "module t\n\ntype C = C { n: Int }\n\nview A(c: C) !{} {\n    <div><B c={c} /></div>\n}\n\nview B(c: C) !{} {\n    <p><A c={c} /></p>\n}\n";
    assert!(
        reported(mutual).iter().any(|d| d.starts_with("PW5020")),
        "{:?}",
        reported(mutual)
    );
    // A block on the way back, in either view, ends it.
    let ended = mutual.replace("<p><A c={c} /></p>", "<p>{#if c.n > 0}<A c={c} />{/if}</p>");
    assert_eq!(reported(&ended), Vec::<String>::new());
}

#[test]
fn a_view_that_contains_itself_holds_no_signal_yet() {
    let held = THREAD.replace(
        "view Thread(c: Comment) !{} {\n",
        "view Thread(c: Comment) !{} {\n    signal open: Bool = false\n\n",
    );
    let found = reported(&held);
    assert!(
        found
            .iter()
            .any(|d| d.starts_with("PW5020") && d.contains("holds a signal")),
        "{found:?}"
    );
}

#[test]
fn a_view_that_contains_itself_shows_no_stream_yet() {
    // A host runs the streams its page's plan names, and an instance's are
    // its view's (ADR-0148): one would show its placeholder for ever.
    let streamed = THREAD
        .replace(
            "view Thread(c: Comment) !{} {\n",
            "type E = E { why: String }\n\n\
             public query More(id: Int) -> Result<List<Comment>, E>\n    freshness 0.seconds\n    \
             consistency eventual\n    cache shared\n    key id\n    concurrency one_per_key\n    \
             on_key_change cancel\n    delivery streamed\n    timeout 3.seconds\n{\n    Ok([])\n}\n\n\
             view Thread(c: Comment) !{} {\n",
        )
        .replace(
            "        <p>{c.text}</p>\n",
            "        <p>{c.text}</p>\n        <stream query={More(c.id)}>\n            \
             <placeholder><p>Loading</p></placeholder>\n            \
             <ready as={more}><p>More</p></ready>\n            \
             <failed as={why}><p>None</p></failed>\n        </stream>\n",
        );
    let found = reported(&streamed);
    assert!(
        found
            .iter()
            .any(|d| d.starts_with("PW5020") && d.contains("shows a query's state in a `<stream>`")),
        "{found:?}"
    );
    // Control: the same stream in a view that does not contain itself.
    let once = streamed.replace("<Thread c={r} />", "<p>{r.text}</p>");
    assert!(
        reported(&once).iter().all(|d| !d.starts_with("PW5020")),
        "{:?}",
        reported(&once)
    );
}

#[test]
fn each_use_is_an_instance_of_the_views_own_template() {
    let all = templates(THREAD);
    let parts = |path: &str| -> String {
        let t = all.iter().find(|t| t.path == path).expect(path);
        serde_json::to_string(&t.chunks).expect("writes")
    };
    // In its own template, inside the loop's row, at two elements deep, and
    // its template nests two.
    let thread = parts("t.Thread");
    assert!(
        thread.contains(
            r#""part":"instance","id":2,"path":"t.Thread","args":[["c","r"]],"elements":2,"deepest":2"#
        ),
        "{thread}"
    );
    // And where it is used: an instance too, never composed in place.
    let root = parts("t.Root");
    assert!(
        root.contains(
            r#""part":"instance","id":0,"path":"t.Thread","args":[["c","c"]],"elements":1,"deepest":2"#
        ),
        "{root}"
    );
    assert!(!root.contains("<li>"), "{root}");
}

#[test]
fn it_renders_to_the_depth_its_data_has_each_instance_in_a_frame() {
    let all = templates(THREAD);
    let root = all.iter().find(|t| t.path == "t.Root").expect("Root");
    let tree = comment(
        1,
        vec![comment(2, vec![comment(4, vec![])]), comment(3, vec![])],
    );
    let env = pw_render::Env::new().set("c", pw_render::Value::from_wire(&tree).expect("a value"));
    let html = pw_render::render(root, &env, &all).expect("renders");
    assert_eq!(
        visible(&html).split_whitespace().collect::<String>(),
        "<ul><li><p>c1</p><ul><li><p>c2</p><ul><li><p>c4</p><ul></ul></li></ul></li>\
         <li><p>c3</p><ul></ul></li></ul></li></ul>"
    );
    // One frame per instance, each its own token: four instances, and the
    // loop's three rows.
    let opened: Vec<&str> = html
        .match_indices("<!--pw:s")
        .map(|(i, _)| &html[i..i + html[i..].find("-->").expect("closed")])
        .filter(|m| m.contains('@'))
        .collect();
    assert_eq!(opened.len(), 7, "{opened:?}");
    let tokens: std::collections::BTreeSet<&str> = opened
        .iter()
        .map(|m| m.split('@').nth(1).expect("token"))
        .collect();
    assert_eq!(tokens.len(), 7, "{opened:?}");
    // Each instance's markup is inside its own frame: `Root`'s, part 0,
    // opens on the comment's `<li>`.
    let at = html.find("<!--pw:s0@").expect("Root's instance");
    let inside = &html[at + html[at..].find("-->").expect("closed") + 3..];
    assert!(inside.starts_with("<li>"), "{html}");
}

#[test]
fn past_what_a_browser_nests_it_is_refused() {
    let all = templates(THREAD);
    let root = all.iter().find(|t| t.path == "t.Root").expect("Root");
    // A chain: each reply nests two more elements than the one it answers.
    let chain = |depth: i64| {
        (0..depth)
            .rev()
            .fold(comment(depth, vec![]), |c, k| comment(k, vec![c]))
    };
    let render = |depth: i64| {
        let env = pw_render::Env::new().set(
            "c",
            pw_render::Value::from_wire(&chain(depth)).expect("a value"),
        );
        pw_render::render(root, &env, &all)
    };
    // `Root`'s `<ul>`, then two for each comment: 1 + 2 × 249 = 499.
    assert!(render(247).is_ok());
    let deep = render(260);
    assert!(
        matches!(deep, Err(pw_render::Blocked::TooDeep { ref path, .. }) if path == "t.Thread"),
        "{deep:?}"
    );
}

#[test]
fn the_renderer_goes_no_deeper_for_each_instance() {
    // The deepest chain the bound allows, on a thread with Rust's default
    // stack, 2 MiB, a server worker's. Each instance is written after the
    // markup around it, never on its stack. Rendered one inside another, it
    // overflowed 4 MiB in a debug build; written after, it needs under one.
    let all = templates(THREAD);
    let root = all.iter().find(|t| t.path == "t.Root").expect("Root");
    let chain = (0..247)
        .rev()
        .fold(comment(247, vec![]), |c, k| comment(k, vec![c]));
    let env = pw_render::Env::new().set("c", pw_render::Value::from_wire(&chain).expect("a value"));
    let html = std::thread::scope(|s| {
        std::thread::Builder::new()
            .stack_size(2 * 1024 * 1024)
            .spawn_scoped(s, || pw_render::render(root, &env, &all))
            .expect("spawns")
            .join()
            .expect("returns")
    })
    .expect("renders");
    assert_eq!(html.matches("<li>").count(), 248);
    // In document order: each comment's text before its reply's.
    let shown = visible(&html);
    let at = |k: i64| shown.find(&format!("<p>c{k}<")).expect("written");
    assert!((0..247).all(|k| at(k) < at(k + 1)));
}

#[test]
fn a_templates_schema_closes_over_the_views_its_instances_reach() {
    // A document is read against its template's schema (ADR-0203). One
    // rendered with another `Thread` holds an instance of a template that
    // has changed, so a change to `Thread` is a change to `Root`'s schema
    // too: its own text, at the same depth.
    let schema = |src: &str, path: &str| {
        templates(src)
            .into_iter()
            .find(|t| t.path == path)
            .expect(path)
            .schema
    };
    let changed = THREAD.replace("<p>{c.text}</p>", "<p>said: {c.text}</p>");
    assert_ne!(schema(THREAD, "t.Thread"), schema(&changed, "t.Thread"));
    assert_ne!(schema(THREAD, "t.Root"), schema(&changed, "t.Root"));
    // Control: a view no instance of `Root`'s reaches leaves it alone.
    let other = format!("{THREAD}\nview Other(c: Comment) !{{}} {{\n    <p>{{c.text}}</p>\n}}\n");
    let other_changed = other.replace(
        "view Other(c: Comment) !{} {\n    <p>{c.text}</p>",
        "view Other(c: Comment) !{} {\n    <p>other: {c.text}</p>",
    );
    assert_ne!(schema(&other, "t.Other"), schema(&other_changed, "t.Other"));
    assert_eq!(schema(&other, "t.Root"), schema(&other_changed, "t.Root"));
    // Through a view reached through another: `Ask` and `Answer` hold each
    // other, and `Page` uses `Ask` alone.
    let mutual = "module t\n\ntype Q = Q { id: Int, text: String, next: List<Q> }\n\n\
        view Ask(q: Q) !{} {\n    <li>{q.text}<ul>{#each q.next as n (n.id)}<Answer q={n} />{/each}</ul></li>\n}\n\n\
        view Answer(q: Q) !{} {\n    <li>{q.text}<ol>{#each q.next as n (n.id)}<Ask q={n} />{/each}</ol></li>\n}\n\n\
        view Page(q: Q) !{} {\n    <ul><Ask q={q} /></ul>\n}\n";
    assert_eq!(reported(mutual), Vec::<String>::new());
    let answered = mutual.replace("<li>{q.text}<ol>", "<li>answer: {q.text}<ol>");
    assert_ne!(schema(mutual, "t.Page"), schema(&answered, "t.Page"));
}

/// Each page's plan, for a program of `src` with the standard packages.
fn plans(src: &str) -> Vec<pw_core::page_values::Planned> {
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    let mut files = vec![("t.pw".to_string(), src.to_string())];
    for dir in ["packages/pw-std", "packages/pw-platform-web"] {
        let mut paths: Vec<std::path::PathBuf> = std::fs::read_dir(root.join(dir))
            .expect(dir)
            .map(|e| e.expect("entry").path())
            .filter(|p| p.extension().is_some_and(|x| x == "pw"))
            .collect();
        paths.sort();
        for p in paths {
            files.push((
                p.display().to_string(),
                std::fs::read_to_string(&p).expect("read"),
            ));
        }
    }
    let hirs: Vec<pw_core::hir::Hir> = files
        .iter()
        .map(|(_, s)| lower_file(s, &parse_tree(s).green))
        .collect();
    let refs: Vec<&pw_core::hir::Hir> = hirs.iter().collect();
    let ws = pw_core::resolve::Workspace::build(&refs);
    let sigs = pw_core::signatures::Signatures::build(&ws, &refs);
    pw_core::page_values::pages(&refs, &ws, &sigs)
}

#[test]
fn an_instance_is_rendered_again_whole_by_what_gives_it_its_value() {
    // A query's value at the top of the page: the host renders the instance
    // again, whole, as it does a block. A signal's: the browser does.
    let src = THREAD.replace(
        "view Root(c: Comment) !{} {\n    <ul>\n        <Thread c={c} />\n    </ul>\n}\n",
        "public query Talk() -> Comment\n    freshness 30.seconds\n    consistency snapshot\n    cache shared\n    concurrency one_per_key\n    timeout 2.seconds\n{\n    Comment { id: 1, text: \"a\", replies: [] }\n}\n\n\
         page Asked() {\n    cache private\n\n    let talk = query Talk()\n\n    view {\n        <main><ul><Thread c={talk} /></ul></main>\n    }\n}\n\n\
         page Held() {\n    cache private\n\n    signal held: Comment = Comment { id: 1, text: \"a\", replies: [] }\n\n    view {\n        <main><ul><Thread c={held} /></ul></main>\n    }\n}\n",
    );
    assert!(
        check_sources(&[("t.pw".to_string(), src.clone())])
            .iter()
            .all(|(_, ds)| ds.is_empty())
    );
    let all = plans(&src);
    let plan = |page: &str| {
        all.iter()
            .find(|p| p.page == page)
            .expect(page)
            .plan
            .clone()
            .expect("planned")
    };
    let asked = plan("t.Asked");
    assert_eq!(asked.blocks, [0], "{asked:?}");
    assert!(asked.live.is_empty(), "{asked:?}");
    let held = plan("t.Held");
    assert!(held.blocks.is_empty(), "{held:?}");
    assert_eq!(held.live.len(), 1, "{held:?}");
    assert_eq!(
        (held.live[0].part, held.live[0].kind.as_str()),
        (0, "instance")
    );
    assert_eq!(held.live[0].reads, ["held"]);
    // Given a signal and a query's value at once: the browser holds the
    // signal alone, and could not render it again. Refused.
    let mixed = src.replace(
        "page Held() {",
        "view Pair(a: Comment, b: Comment) !{} {\n    <li>{a.text} {b.text}<ul>{#each a.replies as r (r.id)}<Pair a={r} b={b} />{/each}</ul></li>\n}\n\n\
         page Mixed() {\n    cache private\n\n    let talk = query Talk()\n    signal held: Comment = Comment { id: 1, text: \"a\", replies: [] }\n\n    view {\n        <main><ul><Pair a={held} b={talk} /></ul></main>\n    }\n}\n\n\
         page Held() {",
    );
    let refused = plans(&mixed)
        .into_iter()
        .find(|p| p.page == "t.Mixed")
        .expect("Mixed")
        .plan
        .expect_err("refused");
    assert!(
        refused.contains("an instance given the signal `held` and `talk`"),
        "{refused}"
    );
}
