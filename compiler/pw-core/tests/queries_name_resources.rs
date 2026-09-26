//! **A query names a resource that exists** (ADR-0108).
//!
//! `let menu = query Menu(id)` reads the resource `Menu`. The name check
//! bound a keyword statement's first word as if the statement declared it,
//! so until 2026-09-26 `query Nonexistent(id)` in a page checked: nothing
//! resolved the name, and the graph kept the read as a dangling edge that no
//! rule reports for a page. Each test states one case, with a control.

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

/// A page reading the store's menu through `read`.
fn page(read: &str) -> String {
    format!(
        "module t\n\nimport Resources.{{ Menu }}\n\
         import domain.{{ StoreId }}\n\n\
         page ShopPage(id: StoreId) {{\n    placement origin\n\n    \
         let menu = {read}\n\n    view {{\n        <main />\n    }}\n}}\n"
    )
}

#[test]
fn a_query_names_a_resource_that_exists() {
    assert_eq!(
        reported(&page("query Nonexistent(id)")),
        ["PW0021 `Nonexistent` does not resolve"],
    );
    // The control: the menu the library declares.
    let found = reported(&page("query Menu(id)"));
    assert!(found.is_empty(), "{found:#?}");
}

#[test]
fn a_subscription_names_one_that_exists() {
    assert_eq!(
        reported(&page("subscription Nothing(id)")),
        ["PW0021 `Nothing` does not resolve"],
    );
}
