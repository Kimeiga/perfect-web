//! **A stream is compiled, and a host is told what to run for it** (ADR-0148).
//!
//! Until 2026-10-03 the template IR refused a `<stream>` (ADR-0075), so a
//! page with one checked and did not build. It is now a part: its query as a
//! component, its arguments as a host computes them, whether the document
//! waits for it, and its placeholder and settled arms. The page plan names
//! each one, with the query's policies.

use pw_core::check::Unit;
use pw_core::lower::lower_file;
use pw_core::template_ir::{Chunk, Part};
use pw_syntax::parse_tree;

/// The store, its `app.pw` changed by `change`, built.
fn build(change: impl Fn(&str) -> String) -> pw_core::build::Build {
    try_build(change).unwrap_or_else(|e| panic!("{e:?}"))
}

/// The store, its `app.pw` changed by `change`, built or refused. The
/// benchmark's store, which streams nothing and does not change (ADR-0156):
/// these tests add a stream to it. The canonical store has two since
/// ADR-0165, which `the_store_streams_its_estimate_and_its_recommendations`
/// reads.
fn try_build(change: impl Fn(&str) -> String) -> Result<pw_core::build::Build, String> {
    try_build_in("benchmarks/baselines/pleris", change)
}

/// [`try_build`], the store read from `base`, under the repository's root.
fn try_build_in(
    base: &str,
    change: impl Fn(&str) -> String,
) -> Result<pw_core::build::Build, String> {
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    let base = root.join(base);
    let mut units = Vec::new();
    let mut add = |path: std::path::PathBuf, change: &dyn Fn(&str) -> String| {
        let src = change(&std::fs::read_to_string(&path).expect("read"));
        units.push(Unit {
            path: path.display().to_string(),
            hir: lower_file(&src, &parse_tree(&src).green),
            src,
        });
    };
    let same = |s: &str| s.to_string();
    for dir in [
        root.join("packages/pw-std"),
        root.join("packages/pw-platform-web"),
        base.join("lib"),
        base.join("store"),
    ] {
        let mut paths: Vec<std::path::PathBuf> = std::fs::read_dir(&dir)
            .unwrap_or_else(|e| panic!("{}: {e}", dir.display()))
            .map(|e| e.expect("entry").path())
            .filter(|p| p.extension().is_some_and(|x| x == "pw"))
            .collect();
        paths.sort();
        for p in paths {
            if p.ends_with("store/app.pw") {
                add(p, &change);
            } else {
                add(p, &same);
            }
        }
    }
    add(base.join("domain.pw"), &same);
    pw_core::build::build(&units)
}

const QUERY: &str =
    "public query Recommendations(id: StoreId) -> Result<List<Recommendation>, RecommendationError>
    freshness     0.seconds
    consistency   eventual
    cache         shared
    key           id
    concurrency   one_per_key
    on_key_change cancel
    delivery      streamed
    timeout       3.seconds
{
    Recommender.for_store(id)
}

";

const SECTION: &str = "
            <section aria-label=\"Recommendations\">
                <stream query={Recommendations(id)}>
                    <placeholder><p>Finding recommendations</p></placeholder>
                    <ready as={items}>
                        <ul>{#each items as item (item.id)}<li>{item.name}</li>{/each}</ul>
                    </ready>
                    <failed as={_why}><p>No recommendations right now</p></failed>
                </stream>
            </section>
        </main>";

/// The store with recommendations streamed below its cart, `section` changed
/// by `markup` and the query's declaration by `query`.
fn with_stream(
    app: &str,
    query: &dyn Fn(&str) -> String,
    markup: &dyn Fn(&str) -> String,
) -> String {
    app.replacen("import Menus\n", "import Menus\nimport Recommender\n", 1)
        .replacen(
            "MenuItemId, PositiveInt }",
            "MenuItemId, PositiveInt, Recommendation, RecommendationError }",
            1,
        )
        .replacen(
            "public query Store(",
            &format!("{}public query Store(", query(QUERY)),
            1,
        )
        .replacen("\n        </main>", &markup(SECTION), 1)
}

fn same(s: &str) -> String {
    s.to_string()
}

fn store_page(b: &pw_core::build::Build) -> &pw_core::template_ir::Template {
    b.templates
        .iter()
        .find(|t| t.path == "store.page.StorePage")
        .expect("the store page's template")
}

fn plan_of(b: pw_core::build::Build) -> Result<pw_core::page_values::PageValues, String> {
    b.pages
        .into_iter()
        .find(|p| p.page == "store.page.StorePage")
        .expect("the store page")
        .plan
}

/// **The canonical store's slots** (charter §15.3, ADR-0165): its delivery
/// estimate, the session's and kept for no one, and its recommendations,
/// public and kept ten minutes. Both are streamed, and each is bounded.
#[test]
fn the_store_streams_its_estimate_and_its_recommendations() {
    let b = try_build_in("examples", |app| app.to_string()).expect("the store builds");
    let planned = plan_of(b).expect("planned");
    let streams: Vec<(&str, &[String], &str, u64, bool)> = planned
        .streams
        .iter()
        .map(|s| {
            (
                s.resource.as_str(),
                s.args.as_slice(),
                s.policy.cache.as_str(),
                s.policy.freshness_ms,
                s.streamed,
            )
        })
        .collect();
    assert_eq!(
        streams,
        [
            (
                "store.page.Estimate",
                ["current_session()".to_string()].as_slice(),
                "private",
                0,
                true
            ),
            (
                "store.page.Recommendations",
                ["id".to_string()].as_slice(),
                "shared",
                600_000,
                true
            ),
        ]
    );
    assert!(
        planned
            .streams
            .iter()
            .all(|s| s.policy.timeout_ms == Some(3000))
    );
    // The document waits for neither: no binding reads them.
    assert!(
        !planned
            .bindings
            .iter()
            .any(|b| b.resource.ends_with("Estimate") || b.resource.ends_with("Recommendations"))
    );
}

#[test]
fn a_stream_is_a_part_with_its_query_and_its_arms() {
    let b = build(|app| with_stream(app, &same, &same));
    let page = store_page(&b);
    let stream = page
        .chunks
        .iter()
        .find_map(|c| match c {
            Chunk::Dynamic(p @ Part::Stream { .. }) => Some(p),
            _ => None,
        })
        .expect("a stream part at the top of the page");
    let Part::Stream {
        query,
        args,
        streamed,
        placeholder,
        ready,
        failed,
        ..
    } = stream
    else {
        unreachable!()
    };
    assert_eq!(query, "store.page.Recommendations");
    assert_eq!(args, &["id".to_string()]);
    assert!(streamed);
    assert!(
        placeholder
            .iter()
            .any(|c| matches!(c, Chunk::Static(s) if s.contains("Finding recommendations")))
    );
    assert_eq!(ready.binding.as_deref(), Some("items"));
    assert_eq!(failed.binding.as_deref(), Some("_why"));
    // The manifest names it, so the browser knows the range is a stream's.
    assert!(
        page.manifest()
            .iter()
            .any(|e| e.kind == "stream" && e.value == "store.page.Recommendations")
    );
}

#[test]
fn the_plan_names_each_stream_with_its_query_s_policies() {
    let planned = plan_of(build(|app| with_stream(app, &same, &same))).expect("planned");
    let [s] = planned.streams.as_slice() else {
        panic!("one stream: {:?}", planned.streams);
    };
    assert_eq!(s.resource, "store.page.Recommendations");
    assert_eq!(s.args, ["id"]);
    assert!(s.streamed);
    assert_eq!(s.policy.timeout_ms, Some(3000));
    assert_eq!(s.policy.cache, "shared");
    // Its region's parts are not the page's: nothing in it is planned as a
    // part, a collection or a block, since the stream's arm renders them.
    assert!(!planned.collections.iter().any(|c| c == "items"));
    // The store as it is has none.
    let plain = plan_of(build(same)).expect("planned");
    assert!(plain.streams.is_empty());
}

#[test]
fn one_the_page_waits_for_is_not_streamed() {
    let at_once = |q: &str| q.replace("    delivery      streamed\n", "");
    let without = |m: &str| {
        m.replace(
            "<placeholder><p>Finding recommendations</p></placeholder>",
            "",
        )
    };
    let planned = plan_of(build(|app| with_stream(app, &at_once, &without))).expect("planned");
    assert!(!planned.streams[0].streamed);
}

#[test]
fn a_stream_inside_a_block_is_not_built_yet() {
    let nested = |m: &str| {
        m.replacen("<stream query", "{#if cart.lines}<stream query", 1)
            .replacen("</stream>", "</stream>{/if}", 1)
    };
    let Err(refused) = try_build(|app| with_stream(app, &same, &nested)) else {
        panic!("refused at build");
    };
    assert!(
        refused.contains("a `<stream>` inside a block or a loop's row is not rendered yet"),
        "{refused}"
    );
}

#[test]
fn a_fallback_no_host_executes_is_refused_by_the_plan() {
    let fallback = |q: &str| q.replace("    delivery ", "    fallback      empty\n    delivery ");
    let refused = plan_of(build(|app| with_stream(app, &fallback, &same))).expect_err("refused");
    assert!(refused.contains("`fallback empty`"), "{refused}");
}

#[test]
fn a_signal_shown_in_a_stream_is_refused_by_the_plan() {
    let signal = |app: &str| {
        with_stream(app, &same, &|m: &str| {
            m.replace("<p>No recommendations right now</p>", "<p>{note}</p>")
        })
        .replacen(
            "    let cart = query Cart(current_session())",
            "    let cart = query Cart(current_session())\n    signal note: String = \"none\"",
            1,
        )
    };
    let refused = plan_of(build(signal)).expect_err("refused");
    assert!(refused.contains("shows the signal `note`"), "{refused}");
}

#[test]
fn a_page_that_streams_a_query_does_not_take_its_authority() {
    // The host runs a stream's query as its own component, under its own
    // contract, as it runs a query a `let` reads. Until 2026-10-03 the page
    // was asked the query's `network.fetch` to render, which `pw diff`
    // showed on T05's reference.
    let b = build(|app| with_stream(app, &same, &same));
    let needs = |id: &str| -> Vec<String> {
        b.contracts
            .iter()
            .find(|c| c.component_id == id)
            .unwrap_or_else(|| panic!("no contract `{id}`"))
            .required_capabilities
            .iter()
            .map(|c| format!("{}.{}", c.family, c.operation))
            .collect()
    };
    assert!(
        !needs("store.page.StorePage")
            .iter()
            .any(|c| c == "network.fetch"),
        "{:?}",
        needs("store.page.StorePage")
    );
    assert!(
        needs("store.page.Recommendations")
            .iter()
            .any(|c| c == "network.fetch")
    );
}
