//! **A page's route** (ADR-0160).
//!
//! A page is served at its route, and the address gives its parameters, one
//! segment each. Until 2026-10-03 a route was read only to check links:
//! nothing said that its `{name}`s were the page's parameters, and the page
//! plan did not carry it, so a host could not serve a page where it said.
//!
//! - PW0340: a route is `/` and segments, a word or a `{parameter}` each, and
//!   names each of its page's parameters once.
//! - PW0621: a route's parameter is text.
//! - PW0341: one route is one page's.
//!
//! Each test states one part, with controls that check clean.

use pw_core::check::check_sources;

fn reported(src: &str) -> Vec<String> {
    check_sources(&[("t.pw".to_string(), src.to_string())])
        .into_iter()
        .flat_map(|(_, ds)| ds.into_iter().map(|d| format!("{} {}", d.code, d.message)))
        .collect()
}

/// A program with an opaque id, and `pages`.
fn program(pages: &str) -> String {
    format!("module t\n\nopaque type StoreId = String\n\n{pages}")
}

/// A page with `params`, served at `route`.
fn page(name: &str, params: &str, route: &str) -> String {
    format!(
        "page {name}({params}) {{\n    route \"{route}\"\n    cache private\n\n    \
         view {{\n        <main><h1>Store</h1></main>\n    }}\n}}\n\n"
    )
}

#[test]
fn a_route_that_names_its_parameters_is_the_control() {
    for (params, route) in [
        ("id: StoreId", "/stores/{id}"),
        ("id: StoreId", "/stores/{id}/menu"),
        (
            "store: StoreId, item: String",
            "/stores/{store}/items/{item}",
        ),
        ("", "/"),
        ("", "/about-us"),
    ] {
        let found = reported(&program(&page("P", params, route)));
        assert!(found.is_empty(), "{route}: {found:#?}");
    }
}

#[test]
fn a_route_names_each_parameter_once_and_nothing_else() {
    assert_eq!(
        reported(&program(&page("P", "id: StoreId", "/stores/{store}"))),
        [
            "PW0340 `/stores/{store}` names `store`, which is not a parameter of `P`",
            "PW0340 `/stores/{store}` does not name `id`, a parameter of `P`: the address \
             would not give it",
        ]
    );
    assert_eq!(
        reported(&program(&page("P", "id: StoreId", "/stores/{id}/{id}"))),
        ["PW0340 `/stores/{id}/{id}` names `id` twice"]
    );
    assert_eq!(
        reported(&program(&page("P", "id: StoreId", "/stores"))),
        [
            "PW0340 `/stores` does not name `id`, a parameter of `P`: the address would not \
             give it"
        ]
    );
}

#[test]
fn a_route_is_slash_and_segments() {
    for route in [
        "stores/{id}",
        "/stores//{id}",
        "/stores/{id",
        "/stores/{}",
        "/a b/{id}",
    ] {
        let found = reported(&program(&page("P", "id: StoreId", route)));
        assert_eq!(
            found,
            [format!(
                "PW0340 `{route}` is not a route: it is `/` and segments, a word or a \
                 `{{parameter}}` each"
            )],
            "{route}"
        );
    }
}

#[test]
fn a_routes_parameter_is_text() {
    assert_eq!(
        reported(&program(&page("P", "n: Int", "/orders/{n}"))),
        ["PW0621 `P`'s parameter `n` is of type `Int`, and an address gives it text"]
    );
    // Controls: a `String`, and an opaque type over one.
    assert!(reported(&program(&page("P", "n: String", "/orders/{n}"))).is_empty());
    assert!(reported(&program(&page("P", "id: StoreId", "/orders/{id}"))).is_empty());
}

#[test]
fn one_route_is_one_pages() {
    let found = reported(&program(&format!(
        "{}{}",
        page("Overview", "id: StoreId", "/stores/{id}"),
        page("Menu", "store: StoreId", "/stores/{store}")
    )));
    assert_eq!(
        found,
        ["PW0341 `Menu` is served at `/stores/{store}`, and so is `Overview`"]
    );
    // Control: another route beside it.
    let found = reported(&program(&format!(
        "{}{}",
        page("Overview", "id: StoreId", "/stores/{id}"),
        page("Menu", "store: StoreId", "/stores/{store}/menu")
    )));
    assert!(found.is_empty(), "{found:#?}");
}

/// **The page's plan carries its route** (ADR-0160), where a host serves
/// it; a page that declares none carries none.
#[test]
fn the_plan_carries_the_route() {
    use pw_core::lower::lower_file;
    use pw_core::resolve::Workspace;
    use pw_core::signatures::Signatures;
    let src = program(&format!(
        "{}page Plain() {{\n    cache private\n\n    view {{\n        <main><h1>Plain</h1></main>\n    }}\n}}\n",
        page("P", "id: StoreId", "/stores/{id}")
    ));
    let hir = lower_file(&src, &pw_syntax::parse_tree(&src).green);
    let hirs = vec![&hir];
    let ws = Workspace::build(&hirs);
    let sigs = Signatures::build(&ws, &hirs);
    let planned = pw_core::page_values::pages(&hirs, &ws, &sigs);
    let route = |page: &str| {
        planned
            .iter()
            .find(|p| p.page == page)
            .unwrap_or_else(|| panic!("no plan for {page}"))
            .plan
            .as_ref()
            .unwrap_or_else(|e| panic!("{page} does not plan: {e}"))
            .route
            .clone()
    };
    assert_eq!(route("t.P").as_deref(), Some("/stores/{id}"));
    assert_eq!(route("t.Plain"), None);
}
