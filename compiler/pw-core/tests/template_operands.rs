//! **What a template's blocks and events take** (ADR-0071).
//!
//! Until 2026-09-26 none of these was related, so each passed `pw check`:
//! - `{#each n}` over a value that is not a list, which the renderer cannot
//!   run over;
//! - `{#if s}` testing a sum type, which the renderer refuses when it
//!   renders: a case is taken apart with `{#match}`;
//! - `on:press={n}`, an event attribute given a value that is not a function.
//!
//! `{#if n}` over a number, a string or a list is the renderer's truth, and
//! stays: ADR-0042 relies on it for a list. Each test states one case, with a
//! control.

use pw_core::check::{Unit, check_sources};
use pw_core::lower::lower_file;
use pw_core::values::Outcome;
use pw_syntax::parse_tree;

fn files(dirs: &[&str]) -> Vec<(String, String)> {
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    let mut out = Vec::new();
    for d in dirs {
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

fn program(src: &str) -> Vec<(String, String)> {
    let mut all = files(&["packages/pw-std", "packages/pw-platform-web"]);
    all.push(("t.pw".to_string(), src.to_string()));
    all
}

fn reported(src: &str) -> Vec<String> {
    let all = program(src);
    check_sources(&all)
        .into_iter()
        .filter(|(n, _)| n == "t.pw")
        .flat_map(|(_, ds)| ds.into_iter().map(|d| format!("{} {}", d.code, d.message)))
        .collect()
}

fn refused(src: &str, code: &str) {
    let found = reported(src);
    assert!(!found.is_empty(), "nothing reported");
    assert!(found.iter().any(|d| d.starts_with(code)), "{found:#?}");
}

fn none(src: &str) {
    let found = reported(src);
    assert!(found.is_empty(), "{found:#?}");
}

#[test]
fn an_each_runs_over_a_list() {
    let wrong = "module t\n\nview V(n: Int) !{} {\n    <ul>\n        {#each n as x (x)}\n            <li>{x}</li>\n        {/each}\n    </ul>\n}\n";
    refused(wrong, "PW0609");
    none(&wrong.replace("V(n: Int)", "V(n: List<Int>)"));
}

#[test]
fn an_if_tests_a_value_with_a_truth() {
    let wrong = "module t\n\ntype Shape =\n    | Circle(Int)\n    | Empty\n\nview V(s: Shape, n: Int) !{} {\n    <div>\n        {#if s}\n            <p>yes</p>\n        {/if}\n    </div>\n}\n";
    refused(wrong, "PW0609");
    // A number has a truth: not zero.
    none(&wrong.replace("{#if s}", "{#if n}"));
    // `{:else if}` tests its condition as `{#if}` does.
    let branch = wrong.replace(
        "{#if s}",
        "{#if n}\n            <p>n</p>\n        {:else if s}",
    );
    refused(&branch, "PW0609");
    none(&branch.replace("{:else if s}", "{:else if n}"));
}

/// An `Option` tested by `{#if}` is PW0600's, which says the same, so it is
/// reported once. No relation says such a condition agrees: that would count
/// a condition the renderer refuses as a decided agreement.
#[test]
fn an_option_condition_is_reported_once() {
    let src = "module t\n\nview V(o: Option<Int>) !{} {\n    <div>\n        {#if o}\n            <p>yes</p>\n        {/if}\n    </div>\n}\n";
    let found = reported(src);
    assert_eq!(found.len(), 1, "{found:#?}");
    assert!(found[0].starts_with("PW0600"), "{found:#?}");

    let units: Vec<Unit> = program(src)
        .into_iter()
        .map(|(path, src)| {
            let hir = lower_file(&src, &parse_tree(&src).green);
            Unit { path, src, hir }
        })
        .collect();
    let agreeing: Vec<String> = pw_core::values::analysis(&units)
        .into_iter()
        .filter(|(p, _)| p == "t.pw")
        .flat_map(|(_, rs)| rs)
        .filter(|r| r.target == "{#if}" && r.outcome == Outcome::Agree)
        .map(|r| r.target)
        .collect();
    assert!(agreeing.is_empty(), "{agreeing:#?}");
}

#[test]
fn an_event_attribute_is_given_a_function() {
    let wrong = "module t\n\nfn go() -> () !{} { todo }\n\nview V(n: Int) !{} {\n    <button type=\"button\" on:press={n}>Go</button>\n}\n";
    refused(wrong, "PW0614");
    none(&wrong.replace("on:press={n}", "on:press={go}"));
    none(&wrong.replace("on:press={n}", "on:press={() => go()}"));
    // An attribute that is not an event takes a value, not a function.
    none(&wrong.replace("on:press={n}", "title={n} on:press={go}"));
}
