//! **A key a page changes** (ADR-0152).
//!
//! A page's `let found = query Search(id, term)`, where `term` is a page
//! signal, is a binding keyed by `term`: the browser reads it again, for the
//! new key, when `term` changes. Until 2026-10-03 PW5301 refused the signal
//! there, as a read in the page's body the server makes once. Two rules hold
//! a key now: it is a `String`, an `Int` or a `Bool` (PW5308), and its query
//! says what its stale work does (PW5309).
//!
//! Each test states one part, with controls.

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

fn plan(src: &str) -> Result<pw_core::page_values::PageValues, String> {
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
    pw_core::build::build(&units)
        .expect("builds")
        .pages
        .into_iter()
        .find(|p| p.page == "t.P")
        .expect("a page")
        .plan
}

/// A search page: `stale` is the query's `on_key_change` line, `signal` the
/// page's signal, `lets` its bindings.
fn page(stale: &str, signal: &str, lets: &str) -> String {
    format!(
        "module t\n\n\
         type Item = Item {{ id: String, name: String }}\n\n\
         public query Search(id: String, term: String) -> List<Item>\n    \
         freshness     30.seconds\n    consistency   snapshot\n    cache         shared\n    \
         key           id, term\n{stale}    timeout       2.seconds\n{{\n    todo\n}}\n\n\
         page P(id: String) {{\n    cache private\n\n    {signal}\n{lets}\n\n    view {{\n        \
         <main>\n            <input type=\"search\" aria-label=\"Search\" bind:value={{term}} />\n            \
         <ul>{{#each found as item (item.id)}}<li>{{item.name}}</li>{{/each}}</ul>\n        \
         </main>\n    }}\n}}\n"
    )
}

const CANCEL: &str = "    on_key_change cancel\n";
const TERM: &str = "signal term: String = \"co\"";
const KEYED: &str = "    let found = query Search(id, term)";

#[test]
fn a_page_query_given_a_signal_is_planned_keyed_by_it() {
    let src = page(CANCEL, TERM, KEYED);
    assert_eq!(reported(&src), Vec::<String>::new());
    let plan = plan(&src).expect("a plan");
    let found = plan
        .bindings
        .iter()
        .find(|b| b.binding == "found")
        .expect("the binding");
    assert_eq!(found.args, ["id", "term"]);
    assert_eq!(found.signals, ["term"]);
    assert_eq!(found.policy.on_key_change.as_deref(), Some("cancel"));
    // The document is rendered for the signal's first value.
    let term = plan
        .signals
        .iter()
        .find(|s| s.name == "term")
        .expect("term");
    assert_eq!(term.initial, serde_json::json!("co"));
    // The list it fills is the server's to render.
    assert!(plan.collections.contains(&"found".to_string()));
}

#[test]
fn a_binding_keyed_by_parameters_alone_has_no_signals() {
    // The control: the same query given the page's parameter twice.
    let src = page(CANCEL, TERM, "    let found = query Search(id, id)");
    assert_eq!(reported(&src), Vec::<String>::new());
    let found = plan(&src)
        .expect("a plan")
        .bindings
        .into_iter()
        .find(|b| b.binding == "found")
        .expect("the binding");
    assert!(found.signals.is_empty());
    // A literal is no key a host computes, a signal's or a parameter's: the
    // plan refuses it, by name.
    let literal = page(CANCEL, TERM, "    let found = query Search(id, \"co\")");
    let refused = plan(&literal).expect_err("a literal key");
    assert!(refused.contains("nor a page signal"), "{refused}");
}

#[test]
fn a_key_is_a_string_an_int_or_a_bool() {
    for (signal, ok) in [
        ("signal term: String = \"co\"", true),
        ("signal term: Int = 0", true),
        ("signal term: Bool = false", true),
        ("signal term: List<String> = []", false),
        ("signal term: Option<String> = None", false),
    ] {
        let reported = reported(&page(CANCEL, signal, KEYED));
        let refused: Vec<&String> = reported
            .iter()
            .filter(|d| d.starts_with("PW5308"))
            .collect();
        assert_eq!(refused.is_empty(), ok, "{signal}: {reported:?}");
        if !ok {
            assert!(
                refused[0].contains("`term` keys `found`, and is a signal of"),
                "{refused:?}"
            );
        }
    }
}

#[test]
fn a_query_a_signal_keys_says_what_its_stale_work_does() {
    let reported = reported(&page("", TERM, KEYED));
    assert_eq!(
        reported
            .iter()
            .filter(|d| d.starts_with("PW5309"))
            .collect::<Vec<_>>(),
        ["PW5309 `term` keys `found`, and `t.Search` declares no `on_key_change`"]
    );
    // Each of the three words is enough.
    for word in ["cancel", "supersede", "keep"] {
        let stale = format!("    on_key_change {word}\n");
        assert!(
            !reported_any(&page(&stale, TERM, KEYED), "PW5309"),
            "{word}"
        );
    }
}

fn reported_any(src: &str, code: &str) -> bool {
    reported(src).iter().any(|d| d.starts_with(code))
}

#[test]
fn a_signal_read_in_the_body_otherwise_is_still_read_once() {
    // PW5301's control: the signal given to a function in the body, not to a
    // query a page binds, is the server reading its first value once.
    let lets = format!("{KEYED}\n    let shout = String.upper(term)");
    let reported = reported(&page(CANCEL, TERM, &lets));
    assert!(
        reported
            .iter()
            .any(|d| d.starts_with("PW5301 `term` is a signal, and this reads it once")),
        "{reported:?}"
    );
    // And the key itself is not.
    assert_eq!(
        reported.iter().filter(|d| d.starts_with("PW5301")).count(),
        1,
        "{reported:?}"
    );
}
