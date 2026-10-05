//! **A `String`'s length is an invariant too** (ADR-0225).
//!
//! `opaque type PostText = String where String.length(value) >= 1 &
//! String.length(value) <= 280`: bounds on the length in code points, as
//! `String.length` counts it (ADR-0040). Every construction is shown to hold
//! them at build (PW0622), as an `Int`'s bounds are (ADR-0179), and a host
//! holds a value from outside to them. Every refusal has an accepted
//! neighbour in the same shape.

use pw_core::check::check_sources;

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

/// The diagnostics for `t.pw`, `code message` each.
fn diagnostics(src: &str) -> Vec<String> {
    let mut sources = library();
    sources.push(("t.pw".to_string(), src.to_string()));
    check_sources(&sources)
        .into_iter()
        .filter(|(n, _)| n == "t.pw")
        .flat_map(|(_, ds)| ds.into_iter().map(|d| format!("{} {}", d.code, d.message)))
        .collect()
}

const HANDLE: &str =
    "opaque type Handle = String where String.length(value) >= 3 & String.length(value) <= 15\n\n";

/// One function, under `Handle`.
fn with(f: &str) -> String {
    format!("module t\n\nimport String\n\n{HANDLE}{f}\n")
}

#[test]
fn a_literal_is_its_length_in_code_points() {
    assert_eq!(
        diagnostics(&with("fn h() -> Handle { Handle(\"ada\") }")),
        Vec::<String>::new()
    );
    // Three code points, twelve bytes; and fifteen, thirty bytes.
    assert_eq!(
        diagnostics(&with(
            "fn h() -> Handle { Handle(\"\u{1F600}\u{1F600}\u{1F600}\") }"
        )),
        Vec::<String>::new()
    );
    let fifteen = format!("fn h() -> Handle {{ Handle(\"{}\") }}", "\u{e9}".repeat(15));
    assert_eq!(diagnostics(&with(&fifteen)), Vec::<String>::new());
    let short = diagnostics(&with("fn h() -> Handle { Handle(\"\u{e9}\u{e9}\") }"));
    assert_eq!(short.len(), 1, "{short:?}");
    assert!(
        short[0].starts_with("PW0622")
            && short[0].contains("String.length(value) >= 3")
            && short[0].ends_with("it is 2 code points long"),
        "{short:?}"
    );
    let long = diagnostics(&with("fn h() -> Handle { Handle(\"abcdefghijklmnop\") }"));
    assert!(long[0].ends_with("it is 16 code points long"), "{long:?}");
}

#[test]
fn a_test_of_the_length_narrows_it() {
    let checked = "fn h(s: String) -> Option<Handle> {\n    if String.length(s) >= 3 & String.length(s) <= 15 {\n        Some(Handle(s))\n    } else {\n        None\n    }\n}";
    assert_eq!(diagnostics(&with(checked)), Vec::<String>::new());
    // Half a test: its length may be anything above.
    let half = diagnostics(&with(
        "fn h(s: String) -> Option<Handle> {\n    if String.length(s) >= 3 {\n        Some(Handle(s))\n    } else {\n        None\n    }\n}",
    ));
    assert!(
        half[0].ends_with("it is at least 3 code points long"),
        "{half:?}"
    );
    // Its negation, in the `else`.
    let negated = diagnostics(&with(
        "fn h(s: String) -> Option<Handle> {\n    if String.length(s) < 3 {\n        None\n    } else {\n        Some(Handle(s))\n    }\n}",
    ));
    assert!(
        negated[0].ends_with("it is at least 3 code points long"),
        "{negated:?}"
    );
    // A test of another value narrows nothing.
    let other = diagnostics(&with(
        "fn h(s: String, t: String) -> Option<Handle> {\n    if String.length(t) >= 3 & String.length(t) <= 15 {\n        Some(Handle(s))\n    } else {\n        None\n    }\n}",
    ));
    assert!(
        other[0].ends_with("it is a `String` of any length"),
        "{other:?}"
    );
    // Nor does a value's test of its own: a length is not a value.
    let value = diagnostics(&with(
        "fn h(s: String, n: Int) -> Option<Handle> {\n    if n >= 3 & n <= 15 {\n        Some(Handle(s))\n    } else {\n        None\n    }\n}",
    ));
    assert!(
        value[0].ends_with("it is a `String` of any length"),
        "{value:?}"
    );
}

#[test]
fn a_handles_value_a_let_and_a_branch_keep_their_lengths() {
    for f in [
        "fn h(x: Handle) -> Handle { Handle(x.value) }",
        "fn h() -> Handle {\n    let t = \"grace\"\n    Handle(t)\n}",
        "fn h(c: Bool) -> Handle { Handle(if c { \"abc\" } else { \"abcdef\" }) }",
    ] {
        assert_eq!(diagnostics(&with(f)), Vec::<String>::new(), "{f}");
    }
    let either = diagnostics(&with(
        "fn h(c: Bool) -> Handle { Handle(if c { \"ab\" } else { \"abcdef\" }) }",
    ));
    assert!(
        either[0].ends_with("it is from 2 to 6 code points long"),
        "{either:?}"
    );
}

#[test]
fn a_length_invariant_the_language_cannot_read_is_refused_where_it_is_written() {
    for (inv, why) in [
        (
            "String where String.length(value) >= 1 & value >= 1",
            "an invariant bounds one thing",
        ),
        (
            "Int where String.length(value) <= 3",
            "states an invariant on `String.length(value)` over `Int`, which only a",
        ),
        (
            "String where String.length(value) <= -1",
            "holds of no value",
        ),
        (
            "String where String.length(other) <= 3",
            "compares `value`, or `String.length(value)`, with an integer",
        ),
    ] {
        let found = diagnostics(&format!(
            "module t\n\nimport String\n\nopaque type X = {inv}\n"
        ));
        assert!(
            found
                .iter()
                .any(|d| d.starts_with("PW0623") && d.contains(why)),
            "{inv}: {found:?}"
        );
    }
    // Controls: a length's bounds alone, either way round, and an `Int`'s
    // value.
    for inv in [
        "String where String.length(value) <= 280",
        "String where 1 <= String.length(value)",
        "Int where value >= 1",
    ] {
        assert_eq!(
            diagnostics(&format!(
                "module t\n\nimport String\n\nopaque type X = {inv}\n"
            )),
            Vec::<String>::new(),
            "{inv}"
        );
    }
}

/// **The feed's post states its text's length where it arrives**
/// (ADR-0225): the contract bounds `post`'s argument, by its length, from 1
/// to 280, for the host to hold a browser's request to.
#[test]
fn the_feeds_post_states_its_texts_length_at_the_boundary() {
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    let mut sources = library();
    sources.push((
        "examples/feed/app.pw".to_string(),
        std::fs::read_to_string(root.join("examples/feed/app.pw")).expect("the feed"),
    ));
    let units: Vec<pw_core::check::Unit> = sources
        .into_iter()
        .map(|(path, src)| pw_core::check::Unit {
            hir: pw_core::lower::lower_file(&src, &pw_syntax::parse_tree(&src).green),
            path,
            src,
        })
        .collect();
    let b = pw_core::build::build(&units).expect("builds");
    assert!(b.refusals().is_empty(), "{:?}", b.refusals());
    let post = b
        .contracts
        .iter()
        .find(|c| c.component_id == "feed.app.post")
        .expect("the post's contract");
    let bounded = &post.exports[0]
        .component
        .as_ref()
        .expect("its export")
        .bounded;
    assert_eq!(bounded.len(), 1, "{bounded:?}");
    assert_eq!(
        (
            bounded[0].argument,
            bounded[0].ty.as_str(),
            bounded[0].measure,
            bounded[0].at_least,
            bounded[0].at_most
        ),
        (
            0,
            "feed.app.PostText",
            pw_core::contract::Measure::Length,
            Some(1),
            Some(280)
        )
    );
}
