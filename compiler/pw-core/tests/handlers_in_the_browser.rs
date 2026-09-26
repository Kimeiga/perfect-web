//! **A resumable handler runs in the browser** (ADR-0113).
//!
//! A handler is loaded and run in the browser when its element is pressed,
//! and it reaches the origin through a command, which is its own component
//! with its own contract. The page's contract leaves its handlers out, and a
//! handler has none, so until 2026-09-26 nothing asked where a handler ran:
//! one calling `Carts.add` itself checked, and `pw emit-handlers` refused it.
//! Each test states one case, with a control.

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

/// A page listing the menu, each item with a button whose handler is
/// `handler`.
fn page(handler: &str) -> String {
    page_with("", handler)
}

/// `page`, with the declarations `extra` beside it.
fn page_with(extra: &str, handler: &str) -> String {
    format!(
        "module t\n\nimport Carts\nimport Resources.{{ Menu }}\n\
         import context.{{ current_session }}\n\
         import domain.{{ StoreId, MenuItem, MenuItemId, PositiveInt, InteractionId, Cart, \
         CartError }}\n\n{extra}\
         command add_to_cart(item: MenuItemId, quantity: PositiveInt) -> Result<Cart, CartError>\n    \
         requires      SignedIn\n    idempotent_by InteractionId\n{{\n    \
         Carts.add(current_session(), item, quantity)\n}}\n\n\
         page ShopPage(id: StoreId) {{\n    placement origin\n    cache private\n\n    \
         let menu = query Menu(id)\n\n    view {{\n        <ul>\n            \
         {{#each menu as item (item.id)}}\n                <li>\n                    \
         <button type=\"button\" on:press={{{handler}}}>Add</button>\n                \
         </li>\n            {{/each}}\n        </ul>\n    }}\n}}\n"
    )
}

#[test]
fn a_handler_performs_only_what_the_browser_may() {
    assert_eq!(
        reported(&page(
            "resumable(captures = { item }) => Carts.add(current_session(), item.id, PositiveInt(1))",
        )),
        [
            "PW5005 `ShopPage`'s handler performs `database.write<Carts>`, which the browser \
          cannot grant"
        ],
    );
    // The controls: the command performs the write, and a function that
    // performs nothing is the browser's to run.
    let found = reported(&page(
        "resumable(captures = { item }) => add_to_cart(item.id, PositiveInt(1))",
    ));
    assert!(found.is_empty(), "{found:#?}");
    let found = reported(&page_with(
        "fn one() -> PositiveInt !{} {\n    PositiveInt(1)\n}\n\n",
        "resumable(captures = { item }) => add_to_cart(item.id, one())",
    ));
    assert!(found.is_empty(), "{found:#?}");
}
