//! **A computed condition decides its block** (ADR-0229, ruling 0073-a).
//!
//! `{#if List.length(thread.replies) == 0}` and `{#if String.length(draft) >
//! 280}` were refused at build until ADR-0229: "an `{#if}` condition must be
//! a value path". A block's subject is read by a path the compiler names now,
//! `#feed.app.PostPage~3`, computed as a text part's value is: by a host from
//! a query's value, which renders the block and renders it again when the
//! value changes; by the browser from a signal's, which renders it again as
//! the signal changes. A value computed inside a block a host renders is the
//! host's too.
//!
//! Each test states one part, with controls.

use pw_core::check::{Unit, check_sources};
use pw_core::lower::lower_file;
use pw_core::page_values::Step;
use pw_syntax::parse_tree;

/// The platform's packages, and `files` after them.
fn program(files: &[(&str, String)]) -> Vec<(String, String)> {
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    let mut out = Vec::new();
    for d in ["packages/pw-std", "packages/pw-platform-web"] {
        let mut paths: Vec<std::path::PathBuf> = std::fs::read_dir(root.join(d))
            .unwrap_or_else(|e| panic!("{d}: {e}"))
            .map(|e| e.expect("entry").path())
            .filter(|p| p.extension().is_some_and(|x| x == "pw"))
            .collect();
        paths.sort();
        for p in paths {
            out.push((
                p.display().to_string(),
                std::fs::read_to_string(&p).expect("read"),
            ));
        }
    }
    out.extend(files.iter().map(|(n, s)| (n.to_string(), s.clone())));
    out
}

fn reported(files: &[(String, String)]) -> Vec<String> {
    check_sources(files)
        .into_iter()
        .filter(|(n, _)| !n.contains("packages/"))
        .flat_map(|(_, ds)| ds.into_iter().map(|d| format!("{} {}", d.code, d.message)))
        .collect()
}

/// The build, or what refused it.
fn build(files: &[(String, String)]) -> Result<pw_core::build::Build, String> {
    let units: Vec<Unit> = files
        .iter()
        .map(|(path, src)| Unit {
            path: path.clone(),
            hir: lower_file(src, &parse_tree(src).green),
            src: src.clone(),
        })
        .collect();
    let b = pw_core::build::build(&units)?;
    match b.refusals() {
        refused if refused.is_empty() => Ok(b),
        refused => Err(refused.join("\n")),
    }
}

fn feed(change: &dyn Fn(&str) -> String) -> Vec<(String, String)> {
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    let src = std::fs::read_to_string(root.join("examples/feed/app.pw")).expect("the feed");
    program(&[("app.pw", change(&src))])
}

/// A page of a post and its UI state, its markup `markup`.
fn page(markup: &str) -> Vec<(String, String)> {
    program(&[(
        "t.pw",
        format!(
            "module t\n\nimport List\n\n\
             type Post = Post {{ id: String, likes: Int, pinned: Bool, tags: List<String> }}\n\n\
             fn shown(open: Bool) -> String !{{}} {{\n    if open {{\n        \"open\"\n    }} else {{\n        \"shut\"\n    }}\n}}\n\n\
             view Counter() !{{}} {{\n    signal n: Int = 0\n    \
             <button type=\"button\" on:press={{() => n = n + 1}}>{{n}}</button>\n}}\n\n\
             public query Thread(id: String) -> Post !{{}} {{\n    \
             Post {{ id: id, likes: 2, pinned: true, tags: [] }}\n}}\n\n\
             page P(id: String) {{\n    route \"/p/{{id}}\"\n    cache private\n\n    \
             signal open: Bool = false\n    signal count: Int = 0\n    let post = query Thread(id)\n\n    \
             view {{\n        <title>P</title>\n        <main>\n            {markup}\n            \
             <button type=\"button\" on:press={{() => open = !open}}>Toggle</button>\n            \
             <button type=\"button\" on:press={{() => count = count + 1}}>More</button>\n        \
             </main>\n    }}\n}}\n"
        ),
    )])
}

fn plan_of<'b>(b: &'b pw_core::build::Build, page: &str) -> &'b pw_core::page_values::PageValues {
    b.pages
        .iter()
        .find(|p| p.page == page)
        .unwrap_or_else(|| panic!("no page {page}"))
        .plan
        .as_ref()
        .unwrap_or_else(|e| panic!("{page}: {e}"))
}

#[test]
fn a_computed_condition_is_a_hosts_from_a_query_and_the_browsers_from_a_signal() {
    let files = feed(&|s| s.to_string());
    assert_eq!(reported(&files), Vec::<String>::new());
    let b = build(&files).unwrap_or_else(|e| panic!("{e}"));
    // The thread page's: a block a host renders, and again, its subject
    // computed into what it is rendered with.
    let thread = plan_of(&b, "feed.app.PostPage");
    let subject = thread
        .derived
        .iter()
        .find(|d| d.path.starts_with("#feed.app.PostPage~"))
        .expect("the condition, the host's");
    assert!(thread.blocks.contains(&subject.part), "{:?}", thread.blocks);
    assert_eq!(subject.binding, "thread");
    assert!(
        matches!(subject.steps.as_slice(), [Step::Derived(_)]),
        "{:?}",
        subject.steps
    );
    // The home page's: a block the browser renders again as the draft
    // changes, by its component, and the module's function.
    let home = plan_of(&b, "feed.app.Home");
    let block = home
        .live
        .iter()
        .find(|l| l.kind == "conditional" && !l.derived.is_empty())
        .expect("the condition, the browser's");
    assert_eq!(
        (block.signal.as_str(), block.path.as_str()),
        ("draft", "draft")
    );
    assert!(
        block.reads.contains(&"draft".to_string()),
        "{:?}",
        block.reads
    );
    let module = b
        .computed
        .iter()
        .find(|c| c.page == "feed.app.Home")
        .and_then(|c| c.module.as_ref().ok())
        .expect("the home page's module");
    assert!(
        module.contains(&format!("  \"{}\": (j) =>", block.part)),
        "{module}"
    );
}

#[test]
fn an_else_if_a_match_and_a_value_in_a_hosts_block_are_computed_too() {
    let files = page(
        "{#if post.pinned}<p>pinned</p>{:else if post.likes > 1}<p>liked</p>{/if}\n            \
         {#match List.get(post.tags, 0)}{:Some(t)}<p>{t}</p>{:None}<p>none</p>{/match}\n            \
         {#if post.pinned}<p>{shown(post.likes > 1)}</p>{/if}",
    );
    assert_eq!(reported(&files), Vec::<String>::new());
    let b = build(&files).unwrap_or_else(|e| panic!("{e}"));
    let plan = plan_of(&b, "t.P");
    // The `{:else if}`'s, the `{#match}`'s and the value in the third block:
    // each the host's, in what the page is rendered with.
    let computed: Vec<u32> = plan.derived.iter().map(|d| d.part).collect();
    assert_eq!(computed.len(), 3, "{:?}", plan.derived);
    // The `{#match}` is a block of its own the host renders again.
    assert!(
        computed.iter().any(|p| plan.blocks.contains(p)),
        "{:?} {:?}",
        plan.derived,
        plan.blocks
    );
    // And none is a text part sent alone: the block is rendered again whole.
    assert!(
        plan.parts.iter().all(|p| !p.path.starts_with('#')),
        "{:?}",
        plan.parts
    );
}

#[test]
fn what_a_condition_does_not_compute_yet_is_refused_by_name() {
    for (files, why) in [
        // A value from a signal inside a block: the browser computes one at
        // the top of the page.
        (
            page("{#if post.pinned}<p>{shown(open)}</p>{/if}"),
            "computes a value from the signal `open` inside a block, and the browser computes \
             one at the top of the page (ruling 0073-a)",
        ),
        // A value from a query inside a block a signal decides, which the
        // browser renders again from the signals alone.
        (
            page("{#if open}<p>{shown(post.pinned)}</p>{/if}"),
            "computes a value inside a block a signal decides, and the browser, which renders \
             that block again, computes none yet (ruling 0073-a)",
        ),
        // A block the browser decides by a computed subject, which reads a
        // query's value inside: it renders it again from the signals alone.
        (
            page("{#if count > 2}<p>{post.likes}</p>{/if}"),
            "reads `post.likes` inside a block a signal decides, and the browser renders that \
             block again from the page's signals alone (ADR-0137)",
        ),
        // A block the browser decides by a computed subject, whose arm holds
        // a view's signals.
        (
            page("{#if count > 2}<Counter />{/if}"),
            "is a block whose subject the browser computes, and whose arms hold a view's \
             signals, which the browser starts again by its arm (ruling 0073-a)",
        ),
    ] {
        assert_eq!(reported(&files), Vec::<String>::new(), "{why}");
        match build(&files) {
            Ok(_) => panic!("built: {why}"),
            Err(e) => assert!(e.contains(why), "{e}"),
        }
    }
    // Controls: a block the browser decides by a computed subject, holding
    // no view's signals; and one a query decides, holding a value the host
    // computes.
    for markup in [
        "{#if count > 2}<p>many</p>{:else}<p>few</p>{/if}",
        "{#if post.likes > 1}<p>{shown(post.pinned)}</p>{/if}",
    ] {
        build(&page(markup)).unwrap_or_else(|e| panic!("{markup}: {e}"));
    }
}
