//! **A view's own signals, and a signal a page provides** (ADR-0144).
//!
//! A view holds its own signals, and each use of it holds its own instance.
//! A signal a module declares, `signal drawer: Bool`, has no value: a page or
//! view gives it one with `provide`, for everything that body contains, and a
//! view names it as it names anything it imports. Until 2026-10-02 a view
//! that held anything but markup was refused, and a signal lived in a page's
//! body alone, so two views could share UI state only through the page.
//!
//! Each test states one part, with controls.

use pw_core::backend::wasm::Encoding;
use pw_core::check::{Unit, check_sources};
use pw_core::lower::lower_file;
use pw_syntax::parse_tree;

fn library() -> Vec<(String, String)> {
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    let mut out = Vec::new();
    for d in ["packages/pw-std", "packages/pw-platform-web"] {
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
    out
}

/// The views the pages below use: a signal they share, and a view with one
/// of its own.
const UI: &str = "module ui\n\n\
signal drawer: Bool\n\n\
view Open() !{} {\n    <button type=\"button\" on:press={() => drawer = true}>Open</button>\n}\n\n\
view Shown() !{} {\n    <div>{#if drawer}<p>open</p>{/if}</div>\n}\n\n\
view Counter() !{} {\n    signal count: Int = 0\n\n    \
<p><button type=\"button\" on:press={() => count = count + 1}>Add</button> <span>{count}</span></p>\n}\n\n\
view Wrap() !{} {\n    <section><Open /></section>\n}\n\n\
view Panel() !{} {\n    provide drawer = false\n\n    <section><Open /><Shown /></section>\n}\n\n\
view Counted() !{} {\n    <section><Counter /></section>\n}\n";

/// A page importing the views, whose body holds `inner`.
fn page(inner: &str) -> String {
    format!(
        "module p\n\nimport ui.{{ drawer, Open, Shown, Counter, Wrap, Panel, Counted }}\n\n\
         page P(items: List<String>) {{\n    cache private\n{inner}\n}}\n"
    )
}

fn sources(page: &str) -> Vec<(String, String)> {
    let mut s = library();
    s.push(("ui.pw".to_string(), UI.to_string()));
    s.push(("p.pw".to_string(), page.to_string()));
    s
}

/// Every diagnostic in the page and the views, as `code message`.
fn reported(page: &str) -> Vec<String> {
    check_sources(&sources(page))
        .into_iter()
        .filter(|(n, _)| n == "p.pw" || n == "ui.pw")
        .flat_map(|(_, ds)| ds.into_iter().map(|d| format!("{} {}", d.code, d.message)))
        .collect()
}

fn build(page: &str) -> pw_core::build::Build {
    let units: Vec<Unit> = sources(page)
        .into_iter()
        .map(|(path, src)| Unit {
            path,
            hir: lower_file(&src, &parse_tree(&src).green),
            src,
        })
        .collect();
    pw_core::build::build(&units).expect("builds")
}

fn plan(b: &pw_core::build::Build) -> pw_core::page_values::PageValues {
    b.pages
        .iter()
        .find(|p| p.page == "p.P")
        .expect("a plan")
        .plan
        .clone()
        .expect("planned")
}

/// The page's template, as the browser receives it: its static markup.
fn markup(b: &pw_core::build::Build) -> String {
    let t = b
        .templates
        .iter()
        .find(|t| t.path == "p.P")
        .expect("the page");
    t.chunks
        .iter()
        .filter_map(|c| match c {
            pw_core::template_ir::Chunk::Static(s) => Some(s.as_str()),
            _ => None,
        })
        .collect()
}

fn signals(b: &pw_core::build::Build) -> Vec<(String, serde_json::Value)> {
    plan(b)
        .signals
        .iter()
        .map(|s| (s.name.clone(), s.initial.clone()))
        .collect()
}

#[test]
fn a_module_signal_is_a_name_and_a_type() {
    let hir = lower_file(UI, &parse_tree(UI).green);
    let drawer = hir
        .all_decls()
        .find(|(_, d)| d.name == "drawer")
        .map(|(_, d)| d)
        .expect("declared");
    assert_eq!(drawer.kind, pw_core::hir::DeclKind::Signal);
    assert!(drawer.mutable, "its handlers change it");
    assert_eq!(
        drawer.ret.as_ref().map(|t| t.written()),
        Some("Bool".to_string())
    );
    // A value written at module level is refused: a `provide` gives one.
    let valued = "module m\n\nsignal open: Bool = false\n";
    let errors = parse_tree(valued).errors;
    assert!(
        errors
            .iter()
            .any(|e| e.code == "PW5306" && e.message.contains("no value of its own")),
        "{errors:?}"
    );
    // Control: a page's own `signal` is still a statement of its body.
    assert!(
        parse_tree(UI).errors.is_empty(),
        "{:?}",
        parse_tree(UI).errors
    );
}

#[test]
fn each_use_of_a_view_holds_its_own_signal() {
    let src = page(
        "\n    view {\n        <main><section id=\"a\"><Counter /></section>\
         <section id=\"b\"><Counter /></section></main>\n    }",
    );
    assert_eq!(reported(&src), Vec::<String>::new());
    let b = build(&src);
    assert!(b.refusals().is_empty(), "{:?}", b.refusals());
    let held = signals(&b);
    assert_eq!(held.len(), 2, "{held:?}");
    assert_ne!(held[0].0, held[1].0, "each use is its own instance");
    for (name, initial) in &held {
        assert!(
            name.starts_with("count~"),
            "named so no source writes it: {name}"
        );
        assert_eq!(initial, &serde_json::json!(0));
    }
    // Each button says which instance its handler's `count` is, and the two
    // are one handler.
    let html = markup(&b);
    for (name, _) in &held {
        assert!(
            html.contains(&format!(
                "data-pw-signals=\"{{&quot;count&quot;:&quot;{name}&quot;}}\""
            )),
            "{html}"
        );
    }
    let t = b.templates.iter().find(|t| t.path == "p.P").expect("page");
    let events: Vec<String> = t
        .manifest()
        .iter()
        .filter(|e| e.kind == "event")
        .map(|e| e.value.clone())
        .collect();
    assert_eq!(events.len(), 2);
    assert_eq!(events[0], events[1], "one compiled handler: {events:?}");
    // Control: a page's own signal keeps the name it is written with.
    let own = page(
        "\n    signal open: Bool = false\n\n    view {\n        <main>\
         <button type=\"button\" on:press={() => open = true}>o</button>{#if open}<p>o</p>{/if}\
         </main>\n    }",
    );
    assert_eq!(reported(&own), Vec::<String>::new());
    let b = build(&own);
    assert_eq!(
        signals(&b),
        [("open".to_string(), serde_json::json!(false))]
    );
    assert!(!markup(&b).contains("data-pw-signals"), "{}", markup(&b));
}

#[test]
fn a_provided_signal_reaches_sibling_views() {
    let src = page(
        "\n    provide drawer = false\n\n    view {\n        <main><Open /><Shown /></main>\n    }",
    );
    assert_eq!(reported(&src), Vec::<String>::new());
    let b = build(&src);
    assert!(b.refusals().is_empty(), "{:?}", b.refusals());
    assert_eq!(
        signals(&b),
        [("drawer".to_string(), serde_json::json!(false))]
    );
    // `<Shown>`'s block is the page's `drawer`'s.
    let live: Vec<(String, String)> = plan(&b)
        .live
        .iter()
        .map(|l| (l.kind.clone(), l.signal.clone()))
        .collect();
    assert_eq!(live, [("conditional".to_string(), "drawer".to_string())]);
    // And `<Open>`'s handler sets it, by the name the page holds it under.
    assert!(!markup(&b).contains("data-pw-signals"), "{}", markup(&b));
    let set = b.handlers.iter().any(|h| match &h.module {
        Encoding::Encoded(m) => {
            m.source.contains("const v0 = true;")
                && m.source.contains("context.set(\"drawer\", v0);")
        }
        _ => false,
    });
    assert!(set, "the handler sets `drawer` through its context");
}

#[test]
fn a_nearer_provide_keeps_what_it_contains_apart() {
    let src = page(
        "\n    provide drawer = false\n\n    view {\n        <main><Open /><Shown /><Panel /></main>\n    }",
    );
    assert_eq!(reported(&src), Vec::<String>::new());
    let b = build(&src);
    assert!(b.refusals().is_empty(), "{:?}", b.refusals());
    let held = signals(&b);
    assert_eq!(held.len(), 2, "{held:?}");
    assert_eq!(held[0].0, "drawer");
    let panel = &held[1].0;
    assert!(panel.starts_with("drawer~"), "{held:?}");
    // The panel's button says it sets the panel's; the page's says nothing.
    let html = markup(&b);
    assert_eq!(html.matches("data-pw-signals").count(), 1, "{html}");
    assert!(
        html.contains(&format!(
            "data-pw-signals=\"{{&quot;drawer&quot;:&quot;{panel}&quot;}}\""
        )),
        "{html}"
    );
    // Each block reads its own.
    let blocks: Vec<String> = plan(&b).live.iter().map(|l| l.signal.clone()).collect();
    assert_eq!(blocks, ["drawer".to_string(), panel.clone()]);
}

#[test]
fn what_nothing_provides_is_refused() {
    // A view the page uses needs it.
    let used = page("\n    view {\n        <main><Open /></main>\n    }");
    assert_eq!(
        reported(&used),
        ["PW5305 nothing provides `ui.drawer` to `P`, and `<Open>` needs it"]
    );
    // Through a view in between, which the message names.
    let through = page("\n    view {\n        <main><Wrap /></main>\n    }");
    assert_eq!(
        reported(&through),
        ["PW5305 nothing provides `ui.drawer` to `P`, and `<Wrap>` needs it, through `<Open>`"]
    );
    // The page's own markup.
    let own = page("\n    view {\n        <main>{#if drawer}<p>o</p>{/if}</main>\n    }");
    assert_eq!(
        reported(&own),
        ["PW5305 nothing provides `ui.drawer` to `P`, and `P` reads it here"]
    );
    // Controls: the page provides it; a view in between provides it.
    let given =
        page("\n    provide drawer = false\n\n    view {\n        <main><Wrap /></main>\n    }");
    assert_eq!(reported(&given), Vec::<String>::new());
    let panel = page("\n    view {\n        <main><Panel /></main>\n    }");
    assert_eq!(reported(&panel), Vec::<String>::new());
}

#[test]
fn a_body_provides_a_module_signal_once() {
    let twice = page(
        "\n    provide drawer = false\n    provide drawer = true\n\n    view {\n        \
         <main><Open /></main>\n    }",
    );
    assert_eq!(
        reported(&twice),
        ["PW5306 `drawer` is provided twice in `P`"]
    );
    let not_a_signal = "module q\n\nfn helper() -> Int { 1 }\n\npage Q() {\n    cache private\n    \
                        provide helper = 1\n\n    view {\n        <main><p>q</p></main>\n    }\n}\n";
    let found: Vec<String> = check_sources(&{
        let mut s = library();
        s.push(("q.pw".to_string(), not_a_signal.to_string()));
        s
    })
    .into_iter()
    .filter(|(n, _)| n == "q.pw")
    .flat_map(|(_, ds)| ds.into_iter().map(|d| format!("{} {}", d.code, d.message)))
    .collect();
    assert_eq!(
        found,
        [
            "PW5306 `helper` is not a signal a module declares: a `provide` gives one its value, \
          `signal helper: Type` at module level"
        ]
    );
    let in_a_fn =
        "module r\n\nsignal on: Bool\n\nfn f() -> Int {\n    provide on = false\n    1\n}\n";
    let found: Vec<String> = check_sources(&{
        let mut s = library();
        s.push(("r.pw".to_string(), in_a_fn.to_string()));
        s
    })
    .into_iter()
    .filter(|(n, _)| n == "r.pw")
    .flat_map(|(_, ds)| ds.into_iter().map(|d| format!("{} {}", d.code, d.message)))
    .collect();
    assert_eq!(
        found,
        [
            "PW5306 `f` is not a page or a view, and a `provide` gives `on` to the markup of the \
          one it is written in"
        ]
    );
    // Control: once.
    let once =
        page("\n    provide drawer = false\n\n    view {\n        <main><Open /></main>\n    }");
    assert_eq!(reported(&once), Vec::<String>::new());
}

#[test]
fn a_view_that_holds_a_signal_is_not_used_in_a_row() {
    let row =
        page("\n    view {\n        <main>{#each items as i (i)}<Counter />{/each}</main>\n    }");
    assert_eq!(
        reported(&row),
        [
            "PW5307 `<Counter>` holds a signal, and a view used in a loop's row holds none yet: \
          each row would need its own"
        ]
    );
    // Through a view that composes one.
    let through =
        page("\n    view {\n        <main>{#each items as i (i)}<Counted />{/each}</main>\n    }");
    assert_eq!(reported(&through).len(), 1, "{:?}", reported(&through));
    assert!(reported(&through)[0].starts_with("PW5307 `<Counted>`"));
    // Control: a view that sets a signal the page provides holds none.
    let sets = page(
        "\n    provide drawer = false\n\n    view {\n        <main>{#each items as i (i)}<Open />{/each}\
         <Shown /></main>\n    }",
    );
    assert_eq!(reported(&sets), Vec::<String>::new());
}

#[test]
fn a_module_signal_is_read_and_changed_where_a_page_signal_is() {
    let write = page(
        "\n    provide drawer = false\n    drawer = true\n\n    view {\n        <main><Shown /></main>\n    }",
    );
    assert_eq!(
        reported(&write),
        ["PW5300 `drawer` is a signal, and it is changed outside a handler"]
    );
    let read = page(
        "\n    provide drawer = false\n    let shown = drawer\n\n    view {\n        <main><Shown /></main>\n    }",
    );
    assert_eq!(
        reported(&read),
        [
            "PW5301 `drawer` is a signal, and this reads it once, on the server, where it never \
          changes"
        ]
    );
    // Control: a template part reads it, and a handler changes it.
    let parts = page(
        "\n    provide drawer = false\n\n    view {\n        <main>{#if drawer}<p>o</p>{/if}\
         <button type=\"button\" on:press={() => drawer = false}>x</button></main>\n    }",
    );
    assert_eq!(reported(&parts), Vec::<String>::new());
}

#[test]
fn a_provide_is_checked_against_the_signal_s_type() {
    let wrong =
        page("\n    provide drawer = 1\n\n    view {\n        <main><Shown /></main>\n    }");
    assert_eq!(
        reported(&wrong),
        ["PW0607 `drawer` holds `Bool` and is assigned `Int`"]
    );
    let right =
        page("\n    provide drawer = true\n\n    view {\n        <main><Shown /></main>\n    }");
    assert_eq!(reported(&right), Vec::<String>::new());
    assert_eq!(
        signals(&build(&right)),
        [("drawer".to_string(), serde_json::json!(true))]
    );
}

#[test]
fn a_block_owns_the_signals_its_arms_hold() {
    let src = page(
        "\n    provide drawer = false\n\n    view {\n        <main><Open />\
         {#if drawer}<Counter />{/if}<Counter /></main>\n    }",
    );
    assert_eq!(reported(&src), Vec::<String>::new());
    let b = build(&src);
    let held = signals(&b);
    assert_eq!(held.len(), 3, "{held:?}");
    let block = plan(&b)
        .live
        .into_iter()
        .find(|l| l.kind == "conditional")
        .expect("the block");
    // The counter in the arm is the block's; the one after it is not.
    assert_eq!(block.owns, [held[1].0.clone()]);
    // An `{:else if}` arm is a block of its own, and its parent's too.
    let chain = page(
        "\n    provide drawer = false\n    signal other: Bool = false\n\n    view {\n        <main>\
         <Open />{#if drawer}<p>a</p>{:else if other}<Counter />{/if}</main>\n    }",
    );
    assert_eq!(reported(&chain), Vec::<String>::new());
    let b = build(&chain);
    let count = signals(&b)
        .into_iter()
        .find(|(n, _)| n.starts_with("count~"))
        .expect("the counter")
        .0;
    let owners: Vec<String> = plan(&b)
        .live
        .into_iter()
        .filter(|l| l.owns.contains(&count))
        .map(|l| l.signal)
        .collect();
    assert_eq!(owners, ["drawer".to_string(), "other".to_string()]);
}

#[test]
fn binding_and_dialogs_read_a_module_signal() {
    // One view, in a module of its own, given what it names by a page.
    let check = |view: &str| -> Vec<String> {
        let ui = format!(
            "module ui\n\nsignal note: String\nsignal qty: Int\nsignal shown: Bool\n\n{view}\n"
        );
        let p = "module p\n\nimport ui.{ note, qty, shown, V }\n\npage P() {\n    cache private\n    \
                 provide note = \"\"\n    provide qty = 0\n    provide shown = false\n\n    view {\n        \
                 <main><V /></main>\n    }\n}\n";
        let mut s = library();
        s.push(("ui.pw".to_string(), ui));
        s.push(("p.pw".to_string(), p.to_string()));
        check_sources(&s)
            .into_iter()
            .filter(|(n, _)| n == "p.pw" || n == "ui.pw")
            .flat_map(|(_, ds)| ds.into_iter().map(|d| format!("{} {}", d.code, d.message)))
            .collect()
    };
    // A field binds a signal of `String` a page provides; one of `Int` waits
    // for a codec.
    let string =
        "view V() !{} {\n    <input type=\"text\" aria-label=\"Note\" bind:value={note} />\n}";
    assert_eq!(check(string), Vec::<String>::new());
    let int =
        check("view V() !{} {\n    <input type=\"text\" aria-label=\"Qty\" bind:value={qty} />\n}");
    assert_eq!(int.len(), 1, "{int:?}");
    assert!(
        int[0].starts_with("PW5304 `bind:value` binds `qty`, a signal of `Int`"),
        "{int:?}"
    );
    // A dialog a module's signal shows hears its closing.
    let heard = "view V() !{} {\n    <div>{#if shown}<dialog aria-label=\"d\" on:close={() => shown = false}>\
                 <p>x</p></dialog>{/if}</div>\n}";
    assert_eq!(check(heard), Vec::<String>::new());
    let unheard = check(
        "view V() !{} {\n    <div>{#if shown}<dialog aria-label=\"d\"><p>x</p></dialog>{/if}</div>\n}",
    );
    assert_eq!(unheard.len(), 1, "{unheard:?}");
    assert!(unheard[0].starts_with("PW5303"), "{unheard:?}");
}

#[test]
fn the_marko_adapter_refuses_what_it_does_not_model() {
    // A `provide`, and a signal a page provides, are refused with a reason:
    // until 2026-10-02 the adapter dropped `provide` and wrote a handler
    // setting a name nothing held.
    let hir = lower_file(UI, &parse_tree(UI).green);
    let signals: std::collections::BTreeSet<String> = ["drawer".to_string()].into();
    let out = pw_core::marko::render_module(&hir, "ui.pw", &signals);
    let skipped = |name: &str| {
        out.skipped
            .iter()
            .find(|s| s.name == name)
            .map(|s| s.reason.clone())
            .unwrap_or_default()
    };
    assert!(
        skipped("Open").contains("`drawer` is a signal a page provides"),
        "{:?}",
        out.skipped
    );
    assert!(
        skipped("Panel").contains("a `provide`"),
        "{:?}",
        out.skipped
    );
    // Control: a view's own signal is Marko's own state.
    let counter = out
        .files
        .iter()
        .find(|(f, _)| f == "Counter.marko")
        .map(|(_, t)| t.clone())
        .expect("rendered");
    assert!(counter.contains("<let/count=0>"), "{counter}");
}
