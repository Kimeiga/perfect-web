//! **Every handler is resumable; what it captures is what it reads**
//! (ADR-0134).
//!
//! Until 2026-10-02 a handler was a lambda written `resumable(..)`, and only
//! that. `on:press={() => count = count + 1}` checked, built, and shipped a
//! button with no code behind it: its event part had no identity and no
//! module, and nothing said so. And `() =>` lowered with one parameter, so a
//! handler written that way was refused as one taking the event.
//!
//! Each test states one part, with controls.

use pw_core::check::{Unit, check_sources};
use pw_core::lower::lower_file;
use pw_syntax::parse_tree;

fn library() -> Vec<(String, String)> {
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    let mut out = Vec::new();
    for d in [
        "packages/pw-std",
        "packages/pw-platform-web",
        "examples/lib",
        "examples/store",
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
    out
}

fn units(src: &str) -> Vec<Unit> {
    let mut sources = library();
    sources.push(("t.pw".to_string(), src.to_string()));
    sources
        .into_iter()
        .map(|(path, src)| Unit {
            path,
            hir: lower_file(&src, &parse_tree(&src).green),
            src,
        })
        .collect()
}

fn reported(src: &str) -> Vec<String> {
    let mut sources = library();
    sources.push(("t.pw".to_string(), src.to_string()));
    check_sources(&sources)
        .into_iter()
        .filter(|(n, _)| n == "t.pw")
        .flat_map(|(_, ds)| ds.into_iter().map(|d| format!("{} {}", d.code, d.message)))
        .collect()
}

/// Each of `t.P`'s event parts: its handler identity and capture paths.
fn events(b: &pw_core::build::Build) -> Vec<(String, Vec<String>)> {
    let t = b
        .templates
        .iter()
        .find(|t| t.path == "t.P")
        .expect("the page's template");
    let ir = serde_json::to_value(t).expect("serializes");
    let mut out = Vec::new();
    fn walk(v: &serde_json::Value, out: &mut Vec<(String, Vec<String>)>) {
        if v["part"] == "event" {
            out.push((
                v["handler"].as_str().unwrap_or_default().to_string(),
                v["captures"]
                    .as_array()
                    .into_iter()
                    .flatten()
                    .filter_map(|c| c.as_str().map(str::to_string))
                    .collect(),
            ));
        }
        match v {
            serde_json::Value::Object(o) => o.values().for_each(|x| walk(x, out)),
            serde_json::Value::Array(a) => a.iter().for_each(|x| walk(x, out)),
            _ => {}
        }
    }
    walk(&ir, &mut out);
    out
}

/// A page over the store's menu, whose view holds `view`.
fn page(view: &str) -> String {
    format!(
        "module t\n\nimport store.page.{{ Menu, add_to_cart }}\nimport domain.{{ StoreId, PositiveInt }}\n\n\
         page P(id: StoreId) {{\n    cache private\n\n    signal count: Int = 0\n\n    \
         let menu = query Menu(id)\n\n    view {{\n        <main>\n{view}\n        </main>\n    }}\n}}\n"
    )
}

#[test]
fn a_handler_written_as_a_lambda_has_code() {
    let src =
        page("            <button type=\"button\" on:press={() => count = count + 1}>Add</button>");
    assert_eq!(reported(&src), Vec::<String>::new());
    let b = pw_core::build::build(&units(&src)).expect("builds");
    let events = events(&b);
    assert_eq!(events.len(), 1);
    assert!(!events[0].0.is_empty(), "the handler has an identity");
    assert!(
        b.handlers.iter().any(|h| matches!(
            &h.module,
            pw_core::backend::wasm::Encoding::Encoded(m) if m.identity == events[0].0
        )),
        "and a module"
    );
    assert!(b.refusals().is_empty(), "{:?}", b.refusals());
}

#[test]
fn a_handler_captures_what_it_reads_of_the_page() {
    // A loop's item, read by a field: what the listed form would carry.
    let inferred = page(
        "            {#each menu as item (item.id)}\n                \
         <button type=\"button\" on:press={() => add_to_cart(item.id, PositiveInt(1))}>Add</button>\n\
         \x20           {/each}",
    );
    let listed = inferred.replace(
        "on:press={() =>",
        "on:press={resumable(captures = { item }) =>",
    );
    for src in [&inferred, &listed] {
        assert_eq!(reported(src), Vec::<String>::new(), "{src}");
    }
    let a = events(&pw_core::build::build(&units(&inferred)).expect("builds"));
    let b = events(&pw_core::build::build(&units(&listed)).expect("builds"));
    assert_eq!(a[0].1, ["item.id"], "inferred");
    assert_eq!(a[0].1, b[0].1, "the same as the listed form");

    // A parameter of the page, whole.
    let param =
        page("            <button type=\"button\" on:press={() => count = limit}>Mine</button>")
            .replace("page P(id: StoreId)", "page P(id: StoreId, limit: Int)");
    assert_eq!(reported(&param), Vec::<String>::new());
    let found = events(&pw_core::build::build(&units(&param)).expect("builds"));
    assert_eq!(found[0].1, ["limit"]);

    // Control: what the handler binds itself is its own.
    let own = page(
        "            <button type=\"button\" on:press={() => {\n                \
         let step = 2\n                count = count + step\n            }}>Add</button>",
    );
    let found = events(&pw_core::build::build(&units(&own)).expect("builds"));
    assert_eq!(found[0].1, Vec::<String>::new(), "its own binding");

    // Control: a signal is the browser's, and is not carried.
    let signal =
        page("            <button type=\"button\" on:press={() => count = count + 1}>Add</button>");
    let found = events(&pw_core::build::build(&units(&signal)).expect("builds"));
    assert_eq!(found[0].1, Vec::<String>::new());
}

#[test]
fn a_listed_capture_is_still_held_to_what_the_handler_reads() {
    // PW5025: the listed form captures what it lists, and reading another
    // binding of the page is the defect it was.
    let src = page(
        "            {#each menu as item (item.id)}\n                \
         <button type=\"button\" on:press={resumable() => add_to_cart(item.id, PositiveInt(1))}>Add</button>\n\
         \x20           {/each}",
    );
    let found = reported(&src);
    assert!(
        found
            .iter()
            .any(|d| d.starts_with("PW5025") && d.contains("`item`")),
        "{found:?}"
    );
}

#[test]
fn a_handler_that_is_not_a_lambda_is_refused_at_build() {
    let src = "module t\n\nfn save() -> () !{} { () }\n\npage P() {\n    cache private\n\n    \
               view {\n        <main><button type=\"button\" on:press={save}>Save</button></main>\n    }\n}\n";
    let b = pw_core::build::build(&units(src)).expect("builds");
    assert!(
        b.refusals()
            .iter()
            .any(|r| r.contains("`t.P`") && r.contains("no code to run")),
        "{:?}",
        b.refusals()
    );
}
