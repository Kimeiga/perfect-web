//! **A block a query decides is planned, at the top of the page**
//! (ADR-0145, ADR-0146).
//!
//! The page plan states each value a host renders a page from. A block a
//! query's value decides was left out of it: the store with `{#if
//! cart.lines}` checked and built, and the development server failed at its
//! first render. ADR-0145 refused such a block; ADR-0146 plans it, and a host
//! renders it again when what it renders changed. What a loop's row reads of
//! another query would not be rendered again with it, and is refused.

use pw_core::check::Unit;
use pw_core::lower::lower_file;
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
        "examples/lib",
        "examples/store",
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
    add(root.join("examples/domain.pw"), &same);
    pw_core::build::build(&units).expect("builds")
}

/// The store's plan, with `app.pw` changed by `change`.
fn plan(change: impl Fn(&str) -> String) -> Result<pw_core::page_values::PageValues, String> {
    build(change)
        .pages
        .into_iter()
        .find(|p| p.page == "store.page.StorePage")
        .expect("the store page")
        .plan
}

#[test]
fn a_block_a_query_decides_is_planned() {
    let planned = plan(|app| {
        app.replacen(
            "                <p id=\"cart-count\">{cart.line_count}</p>",
            "                <p id=\"cart-count\">{cart.line_count}</p>\n                {#if cart.lines}<p>Ready when you are.</p>{/if}",
            1,
        )
    })
    .expect("planned");
    assert_eq!(planned.blocks.len(), 1, "{:?}", planned.blocks);
    // A block inside one is rendered with it, and is not planned itself.
    let nested = plan(|app| {
        app.replacen(
            "                <p id=\"cart-count\">{cart.line_count}</p>",
            "                <p id=\"cart-count\">{cart.line_count}</p>\n                {#if cart.lines}<div>{#if cart.lines}<p>Ready.</p>{/if}</div>{/if}",
            1,
        )
    })
    .expect("planned");
    assert_eq!(nested.blocks.len(), 1, "{:?}", nested.blocks);
    // Control: the store as it is has none.
    assert_eq!(
        plan(|app| app.to_string()).expect("planned").blocks,
        Vec::<u32>::new()
    );
}

#[test]
fn what_a_row_reads_of_another_query_is_refused() {
    // A menu row that says whether the cart holds anything: rendered again
    // when the menu changes, and not when the cart does.
    let block = plan(|app| {
        app.replacen(
            "                            <span>{item.name}</span>",
            "                            <span>{item.name}</span>{#if cart.lines}<b>!</b>{/if}",
            1,
        )
    });
    let why = block.expect_err("refused");
    assert!(why.contains("reads `cart` inside a loop's row"), "{why}");
    let text = plan(|app| {
        app.replacen(
            "                            <span>{item.name}</span>",
            "                            <span>{item.name} {store.name}</span>",
            1,
        )
    });
    assert!(
        text.expect_err("refused")
            .contains("reads `store` inside a loop's row")
    );
}
