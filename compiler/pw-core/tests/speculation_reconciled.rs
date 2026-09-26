//! **A command invalidates the entry it speculates on** (ADR-0105).
//!
//! ADR-0025: an optimistic transition displays a speculative value, and "the
//! command succeeds → authoritative result reconciles it". Both backends
//! reconcile through the entry: the Marko backend gives a binding its
//! command's result when the command `invalidates` the query, and the dev
//! server's page learns the new value when an event reaches the entry.
//! Until 2026-09-26 a command speculating on `Cart` with neither clause
//! checked, and the speculation stayed on the page as if committed. Each
//! test states one case, with a control.

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

/// A command speculating on the library's `Cart`, with `clauses`. The
/// library's `Cart` reads nothing, so PW5106 has nothing to say about it.
fn add(clauses: &str) -> String {
    format!(
        "module t\n\nimport Carts\nimport Resources.{{ Cart }}\n\
         import context.{{ current_session }}\n\
         import domain.{{ CartError, InteractionId, MenuItemId, PositiveInt }}\n\
         import Events.{{ CartChanged }}\nimport capability.{{ Session, SessionId }}\n\n\
         command add_to_cart(item: MenuItemId, quantity: PositiveInt)\n    \
         -> Result<domain.Cart, CartError>\n    requires      SignedIn\n    \
         idempotent_by InteractionId\n    \
         optimistic    Cart(current_session()) as cart => Carts.with_line(cart, item, quantity)\n\
         {clauses}{{\n    Carts.add(current_session(), item, quantity)\n}}\n"
    )
}

/// A session cart query of this module's own, reading the cart, listening
/// for `CartChanged` or not.
fn basket(listens: bool) -> String {
    format!(
        "\nsession query Basket(session: Session<SessionId>) -> Result<domain.Cart, CartError>\n    \
         freshness      0.seconds\n    consistency    read_your_writes\n    \
         cache          private\n{}{{\n    Carts.current(session)\n}}\n",
        if listens {
            "    invalidates_on CartChanged(session)\n"
        } else {
            ""
        }
    )
}

/// `add(clauses)`, speculating on `Basket` rather than the library's cart.
fn add_to_basket(clauses: &str, listens: bool) -> String {
    add(clauses).replace(
        "    optimistic    Cart(current_session())",
        "    optimistic    Basket(current_session())",
    ) + &basket(listens)
}

#[test]
fn a_speculation_is_reconciled_by_its_command() {
    let found = reported(&add(""));
    assert_eq!(
        found,
        ["PW5107 `add_to_cart` speculates on `Cart` and does not invalidate it"],
    );
    // The control: the command names the entry.
    let found = reported(&add("    invalidates   Cart(current_session())\n"));
    assert!(found.is_empty(), "{found:#?}");
    // A clause naming nothing is its own defect, PW5100's alone.
    let found = reported(&add("    invalidates   Cartt(current_session())\n"));
    assert_eq!(
        found,
        ["PW5100 `add_to_cart` invalidates `Cartt`, which nothing declares"],
    );
}

#[test]
fn an_event_reconciles_only_an_entry_that_hears_it() {
    // The library's `Cart` does not listen for `CartChanged`.
    let found = reported(&add("    emits         CartChanged(current_session())\n"));
    assert_eq!(
        found,
        ["PW5107 `add_to_cart` speculates on `Cart` and does not invalidate it"],
    );
    // `Basket` does.
    let found = reported(&add_to_basket(
        "    emits         CartChanged(current_session())\n",
        true,
    ));
    assert!(found.is_empty(), "{found:#?}");
}

#[test]
fn a_speculated_reader_is_reported_once() {
    // `Basket` reads the cart the command writes, so PW5106 would report it
    // too: one missing clause, one diagnostic.
    let found = reported(&add_to_basket("", false));
    assert_eq!(
        found,
        ["PW5107 `add_to_cart` speculates on `Basket` and does not invalidate it"],
    );
}

#[test]
fn a_command_answers_for_its_own_speculation() {
    // A second command invalidates the cart; this one does not.
    let src = add("")
        + "\ncommand clear_cart() -> Result<domain.Cart, CartError>\n    \
           requires      SignedIn\n    idempotent_by InteractionId\n    \
           invalidates   Cart(current_session())\n{\n    Carts.clear(current_session())\n}\n";
    assert_eq!(
        reported(&src),
        ["PW5107 `add_to_cart` speculates on `Cart` and does not invalidate it"],
    );
}

#[test]
fn the_speculation_is_underlined() {
    let src = add("");
    let d = check_sources(&program(&src))
        .into_iter()
        .find(|(n, _)| n == "t.pw")
        .expect("t.pw")
        .1
        .into_iter()
        .find(|d| d.code == "PW5107")
        .expect("PW5107");
    assert_eq!(&src[d.primary_span.clone()], "Cart(current_session())");
    let labels: Vec<&str> = d.related.iter().map(|r| r.label.as_str()).collect();
    assert_eq!(labels, ["the speculation", "`add_to_cart` is the command"]);
    assert!(src[d.related[0].span.clone()].starts_with("optimistic"));
    let repairs: Vec<&str> = d.repairs.iter().map(|r| r.description.as_str()).collect();
    assert_eq!(
        repairs,
        [
            "declare `invalidates Cart(..)` on `add_to_cart`",
            "or emit an event, and have `Cart` listen for it with `invalidates_on`",
        ]
    );
}
