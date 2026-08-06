//! E5 gate item 1 — privacy and placement across file boundaries.
//!
//! Charter §14 M5 asks for cross-file integration tests specifically, and the
//! reason is the failure they catch: a label or a capability that is obvious
//! inside one file and invisible when the declaration that carries it lives in
//! another. That is not a hypothetical here — R-004 was caught for a year
//! through an ambient module union, and when E2B removed the union the fixture
//! went silent, because the `Session` label came from `cart.queries` and
//! nothing said where to look.
//!
//! Each test below is a whole program of several modules where the invariant
//! **cannot be decided from any single file**.

use pw_core::check::check_sources;

fn program(files: &[(&str, &str)]) -> Vec<(String, Vec<String>)> {
    let owned: Vec<(String, String)> = files
        .iter()
        .map(|(n, s)| ((*n).to_string(), (*s).to_string()))
        .collect();
    check_sources(&owned)
        .into_iter()
        .map(|(n, ds)| (n, ds.iter().map(|d| d.symbol().to_string()).collect()))
        .collect()
}

fn symbols_of<'a>(results: &'a [(String, Vec<String>)], file: &str) -> &'a [String] {
    results
        .iter()
        .find(|(n, _)| n == file)
        .map(|(_, s)| s.as_slice())
        .unwrap_or(&[])
}

const CAPABILITY: &str = "module capability\n\n\
    opaque type Secret<C> = String\n\
    type Payments = Payments { name: String }\n\
    opaque type SessionId = String\n\
    opaque type Session<S> = String\n";

/// 1. A label that only exists in another file.
///
/// The page is unremarkable read alone: it renders a value bound from a query.
/// The query is `session`-scoped, and that declaration is in a third file.
#[test]
fn a_session_label_crosses_two_module_boundaries() {
    let r = program(&[
        ("cap.pw", CAPABILITY),
        (
            "domain.pw",
            "module domain\n\ntype Cart = Cart { line_count: Int }\n\
             opaque type SessionId = String\n\
             type CartError = CartError { why: String }\n",
        ),
        (
            "queries.pw",
            "module cart.queries\n\nimport domain.{ Cart, CartError, SessionId }\n\n\
             session query Cart(s: SessionId) -> Result<Cart, CartError>\n    \
                 cache private\n{\n    todo\n}\n",
        ),
        (
            "page.pw",
            "module store.page\n\nimport domain.{ Cart }\nimport cart.queries\n\n\
             page P() {\n    cache shared\n    let cart = query Cart(s)\n    \
             view { <main><p>{cart.line_count}</p></main> }\n}\n",
        ),
    ]);
    assert!(
        symbols_of(&r, "page.pw").contains(&"private_in_shared_cache".to_string()),
        "the label is declared two files away: {:?}",
        symbols_of(&r, "page.pw")
    );
}

/// 2. A capability the named world cannot grant, reached through a helper in
///    another file.
#[test]
fn a_capability_crosses_a_module_boundary_through_a_helper() {
    let r = program(&[
        (
            "device.pw",
            "module device\n\nopaque type Location = String\n\n\
             fn current_location() -> Location !{ device.location } { todo }\n",
        ),
        (
            "helper.pw",
            "module geo\n\nimport device\n\n\
             fn here() -> Location !{ device.location } { device.current_location() }\n",
        ),
        (
            "query.pw",
            "module delivery\n\nimport geo\n\n\
             query Estimate(id: Int) -> Location\n    placement origin\n    \
             cache private\n{\n    geo.here()\n}\n",
        ),
    ]);
    assert!(
        symbols_of(&r, "query.pw").contains(&"declared_placement_cannot_grant".to_string()),
        "{:?}",
        symbols_of(&r, "query.pw")
    );
}

/// 3. A secret whose label comes from another file, reaching markup.
#[test]
fn a_secret_crosses_a_module_boundary_into_markup() {
    let r = program(&[
        ("cap.pw", CAPABILITY),
        (
            "secrets.pw",
            "module secrets\n\nimport capability.{ Secret, Payments }\n\n\
             fn payments() -> Secret<Payments> !{ secret.read } { todo }\n",
        ),
        (
            "page.pw",
            "module checkout\n\nimport secrets\n\n\
             page Checkout() {\n    placement origin\n    \
             let key = secrets.payments()\n    \
             view { <main><p>{key}</p></main> }\n}\n",
        ),
    ]);
    assert!(
        symbols_of(&r, "page.pw").contains(&"secret_to_browser".to_string()),
        "{:?}",
        symbols_of(&r, "page.pw")
    );
}

/// 4. An effect that only becomes forbidden because of where the CALLER is.
///
/// `Stores.get` is perfectly legal. It is illegal in a view, and the view is in
/// a different file from the function.
#[test]
fn an_effect_becomes_forbidden_only_at_the_caller_in_another_file() {
    let r = program(&[
        (
            "stores.pw",
            "module Stores\n\ntype Store = Store { name: String }\n\n\
             fn get(id: Int) -> Store !{ database.read } { todo }\n",
        ),
        (
            "view.pw",
            "module store.badge\n\nimport Stores\n\n\
             view Badge(id: Int) !{ database.read } {\n    \
             <p>{Stores.get(id).name}</p>\n}\n",
        ),
    ]);
    assert!(
        symbols_of(&r, "view.pw").contains(&"forbidden_effect".to_string()),
        "{:?}",
        symbols_of(&r, "view.pw")
    );
    assert!(
        symbols_of(&r, "stores.pw").is_empty(),
        "the function itself is legal: {:?}",
        symbols_of(&r, "stores.pw")
    );
}

/// 5. The control: the same four shapes, all legitimate.
///
/// Without this the suite proves the checks fire across files and not that they
/// discriminate. Each half here is the neighbour of one test above.
#[test]
fn the_same_shapes_across_files_are_clean_when_they_are_correct() {
    let r = program(&[
        ("cap.pw", CAPABILITY),
        (
            "domain.pw",
            "module domain\n\ntype Cart = Cart { line_count: Int }\n\
             opaque type SessionId = String\n\
             type CartError = CartError { why: String }\n",
        ),
        (
            "queries.pw",
            "module cart.queries\n\nimport domain.{ Cart, CartError, SessionId }\n\n\
             session query Cart(s: SessionId) -> Result<Cart, CartError>\n    \
                 cache private\n{\n    todo\n}\n",
        ),
        // The private value, in a PRIVATE cache.
        (
            "page.pw",
            "module store.page\n\nimport domain.{ Cart }\nimport cart.queries\n\n\
             page P() {\n    cache private\n    let cart = query Cart(s)\n    \
             view { <main><p>{cart.line_count}</p></main> }\n}\n",
        ),
        (
            "device.pw",
            "module device\n\nopaque type Location = String\n\n\
             fn current_location() -> Location !{ device.location } { todo }\n",
        ),
        // The device read, in the BROWSER.
        (
            "widget.pw",
            "module locator\n\nimport device\n\n\
             component W() {\n    placement browser\n    \
             let here = device.current_location()\n    view { <p>Here</p> }\n}\n",
        ),
    ]);
    for f in ["page.pw", "widget.pw", "queries.pw", "device.pw"] {
        assert!(
            symbols_of(&r, f).is_empty(),
            "{f} must be clean: {:?}",
            symbols_of(&r, f)
        );
    }
}
