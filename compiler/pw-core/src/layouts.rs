//! **A page's layout** (ADR-0303): the markup and the bindings the pages
//! that name it share, each page shown in its `<slot />`.
//!
//! ```text
//! session layout StoreLayout {
//!     let cart = query Cart(current_session())
//!     view {
//!         <header>… {cart.line_count} …</header>
//!         <slot />
//!     }
//! }
//!
//! session page CartPage() {
//!     route  "/cart"
//!     layout StoreLayout
//!     …
//! }
//! ```
//!
//! A layout is composed into each page that names it, as a view is
//! (ADR-0136), with its parts and elements numbered before the page's, so
//! they are the same on every such page. Its bindings and signals are the
//! page's, under names no source can write: [`bound`].

use crate::diagnostics::{Detector, Diagnostic, Related, Repair, Severity};
use crate::hir::{Decl, DeclKind, Expr, Hir, Node, Span};
use crate::resolve::{DefId, Namespace, Resolution, Workspace};

/// The markers around the page's markup in its layout's: what a navigation
/// finds to show another page in the layout it keeps. Comments a browser
/// keeps in the document, and not the renderer's part markers
/// (`<!--pw:s3-->`), which the runtime indexes.
pub const SLOT_START: &str = "<!--pw-slot-->";
pub const SLOT_END: &str = "<!--/pw-slot-->";

/// **The layout a page's `layout` clause names, as written**, and where.
/// Read where a clause is (the page's policies), and where a body writes it
/// as two names on a line, as a route is (`routes::declared_route`): a
/// clause written after a statement is not lost.
pub fn written(hir: &Hir, decl: &Decl) -> Option<(String, Span)> {
    if let Some(p) = decl.policy("layout") {
        let v = p.value.trim();
        if !v.is_empty() {
            return Some((v.to_string(), p.span.clone()));
        }
    }
    let body = hir.body(decl.body?);
    let Expr::Block { stmts } = body.expr(body.root) else {
        return None;
    };
    let mut it = stmts.iter().peekable();
    while let Some(s) = it.next() {
        if !matches!(body.expr(*s), Expr::Name(n) if n == "layout") {
            continue;
        }
        if let Some(next) = it.peek()
            && let Expr::Name(n) = body.expr(**next)
        {
            return Some((n.clone(), body.expr_span(**next)));
        }
    }
    None
}

/// What a page's `layout` clause names, resolved in the page's file as a
/// view's tag is: a declaration, or nothing visible.
pub fn named(ws: &Workspace, unit: usize, name: &str) -> Option<DefId> {
    match ws.resolve_in(unit, Namespace::Ui, name) {
        Resolution::Local(def) | Resolution::Imported { def, .. } => Some(def),
        _ => None,
    }
}

/// **The layout a page is shown in**: its declaration, where the page names
/// one and it is a layout. Anything else is no layout, and PW0352 says why.
pub fn of(hirs: &[&Hir], ws: &Workspace, unit: usize, decl: &Decl) -> Option<DefId> {
    if decl.kind != DeclKind::Page {
        return None;
    }
    let (name, _) = written(hirs.get(unit)?, decl)?;
    let def = named(ws, unit, &name)?;
    crate::resolve::declaration(hirs, def)
        .is_some_and(|d| d.kind == DeclKind::Layout)
        .then_some(def)
}

/// **The name a layout's own value is read by in a page**: `cart~StoreLayout`
/// for its binding `cart`, `open~StoreLayout` for its signal `open`. `~` is
/// in no identifier, so no source writes it, and a view's renaming
/// (`open~2`, ADR-0136) ends in a number where this ends in a name.
pub fn bound(layout: &str, name: &str) -> String {
    format!("{name}~{layout}")
}

/// **What a layout and the pages that name it are held to** (ADR-0303):
///
/// - a page's `layout` names a layout, once (PW0352);
/// - a layout is given nothing: no parameters (PW0353);
/// - its view holds one `<slot />`, at its top, empty (PW5044, PW5045), and
///   nothing else holds one (PW5044);
/// - a page declares at least its layout's audience (PW5046).
///
/// The clauses a layout does not take, `route`, `cache`, `placement` and
/// the rest, are the policy table's (PW5105), as `layout` written anywhere
/// but a page is.
pub fn check(hirs: &[&Hir], ws: &Workspace, unit: usize) -> Vec<Diagnostic> {
    let mut out = Vec::new();
    let Some(hir) = hirs.get(unit) else {
        return out;
    };
    for (_, decl) in hir.all_decls() {
        slots(hir, decl, &mut out);
        if decl.kind == DeclKind::Layout
            && let Some(first) = decl.params.first()
        {
            let span = Span {
                start: first.span.start,
                end: decl.params.last().map_or(first.span.end, |p| p.span.end),
            };
            out.push(diagnostic(
                crate::codes::LAYOUT_TAKES_NOTHING,
                "layout_takes_nothing",
                format!(
                    "the layout `{}` declares parameters, and a page gives its layout nothing",
                    decl.name
                ),
                span,
                "A layout is the markup and the bindings the pages that name it share, \
                 kept in place as one page gives way to another: what it shows is read \
                 from the request's reader, never from the page, so two pages that name \
                 it show it the same.",
                "read what it shows from the reader (`current_session()`, \
                 `current_user()`), or show it in the page",
            ));
        }
        if decl.kind != DeclKind::Page {
            continue;
        }
        let clauses: Vec<&crate::hir::Policy> = decl
            .policies
            .iter()
            .filter(|p| p.name == "layout")
            .collect();
        if let [_, second, ..] = clauses.as_slice() {
            out.push(diagnostic(
                crate::codes::LAYOUT_NAMES_A_LAYOUT,
                "layout_names_a_layout",
                format!("the page `{}` names two layouts", decl.name),
                second.span.clone(),
                "A page is shown in one layout: its markup is placed in that layout's \
                 `<slot />`.",
                "remove one `layout` clause",
            ));
        }
        let Some((name, span)) = written(hir, decl) else {
            continue;
        };
        let named = named(ws, unit, &name);
        let kind = named.and_then(|d| crate::resolve::declaration(hirs, d).map(|d| d.kind));
        if kind != Some(DeclKind::Layout) {
            let what = match kind {
                Some(k) => format!("names {}", crate::check::described(k)),
                None => "names nothing this module sees".to_string(),
            };
            out.push(diagnostic(
                crate::codes::LAYOUT_NAMES_A_LAYOUT,
                "layout_names_a_layout",
                format!("`layout {name}` {what}: a page is shown in a layout"),
                span,
                "`layout` names the layout declaration whose markup the page is shown \
                 in, which the module declares or imports.",
                "name a `layout` this module declares or imports",
            ));
            continue;
        }
        let (Some(def), Some(layout)) = (
            named,
            named.and_then(|d| crate::resolve::declaration(hirs, d)),
        ) else {
            continue;
        };
        // The page's audience against its layout's: what its resume manifest
        // may hold (ADR-0172), which the layout's handlers' captures are
        // checked against where they are written.
        let theirs = crate::resume::manifest_scope(hirs[def.unit], layout);
        let ours = crate::resume::manifest_scope(hir, decl);
        let short = match (&theirs, &ours) {
            (Some(l), Some(p)) => !l.flows_into(p),
            (Some(l), None) => !l.is_public(),
            (None, _) => false,
        };
        if short {
            let audience = |d: &Decl| d.visibility.clone().unwrap_or_else(|| "public".into());
            out.push(Diagnostic {
                related: vec![Related {
                    span: layout.name_span.clone(),
                    label: format!("`{}` is declared `{}` here", layout.name, audience(layout)),
                }],
                ..diagnostic(
                    crate::codes::LAYOUT_AUDIENCE,
                    "layout_audience",
                    format!(
                        "the page `{}` is `{}`, and its layout `{}` is `{}`: the page's \
                         document holds the layout's values",
                        decl.name,
                        audience(decl),
                        layout.name,
                        audience(layout)
                    ),
                    span,
                    "A page's document holds what its layout shows, and its resume \
                     manifest what the layout's handlers capture. A layout declared for a \
                     session's eyes, shown in a page declared for anyone's, would be a \
                     session's values in a document anyone may be served.",
                    &format!("declare the page `{}`", audience(layout)),
                )
            });
        }
    }
    out
}

/// Each `<slot />` in `decl`'s markup, held to where it may be (PW5044),
/// and a layout's view held to one (PW5045).
fn slots(hir: &Hir, decl: &Decl, out: &mut Vec<Diagnostic>) {
    let Some(b) = decl.body else { return };
    let body = hir.body(b);
    let roots = crate::template_ir::roots_of(body);
    if roots.is_empty() && decl.kind != DeclKind::Layout {
        return;
    }
    // Each slot, with whether it is at the top: in no block and no stream.
    let mut found: Vec<(crate::hir::NodeId, bool)> = Vec::new();
    let mut stack: Vec<(crate::hir::NodeId, bool)> =
        roots.iter().rev().map(|r| (*r, true)).collect();
    while let Some((id, top)) = stack.pop() {
        match body.node(id) {
            Node::Element { tag, children, .. } => {
                if tag == "slot" {
                    found.push((id, top));
                }
                let inside = top && tag != "stream";
                stack.extend(children.iter().rev().map(|c| (*c, inside)));
            }
            Node::Block { children, .. } => {
                stack.extend(children.iter().rev().map(|c| (*c, false)));
            }
            _ => {}
        }
    }
    let mut placed = false;
    for (id, top) in found {
        let Node::Element {
            attrs, children, ..
        } = body.node(id)
        else {
            continue;
        };
        let reason = if decl.kind != DeclKind::Layout {
            Some(format!(
                "`<slot />` in {} `{}`: a slot is a layout's, where the page that names it \
                 is shown",
                crate::check::described(decl.kind),
                decl.name
            ))
        } else if !top {
            Some(format!(
                "`<slot />` in a block or a stream of the layout `{}`: the page would be \
                 shown only when it decides so, or more than once",
                decl.name
            ))
        } else if !attrs.is_empty() || !children.is_empty() {
            Some(format!(
                "`<slot />` with {} in the layout `{}`: the page is what it holds",
                if attrs.is_empty() {
                    "content"
                } else {
                    "attributes"
                },
                decl.name
            ))
        } else if placed {
            Some(format!(
                "a second `<slot />` in the layout `{}`: a page is shown once",
                decl.name
            ))
        } else {
            placed = true;
            None
        };
        if let Some(message) = reason {
            out.push(diagnostic(
                crate::codes::SLOT_MISPLACED,
                "slot_misplaced",
                message,
                body.node_span(id),
                "A layout's `<slot />` is where the page that names the layout is shown: \
                 once, at the top of the layout's own view, so every page is shown whole \
                 and the layout's parts are numbered the same on every page.",
                "write one `<slot />` at the top of the layout's view, outside any block",
            ));
        }
    }
    if decl.kind == DeclKind::Layout && !placed {
        out.push(diagnostic(
            crate::codes::LAYOUT_WITHOUT_SLOT,
            "layout_without_slot",
            format!(
                "the layout `{}` has no `<slot />` to show its page in",
                decl.name
            ),
            decl.name_span.clone(),
            "A layout is shown around the page that names it, which is placed where its \
             view writes `<slot />`.",
            "write `<slot />` in the layout's view, where the page goes",
        ));
    }
}

fn diagnostic(
    code: crate::codes::Code,
    reason: &'static str,
    message: String,
    span: Span,
    explanation: &str,
    repair: &str,
) -> Diagnostic {
    Diagnostic {
        code: code.id,
        invariant: code.invariant,
        reason,
        detector: Detector::DeclarationRule,
        severity: Severity::Error,
        message,
        primary_span: span,
        related: Vec::new(),
        explanation: Some(explanation.to_string()),
        repairs: vec![Repair {
            description: repair.to_string(),
            replacement: None,
        }],
    }
}
