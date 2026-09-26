//! **A resumable handler reads what it captures** (ADR-0110).
//!
//! A resumable handler runs later, in the browser, with what it captured and
//! nothing else: its captures are written into the document when the page
//! renders and read back when it runs. Until 2026-09-26 `resumable() =>
//! add_to_cart(item.id, ..)` inside `{#each menu as item}` checked, and
//! `pw emit-handlers` refused it: "`item` is not bound here". Each test
//! states one case, with a control.

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
fn a_handler_reads_what_it_captures() {
    assert_eq!(
        reported(&page("resumable() => add_to_cart(item.id, PositiveInt(1))")),
        ["PW5025 `ShopPage`'s handler reads `item`, which it does not capture"],
    );
    // The controls: the store's own handler, and one binding its own value
    // beside the program's declarations, which are not captures.
    let found = reported(&page(
        "resumable(captures = { item }) => add_to_cart(item.id, PositiveInt(1))",
    ));
    assert!(found.is_empty(), "{found:#?}");
    let found = reported(&page(
        "resumable(captures = { item }) => {\n                        \
         let quantity = PositiveInt(1)\n                        \
         add_to_cart(item.id, quantity)\n                    }",
    ));
    assert!(found.is_empty(), "{found:#?}");
}

#[test]
fn a_page_parameter_is_captured_too() {
    // `id` is the page's, bound where it renders. Read twice, reported once.
    assert_eq!(
        reported(&page(
            "resumable(captures = { item }) => {\n                        \
             let _store = id\n                        \
             let _again = id\n                        \
             add_to_cart(item.id, PositiveInt(1))\n                    }",
        )),
        ["PW5025 `ShopPage`'s handler reads `id`, which it does not capture"],
    );
}

#[test]
fn a_shorthand_field_reads_its_name() {
    // `Pick { n: 1, item }` reads `item`, and nothing else in the handler
    // does. A first field written short is a block (KNOWN_LIMITATIONS).
    let pick = "type Pick = Pick { n: Int, item: MenuItem }\n\n\
                fn chosen(p: Pick) -> MenuItemId !{} {\n    p.item.id\n}\n\n";
    assert_eq!(
        reported(&page_with(
            pick,
            "resumable() => add_to_cart(chosen(Pick { n: 1, item }), PositiveInt(1))",
        )),
        ["PW5025 `ShopPage`'s handler reads `item`, which it does not capture"],
    );
    // The control: captured, it is not this rule's. (The artifact's capture
    // paths miss a shorthand read, which PW5017 reports: NEXT.)
    let found = reported(&page_with(
        pick,
        "resumable(captures = { item }) => add_to_cart(chosen(Pick { n: 1, item }), PositiveInt(1))",
    ));
    assert!(!found.iter().any(|m| m.starts_with("PW5025")), "{found:#?}");
}
