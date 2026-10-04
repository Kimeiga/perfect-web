//! **An attribute at the top of the page that reads a query's value is set
//! again when the value changes** (ADR-0171).
//!
//! A host sets a text part again when what it reads changes (ADR-0125), a
//! list's rows (ADR-0145), and a block (ADR-0146). Until 2026-10-03 an
//! attribute at the top of a page was in none of these. `hidden={cart.lines}`
//! kept the value the document was rendered with, for its life. The plan now
//! names each such attribute, and a host sets it as it sets a text part.
//!
//! The store is the benchmark's, which does not change (ADR-0156), without
//! `add_to_cart`'s speculation, which these tests are not about. Until
//! ADR-0172 it would not have reached the attribute (ADR-0170).

use pw_core::check::Unit;
use pw_core::lower::lower_file;
use pw_core::page_values::PageValues;
use pw_syntax::parse_tree;

/// The store, its `app.pw` changed by `change`, built.
fn build(change: impl Fn(&str) -> String) -> pw_core::build::Build {
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
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
        "packages/pw-std",
        "packages/pw-platform-web",
        "benchmarks/baselines/pleris/lib",
        "benchmarks/baselines/pleris/store",
    ] {
        let mut paths: Vec<std::path::PathBuf> = std::fs::read_dir(root.join(dir))
            .unwrap_or_else(|e| panic!("{dir}: {e}"))
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
    add(root.join("benchmarks/baselines/pleris/domain.pw"), &same);
    pw_core::build::build(&units).expect("builds")
}

const COUNT: &str = "                <p id=\"cart-count\">{cart.line_count}</p>\n";
const OPTIMISTIC: &str =
    "    optimistic    Cart(current_session()) as cart => Carts.with_line(cart, item, quantity)\n";

/// The store with `markup` after the cart's count, without the speculation.
fn store(markup: &str) -> impl Fn(&str) -> String + '_ {
    move |app: &str| {
        assert_eq!(app.matches(COUNT).count(), 1, "the count's anchor");
        assert_eq!(
            app.matches(OPTIMISTIC).count(),
            1,
            "the speculation's anchor"
        );
        app.replace(OPTIMISTIC, "")
            .replace(COUNT, &format!("{COUNT}                {markup}\n"))
    }
}

fn plan(markup: &str) -> PageValues {
    let b = build(store(markup));
    assert!(b.refusals().is_empty(), "{:?}", b.refusals());
    b.pages
        .into_iter()
        .find(|p| p.page == "store.page.StorePage")
        .expect("the store page")
        .plan
        .expect("planned")
}

/// The id of the part of `kind` that reads `value`, in the store's
/// template, as the parts manifest lists it.
fn part_of(markup: &str, kind: &str, value: &str) -> u32 {
    let b = build(store(markup));
    b.templates
        .iter()
        .find(|t| t.path == "store.page.StorePage")
        .expect("the store's template")
        .manifest()
        .into_iter()
        .find(|e| e.kind == kind && e.value == value)
        .unwrap_or_else(|| panic!("no {kind} part reads `{value}`"))
        .id
        .0
}

#[test]
fn an_attribute_at_the_top_that_reads_a_query_is_planned() {
    let markup = "<p id=\"empty\" hidden={cart.lines}>Your cart is empty.</p>";
    assert_eq!(
        plan(markup).attributes,
        [part_of(markup, "boolean_attribute", "cart.lines")]
    );
}

#[test]
fn one_written_with_several_values_is_planned_once() {
    let markup = "<p title=\"{store.name}, from {store.hours.opens_minute}\">here</p>";
    assert_eq!(
        plan(markup).attributes,
        [part_of(
            markup,
            "interpolated_attribute",
            "store.name store.hours.opens_minute"
        )]
    );
}

#[test]
fn one_a_block_or_a_row_renders_is_not_the_pages_to_set() {
    // A block renders its attributes again whole (ADR-0146), and a row sets
    // its own (ADR-0168). Neither is the page's to set.
    let markup = "{#if cart.lines}<p hidden={cart.lines}>in a block</p>{/if}\
                  <ul>{#each cart.lines as line (line.item_id)}\
                  <li title={line.quantity.count}>a line</li>{/each}</ul>";
    assert!(plan(markup).attributes.is_empty());
}
