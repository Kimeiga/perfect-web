//! **A cache key names each parameter its entry depends on** (ADR-0107).
//!
//! A query's entry is shared by every call whose key is equal, and a
//! subscription's stream by every call whose `dedupe_by` is. Until 2026-09-26
//! `query Other(id, other)` with `key id` and a body reading `other` checked,
//! and `Other(1, 2)` and `Other(1, 3)` shared one entry: the second call was
//! given the first's answer. PW5004 held a key to the privacy partitions a
//! value depends on, and nothing held it to the parameters. Each test states
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

/// A shared store query of two stores, keyed by `key`, whose body is `body`.
fn query(key: &str, body: &str) -> String {
    format!(
        "module t\n\nimport Stores\nimport domain.{{ Store, StoreError, StoreId }}\n\n\
         public query Other(id: StoreId, other: StoreId) -> Result<Store, StoreError>\n    \
         freshness      30.seconds\n    consistency    snapshot\n    cache          shared\n    \
         key            {key}\n{{\n    {body}\n}}\n"
    )
}

/// An order subscription of two orders, deduplicated by `by`.
fn subscription(by: &str) -> String {
    format!(
        "module t\n\nimport domain.{{ OrderId, OrderProgress, TrackingError }}\n\
         import stream.{{ Stream }}\nimport Tracking\n\n\
         subscription Follow(order: OrderId, other: OrderId)\n    \
         -> Stream<OrderProgress, TrackingError>\n    scope         component\n    \
         transport     websocket\n    \
         reconnect     bounded_exponential(max = 5, jitter = true)\n    \
         dedupe_by     {by}\n    on_scope_exit close\n{{\n    Tracking.follow(other)\n}}\n"
    )
}

#[test]
fn a_key_names_each_parameter_its_body_reads() {
    assert_eq!(
        reported(&query("id", "Stores.get(other)")),
        ["PW0336 `Other`'s `key` omits `other`, which its body reads"],
    );
    // The controls: the key names it, and a body reading only what the key
    // names.
    let found = reported(&query("id, other", "Stores.get(other)"));
    assert!(found.is_empty(), "{found:#?}");
    let found = reported(&query("id", "Stores.get(id)"));
    assert!(found.is_empty(), "{found:#?}");
    // A key naming nothing is PW0335's alone: one defect, one diagnostic.
    assert_eq!(
        reported(&query("store", "Stores.get(other)")),
        ["PW0335 `key store`: `store` is not a parameter of `Other`"],
    );
}

#[test]
fn a_read_on_one_branch_is_a_read() {
    // On one branch only: the entry still depends on `other` there.
    assert_eq!(
        reported(&query(
            "id",
            "if id == other {\n        Stores.get(id)\n    } else {\n        Stores.get(id)\n    }"
        )),
        ["PW0336 `Other`'s `key` omits `other`, which its body reads"],
    );
}

#[test]
fn a_subscription_dedupes_by_what_it_reads() {
    assert_eq!(
        reported(&subscription("order")),
        ["PW0336 `Follow`'s `dedupe_by` omits `other`, which its body reads"],
    );
    let found = reported(&subscription("order, other"));
    assert!(found.is_empty(), "{found:#?}");
}

#[test]
fn the_key_is_underlined_and_the_read_shown() {
    let src = query("id", "Stores.get(other)");
    let d = check_sources(&program(&src))
        .into_iter()
        .find(|(n, _)| n == "t.pw")
        .expect("t.pw")
        .1
        .into_iter()
        .find(|d| d.code == "PW0336")
        .expect("PW0336");
    assert!(src[d.primary_span.clone()].starts_with("key"));
    assert_eq!(&src[d.related[0].span.clone()], "other");
    assert_eq!(d.related[0].label, "`other` is read here");
    let repairs: Vec<&str> = d.repairs.iter().map(|r| r.description.as_str()).collect();
    assert_eq!(repairs, ["key the entry by `other` too: `key id, other`"]);
}
