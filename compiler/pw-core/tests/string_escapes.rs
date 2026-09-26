//! **A string's escapes, in the checker and every backend that is not a
//! component** (ADR-0049). The component's values are
//! `pw-conformance/tests/strings.rs`'s, run through the host.

use pw_core::hir::{AttrValue, Expr, Literal, Node};
use pw_core::lower::lower_file;
use pw_syntax::parse_tree;

fn lowered(src: &str) -> pw_core::hir::Hir {
    let p = parse_tree(src);
    assert!(p.errors.is_empty(), "{:?}", p.errors);
    lower_file(src, &p.green)
}

/// The expression a one-statement body `f` returns.
fn body_value(hir: &pw_core::hir::Hir) -> (pw_core::hir::ExprId, &pw_core::hir::Body) {
    let (_, f) = hir.all_decls().find(|(_, d)| d.name == "f").expect("f");
    let body = hir.body(f.body.expect("a body"));
    let Expr::Block { stmts } = body.expr(body.root) else {
        panic!("a block")
    };
    (*stmts.last().expect("a statement"), body)
}

#[test]
fn an_escape_the_language_does_not_define_is_refused_at_its_backslash() {
    let src = "module m\n\nfn f() -> String !{} {\n    \"ok \\q no\"\n}\n";
    let p = parse_tree(src);
    let e = p
        .errors
        .iter()
        .find(|e| e.code == "PW0014")
        .unwrap_or_else(|| panic!("{:?}", p.errors));
    assert_eq!(&src[e.span.clone()], "\\", "the span is the backslash");
    assert!(e.message.contains("`\\q`"), "{}", e.message);

    // Control: the escapes the language defines parse.
    let fine = src.replace("\\q", "\\n\\t\\\"\\\\\\{\\}\\u{1F600}");
    assert!(
        parse_tree(&fine).errors.is_empty(),
        "{:?}",
        parse_tree(&fine).errors
    );
}

#[test]
fn a_literal_is_its_decoded_value_and_an_escaped_brace_opens_no_hole() {
    let hir = lowered("module m\n\nfn f() -> String !{} {\n    \"a\\{b\\}\\n\"\n}\n");
    let (v, body) = body_value(&hir);
    let Expr::Literal(l @ Literal::Str(_)) = body.expr(v) else {
        panic!("a literal, not an interpolation: {:?}", body.expr(v))
    };
    assert_eq!(l.string_value().as_deref(), Some("a{b}\n"));
}

#[test]
fn a_hole_beside_an_escape_is_an_expression() {
    let hir = lowered("module m\n\nfn f(x: Int) -> String !{} {\n    \"\\t{x}\\n\"\n}\n");
    let (v, body) = body_value(&hir);
    let Expr::Interpolated { parts, .. } = body.expr(v) else {
        panic!("an interpolation: {:?}", body.expr(v))
    };
    assert_eq!(parts.len(), 1);
    assert!(matches!(body.expr(parts[0]), Expr::Name(n) if n == "x"));
}

#[test]
fn a_triple_quoted_string_is_raw() {
    let hir = lowered("module m\n\nfn f() -> String !{} {\n    \"\"\"a\\n {b}\"\"\"\n}\n");
    let (v, body) = body_value(&hir);
    let Expr::Literal(l) = body.expr(v) else {
        panic!("a literal: {:?}", body.expr(v))
    };
    assert_eq!(l.string_value().as_deref(), Some("a\\n {b}"));
}

#[test]
fn a_markup_attribute_is_html_and_keeps_its_backslashes() {
    // `pattern="\d+"` is an HTML attribute's text, not a Pleris string: no
    // escape is refused, and none is decoded.
    let src = "module m\n\nview V() !{} {\n    <input pattern=\"\\d+\" aria-label=\"n\" />\n}\n";
    let hir = lowered(src);
    let (_, v) = hir.all_decls().find(|(_, d)| d.name == "V").expect("V");
    let body = hir.body(v.body.expect("a body"));
    let pattern = body
        .nodes
        .iter()
        .find_map(|(_, n, _)| match n {
            Node::Element { attrs, .. } => attrs.iter().find(|a| a.name == "pattern").cloned(),
            _ => None,
        })
        .expect("the attribute");
    assert!(
        // The attribute's text as written, quotes included.
        matches!(&pattern.value, AttrValue::Static(s) if s == "\"\\d+\""),
        "{:?}",
        pattern.value
    );
}

#[test]
fn koka_is_given_the_value_in_its_own_escapes() {
    let hir =
        lowered("module m\n\nfn f() -> String !{} {\n    \"a\\nb \\\"q\\\" \\u{1F600}\"\n}\n");
    let out = pw_core::koka::lower_module(&hir, "m");
    assert!(
        out.source.contains(r#""a\u000Ab \"q\" \U01F600""#),
        "{}",
        out.source
    );
}

#[test]
fn marko_is_given_the_value_and_an_interpolation_joined() {
    let src = "module m\n\nview V(name: String) !{} {\n    <p>{\"hi {name}\\n\"}</p>\n}\n";
    let p = parse_tree(src);
    assert!(p.errors.is_empty(), "{:?}", p.errors);
    let out = pw_core::marko::render_module(&lower_file(src, &p.green), "t.pw");
    assert!(out.skipped.is_empty(), "{:?}", out.skipped);
    let (_, text) = &out.files[0];
    assert!(
        text.contains(r#"("hi " + String(input.name) + "\n")"#),
        "{text}"
    );
}
