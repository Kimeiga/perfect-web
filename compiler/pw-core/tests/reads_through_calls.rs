//! **What a declaration reads, it reads through what it calls** (ADR-0118).
//!
//! The privacy rules read what a body reads one call deep. Until 2026-09-26 a
//! public query that read the session, whether itself, through a helper or
//! through another query, made one reader's value that each rule took for a
//! public one: a shared fragment of it (PW5101), a shared cache of it
//! (PW5004), a shared page reading it (PW5001) and the page's contract each
//! passed. Each test states the defect in each shape, with controls.

use pw_core::check::check_sources;

fn sources(src: &str) -> Vec<(String, String)> {
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
    out.push((
        "domain.pw".to_string(),
        std::fs::read_to_string(root.join("examples/domain.pw")).expect("domain.pw"),
    ));
    out.push(("t.pw".to_string(), src.to_string()));
    out
}

fn reported(src: &str) -> Vec<String> {
    check_sources(&sources(src))
        .into_iter()
        .filter(|(n, _)| n == "t.pw")
        .flat_map(|(_, ds)| ds.into_iter().map(|d| format!("{} {}", d.code, d.message)))
        .collect()
}

const IMPORTS: &str = "module t\n\nimport Carts\nimport Resources\n\
     import context.{ current_session, current_user }\n\
     import domain.{ Cart, CartError, StoreId, MenuItem, StoreError }\n\
     import capability.{ Session, SessionId, User, UserId }\n\n";

/// A public, uncached query `name` returning `ty`, whose body is `body`.
fn query(name: &str, params: &str, ty: &str, body: &str) -> String {
    format!(
        "public query {name}({params}) -> {ty}\n    freshness      0.seconds\n    \
         consistency    snapshot\n{{\n    {body}\n}}\n\n"
    )
}

/// A fragment at the edge in `partition`, depending on `on`.
fn fragment(name: &str, params: &str, partition: &str, on: &str) -> String {
    format!(
        "materialize {name}({params}) {{\n    placement      edge\n    \
         partition      {partition}\n    depends_on     {on}\n    \
         regenerate     on_invalidation\n    stampede       single_flight\n    \
         fallback       last_known_good\n}}\n"
    )
}

const CART: &str = "Result<Cart, CartError>";

/// The helper a query reads the session through.
const HELPER: &str = "fn mine() -> Result<Cart, CartError> !{ session.read, database.read<Carts> } \
                      {\n    Carts.current(current_session())\n}\n\n";

#[test]
fn a_shared_fragment_is_as_private_as_what_its_dependency_reads() {
    let shared = "materialized into a shared entry and depends on";
    for (shape, program, expected) in [
        (
            "reading the session",
            format!(
                "{IMPORTS}{}{}",
                query("Mine", "", CART, "Carts.current(current_session())"),
                fragment("Strip", "", "public", "Mine()")
            ),
            format!("PW5101 `Strip` is {shared} `Mine`, which reads Session<SessionId>"),
        ),
        (
            "through a helper",
            format!(
                "{IMPORTS}{HELPER}{}{}",
                query("Mine", "", CART, "mine()"),
                fragment("Strip", "", "public", "Mine()")
            ),
            format!("PW5101 `Strip` is {shared} `Mine`, which reads Session<SessionId>"),
        ),
        (
            "through another query",
            format!(
                "{IMPORTS}{}{}{}",
                query("Mine", "", CART, "Carts.current(current_session())"),
                query("Relay", "", CART, "Mine()"),
                fragment("Strip", "", "public", "Relay()")
            ),
            format!("PW5101 `Strip` is {shared} `Relay`, which reads Session<SessionId>"),
        ),
        (
            "relaying a session query",
            format!(
                "{IMPORTS}{}{}",
                query(
                    "Relay",
                    "session: Session<SessionId>",
                    CART,
                    "Resources.Cart(session)"
                ),
                fragment(
                    "Strip",
                    "session: Session<SessionId>",
                    "public",
                    "Relay(session)"
                )
            ),
            format!("PW5101 `Strip` is {shared} `Relay`, which reads Session<SessionId>"),
        ),
        (
            "reading the user",
            format!(
                "{IMPORTS}{}{}",
                query("Me", "", "User<UserId>", "current_user()"),
                fragment("Strip", "", "public", "Me()")
            ),
            format!("PW5101 `Strip` is {shared} `Me`, which reads User<UserId>"),
        ),
    ] {
        assert_eq!(reported(&program), [expected], "{shape}");
    }

    // The controls: the same fragment kept private, and a public query
    // relaying the public menu.
    let found = reported(&format!(
        "{IMPORTS}{HELPER}{}{}",
        query("Mine", "", CART, "mine()"),
        fragment("Strip", "", "private", "Mine()")
    ));
    assert!(found.is_empty(), "{found:#?}");
    let found = reported(&format!(
        "{IMPORTS}{}{}",
        query(
            "Relay",
            "id: StoreId",
            "Result<List<MenuItem>, StoreError>",
            "Resources.Menu(id)"
        ),
        fragment("Strip", "id: StoreId", "public", "Relay(id)")
    ));
    assert!(found.is_empty(), "{found:#?}");
    // And a query that signs with a key and returns what it fetched: a public
    // value (ADR-0085), which a shared fragment may hold. A secret in a
    // fragment is not this rule's question.
    let found = reported(
        "module t\n\nimport secrets\nimport capability.{ Secret, Payments }\n\n\
         fn rates(key: Secret<Payments>) -> Int !{} { todo }\n\n\
         public query Rates() -> Int\n    freshness      0.seconds\n    \
         consistency    snapshot\n{\n    rates(secrets.payments())\n}\n\n\
         materialize Strip() {\n    placement      edge\n    partition      public\n    \
         depends_on     Rates()\n    regenerate     on_invalidation\n    \
         stampede       single_flight\n    fallback       last_known_good\n}\n",
    );
    assert!(found.is_empty(), "{found:#?}");
}

/// `query`, with a shared cache keyed by nothing.
fn shared_query(body: &str) -> String {
    format!(
        "public query Mine() -> {CART}\n    freshness      0.seconds\n    \
         consistency    snapshot\n    cache          shared\n{{\n    {body}\n}}\n"
    )
}

#[test]
fn a_shared_cache_reads_through_a_helper() {
    // PW5001's since ADR-0128: a session's value is refused a shared cache
    // whatever its key, where PW5004 asked for the session in the key.
    let expected = ["PW5001 `Mine` is Session<SessionId> and declares a shared cache"];
    assert_eq!(
        reported(&format!(
            "{IMPORTS}{}",
            shared_query("Carts.current(current_session())")
        )),
        expected,
        "reading the session"
    );
    assert_eq!(
        reported(&format!("{IMPORTS}{HELPER}{}", shared_query("mine()"))),
        expected,
        "through a helper"
    );
    // The control: the helper's cart, cached per reader.
    let found = reported(&format!(
        "{IMPORTS}{HELPER}{}",
        shared_query("mine()").replace("cache          shared", "cache          private")
    ));
    assert!(found.is_empty(), "{found:#?}");
}

/// A page cached `cache`, reading `query Relay()`.
fn page(cache: &str) -> String {
    format!(
        "page ShopPage(id: StoreId) {{\n    cache {cache}\n\n    \
         let basket = query Relay()\n\n    view {{ <main /> }}\n}}\n"
    )
}

#[test]
fn a_shared_page_reads_through_a_public_query() {
    // Relaying a session query: PW5001's, as reading the session query is.
    assert_eq!(
        reported(&format!(
            "{IMPORTS}{}{}",
            query("Relay", "", CART, "Resources.Cart(current_session())"),
            page("shared")
        )),
        ["PW5001 `ShopPage` is Session<SessionId> and declares a shared cache"],
    );
    // Reading the session: PW5001's too since ADR-0128, where it was PW5004
    // asking for the session in the key.
    assert_eq!(
        reported(&format!(
            "{IMPORTS}{}{}",
            query("Relay", "", CART, "Carts.current(current_session())"),
            page("shared")
        )),
        ["PW5001 `ShopPage` is Session<SessionId> and declares a shared cache"],
    );
    // The controls: the same page, cached per reader; and a shared page whose
    // handler calls a command that reads the session. A call to a command is
    // a request, and what the command reads is its own (ADR-0113).
    let found = reported(&format!(
        "{IMPORTS}{}{}",
        query("Relay", "", CART, "Resources.Cart(current_session())"),
        page("private")
    ));
    assert!(found.is_empty(), "{found:#?}");
    let found = reported(
        "module t\n\nimport Carts\nimport Resources.{ Menu }\n\
         import context.{ current_session }\n\
         import domain.{ StoreId, MenuItemId, PositiveInt, InteractionId, Cart, CartError }\n\n\
         command add_to_cart(item: MenuItemId, quantity: PositiveInt) -> Result<Cart, CartError>\n    \
         requires      SignedIn\n    idempotent_by InteractionId\n{\n    \
         Carts.add(current_session(), item, quantity)\n}\n\n\
         page ShopPage(id: StoreId) {\n    placement origin\n    cache shared\n\n    \
         let menu = query Menu(id)\n\n    view {\n        <ul>\n            \
         {#each menu as item (item.id)}\n                <li>\n                    \
         <button type=\"button\" on:press={resumable(captures = { item }) => \
         add_to_cart(item.id, PositiveInt(1))}>Add</button>\n                </li>\n            \
         {/each}\n        </ul>\n    }\n}\n",
    );
    assert!(found.is_empty(), "{found:#?}");
}

#[test]
fn a_contract_is_as_private_as_what_its_component_reads() {
    use pw_core::contract::contracts;
    use pw_core::hir::Hir;
    use pw_core::lower::lower_file;
    use pw_core::resolve::Workspace;
    use pw_core::signatures::Signatures;

    // The contract's placement reads the label the checker does. A page
    // reading the session's cart through a public query may not be built
    // before a request; its contract allowed `build` until 2026-09-26.
    let placements = |relayed: &str| {
        let program = sources(&format!(
            "{IMPORTS}{}{}",
            query("Relay", "", CART, relayed),
            page("private")
        ));
        let hirs: Vec<Hir> = program
            .iter()
            .map(|(_, s)| lower_file(s, &pw_syntax::parse_tree(s).green))
            .collect();
        let refs: Vec<&Hir> = hirs.iter().collect();
        let ws = Workspace::build(&refs);
        let sigs = Signatures::build(&ws, &refs);
        contracts(&refs, &sigs, &ws)
            .into_iter()
            .find(|c| c.component_id == "t.ShopPage")
            .expect("the page's contract")
            .allowed_placements
    };
    assert_eq!(
        placements("Resources.Cart(current_session())"),
        ["browser", "edge", "origin"]
    );
    // The control: a public query relaying nothing of a reader's.
    assert_eq!(placements("todo"), ["build", "browser", "edge", "origin"]);
}
