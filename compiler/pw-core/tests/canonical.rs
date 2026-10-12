//! **A page states its canonical address** (ADR-0311): `<link
//! rel="canonical" href={P(id, name)} />` at the top of its view, beside its
//! title and metadata. Its address is a page's, checked as `navigate`'s is
//! (ADR-0280), and its host writes it into the head, absolute with the
//! deployment's origin, and `og:url` from it.
//!
//! - PW5035: one anywhere else is refused, saying where it goes;
//! - PW5051: what it addresses is a page served at a route, given values the
//!   page reads, or text;
//! - PW0604, PW0605: its arguments are the page's parameters;
//! - PW5034: `og:url` is the host's.
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

/// A page served at `/stores/{id}/{name}` whose view is its title and
/// `view`; a page at `/about`; a page served at no route; and a function.
fn program(view: &str) -> String {
    format!(
        "module t\n\nopaque type StoreId = String\n\n\
         page P(id: StoreId, name: String) {{\n    route \"/stores/{{id}}/{{name}}\"\n    \
         cache private\n\n    view {{\n        <title>Store</title>\n{view}\n    }}\n}}\n\n\
         page About() {{\n    route \"/about\"\n    cache shared\n\n    view {{\n        \
         <title>About</title>\n        <main><h1>About</h1></main>\n    }}\n}}\n\n\
         page Draft() {{\n    cache private\n\n    view {{\n        <main><h1>Draft</h1></main>\n    \
         }}\n}}\n\n\
         fn shout(s: String) -> String {{\n    s\n}}\n"
    )
}

const MAIN: &str = "        <main><h1>Store</h1></main>";

fn with_link(link: &str) -> String {
    program(&format!("        {link}\n{MAIN}"))
}

/// `P`'s template, as `pw build` emits it.
fn template(src: &str) -> Template {
    let units = vec![Unit {
        path: "t.pw".to_string(),
        hir: lower_file(src, &parse_tree(src).green),
        src: src.to_string(),
    }];
    let b = pw_core::build::build(&units).expect("builds");
    b.templates
        .iter()
        .find(|t| t.path == "t.P")
        .expect("the template")
        .clone()
}

fn canonical(t: &Template) -> Vec<&Part> {
    t.chunks
        .iter()
        .filter_map(|c| match c {
            Chunk::Dynamic(p @ Part::Canonical { .. }) => Some(p),
            _ => None,
        })
        .collect()
}

#[test]
fn a_page_s_canonical_address_is_a_part_of_its_head_after_its_metadata() {
    let src = program(&format!(
        "        <meta name=\"description\" content={{name}} />\n        \
         <link rel=\"canonical\" href={{P(id, \"menu\")}} />\n{MAIN}"
    ));
    assert_eq!(reported(&src), Vec::<String>::new());
    let t = template(&src);
    let found = canonical(&t);
    assert_eq!(
        found,
        [&Part::Canonical {
            id: found[0].id().expect("an id"),
            route: "/stores/{id}/{name}".to_string(),
            args: vec![
                ("id".to_string(), TitlePiece::Value("id".to_string())),
                ("name".to_string(), TitlePiece::Text("menu".to_string())),
            ],
        }]
    );
    // After the metadata, which is after the title: writing one moves no
    // other part, and it is the document's, not the body's.
    let manifest = t.manifest();
    let kinds: Vec<&str> = manifest.iter().rev().take(3).map(|e| e.kind).collect();
    assert_eq!(kinds, ["canonical", "meta", "title"], "{manifest:?}");
    assert!(
        manifest
            .iter()
            .filter(|e| e.kind == "canonical")
            .all(|e| e.anchor == Anchor::Document)
    );
    // Nothing of it is written in the body.
    let body: String = t
        .chunks
        .iter()
        .filter_map(|c| match c {
            Chunk::Static(s) => Some(s.as_str()),
            _ => None,
        })
        .collect();
    assert!(!body.contains("canonical"), "{body}");
}

#[test]
fn its_arguments_are_given_by_the_parameter_each_gives() {
    // By name, in any order: the route's order is the address's.
    let named = with_link("<link rel=\"canonical\" href={P(name: name, id: id)} />");
    assert_eq!(reported(&named), Vec::<String>::new());
    let t = template(&named);
    let Part::Canonical { args, .. } = canonical(&t)[0] else {
        panic!("a canonical part");
    };
    assert_eq!(
        args,
        &[
            ("id".to_string(), TitlePiece::Value("id".to_string())),
            ("name".to_string(), TitlePiece::Value("name".to_string())),
        ]
    );
    // Another page's address: a page may name another as its own.
    let about = with_link("<link rel=\"canonical\" href={About()} />");
    assert_eq!(reported(&about), Vec::<String>::new());
    let t = template(&about);
    let Part::Canonical { route, args, .. } = canonical(&t)[0] else {
        panic!("a canonical part");
    };
    assert_eq!((route.as_str(), args.len()), ("/about", 0));
    // A relation in another case is the same one, as HTML compares them.
    let upper = with_link("<link rel=\"CANONICAL\" href={P(id, name)} />");
    assert_eq!(reported(&upper), Vec::<String>::new());
    assert_eq!(canonical(&template(&upper)).len(), 1);
}

#[test]
fn a_canonical_link_is_the_page_s_once_at_the_top_of_its_view() {
    for (view, said) in [
        (
            "        <main><link rel=\"canonical\" href={P(id, name)} /><h1>Store</h1></main>"
                .to_string(),
            "PW5035 `<link rel=\"canonical\">` in `P` is inside an element or a block, not at the top of its view, where HTML does not allow it and nothing reads it",
        ),
        (
            format!(
                "        {{#if name == \"x\"}}<link rel=\"canonical\" href={{P(id, name)}} />{{/if}}\n{MAIN}"
            ),
            "PW5035 `<link rel=\"canonical\">` in `P` is inside an element or a block, not at the top of its view, where HTML does not allow it and nothing reads it",
        ),
        (
            format!(
                "        <link rel=\"canonical\" href={{P(id, name)}} />\n        \
                 <link rel=\"canonical\" href={{About()}} />\n{MAIN}"
            ),
            "PW5035 `P` states its canonical address twice",
        ),
        (
            format!("        <link rel=\"canonical preload\" href={{P(id, name)}} />\n{MAIN}"),
            "PW5035 `<link rel=\"canonical preload\">` in `P` is the page's canonical address and more: it names `canonical` alone",
        ),
        (
            format!(
                "        <link rel=\"canonical\" hreflang=\"en\" href={{P(id, name)}} />\n{MAIN}"
            ),
            "PW5035 `<link rel=\"canonical\">` in `P` has `hreflang`, which the host does not write: a canonical link is its relation and its address",
        ),
    ] {
        assert_eq!(reported(&program(&view)), [said.to_string()], "{view}");
    }
    // In a view, which every page that composes it would share.
    assert_eq!(
        reported(
            "module t\n\npage About() {\n    route \"/about\"\n    cache shared\n\n    view {\n        \
             <title>About</title>\n        <main><h1>About</h1></main>\n    }\n}\n\n\
             view Head() !{} {\n    <link rel=\"canonical\" href={About()} />\n    <h2>Store</h2>\n}\n"
        ),
        [
            "PW5035 `<link rel=\"canonical\">` in the view `Head` would name one address for every page that composes it: a page states its own"
        ]
    );
}

#[test]
fn a_canonical_address_is_a_page_s() {
    let text = "PW5051 `<link rel=\"canonical\">` in `P` states its address as text: a canonical address is a page's, which the compiler checks";
    for (link, said) in [
        (
            "<link rel=\"canonical\" />",
            "PW5051 `<link rel=\"canonical\">` in `P` states no address",
        ),
        ("<link rel=\"canonical\" href=\"/about\" />", text),
        (
            "<link rel=\"canonical\" href=\"/stores/{id}/{name}\" />",
            text,
        ),
        (
            "<link rel=\"canonical\" href={name} />",
            "PW5051 `<link rel=\"canonical\">` in `P` is an address computed in place: a canonical address is a page's",
        ),
        (
            "<link rel=\"canonical\" href={shout(name)} />",
            "PW5051 `<link rel=\"canonical\" href={shout(..)}>` in `P` names a function: a canonical address is a page's",
        ),
        (
            "<link rel=\"canonical\" href={Nowhere(id)} />",
            "PW5051 `<link rel=\"canonical\" href={Nowhere(..)}>` in `P` names nothing this module sees: a canonical address is a page's",
        ),
        (
            "<link rel=\"canonical\" href={Draft()} />",
            "PW5051 `<link rel=\"canonical\">` in `P` names `Draft`, which is served at no route",
        ),
        (
            "<link rel=\"canonical\" href={P(id, shout(name))} />",
            "PW5051 `<link rel=\"canonical\">` in `P` gives `P` a value computed in place: each argument is a value the page reads, or text",
        ),
    ] {
        assert_eq!(reported(&with_link(link)), [said.to_string()], "{link}");
    }
}

#[test]
fn its_arguments_are_the_page_s_parameters() {
    // As a call's are: how many, and each one's type.
    let fewer = reported(&with_link("<link rel=\"canonical\" href={P(id)} />"));
    assert!(
        fewer.len() == 1 && fewer[0].starts_with("PW0604"),
        "{fewer:?}"
    );
    let wrong = reported(&with_link(
        "<link rel=\"canonical\" href={P(name, name)} />",
    ));
    assert!(
        wrong.len() == 1 && wrong[0].starts_with("PW0605"),
        "{wrong:?}"
    );
}

#[test]
fn a_page_is_called_nowhere_else() {
    // The control: a call that names a page anywhere but a canonical link's
    // address is still refused (PW0027).
    let found = reported(&program(
        "        <link rel=\"canonical\" href={P(id, name)} />\n        \
         <main><p>{P(id, name)}</p></main>",
    ));
    assert_eq!(
        found,
        ["PW0027 `P` is a page, which a call cannot name".to_string()]
    );
}

#[test]
fn open_graph_s_address_is_the_host_s() {
    for property in ["og:url", "OG:URL"] {
        let src = program(&format!(
            "        <meta property=\"{property}\" content=\"https://kiokun.com/stores\" />\n{MAIN}"
        ));
        assert_eq!(
            reported(&src),
            [
                "PW5034 `<meta property=\"og:url\">` in `P` is the host's: it writes Open Graph's address from the page's canonical address (ADR-0311)"
                    .to_string()
            ],
            "{property}"
        );
    }
    // The control: Open Graph's title is the page's.
    let title = program(&format!(
        "        <meta property=\"og:title\" content={{name}} />\n{MAIN}"
    ));
    assert_eq!(reported(&title), Vec::<String>::new());
}

#[test]
fn an_address_carries_nothing_of_the_page_it_names() {
    // A page's address is a path: it runs nothing, and holds no value of the
    // page it names. A shared page naming a session page's address is
    // public. While the address was lowered as a call of the page, the page
    // was read as reading it, and refused as the session's (PW5001).
    let src = "module t\n\n\
               session page Cart() {\n    route \"/cart\"\n    cache private\n\n    \
               view {\n        <title>Cart</title>\n        <main><h1>Cart</h1></main>\n    }\n}\n\n\
               page About() {\n    route \"/about\"\n    cache shared\n\n    view {\n        \
               <title>About</title>\n        <link rel=\"canonical\" href={Cart()} />\n        \
               <main><h1>About</h1></main>\n    }\n}\n";
    assert_eq!(reported(src), Vec::<String>::new());
    // Lowered as an address, never as a call.
    let hir = lower_file(src, &parse_tree(src).green);
    let (_, about) = hir
        .all_decls()
        .find(|(_, d)| d.name == "About")
        .expect("the page");
    let body = hir.body(about.body.expect("a body"));
    let written: Vec<&pw_core::hir::Expr> = body.walk().into_iter().map(|e| body.expr(e)).collect();
    assert!(
        written
            .iter()
            .any(|e| matches!(e, pw_core::hir::Expr::Address { page, .. } if page == "Cart")),
        "{written:?}"
    );
    assert!(
        !written
            .iter()
            .any(|e| matches!(e, pw_core::hir::Expr::Call { .. })),
        "{written:?}"
    );
}
