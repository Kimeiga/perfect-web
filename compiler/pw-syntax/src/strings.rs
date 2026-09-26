//! **What a string token means** (ADR-0049).
//!
//! Assumption A-023 left every string with a backslash, and every `"""`
//! string, without a value. The lexer knew only that a backslash keeps the
//! next character inside the string. The handler backend and the Wasm
//! lowering refused such strings, and the Koka and Marko backends passed the
//! token through to targets whose rules differ.
//!
//! This module is the one reading of a string token. The grammar asks it
//! whether an escape exists; the lowering asks it where the holes are; every
//! backend asks it for the characters.
//!
//! # The rules
//!
//! In a `"..."` string:
//! - `\n`, `\t` and `\r` are a line feed, a tab and a carriage return;
//! - `\\` and `\"` are a backslash and a quote;
//! - `\{` and `\}` are braces, which do not open a hole;
//! - `\u{1F600}` is one Unicode scalar value, in one to six hex digits;
//! - any other backslash is an error (`PW0014`), never a guess;
//! - `{expr}` is a hole, ending at the first `}`, and it must hold an
//!   expression. A `{` with no `}` after it, and a `}` alone, are text.
//!
//! A `"""..."""` string is raw: its value is exactly the characters between
//! the delimiters, with no escapes and no holes. It exists for text written
//! as it reads, such as an escape hatch's `because` justification.

/// One piece of a string token, in order.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Piece {
    /// Characters, escapes decoded.
    Text(String),
    /// `{expr}`: the expression's source, and the byte offset in the token
    /// where that source starts.
    Hole { source: String, at: usize },
}

/// A string the rules above give no value.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StringError {
    /// The byte offset in the token of what is refused.
    pub at: usize,
    pub message: String,
}

/// The escapes, as a diagnostic lists them.
pub const ESCAPES: &str = r#"`\n`, `\t`, `\r`, `\\`, `\"`, `\{`, `\}` and `\u{..}`"#;

/// The pieces of a string token, quotes included, as the program wrote it.
///
/// Adjacent text is one piece. A token that is not a string (no quotes)
/// has one text piece, itself, so a caller that passes something else gets
/// its characters back rather than a crash.
pub fn pieces(token: &str) -> Result<Vec<Piece>, StringError> {
    if let Some(raw) = token
        .strip_prefix("\"\"\"")
        .and_then(|t| t.strip_suffix("\"\"\""))
    {
        return Ok(vec![Piece::Text(raw.to_string())]);
    }
    let Some(inner) = token.strip_prefix('"').and_then(|t| t.strip_suffix('"')) else {
        return Ok(vec![Piece::Text(token.to_string())]);
    };
    let mut out = Vec::new();
    let mut text = String::new();
    let mut chars = inner.char_indices().peekable();
    while let Some((i, c)) = chars.next() {
        // Offsets in the token, which begins with the quote.
        let at = i + 1;
        match c {
            '\\' => {
                let Some((_, e)) = chars.next() else {
                    return Err(StringError {
                        at,
                        message: "a backslash ends the string; write `\\\\` for one".into(),
                    });
                };
                match e {
                    'n' => text.push('\n'),
                    't' => text.push('\t'),
                    'r' => text.push('\r'),
                    '\\' => text.push('\\'),
                    '"' => text.push('"'),
                    '{' => text.push('{'),
                    '}' => text.push('}'),
                    'u' => text.push(unicode(&mut chars, at)?),
                    other => {
                        return Err(StringError {
                            at,
                            message: format!(
                                "`\\{other}` is not an escape; the escapes are {ESCAPES}"
                            ),
                        });
                    }
                }
            }
            '{' => {
                let rest = &inner[i + 1..];
                let Some(close) = rest.find('}') else {
                    text.push('{');
                    continue;
                };
                let source = &rest[..close];
                if source.trim().is_empty() {
                    return Err(StringError {
                        at,
                        message: "an empty `{}` interpolates nothing; write `\\{` and `\\}` \
                                  for braces"
                            .into(),
                    });
                }
                if !text.is_empty() {
                    out.push(Piece::Text(std::mem::take(&mut text)));
                }
                out.push(Piece::Hole {
                    source: source.to_string(),
                    at: at + 1,
                });
                // Past the hole and its `}`.
                while chars.peek().is_some_and(|(j, _)| *j <= i + 1 + close) {
                    chars.next();
                }
            }
            other => text.push(other),
        }
    }
    if !text.is_empty() || out.is_empty() {
        out.push(Piece::Text(text));
    }
    Ok(out)
}

/// A string token's value, when it has no holes.
pub fn value(token: &str) -> Result<Option<String>, StringError> {
    let pieces = pieces(token)?;
    Ok(match pieces.as_slice() {
        [Piece::Text(t)] => Some(t.clone()),
        _ => None,
    })
}

/// `\u{..}` after the `u`: one to six hex digits in braces, naming a
/// Unicode scalar value.
fn unicode(
    chars: &mut std::iter::Peekable<std::str::CharIndices<'_>>,
    at: usize,
) -> Result<char, StringError> {
    let refuse = |message: String| StringError { at, message };
    if chars.next().map(|(_, c)| c) != Some('{') {
        return Err(refuse(
            "`\\u` takes its code point in braces: `\\u{1F600}`".into(),
        ));
    }
    let mut digits = String::new();
    loop {
        match chars.next() {
            Some((_, '}')) => break,
            Some((_, c)) if c.is_ascii_hexdigit() && digits.len() < 6 => digits.push(c),
            _ => {
                return Err(refuse(
                    "`\\u{..}` holds one to six hex digits and a closing `}`".into(),
                ));
            }
        }
    }
    let n = u32::from_str_radix(&digits, 16)
        .map_err(|_| refuse("`\\u{}` holds at least one hex digit".into()))?;
    char::from_u32(n).ok_or_else(|| {
        refuse(format!(
            "`\\u{{{digits}}}` is not a Unicode scalar value: a surrogate, or past U+10FFFF"
        ))
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn text(t: &str) -> Piece {
        Piece::Text(t.to_string())
    }

    #[test]
    fn each_escape_is_its_character() {
        assert_eq!(
            value(r#""a\nb\tc\rd\\e\"f\{g\}h""#).unwrap(),
            Some("a\nb\tc\rd\\e\"f{g}h".to_string())
        );
        assert_eq!(
            value(r#""\u{41}\u{1F600}""#).unwrap(),
            Some("A😀".to_string())
        );
    }

    #[test]
    fn an_escape_the_language_does_not_define_is_an_error() {
        for bad in [
            r#""\q""#,
            r#""\x41""#,
            r#""\u0041""#,
            r#""\u{}""#,
            r#""\u{1234567}""#,
        ] {
            assert!(pieces(bad).is_err(), "{bad} must be refused");
        }
        // Surrogates and values past U+10FFFF name no character.
        assert!(pieces(r#""\u{D800}""#).is_err());
        assert!(pieces(r#""\u{110000}""#).is_err());
    }

    #[test]
    fn a_hole_is_its_source_and_where_it_starts() {
        let token = r#""charging {token} now""#;
        assert_eq!(
            pieces(token).unwrap(),
            vec![
                text("charging "),
                Piece::Hole {
                    source: "token".to_string(),
                    at: 11
                },
                text(" now")
            ]
        );
        assert_eq!(&token[11..16], "token");
    }

    #[test]
    fn an_escaped_brace_opens_no_hole() {
        assert_eq!(pieces(r#""\{x}""#).unwrap(), vec![text("{x}")]);
        assert_eq!(value(r#""a\{b\}c""#).unwrap(), Some("a{b}c".to_string()));
    }

    #[test]
    fn an_unmatched_brace_is_text_and_an_empty_hole_is_an_error() {
        assert_eq!(value(r#""a { b""#).unwrap(), Some("a { b".to_string()));
        assert_eq!(value(r#""a } b""#).unwrap(), Some("a } b".to_string()));
        assert!(pieces(r#""a {} b""#).is_err());
        assert!(pieces(r#""a { } b""#).is_err());
    }

    #[test]
    fn a_triple_quoted_string_is_raw() {
        assert_eq!(
            value("\"\"\"a\\n {b}\nc\"\"\"").unwrap(),
            Some("a\\n {b}\nc".to_string())
        );
    }

    #[test]
    fn a_string_with_a_hole_has_no_single_value() {
        assert_eq!(value(r#""a {b}""#).unwrap(), None);
    }
}
