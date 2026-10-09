//! **What a value holds is one label** (ADR-0282): placement, a cache and a
//! fragment read it alike.
//!
//! Four findings of 2026-10-03, each reproduced at `d346c43`:
//! 1. a secret a query answers, kept by a shared fragment at the edge or in
//!    the browser, checked clean, where the same value rendered by a shared
//!    page was refused;
//! 2. a fragment placed at build read a session's cart;
//! 3. a page placed at build read the session through a query of its own,
//!    where reading it itself was refused;
//! 4. PW5002 said "it requires" and nothing after it, where a label alone
//!    ruled out every world.
//!
//! Each test states one, with its controls: a secret a query only uses, as
//! a key to fetch something public, is held by nothing that reads it.

use pw_core::check::check_sources;

fn sources(program: &str) -> Vec<(String, String)> {
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    let mut out = Vec::new();
    for d in [
        "packages/pw-std",
        "packages/pw-platform-web",
        "examples/lib",
    ] {
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
    let domain = root.join("examples/domain.pw");
    out.push((
        domain.display().to_string(),
        std::fs::read_to_string(&domain).expect("read"),
    ));
    out.push(("t.pw".to_string(), program.to_string()));
    out
}

/// Each diagnostic `program` is given, as `CODE message`.
fn reported(program: &str) -> Vec<String> {
    check_sources(&sources(program))
        .into_iter()
        .filter(|(n, _)| n == "t.pw")
        .flat_map(|(_, ds)| ds.into_iter().map(|d| format!("{} {}", d.code, d.message)))
        .collect()
}

fn with(code: &str, all: &[String]) -> Vec<String> {
    all.iter()
        .filter(|d| d.starts_with(code))
        .cloned()
        .collect()
}

/// A public query answering a secret, one using a secret to answer
/// something public, and `extra`.
fn secrets(extra: &str) -> String {
    format!(
        "module t\n\n\
         import secrets\n\
         import capability.{{ Secret, Payments }}\n\
         import domain.{{ StoreId }}\n\
         import Events.{{ MenuChanged }}\n\n\
         public query PaymentsKey(id: StoreId) -> Secret<Payments> {{\n    secrets.payments()\n}}\n\n\
         // Uses the key, and answers what it fetched: a public value (ADR-0085).\n\
         fn rates(key: Secret<Payments>) -> Int !{{}} {{ todo }}\n\n\
         public query Rates(id: StoreId) -> Int {{\n    rates(secrets.payments())\n}}\n\n\
         {extra}\n"
    )
}

fn fragment(name: &str, placement: &str, partition: &str, depends: &str) -> String {
    format!(
        "materialize {name}(id: StoreId) {{\n    \
         placement      {placement}\n    \
         partition      {partition}\n    \
         depends_on     {depends}(id)\n    \
         invalidates_on MenuChanged(id)\n    \
         regenerate     on_invalidation\n    \
         stampede       single_flight\n    \
         fallback       last_known_good\n}}\n"
    )
}

// --- 1. a secret a query answers -------------------------------------------------

#[test]
fn a_secret_a_query_answers_is_kept_by_no_shared_fragment() {
    let all = reported(&secrets(&fragment(
        "KeyFragment",
        "origin",
        "public",
        "PaymentsKey",
    )));
    let shared = with("PW5101", &all);
    assert!(
        shared.len() == 1 && shared[0].contains("Secret<Payments>"),
        "{all:?}"
    );
    // The control: a query that only uses the secret, as a key, answers a
    // public value, and a shared fragment of it is no one's secret.
    let uses = reported(&secrets(&fragment(
        "RatesFragment",
        "origin",
        "public",
        "Rates",
    )));
    assert_eq!(with("PW5101", &uses), Vec::<String>::new(), "{uses:?}");
}

#[test]
fn a_fragment_holding_a_secret_runs_only_at_the_origin() {
    for world in ["edge", "browser"] {
        let all = reported(&secrets(&fragment(
            "KeyFragment",
            world,
            "public",
            "PaymentsKey",
        )));
        let nowhere = with("PW5002", &all);
        assert!(
            nowhere.len() == 1 && nowhere[0].contains("it holds Secret<Payments>"),
            "{world}: {all:?}"
        );
    }
    // The controls: at the origin, where a secret may be held; and at the
    // edge, a fragment of what the key fetched.
    let origin = reported(&secrets(&fragment(
        "KeyFragment",
        "origin",
        "public",
        "PaymentsKey",
    )));
    assert_eq!(with("PW5002", &origin), Vec::<String>::new(), "{origin:?}");
    let edge = reported(&secrets(&fragment(
        "RatesFragment",
        "edge",
        "public",
        "Rates",
    )));
    assert_eq!(with("PW5002", &edge), Vec::<String>::new(), "{edge:?}");
}

// --- 2. a fragment built before any request --------------------------------------

/// A fragment of the session's cart, placed at `placement`.
fn cart_fragment(placement: &str) -> String {
    format!(
        "module t\n\n\
         import Resources.{{ Cart }}\n\
         import context.{{ current_session }}\n\n\
         materialize CartFragment() {{\n    \
         placement      {placement}\n    \
         depends_on     Cart(current_session())\n    \
         regenerate     on_invalidation\n    \
         stampede       single_flight\n    \
         fallback       last_known_good\n}}\n"
    )
}

#[test]
fn a_fragment_built_before_any_request_holds_no_session() {
    let all = reported(&cart_fragment("build"));
    let nowhere = with("PW5002", &all);
    assert!(
        nowhere.len() == 1 && nowhere[0].contains("it holds Session<SessionId>"),
        "{all:?}"
    );
    // The control: at the origin, where a request's session is.
    let origin = reported(&cart_fragment("origin"));
    assert_eq!(with("PW5002", &origin), Vec::<String>::new(), "{origin:?}");
}

// --- 3. the session read through a query of the page's own ----------------------

/// A page placed at `placement` reading the session's cart through `read`.
fn built_page(placement: &str, read: &str) -> String {
    format!(
        "module t\n\n\
         import Carts\n\
         import Resources.{{ Cart }}\n\
         import context.{{ current_session }}\n\
         import domain.{{ Cart, CartError }}\n\n\
         // The session read inside, and named in no signature.\n\
         query MyCart() -> Result<Cart, CartError> {{\n    \
         Carts.current(current_session())\n}}\n\n\
         page Built() {{\n    route \"/built\"\n    placement {placement}\n    \
         let cart = {read}\n    \
         view {{\n        <title>Cart</title>\n        <main><p>{{cart.line_count}}</p></main>\n    }}\n}}\n"
    )
}

#[test]
fn a_page_reading_the_session_through_a_query_of_its_own_is_not_built() {
    for read in ["query MyCart()", "query Cart(current_session())"] {
        let all = reported(&built_page("build", read));
        let nowhere = with("PW5002", &all);
        assert!(
            nowhere.len() == 1 && nowhere[0].contains("it holds Session<SessionId>"),
            "{read}: {all:?}"
        );
        // The control: at the origin.
        let origin = reported(&built_page("origin", read));
        assert_eq!(
            with("PW5002", &origin),
            Vec::<String>::new(),
            "{read}: {origin:?}"
        );
    }
}

#[test]
fn the_contract_places_it_where_the_checker_does() {
    use pw_core::check::Unit;
    use pw_core::lower::lower_file;
    use pw_syntax::parse_tree;
    // Unpinned, so the contract's solution is read whole.
    let units: Vec<Unit> =
        sources(&built_page("origin", "query MyCart()").replace("    placement origin\n", ""))
            .into_iter()
            .map(|(path, src)| Unit {
                hir: lower_file(&src, &parse_tree(&src).green),
                path,
                src,
            })
            .collect();
    let hirs: Vec<&pw_core::hir::Hir> = units.iter().map(|u| &u.hir).collect();
    let ws = pw_core::resolve::Workspace::build(&hirs);
    let sigs = pw_core::signatures::Signatures::build(&ws, &hirs);
    let page = pw_core::contract::contracts(&hirs, &sigs, &ws)
        .into_iter()
        .find(|c| c.component_id == "t.Built")
        .expect("the page's contract");
    // The session is read through `MyCart`, and the build world may not
    // hold one: the contract does not place it there.
    assert!(
        !page.allowed_placements.iter().any(|w| w == "build"),
        "{:?}",
        page.allowed_placements
    );
    assert!(
        page.allowed_placements.iter().any(|w| w == "origin"),
        "{:?}",
        page.allowed_placements
    );
}

// --- 4. what rules each world out ------------------------------------------------

#[test]
fn a_placement_refusal_says_what_rules_each_world_out() {
    // A label alone: the message names it, where it said "it requires" and
    // stopped.
    let label = reported(
        "module t\n\nsession page BuiltSession() {\n    route \"/s\"\n    placement build\n    \
         view {\n        <title>S</title>\n        <main><p>Hi</p></main>\n    }\n}\n",
    );
    let nowhere = with("PW5002", &label);
    assert_eq!(
        nowhere,
        ["PW5002 `BuiltSession` cannot run in any world: it holds Session<SessionId>"],
        "{label:?}"
    );
}
