//! **A page states its canonical address** (ADR-XXXX): `<link
//! rel="canonical" href={ItemPage(name)} />` at the top of its view, beside
//! its title (ADR-0183) and its metadata (ADR-0186). Its address is a page's,
//! named by its declaration and given its parameters as `navigate` gives
//! them (ADR-0280). The host writes it into the document's head, absolute
//! with the deployment's public origin, and `og:url` from it.
//!
//! Where a page's canonical link is, and what it addresses, is read here
//! once, for three readers: the checker (PW5035 where it is, PW5051 what it
//! addresses, and no PW0027 for the page its address names), the value
//! relations, which relate its arguments to the page's parameters, and the
//! template IR, which lowers it to a part of the head.

use std::collections::BTreeSet;

use crate::hir::{AttrValue, Body, Decl, DeclKind, Expr, ExprId, Node, NodeId};
use crate::resolve::{DefId, Namespace, Resolution, Workspace};

/// Whether `rel`, as written, names `canonical`, compared as HTML compares
/// keywords, ignoring ASCII case.
pub(crate) fn names_canonical(rel: &str) -> bool {
    rel.split_ascii_whitespace()
        .any(|r| r.eq_ignore_ascii_case("canonical"))
}

/// Whether a node is a `<link>` whose `rel`, as written, names `canonical`.
/// A `rel` computed in place names nothing here, and is PW5035's.
pub(crate) fn is_canonical_link(body: &Body, n: NodeId) -> bool {
    let Node::Element { tag, attrs, .. } = body.node(n) else {
        return false;
    };
    tag == "link"
        && attrs.iter().any(|a| {
            a.name == "rel"
                && matches!(&a.value, AttrValue::Static(v)
                    if names_canonical(v.trim_matches(|c| c == '"' || c == '\'')))
        })
}

/// **The page's canonical link**: the first `<link rel="canonical">` at the
/// top of a page's view. Any other one, a second, one in a view, one inside
/// an element or a block, is refused (PW5035) and is not the page's.
pub(crate) fn page_link(decl: &Decl, body: &Body) -> Option<NodeId> {
    if decl.kind != DeclKind::Page {
        return None;
    }
    crate::template_ir::roots_of(body)
        .into_iter()
        .find(|r| is_canonical_link(body, *r))
}

/// **The address a canonical link states**: its `href`, where that is a
/// call, `ItemPage(name)`. Anything else is refused (PW5051).
pub(crate) fn address(body: &Body, link: NodeId) -> Option<ExprId> {
    let Node::Element { attrs, .. } = body.node(link) else {
        return None;
    };
    let href = attrs.iter().find(|a| a.name == "href")?;
    match &href.value {
        AttrValue::Expr(e) if matches!(body.expr(*e), Expr::Call { .. }) => Some(*e),
        _ => None,
    }
}

/// **The calls in a body that are a page's address**: each canonical link's,
/// the page's own and any other, which PW5035 refuses saying where one goes,
/// not that a page is called. A call elsewhere that names a page is refused
/// (PW0027): a page is reached by its route, not called.
pub(crate) fn address_calls(body: &Body) -> BTreeSet<ExprId> {
    body.walk_markup(&crate::template_ir::roots_of(body))
        .into_iter()
        .filter(|n| is_canonical_link(body, *n))
        .filter_map(|n| address(body, n))
        .collect()
}

/// **The page an address names**, by its declaration, among the views, as
/// `navigate` names one (ADR-0280). `None` where the path names nothing a
/// page could be; what it names then is PW5051's to say.
pub(crate) fn page_named(ws: &Workspace, unit: usize, path: &str) -> Option<DefId> {
    let found = match path.contains('.') {
        true => ws.resolve_path_in(unit, Namespace::Ui, path),
        false => ws.resolve_in(unit, Namespace::Ui, path),
    };
    match found {
        Resolution::Local(d) | Resolution::Imported { def: d, .. } => Some(d),
        _ => None,
    }
}
