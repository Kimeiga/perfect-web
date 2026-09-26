//! **A timeout is a budget above zero** (ADR-0109).
//!
//! A request is given its `timeout` and ends when it is spent: the resource
//! runtime expires a flight once `now >= started + timeout`. Until
//! 2026-09-26 `timeout 0.seconds` checked, being a duration, and every
//! request ended before it started, so the query could never answer.
//! `freshness 0.seconds` is another thing: a promise never to serve a stale
//! value, which the store's cart makes.

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

/// A store query given `timeout` and `freshness`.
fn query(timeout: &str, freshness: &str) -> String {
    format!(
        "module t\n\nimport Stores\nimport domain.{{ Store, StoreError, StoreId }}\n\n\
         public query Shop(id: StoreId) -> Result<Store, StoreError>\n    \
         freshness      {freshness}\n    consistency    snapshot\n    cache          shared\n    \
         key            id\n    timeout        {timeout}\n{{\n    Stores.get(id)\n}}\n"
    )
}

#[test]
fn a_timeout_is_a_budget_above_zero() {
    for zero in ["0.seconds", "0.minutes"] {
        assert_eq!(
            reported(&query(zero, "30.seconds")),
            [format!(
                "PW0335 `timeout {zero}`: `{zero}` is no time: every request would end \
                 before it starts"
            )],
        );
    }
    // The controls: a budget, and a freshness of zero, which is a promise.
    let found = reported(&query("2.seconds", "30.seconds"));
    assert!(found.is_empty(), "{found:#?}");
    let found = reported(&query("2.seconds", "0.seconds"));
    assert!(found.is_empty(), "{found:#?}");
}
