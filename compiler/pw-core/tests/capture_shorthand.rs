//! **A shorthand field reads its capture** (ADR-0111).
//!
//! A resumable handler's capture paths say what the document must carry for
//! it: `item.id` when it reads `item.id`, the whole `item` when it reads
//! `item`. A record's shorthand field, `Pick { n: 1, item }`, reads `item`
//! with no expression of its own, and until 2026-09-26 the paths did not
//! count it. The handler read nothing of `item` by its artifact, the manifest
//! captured `item`, and PW5017 refused a program with nothing wrong in it.
//! The handler backend read a shorthand field from bindings alone, so once
//! checked, the handler was still refused: "`item` is not bound here".

use pw_core::backend::js;
use pw_core::backend::wasm::Encoding;
use pw_core::check::{Unit, check_sources};
use pw_core::lower::lower_file;
use pw_syntax::parse_tree;

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
fn a_capture_read_through_a_shorthand_field_is_read() {
    let pick = "type Pick = Pick { n: Int, item: MenuItem }\n\n\
                fn chosen(p: Pick) -> MenuItemId !{} {\n    p.item.id\n}\n\n";
    let found = reported(&page_with(
        pick,
        "resumable(captures = { item }) => add_to_cart(chosen(Pick { n: 1, item }), PositiveInt(1))",
    ));
    assert!(found.is_empty(), "{found:#?}");
    // The control: a capture read through a field, as the store reads it.
    let found = reported(&page(
        "resumable(captures = { item }) => add_to_cart(item.id, PositiveInt(1))",
    ));
    assert!(found.is_empty(), "{found:#?}");
}

/// The handler of `page_with(extra, handler)`, compiled, or why not.
fn compiled(extra: &str, handler: &str) -> Result<String, String> {
    let units: Vec<Unit> = program(&page_with(extra, handler))
        .into_iter()
        .map(|(path, src)| Unit {
            path,
            hir: lower_file(&src, &parse_tree(&src).green),
            src,
        })
        .collect();
    let compiled = js::compile(&units).map_err(|e| format!("{e:?}"))?;
    let [one] = compiled.as_slice() else {
        return Err(format!("{} handlers", compiled.len()));
    };
    match &one.module {
        Encoding::Encoded(m) => Ok(m.source.clone()),
        other => Err(format!("{other}")),
    }
}

#[test]
fn a_handler_builds_a_record_from_a_capture() {
    let pick = "type Pick = Pick { n: Int, item: MenuItem }\n\n\
                fn chosen(p: Pick) -> MenuItemId !{} {\n    p.item.id\n}\n\n";
    let source = compiled(
        pick,
        "resumable(captures = { item }) => add_to_cart(chosen(Pick { n: 1, item }), PositiveInt(1))",
    )
    .unwrap_or_else(|e| panic!("the handler compiles: {e}"));
    // It reads the whole item from what the document carries.
    assert!(source.contains("context.captures[\"item\"]"), "{source}");
}
