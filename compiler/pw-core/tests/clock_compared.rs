//! **The wall clock, read by comparing it** (ADR-XXXX): a value that asks
//! whether an instant has passed, or a zone's date, holds until a known
//! instant, the same for every reader, and the host reads it again then. So
//! one entry may serve every reader; and a page built once, which nothing
//! tells, may not compare it. Each test states one case, with a control.

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

/// Whether a sale that ends at 21:00 UTC today is on.
const ON: &str = "match clock.time(21, 0) {\n        \
                  Some(nine) => !clock.passed(clock.at(clock.today_in(clock.zone(\"UTC\")), nine, clock.zone(\"UTC\"))),\n        \
                  None => false,\n    }";

/// A page placed at `world` that says whether the sale is on.
fn page(world: &str) -> String {
    format!(
        "module t\n\nimport clock\n\n\
         page Sale() !{{ clock.compare, clock.zone }} {{\n    placement {world}\n    cache     private\n    \
         let on = {ON}\n    view {{ <main><p>{{on}}</p></main> }}\n}}\n"
    )
}

#[test]
fn a_page_built_once_may_not_compare_the_clock() {
    let found = reported(&page("build"));
    for code in ["PW5005", "PW0401"] {
        assert!(
            found
                .iter()
                .any(|d| d.starts_with(code) && d.contains("clock.compare")),
            "{code}: {found:#?}"
        );
    }
    // The control: served, it may.
    let found = reported(&page("origin"));
    assert!(found.is_empty(), "{found:#?}");
}

#[test]
fn a_query_that_compares_the_clock_may_be_kept_for_every_reader() {
    // One entry for every reader, held until the instant it compared.
    let query = format!(
        "module t\n\nimport clock\n\n\
         public query Sale() -> Bool\n    freshness   10.minutes\n    consistency snapshot\n    \
         cache       shared\n    concurrency one_per_key\n    timeout     2.seconds\n{{\n    {ON}\n}}\n"
    );
    let found = reported(&query);
    assert!(found.is_empty(), "{found:#?}");
}

#[test]
fn a_public_materialization_may_not_compare_the_clock() {
    // Made again when an event reaches it, and no event says 21:00 passed.
    let fragment = |effect: &str| {
        format!(
            "module t\n\nimport clock\nimport domain.{{ StoreId }}\nimport Resources.{{ Menu }}\n\
             import Events.{{ MenuChanged }}\n\n\
             materialize SaleFragment(id: StoreId) {{\n    placement      origin\n    partition      {effect}\n    \
             depends_on     Menu(id)\n    invalidates_on MenuChanged(id)\n\n    \
             view {{\n        <p>{{clock.today_in(clock.zone(\"UTC\"))}}</p>\n    }}\n}}\n"
        )
    };
    let found = reported(&fragment("public"));
    assert!(
        found
            .iter()
            .any(|d| d.starts_with("PW0401") && d.contains("clock.compare")),
        "{found:#?}"
    );
}
