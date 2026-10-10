//! **`pw build`: the store, from source to every artifact** (E10 gate item 1).
//!
//! "Store application builds from source to browser artifacts and Wasm
//! Components without Koka or Marko." `pw_core::build::build` composes the
//! stages that own each artifact. These tests hold what it produces for the
//! store, and that it refuses rather than builds around what it cannot build.

use pw_core::backend::component::Built;
use pw_core::build::build;
use pw_core::check::Unit;
use pw_core::hir::DeclKind;
use pw_core::lower::lower_file;
use pw_syntax::parse_tree;

fn unit(path: &str, src: &str) -> Unit {
    Unit {
        path: path.to_string(),
        hir: lower_file(src, &parse_tree(src).green),
        src: src.to_string(),
    }
}

fn store_units() -> Vec<Unit> {
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    let mut paths = Vec::new();
    for d in [
        "packages/pw-std",
        "packages/pw-platform-web",
        "examples/lib",
        "examples/store",
    ] {
        let mut ps: Vec<std::path::PathBuf> = std::fs::read_dir(root.join(d))
            .unwrap_or_else(|e| panic!("{d}: {e}"))
            .map(|e| e.expect("entry").path())
            .filter(|p| p.extension().is_some_and(|x| x == "pw"))
            .collect();
        ps.sort();
        paths.extend(ps);
    }
    paths.push(root.join("examples/domain.pw"));
    paths
        .iter()
        .map(|p| {
            unit(
                &p.display().to_string(),
                &std::fs::read_to_string(p).expect("read"),
            )
        })
        .collect()
}

#[test]
fn the_store_builds_every_artifact_from_source() {
    let b = build(&store_units()).expect("the store checks");
    assert!(b.refusals().is_empty(), "{:?}", b.refusals());

    let components: Vec<&str> = b
        .components
        .iter()
        .filter(|(_, x)| matches!(x, Built::Component { .. }))
        .map(|(id, _)| id.as_str())
        .collect();
    assert_eq!(
        components,
        [
            // The member functions the page reads through, each a component
            // of its own since ADR-0125: a line's count, a price as text
            // (ADR-0169), the cart's count, and a line's and the cart's
            // totals (ADR-0172).
            // Track `store-accounts` (ADR-XXXX): a user's cart's count
            // and its subtotal, its record's own.
            "UserCarts.line_count",
            "UserCarts.subtotal",
            "domain.count",
            "domain.display",
            // A cart's line, priced (ADR-0172).
            "domain.total",
            "store.page.Cart",
            // The store's slots, since ADR-0165: each stream's query.
            "store.page.Estimate",
            "store.page.Menu",
            // The menu counted, and the line the page shows from it: each a
            // materialization that derives its value, a component the host
            // runs to keep it (ADR-0277).
            "store.page.MenuLine",
            "store.page.MenuSize",
            // A session's order (ADR-0193).
            "store.page.Order",
            // The places an address may name, and whether the store reaches
            // the reader's (track `store-accounts`).
            "store.page.Places",
            "store.page.Reach",
            "store.page.Recommendations",
            // The reader's addresses (track `store-accounts`).
            "store.page.SavedAddresses",
            "store.page.Store",
            // The stores the home page lists (ADR-0192).
            "store.page.StoreList",
            "store.page.add_address",
            "store.page.add_to_cart",
            "store.page.choose_address",
            "store.page.clear_cart",
            // A line's controls (ADR-0172).
            "store.page.decrease_in_cart",
            "store.page.increase_in_cart",
            // The cart, placed as an order (ADR-0193).
            "store.page.place_order",
            "store.page.remove_address",
            "store.page.remove_from_cart",
            "store.page.rename_address",
        ],
        "every command, query and materialization the page reaches, compiled and audited"
    );
    for (id, x) in &b.components {
        if let Built::Component { compiled, audited } = x {
            assert!(*audited > 0, "{id}: the audit compared nothing");
            assert!(!compiled.component.bytes.is_empty());
        }
    }

    // The page renders through the template IR, and the library's `todo`
    // queries are placeholders nothing the store builds depends on.
    assert!(
        b.components
            .iter()
            .any(|(id, x)| id == "store.page.StorePage"
                && matches!(
                    x,
                    Built::NoBody {
                        kind: DeclKind::Page
                    }
                ))
    );
    assert_eq!(
        b.placeholders(),
        [
            "Resources.Cart",
            "Resources.Menu",
            "Resources.Order",
            "Resources.Store",
            // The library's `user` query the corpus imports (track
            // `store-accounts`, A-033 and R-064).
            "Saved.SavedFor"
        ]
    );

    assert_eq!(
        b.templates.len(),
        5,
        "StorePage, CartPage (ADR-0190), HomePage (ADR-0192), OrderPage (ADR-0193) and \
         AddressPage (track `store-accounts`)"
    );
    assert_eq!(
        b.handlers.len(),
        15,
        "add_to_cart, clear_cart, and a line's three (ADR-0172); the cart \
         page's clear and a line's three (ADR-0190), and its place_order \
         (ADR-0193); the address page's label and its four (track \
         `store-accounts`)"
    );
    assert_eq!(
        b.contracts.len(),
        b.components.len(),
        "one outcome per contract"
    );
    assert!(b.wit.contains("world "), "the WIT the components implement");
}

/// **A placeholder something depends on fails the build.** A page whose query
/// is `todo` would render by calling a body nobody wrote.
#[test]
fn a_placeholder_that_something_depends_on_is_a_refusal() {
    let program = "\
module shop.ui

query Count() -> Int { todo }

page Home() {
    let n = query Count()
    view {
        <p>{n}</p>
    }
}
";
    let b = build(&[unit("shop.pw", program)]).expect("it checks");
    assert_eq!(b.placeholders(), ["shop.ui.Count"]);
    let refused = b.refusals();
    println!("{refused:?}");
    assert_eq!(
        refused,
        ["`shop.ui.Home` depends on `shop.ui.Count`, whose body is a placeholder (`todo`)"]
    );

    // The control: the same program with the body written builds.
    let written = program.replace("{ todo }", "{ 3 }");
    let b = build(&[unit("shop.pw", &written)]).expect("it checks");
    assert!(b.refusals().is_empty(), "{:?}", b.refusals());
    assert!(b.placeholders().is_empty());
}

#[test]
fn a_program_that_does_not_check_builds_nothing() {
    let units: Vec<Unit> = store_units()
        .into_iter()
        .filter(|u| !u.path.contains("packages/"))
        .collect();
    let err = build(&units).expect_err("does not check");
    assert!(err.contains("does not check"), "{err}");
}
