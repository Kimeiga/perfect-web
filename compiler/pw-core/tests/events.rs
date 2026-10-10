//! **A handler is given its event** (ADR-0138, ADR-0131's first slice).
//!
//! An event gives its handler one record of plain data, read in the browser's
//! listener before any code loads: `on:input={(e: InputEvent) => q = e.value}`.
//! Until 2026-10-02 none of it existed:
//! - a lambda's written parameter type was dropped by the HIR, so a written
//!   type checked nothing, in any lambda;
//! - a handler with a parameter was refused at build, so no handler could
//!   read what was typed;
//! - the parts manifest did not say which event a part handles, and the
//!   runtime listened for a click whatever it was.
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

fn reported(src: &str) -> Vec<String> {
    let mut sources = library();
    sources.push(("t.pw".to_string(), src.to_string()));
    check_sources(&sources)
        .into_iter()
        .filter(|(n, _)| n == "t.pw")
        .flat_map(|(_, ds)| ds.into_iter().map(|d| format!("{} {}", d.code, d.message)))
        .collect()
}

fn build(src: &str) -> pw_core::build::Build {
    let mut sources = library();
    sources.push(("t.pw".to_string(), src.to_string()));
    let units: Vec<Unit> = sources
        .into_iter()
        .map(|(path, src)| Unit {
            path,
            hir: lower_file(&src, &parse_tree(&src).green),
            src,
        })
        .collect();
    pw_core::build::build(&units).expect("builds")
}

/// A page with a signal `q`, whose view holds `markup`.
fn page(markup: &str) -> String {
    format!(
        "module t\n\nimport events.{{ InputEvent, PressEvent, KeyEvent }}\n\n\
         page P() {{\n    cache private\n\n    signal q: String = \"\"\n\n    \
         view {{\n        <main>{markup}<p>{{q}}</p></main>\n    }}\n}}\n"
    )
}

/// What a handler module does when its event is `event`: each signal it
/// sets, in order, under Node.
fn sets(module: &pw_core::backend::js::HandlerModule, event: &str) -> serde_json::Value {
    let temp = tempfile::TempDir::with_prefix("pw-event-").expect("a temporary directory");
    let dir = temp.path().to_path_buf();
    std::fs::write(dir.join("handler.mjs"), &module.source).expect("write");
    std::fs::write(
        dir.join("run.mjs"),
        "import { run } from \"./handler.mjs\";\n\
         const sets = [];\n\
         await run({\n  captures: {},\n  event: JSON.parse(process.argv[2]),\n  \
         get: () => undefined,\n  set: (name, value) => sets.push([name, value]),\n  \
         command: async () => ({ $case: \"ok\" }),\n});\n\
         console.log(JSON.stringify(sets));\n",
    )
    .expect("write");
    let out = std::process::Command::new("node")
        .arg(dir.join("run.mjs"))
        .arg(event)
        .output()
        .expect("node runs");
    assert!(
        out.status.success(),
        "{}",
        String::from_utf8_lossy(&out.stderr)
    );
    serde_json::from_slice(&out.stdout).expect("JSON")
}

/// The one handler module a build compiled.
fn the_module(b: &pw_core::build::Build) -> pw_core::backend::js::HandlerModule {
    let modules: Vec<_> = b
        .handlers
        .iter()
        .filter_map(|h| match &h.module {
            Encoding::Encoded(m) => Some(m.clone()),
            _ => None,
        })
        .collect();
    assert_eq!(modules.len(), 1, "{:?}", b.refusals());
    modules[0].clone()
}

#[test]
fn a_handler_is_given_its_event_s_record() {
    for handler in [
        // Its type written, and its type the event's where none is.
        "(e: InputEvent) => q = e.value",
        "(e) => q = e.value",
    ] {
        let src = page(&format!(
            "<input type=\"text\" aria-label=\"Search\" on:input={{{handler}}} />"
        ));
        assert_eq!(reported(&src), Vec::<String>::new(), "{handler}");
        let b = build(&src);
        assert!(b.refusals().is_empty(), "{handler}: {:?}", b.refusals());
        let m = the_module(&b);
        assert!(m.source.contains("context.event"), "{}", m.source);
        assert_eq!(
            sets(&m, r#"{"value":"cortado"}"#),
            serde_json::json!([["q", "cortado"]]),
            "{handler}"
        );
    }
    // Control: a handler that takes nothing reads no event.
    let src = page("<button type=\"button\" on:press={() => q = \"pressed\"}>Go</button>");
    let m = the_module(&build(&src));
    assert!(!m.source.contains("context.event"), "{}", m.source);
}

#[test]
fn a_handler_written_for_another_event_is_refused() {
    let wrong = page(
        "<input type=\"text\" aria-label=\"Search\" on:input={(e: PressEvent) => q = \"x\"} />",
    );
    let found = reported(&wrong);
    assert!(
        found.iter().any(|d| d.starts_with(
            "PW0602 `on:input` expects a handler taking `InputEvent`, or nothing, found one \
             taking `PressEvent`"
        )),
        "{found:#?}"
    );
    let two = page("<input type=\"text\" aria-label=\"Search\" on:input={(a, b) => q = \"x\"} />");
    let found = reported(&two);
    assert!(
        found
            .iter()
            .any(|d| d.starts_with("PW0602") && d.contains("found one taking 2 parameters")),
        "{found:#?}"
    );
    // A field the event's record does not have is the record's to refuse.
    let field = page("<input type=\"text\" aria-label=\"Search\" on:input={(e) => q = e.key} />");
    let found = reported(&field);
    assert!(
        found
            .iter()
            .any(|d| d.starts_with("PW0610") && d.contains("`key`")),
        "{found:#?}"
    );
    // Control: the key is a keydown's.
    let key = page(
        "<input type=\"text\" aria-label=\"Search\" on:keydown={(e: KeyEvent) => q = e.key} />",
    );
    assert_eq!(reported(&key), Vec::<String>::new());
}

#[test]
fn a_modifier_is_one_the_runtime_applies() {
    let form = |modifier: &str| {
        page(&format!(
            "<form on:submit{modifier}={{() => q = \"sent\"}}>\
             <button type=\"submit\">Send</button></form>"
        ))
    };
    let found = reported(&form("|prevnt"));
    assert_eq!(
        found,
        [
            "PW5027 `on:submit|prevnt` names no modifier the runtime applies: `prevent`, `stop` or `refusable`"
        ]
    );
    for ok in ["|prevent", "|stop", "|prevent|stop", ""] {
        assert_eq!(reported(&form(ok)), Vec::<String>::new(), "{ok}");
    }
}

#[test]
fn the_manifest_says_which_event_a_part_handles() {
    let src = page(
        "<input type=\"text\" aria-label=\"Search\" on:input={(e) => q = e.value} \
         on:keydown={(e) => q = e.key} />\
         <form on:submit|prevent={() => q = \"sent\"}><button type=\"submit\">Send</button></form>",
    );
    let b = build(&src);
    let t = b
        .templates
        .iter()
        .find(|t| t.path == "t.P")
        .expect("the page");
    let events: Vec<(String, Vec<String>)> = t
        .manifest()
        .into_iter()
        .filter(|e| e.kind == "event")
        .map(|e| (e.event, e.modifiers))
        .collect();
    assert_eq!(
        events,
        [
            ("input".to_string(), vec![]),
            ("keydown".to_string(), vec![]),
            ("submit".to_string(), vec!["prevent".to_string()]),
        ]
    );
}

#[test]
fn a_lambda_s_written_parameter_type_is_checked() {
    // Dropped until 2026-10-02, in every lambda: `Nada` checked clean.
    let src = "module t\n\nfn f() -> Int !{} {\n    let g = fn(y: Nada) 1\n    1\n}\n";
    let found = reported(src);
    assert_eq!(
        found,
        ["PW0026 parameter `y` names `Nada`: `Nada` is not a type visible here"],
        "{found:#?}"
    );
    // And a written one types its parameter: an `Int` has no `value`.
    let src = "module t\n\nfn f() -> Int !{} {\n    let g = fn(y: Int) y.value\n    1\n}\n";
    let found = reported(src);
    assert!(found.iter().any(|d| d.starts_with("PW0610")), "{found:#?}");
}

#[test]
fn an_element_s_handlers_are_lowered_last_and_together() {
    // The renderer writes an element's captures once, for the run of its
    // handlers (ADR-0138), so they are adjacent whatever is written between
    // them, and the element's other attributes come first.
    let src = page(
        "<input on:input={(e) => q = e.value} type=\"text\" aria-label=\"Search\" \
         on:keydown={(e) => q = e.key} />",
    );
    assert_eq!(reported(&src), Vec::<String>::new());
    let b = build(&src);
    let t = b
        .templates
        .iter()
        .find(|t| t.path == "t.P")
        .expect("the page");
    let shape: Vec<String> = t
        .chunks
        .iter()
        .map(|c| match c {
            pw_core::template_ir::Chunk::Static(s) => format!("static {s}"),
            pw_core::template_ir::Chunk::Dynamic(p) => p.kind().to_string(),
        })
        .collect();
    let input = shape
        .iter()
        .position(|s| s.contains("<input"))
        .expect("the input");
    assert!(
        shape[input].contains("type=\"text\" aria-label=\"Search\""),
        "{shape:#?}"
    );
    assert_eq!(shape[input + 1], "event", "{shape:#?}");
    assert_eq!(shape[input + 2], "event", "{shape:#?}");
}
