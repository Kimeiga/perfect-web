//! **A handler navigates after its command commits** (ADR-0280).
//!
//! `navigate Page(args)` names a page, whose parameters type its arguments
//! as a call's are, and is written in a handler, last in the `Ok` arm of the
//! command's answer nearest it. Its module goes to the page's route, each
//! `{name}` segment given its argument, once the answer is in, and only in
//! the `Ok` case.
//!
//! - PW5042: a navigation names a page.
//! - PW0604 and PW0605: its arguments are the page's parameters.
//! - PW5043: it is last in the `Ok` arm of a command's answer, in a handler.
//!
//! Each test states one part, with its controls.

use pw_core::backend::js;
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

fn with(code: &str, reported: &[String]) -> Vec<String> {
    reported
        .iter()
        .filter(|d| d.starts_with(code))
        .cloned()
        .collect()
}

/// A program with a command that declares a `Result` and one that does not,
/// two pages to go to, `Q()` at `/q` and `R(name)` at `/r/{name}`, `extra`
/// declarations, and a button whose handler is `handler`.
fn program(extra: &str, handler: &str) -> String {
    format!(
        "module t\n\n\
         opaque type InteractionId = String\n\n\
         type Cart = Cart {{ n: Int }}\n\n\
         type CartError =\n    | Unavailable\n    | Expired\n\n\
         command add(n: Int) -> Result<Cart, CartError>\n    requires      SignedIn\n    \
         idempotent_by InteractionId\n{{\n    Ok(Cart {{ n: n }})\n}}\n\n\
         command count(n: Int) -> Int\n    requires      SignedIn\n    \
         idempotent_by InteractionId\n{{\n    n\n}}\n\n\
         {extra}\n\n\
         page Q() {{\n    route \"/q\"\n    cache private\n\n    \
         view {{\n        <title>Q</title>\n        <main>\n            <h1>Q</h1>\n        \
         </main>\n    }}\n}}\n\n\
         page R(name: String) {{\n    route \"/r/{{name}}\"\n    cache private\n\n    \
         view {{\n        <title>R</title>\n        <main>\n            <h1>{{name}}</h1>\n        \
         </main>\n    }}\n}}\n\n\
         page P() {{\n    cache private\n\n    signal note: String = \"\"\n\n    \
         view {{\n        <main>\n            \
         <button type=\"button\" on:press={{() => {handler}}}>Add</button>\n        \
         </main>\n    }}\n}}\n"
    )
}

/// What `program("", handler)` reports under the three codes this ruling
/// and its arguments use.
fn navigation_codes(handler: &str) -> Vec<String> {
    let all = reported(&program("", handler));
    ["PW5042", "PW5043", "PW0604", "PW0605"]
        .iter()
        .flat_map(|c| with(c, &all))
        .collect()
}

// --- PW5042 --------------------------------------------------------------------

#[test]
fn a_navigation_names_a_page() {
    let refused =
        navigation_codes("match add(1) { Ok(_) => navigate Nowhere(), Err(_) => note = \"no\" }");
    assert!(
        refused.len() == 1 && refused[0].starts_with("PW5042") && refused[0].contains("nothing"),
        "{refused:?}"
    );
    // A command is no page.
    let command =
        navigation_codes("match add(1) { Ok(_) => navigate count(1), Err(_) => note = \"no\" }");
    assert!(
        command.len() == 1 && command[0].contains("names a command"),
        "{command:?}"
    );
    // The control: a page.
    assert_eq!(
        navigation_codes("match add(1) { Ok(_) => navigate Q(), Err(_) => note = \"no\" }"),
        Vec::<String>::new()
    );
}

// --- PW0604 and PW0605 ---------------------------------------------------------

#[test]
fn its_arguments_are_the_pages_parameters() {
    let none = navigation_codes("match add(1) { Ok(_) => navigate R(), Err(_) => note = \"no\" }");
    assert!(none.len() == 1 && none[0].starts_with("PW0604"), "{none:?}");
    let a_number =
        navigation_codes("match add(1) { Ok(_) => navigate R(1), Err(_) => note = \"no\" }");
    assert!(
        a_number.len() == 1 && a_number[0].starts_with("PW0605"),
        "{a_number:?}"
    );
    // The control: its parameter, text.
    assert_eq!(
        navigation_codes("match add(1) { Ok(_) => navigate R(\"a b\"), Err(_) => note = \"no\" }"),
        Vec::<String>::new()
    );
}

// --- PW5043 --------------------------------------------------------------------

#[test]
fn it_is_written_last_in_the_ok_arm_of_a_commands_answer() {
    for refused in [
        // A refusal's arm: nothing committed.
        "match add(1) { Ok(_) => note = \"\", Err(_) => navigate Q() }",
        // Before the answer: the command is not yet answered.
        "{ navigate Q(); match add(1) { Ok(_) => note = \"\", Err(_) => note = \"no\" } }",
        // After a command that answers nothing: no `Ok` says it committed.
        "{ count(1); navigate Q() }",
        // Not last: what follows it would run on a page being left.
        "match add(1) { Ok(_) => { navigate Q(); note = \"x\" }, Err(_) => note = \"no\" }",
        // The answer nearest it is a refusal's, though an `Ok` encloses it.
        "match add(1) { Ok(_) => match add(2) { Ok(_) => note = \"\", Err(_) => navigate Q() }, \
         Err(_) => note = \"no\" }",
    ] {
        let got = navigation_codes(refused);
        assert!(
            got.len() == 1 && got[0].starts_with("PW5043"),
            "{refused}: {got:?}"
        );
    }
    // The controls: last in an `Ok` arm, through a block, an `if`, a `match`
    // on something else, and an answer bound by a `let`.
    for allowed in [
        "match add(1) { Ok(_) => navigate Q(), Err(_) => note = \"no\" }",
        "match add(1) { Ok(_) => { note = \"x\"; navigate Q() }, Err(_) => note = \"no\" }",
        "match add(1) { Ok(_) => if note == \"\" { navigate Q() } else { note = \"\" }, \
         Err(_) => note = \"no\" }",
        "{ let answer = add(1); match answer { Ok(_) => navigate Q(), Err(_) => note = \"no\" } }",
        "match add(1) { Err(_) => match add(2) { Ok(_) => navigate Q(), Err(_) => note = \"\" }, \
         Ok(_) => note = \"\" }",
    ] {
        assert_eq!(navigation_codes(allowed), Vec::<String>::new(), "{allowed}");
    }
}

#[test]
fn a_navigation_outside_a_handler_is_refused() {
    let got = with(
        "PW5043",
        &reported(&program(
            "fn go() -> Unit {\n    navigate Q()\n}",
            "match add(1) { Ok(_) => note = \"\", Err(_) => note = \"no\" }",
        )),
    );
    assert!(
        got.len() == 1 && got[0].contains("outside a handler"),
        "{got:?}"
    );
}

// --- the module -----------------------------------------------------------------

/// The handler module `program("", handler)` compiles to.
fn module(handler: &str) -> String {
    let mut sources = library();
    sources.push(("t.pw".to_string(), program("", handler)));
    let units: Vec<Unit> = sources
        .into_iter()
        .map(|(path, src)| Unit {
            hir: lower_file(&src, &parse_tree(&src).green),
            path,
            src,
        })
        .collect();
    let compiled = js::compile(&units).expect("the program checks");
    let mut modules = compiled.into_iter().filter_map(|c| match c.module {
        Encoding::Encoded(m) => Some(m.source),
        _ => None,
    });
    let source = modules.next().expect("one handler");
    assert!(modules.next().is_none(), "one handler only");
    source
}

/// What the module did, run under Node with a context whose commands answer
/// `answer`: each command sent, then each navigation, in order.
fn ran(source: &str, answer: &str) -> serde_json::Value {
    static RUNS: std::sync::atomic::AtomicUsize = std::sync::atomic::AtomicUsize::new(0);
    let dir = std::env::temp_dir().join(format!(
        "pw-navigate-{}-{}",
        std::process::id(),
        RUNS.fetch_add(1, std::sync::atomic::Ordering::SeqCst)
    ));
    std::fs::create_dir_all(&dir).expect("temp");
    std::fs::write(dir.join("handler.mjs"), source).expect("write");
    std::fs::write(
        dir.join("run.mjs"),
        "import { run } from \"./handler.mjs\";\n\
         const did = [];\n\
         const context = {\n  captures: {},\n  \
         command: async (id, args) => {\n    did.push([\"command\", id, args]);\n    \
         await new Promise((r) => setTimeout(r, 1));\n    \
         return JSON.parse(process.argv[2]);\n  },\n  \
         navigate: async (route, args) => {\n    did.push([\"navigate\", route, args]);\n  },\n  \
         get: () => \"\",\n  set: (name, value) => did.push([\"set\", name, value]),\n};\n\
         await run(context);\n\
         console.log(JSON.stringify(did));\n",
    )
    .expect("write");
    let out = std::process::Command::new("node")
        .arg("run.mjs")
        .arg(answer)
        .current_dir(&dir)
        .output()
        .expect("node runs: a handler's module is tested under Node");
    std::fs::remove_dir_all(&dir).ok();
    assert!(
        out.status.success(),
        "{}",
        String::from_utf8_lossy(&out.stderr)
    );
    serde_json::from_slice(&out.stdout).expect("one line of JSON")
}

#[test]
fn the_module_goes_to_the_page_after_the_answer_and_only_on_ok() {
    let source = module("match add(1) { Ok(_) => navigate R(\"a b\"), Err(_) => note = \"no\" }");
    // The page's route, and its parameter's value by name: the runtime fills
    // and encodes the segment.
    assert!(
        source.contains("await context.navigate(\"/r/{name}\", { \"name\": "),
        "{source}"
    );
    // Answered `Ok`: sent, then gone, with the value.
    assert_eq!(
        ran(&source, r#"{"$case":"ok"}"#),
        serde_json::json!([
            ["command", "t.add", [1]],
            ["navigate", "/r/{name}", { "name": "a b" }]
        ])
    );
    // Answered its declared error: the `Err` arm runs, and nothing goes.
    assert_eq!(
        ran(
            &source,
            r#"{"$case":"err","value":{"$case":"unavailable"}}"#
        ),
        serde_json::json!([["command", "t.add", [1]], ["set", "note", "no"]])
    );
}

#[test]
fn a_page_with_no_parameters_is_given_none() {
    let source = module("match add(1) { Ok(_) => navigate Q(), Err(_) => note = \"no\" }");
    assert!(
        source.contains("await context.navigate(\"/q\", {});"),
        "{source}"
    );
    assert_eq!(
        ran(&source, r#"{"$case":"ok"}"#),
        serde_json::json!([["command", "t.add", [1]], ["navigate", "/q", {}]])
    );
}
