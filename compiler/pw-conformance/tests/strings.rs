//! **A string's escapes, compiled** (ADR-0049).
//!
//! Until 2026-09-25 a string literal with a backslash, or a `"""` one, had no
//! value: the Wasm lowering and the handler backend refused it (A-023). Each
//! query here is compiled to a component and run through the E8 host, and its
//! result is compared with the characters the escapes name, written in Rust.

use std::collections::BTreeMap;

use pw_conformance::{Runnable, compile, units};
use pw_host::engine::Val;

const PROGRAM: &str = r#"module e

import String

public query Controls() -> String { "a\nb\tc\rd" }

public query Quoted() -> String { "say \"hi\" \\ done" }

public query Braces() -> String { "\{not a hole\}" }

public query Scalars() -> String { "\u{41}\u{e9}\u{4EBA}\u{1F600}" }

public query Counted() -> Int { String.length("\u{1F600}\n") }

public query Framed(text: String) -> String { "[\t{text}\n]" }

public query Mixed(n: Int) -> String { "\{{n}\}" }
"#;

fn compiled(id: &str) -> Runnable {
    Runnable::new(compile(&units(&[("e.pw", PROGRAM)]), id))
}

fn call(id: &str, args: &[Val]) -> Val {
    compiled(id)
        .call(&BTreeMap::new(), args)
        .map(|mut out| out.remove(0))
        .unwrap_or_else(|e| panic!("{id}: {e}"))
}

fn text(s: &str) -> Val {
    Val::String(s.to_string())
}

#[test]
fn each_escape_is_the_character_it_names() {
    assert_eq!(call("e.Controls", &[]), text("a\nb\tc\rd"));
    assert_eq!(call("e.Quoted", &[]), text("say \"hi\" \\ done"));
    assert_eq!(call("e.Braces", &[]), text("{not a hole}"));
    assert_eq!(call("e.Scalars", &[]), text("A\u{e9}\u{4EBA}\u{1F600}"));
}

#[test]
fn an_escape_is_one_code_point() {
    // `String.length` counts code points: the emoji and the line feed.
    assert_eq!(call("e.Counted", &[]), Val::S64(2));
}

#[test]
fn escapes_and_holes_share_a_string() {
    assert_eq!(call("e.Framed", &[text("x y")]), text("[\tx y\n]"));
    // An escaped brace beside a hole: the braces are text, the hole a value.
    assert_eq!(call("e.Mixed", &[Val::S64(7)]), text("{7}"));
}

#[test]
fn an_escape_the_language_does_not_define_does_not_compile() {
    let bad = "module b\n\npublic query Bad() -> String { \"a\\qb\" }\n";
    let parsed = pw_syntax::parse_tree(bad);
    assert!(
        parsed.errors.iter().any(|e| e.code == "PW0014"),
        "{:?}",
        parsed.errors.iter().map(|e| e.code).collect::<Vec<_>>()
    );
}
