//! **What a handler at the top of the page captures is set again when it
//! changes** (ADR-0217, ADR-0210's urgent defect 2).
//!
//! A row's captures were set again with the row (ADR-0172). A handler at the
//! top of the page was in no plan: a button on the cart's page that captured
//! the cart kept the cart the page was first rendered with, and a press after
//! a commit, or during a speculation, sent that. The plan lists it now, for
//! the server, and the page's speculation renders it again in the browser.

use pw_core::check::Unit;
use pw_core::lower::lower_file;
use pw_syntax::parse_tree;

/// The store, its cart's page given a button that captures the cart.
fn store(keep: bool) -> Vec<Unit> {
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    let mut out = Vec::new();
    for dir in [
        "packages/pw-std",
        "packages/pw-platform-web",
        "examples/lib",
        "examples/store",
    ] {
        let mut files: Vec<std::path::PathBuf> = std::fs::read_dir(root.join(dir))
            .unwrap_or_else(|e| panic!("{dir}: {e}"))
            .map(|e| e.expect("entry").path())
            .filter(|p| p.extension().is_some_and(|x| x == "pw"))
            .collect();
        files.sort();
        for p in files {
            let mut src = std::fs::read_to_string(&p).expect("read");
            if keep && p.ends_with("examples/store/app.pw") {
                let page = "    signal notice: String = \"\"\n\n    view {\n        <title>Your cart</title>\n        <main>\n";
                assert!(src.contains(page), "the cart's page");
                src = src
                    .replacen("import domain.{ ", "import domain.{ CartLine, ", 1)
                    .replacen(
                        page,
                        "    signal notice: String = \"\"\n    signal kept: List<CartLine> = []\n\n    view {\n        <title>Your cart</title>\n        <main>\n            <button type=\"button\" id=\"keep\" on:press={resumable(captures = { cart }) => kept = cart.lines}>Keep</button>\n",
                        1,
                    );
            }
            out.push((p.display().to_string(), src));
        }
    }
    out.push((
        "domain.pw".to_string(),
        std::fs::read_to_string(root.join("examples/domain.pw")).expect("domain.pw"),
    ));
    out.into_iter()
        .map(|(path, src)| Unit {
            hir: lower_file(&src, &parse_tree(&src).green),
            path,
            src,
        })
        .collect()
}

/// The cart page's planned captures, and its speculation's regions and module.
fn cart_page(keep: bool) -> (Vec<u32>, Vec<u32>, String) {
    let build = pw_core::build::build(&store(keep)).expect("builds");
    let plan = build
        .pages
        .into_iter()
        .find(|p| p.page == "store.page.CartPage")
        .expect("the cart's page")
        .plan
        .expect("planned");
    let speculation = build
        .speculations
        .into_iter()
        .find(|s| s.page == "store.page.CartPage")
        .expect("the cart's speculation");
    let pw_core::backend::wasm::Encoding::Encoded(module) = speculation.module else {
        panic!("the cart's speculation compiles");
    };
    (plan.captures, module.regions, module.source)
}

#[test]
fn a_handlers_captures_at_the_top_are_planned_and_speculated() {
    let (captures, regions, source) = cart_page(true);
    assert_eq!(captures.len(), 1, "the button's: {captures:?}");
    assert!(regions.contains(&captures[0]), "{regions:?}");
    assert!(
        source.contains(&format!("{{ kind: \"captures\", part: {} }}", captures[0])),
        "{source}"
    );
}

#[test]
fn a_page_whose_handlers_capture_no_query_plans_none() {
    // The store's own cart page: its rows' handlers capture each `line`,
    // which the row sets again (ADR-0172).
    let (captures, _, source) = cart_page(false);
    assert_eq!(captures, Vec::<u32>::new());
    assert!(!source.contains("kind: \"captures\""), "{source}");
}
