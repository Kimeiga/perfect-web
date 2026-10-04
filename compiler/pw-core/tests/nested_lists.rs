//! **A loop over a list inside a query's value** (ADR-0170).
//!
//! `{#each cart.lines as line (line.item_id)}` iterates a list inside the
//! cart's value. Until 2026-10-03 the plan recorded the binding, `cart`, as
//! the list, and a host asked for it found a record: the page built, and the
//! development server failed at the cart's first change. A row's member read
//! was planned only for a loop over a binding itself (ADR-0169).
//!
//! And a part a speculation would not reach was refused: the store's `cart`
//! is speculated by `add_to_cart`'s `optimistic` clause (ADR-0122), and a
//! loop over its lines would have shown the lines it held beside the count
//! the speculation showed. Since ADR-0172 the browser renders such a part
//! again, and what it could not render is refused.
//!
//! The store is the benchmark's, which does not change (ADR-0156).

use pw_core::check::Unit;
use pw_core::lower::lower_file;
use pw_core::page_values::{PageValues, RowRead, Step};
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

/// The store with `markup` after the cart's count, and `add_to_cart`'s
/// speculation kept or taken out.
fn store(markup: &str, speculated: bool) -> impl Fn(&str) -> String + '_ {
    move |app: &str| {
        assert_eq!(app.matches(COUNT).count(), 1, "the count's anchor");
        assert_eq!(
            app.matches(OPTIMISTIC).count(),
            1,
            "the speculation's anchor"
        );
        let app = app.replace(COUNT, &format!("{COUNT}                {markup}\n"));
        if speculated {
            app
        } else {
            app.replace(OPTIMISTIC, "")
        }
    }
}

fn plan(build: &pw_core::build::Build) -> PageValues {
    build
        .pages
        .iter()
        .find(|p| p.page == "store.page.StorePage")
        .expect("the store page")
        .plan
        .clone()
        .expect("planned")
}

const LINES: &str = "<ul id=\"lines\">{#each cart.lines as line (line.item_id)}\
                     <li>{line.quantity.count}</li>{/each}</ul>";

#[test]
fn a_list_inside_a_querys_value_is_iterated_by_its_path() {
    let b = build(store(LINES, false));
    assert!(b.refusals().is_empty(), "{:?}", b.refusals());
    let plan = plan(&b);
    assert!(
        plan.collections.contains(&"cart.lines".to_string()),
        "{:?}",
        plan.collections
    );
    assert!(
        !plan.collections.contains(&"cart".to_string()),
        "{:?}",
        plan.collections
    );
    // Its rows read a member of their item, which a host computes for each
    // row (ADR-0169).
    assert_eq!(
        plan.rows,
        [RowRead {
            collection: "cart.lines".into(),
            binding: "line".into(),
            path: "line.quantity.count".into(),
            steps: vec![
                Step::Field("quantity".into()),
                Step::Member("domain.count".into())
            ],
        }]
    );
}

/// The store page's speculation, built with `markup`.
fn speculation(markup: &str) -> pw_core::backend::speculation::Compiled {
    build(store(markup, true))
        .speculations
        .into_iter()
        .find(|s| s.page == "store.page.StorePage")
        .expect("the store page speculates")
}

/// The id of the part of `kind` that reads `value`, as the parts manifest
/// lists it, in the store with `markup`.
fn part_of(markup: &str, kind: &str, value: &str) -> u32 {
    build(store(markup, true))
        .templates
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
fn a_part_that_reads_a_speculated_value_is_rendered_again() {
    // ADR-0170 refused each of these, as a part the speculation would not
    // reach. ADR-0172 renders each again in the browser, from the speculated
    // value: a region.
    for (markup, kind, value) in [
        (LINES, "each", "cart.lines"),
        (
            "{#if cart.lines}<p>In your cart</p>{/if}",
            "conditional",
            "cart.lines",
        ),
        (
            "<p hidden={cart.lines}>Your cart is empty.</p>",
            "boolean_attribute",
            "cart.lines",
        ),
    ] {
        let b = build(store(markup, true));
        assert!(b.refusals().is_empty(), "{markup}: {:?}", b.refusals());
        let s = speculation(markup);
        let pw_core::backend::wasm::Encoding::Encoded(m) = &s.module else {
            panic!("{markup}: {}", s.module);
        };
        assert_eq!(m.regions, [part_of(markup, kind, value)], "{markup}");
    }
}

#[test]
fn a_region_that_reads_what_the_browser_does_not_hold_is_refused() {
    // The browser renders a region from the speculated value, the page's
    // signals and the names bound inside it, as it renders a signal's block
    // (ADR-0137). The store's name is none of them.
    for (markup, what) in [
        (
            "<ul>{#each cart.lines as line (line.item_id)}\
             <li>{line.quantity.count} from {store.name}</li>{/each}</ul>",
            "reads `store.name` inside part",
        ),
        (
            "{#if cart.lines}<p title={store.name}>In your cart</p>{/if}",
            "reads `store.name` inside part",
        ),
    ] {
        let refused = build(store(markup, true)).refusals();
        assert!(
            refused
                .iter()
                .any(|r| r.starts_with("`store.page.StorePage`'s speculations:")
                    && r.contains(
                        "a value a speculated region reads that the browser does not hold"
                    )
                    && r.contains(what)),
            "{markup}: {refused:#?}"
        );
    }
}

#[test]
fn a_region_whose_handler_captures_what_the_browser_does_not_hold_is_refused() {
    // The region writes what each handler in it captures. The handler sets a
    // signal of the page's to the store's name, which the browser does not
    // hold.
    const LET: &str = "    let cart = query Cart(current_session())\n";
    let markup = "{#if cart.lines}<button type=\"button\" \
                  on:press={() => picked = store.name}>Name</button>{/if}";
    let refused = build(|app: &str| {
        assert_eq!(app.matches(LET).count(), 1, "the cart's anchor");
        store(markup, true)(app).replace(LET, &format!("{LET}    signal picked: String = \"\"\n"))
    })
    .refusals();
    assert!(
        refused
            .iter()
            .any(|r| r.starts_with("`store.page.StorePage`'s speculations:")
                && r.contains("a value a speculated region reads that the browser does not hold")
                && r.contains("captures `store.name` inside part")),
        "{refused:#?}"
    );
}

#[test]
fn a_speculated_value_read_in_a_block_no_region_renders_is_refused() {
    // Inside the menu's rows, which read the menu and not the cart: nothing
    // would render it again.
    let markup = "<ul>{#each menu as item (item.id)}\
                  <li hidden={cart.lines}>{item.name}</li>{/each}</ul>";
    let refused = build(store(markup, true)).refusals();
    assert!(
        refused
            .iter()
            .any(|r| r.starts_with("`store.page.StorePage`'s speculations:")
                && r.contains(
                    "a speculated value read where this module does not render it again"
                )
                && r.contains("reads `cart.lines` inside a block")),
        "{refused:#?}"
    );
}

#[test]
fn the_count_alone_is_speculated_as_before() {
    // The control: the store as it is, its count a text part the
    // speculation sets.
    let b = build(|app: &str| app.to_string());
    assert!(b.refusals().is_empty(), "{:?}", b.refusals());
    assert!(plan(&b).collections.iter().all(|c| c != "cart"));
}
