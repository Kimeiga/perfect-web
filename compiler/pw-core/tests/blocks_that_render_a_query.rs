//! **A block that renders a query's value is planned, whatever decides it**
//! (ADR-0320).
//!
//! The plan named a block at the top of the page when a query's value
//! decided it (ADR-0146), and a host rendered it again when what it rendered
//! changed. A block a page's parameter decides, or what the page asks of its
//! reader, was named by nothing: its arm is the document's for its life, but
//! what the arm renders can still read a query, and nothing told it when the
//! query changed. W9's finding (2026-10-12): the search's sentence, inside
//! the `Some` arm of `{#match q}`, kept the sentence the page was served
//! with. Each test states one case, with its control.

use pw_core::check::Unit;
use pw_core::lower::lower_file;
use pw_syntax::parse_tree;

/// The store, its `app.pw` changed by `app` and its `search.pw` by `search`.
fn store(app: &dyn Fn(&str) -> String, search: &dyn Fn(&str) -> String) -> Vec<Unit> {
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
            let src = std::fs::read_to_string(&p).expect("read");
            let src = if p.ends_with("examples/store/app.pw") {
                app(&src)
            } else if p.ends_with("examples/store/search.pw") {
                search(&src)
            } else {
                src
            };
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

fn same(s: &str) -> String {
    s.to_string()
}

/// The ids of the blocks in `plan`'s template whose kind is `kind` and
/// whose subject is `value`.
fn blocks_on(units: &[Unit], page: &str, kind: &str, value: &str) -> (Vec<u32>, Vec<u32>) {
    let build = pw_core::build::build(units).unwrap_or_else(|e| panic!("builds: {e}"));
    let planned = build
        .pages
        .iter()
        .find(|p| p.page == page)
        .unwrap_or_else(|| panic!("no page {page}"));
    let plan = planned
        .plan
        .as_ref()
        .unwrap_or_else(|e| panic!("{page}: {e}"));
    let template = build
        .templates
        .iter()
        .find(|t| t.path == page)
        .unwrap_or_else(|| panic!("no template {page}"));
    let ids = template
        .manifest()
        .into_iter()
        .filter(|e| e.kind == kind && e.value == value)
        .map(|e| e.id.0)
        .collect();
    (ids, plan.blocks.clone())
}

/// **The search's sentence, inside the `Some` arm of `{#match q}`**: the
/// block is planned, so a host tells what it renders. Its control: the arm
/// holding text alone, which no query moves, is not.
#[test]
fn a_block_a_parameter_decides_that_renders_a_query_is_planned() {
    let (matches, planned) = blocks_on(
        &store(&same, &same),
        "store.search.SearchPage",
        "match",
        "q",
    );
    assert_eq!(matches.len(), 1, "the page's match on `q`: {matches:?}");
    assert!(planned.contains(&matches[0]), "{planned:?}");

    // Both of the arm's sentences, what was found and where the search was
    // for (ADR-0308, amended), each made plain text.
    let sentence = "{:Some(_)}<p id=\"search-said\" role=\"status\">{search.said}</p>\
                    <p id=\"search-near\">{search.near}</p>";
    let quiet = |s: &str| {
        assert_eq!(s.matches(sentence).count(), 1, "the sentences' anchor");
        s.replace(
            sentence,
            "{:Some(_)}<p id=\"search-said\">Searched.</p><p id=\"search-near\">Near.</p>",
        )
    };
    let (matches, planned) = blocks_on(
        &store(&same, &quiet),
        "store.search.SearchPage",
        "match",
        "q",
    );
    assert_eq!(matches.len(), 1, "{matches:?}");
    assert!(!planned.contains(&matches[0]), "{planned:?}");
}

/// The cart's page, `markup` at the top of its `<main>`, and a signal a
/// handler there may set.
fn cart_page(markup: &str) -> impl Fn(&str) -> String + '_ {
    move |src: &str| {
        // Track `order-total`: after the code's and the typed tip's.
        let page = "    signal tip_typed: String = \"\"\n\n    view {\n        <title>Your cart</title>\n        <main>\n";
        assert_eq!(src.matches(page).count(), 1, "the cart's page");
        src.replacen("import domain.{ ", "import domain.{ CartLine, ", 1)
            .replacen(
                page,
                &format!(
                    "    signal tip_typed: String = \"\"\n    signal kept: List<CartLine> = []\n\n    \
                     view {{\n        <title>Your cart</title>\n        <main>\n            \
                     {markup}\n"
                ),
                1,
            )
    }
}

/// **A block what the page asks of its reader decides** (ADR-0307), the
/// answer the document's for its life: planned where anything it renders
/// reads a query's value, by a value the host computes, a loop's list, an
/// attribute or what a handler captures; not where nothing does. (A text
/// part that reads one is the search's, above: the cart's own fields are
/// its lines, and a member read inside a block is refused.)
#[test]
fn a_block_the_readers_answer_decides_is_planned_where_it_renders_a_query() {
    let reads = [
        "{#if SignedIn}<p id=\"t\">{List.length(cart.lines)}</p>{/if}",
        "{#if SignedIn}<ul id=\"t\">{#each cart.lines as l (l.item_id)}<li>{l.name}</li>{/each}</ul>{/if}",
        "{#if SignedIn}<p id=\"t\" hidden={cart.lines}>Empty.</p>{/if}",
        "{#if SignedIn}<button type=\"button\" id=\"t\" on:press={resumable(captures = { cart }) => kept = cart.lines}>Keep</button>{/if}",
    ];
    let quiet = [
        "{#if SignedIn}<p id=\"t\">Signed in.</p>{/if}",
        "{#if SignedIn}<button type=\"button\" id=\"t\" on:press={() => kept = []}>Forget</button>{/if}",
    ];
    for (markup, planned) in reads
        .iter()
        .map(|m| (m, true))
        .chain(quiet.iter().map(|m| (m, false)))
    {
        let units = store(&cart_page(markup), &same);
        let (ifs, blocks) = blocks_on(
            &units,
            "store.page.CartPage",
            "conditional",
            "SignedIn~holds",
        );
        // The first is the one written at the top of `<main>`.
        let first = *ifs.first().unwrap_or_else(|| panic!("{markup}: no block"));
        assert_eq!(blocks.contains(&first), planned, "{markup}: {blocks:?}");
    }
}

/// **A block inside one is rendered with it** (ADR-0146): the outer one is
/// planned, the inner one is not.
#[test]
fn a_block_inside_one_is_rendered_with_it() {
    let units = store(
        &cart_page(
            "{#if SignedIn}<div id=\"t\">{#if SignedIn}<p>{List.length(cart.lines)}</p>{/if}</div>{/if}",
        ),
        &same,
    );
    let (ifs, blocks) = blocks_on(
        &units,
        "store.page.CartPage",
        "conditional",
        "SignedIn~holds",
    );
    assert!(blocks.contains(&ifs[0]), "the outer one: {blocks:?}");
    assert!(!blocks.contains(&ifs[1]), "the inner one: {blocks:?}");
}

/// The cart's page as the store ships it: what a query decides, the order's
/// link and, since track `order-total`, the checkout's disclosure and its
/// code's Remove, and, since track `scheduled-orders`, the checkout's line of
/// when the order is for; and its `{#if SignedIn}` Place order, which
/// captures the checkout, the total it passes (ADR-0307: planned where a
/// handler there captures a query's value). The cart's own fees line,
/// decided by its lines, is the checkout's now.
#[test]
fn the_cart_page_as_it_is_plans_what_it_did() {
    let units = store(&same, &same);
    let build = pw_core::build::build(&units).unwrap_or_else(|e| panic!("builds: {e}"));
    let plan = build
        .pages
        .iter()
        .find(|p| p.page == "store.page.CartPage")
        .and_then(|p| p.plan.as_ref().ok())
        .expect("the cart's page, planned");
    let template = build
        .templates
        .iter()
        .find(|t| t.path == "store.page.CartPage")
        .expect("its template");
    let mut subjects: Vec<String> = template
        .manifest()
        .into_iter()
        .filter(|e| plan.blocks.contains(&e.id.0))
        .map(|e| e.value)
        .collect();
    subjects.sort();
    assert_eq!(
        subjects,
        [
            "SignedIn~holds",
            "checkout.any",
            "checkout.any",
            "checkout.promo",
            "order"
        ],
        "{:?}",
        plan.blocks
    );
}
