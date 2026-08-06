//! `pw-syntax` — E2's lossless front end.
//!
//! Three properties, in the order they matter:
//!
//! 1. **Lossless.** Every byte is covered by a token, so `pw fmt` can round-trip
//!    comments and spacing. [`lexer::reconstruct`] asserts it.
//! 2. **Precise spans.** Byte ranges reach diagnostics unchanged (ADR-0009).
//! 3. **Error recovery.** One typo must not hide every later error — charter
//!    §1.16's AI-reliability goal depends on seeing the whole batch per compile.

pub mod ast;
pub mod lexer;
pub mod parser;

pub use ast::{Decl, DeclKind, SourceFile, Visibility};
pub use lexer::{Kind, Span, Token, lex};
pub use parser::{ParseError, Parsed, parse};
