//! **A comment in markup is `<!-- -->`, and markup text is text** (ADR-0167).
//!
//! Until 2026-10-03:
//! - a `//` line between two elements checked, and the page showed it: ADR-0165
//!   explained the store's slots in one, and the store printed it;
//! - `<p>http://example.com</p>` did not parse, since the lexer read `//` as a
//!   comment that ran over the closing tag;
//! - `<!-- note --><p>one</p>` built `< note p>one</>`, and nothing said so.
//!
//! - PW5028: a line of markup text that begins with `//` or `/*` reads as a
//!   comment, and the page would show it.
//! - `<!-- … -->` is a comment, and no page shows it.
//! - An unquoted attribute value is read as HTML reads it: `href=http://x.y`
//!   built `href="http"` until 2026-10-03, and broke the element after it.
//!
//! Each test states one part, with controls that check clean.

use pw_core::check::check_sources;
use pw_core::template_ir::Chunk;

/// A page whose `<main>` holds `markup`.
fn page(markup: &str) -> String {
    format!(
        "module t\n\npage P() {{\n    cache private\n\n    view {{\n        <main>\n            \
         <h1>Title</h1>\n            {markup}\n            <p>one</p>\n        </main>\n    }}\n}}\n"
    )
}

fn reported(markup: &str) -> Vec<String> {
    check_sources(&[("t.pw".to_string(), page(markup))])
        .into_iter()
        .flat_map(|(_, ds)| ds.into_iter().map(|d| format!("{} {}", d.code, d.message)))
        .collect()
}

/// What the page writes, its static text joined: what a browser is sent.
fn rendered(markup: &str) -> String {
    let src = page(markup);
    let parsed = pw_syntax::parse_tree(&src);
    assert!(parsed.ok(), "{:?}", parsed.errors);
    let hir = pw_core::lower::lower_file(&src, &parsed.green);
    let hirs = vec![&hir];
    let ws = pw_core::resolve::Workspace::build(&hirs);
    let sigs = pw_core::signatures::Signatures::build(&ws, &hirs);
    let templates = pw_core::build::templates(&hirs, &[src.as_str()], &sigs);
    let page = templates
        .iter()
        .find(|t| t.path == "t.P")
        .expect("the page's template");
    page.chunks
        .iter()
        .map(|c| match c {
            Chunk::Static(s) => s.clone(),
            Chunk::Dynamic(_) => "{..}".to_string(),
        })
        .collect()
}

#[test]
fn a_line_that_reads_as_a_comment_is_refused() {
    for (markup, line) in [
        ("// the slots, explained", "// the slots, explained"),
        ("/* the slots */", "/* the slots */"),
        ("<p>x</p> // after it", "// after it"),
        (
            "<p>\n                // inside it\n            </p>",
            "// inside it",
        ),
    ] {
        assert_eq!(
            reported(markup),
            [format!(
                "PW5028 `{line}` reads as a comment, and the page would show it"
            )],
            "{markup}"
        );
    }
}

#[test]
fn text_that_holds_slashes_and_a_comment_in_markup_are_the_controls() {
    for markup in [
        "<p>http://example.com</p>",
        "<p>a // b</p>",
        "<p>see <a href=\"https://x.y/z\">here</a></p>",
        "<!-- the slots, explained -->",
        "<!-- a // <p> { here -->",
        "<p>{\"// as text\"}</p>",
        // A stylesheet's own comment. (Its braces would be holes: ADR-0094.)
        "<style>\n                /* the menu */\n            </style>",
    ] {
        let found = reported(markup);
        assert!(found.is_empty(), "{markup}: {found:#?}");
    }
}

#[test]
fn a_comment_is_not_sent_and_text_with_slashes_is() {
    let page = rendered("<!-- a note --><p>http://example.com // b</p>");
    assert!(!page.contains("a note"), "{page}");
    assert!(!page.contains("<!--"), "{page}");
    assert!(page.contains("<p>http://example.com // b</p>"), "{page}");
    // And what follows a comment is whole.
    assert!(page.contains("<p>one</p>"), "{page}");
}

#[test]
fn an_unquoted_attribute_value_is_sent_whole() {
    let page = rendered("<a href=http://x.y>y</a><a href=//cdn.x/y.js>z</a>");
    assert!(page.contains("<a href=\"http://x.y\">y</a>"), "{page}");
    assert!(page.contains("<a href=\"//cdn.x/y.js\">z</a>"), "{page}");
    assert!(page.contains("<p>one</p>"), "{page}");
}
