//! **A page's parameter is rendered on every page** (ADR-0231).
//!
//! A page that binds a query was rendered without its parameters. A title,
//! an attribute or a handler's captures that read one built, then every
//! request for the page was answered 503, and a text part that read one was
//! refused at build: "part 1 reads `id`, which no query binds". Each builds
//! now, and none is a part a host sets again: a parameter is the document's
//! for its life. What the browser renders again from its own values reads
//! none, and a value computed from one is a later step of ruling 0073-a.
//!
//! Each test states one part, with controls.

use pw_core::check::{Unit, check_sources};
use pw_core::lower::lower_file;
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

/// The feed, its thread page's markup given `markup` after its replies.
fn thread_page(markup: &str) -> Vec<(String, String)> {
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    let src = std::fs::read_to_string(root.join("examples/feed/app.pw")).expect("the feed");
    let at = "            <Replies post={thread} />\n";
    assert_eq!(src.matches(at).count(), 1, "the thread page's replies");
    program(&[(
        "app.pw",
        src.replace(at, &format!("{at}            {markup}\n")),
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
fn a_parameter_is_read_where_a_value_is_and_set_again_nowhere() {
    // A text part, an attribute, a link's text and a block a host renders
    // read the thread's `id`, and the reply form's handler captures it.
    let files = thread_page(
        "<p id=\"which\" title=\"{id}\"><a href=\"/post/{id}\">{id}</a></p>\n            \
         {#if thread.likes > 2}<p>{id} is popular</p>{/if}",
    );
    assert_eq!(reported(&files), Vec::<String>::new());
    let b = build(&files).unwrap_or_else(|e| panic!("{e}"));
    let plan = plan_of(&b, "feed.app.PostPage");
    assert_eq!(plan.params, ["id"]);
    // A host sets nothing again for it: every part it sets again reads the
    // thread, no attribute and no handler's captures are among them.
    assert!(
        plan.parts.iter().all(|p| p.binding == "thread"),
        "{:?}",
        plan.parts
    );
    assert_eq!(plan.attributes, Vec::<u32>::new());
    assert_eq!(plan.captures, Vec::<u32>::new());
    // The page's title reads it too.
    let titled = thread_page("").into_iter().map(|(n, s)| {
        let s = s.replace("<title>Post</title>", "<title>Post {id}</title>");
        (n, s)
    });
    build(&titled.collect::<Vec<_>>()).unwrap_or_else(|e| panic!("the title: {e}"));
}

#[test]
fn what_reads_a_parameter_where_a_host_does_not_render_is_refused_by_name() {
    for (markup, why) in [
        // Inside a block a signal decides, which the browser renders again
        // from the page's signals alone.
        (
            "{#if answer}<p>{id}</p>{/if}",
            "reads `id` inside a block a signal decides, and the browser renders that block \
             again from the page's signals alone (ADR-0137)",
        ),
        // A value computed from it, which a later step of ruling 0073-a
        // computes.
        (
            "<p>{String.length(id.value)}</p>",
            "computes a value from the page's parameter `id`, and a host computes one from a \
             query's value alone (ruling 0073-a)",
        ),
    ] {
        let files = thread_page(markup);
        assert_eq!(reported(&files), Vec::<String>::new(), "{markup}");
        match build(&files) {
            Ok(_) => panic!("built: {markup}"),
            Err(e) => assert!(e.contains(why), "{markup}: {e}"),
        }
    }
    // The control: the same parameter in a block a host renders.
    build(&thread_page(
        "{#if List.length(thread.replies) > 1}<p>{id}</p>{/if}",
    ))
    .unwrap_or_else(|e| panic!("{e}"));
}
