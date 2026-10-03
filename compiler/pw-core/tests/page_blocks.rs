//! **A block a query decides is refused by the plan, for now** (ADR-0145).
//!
//! The page plan states each value a host renders a page from. A block a
//! query's value decides was left out of it, so a host had no value for its
//! subject: the store with `{#if cart.lines}` checked and built, and the
//! development server failed at its first render. The plan refuses such a
//! block now, naming it, so `pw build` says so.

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

#[test]
fn a_block_a_query_decides_is_refused_by_the_plan() {
    let built = build(|app| {
        app.replacen(
            "                <p id=\"cart-count\">{cart.line_count}</p>",
            "                <p id=\"cart-count\">{cart.line_count}</p>\n                {#if cart.lines}<p>Ready when you are.</p>{/if}",
            1,
        )
    });
    let refused = built.refusals();
    assert!(
        refused
            .iter()
            .any(|r| r.contains("`store.page.StorePage`'s values")
                && r.contains("a block `cart.lines` decides, a query's value")),
        "{refused:#?}"
    );
    // Control: the store as it is plans.
    assert_eq!(
        build(|app| app.to_string()).refusals(),
        Vec::<String>::new()
    );
}
