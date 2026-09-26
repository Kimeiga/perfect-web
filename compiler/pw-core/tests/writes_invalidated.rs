//! **A command invalidates what it writes** (ADR-0101).
//!
//! ADR-0007 made invalidation explicit: a command `invalidates` the entries it
//! drops and `emits` typed events, and a cached query listens for them with
//! `invalidates_on`. Until 2026-09-26 nothing held a command to the data it
//! writes: an `add_to_cart` with neither clause checked, and every page kept
//! the cart from before. Each test states one case, with a control.

use pw_core::check::check_sources;

fn program(files: &[(&str, String)]) -> Vec<(String, String)> {
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
    for (name, src) in files {
        out.push((name.to_string(), src.clone()));
    }
    out
}

/// What the given files report, as `CODE message`.
fn reported(files: &[(&str, String)]) -> Vec<String> {
    let names: Vec<&str> = files.iter().map(|(n, _)| *n).collect();
    check_sources(&program(files))
        .into_iter()
        .filter(|(n, _)| names.contains(&n.as_str()))
        .flat_map(|(_, ds)| ds.into_iter().map(|d| format!("{} {}", d.code, d.message)))
        .collect()
}

/// Module `name`, holding `parts`.
fn module(name: &str, parts: &[String]) -> String {
    format!(
        "module {name}\n\nimport Carts\nimport Stores\nimport context.{{ current_session }}\n\
         import domain.{{ Cart, CartError, InteractionId, MenuItemId, PositiveInt, Store, \
         StoreError, StoreId }}\nimport capability.{{ Session, SessionId }}\n\
         import Events.{{ CartChanged }}\n\n{}",
        parts.join("\n")
    )
}

/// A cart query with no staleness window, listening for `CartChanged` or not.
fn basket(listens: bool) -> String {
    format!(
        "session query Basket(session: Session<SessionId>) -> Result<Cart, CartError>\n    \
         freshness      0.seconds\n    consistency    read_your_writes\n    \
         cache          private\n{}{{\n    Carts.current(session)\n}}\n",
        if listens {
            "    invalidates_on CartChanged(session)\n"
        } else {
            ""
        }
    )
}

/// A command adding to the cart, with `clauses` in its header.
fn add(clauses: &str) -> String {
    format!(
        "command add(item: MenuItemId, quantity: PositiveInt) -> Result<Cart, CartError>\n    \
         requires      SignedIn\n    idempotent_by InteractionId\n{clauses}{{\n    \
         Carts.add(current_session(), item, quantity)\n}}\n"
    )
}

/// A second cart query, which does listen for `CartChanged`.
fn tally() -> String {
    "session query Tally(session: Session<SessionId>) -> Bool\n    \
     freshness      0.seconds\n    consistency    read_your_writes\n    \
     cache          private\n    invalidates_on CartChanged(session)\n{\n    \
     Carts.is_empty(session)\n}\n"
        .to_string()
}

/// A second command, clearing the cart and declaring nothing.
fn empty() -> String {
    "command empty() -> Result<Cart, CartError>\n    requires      SignedIn\n    \
     idempotent_by InteractionId\n{\n    Carts.clear(current_session())\n}\n"
        .to_string()
}

/// A public store query that may be `freshness` stale.
fn shop(freshness: &str) -> String {
    format!(
        "public query Shop(id: StoreId) -> Result<Store, StoreError>\n    \
         freshness      {freshness}\n    consistency    snapshot\n    \
         cache          shared\n    key            id\n{{\n    Stores.get(id)\n}}\n"
    )
}

/// A command renaming a store, through a write the program declares.
fn rename() -> String {
    "fn rename(id: StoreId) -> Result<Store, StoreError> !{ database.write<Stores> }\n\n\
     command rename_shop(id: StoreId) -> Result<Store, StoreError>\n    \
     requires      SignedIn\n    idempotent_by InteractionId\n{\n    rename(id)\n}\n"
        .to_string()
}

#[test]
fn a_command_invalidates_what_it_writes() {
    let found = reported(&[("t.pw", module("t", &[basket(false), add("")]))]);
    assert_eq!(
        found,
        ["PW5106 `add` writes `Carts` and does not invalidate `Basket`, which reads it"],
    );
    // The controls: naming the entry, and emitting an event it listens for.
    let found = reported(&[(
        "t.pw",
        module(
            "t",
            &[
                basket(false),
                add("    invalidates   Basket(current_session())\n"),
            ],
        ),
    )]);
    assert!(found.is_empty(), "{found:#?}");
    let found = reported(&[(
        "t.pw",
        module(
            "t",
            &[
                basket(true),
                add("    emits         CartChanged(current_session())\n"),
            ],
        ),
    )]);
    assert!(found.is_empty(), "{found:#?}");
    // A clause naming nothing is its own defect, and reported once, by
    // PW5100: what it meant to invalidate is not known.
    let found = reported(&[(
        "t.pw",
        module(
            "t",
            &[
                basket(false),
                add("    invalidates   Baskett(current_session())\n"),
            ],
        ),
    )]);
    assert_eq!(
        found,
        ["PW5100 `add` invalidates `Baskett`, which nothing declares"],
    );
    // So is one naming an event, by PW5103.
    let found = reported(&[(
        "t.pw",
        module(
            "t",
            &[
                basket(false),
                add("    invalidates   CartChanged(current_session())\n"),
            ],
        ),
    )]);
    assert_eq!(
        found,
        ["PW5103 `add` invalidates `CartChanged`, which is an event, not a resource"],
    );
}

#[test]
fn an_event_the_reader_does_not_hear_invalidates_nothing() {
    // `Tally` hears it, and that says nothing about `Basket`.
    let found = reported(&[(
        "t.pw",
        module(
            "t",
            &[
                basket(false),
                tally(),
                add("    emits         CartChanged(current_session())\n"),
            ],
        ),
    )]);
    assert_eq!(
        found,
        ["PW5106 `add` writes `Carts` and does not invalidate `Basket`, which reads it"],
    );
}

#[test]
fn a_command_answers_for_its_own_write() {
    // `add` reaches `Basket`, and `empty`, which writes the same cart, does not.
    let found = reported(&[(
        "t.pw",
        module(
            "t",
            &[
                basket(false),
                add("    invalidates   Basket(current_session())\n"),
                empty(),
            ],
        ),
    )]);
    assert_eq!(
        found,
        ["PW5106 `empty` writes `Carts` and does not invalidate `Basket`, which reads it"],
    );
}

#[test]
fn the_write_is_underlined_and_the_reader_named() {
    let src = module("t", &[basket(false), add("")]);
    let files = [("t.pw", src.clone())];
    let d = check_sources(&program(&files))
        .into_iter()
        .find(|(n, _)| n == "t.pw")
        .expect("t.pw")
        .1
        .into_iter()
        .find(|d| d.code == "PW5106")
        .expect("PW5106");
    assert_eq!(
        &src[d.primary_span.clone()],
        "Carts.add(current_session(), item, quantity)"
    );
    let labels: Vec<&str> = d.related.iter().map(|r| r.label.as_str()).collect();
    assert_eq!(labels, ["`add` is the command", "`Basket` reads `Carts`"]);
    assert!(src[d.related[1].span.clone()].starts_with("session query Basket"));
    let repairs: Vec<&str> = d.repairs.iter().map(|r| r.description.as_str()).collect();
    assert_eq!(
        repairs,
        [
            "declare `invalidates Basket(..)` on `add`",
            "or emit an event, and have `Basket` listen for it with `invalidates_on`",
        ]
    );
}

#[test]
fn a_reader_in_another_module_is_reached_too() {
    let found = reported(&[
        ("q.pw", module("q", &[basket(false)])),
        ("t.pw", module("t", &[add("")])),
    ]);
    assert_eq!(
        found,
        ["PW5106 `add` writes `Carts` and does not invalidate `q.Basket`, which reads it"],
    );
    // The control: the event reaches it without naming it.
    let found = reported(&[
        ("q.pw", module("q", &[basket(true)])),
        (
            "t.pw",
            module(
                "t",
                &[add("    emits         CartChanged(current_session())\n")],
            ),
        ),
    ]);
    assert!(found.is_empty(), "{found:#?}");
}

#[test]
fn a_staleness_window_is_the_readers_declaration() {
    let found = reported(&[("t.pw", module("t", &[shop("5.minutes"), rename()]))]);
    assert!(found.is_empty(), "{found:#?}");
    // The control: with no window, the store's name stays as it was.
    let found = reported(&[("t.pw", module("t", &[shop("0.seconds"), rename()]))]);
    assert_eq!(
        found,
        ["PW5106 `rename_shop` writes `Stores` and does not invalidate `Shop`, which reads it"],
    );
}

#[test]
fn a_write_reaches_the_readers_of_its_domain_only() {
    let found = reported(&[("t.pw", module("t", &[shop("0.seconds"), add("")]))]);
    assert!(found.is_empty(), "{found:#?}");
    // The control: a reader of the cart.
    let found = reported(&[(
        "t.pw",
        module("t", &[shop("0.seconds"), basket(false), add("")]),
    )]);
    assert_eq!(
        found,
        ["PW5106 `add` writes `Carts` and does not invalidate `Basket`, which reads it"],
    );
}
