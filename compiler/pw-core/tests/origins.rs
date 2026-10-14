//! **A page loads from its own origin** (ADR-0195's ruling 13, the build's
//! half).
//!
//! The host sends each page a Content-Security-Policy generated from its
//! build and its deployment, which allows a page's loads from its own origin
//! and its deployment's providers alone. Until step two, an address written
//! from another origin built, and the browser refused it.
//!
//! - PW5052: an address a page's markup fetches, written as text, is a path,
//!   and a `<link>`'s address is written as text.
//!
//! Each test states one part, with controls that check clean.

use pw_core::check::check_sources;
use pw_core::origins::another_origin;

fn reported(src: &str) -> Vec<String> {
    check_sources(&[("t.pw".to_string(), src.to_string())])
        .into_iter()
        .flat_map(|(_, ds)| ds.into_iter().map(|d| format!("{} {}", d.code, d.message)))
        .collect()
}

/// A page served at `/stores/{id}`, whose `<main>` holds `markup`.
fn page(markup: &str) -> String {
    format!(
        "module t\n\nopaque type StoreId = String\n\n\
         page P(id: StoreId, src: String) {{\n    route \"/stores/{{id}}/{{src}}\"\n    cache private\n\n    \
         view {{\n        <title>Store</title>\n        \
         <main><h1>Store</h1>{markup}</main>\n    }}\n}}\n"
    )
}

/// **An address's origin, as a browser's URL parser reads it**: a scheme
/// and its authority; a `//` and its `\` spellings, which name a host; the
/// controls and spaces it strips, and the tabs and newlines it removes;
/// none for a path, a query or a fragment.
#[test]
fn an_address_names_another_origin_as_a_browser_reads_it() {
    for (address, origin) in [
        ("https://cdn.example/logo.png", "https://cdn.example"),
        ("HTTPS://CDN.example:8443/a?b#c", "https://CDN.example:8443"),
        ("//cdn.example/logo.png", "//cdn.example"),
        ("\\\\cdn.example/logo.png", "//cdn.example"),
        ("/\\cdn.example/logo.png", "//cdn.example"),
        (" \t//cdn.example/logo.png\n", "//cdn.example"),
        ("/\t/cdn.example/logo.png", "//cdn.example"),
        ("data:image/png;base64,iVBORw0KGgo=", "data:"),
        ("blob:https://x/1", "blob:"),
        ("about:blank", "about:"),
        ("h\tttps://cdn.example/a", "https://cdn.example"),
    ] {
        assert_eq!(
            another_origin(address).as_deref(),
            Some(origin),
            "{address:?}"
        );
    }
    for path in [
        "/logo.png",
        "logo.png",
        "./a/b.png",
        "../a",
        "?q=1",
        "#top",
        "",
        "/a:b",
        "a/b:c",
    ] {
        assert_eq!(another_origin(path), None, "{path:?}");
    }
}

#[test]
fn a_load_from_another_origin_is_refused_where_it_is_written() {
    for (markup, said) in [
        (
            "<img src=\"https://images.example/logo.png\" alt=\"Logo\" />",
            "PW5052 `<img src=\"https://images.example/logo.png\">` in `P` loads from `https://images.example`, another origin: a page loads from its own",
        ),
        (
            "<iframe src=\"//maps.example/embed\" title=\"Map\"></iframe>",
            "PW5052 `<iframe src=\"//maps.example/embed\">` in `P` loads from `//maps.example`, another origin: a page loads from its own",
        ),
        // A `srcset`'s each candidate, its descriptor after it.
        (
            "<img src=\"/a.png\" srcset=\"/a.png 1x, https://images.example/a@2x.png 2x\" alt=\"A\" />",
            "PW5052 `<img srcset=\"/a.png 1x, https://images.example/a@2x.png 2x\">` in `P` loads from `https://images.example`, another origin: a page loads from its own",
        ),
        (
            "<img src=\"data:image/png;base64,iVBORw0KGgo=\" alt=\"A\" />",
            "PW5052 `<img src=\"data:image/png;base64,iVBORw0KGgo=\">` in `P` loads from `data:`, another origin: a page loads from its own",
        ),
        (
            "<link rel=\"preconnect\" href=\"https://images.example\" />",
            "PW5052 `<link href=\"https://images.example\">` in `P` loads from `https://images.example`, another origin: a page loads from its own",
        ),
        // Built from text and a value: the text before its first value is
        // where it starts.
        (
            "<img src=\"https://images.example/{id}.png\" alt=\"A\" />",
            "PW5052 `<img src=\"https://images.example/{id}.png\">` in `P` loads from `https://images.example`, another origin: a page loads from its own",
        ),
        // Every candidate of a `srcset` built with a value, not only the
        // text before its first value.
        (
            "<img src=\"/a.png\" srcset=\"/images/{id}.png 1x, https://images.example/b@2x.png 2x\" alt=\"A\" />",
            "PW5052 `<img srcset=\"/images/{id}.png 1x, https://images.example/b@2x.png 2x\">` in `P` loads from `https://images.example`, another origin: a page loads from its own",
        ),
        // A value where the host is: another origin all the same.
        (
            "<iframe src=\"https://{src}/embed\" title=\"Map\"></iframe>",
            "PW5052 `<iframe src=\"https://{src}/embed\">` in `P` loads from `https://\u{2026}`, another origin: a page loads from its own",
        ),
        (
            "<video src=\"/v.mp4\" poster=\"https://images.example/p.png\"></video>",
            "PW5052 `<video poster=\"https://images.example/p.png\">` in `P` loads from `https://images.example`, another origin: a page loads from its own",
        ),
        (
            "<a href=\"/stores/{id}/{src}\" ping=\"https://track.example/p\">X</a>",
            "PW5052 `<a ping=\"https://track.example/p\">` in `P` loads from `https://track.example`, another origin: a page loads from its own",
        ),
    ] {
        let found = reported(&page(markup));
        assert_eq!(found, [said.to_string()], "{markup}");
    }
    // In a view as in a page: a view's markup is its page's body.
    assert_eq!(
        reported(
            "module t\n\nview Logo() !{} {\n    <img src=\"https://images.example/logo.png\" alt=\"Logo\" />\n}\n"
        ),
        [
            "PW5052 `<img src=\"https://images.example/logo.png\">` in `Logo` loads from `https://images.example`, another origin: a page loads from its own"
                .to_string()
        ]
    );
}

#[test]
fn a_links_address_is_written_as_text() {
    for markup in [
        "<link rel=\"stylesheet\" href={src} />",
        "<link rel=\"stylesheet\" href=\"{src}/a.css\" />",
        "<link rel=\"preload\" href=\"ht{src}\" />",
    ] {
        let found = reported(&page(markup));
        assert_eq!(
            found,
            ["PW5052 `<link href>` in `P` is an address a value chooses: a `<link>`'s address is written as text, so its build knows the origin it names".to_string()],
            "{markup}"
        );
    }
}

/// **The controls**: a path, relative or not, a query and a fragment; a
/// link's `href`, which is a navigation; an image whose address a value
/// chooses, which the page's policy holds in the browser; a link that is an
/// item's property, a value no browser fetches; a link's path built with a
/// value; and a page's canonical address, a page's (PW5051).
#[test]
fn a_load_from_this_origin_and_what_is_no_load_check_clean() {
    for markup in [
        "<img src=\"/logo.png\" alt=\"Logo\" />",
        "<img src=\"logo.png\" srcset=\"logo.png 1x, logo@2x.png 2x\" alt=\"Logo\" />",
        "<iframe src=\"/maps/embed?x=1#here\" title=\"Map\"></iframe>",
        "<a href=\"https://example.com/help\">Help</a>",
        "<img src={src} alt=\"A\" />",
        "<img src=\"/images/{src}.png\" alt=\"A\" />",
        "<div itemscope><link itemprop=\"availability\" href=\"https://schema.org/InStock\" /></div>",
        "<link rel=\"stylesheet\" href=\"/stores/{id}/menu.css\" />",
        "<link rel=\"preload\" href=\"/a.css\" as=\"style\" />",
    ] {
        assert_eq!(reported(&page(markup)), Vec::<String>::new(), "{markup}");
    }
    let canonical = "module t\n\nopaque type StoreId = String\n\n\
         page P(id: StoreId) {\n    route \"/stores/{id}\"\n    cache private\n\n    \
         view {\n        <title>Store</title>\n        <link rel=\"canonical\" href={P(id)} />\n        \
         <main><h1>Store</h1></main>\n    }\n}\n";
    assert_eq!(reported(canonical), Vec::<String>::new());
}
