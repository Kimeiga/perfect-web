//! Toy parser for ONE declaration form: a `query` header.
//!
//! Deliberately tiny. The point of this spike is not the grammar — it is
//! whether the pipeline
//!
//!     source -> spans -> semantic check -> multi-span diagnostic
//!
//! is ergonomic in Rust. So the grammar covers exactly enough to produce the
//! one semantic error the charter cares most about (§7.8: a `Session`-labeled
//! resource must not land in a shared cache).
//!
//! Grammar:
//!
//! ```text
//! query_decl := visibility "query" IDENT "(" params? ")" policy* ("{" ... "}")?
//! visibility := "public" | "session" | "private"
//! params     := IDENT ":" IDENT ("," IDENT ":" IDENT)*
//! policy     := "freshness" INT "." IDENT
//!             | "consistency" ("snapshot" | "read_your_writes" | "eventual")
//!             | "cache" ("shared" | "private")
//! ```

use crate::diag::Diagnostic;
use crate::lexer::{Span, Token, TokenKind, lex};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Visibility {
    Public,
    Session,
    Private,
}

impl Visibility {
    /// The privacy label this visibility implies, in the vocabulary of charter §7.8.
    pub fn privacy_label(self) -> &'static str {
        match self {
            Visibility::Public => "Public",
            Visibility::Session => "Session<SessionId>",
            Visibility::Private => "User<UserId>",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CachePartition {
    Shared,
    Private,
}

#[derive(Debug, Clone)]
pub struct Param {
    pub name: String,
    pub ty: String,
    pub span: Span,
}

#[derive(Debug, Clone)]
pub struct QueryDecl {
    pub visibility: Visibility,
    /// Span of the visibility keyword — this is where a privacy label *originates*,
    /// and charter §16.3 requires the diagnostic to name that origin.
    pub visibility_span: Span,
    pub name: String,
    pub name_span: Span,
    pub params: Vec<Param>,
    pub freshness: Option<(i64, String, Span)>,
    pub consistency: Option<(String, Span)>,
    /// Span covers the whole `cache <kind>` policy, not just the keyword.
    pub cache: Option<(CachePartition, Span)>,
}

pub struct ParseOutcome {
    pub decl: Option<QueryDecl>,
    pub diagnostics: Vec<Diagnostic>,
}

struct Parser {
    tokens: Vec<Token>,
    pos: usize,
    diagnostics: Vec<Diagnostic>,
}

impl Parser {
    fn peek(&self) -> &Token {
        &self.tokens[self.pos.min(self.tokens.len() - 1)]
    }

    fn at_eof(&self) -> bool {
        matches!(self.peek().kind, TokenKind::Eof)
    }

    fn bump(&mut self) -> Token {
        let t = self.peek().clone();
        if !self.at_eof() {
            self.pos += 1;
        }
        t
    }

    fn eat_word(&mut self, want: &str) -> bool {
        if matches!(&self.peek().kind, TokenKind::Word(w) if w == want) {
            self.bump();
            true
        } else {
            false
        }
    }

    fn expect(&mut self, want: TokenKind, ctx: &str) -> Option<Token> {
        if self.peek().kind == want {
            Some(self.bump())
        } else {
            let found = self.peek().describe();
            let span = self.peek().span.clone();
            let want_desc = Token {
                kind: want,
                span: 0..0,
            }
            .describe();
            self.diagnostics.push(
                Diagnostic::error(
                    "PW0001",
                    format!("expected {want_desc} {ctx}, found {found}"),
                )
                .primary(span, format!("expected {want_desc} here")),
            );
            None
        }
    }

    fn word(&mut self, ctx: &str) -> Option<(String, Span)> {
        match self.peek().kind.clone() {
            TokenKind::Word(w) => {
                let span = self.bump().span;
                Some((w, span))
            }
            _ => {
                let found = self.peek().describe();
                let span = self.peek().span.clone();
                self.diagnostics.push(
                    Diagnostic::error("PW0001", format!("expected {ctx}, found {found}"))
                        .primary(span, format!("expected {ctx} here")),
                );
                None
            }
        }
    }

    /// Error recovery: skip forward to something that plausibly starts the next
    /// policy clause or ends the declaration. Without this, one typo hides every
    /// later error — the charter's AI-friendliness goal (§1.16, deterministic
    /// compiler feedback) depends on reporting the whole batch.
    fn recover_to_policy_boundary(&mut self) {
        const ANCHORS: [&str; 4] = ["freshness", "consistency", "cache", "query"];
        while !self.at_eof() {
            if let TokenKind::Word(w) = &self.peek().kind
                && ANCHORS.contains(&w.as_str())
            {
                return;
            }
            if matches!(self.peek().kind, TokenKind::LBrace) {
                return;
            }
            self.bump();
        }
    }
}

pub fn parse(source: &str) -> ParseOutcome {
    let mut p = Parser {
        tokens: lex(source),
        pos: 0,
        diagnostics: Vec::new(),
    };

    // --- visibility ---------------------------------------------------------
    let (vis_word, visibility_span) =
        match p.word("a visibility (`public`, `session`, or `private`)") {
            Some(v) => v,
            None => {
                return ParseOutcome {
                    decl: None,
                    diagnostics: p.diagnostics,
                };
            }
        };
    let visibility = match vis_word.as_str() {
        "public" => Visibility::Public,
        "session" => Visibility::Session,
        "private" => Visibility::Private,
        other => {
            p.diagnostics.push(
                Diagnostic::error("PW0002", format!("unknown visibility `{other}`"))
                    .primary(visibility_span.clone(), "not a visibility")
                    .help("expected one of: `public`, `session`, `private`"),
            );
            return ParseOutcome {
                decl: None,
                diagnostics: p.diagnostics,
            };
        }
    };

    // --- `query` keyword ----------------------------------------------------
    if !p.eat_word("query") {
        let found = p.peek().describe();
        let span = p.peek().span.clone();
        p.diagnostics.push(
            Diagnostic::error("PW0001", format!("expected `query`, found {found}"))
                .primary(span, "expected `query` here")
                .note("this spike parses only `query` declarations; `command` and `subscription` arrive in Milestone 2"),
        );
        return ParseOutcome {
            decl: None,
            diagnostics: p.diagnostics,
        };
    }

    // --- name ---------------------------------------------------------------
    let Some((name, name_span)) = p.word("a query name") else {
        return ParseOutcome {
            decl: None,
            diagnostics: p.diagnostics,
        };
    };

    // --- parameters ---------------------------------------------------------
    let mut params = Vec::new();
    if p.expect(TokenKind::LParen, "after the query name")
        .is_some()
    {
        while !matches!(p.peek().kind, TokenKind::RParen) && !p.at_eof() {
            let Some((pname, pspan)) = p.word("a parameter name") else {
                p.recover_to_policy_boundary();
                break;
            };
            if p.expect(TokenKind::Colon, "after the parameter name")
                .is_none()
            {
                p.recover_to_policy_boundary();
                break;
            }
            let Some((ty, tyspan)) = p.word("a parameter type") else {
                p.recover_to_policy_boundary();
                break;
            };
            params.push(Param {
                name: pname,
                ty,
                span: pspan.start..tyspan.end,
            });
            if matches!(p.peek().kind, TokenKind::Comma) {
                p.bump();
            } else {
                break;
            }
        }
        p.expect(TokenKind::RParen, "to close the parameter list");
    }

    // --- policies -----------------------------------------------------------
    let mut freshness = None;
    let mut consistency = None;
    let mut cache = None;

    while let TokenKind::Word(w) = p.peek().kind.clone() {
        let kw_span = p.peek().span.clone();
        match w.as_str() {
            "freshness" => {
                p.bump();
                let TokenKind::Int(n) = p.peek().kind.clone() else {
                    let found = p.peek().describe();
                    let span = p.peek().span.clone();
                    p.diagnostics.push(
                        Diagnostic::error("PW0003", format!("expected a duration, found {found}"))
                            .primary(span, "expected a number here")
                            .help(
                                "write a duration as `<number>.<unit>`, for example `30.seconds`",
                            ),
                    );
                    p.recover_to_policy_boundary();
                    continue;
                };
                p.bump();
                if p.expect(TokenKind::Dot, "between the duration and its unit")
                    .is_none()
                {
                    p.recover_to_policy_boundary();
                    continue;
                }
                let Some((unit, unit_span)) = p.word("a duration unit") else {
                    p.recover_to_policy_boundary();
                    continue;
                };
                freshness = Some((n, unit, kw_span.start..unit_span.end));
            }
            "consistency" => {
                p.bump();
                let Some((kind, kspan)) = p.word("a consistency policy") else {
                    p.recover_to_policy_boundary();
                    continue;
                };
                const KNOWN: [&str; 3] = ["snapshot", "read_your_writes", "eventual"];
                if !KNOWN.contains(&kind.as_str()) {
                    p.diagnostics.push(
                        Diagnostic::error("PW0004", format!("unknown consistency policy `{kind}`"))
                            .primary(kspan.clone(), "not a known consistency policy")
                            .help("expected one of: `snapshot`, `read_your_writes`, `eventual`"),
                    );
                }
                consistency = Some((kind, kw_span.start..kspan.end));
            }
            "cache" => {
                p.bump();
                let Some((kind, kspan)) = p.word("a cache partition") else {
                    p.recover_to_policy_boundary();
                    continue;
                };
                let partition = match kind.as_str() {
                    "shared" => Some(CachePartition::Shared),
                    "private" => Some(CachePartition::Private),
                    other => {
                        p.diagnostics.push(
                            Diagnostic::error(
                                "PW0005",
                                format!("unknown cache partition `{other}`"),
                            )
                            .primary(kspan.clone(), "not a known cache partition")
                            .help("expected one of: `shared`, `private`"),
                        );
                        None
                    }
                };
                if let Some(part) = partition {
                    cache = Some((part, kw_span.start..kspan.end));
                }
            }
            _ => break,
        }
    }

    // Optional body — contents are not interpreted by this spike.
    if matches!(p.peek().kind, TokenKind::LBrace) {
        let mut depth = 0usize;
        loop {
            match p.peek().kind {
                TokenKind::LBrace => depth += 1,
                TokenKind::RBrace => {
                    depth -= 1;
                    if depth == 0 {
                        p.bump();
                        break;
                    }
                }
                TokenKind::Eof => break,
                _ => {}
            }
            p.bump();
        }
    }

    // Trailing junk.
    if !p.at_eof() {
        let found = p.peek().describe();
        let span = p.peek().span.clone();
        p.diagnostics.push(
            Diagnostic::error(
                "PW0006",
                format!("unexpected {found} after the declaration"),
            )
            .primary(span, "not part of a query declaration"),
        );
    }

    ParseOutcome {
        decl: Some(QueryDecl {
            visibility,
            visibility_span,
            name,
            name_span,
            params,
            freshness,
            consistency,
            cache,
        }),
        diagnostics: p.diagnostics,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_a_well_formed_public_query() {
        let src = "public query store(id: StoreId) freshness 30.seconds consistency snapshot cache shared";
        let out = parse(src);
        assert!(out.diagnostics.is_empty(), "{:?}", out.diagnostics);
        let d = out.decl.unwrap();
        assert_eq!(d.visibility, Visibility::Public);
        assert_eq!(d.name, "store");
        assert_eq!(d.params.len(), 1);
        assert_eq!(d.params[0].ty, "StoreId");
        assert_eq!(d.freshness.as_ref().unwrap().0, 30);
        assert_eq!(d.cache.as_ref().unwrap().0, CachePartition::Shared);
    }

    #[test]
    fn visibility_span_points_at_the_keyword() {
        let src = "session query cart() cache private";
        let out = parse(src);
        let d = out.decl.unwrap();
        assert_eq!(&src[d.visibility_span.clone()], "session");
    }

    #[test]
    fn recovers_and_reports_more_than_one_error() {
        // Two independent mistakes: bad consistency policy, bad cache partition.
        let src = "public query store(id: StoreId) consistency eventualy cache sharedd";
        let out = parse(src);
        assert!(
            out.diagnostics.len() >= 2,
            "expected >=2 diagnostics, got {:?}",
            out.diagnostics
        );
    }

    #[test]
    fn malformed_input_never_panics() {
        for src in [
            "",
            "(((",
            "public",
            "public query",
            "public query store(",
            "public query store(id:",
            "session query cart() freshness",
            "§§§",
            "public query store() cache",
            "}{",
        ] {
            let _ = parse(src); // must not panic
        }
    }
}
