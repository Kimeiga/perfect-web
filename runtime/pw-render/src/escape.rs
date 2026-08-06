//! Context-sensitive escaping.
//!
//! Architect ruling, 2026-08-06:
//!
//! > Do not have one generic `escape_html(value)` and use it everywhere. The
//! > renderer must know whether a value occupies Text, Attribute, URL, Style
//! > or RawHtml.
//!
//! The reason is that the same bytes are safe in one place and an exploit in
//! another. `<script>alert(1)</script>` is inert text and a script element.
//! `" onclick="steal()` is an ordinary string and an attribute breakout. A
//! `javascript:` URL is neither of those and no amount of character escaping
//! touches it.
//!
//! # Escaping is chosen by the IR, not by the call site
//!
//! Every function here takes the value and nothing else. The decision of WHICH
//! to call was made in `pw_core::template_ir`, from where the value appears in
//! the markup, and travels with the part. A renderer that decided at the call
//! site would be making the same decision in two places, and the second one is
//! the one nobody reviews.

/// Character data between tags.
///
/// `&`, `<` and `>` — the last is not strictly required by the parser in text
/// position, and it is escaped anyway because a value spanning a `<` from
/// elsewhere is easier to reason about when neither end can appear.
pub fn text(value: &str) -> String {
    let mut out = String::with_capacity(value.len() + 8);
    for c in value.chars() {
        match c {
            '&' => out.push_str("&amp;"),
            '<' => out.push_str("&lt;"),
            '>' => out.push_str("&gt;"),
            _ => out.push(c),
        }
    }
    out
}

/// A double-quoted attribute value.
///
/// `"` is the one that breaks out. `&` because an entity would otherwise be
/// decoded by the parser and change the value. `'` and `` ` `` because a
/// downstream consumer may requote, and `<` because a value containing one is
/// never intended as markup.
///
/// The renderer always emits double quotes, so a single quote could be left
/// alone. It is escaped anyway: the cost is four bytes and the alternative is
/// a rule that depends on a choice made elsewhere in the file.
pub fn attribute(value: &str) -> String {
    let mut out = String::with_capacity(value.len() + 8);
    for c in value.chars() {
        match c {
            '&' => out.push_str("&amp;"),
            '"' => out.push_str("&quot;"),
            '\'' => out.push_str("&#39;"),
            '<' => out.push_str("&lt;"),
            '>' => out.push_str("&gt;"),
            '`' => out.push_str("&#96;"),
            _ => out.push(c),
        }
    }
    out
}

/// Schemes that execute rather than locate.
const DANGEROUS_SCHEMES: &[&str] = &["javascript:", "vbscript:", "data:"];

/// A URL-valued attribute.
///
/// Two jobs, and only the second is escaping.
///
/// **The scheme.** `javascript:alert(1)` in an `href` is a script, and character
/// escaping does not touch it — the parser decodes entities before the
/// navigation code sees the value, so `&#106;avascript:` runs too. A scheme
/// that executes is replaced entirely, because there is no way to render it
/// safely and rendering it partially is worse.
///
/// `data:` is refused with the others. It is not always dangerous, and deciding
/// which data URLs are safe is a media-type question this layer cannot answer
/// — a caller that needs one has a value carrying the raw capability.
///
/// **The characters.** Then the ordinary attribute rules, because the value
/// still sits inside quotes.
pub fn url(value: &str) -> String {
    let normalized: String = value
        .chars()
        .filter(|c| !c.is_whitespace() && !c.is_control())
        .collect::<String>()
        .to_ascii_lowercase();
    // Entity-decoded before comparison, because the HTML parser decodes before
    // the URL is used and a check on the raw bytes would see a different string
    // from the one the browser navigates to.
    let decoded = decode_entities(&normalized);
    if DANGEROUS_SCHEMES.iter().any(|s| decoded.starts_with(s)) {
        return "about:blank".to_string();
    }
    attribute(value)
}

/// Enough entity decoding to compare a scheme.
///
/// Not a general decoder and not trying to be: the question is only whether the
/// first colon-terminated token is an executing scheme, so numeric and named
/// references for the ASCII letters and the colon are what matter.
fn decode_entities(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    let mut rest = s;
    while let Some(i) = rest.find('&') {
        out.push_str(&rest[..i]);
        rest = &rest[i..];
        let Some(end) = rest[..rest.len().min(12)].find(';') else {
            out.push('&');
            rest = &rest[1..];
            continue;
        };
        let body = &rest[1..end];
        let decoded = if let Some(hex) = body.strip_prefix("#x").or(body.strip_prefix("#X")) {
            u32::from_str_radix(hex, 16).ok().and_then(char::from_u32)
        } else if let Some(dec) = body.strip_prefix('#') {
            dec.parse::<u32>().ok().and_then(char::from_u32)
        } else {
            match body {
                "colon" => Some(':'),
                "amp" => Some('&'),
                "Tab" | "NewLine" => Some(' '),
                _ => None,
            }
        };
        match decoded {
            Some(c) if !c.is_whitespace() && !c.is_control() => out.push(c),
            Some(_) => {}
            None => out.push_str(&rest[..=end]),
        }
        rest = &rest[end + 1..];
    }
    out.push_str(rest);
    out
}

/// A `style` attribute value.
///
/// CSS is its own language and this is not a CSS parser. What it does is remove
/// the two constructs that turn a declaration list into something else —
/// `expression(` (legacy IE, still parsed by some consumers) and a `url(` with
/// an executing scheme — and then apply the attribute rules.
///
/// A caller who needs arbitrary CSS has the same answer as one who needs
/// arbitrary HTML: a value carrying the capability.
pub fn style(value: &str) -> String {
    let lower = value.to_ascii_lowercase();
    let suspicious = lower.contains("expression(")
        || DANGEROUS_SCHEMES
            .iter()
            .any(|s| lower.contains(&format!("url({s}")) || lower.contains(&format!("url('{s}")));
    if suspicious {
        return String::new();
    }
    attribute(value)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_script_in_text_becomes_inert() {
        let out = text("<script>alert(1)</script>");
        assert!(!out.contains('<'), "{out}");
        assert!(out.contains("&lt;script&gt;"), "{out}");
    }

    #[test]
    fn an_attribute_breakout_stays_one_value() {
        let out = attribute("\" onclick=\"steal()");
        assert!(!out.contains('"'), "{out}");
    }

    #[test]
    fn an_executing_scheme_is_replaced_not_escaped() {
        assert_eq!(url("javascript:alert(1)"), "about:blank");
        assert_eq!(url("JaVaScRiPt:alert(1)"), "about:blank");
        // The parser strips these before resolving the scheme, so the check
        // must too — otherwise the string compared is not the string used.
        assert_eq!(url("java\tscript:alert(1)"), "about:blank");
        assert_eq!(url(" javascript:alert(1)"), "about:blank");
        assert_eq!(url("&#106;avascript:alert(1)"), "about:blank");
    }

    #[test]
    fn an_ordinary_url_survives() {
        // The control. Without it the rule above is satisfied by returning
        // "about:blank" for everything.
        assert_eq!(
            url("/menu?store=47&locale=en"),
            "/menu?store=47&amp;locale=en"
        );
        assert_eq!(url("https://example.test/a"), "https://example.test/a");
    }

    #[test]
    fn style_drops_the_two_constructs_that_execute() {
        assert_eq!(style("width: expression(alert(1))"), "");
        assert_eq!(style("background: url(javascript:alert(1))"), "");
        assert_eq!(style("color: red"), "color: red");
    }
}
