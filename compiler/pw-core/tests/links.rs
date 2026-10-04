//! **A `<link>` is written where HTML allows it** (ADR-0189).
//!
//! Markup is written into the document's body, and HTML allows a `<link>`
//! there only when each of its relations is body-ok, or when it is an item's
//! property. Until 2026-10-04 `<link rel="canonical">` and `rel="icon"`
//! checked and built, and were written into the body, where nothing reads
//! them.
//!
//! - PW5035: a `<link>` in markup is one HTML allows in the body.
//!
//! Each test states one part, with controls that check clean.

use pw_core::check::{Unit, check_sources};
use pw_core::lower::lower_file;
use pw_core::template_ir::Chunk;
use pw_syntax::parse_tree;

fn reported(src: &str) -> Vec<String> {
    check_sources(&[("t.pw".to_string(), src.to_string())])
        .into_iter()
        .flat_map(|(_, ds)| ds.into_iter().map(|d| format!("{} {}", d.code, d.message)))
        .collect()
}

/// A page served at `/stores/{id}`, whose view holds `markup` before its
/// `<main>`.
fn page(markup: &str) -> String {
    format!(
        "module t\n\nopaque type StoreId = String\n\n\
         page P(id: StoreId) {{\n    route \"/stores/{{id}}\"\n    cache private\n\n    \
         view {{\n        <title>Store</title>\n        {markup}\n        \
         <main><h1>Store</h1></main>\n    }}\n}}\n"
    )
}

#[test]
fn a_link_the_head_holds_is_refused_in_markup() {
    for (markup, said) in [
        (
            "<link rel=\"canonical\" href=\"/stores/{id}\" />",
            "PW5035 `<link rel=\"canonical\">` in `P` is written into the body, where HTML does not allow `canonical` and nothing reads it: the document's head holds the page's title and metadata, and the host's links",
        ),
        (
            "<link rel=\"icon\" href=\"/favicon.ico\" />",
            "PW5035 `<link rel=\"icon\">` in `P` is written into the body, where HTML does not allow `icon` and nothing reads it: the document's head holds the page's title and metadata, and the host's links",
        ),
        // One relation that is not body-ok among ones that are.
        (
            "<link rel=\"preload alternate\" href=\"/a.css\" />",
            "PW5035 `<link rel=\"preload alternate\">` in `P` is written into the body, where HTML does not allow `alternate` and nothing reads it: the document's head holds the page's title and metadata, and the host's links",
        ),
        (
            "<link href=\"/a.css\" />",
            "PW5035 `<link>` in `P` names no relation and no item's property",
        ),
        (
            "<link rel=\" \" href=\"/a.css\" />",
            "PW5035 `<link>` in `P` names no relation",
        ),
        (
            "<link rel=\"stylesheet\" itemprop=\"url\" href=\"/a.css\" />",
            "PW5035 `<link>` in `P` is a relation and an item's property at once: HTML allows one of `rel` and `itemprop`",
        ),
        (
            "<link rel={id} href=\"/a.css\" />",
            "PW5035 `<link rel>` in `P` is a value computed in place, and which relation it names decides whether HTML allows it in the body",
        ),
    ] {
        let found = reported(&page(markup));
        assert_eq!(found, [said.to_string()], "{markup}");
    }
    // In a view as in a page: a view's markup is its page's body.
    assert_eq!(
        reported(
            "module t\n\nview Head() !{} {\n    <link rel=\"manifest\" href=\"/app.webmanifest\" />\n    <h2>Store</h2>\n}\n"
        ),
        [
            "PW5035 `<link rel=\"manifest\">` in `Head` is written into the body, where HTML does not allow `manifest` and nothing reads it: the document's head holds the page's title and metadata, and the host's links"
        ]
    );
}

#[test]
fn a_link_html_allows_in_the_body_is_written_where_it_is() {
    // Controls: every body-ok relation, in any case, several in one; and an
    // item's property.
    let src = page(
        "<link rel=\"stylesheet\" href=\"/menu.css\" />\n        \
         <link rel=\"Preload\" as=\"image\" href=\"/front.jpg\" />\n        \
         <link rel=\"preconnect dns-prefetch\" href=\"https://images.example.com\" />\n        \
         <link rel=\"prefetch\" href=\"/stores/48\" />\n        \
         <link rel=\"modulepreload\" href=\"/pw-runtime.mjs\" />\n        \
         <link rel=\"pingback\" href=\"/pingback\" />\n        \
         <div itemscope itemtype=\"https://schema.org/Offer\"><link itemprop=\"availability\" href=\"https://schema.org/InStock\" /></div>",
    );
    assert_eq!(reported(&src), Vec::<String>::new());
    // And it is written in the body, where it is.
    let units = vec![Unit {
        path: "t.pw".to_string(),
        hir: lower_file(&src, &parse_tree(&src).green),
        src: src.clone(),
    }];
    let b = pw_core::build::build(&units).expect("builds");
    let t = b
        .templates
        .iter()
        .find(|t| t.path == "t.P")
        .expect("the template");
    let body: String = t
        .chunks
        .iter()
        .filter_map(|c| match c {
            Chunk::Static(s) => Some(s.as_str()),
            Chunk::Dynamic(_) => None,
        })
        .collect();
    assert!(
        body.contains("<link rel=\"stylesheet\" href=\"/menu.css\">")
            && body
                .contains("<link itemprop=\"availability\" href=\"https://schema.org/InStock\">"),
        "{body}"
    );
}
