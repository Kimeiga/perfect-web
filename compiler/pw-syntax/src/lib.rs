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
