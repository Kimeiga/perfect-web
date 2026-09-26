//! **A call's privacy is the declaration it resolves to** (ADR-0112).
//!
//! The privacy rules read a callee's label, and its sink level, by the
//! callee's fully qualified spelling. Until 2026-09-26 `payments()` after
//! `import secrets.{ payments }` carried no label where `secrets.payments()`
//! did: a secret reached markup (PW5003), a public log took one (PW5006),
//! and a shared cache left the session out of its key (PW5004), each with
//! `pw check` passing. Each test states both spellings, and a control.

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

/// R-003's page: a payments key rendered into markup.
fn secret_page(import: &str, call: &str) -> String {
    format!(
        "module t\n\nimport {import}\nimport capability.{{ Secret, Payments }}\n\n\
         page Checkout() {{\n    placement origin\n\n    \
         let key: Secret<Payments> = {call}\n\n    view {{\n        \
         <pay-button api_key={{key}} />\n    }}\n}}\n"
    )
}

/// R-006's function: a payments token written to a log through `sink`.
fn logging(import: &str, sink: &str) -> String {
    format!(
        "module t\n\nimport {import}\nimport secrets\n\
         import capability.{{ Payments, Public }}\nimport domain.{{ OrderId }}\n\n\
         fn trace_capture(order: OrderId) -> () !{{ log<Public>, secret<Payments> }} {{\n    \
         let token = secrets.payments()\n    {sink}(\"capturing with token {{token}}\")\n}}\n"
    )
}

/// A shared, unkeyed query of the asking session's cart.
fn mine(import: &str, call: &str) -> String {
    format!(
        "module t\n\nimport Carts\nimport {import}\nimport domain.{{ Cart, CartError }}\n\n\
         public query Mine() -> Result<Cart, CartError>\n    \
         freshness      0.seconds\n    consistency    snapshot\n    cache          shared\n\
         {{\n    Carts.current({call})\n}}\n"
    )
}

#[test]
fn a_secret_is_one_however_it_is_imported() {
    for (import, call) in [
        ("secrets", "secrets.payments()"),
        ("secrets.{ payments }", "payments()"),
    ] {
        assert_eq!(
            reported(&secret_page(import, call)),
            ["PW5003 `key` is Secret<Payments> and is rendered into markup"],
            "{call}"
        );
    }
}

#[test]
fn a_sink_is_one_however_it_is_imported() {
    for (import, sink) in [("log", "log.public"), ("log.{ public }", "public")] {
        assert_eq!(
            reported(&logging(import, sink)),
            ["PW5006 cannot log a `Secret<Payments>` value at privacy level `Public`"],
            "{sink}"
        );
    }
    // The control: a function of the module's own, named `public`, is not the
    // log's sink, which no spelling makes it.
    let own = "module t\n\nimport secrets\nimport capability.{ Payments }\n\
               import domain.{ OrderId }\n\n\
               fn public(message: String) -> () !{} {\n    ()\n}\n\n\
               fn trace_capture(order: OrderId) -> () !{ secret<Payments> } {\n    \
               let token = secrets.payments()\n    public(\"capturing with token {token}\")\n}\n";
    let found = reported(own);
    assert!(found.is_empty(), "{found:#?}");
}

#[test]
fn a_session_read_is_one_however_it_is_imported() {
    for (import, call) in [
        ("context", "context.current_session()"),
        ("context.{ current_session }", "current_session()"),
    ] {
        assert_eq!(
            reported(&mine(import, call)),
            ["PW5004 `Mine` is Session<SessionId> but its shared cache key omits session"],
            "{call}"
        );
    }
}

#[test]
fn a_value_refused_a_shared_cache_is_reported_once() {
    // A page reading a `session query` in a shared cache is PW5001's, whose
    // repairs include partitioning the cache. Read with the session's label
    // as well, it was PW5004's too: two diagnostics for one defect, with the
    // qualified spelling before ADR-0112 and with either after it.
    for (import, call) in [
        ("context", "context.current_session()"),
        ("context.{ current_session }", "current_session()"),
    ] {
        let src = format!(
            "module t\n\nimport Carts\nimport {import}\n\
             import domain.{{ Cart, CartError, StoreId }}\n\
             import capability.{{ Session, SessionId }}\n\n\
             session query Basket(session: Session<SessionId>) -> Result<Cart, CartError>\n    \
             freshness      0.seconds\n    consistency    read_your_writes\n    \
             cache          private\n{{\n    Carts.current(session)\n}}\n\n\
             page ShopPage(id: StoreId) {{\n    cache shared\n\n    \
             let basket = query Basket({call})\n\n    view {{ <main /> }}\n}}\n"
        );
        assert_eq!(
            reported(&src),
            ["PW5001 `ShopPage` is Session<SessionId> and declares a shared cache"],
            "{call}"
        );
    }
}
