//! Source mapping for interpolation holes.
//!
//! Holes are parsed by the real grammar inside a synthetic wrapper whose
//! prefix is space-padded to the hole's own offset, so spans come back already
//! pointing into the real file. That is the right technique — the alternative
//! is a second grammar that disagrees with the first — but it has one property
//! everything depends on, and it is arithmetic:
//!
//! ```text
//! a span produced inside the synthetic wrapper
//!     == the exact byte span in the original .pw file
//! ```
//!
//! Architect ruling, 2026-08-06: test it against multibyte text, nested
//! braces, braces inside strings, comments, multiline holes, first and last
//! bytes, nested interpolation, and malformed expressions. A misleading span
//! is worse than no diagnostic, because it sends the reader to the wrong line
//! with confidence.

use pw_core::hir::Expr;
use pw_core::lower::lower_file;
use pw_syntax::parse_tree;

/// Every `Name` inside an interpolation, as `(text, the source it spans)`.
fn holes(src: &str) -> Vec<(String, String)> {
    let hir = lower_file(src, &parse_tree(src).green);
    let mut out = Vec::new();
    for (_, d) in hir.all_decls() {
        let Some(b) = d.body else { continue };
        let body = hir.body(b);
        for id in body.walk() {
            let Expr::Interpolated { parts, .. } = body.expr(id) else {
                continue;
            };
            for p in parts {
                for inner in body.walk_from(*p) {
                    if let Expr::Name(n) = body.expr(inner) {
                        let s = body.expr_span(inner);
                        out.push((
                            n.clone(),
                            src.get(s.start..s.end)
                                .unwrap_or("<OUT OF BOUNDS>")
                                .to_string(),
                        ));
                    }
                }
            }
        }
    }
    out
}

fn wrap(body: &str) -> String {
    format!("module m\n\nfn f() -> () !{{}} {{\n    {body}\n}}\n")
}

/// The core property, stated once: a hole's span is the hole's own bytes.
fn assert_spans_itself(src: &str, expected: &[&str]) {
    let found = holes(src);
    let names: Vec<&str> = found.iter().map(|(n, _)| n.as_str()).collect();
    assert_eq!(names, expected, "wrong names in {src:?}");
    for (name, spanned) in &found {
        assert_eq!(
            name, spanned,
            "span mismatch in {src:?}: `{name}` spans `{spanned}`"
        );
    }
}

#[test]
fn a_simple_hole_spans_itself() {
    assert_spans_itself(&wrap(r#"log("total {count} items")"#), &["count"]);
}

#[test]
fn multibyte_text_before_the_hole_does_not_shift_the_span() {
    // The failure this guards against is off-by-N in BYTES vs CHARS. `é` is
    // two bytes and `→` is three; an implementation counting characters puts
    // the span in the middle of a codepoint or several bytes early.
    assert_spans_itself(&wrap(r#"log("café → {count} items")"#), &["count"]);
    assert_spans_itself(&wrap(r#"log("🙂🙂🙂 {count}")"#), &["count"]);
}

#[test]
fn multibyte_text_inside_the_hole_is_spanned_exactly() {
    // `pw` identifiers are ASCII (`lexer.rs` uses `is_ascii_alphabetic`), so
    // `café` is not one — it lexes as `caf` and then a stray byte. That is a
    // lexer decision and not this feature's business; what IS this feature's
    // business is that the span still points at the right bytes afterwards,
    // which is exactly where multibyte arithmetic goes wrong.
    let src = wrap(r#"log("{café}")"#);
    let found = holes(&src);
    assert_eq!(found.len(), 1, "expected one name, got {found:?}");
    let (name, spanned) = &found[0];
    assert_eq!(name, "caf", "the lexer's ASCII rule, not a span bug");
    assert_eq!(name, spanned, "`{name}` spans `{spanned}`");

    // And with multibyte AFTER a valid identifier in the same hole, which is
    // where a byte/char confusion would show up as a span running long.
    let src = wrap(r#"log("{count} — café")"#);
    assert_spans_itself(&src, &["count"]);
}

#[test]
fn a_hole_at_the_very_start_and_very_end_of_a_string() {
    assert_spans_itself(&wrap(r#"log("{a} middle {b}")"#), &["a", "b"]);
    assert_spans_itself(&wrap(r#"log("{only}")"#), &["only"]);
}

#[test]
fn several_holes_each_span_themselves() {
    assert_spans_itself(
        &wrap(r#"log("{a} then {bb} then {ccc}")"#),
        &["a", "bb", "ccc"],
    );
}

#[test]
fn a_field_access_in_a_hole_spans_its_own_parts() {
    // The case the whole feature exists for: `{token.value}` must be visible
    // as an expression, and its base must span the base.
    let src = wrap(r#"log("k {token.value} k")"#);
    let found = holes(&src);
    assert!(
        found.iter().any(|(n, s)| n == "token" && s == "token"),
        "expected `token` spanning itself, got {found:?}"
    );
}

#[test]
fn a_comment_before_the_string_does_not_shift_the_span() {
    let src = "module m\n\n// a comment with a {brace} in it\nfn f() -> () !{} {\n    \
               log(\"x {count} y\")\n}\n";
    assert_spans_itself(src, &["count"]);
}

/// Shapes the lowering must survive without producing a wrong span.
///
/// It is allowed to find nothing in these — an empty hole is not an
/// expression, and a malformed one has no sensible tree. What it may not do is
/// produce a span pointing at the wrong bytes, which
/// [`assert_spans_itself`]'s equality would catch, or panic.
#[test]
fn awkward_holes_produce_no_span_rather_than_a_wrong_one() {
    for body in [
        r#"log("{}")"#,              // empty
        r#"log("{ }")"#,             // whitespace only
        r#"log("no holes at all")"#, // none
        r#"log("unclosed {count")"#, // no closing brace
        r#"log("}{")"#,              // reversed
        r#"log("{a}{b}")"#,          // adjacent
        r#"log("{{a}}")"#,           // doubled braces
        r#"log("{1 +}")"#,           // malformed expression
        r#"log("{f(x)}")"#,          // a call
        r#"log("{a\n b}")"#,         // an escape inside
    ] {
        let src = wrap(body);
        for (name, spanned) in holes(&src) {
            assert_eq!(
                name, spanned,
                "span mismatch in {body:?}: `{name}` spans `{spanned}`"
            );
        }
    }
}

/// A hole earlier than the synthetic prefix is long.
///
/// The padding technique needs `hole_offset >= prefix.len()`. Any real file
/// clears it — a module declaration alone is longer — but the guard must exist
/// rather than be argued, because the consequence of it being wrong is a
/// negative padding length and a span pointing anywhere.
#[test]
fn a_hole_too_early_for_the_padding_is_skipped_not_mis_spanned() {
    // The shortest file that can contain a string at all.
    let src = "module m\nfn f()->(){log(\"{a}\")}\n";
    for (name, spanned) in holes(src) {
        assert_eq!(name, spanned, "span mismatch in a minimal file");
    }
}
