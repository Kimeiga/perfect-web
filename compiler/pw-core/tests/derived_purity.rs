//! **A `derived` value performs no effect** (ADR-0082).
//!
//! The charter's `derived` is "a pure value computed from other values"
//! (§7.5), recomputed whenever they change. Until 2026-09-26 nothing held it
//! to that, as ADR-0047 recorded: `let t = derived clock.now()` checked, in a
//! declaration whose row allows the clock. Everything the value performs
//! counts: what it calls, the members it reads, and the functions it names
//! (ADR-0078). Each test states one case, with a control.

use pw_core::check::check_sources;

fn program(src: &str) -> Vec<(String, String)> {
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
    out.push(("t.pw".to_string(), src.to_string()));
    out
}

fn reported(decls: &str) -> Vec<String> {
    let src = format!(
        "module t\n\nimport clock\nimport List\nimport browser.{{ ElementRef }}\n\n\
         fn stamp(n: Int) -> Int !{{ clock.read }} {{ clock.now() + n }}\n\n{decls}\n"
    );
    check_sources(&program(&src))
        .into_iter()
        .filter(|(n, _)| n == "t.pw")
        .flat_map(|(_, ds)| ds.into_iter().map(|d| format!("{} {}", d.code, d.message)))
        .collect()
}

fn impure(decls: &str, effect: &str) {
    let found = reported(decls);
    assert_eq!(found.len(), 1, "{decls}: {found:#?}");
    assert!(
        found[0].starts_with("PW0334")
            && found[0].contains(&format!("`derived` value performs `{effect}`")),
        "{decls}: {found:#?}"
    );
}

fn clean(decls: &str) {
    let found = reported(decls);
    assert!(found.is_empty(), "{decls}: {found:#?}");
}

#[test]
fn a_derived_value_performs_no_effect() {
    impure(
        "fn f() -> Int !{ clock.read } {\n    let t = derived clock.now()\n    t\n}",
        "clock.read",
    );
    clean("fn f(n: Int) -> Int !{} {\n    let t = derived n + 1\n    t\n}");
    // An effect beside it, in the same body, is the body's.
    clean(
        "fn f() -> Int !{ clock.read } {\n    let a = clock.now()\n    let t = derived a + 1\n    t\n}",
    );
}

#[test]
fn what_a_derived_value_names_and_reads_counts() {
    // A function named as a value (ADR-0078), and one written in place.
    impure(
        "fn f(xs: List<Int>) -> List<Int> !{ clock.read } {\n    let ys = derived List.map(xs, stamp)\n    ys\n}",
        "clock.read",
    );
    impure(
        "fn f(xs: List<Int>) -> List<Int> !{ clock.read } {\n    let ys = derived List.map(xs, (x) => clock.now())\n    ys\n}",
        "clock.read",
    );
    // A member it reads.
    impure(
        "fn f(el: ElementRef) -> Float !{ layout.measure } {\n    let w = derived el.offsetWidth\n    w\n}",
        "layout.measure",
    );
}
