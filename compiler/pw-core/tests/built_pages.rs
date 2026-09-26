//! **What is built before any request reads no request's value** (ADR-0114).
//!
//! `placement build` makes a declaration's output a file produced before any
//! request exists, and served as it is to every one. A parameter is supplied
//! by a request. Until 2026-09-26 a build-placed page rendering its `id`
//! checked. Each test states one case, with a control.

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

/// A store page placed at `world`, whose heading is `heading`.
fn page(world: &str, heading: &str) -> String {
    format!(
        "module t\n\nimport domain.{{ StoreId }}\n\n\
         page ShopInfo(id: StoreId) {{\n    placement {world}\n\n    view {{\n        \
         <main>\n            <h1>{heading}</h1>\n        </main>\n    }}\n}}\n"
    )
}

#[test]
fn a_built_page_reads_no_parameter() {
    // Read twice, reported once.
    assert_eq!(
        reported(&page("build", "Store {id}, {id}")),
        [
            "PW5026 `ShopInfo` is built before any request, and reads `id`, which a request \
          supplies"
        ],
    );
    // The controls: built without reading it, and read where requests are.
    let found = reported(&page("build", "Store"));
    assert!(found.is_empty(), "{found:#?}");
    let found = reported(&page("origin", "Store {id}"));
    assert!(found.is_empty(), "{found:#?}");
}
