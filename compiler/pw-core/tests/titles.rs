//! **A page states its title** (ADR-0183).
//!
//! Until 2026-10-04 a page could not say what it is: the host titled every
//! store's page "Store", which WCAG 2.4.2 names failure F25, and a demo page
//! by its declaration's name. A page now writes `<title>` at the top of its
//! view, as text and the values it reads, and its host writes it into the
//! document's head.
//!
//! - PW5029: a page served at a route states its title.
//! - PW5030: a title is the page's: once, at the top of its view, written as
//!   text and values.
//! - The template holds it as a part of its own, numbered after every other,
//!   and the page's plan names it, so a host sets it again when what it
//!   reads changes.
//!
//! Each test states one part, with controls that check clean.

use pw_core::check::{Unit, check_sources};
use pw_core::lower::lower_file;
use pw_core::template_ir::{Anchor, Chunk, Part, Template, TitlePiece};
use pw_syntax::parse_tree;

fn reported(src: &str) -> Vec<String> {
    check_sources(&[("t.pw".to_string(), src.to_string())])
        .into_iter()
        .flat_map(|(_, ds)| ds.into_iter().map(|d| format!("{} {}", d.code, d.message)))
        .collect()
}

/// A program with an opaque id, and `decls`.
fn program(decls: &str) -> String {
    format!("module t\n\nopaque type StoreId = String\n\n{decls}")
}

/// A page served at `/stores/{id}` whose view is `view`.
fn routed(view: &str) -> String {
    program(&format!(
        "page P(id: StoreId) {{\n    route \"/stores/{{id}}\"\n    cache private\n\n    \
         view {{\n{view}\n    }}\n}}\n"
    ))
}

/// The store, its `app.pw` changed by `change`, built.
fn build(change: impl Fn(&str) -> String) -> Result<pw_core::build::Build, String> {
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    let mut units = Vec::new();
    let mut add = |path: std::path::PathBuf, change: &dyn Fn(&str) -> String| {
        let src = change(&std::fs::read_to_string(&path).expect("read"));
        units.push(Unit {
            path: path.display().to_string(),
            hir: lower_file(&src, &parse_tree(&src).green),
            src,
        });
    };
    let same = |s: &str| s.to_string();
    for dir in [
        "packages/pw-std",
        "packages/pw-platform-web",
        "examples/lib",
        "examples/store",
    ] {
        let mut paths: Vec<std::path::PathBuf> = std::fs::read_dir(root.join(dir))
            .unwrap_or_else(|e| panic!("{dir}: {e}"))
            .map(|e| e.expect("entry").path())
            .filter(|p| p.extension().is_some_and(|x| x == "pw"))
            .collect();
        paths.sort();
        for p in paths {
            if p.ends_with("store/app.pw") {
                add(p, &change);
            } else {
                add(p, &same);
            }
        }
    }
    add(root.join("examples/domain.pw"), &same);
    pw_core::build::build(&units)
}

/// The store's title, as `app.pw` writes it.
const TITLE: &str = "        <title>{store.name}</title>\n";
/// Its route.
const ROUTE: &str = "    route        \"/stores/{id}\"\n";

fn store_template(b: &pw_core::build::Build) -> &Template {
    b.templates
        .iter()
        .find(|t| t.path == "store.page.StorePage")
        .expect("the store's template")
}

#[test]
fn a_page_s_title_is_a_part_of_the_document_numbered_after_the_rest() {
    let b = build(|s| s.to_string()).expect("builds");
    assert!(b.refusals().is_empty(), "{:?}", b.refusals());
    let t = store_template(&b);
    let titles: Vec<&Part> = t
        .chunks
        .iter()
        .filter_map(|c| match c {
            Chunk::Dynamic(p @ Part::Title { .. }) => Some(p),
            _ => None,
        })
        .collect();
    let [Part::Title { id, pieces }] = titles[..] else {
        panic!("one title at the top of the template: {titles:?}");
    };
    assert_eq!(pieces, &[TitlePiece::Value("store.name".to_string())]);
    // After every part of the body, wherever it is written: the store writes
    // it before `<main>`. Only the page's metadata follows it (ADR-0186).
    let manifest = t.manifest();
    assert_eq!(
        manifest.iter().rfind(|e| e.kind != "meta").map(|e| e.id),
        Some(*id),
        "{manifest:?}"
    );
    assert!(
        manifest
            .iter()
            .filter(|e| e.kind == "meta")
            .all(|e| e.id.0 > id.0),
        "{manifest:?}"
    );
    let entry = manifest.iter().find(|e| e.id == *id).expect("listed");
    assert_eq!(
        (entry.kind, entry.anchor, entry.value.as_str()),
        ("title", Anchor::Document, "store.name")
    );
    // And the page's plan names it, for a host to set again.
    let plan = b
        .pages
        .iter()
        .find(|p| p.page == "store.page.StorePage")
        .expect("the store's plan")
        .plan
        .as_ref()
        .expect("planned");
    assert_eq!(plan.title, Some(id.0));
}

#[test]
fn writing_a_title_moves_no_part_of_the_body() {
    let with = build(|s| s.to_string()).expect("builds");
    // Without it the store states no title, which its route refuses: the
    // control is the store served at no route.
    let without = build(|s| {
        assert_eq!(s.matches(TITLE).count(), 1, "the title's anchor");
        assert_eq!(s.matches(ROUTE).count(), 1, "the route's anchor");
        s.replace(TITLE, "").replace(ROUTE, "")
    })
    .expect("builds");
    // The head's parts aside: the metadata, numbered after the title
    // (ADR-0186), is set again by nothing.
    let others = |b: &pw_core::build::Build| -> Vec<(u32, &'static str, String)> {
        store_template(b)
            .manifest()
            .into_iter()
            .filter(|e| e.kind != "title" && e.kind != "meta")
            .map(|e| (e.id.0, e.kind, e.value))
            .collect()
    };
    assert_eq!(others(&with), others(&without));
    assert_ne!(
        store_template(&with).schema,
        store_template(&without).schema,
        "a title is part of what the template is"
    );
}

#[test]
fn a_page_served_at_a_route_states_its_title() {
    assert_eq!(
        reported(&routed("        <main><h1>Store</h1></main>")),
        ["PW5029 `P` is served at `/stores/{id}` and states no title"]
    );
    // A title that says nothing states none.
    assert_eq!(
        reported(&routed(
            "        <title>  </title>\n        <main><h1>Store</h1></main>"
        )),
        ["PW5029 `P` is served at `/stores/{id}` and states no title"]
    );
    // Controls: text, a value, both; and a page served at no route, which
    // its host names.
    for title in [
        "<title>Store</title>",
        "<title>{id}</title>",
        "<title>Store {id}</title>",
    ] {
        let view = format!("        {title}\n        <main><h1>Store</h1></main>");
        assert_eq!(reported(&routed(&view)), Vec::<String>::new(), "{title}");
    }
    let unrouted = program(
        "page P(id: StoreId) {\n    cache private\n\n    view {\n        \
         <main><h1>Store</h1></main>\n    }\n}\n",
    );
    assert_eq!(reported(&unrouted), Vec::<String>::new());
}

#[test]
fn a_title_is_the_page_s_once_at_the_top_of_its_view() {
    let main = "        <main><h1>Store</h1></main>";
    let title = "        <title>Store</title>";
    for (view, said) in [
        (
            format!("        <main><title>Store</title><h1>Store</h1></main>\n{title}"),
            "`<title>` in `P` is inside an element or a block, not at the top of its view",
        ),
        (
            format!("        {{#if true}}<title>Store</title>{{/if}}\n{title}\n{main}"),
            "`<title>` in `P` is inside an element or a block, not at the top of its view",
        ),
        (
            format!("{title}\n{title}\n{main}"),
            "`P` states a second `<title>`",
        ),
        (
            format!("        <title><b>Store</b></title>\n{main}"),
            "`<title>` in `P` holds `<b>`, and a title is text and values",
        ),
        (
            format!("        <title>{{String.length(\"a\")}}</title>\n{main}"),
            "`<title>` in `P` holds a value computed in place, and a title is text and values",
        ),
    ] {
        let found = reported(&routed(&view));
        assert!(
            found.contains(&format!("PW5030 {said}")),
            "{view}\n{found:#?}"
        );
    }
    // In a view: it would title every page that composes it.
    let in_a_view =
        program("view Heading() !{} {\n    <title>Store</title>\n    <h1>Store</h1>\n}\n");
    assert_eq!(
        reported(&in_a_view),
        ["PW5030 `<title>` in the view `Heading` titles every page that composes it"]
    );
    // Controls: after `<main>` is still the top of the view; and `<Title>` is
    // a view, not a title (ADR-0072).
    assert_eq!(
        reported(&routed(&format!("{main}\n{title}"))),
        Vec::<String>::new()
    );
    let a_view_named_title = program(
        "view Title(id: StoreId) !{} {\n    <h1>{id}</h1>\n}\n\n\
         page P(id: StoreId) {\n    route \"/stores/{id}\"\n    cache private\n\n    view {\n        \
         <title>Store</title>\n        <main><Title id={id} /></main>\n    }\n}\n",
    );
    assert_eq!(reported(&a_view_named_title), Vec::<String>::new());
}

#[test]
fn a_title_reads_no_signal() {
    // A signal is what a person has done on the page, not what the page is.
    let src = program(
        "page P(id: StoreId) {\n    route \"/stores/{id}\"\n    cache private\n\n    \
         signal greeting: String = \"Hello\"\n\n    view {\n        \
         <title>{greeting}</title>\n        <main><h1>{greeting}</h1>\n        \
         <button type=\"button\" on:press={() => greeting = \"Hi\"}>Greet</button></main>\n    }\n}\n",
    );
    assert_eq!(reported(&src), Vec::<String>::new(), "it checks");
    let units = vec![Unit {
        path: "t.pw".to_string(),
        hir: lower_file(&src, &parse_tree(&src).green),
        src: src.clone(),
    }];
    // Refused where the page's parts are planned (ADR-0137).
    let b = pw_core::build::build(&units).expect("builds");
    let why = b
        .pages
        .iter()
        .find(|p| p.page == "t.P")
        .expect("a plan")
        .plan
        .as_ref()
        .expect_err("refused");
    assert!(
        why.contains("is a title part the signal `greeting` decides"),
        "{why}"
    );
}

#[test]
fn a_title_reads_no_value_a_press_speculates() {
    // The cart is speculated by the store's Add (ADR-0172); the browser
    // renders no title again from a speculation.
    let b = build(|s| {
        assert_eq!(s.matches(TITLE).count(), 1, "the title's anchor");
        s.replace(
            TITLE,
            "        <title>{cart.line_count} in the cart</title>\n",
        )
    })
    .expect("builds");
    let refused = b.refusals().join("\n");
    assert!(
        refused.contains("a title that reads a speculated value")
            && refused.contains("reads `cart.line_count`"),
        "{refused}"
    );
    // Control: the store's own title reads the store, which no press changes.
    assert!(
        build(|s| s.to_string())
            .expect("builds")
            .refusals()
            .is_empty()
    );
}
