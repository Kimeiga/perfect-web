//! **Every code the compiler writes is registered, once** (ADR-0239).
//!
//! The registry is the codes' one source of truth (`codes.rs`), and nothing
//! compared what the compiler writes with it:
//! - the declaration rules wrote PW0101 and PW0102, which it did not have, so
//!   a diagnostic carrying either had no symbol;
//! - the parser wrote PW0102 too, for a `for` with no `in`: one number, two
//!   meanings;
//! - a reader's value in a shared cache was two errors, PW0100 from the
//!   declaration rules and PW5001 from the label algebra, for one clause.
//!
//! The registry's own tests read the compiler for the codes it writes, as
//! rustc's `tidy` does (`codes.rs`). These state what a program is told.

use pw_core::check::check_sources;
use pw_core::diagnostics::UNREGISTERED;

/// The platform's packages, and `src` as `app.pw` after them.
fn program(src: &str) -> Vec<(String, String)> {
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    let mut paths: Vec<std::path::PathBuf> = std::fs::read_dir(root.join("packages/pw-std"))
        .expect("pw-std")
        .map(|e| e.expect("entry").path())
        .filter(|p| p.extension().is_some_and(|x| x == "pw"))
        .collect();
    paths.sort();
    let mut out: Vec<(String, String)> = paths
        .into_iter()
        .map(|p| {
            (
                p.display().to_string(),
                std::fs::read_to_string(&p).expect("read"),
            )
        })
        .collect();
    out.push(("app.pw".to_string(), src.to_string()));
    out
}

/// What the checker reports of `src`: each code, its symbol and its message.
fn reported(src: &str) -> Vec<(String, &'static str, String)> {
    check_sources(&program(src))
        .into_iter()
        .filter(|(n, _)| n == "app.pw")
        .flat_map(|(_, ds)| ds)
        .map(|d| (d.code.to_string(), d.symbol(), d.message.clone()))
        .collect()
}

#[test]
fn a_readers_value_in_a_shared_cache_is_one_error() {
    // Each with a freshness, which a shared cache asks for (PW0200); a
    // reader's of none, since its state is not served stale (PW0102).
    let src = |visibility: &str, freshness: &str| {
        format!(
            "module c\n\n{visibility} query Cart(s: String) -> Int\n    freshness {freshness}\n    \
             cache shared\n{{\n    0\n}}\n"
        )
    };
    // The control: a public value is shareable.
    assert_eq!(reported(&src("public", "30.seconds")), vec![]);
    for visibility in ["session", "private"] {
        let found = reported(&src(visibility, "0.seconds"));
        assert_eq!(found.len(), 1, "{visibility}: {found:#?}");
        assert_eq!(found[0].0, "PW5001", "{visibility}: {found:#?}");
        assert_eq!(found[0].1, "private_in_shared_cache");
    }
    // With charter §16.3's shape: where the label came from, why, and the
    // repair.
    let program = program(&src("session", "0.seconds"));
    let d = check_sources(&program)
        .into_iter()
        .filter(|(n, _)| n == "app.pw")
        .flat_map(|(_, ds)| ds)
        .next()
        .expect("the error");
    assert!(!d.related.is_empty(), "an origin span");
    assert!(d.explanation.is_some(), "why");
    assert!(!d.repairs.is_empty(), "a repair");
}

#[test]
fn each_code_has_one_meaning() {
    // A `for` with no `in`, and a stale session read: PW0102 named both.
    let for_without_in =
        reported("module f\n\nfn f(xs: List<Int>) -> Int !{} {\n    for x xs { x }\n    0\n}\n");
    assert_eq!(
        for_without_in,
        [(
            "PW0018".to_string(),
            "for_needs_in",
            "expected `in` after the loop binding".to_string()
        )]
    );
    let stale = reported(
        "module c\n\nsession query Cart(s: String) -> Int\n    freshness 30.seconds\n{\n    0\n}\n",
    );
    assert_eq!(stale.len(), 1, "{stale:#?}");
    assert_eq!(
        (stale[0].0.as_str(), stale[0].1),
        ("PW0102", "session_state_is_fresh")
    );
    let read_your_writes = reported(
        "module s\n\npublic query Store(id: Int) -> Int\n    consistency read_your_writes\n{\n    0\n}\n",
    );
    assert_eq!(read_your_writes.len(), 1, "{read_your_writes:#?}");
    assert_eq!(
        (read_your_writes[0].0.as_str(), read_your_writes[0].1),
        ("PW0101", "read_your_writes_needs_a_session")
    );
    // Every symbol registered.
    for (code, symbol, _) in for_without_in.iter().chain(&stale).chain(&read_your_writes) {
        assert_ne!(*symbol, UNREGISTERED, "{code}");
    }
}
