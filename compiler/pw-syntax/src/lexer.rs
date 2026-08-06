//! Lossless lexer.
//!
//! **Every byte of the input is covered by exactly one token**, including
//! whitespace and comments. That is what "lossless" means here and it is the
//! property `pw fmt` idempotence depends on: a formatter that cannot see the
//! comments cannot preserve them.
//!
//! The E0 diagnostic spike (ADR-0009) proved a hand-written lexer with
//! `Range<usize>` spans carries positions all the way into a rendered
//! diagnostic. It did **not** answer the lossless question, because it discarded
//! trivia. This does not.
//!
//! Invariant, asserted by `lex_covers_every_byte` and by a property test over
//! the whole corpus: concatenating every token's source text reproduces the
//! input exactly.

pub type Span = std::ops::Range<usize>;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Kind {
    // --- trivia: preserved, never discarded -------------------------------
    Whitespace,
    LineComment,
    /// A `// @key: value` header line. Lexed distinctly because the corpus
    /// format depends on it and `pw fmt` must not reflow it.
    DocAttr,

    // --- literals and names -----------------------------------------------
    Ident,
    Int,
    Float,
    Str,
    /// A string with no closing quote. Carried as a token so the parser can
    /// recover instead of aborting.
    UnterminatedStr,

    // --- punctuation --------------------------------------------------------
    LParen,
    RParen,
    LBrace,
    RBrace,
    LBracket,
    RBracket,
    LAngle,
    RAngle,
    Comma,
    Colon,
    Semi,
    Dot,
    Arrow,
    FatArrow,
    /// `==` `!=` `<=` `>=`. Lexed compound so `a >= b` is not `a > (= b)`.
    /// `<` and `>` stay separate angle tokens because they also delimit type
    /// arguments; only the two-byte comparison forms are joined.
    Cmp,
    Pipe,
    /// `|>` — the pipeline operator. One token, because lexing it as
    /// `Pipe` then `RAngle` makes the parser see a bitwise-or followed by a
    /// stray `>`, which is how the corpus's `a |> f |> g` chains first failed.
    PipeGt,
    Bang,
    Eq,
    Question,
    At,
    Amp,
    Plus,
    Minus,
    Star,
    Slash,
    Percent,
    /// Opens a template block: `{#each ...}`, `{#if ...}`.
    Hash,
    /// Closes a template block: `{/each}`.
    CloseSlash,
    Underscore,

    /// A byte that is not part of any legal token.
    Unknown,
    Eof,
}

impl Kind {
    /// Trivia carries no meaning to the parser but must survive to the
    /// formatter.
    pub fn is_trivia(self) -> bool {
        matches!(self, Kind::Whitespace | Kind::LineComment | Kind::DocAttr)
    }

    pub fn describe(self) -> &'static str {
        match self {
            Kind::Whitespace => "whitespace",
            Kind::LineComment => "comment",
            Kind::DocAttr => "attribute comment",
            Kind::Ident => "an identifier",
            Kind::Int => "an integer",
            Kind::Float => "a number",
            Kind::Str => "a string",
            Kind::UnterminatedStr => "an unterminated string",
            Kind::LParen => "`(`",
            Kind::RParen => "`)`",
            Kind::LBrace => "`{`",
            Kind::RBrace => "`}`",
            Kind::LBracket => "`[`",
            Kind::RBracket => "`]`",
            Kind::LAngle => "`<`",
            Kind::RAngle => "`>`",
            Kind::Comma => "`,`",
            Kind::Colon => "`:`",
            Kind::Semi => "`;`",
            Kind::Dot => "`.`",
            Kind::Arrow => "`->`",
            Kind::Cmp => "a comparison operator",
            Kind::FatArrow => "`=>`",
            Kind::Pipe => "`|`",
            Kind::PipeGt => "`|>`",
            Kind::Bang => "`!`",
            Kind::Eq => "`=`",
            Kind::Question => "`?`",
            Kind::At => "`@`",
            Kind::Amp => "`&`",
            Kind::Plus => "`+`",
            Kind::Minus => "`-`",
            Kind::Star => "`*`",
            Kind::Slash => "`/`",
            Kind::Percent => "`%`",
            Kind::Hash => "`#`",
            Kind::CloseSlash => "`{/`",
            Kind::Underscore => "`_`",
            Kind::Unknown => "an unexpected character",
            Kind::Eof => "end of file",
        }
    }
}

#[derive(Debug, Clone)]
pub struct Token {
    pub kind: Kind,
    pub span: Span,
}

impl Token {
    pub fn text<'a>(&self, source: &'a str) -> &'a str {
        &source[self.span.clone()]
    }
}

/// Lex `source` into a complete, gap-free token stream terminated by `Eof`.
pub fn lex(source: &str) -> Vec<Token> {
    let b = source.as_bytes();
    let mut out = Vec::new();
    let mut i = 0usize;

    macro_rules! push {
        ($kind:expr, $len:expr) => {{
            out.push(Token {
                kind: $kind,
                span: i..i + $len,
            });
            i += $len;
        }};
    }

    while i < b.len() {
        let c = b[i];

        // --- whitespace ---------------------------------------------------
        if c.is_ascii_whitespace() {
            let start = i;
            while i < b.len() && b[i].is_ascii_whitespace() {
                i += 1;
            }
            out.push(Token {
                kind: Kind::Whitespace,
                span: start..i,
            });
            continue;
        }

        // --- comments -----------------------------------------------------
        if c == b'/' && i + 1 < b.len() && b[i + 1] == b'/' {
            let start = i;
            while i < b.len() && b[i] != b'\n' {
                i += 1;
            }
            // `// @key: value` is the corpus header form; keep it distinct so
            // the formatter never reflows or reorders it.
            let text = &source[start..i];
            let kind = if text.starts_with("// @") || text.starts_with("//@") {
                Kind::DocAttr
            } else {
                Kind::LineComment
            };
            out.push(Token {
                kind,
                span: start..i,
            });
            continue;
        }

        // --- strings ------------------------------------------------------
        // `"""..."""` spans lines. A single-quoted string still ends at the
        // newline: that recovery property is why the multi-line form is
        // explicit rather than a relaxation of the ordinary one.
        if c == b'"' && b[i..].starts_with(b"\"\"\"") {
            let start = i;
            i += 3;
            let mut closed = false;
            while i < b.len() {
                if b[i..].starts_with(b"\"\"\"") {
                    i += 3;
                    closed = true;
                    break;
                }
                i += 1;
            }
            out.push(Token {
                kind: if closed {
                    Kind::Str
                } else {
                    Kind::UnterminatedStr
                },
                span: start..i,
            });
            continue;
        }
        if c == b'"' {
            let start = i;
            i += 1;
            let mut closed = false;
            while i < b.len() {
                if b[i] == b'\\' && i + 1 < b.len() {
                    i += 2;
                    continue;
                }
                if b[i] == b'"' {
                    i += 1;
                    closed = true;
                    break;
                }
                if b[i] == b'\n' {
                    break; // a newline ends an unterminated string
                }
                i += 1;
            }
            out.push(Token {
                kind: if closed {
                    Kind::Str
                } else {
                    Kind::UnterminatedStr
                },
                span: start..i,
            });
            continue;
        }

        // --- numbers ------------------------------------------------------
        if c.is_ascii_digit() {
            let start = i;
            while i < b.len() && b[i].is_ascii_digit() {
                i += 1;
            }
            // `30.seconds` is a duration, not a float: only digits after the
            // dot make it a float. This distinction matters because the corpus
            // is full of `30.seconds` and `0.1`.
            let mut kind = Kind::Int;
            if i + 1 < b.len() && b[i] == b'.' && b[i + 1].is_ascii_digit() {
                i += 1;
                while i < b.len() && b[i].is_ascii_digit() {
                    i += 1;
                }
                kind = Kind::Float;
            }
            out.push(Token {
                kind,
                span: start..i,
            });
            continue;
        }

        // --- identifiers --------------------------------------------------
        if c.is_ascii_alphabetic() || c == b'_' {
            let start = i;
            while i < b.len() && (b[i].is_ascii_alphanumeric() || b[i] == b'_') {
                i += 1;
            }
            let kind = if &source[start..i] == "_" {
                Kind::Underscore
            } else {
                Kind::Ident
            };
            out.push(Token {
                kind,
                span: start..i,
            });
            continue;
        }

        // --- multi-byte punctuation ---------------------------------------
        if c == b'-' && i + 1 < b.len() && b[i + 1] == b'>' {
            push!(Kind::Arrow, 2);
            continue;
        }
        if c == b'=' && i + 1 < b.len() && b[i + 1] == b'>' {
            push!(Kind::FatArrow, 2);
            continue;
        }
        if c == b'|' && i + 1 < b.len() && b[i + 1] == b'>' {
            push!(Kind::PipeGt, 2);
            continue;
        }
        if matches!(c, b'=' | b'!' | b'<' | b'>') && i + 1 < b.len() && b[i + 1] == b'=' {
            push!(Kind::Cmp, 2);
            continue;
        }

        // --- single-byte punctuation ---------------------------------------
        let kind = match c {
            b'(' => Kind::LParen,
            b')' => Kind::RParen,
            b'{' => Kind::LBrace,
            b'}' => Kind::RBrace,
            b'[' => Kind::LBracket,
            b']' => Kind::RBracket,
            b'<' => Kind::LAngle,
            b'>' => Kind::RAngle,
            b',' => Kind::Comma,
            b':' => Kind::Colon,
            b';' => Kind::Semi,
            b'.' => Kind::Dot,
            b'|' => Kind::Pipe,
            b'!' => Kind::Bang,
            b'=' => Kind::Eq,
            b'?' => Kind::Question,
            b'@' => Kind::At,
            b'&' => Kind::Amp,
            b'+' => Kind::Plus,
            b'-' => Kind::Minus,
            b'*' => Kind::Star,
            b'/' => Kind::Slash,
            b'%' => Kind::Percent,
            b'#' => Kind::Hash,
            _ => Kind::Unknown,
        };
        // A multi-byte UTF-8 character must be consumed whole, or the spans
        // stop landing on character boundaries and slicing panics.
        let len = if kind == Kind::Unknown {
            source[i..]
                .chars()
                .next()
                .map(|ch| ch.len_utf8())
                .unwrap_or(1)
        } else {
            1
        };
        push!(kind, len);
    }

    out.push(Token {
        kind: Kind::Eof,
        span: source.len()..source.len(),
    });
    out
}

/// The losslessness invariant, as a function so callers can assert it too.
pub fn reconstruct(source: &str, tokens: &[Token]) -> String {
    tokens
        .iter()
        .filter(|t| t.kind != Kind::Eof)
        .map(|t| t.text(source))
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn round_trips(src: &str) -> bool {
        reconstruct(src, &lex(src)) == src
    }

    #[test]
    fn lex_covers_every_byte() {
        let src = "module store.pricing\n\n// a comment\nfn f(x: Int) -> Int !{} {\n    x + 1\n}\n";
        let toks = lex(src);
        assert_eq!(reconstruct(src, &toks), src);
        // Adjacent spans must touch with no gaps.
        let mut cursor = 0;
        for t in toks.iter().filter(|t| t.kind != Kind::Eof) {
            assert_eq!(t.span.start, cursor, "gap or overlap before {:?}", t.kind);
            cursor = t.span.end;
        }
        assert_eq!(cursor, src.len());
    }

    #[test]
    fn round_trips_on_awkward_input() {
        for src in [
            "",
            "   ",
            "\n\n\n",
            "// just a comment",
            "// @corpus: accepted\n// @id: A-001\n",
            "\"unterminated",
            "\"escaped \\\" quote\"",
            "30.seconds",
            "0.1",
            "a->b=>c",
            "§ unknown bytes ¤",
            "match x { A => 1, _ => 2 }",
        ] {
            assert!(round_trips(src), "did not round-trip: {src:?}");
        }
    }

    #[test]
    fn doc_attrs_are_lexed_apart_from_ordinary_comments() {
        let src = "// @corpus: accepted\n// ordinary\n";
        let toks = lex(src);
        let kinds: Vec<Kind> = toks
            .iter()
            .map(|t| t.kind)
            .filter(|k| !matches!(k, Kind::Whitespace | Kind::Eof))
            .collect();
        assert_eq!(kinds, vec![Kind::DocAttr, Kind::LineComment]);
    }

    #[test]
    fn durations_are_not_floats() {
        // `30.seconds` must lex as Int Dot Ident, not as a float, because the
        // corpus writes freshness policies that way.
        let toks: Vec<Kind> = lex("30.seconds")
            .iter()
            .map(|t| t.kind)
            .filter(|k| *k != Kind::Eof)
            .collect();
        assert_eq!(toks, vec![Kind::Int, Kind::Dot, Kind::Ident]);
        // A real float still lexes as one.
        let toks: Vec<Kind> = lex("0.5")
            .iter()
            .map(|t| t.kind)
            .filter(|k| *k != Kind::Eof)
            .collect();
        assert_eq!(toks, vec![Kind::Float]);
    }

    #[test]
    fn multibyte_characters_do_not_split_spans() {
        // §, — and emoji appear in the corpus comments. Slicing a span that
        // lands mid-character panics, so this is a real hazard.
        let src = "fn f() { \"§ — 🚀\" }";
        assert!(round_trips(src));
        for t in lex(src) {
            let _ = &src[t.span.clone()]; // must not panic
        }
    }

    #[test]
    fn unterminated_string_is_a_token_not_a_panic() {
        let toks = lex("let s = \"oops\nlet t = 1");
        assert!(toks.iter().any(|t| t.kind == Kind::UnterminatedStr));
        // Lexing continues past it.
        assert!(toks.iter().filter(|t| t.kind == Kind::Ident).count() >= 2);
    }

    #[test]
    fn the_pipeline_operator_is_one_token() {
        let toks: Vec<Kind> = lex("a |> f")
            .iter()
            .map(|t| t.kind)
            .filter(|k| !matches!(k, Kind::Whitespace | Kind::Eof))
            .collect();
        assert_eq!(toks, vec![Kind::Ident, Kind::PipeGt, Kind::Ident]);
        // A bare `|` is still a `|`.
        let toks: Vec<Kind> = lex("A | B")
            .iter()
            .map(|t| t.kind)
            .filter(|k| !matches!(k, Kind::Whitespace | Kind::Eof))
            .collect();
        assert_eq!(toks, vec![Kind::Ident, Kind::Pipe, Kind::Ident]);
    }

    #[test]
    fn underscore_is_distinct_from_an_identifier() {
        let toks: Vec<Kind> = lex("_ _x")
            .iter()
            .map(|t| t.kind)
            .filter(|k| !matches!(k, Kind::Whitespace | Kind::Eof))
            .collect();
        assert_eq!(toks, vec![Kind::Underscore, Kind::Ident]);
    }
}
