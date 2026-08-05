//! Hand-written lexer producing tokens with byte spans.
//!
//! Spike question (charter §14 M0 task 11): are hand-written lexing + explicit
//! byte spans ergonomic enough to carry precise source locations all the way to
//! a rendered diagnostic, before Milestone 2 commits to a parser library?
//!
//! Every token keeps its exact `start..end` byte range. Nothing is discarded
//! that a span needs — that is the property Milestone 2's lossless tree must
//! preserve at a larger scale.

/// A byte range into the original source. Half-open, `start..end`.
pub type Span = std::ops::Range<usize>;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TokenKind {
    /// A bare word: keyword or identifier. Keyword-ness is decided by the
    /// parser, not the lexer, so that `cache` can be a field name elsewhere.
    Word(String),
    Int(i64),
    LParen,
    RParen,
    Colon,
    Comma,
    Dot,
    LBrace,
    RBrace,
    /// Lexing failed here. Carried as a token so the parser can recover and
    /// keep reporting later errors instead of aborting on the first problem.
    Unknown(char),
    Eof,
}

#[derive(Debug, Clone)]
pub struct Token {
    pub kind: TokenKind,
    pub span: Span,
}

impl Token {
    /// Human-readable name used in "expected X, found Y" diagnostics.
    pub fn describe(&self) -> String {
        match &self.kind {
            TokenKind::Word(w) => format!("`{w}`"),
            TokenKind::Int(n) => format!("`{n}`"),
            TokenKind::LParen => "`(`".into(),
            TokenKind::RParen => "`)`".into(),
            TokenKind::Colon => "`:`".into(),
            TokenKind::Comma => "`,`".into(),
            TokenKind::Dot => "`.`".into(),
            TokenKind::LBrace => "`{`".into(),
            TokenKind::RBrace => "`}`".into(),
            TokenKind::Unknown(c) => format!("unexpected character `{c}`"),
            TokenKind::Eof => "end of file".into(),
        }
    }
}

pub fn lex(source: &str) -> Vec<Token> {
    let bytes = source.as_bytes();
    let mut tokens = Vec::new();
    let mut i = 0usize;

    while i < bytes.len() {
        let start = i;
        let c = bytes[i] as char;

        // Whitespace.
        if c.is_ascii_whitespace() {
            i += 1;
            continue;
        }

        // Line comment.
        if c == '/' && i + 1 < bytes.len() && bytes[i + 1] == b'/' {
            while i < bytes.len() && bytes[i] != b'\n' {
                i += 1;
            }
            continue;
        }

        let kind = match c {
            '(' => {
                i += 1;
                TokenKind::LParen
            }
            ')' => {
                i += 1;
                TokenKind::RParen
            }
            ':' => {
                i += 1;
                TokenKind::Colon
            }
            ',' => {
                i += 1;
                TokenKind::Comma
            }
            '.' => {
                i += 1;
                TokenKind::Dot
            }
            '{' => {
                i += 1;
                TokenKind::LBrace
            }
            '}' => {
                i += 1;
                TokenKind::RBrace
            }
            c if c.is_ascii_digit() => {
                while i < bytes.len() && (bytes[i] as char).is_ascii_digit() {
                    i += 1;
                }
                // Cannot fail: the slice is ASCII digits. A real compiler would
                // still report overflow as a diagnostic rather than panicking.
                match source[start..i].parse::<i64>() {
                    Ok(n) => TokenKind::Int(n),
                    Err(_) => TokenKind::Unknown(c),
                }
            }
            c if c.is_ascii_alphabetic() || c == '_' => {
                while i < bytes.len()
                    && ((bytes[i] as char).is_ascii_alphanumeric() || bytes[i] == b'_')
                {
                    i += 1;
                }
                TokenKind::Word(source[start..i].to_string())
            }
            other => {
                i += 1;
                TokenKind::Unknown(other)
            }
        };

        tokens.push(Token {
            kind,
            span: start..i,
        });
    }

    tokens.push(Token {
        kind: TokenKind::Eof,
        span: source.len()..source.len(),
    });
    tokens
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn spans_are_exact_byte_ranges() {
        let src = "public query store";
        let toks = lex(src);
        assert_eq!(toks[0].span, 0..6);
        assert_eq!(&src[toks[0].span.clone()], "public");
        assert_eq!(toks[2].span, 13..18);
        assert_eq!(&src[toks[2].span.clone()], "store");
    }

    #[test]
    fn unknown_characters_become_tokens_not_panics() {
        let toks = lex("query § store");
        assert!(
            toks.iter()
                .any(|t| matches!(t.kind, TokenKind::Unknown('§')))
        );
        // Lexing continues past the bad character so the parser can recover.
        assert!(
            toks.iter()
                .any(|t| matches!(&t.kind, TokenKind::Word(w) if w == "store"))
        );
    }

    #[test]
    fn comments_are_skipped_without_shifting_later_spans() {
        let src = "// note\nquery";
        let toks = lex(src);
        assert_eq!(&src[toks[0].span.clone()], "query");
    }
}
