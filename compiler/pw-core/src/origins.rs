//! **A page loads from its own origin** (ADR-0195's ruling 13, its build's
//! half; PW5052).
//!
//! The host sends each page a Content-Security-Policy generated from its
//! build and its deployment (`spikes/own-renderer/server/src/csp.rs`): what
//! a page loads comes from this origin, or from a provider its deployment
//! names. An address the markup writes as text that names another origin
//! would build, and its page's policy would refuse it in the browser. It is
//! refused here instead, where it is written:
//!
//! - an address a browser fetches, or sends a form to, written as text, is
//!   a path: no scheme, and no `//` that names a host (WHATWG URL: a URL's
//!   leading and trailing C0 controls and spaces are stripped, its tabs and
//!   newlines removed, and a `\` is a `/` in an `http` one);
//! - a `<link>`'s address is written as text (ruling 13: "a literal or a
//!   build's asset", and a build has no assets): a value names an origin no
//!   build can read. A page's canonical address is a page's (PW5051), and a
//!   link that is an item's property (`itemprop`, no `rel`) is a value a
//!   browser never fetches.
//!
//! A form's `action` is PW5041's: a route that answers its `method`. A
//! link's `href` (`<a>`, `<area>`) is a navigation, which no policy
//! governs. An address a value chooses, `<img src={…}>`, is the policy's to
//! hold in the browser: no build can read where it points.

use crate::codes;
use crate::diagnostics::{Detector, Diagnostic, Related, Repair, Severity};
use crate::hir::{AttrValue, DeclKind, Expr, Hir, Node};

/// **What a browser fetches, or sends to**, by element and attribute: a
/// resource it loads, a frame, a submission, a ping.
const LOADS: &[(&str, &str)] = &[
    ("link", "href"),
    ("img", "src"),
    ("img", "srcset"),
    ("source", "src"),
    ("source", "srcset"),
    ("video", "src"),
    ("video", "poster"),
    ("audio", "src"),
    ("track", "src"),
    ("iframe", "src"),
    ("embed", "src"),
    ("object", "data"),
    ("input", "src"),
    ("input", "formaction"),
    ("button", "formaction"),
    ("a", "ping"),
    ("area", "ping"),
    ("image", "href"),
    ("use", "href"),
];

/// **The origin an address written as text names**, where it is not this
/// one: its scheme and what follows it up to its path (`https://cdn.example`),
/// a scheme with no host (`data:`), or the host a `//` names; `None` for a
/// path, a query or a fragment, which a browser resolves against this page's
/// own address.
pub fn another_origin(address: &str) -> Option<String> {
    // As the URL parser reads it: its leading and trailing C0 controls and
    // spaces stripped, every tab and newline removed.
    let trimmed = address.trim_matches(|c: char| c <= ' ');
    let read: String = trimmed
        .chars()
        .filter(|c| !matches!(c, '\t' | '\n' | '\r'))
        .collect();
    let scheme_end = read.find(':').filter(|&end| {
        let scheme = &read[..end];
        scheme.starts_with(|c: char| c.is_ascii_alphabetic())
            && scheme
                .chars()
                .all(|c| c.is_ascii_alphanumeric() || matches!(c, '+' | '-' | '.'))
    });
    if let Some(end) = scheme_end {
        let rest = &read[end + 1..];
        let authority = rest
            .trim_start_matches(['/', '\\'])
            .split(['/', '\\', '?', '#'])
            .next()
            .unwrap_or_default();
        return Some(if rest.starts_with(['/', '\\']) && !authority.is_empty() {
            format!("{}://{}", read[..end].to_ascii_lowercase(), authority)
        } else {
            format!("{}:", read[..end].to_ascii_lowercase())
        });
    }
    // A `//`, and its `\` spellings, names a host: the page's scheme, and
    // another origin.
    let mut leading = read.chars();
    if matches!(
        (leading.next(), leading.next()),
        (Some('/' | '\\'), Some('/' | '\\'))
    ) {
        let host = read[2..]
            .split(['/', '\\', '?', '#'])
            .next()
            .unwrap_or_default();
        return Some(format!("//{host}"));
    }
    None
}

/// Each address an attribute's text holds: one, or a `srcset`'s each
/// candidate, its descriptor after its address.
fn addresses<'t>(attribute: &str, text: &'t str) -> Vec<&'t str> {
    if attribute.eq_ignore_ascii_case("srcset") {
        text.split(',')
            .filter_map(|candidate| candidate.split_ascii_whitespace().next())
            .collect()
    } else {
        vec![text]
    }
}

/// **An address built from text and values, as text**: each value
/// `{…}` an ellipsis, which no scheme, host separator or path separator is,
/// so what the text names is read whole, and a value's place in an origin
/// shows as one (`https://…`).
fn with_values_unread(text: &str) -> String {
    let mut out = String::new();
    let mut rest = text;
    while let Some(open) = rest.find('{') {
        out.push_str(&rest[..open]);
        match rest[open..].find('}') {
            Some(close) => {
                out.push('\u{2026}');
                rest = &rest[open + close + 1..];
            }
            None => {
                rest = &rest[open..];
                break;
            }
        }
    }
    out.push_str(rest);
    out
}

/// An attribute value's text as written, its quotes removed.
fn unquoted(v: &str) -> &str {
    v.strip_prefix('"')
        .and_then(|v| v.strip_suffix('"'))
        .or_else(|| v.strip_prefix('\'').and_then(|v| v.strip_suffix('\'')))
        .unwrap_or(v)
}

/// **What `hir` loads from another origin**, and each `<link>` whose address
/// a value chooses.
pub fn check(hir: &Hir) -> Vec<Diagnostic> {
    let mut out = Vec::new();
    for (decl_id, decl) in hir.all_decls() {
        let Some(body_id) = decl.body else { continue };
        let body = hir.body(body_id);
        let mut roots = Vec::new();
        for e in body.walk() {
            if let Expr::Template { roots: r, .. } = body.expr(e) {
                roots.extend(r.iter().copied());
            }
        }
        for n in body.walk_markup(&roots) {
            let Node::Element { tag, attrs, .. } = body.node(n) else {
                continue;
            };
            let element = tag.to_ascii_lowercase();
            let has = |name: &str| attrs.iter().any(|a| a.name.eq_ignore_ascii_case(name));
            for a in attrs {
                // `xlink:href` is SVG's older spelling of `href`.
                let name = a
                    .name
                    .rsplit(':')
                    .next()
                    .unwrap_or(&a.name)
                    .to_ascii_lowercase();
                if !LOADS.contains(&(element.as_str(), name.as_str())) {
                    continue;
                }
                // A link that is an item's property is a value, never fetched;
                // a page's canonical address is a page's (PW5051).
                if element == "link"
                    && ((!has("rel") && has("itemprop"))
                        || crate::canonical::is_canonical_link(body, n))
                {
                    continue;
                }
                let written = |text: &str| -> Option<String> {
                    addresses(&name, unquoted(text))
                        .into_iter()
                        .find_map(another_origin)
                };
                let message = match &a.value {
                    AttrValue::Static(text) => written(text).map(|origin| {
                        format!(
                            "`<{tag} {}=\"{}\">` in `{}` loads from {}, another origin: \
                             a page loads from its own",
                            a.name,
                            unquoted(text),
                            decl.name,
                            describe(&origin)
                        )
                    }),
                    AttrValue::Expr(e) => match body.expr(*e) {
                        // An address built from text and values: read whole, each
                        // value a placeholder that names nothing.
                        Expr::Interpolated { text, .. } => {
                            let start = unquoted(text).split('{').next().unwrap_or_default();
                            match written(with_values_unread(unquoted(text)).as_str()) {
                                Some(origin) => Some(format!(
                                    "`<{tag} {}=\"{}\">` in `{}` loads from {}, another \
                                     origin: a page loads from its own",
                                    a.name,
                                    unquoted(text),
                                    decl.name,
                                    describe(&origin)
                                )),
                                // A link's text that is already a path, a query or a
                                // fragment before its first value names this origin;
                                // one that is not could still become any.
                                None if element == "link" && !start.contains(['/', '?', '#']) => {
                                    Some(computed(tag, &a.name, &decl.name))
                                }
                                None => None,
                            }
                        }
                        _ if element == "link" => Some(computed(tag, &a.name, &decl.name)),
                        _ => None,
                    },
                    AttrValue::None => None,
                };
                let Some(message) = message else { continue };
                out.push(diagnostic(
                    hir,
                    decl_id,
                    &decl.name,
                    decl.kind,
                    a.span.clone(),
                    message,
                ));
            }
        }
    }
    out
}

/// How an origin is named in a message.
fn describe(origin: &str) -> String {
    format!("`{origin}`")
}

fn computed(tag: &str, attribute: &str, decl: &str) -> String {
    format!(
        "`<{tag} {attribute}>` in `{decl}` is an address a value chooses: a `<link>`'s \
         address is written as text, so its build knows the origin it names"
    )
}

fn diagnostic(
    hir: &Hir,
    decl_id: crate::hir::DeclId,
    decl: &str,
    kind: DeclKind,
    at: crate::hir::Span,
    message: String,
) -> Diagnostic {
    let renders = match kind {
        DeclKind::Page => "renders this",
        _ => "renders this, in each page that uses it",
    };
    Diagnostic {
        code: codes::LOAD_FROM_ANOTHER_ORIGIN.id,
        invariant: codes::LOAD_FROM_ANOTHER_ORIGIN.invariant,
        reason: "load_from_another_origin",
        detector: Detector::DeclarationRule,
        severity: Severity::Error,
        message,
        primary_span: at,
        related: vec![Related::new(
            hir.decl_span(decl_id),
            format!("`{decl}` {renders}"),
        )],
        explanation: Some(
            "The host sends each page a Content-Security-Policy generated from its build \
             and its deployment: a page loads its images, frames, styles and scripts from \
             its own origin, and from the providers its deployment names, and from no other. \
             An address written here that names another origin would build, and the \
             browser would refuse to load it. A `<link>`'s address is written as text, so \
             the build knows which origin it names. A link (`<a href>`) goes anywhere: a \
             navigation is not a load."
                .to_string(),
        ),
        repairs: vec![Repair {
            description: "serve what it loads from this origin, and write its path (`/logo.png`)"
                .to_string(),
            replacement: None,
        }],
    }
}
