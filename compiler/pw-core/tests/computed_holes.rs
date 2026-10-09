//! **A value the template computes compiles** (ADR-0226, ruling 0073-a).
//!
//! A hole that is no path, `{counted(List.length(thread.replies), ..)}`, or
//! an attribute's whole value, `disabled={List.length(post.replies) == 0}`,
//! was refused at build until ADR-0226: "a template hole is read by path".
//! The compiler lifts each into a function of the one query value it reads,
//! a component of its own, and the template reads it by a path the compiler
//! names, `#{template}~{part}`. A host computes it when the page renders and
//! again when the value changes. It performs nothing (PW0334, revision 2).
//!
//! Each test states one part, with controls.

use pw_core::backend::component::Built;
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

/// The feed reference app, its `app.pw` changed by `change`.
fn feed(change: &dyn Fn(&str) -> String) -> Vec<(String, String)> {
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    let src = std::fs::read_to_string(root.join("examples/feed/app.pw")).expect("the feed");
    program(&[("app.pw", change(&src))])
}

/// `code message` for each diagnostic on the files after the packages.
fn reported(files: &[(String, String)]) -> Vec<String> {
    let own: Vec<String> = files
        .iter()
        .filter(|(n, _)| !n.contains("packages/"))
        .map(|(n, _)| n.clone())
        .collect();
    check_sources(files)
        .into_iter()
        .filter(|(n, _)| own.contains(n))
        .flat_map(|(_, ds)| ds.into_iter().map(|d| format!("{} {}", d.code, d.message)))
        .collect()
}

fn build(files: &[(String, String)]) -> pw_core::build::Build {
    let units: Vec<Unit> = files
        .iter()
        .map(|(path, src)| Unit {
            path: path.clone(),
            hir: lower_file(src, &parse_tree(src).green),
            src: src.clone(),
        })
        .collect();
    pw_core::build::build(&units).expect("builds")
}

/// What a build refuses: a template part the renderer cannot render, or
/// each refusal of a build that completed.
fn refused(files: &[(String, String)]) -> String {
    let units: Vec<Unit> = files
        .iter()
        .map(|(path, src)| Unit {
            path: path.clone(),
            hir: lower_file(src, &parse_tree(src).green),
            src: src.clone(),
        })
        .collect();
    match pw_core::build::build(&units) {
        Ok(b) => b.refusals().join("\n"),
        Err(e) => e,
    }
}

/// A page that shows a post, its markup `markup`.
fn page(markup: &str) -> Vec<(String, String)> {
    program(&[(
        "t.pw",
        format!(
            "module t\n\nimport List\nimport clock\nimport html.{{ raw_html }}\n\n\
             type Reply = Reply {{ id: String, text: String }}\n\n\
             type Author = Author {{ name: String }}\n\n\
             type Post = Post {{ id: String, text: String, likes: Int, pinned: Bool, author: \
             Author, replies: List<Reply> }}\n\n\
             fn counted(n: Int, one: String, many: String) -> String !{{}} {{\n    \
             if n == 1 {{\n        \"1 {{one}}\"\n    }} else {{\n        \"{{n}} {{many}}\"\n    }}\n}}\n\n\
             fn shown(open: Bool) -> String !{{}} {{\n    if open {{\n        \"open\"\n    }} else {{\n        \"shut\"\n    }}\n}}\n\n\
             fn stamp(n: Int) -> Int !{{ clock.read }} {{\n    clock.now() + n\n}}\n\n\
             fn apply(f: fn(Int) -> Int) -> Int !{{}} {{\n    f(1)\n}}\n\n\
             fn rows() -> List<Post> !{{ database.read<Post> }}\n    host \"t:data/posts#rows\"\n\n\
             public query Thread(id: String) -> Post !{{}} {{\n    \
             Post {{ id: id, text: \"Hello.\", likes: 2, pinned: true, author: Author {{ name: \
             \"Ada\" }}, replies: [] }}\n}}\n\n\
             public query Other() -> Post !{{}} {{\n    \
             Post {{ id: \"o\", text: \"Other.\", likes: 1, pinned: false, author: Author {{ name: \
             \"Grace\" }}, replies: [] }}\n}}\n\n\
             view Counted(p: Post) !{{}} {{\n    <p>{{counted(p.likes, \"like\", \"likes\")}}</p>\n}}\n\n\
             view Named(a: Author) !{{}} {{\n    <p>{{shown(a.name == \"Ada\")}}</p>\n}}\n\n\
             page P(id: String) {{\n    route \"/p/{{id}}\"\n    cache private\n\n    \
             signal open: Bool = false\n    let post = query Thread(id)\n    let other = query Other()\n\n    \
             view {{\n        <title>P</title>\n        <main>\n            {markup}\n            \
             <button type=\"button\" on:press={{() => open = !open}}>Toggle</button>\n        \
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
fn a_value_the_template_computes_is_lifted_and_planned() {
    let files = feed(&|s| s.to_string());
    assert_eq!(reported(&files), Vec::<String>::new());
    let b = build(&files);
    assert!(b.refusals().is_empty(), "{:?}", b.refusals());
    // The thread page's two counts: each read by the path the compiler
    // names, from the thread, by the function lifted out of the template.
    let plan = plan_of(&b, "feed.app.PostPage");
    for part in [1u32, 2] {
        let found = plan
            .parts
            .iter()
            .find(|p| p.part == part)
            .unwrap_or_else(|| panic!("part {part}: {:?}", plan.parts));
        assert_eq!(found.path, format!("#feed.app.PostPage~{part}"));
        assert_eq!(found.binding, "thread");
        assert_eq!(
            found.steps,
            [Step::Derived(format!("feed.app.PostPage.derived_{part}"))]
        );
        // A component of its own: it imports nothing, and needs nothing.
        let id = format!("feed.app.PostPage.derived_{part}");
        let contract = b
            .contracts
            .iter()
            .find(|c| c.component_id == id)
            .unwrap_or_else(|| panic!("no contract {id}"));
        assert_eq!(contract.exports.len(), 1);
        assert_eq!(contract.exports[0].kind, "derived");
        assert!(contract.required_capabilities.is_empty(), "{contract:?}");
        assert!(contract.imports.is_empty(), "{contract:?}");
        match b.components.iter().find(|(c, _)| *c == id) {
            Some((_, Built::Component { compiled, .. })) => {
                assert!(compiled.contract.imports.is_empty())
            }
            other => panic!("{id}: {other:?}"),
        }
        // What it takes and answers, as the WIT says.
        assert!(
            b.wit.contains(&format!(
                "derived{part}: func(arg0: feed-app-post) -> string;"
            )),
            "{}",
            b.wit
        );
    }
    // The template reads each by that path.
    let template = b
        .templates
        .iter()
        .find(|t| t.path == "feed.app.PostPage")
        .expect("the thread page's template");
    let chunks = format!("{:?}", template.chunks);
    assert!(chunks.contains("\"#feed.app.PostPage~1\""), "{chunks}");
    assert!(chunks.contains("\"#feed.app.PostPage~2\""), "{chunks}");
    // Control: the home page's host computes nothing. What it computes is
    // from its draft, a signal, which the browser computes (ADR-0227).
    let home = plan_of(&b, "feed.app.Home");
    assert!(
        home.parts
            .iter()
            .chain(&home.derived)
            .all(|p| !p.steps.iter().any(|s| matches!(s, Step::Derived(_)))),
        "the home page's host computes nothing: {:?}",
        home.parts
    );
}

#[test]
fn a_computed_attribute_is_a_value_the_host_sets() {
    let files = page(
        "<p class={counted(post.likes, \"like\", \"likes\")}>{post.text}</p>\n            \
         <button type=\"button\" disabled={List.length(post.replies) == 0}>Replies</button>",
    );
    assert_eq!(reported(&files), Vec::<String>::new());
    let b = build(&files);
    assert!(b.refusals().is_empty(), "{:?}", b.refusals());
    let plan = plan_of(&b, "t.P");
    assert_eq!(plan.derived.len(), 2, "{:?}", plan.derived);
    for d in &plan.derived {
        assert_eq!(d.path, format!("#t.P~{}", d.part));
        assert_eq!(d.binding, "post");
        assert_eq!(d.steps, [Step::Derived(format!("t.P.derived_{}", d.part))]);
        // Set again as any attribute that reads a query's value is.
        assert!(plan.attributes.contains(&d.part), "{:?}", plan.attributes);
    }
    // The class is text; the boolean attribute is a `Bool`.
    let [class, disabled] = [plan.derived[0].part, plan.derived[1].part];
    assert!(
        b.wit
            .contains(&format!("derived{class}: func(arg0: t-post) -> string;")),
        "{}",
        b.wit
    );
    assert!(
        b.wit
            .contains(&format!("derived{disabled}: func(arg0: t-post) -> bool;")),
        "{}",
        b.wit
    );
    // Control: the same attributes read by path compute nothing.
    let b = build(&page(
        "<p class={post.text}>{post.text}</p>\n            \
         <button type=\"button\" disabled={open}>Replies</button>",
    ));
    assert!(b.refusals().is_empty(), "{:?}", b.refusals());
    assert!(plan_of(&b, "t.P").derived.is_empty());
}

/// **A view composed in the page computes as the page does** (ADR-0136): its
/// parameter is the page's value, given whole or a field of it.
#[test]
fn a_view_composed_in_the_page_computes_from_the_pages_value() {
    let files = page("<Counted p={post} />\n            <Named a={post.author} />");
    assert_eq!(reported(&files), Vec::<String>::new());
    let b = build(&files);
    assert!(b.refusals().is_empty(), "{:?}", b.refusals());
    let plan = plan_of(&b, "t.P");
    let steps: Vec<(&str, &[Step])> = plan
        .parts
        .iter()
        .map(|p| (p.binding.as_str(), p.steps.as_slice()))
        .collect();
    let derived = |n: u32| Step::Derived(format!("t.P.derived_{n}"));
    let [first, second] = [plan.parts[0].part, plan.parts[1].part];
    assert_eq!(
        steps,
        [
            ("post", &[derived(first)][..]),
            (
                "post",
                &[Step::Field("author".to_string()), derived(second)][..]
            ),
        ]
    );
    // Each function takes what its view names: a `Post`, and an `Author`.
    for (n, input) in [(first, "t-post"), (second, "t-author")] {
        assert!(
            b.wit
                .contains(&format!("derived{n}: func(arg0: {input}) -> string;")),
            "{}",
            b.wit
        );
    }
}

#[test]
fn what_a_host_does_not_compute_is_refused_by_name() {
    for (markup, why) in [
        (
            "{#if open}<p>{counted(post.likes, \"a\", \"b\")}</p>{/if}",
            "computes a value inside a block a signal decides, and the browser, which renders \
             that block again, computes none yet (ruling 0073-a)",
        ),
        (
            "<ul>{#each post.replies as r (r.id)}<li>{shown(r.id == post.id)}</li>{/each}</ul>",
            "computes a value from `r` and `post`, and a host computes one from one value \
             (ruling 0073-a)",
        ),
        (
            "<p>{counted(post.likes + other.likes, \"a\", \"b\")}</p>",
            "computes a value from `post` and `other`, and a host computes one from one value \
             (ruling 0073-a)",
        ),
        (
            "<p>{counted(1, \"a\", \"b\")}</p>",
            "computes a value from nothing it reads",
        ),
        (
            "<p>{shown(id == \"p1\")}</p>",
            "computes a value from the page's parameter `id`, and a host computes one from a \
             query's value alone (ruling 0073-a)",
        ),
        (
            "<a href=\"/p/{shown(post.pinned)}\">x</a>",
            "a hole in an attribute's text is a value path, and a computed value is written \
             as the attribute's whole value",
        ),
    ] {
        let files = page(markup);
        assert_eq!(reported(&files), Vec::<String>::new(), "{markup}");
        let refused = refused(&files);
        assert!(refused.contains(why), "{markup}: {refused}");
    }
    // Controls: one query's value, at the top of the page, builds; and with
    // a lambda in it, whose names are its own, not values it reads.
    for markup in [
        "<p>{counted(post.likes, \"a\", \"b\")}</p>",
        "<p>{counted(List.fold(post.replies, 0, (n, r) => n + 1), \"a\", \"b\")}</p>",
    ] {
        let b = build(&page(markup));
        assert!(b.refusals().is_empty(), "{markup}: {:?}", b.refusals());
        assert_eq!(plan_of(&b, "t.P").parts.len(), 1, "{markup}");
    }
    // And inside a block a host renders, since ADR-0229: computed into what
    // the block is rendered with.
    let b = build(&page(
        "{#if post.pinned}<p>{counted(post.likes, \"a\", \"b\")}</p>{/if}",
    ));
    assert!(b.refusals().is_empty(), "{:?}", b.refusals());
    assert_eq!(plan_of(&b, "t.P").derived.len(), 1);
}

#[test]
fn a_view_that_contains_itself_computes_nothing_yet_and_a_speculated_value_is_computed() {
    // A view that contains itself is an instance made at run time, whose
    // template no plan computes for.
    let files = feed(&|s| {
        s.replace(
            "        <p>{post.text}</p>\n        <ul>",
            "        <p>{post.text}</p>\n        <p>{List.length(post.replies)}</p>\n        <ul>",
        )
    });
    assert_eq!(reported(&files), Vec::<String>::new());
    let refusals = refused(&files);
    assert!(
        refusals.contains(
            "`<Replies>` contains itself and computes a value in its template, and a view \
             that contains itself computes none yet (ruling 0073-a)"
        ),
        "{refusals}"
    );
    // A value the page speculates on, whole (ADR-0228): computed again from
    // the speculation, so the count moves with the list it counts. Until
    // ADR-0228 it was refused: the browser would have shown the list with
    // the post, and the host's count without it.
    let files = feed(&|s| {
        s.replace(
            "            <h1>Home</h1>\n",
            "            <h1>Home</h1>\n            <p>{List.length(feed)} posts</p>\n",
        )
    });
    assert_eq!(reported(&files), Vec::<String>::new());
    let b = build(&files);
    assert!(b.refusals().is_empty(), "{:?}", b.refusals());
    let home = b
        .speculations
        .iter()
        .find(|s| s.page == "feed.app.Home")
        .expect("the home page speculates");
    let pw_core::backend::wasm::Encoding::Encoded(module) = &home.module else {
        panic!("{:?}", home.module);
    };
    let part = plan_of(&b, "feed.app.Home")
        .parts
        .iter()
        .find(|p| p.path.starts_with('#'))
        .expect("the count")
        .part;
    assert!(
        module.source.contains(&format!("\"{part}\": f")),
        "{}",
        module.source
    );
    // Control: the thread page, which speculates on nothing, computes.
    assert!(build(&feed(&|s| s.to_string())).refusals().is_empty());
}

#[test]
fn a_value_the_template_computes_performs_nothing() {
    // In a hole, an attribute, and through a function named as a value:
    // the stronger statement, not "undeclared" (PW0400).
    for (markup, effect) in [
        (
            "<p>{counted(clock.now(), \"a\", \"b\")}</p>",
            "`clock.read`",
        ),
        (
            "<p title={counted(clock.now(), \"a\", \"b\")}>x</p>",
            "`clock.read`",
        ),
        (
            "<p>{counted(apply(stamp), \"a\", \"b\")}</p>",
            "`clock.read`",
        ),
    ] {
        let found = reported(&page(markup));
        assert_eq!(
            found,
            [format!(
                "PW0334 a value `P`'s template computes performs {effect}"
            )],
            "{markup}"
        );
    }
    // An effect a page may not perform at all is said once, as that.
    let found = reported(&page("<p>{counted(List.length(rows()), \"a\", \"b\")}</p>"));
    assert_eq!(found.len(), 1, "{found:#?}");
    assert!(found[0].starts_with("PW0401"), "{found:#?}");
    // An escape hatch's is its audit record's to decide.
    let found = reported(&page("<p>{raw_html(post.text)}</p>"));
    assert_eq!(
        found,
        ["PW5010 `raw_html` requires the `unsafe.raw_html` capability and a justification"]
    );
    // Controls: the clock read by a handler, and by the page's body as it
    // renders, shown by its path; a computed value that performs nothing.
    for markup in [
        "<button type=\"button\" on:press={() => open = clock.now() > 0}>Now</button>",
        "<p>{counted(post.likes, \"a\", \"b\")}</p>",
    ] {
        assert_eq!(reported(&page(markup)), Vec::<String>::new(), "{markup}");
    }
}

#[test]
fn a_build_input_a_template_reads_is_no_hosts() {
    // A page placed at build may read its build's input as it renders, and
    // the checker says nothing (A-013). A host would run `include_markdown`
    // as compiled code, and answer its stub's "": the build refuses it.
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    let src = std::fs::read_to_string(
        root.join("examples/accepted/A-013-build-time-deterministic-page.pw"),
    )
    .expect("A-013");
    let files = program(&[("A-013.pw", src)]);
    assert_eq!(reported(&files), Vec::<String>::new());
    let refusals = refused(&files);
    assert!(
        refusals.contains(
            "computes a value that performs `build.input.read<WorkspaceFile>`, and a host \
             computes only a value that performs nothing"
        ),
        "{refusals}"
    );
}

#[test]
fn a_member_an_imported_module_lacks_does_not_resolve() {
    // Until ADR-0226 the qualified call check took an import's name for a
    // local, so `String.nope(s)` checked clean wherever `String` was
    // imported: A-032's first draft called `String.concat`, which no module
    // declares.
    let module = |body: &str| {
        program(&[(
            "t.pw",
            format!(
                "module t\n\nimport String\n\nfn f(s: String) -> Int !{{}} {{\n    {body}\n}}\n"
            ),
        )])
    };
    assert_eq!(
        reported(&module("String.nope(s)")),
        ["PW0021 `String.nope` does not resolve"]
    );
    assert_eq!(reported(&module("String.length(s)")), Vec::<String>::new());
    // In a template too.
    assert_eq!(
        reported(&page("<p>{String.nope(post.text)}</p>")),
        ["PW0021 `String.nope` does not resolve"]
    );
    // An optimistic clause still reads a module by its name: the store's
    // `Carts.with_line(cart, ..)` checks, through `Carts` alone.
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    let mut files = program(&[]);
    for d in ["examples/lib", "examples/store"] {
        let mut paths: Vec<std::path::PathBuf> = std::fs::read_dir(root.join(d))
            .expect(d)
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
    files.push((
        "domain.pw".to_string(),
        std::fs::read_to_string(root.join("examples/domain.pw")).expect("domain"),
    ));
    let store = |change: &dyn Fn(&str) -> String| -> Vec<String> {
        let files: Vec<(String, String)> = files
            .iter()
            .map(|(n, s)| {
                if n.ends_with("store/app.pw") {
                    (n.clone(), change(s))
                } else {
                    (n.clone(), s.clone())
                }
            })
            .collect();
        check_sources(&files)
            .into_iter()
            .flat_map(|(_, ds)| ds.into_iter().map(|d| format!("{} {}", d.code, d.message)))
            .collect()
    };
    assert_eq!(store(&|s| s.to_string()), Vec::<String>::new());
    assert_eq!(
        // A user's cart's transitions since track `store-accounts`.
        store(&|s| s.replacen(
            "UserCarts.with_line(cart, item, quantity)",
            "UserCarts.nope(cart)",
            1
        )),
        ["PW0021 `UserCarts.nope` does not resolve"]
    );
}
