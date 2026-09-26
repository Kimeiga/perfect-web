//! **A command's write reaches the fragments built on it** (ADR-0103).
//!
//! A materialization regenerates only when an event reaches it (`regenerate`
//! has one value, `on_invalidation`), and the materializer reads events, not a
//! command's `invalidates`. So a fragment has no staleness window of its own.
//! Until 2026-09-26 one built from a query with a window, which PW5106 leaves
//! to expire, kept what it rendered after a command wrote what the query
//! reads, and nothing required the command to emit anything. Each test states
//! one case, with a control.

use pw_core::check::check_sources;

fn program(src: &str) -> Vec<(String, String)> {
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
    check_sources(&program(src))
        .into_iter()
        .filter(|(n, _)| n == "t.pw")
        .flat_map(|(_, ds)| ds.into_iter().map(|d| format!("{} {}", d.code, d.message)))
        .collect()
}

/// A store query with `freshness` and `listens` clauses, a fragment built
/// from it with `hears`, and a command renaming the store with `clauses`.
fn program_of(freshness: &str, listens: &str, hears: &str, clauses: &str) -> String {
    format!(
        "module t\n\nimport Carts\nimport Menus\nimport Stores\n\
         import context.{{ current_session }}\nimport domain.{{ Cart, CartError, \
         InteractionId, MenuItem, MenuItemId, PositiveInt, Store, StoreError, StoreId }}\n\
         import Events.{{ StoreChanged }}\n\n\
         public query Shop(id: StoreId) -> Result<Store, StoreError>\n    \
         freshness      {freshness}\n    consistency    snapshot\n    cache          shared\n    \
         key            id\n{listens}{{\n    Stores.get(id)\n}}\n\n\
         materialize ShopFragment(id: StoreId) {{\n    placement      edge\n    \
         partition      public\n    depends_on     Shop(id)\n{hears}    \
         regenerate     on_invalidation\n    stampede       single_flight\n    \
         fallback       last_known_good\n}}\n\n\
         fn rename(id: StoreId) -> Result<Store, StoreError> !{{ database.write<Stores> }}\n\n\
         command rename_shop(id: StoreId) -> Result<Store, StoreError>\n    \
         requires      SignedIn\n    idempotent_by InteractionId\n{clauses}{{\n    \
         rename(id)\n}}\n"
    )
}

const LISTENS: &str = "    invalidates_on StoreChanged(id)\n";
const EMITS: &str = "    emits         StoreChanged(id)\n";

#[test]
fn a_fragment_hears_of_a_write_to_what_it_is_built_from() {
    // `Shop` may be five minutes stale; the fragment, never refreshed but by
    // an event, would keep the old store for good.
    let found = reported(&program_of("5.minutes", "", LISTENS, ""));
    assert_eq!(
        found,
        [
            "PW5106 `rename_shop` writes `Stores`, and no event it emits reaches \
          `ShopFragment`, which is built from `Shop`"
        ],
    );
    // The controls: the command emits what the fragment listens for, and a
    // command adding to the cart writes nothing the fragment shows.
    let found = reported(&program_of("5.minutes", "", LISTENS, EMITS));
    assert!(found.is_empty(), "{found:#?}");
    let src = program_of("5.minutes", "", LISTENS, "").replace(
        "command rename_shop(id: StoreId) -> Result<Store, StoreError>\n    \
         requires      SignedIn\n    idempotent_by InteractionId\n{\n    rename(id)\n}\n",
        "command add(item: MenuItemId, quantity: PositiveInt) -> Result<Cart, CartError>\n    \
         requires      SignedIn\n    idempotent_by InteractionId\n{\n    \
         Carts.add(current_session(), item, quantity)\n}\n",
    );
    assert!(src.contains("command add("), "the command was replaced");
    let found = reported(&src);
    assert!(found.is_empty(), "{found:#?}");
}

#[test]
fn naming_the_query_does_not_reach_the_fragment() {
    // `invalidates Shop(id)` drops the query's entries, and the materializer
    // reads events only.
    let found = reported(&program_of(
        "0.seconds",
        "",
        LISTENS,
        "    invalidates   Shop(id)\n",
    ));
    assert_eq!(
        found,
        [
            "PW5106 `rename_shop` writes `Stores`, and no event it emits reaches \
          `ShopFragment`, which is built from `Shop`"
        ],
    );
    let found = reported(&program_of(
        "0.seconds",
        "",
        LISTENS,
        "    invalidates   Shop(id)\n    emits         StoreChanged(id)\n",
    ));
    assert!(found.is_empty(), "{found:#?}");
}

#[test]
fn an_event_reaches_a_fragment_through_what_it_reads() {
    // The fragment listens for nothing, and `Shop` does (ADR-0102).
    let found = reported(&program_of("5.minutes", LISTENS, "", EMITS));
    assert!(found.is_empty(), "{found:#?}");
    // The control: nothing listens, so the event reaches nothing.
    let found = reported(&program_of("5.minutes", "", "", EMITS));
    assert_eq!(
        found,
        [
            "PW5106 `rename_shop` writes `Stores`, and no event it emits reaches \
          `ShopFragment`, which is built from `Shop`"
        ],
    );
}

#[test]
fn the_write_is_underlined_and_the_fragment_named() {
    // Built from a second query too, which reads the menus: the message
    // names the one that reads what the command writes.
    let src = program_of("5.minutes", "", LISTENS, "")
        .replace(
            "    depends_on     Shop(id)\n",
            "    depends_on     Shop(id), Dishes(id)\n",
        )
        .replace(
            "materialize ShopFragment",
            "public query Dishes(id: StoreId) -> Result<List<MenuItem>, StoreError>\n    \
             freshness      5.minutes\n    consistency    snapshot\n    \
             cache          shared\n    key            id\n{\n    \
             Menus.for_store(id)\n}\n\nmaterialize ShopFragment",
        );
    assert!(src.contains("Shop(id), Dishes(id)"), "both are read");
    assert_eq!(
        reported(&src),
        [
            "PW5106 `rename_shop` writes `Stores`, and no event it emits reaches \
          `ShopFragment`, which is built from `Shop`"
        ],
    );
    let d = check_sources(&program(&src))
        .into_iter()
        .find(|(n, _)| n == "t.pw")
        .expect("t.pw")
        .1
        .into_iter()
        .find(|d| d.code == "PW5106")
        .expect("PW5106");
    assert_eq!(&src[d.primary_span.clone()], "rename(id)");
    let labels: Vec<&str> = d.related.iter().map(|r| r.label.as_str()).collect();
    assert_eq!(
        labels,
        [
            "`rename_shop` is the command",
            "`ShopFragment` is built from `Shop`, which reads `Stores`"
        ]
    );
    assert!(src[d.related[1].span.clone()].starts_with("materialize ShopFragment"));
    let repairs: Vec<&str> = d.repairs.iter().map(|r| r.description.as_str()).collect();
    assert_eq!(
        repairs,
        ["emit `StoreChanged`, which reaches `ShopFragment`"]
    );
}

#[test]
fn a_fragment_that_reads_in_its_body_is_held_too() {
    // A materialize block may hold statements after its policies, and this
    // one reads the store itself.
    let fragment = |clauses: &str| {
        format!(
            "module t\n\nimport Stores\nimport domain.{{ InteractionId, Store, StoreError, \
             StoreId }}\nimport Events.{{ StoreChanged }}\n\n\
             materialize ShopName(id: StoreId) {{\n    placement      origin\n    \
             partition      public\n    invalidates_on StoreChanged(id)\n    \
             regenerate     on_invalidation\n    stampede       single_flight\n    \
             fallback       last_known_good\n    let store = Stores.get(id)\n}}\n\n\
             fn rename(id: StoreId) -> Result<Store, StoreError> !{{ database.write<Stores> }}\n\n\
             command rename_shop(id: StoreId) -> Result<Store, StoreError>\n    \
             requires      SignedIn\n    idempotent_by InteractionId\n{clauses}{{\n    \
             rename(id)\n}}\n"
        )
    };
    assert_eq!(
        reported(&fragment("")),
        [
            "PW5106 `rename_shop` writes `Stores`, and no event it emits reaches \
          `ShopName`, which reads it"
        ],
    );
    let found = reported(&fragment(EMITS));
    assert!(found.is_empty(), "{found:#?}");
}
