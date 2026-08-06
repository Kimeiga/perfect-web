//! `pw-syntax` — E2's lossless front end.
//!
//! Three properties, in the order they matter:
//!
//! 1. **Lossless.** Every byte is covered by a token, so `pw fmt` can round-trip
//!    comments and spacing. [`lexer::reconstruct`] asserts it.
//! 2. **Precise spans.** Byte ranges reach diagnostics unchanged (ADR-0009).
//! 3. **Error recovery.** One typo must not hide every later error — charter
//!    §1.16's AI-reliability goal depends on seeing the whole batch per compile.
//!
//! Since ADR-0012 the lossless representation is a **Rowan green tree**
//! ([`tree`], [`kind`]). The invariants the hand-rolled representation
//! established are retained as tests against it. Semantic analysis consumes HIR
//! ids and spans, never these nodes.

pub mod ast;
pub mod fmt;
pub mod grammar;
pub mod kind;
pub mod lexer;
pub mod parser;
pub mod tree;

pub use ast::{Decl, DeclKind, SourceFile, Visibility};
pub use fmt::{format_source, format_tree};
pub use grammar::{Parse, SyntaxError, parse_tree};
pub use kind::{Pw, SyntaxKind, SyntaxNode, SyntaxToken};
pub use lexer::{Kind, Span, Token, lex};
pub use parser::{ParseError, Parsed, parse};
pub use tree::{TreeBuilder, flat_tree, tree_text};

/// A policy value with its layout removed, so wrapping a long clause across
/// lines does not change what it declares.
///
/// ```text
/// invalidates_on MenuChanged(id),
///     InventoryChanged(id, item)
/// ```
///
/// declares the same two events as the one-line spelling, and every consumer
/// wants the same string for both. One implementation because there are two
/// parsers: the declaration parser normalised and the tree lowering did not,
/// so `pw explain` and the HIR disagreed about a value neither had got wrong.
///
/// Whitespace **inside a string literal is preserved**. `because "a  b"` is a
/// justification a human wrote and reflowing it would be editing it.
pub fn collapse_policy_whitespace(value: &str) -> String {
    let mut out = String::with_capacity(value.len());
    let mut in_string = false;
    let mut pending_space = false;
    for c in value.chars() {
        if c == '"' {
            in_string = !in_string;
        }
        if !in_string && c.is_whitespace() {
            pending_space = !out.is_empty();
            continue;
        }
        if pending_space {
            out.push(' ');
            pending_space = false;
        }
        out.push(c);
    }
    out
}
