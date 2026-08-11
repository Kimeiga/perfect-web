//! Charter §8.2 — internal links are checked against the route table.
//!
//! A broken internal link is statically detectable, so it should never reach
//! production. The route table is what the program declares: every `page` with
//! a `route "/…"` clause. Nothing is inferred from a file's name or a module's
//! path, because a link is dead relative to what the program *declares*, and a
//! convention would make the check agree with itself rather than with the app.
//!
//! Only internal links are checked. `href="https://…"`, `href="#section"` and
//! `href="mailto:…"` name something this program does not own.

use std::collections::BTreeSet;

use crate::codes;
use crate::diagnostics::{Detector, Diagnostic, Related, Repair, Severity};
use crate::hir::{AttrValue, Expr, Hir, Node};

/// Every route the program declares.
pub fn table(hirs: &[&Hir]) -> BTreeSet<String> {
    let mut out = BTreeSet::new();
    for hir in hirs {
        for (_, decl) in hir.all_decls() {
            // **The policy first, the body scan second** — the same order
            // `declared_world` and `declared_cache` use, and for the same
            // reason. `route "/stores/{id}"` was a bare `Name` pair in a page's
            // executable body until 2026-08-11, when UI declarations began
            // parsing their policies inside their braces (architect ruling,
            // policy values leave the executable body tree). This reader was
            // the one that still looked only in the body, and `R-023` lost its
            // catch: with no route table, every link is checked against
            // nothing.
            if let Some(p) = decl.policy("route") {
                let v = p.value.trim();
                if !v.is_empty() {
                    out.insert(v.trim_matches('"').to_string());
                    continue;
                }
            }
            let Some(body_id) = decl.body else { continue };
            let body = hir.body(body_id);
            let Expr::Block { stmts } = body.expr(body.root) else {
                continue;
            };
            let mut it = stmts.iter().peekable();
            while let Some(s) = it.next() {
                if !matches!(body.expr(*s), Expr::Name(n) if n == "route") {
                    continue;
                }
                let Some(next) = it.peek() else { continue };
                if let Some(path) = body.string_text(**next) {
                    out.insert(path.trim_matches('"').to_string());
                }
            }
        }
    }
    out
}

pub fn check(hir: &Hir, table: &BTreeSet<String>, out: &mut Vec<Diagnostic>) {
    // With no declared routes there is nothing to check against, and reporting
    // every link would be reporting that the feature is unused.
    if table.is_empty() {
        return;
    }

    for (id, decl) in hir.all_decls() {
        let Some(body_id) = decl.body else { continue };
        let body = hir.body(body_id);
        let at = hir.decl_span(id);
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
            if tag != "a" {
                continue;
            }
            for a in attrs {
                if a.name != "href" {
                    continue;
                }
                let AttrValue::Static(raw) = &a.value else {
                    continue;
                };
                let target = raw.trim_matches('"');
                if !is_internal(target) || table.iter().any(|r| matches(r, target)) {
                    continue;
                }
                out.push(Diagnostic {
                    code: codes::DEAD_INTERNAL_LINK.id,
                    invariant: codes::DEAD_INTERNAL_LINK.invariant,
                    reason: "link_matches_no_declared_route",
                    detector: Detector::PatternMatrix,
                    severity: Severity::Error,
                    message: format!("internal link target `{target}` matches no declared route"),
                    primary_span: a.span.clone(),
                    related: vec![Related {
                        span: at.clone(),
                        label: format!("`{}` renders the link", decl.name),
                    }],
                    explanation: Some(format!(
                        "Internal links are checked against the route table, and this \
                         program declares {}. A link that matches none of them is \
                         broken at the moment it is written rather than the moment \
                         somebody clicks it.",
                        table
                            .iter()
                            .map(|r| format!("`{r}`"))
                            .collect::<Vec<_>>()
                            .join(", ")
                    )),
                    repairs: vec![Repair {
                        description: format!(
                            "declare a page with `route \"{target}\"`, or link to a \
                             route that exists"
                        ),
                        replacement: None,
                    }],
                });
            }
        }
    }
}

/// Does this program own the target?
fn is_internal(target: &str) -> bool {
    target.starts_with('/') && !target.starts_with("//")
}

/// Does a declared route match a link target?
///
/// Segment by segment, where a `{…}` segment in the route matches any single
/// segment in the target. `/stores/{id}` matches `/stores/7`, and matches
/// `/stores/{id}` as written in source, because the interpolation occupies
/// exactly one segment. It does not match `/stores/7/reviews`: a parameter
/// stands for one segment, not for the rest of the path.
fn matches(route: &str, target: &str) -> bool {
    let r: Vec<&str> = route.trim_matches('/').split('/').collect();
    let t: Vec<&str> = target.trim_matches('/').split('/').collect();
    r.len() == t.len()
        && r.iter()
            .zip(&t)
            .all(|(r, t)| (r.starts_with('{') && r.ends_with('}')) || r == t)
}
