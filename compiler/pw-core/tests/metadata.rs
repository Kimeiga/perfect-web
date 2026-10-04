//! **A page states its metadata** (ADR-0186): what it says of itself to what
//! reads it without showing it, a search engine's result or a link's
//! preview. `<meta name="description" content={store.description} />` at the
//! top of its view, which its host writes into the document's head.
//!
//! - PW5034: a page's metadata is the page's: at the top of its view, named
//!   as text, its content text and values.
//! - The template holds each as a part of its own, after the title's.
//! - It is written as the page is served and set again by nothing: it reads
//!   no signal, and what a press speculates is no metadata's.
//!
//! Each test states one part, with controls that check clean.

use pw_core::check::{Unit, check_sources};
use pw_core::lower::lower_file;
use pw_core::template_ir::{Anchor, Chunk, Part, TitlePiece};
use pw_syntax::parse_tree;

fn reported(src: &str) -> Vec<String> {
    check_sources(&[("t.pw".to_string(), src.to_string())])
        .into_iter()
        .flat_map(|(_, ds)| ds.into_iter().map(|d| format!("{} {}", d.code, d.message)))
        .collect()
}

/// A page served at `/stores/{id}`, given its store's name, whose view is
/// `view`.
fn page(view: &str) -> String {
    format!(
        "module t\n\nopaque type StoreId = String\n\n\
         page P(id: StoreId, name: String) {{\n    route \"/stores/{{id}}/{{name}}\"\n    cache private\n\n    \
         view {{\n        <title>Store</title>\n{view}\n    }}\n}}\n"
    )
}

const MAIN: &str = "        <main><h1>Store</h1></main>";

#[test]
fn a_page_s_metadata_is_a_part_of_its_head_after_its_title() {
    let src = page(&format!(
        "        <meta name=\"description\" content=\"{{name}}, and its menu.\" />\n\
         <meta property=\"og:title\" content={{name}} />\n{MAIN}"
    ));
    assert_eq!(reported(&src), Vec::<String>::new());
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
    let metas: Vec<&Part> = t
        .chunks
        .iter()
        .filter_map(|c| match c {
            Chunk::Dynamic(p @ Part::Meta { .. }) => Some(p),
            _ => None,
        })
        .collect();
    assert_eq!(
        metas,
        [
            &Part::Meta {
                id: metas[0].id().expect("an id"),
                attribute: "name".to_string(),
                key: "description".to_string(),
                content: vec![
                    TitlePiece::Value("name".to_string()),
                    TitlePiece::Text(", and its menu.".to_string()),
                ],
            },
            &Part::Meta {
                id: metas[1].id().expect("an id"),
                attribute: "property".to_string(),
                key: "og:title".to_string(),
                content: vec![TitlePiece::Value("name".to_string())],
            },
        ]
    );
    // After the title, which is after the rest: writing metadata moves no
    // other part.
    let manifest = t.manifest();
    let kinds: Vec<&str> = manifest.iter().rev().take(3).map(|e| e.kind).collect();
    assert_eq!(kinds, ["meta", "meta", "title"], "{manifest:?}");
    assert!(
        manifest
            .iter()
            .filter(|e| e.kind == "meta")
            .all(|e| e.anchor == Anchor::Document)
    );
}

#[test]
fn a_page_s_metadata_is_the_page_s_at_the_top_of_its_view() {
    for (view, said) in [
        (
            "        <main><meta name=\"description\" content=\"Store\" /><h1>Store</h1></main>"
                .to_string(),
            "PW5034 `<meta>` in `P` is inside an element or a block, not at the top of its view",
        ),
        (
            format!("        <meta charset=\"utf-8\" />\n{MAIN}"),
            "PW5034 `<meta charset>` in `P` is the host's: it writes the document's charset and viewport (ADR-0182)",
        ),
        (
            format!("        <meta name=\"viewport\" content=\"width=480\" />\n{MAIN}"),
            "PW5034 `<meta name=\"viewport\">` in `P` is the host's: it writes the document's charset and viewport (ADR-0182)",
        ),
        (
            format!("        <meta http-equiv=\"refresh\" content=\"5\" />\n{MAIN}"),
            "PW5034 `<meta http-equiv>` in `P` is the host's: it writes the document's charset and viewport (ADR-0182)",
        ),
        (
            format!("        <meta content=\"Store\" />\n{MAIN}"),
            "PW5034 `<meta>` in `P` names nothing it describes",
        ),
        (
            format!("        <meta name={{name}} content=\"Store\" />\n{MAIN}"),
            "PW5034 `<meta name>` in `P` is named by a value computed in place",
        ),
        (
            format!("        <meta name=\"description\" />\n{MAIN}"),
            "PW5034 `<meta name=\"description\">` in `P` has no content",
        ),
        (
            format!(
                "        <meta name=\"description\" content={{String.length(name)}} />\n{MAIN}"
            ),
            "PW5034 `<meta name=\"description\">` in `P` holds a value computed in place, and its content is text and values",
        ),
        (
            format!(
                "        <meta name=\"description\" content=\"{{String.length(name)}} stores\" />\n{MAIN}"
            ),
            "PW5034 `<meta name=\"description\">` in `P` holds a value computed in place, and its content is text and values",
        ),
        (
            format!("        <meta name=\"VIEWPORT\" content=\"width=480\" />\n{MAIN}"),
            "PW5034 `<meta name=\"viewport\">` in `P` is the host's: it writes the document's charset and viewport (ADR-0182)",
        ),
        (
            format!(
                "        <meta name=\"description\" property=\"og:description\" content=\"Store\" />\n{MAIN}"
            ),
            "PW5034 `<meta>` in `P` names what it describes twice, by `name` and by `property`: one `<meta>` names one",
        ),
        // The head writes a name and a content: a `media` or a `lang` would
        // be lost.
        (
            format!(
                "        <meta name=\"theme-color\" media=\"(prefers-color-scheme: dark)\" content=\"black\" />\n{MAIN}"
            ),
            "PW5034 `<meta name=\"theme-color\">` in `P` has `media`, which the host does not write: a page's metadata is its name and its content",
        ),
        (
            format!("        <meta name=\"description\" content=\"  \" />\n{MAIN}"),
            "PW5034 `<meta name=\"description\">` in `P` has no content",
        ),
        // A name HTML allows once, compared as HTML compares it.
        (
            format!(
                "        <meta name=\"Description\" content=\"One\" />\n\
                 <meta name=\"description\" content=\"Two\" />\n{MAIN}"
            ),
            "PW5034 `P` states `<meta name=\"description\">` twice, which HTML allows once",
        ),
        (
            format!(
                "        <meta name=\"theme-color\" content=\"white\" />\n\
                 <meta name=\"theme-color\" content=\"black\" />\n{MAIN}"
            ),
            "PW5034 `P` states `<meta name=\"theme-color\">` twice, which HTML allows once",
        ),
        (
            format!(
                "        <meta itemprop=\"price\" name=\"description\" content=\"3.50\" />\n{MAIN}"
            ),
            "PW5034 `<meta itemprop>` in `P` is an item's property and names the page's metadata too: HTML allows one of `name`, `http-equiv`, `charset` and `itemprop`",
        ),
    ] {
        let found = reported(&page(&view));
        assert!(found.contains(&said.to_string()), "{view}\n{found:#?}");
    }
    // In a view: it would describe every page that composes it.
    assert_eq!(
        reported(
            "module t\n\nview Card() !{} {\n    <meta name=\"description\" content=\"A store.\" />\n    <h2>Store</h2>\n}\n"
        ),
        ["PW5034 `<meta>` in the view `Card` describes every page that composes it"]
    );
    // Controls: after `<main>`; a `property` that repeats, which Open Graph
    // reads as a list; a name HTML lets repeat.
    assert_eq!(
        reported(&page(&format!(
            "{MAIN}\n        <meta property=\"og:image\" content=\"/a.jpg\" />\n\
             <meta property=\"og:image\" content=\"/b.jpg\" />\n\
             <meta name=\"keywords\" content=\"coffee\" />\n\
             <meta name=\"keywords\" content=\"espresso\" />\n\
             <meta name=\"description\" content=\"Store {{name}}\" />"
        ))),
        Vec::<String>::new()
    );
}

#[test]
fn an_item_s_property_is_written_where_it_is() {
    // `<meta itemprop>` is microdata: a property of the item around it, or
    // of one that names it by `itemref`, in a page or a view, not the page's
    // metadata.
    let offer = "<div itemscope itemtype=\"https://schema.org/Offer\" itemref=\"currency\">\
                 <meta itemprop=\"price\" content=\"3.50\" /><p>Espresso</p></div>";
    let src = page(&format!(
        "        <meta id=\"currency\" itemprop=\"priceCurrency\" content=\"USD\" />\n        \
         <main><h1>Store</h1>{offer}</main>"
    ));
    assert_eq!(reported(&src), Vec::<String>::new());
    assert_eq!(
        reported(&format!(
            "module t\n\nview Offer() !{{}} {{\n    {offer}\n}}\n"
        )),
        Vec::<String>::new()
    );
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
        body.contains("<meta id=\"currency\" itemprop=\"priceCurrency\" content=\"USD\">")
            && body.contains("<meta itemprop=\"price\" content=\"3.50\"><p>Espresso</p>"),
        "{body}"
    );
    assert!(
        !t.chunks
            .iter()
            .any(|c| matches!(c, Chunk::Dynamic(Part::Meta { .. }))),
        "{:?}",
        t.chunks
    );
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

/// The store's description, as `app.pw` writes it.
const DESCRIPTION: &str = "        <meta name=\"description\" content={store.description} />\n";

#[test]
fn a_page_s_metadata_may_read_what_a_press_speculates() {
    // Written as the page is served and set again by nothing: what a press
    // speculates (the cart, ADR-0172) is no metadata's, and the page's
    // speculation does not refuse it, as it refuses a title that reads it.
    // The cart's count is a member function, which a host computes only in
    // text in the page's body (ADR-0125): that is the one refusal.
    let refused = build(|s| {
        assert_eq!(
            s.matches(DESCRIPTION).count(),
            1,
            "the description's anchor"
        );
        s.replace(
            DESCRIPTION,
            "        <meta name=\"description\" content=\"{cart.line_count} in the cart\" />\n",
        )
    })
    .expect("builds")
    .refusals();
    assert_eq!(refused.len(), 1, "{refused:#?}");
    assert!(
        refused[0]
            .contains("reads `cart.line_count` through a member function in the page's metadata"),
        "{refused:#?}"
    );
    // Control: the store's own description reads the store.
    assert_eq!(
        build(|s| s.to_string()).expect("builds").refusals(),
        Vec::<String>::new()
    );
}

#[test]
fn a_page_s_metadata_reads_no_signal() {
    // A signal is what a person has done on the page, not what the page is,
    // and the browser writes no metadata again.
    let src = "module t\n\nopaque type StoreId = String\n\n\
         page P(id: StoreId) {\n    route \"/stores/{id}\"\n    cache private\n\n    \
         signal greeting: String = \"Hello\"\n\n    view {\n        <title>Store</title>\n        \
         <meta name=\"description\" content={greeting} />\n        <main><h1>{greeting}</h1>\n        \
         <button type=\"button\" on:press={() => greeting = \"Hi\"}>Greet</button></main>\n    }\n}\n"
        .to_string();
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
        why.contains("is a meta part the signal `greeting` decides"),
        "{why}"
    );
}
