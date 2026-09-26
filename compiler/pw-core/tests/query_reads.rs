//! **A query reads** (ADR-0100).
//!
//! Charter §7.5: a `query` is a "keyed remote read with cache, freshness,
//! lifecycle, cancellation, and dedupe", and a `command` is the "explicit
//! mutation with authorization, idempotency, transaction, optimistic
//! behavior, and invalidation". Until 2026-09-26 a query could write: one
//! that cleared a cart checked, cached for thirty seconds, so the write
//! happened once for every reader of its entry, and again on every retry.
//! Each test states one case, with a control.

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

/// A declaration of `kind`, named `N`, whose body clears or reads the cart.
fn declared(kind: &str, row: &str, body: &str) -> String {
    format!(
        "module t\n\nimport Carts\nimport context.{{ current_session }}\n\
         import domain.{{ Cart, CartError, InteractionId }}\n\n\
         {kind} N() -> Result<Cart, CartError> !{{ session.read, {row} }}\n    \
         freshness 0.seconds\n    cache private\n{{\n    {body}\n}}\n"
    )
}

#[test]
fn a_query_writes_nothing() {
    let found = reported(&declared(
        "session query",
        "database.write<Carts>",
        "Carts.clear(current_session())",
    ));
    assert_eq!(
        found,
        vec!["PW0401 `N` performs `database.write<Carts>`, which a query may not do"],
    );
    // The control: reading.
    let found = reported(&declared(
        "session query",
        "database.read<Carts>",
        "Carts.current(current_session())",
    ));
    assert!(found.is_empty(), "{found:#?}");
}
