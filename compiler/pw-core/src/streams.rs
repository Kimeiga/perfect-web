//! **A stream region, and what reads a streamed query** (ADR-0148).
//!
//! ```text
//! <stream query={Recommendations(id)}>
//!     <placeholder> .. </placeholder>      while a streamed query is pending
//!     <ready as={items}> .. </ready>       what it answered
//!     <failed as={why}> .. </failed>       why not: `Some(e)`, its declared
//! </stream>                                error; `None`, the host's failure
//! ```
//!
//! A query's delivery is its declaration's to say. `delivery streamed` sends
//! the page before the query answers, and the region shows its placeholder
//! until the server sends what the query settled to, in the same response.
//! Any other delivery makes the page wait for it, as a `let` does, and the
//! region shows what it settled to from the start.
//!
//! Three rules, so a region is never in a state its markup does not show:
//! - PW5400: a streamed query is read only by a stream;
//! - PW5401: a stream shows each state its query can be in, and no other;
//! - PW5402: a streamed query declares a `timeout`.

use crate::codes::Code;
use crate::diagnostics::{Detector, Diagnostic, Repair, Severity};
use crate::hir::{AttrValue, Body, Decl, DeclKind, Expr, ExprId, Hir, Node, NodeId, Span};
use crate::resolve::{DefId, Namespace, Resolution, Workspace};
use crate::signatures::Signatures;

/// The three parts a stream holds, in the order a reader meets them.
const PARTS: [&str; 3] = ["placeholder", "ready", "failed"];

/// **Is this query declared `delivery streamed`?** Its page is sent before it
/// answers.
pub fn streamed(decl: &Decl) -> bool {
    decl.policy("delivery")
        .is_some_and(|p| p.value.trim() == "streamed")
}

/// A path written in `unit`, resolved as a term.
fn term(ws: &Workspace, unit: usize, path: &str) -> Option<DefId> {
    let r = match path.contains('.') {
        true => ws.resolve_path(unit, path),
        false => ws.resolve_in(unit, Namespace::Term, path),
    };
    match r {
        Resolution::Local(d) | Resolution::Imported { def: d, .. } => Some(d),
        _ => None,
    }
}

/// **The query a `<stream>` shows**: its `query` attribute, a call of a
/// query, resolved. `None` for anything else.
pub fn stream_query<'a>(
    ws: &Workspace,
    hirs: &'a [&'a Hir],
    unit: usize,
    body: &Body,
    call: ExprId,
) -> Option<(DefId, &'a Decl)> {
    let Expr::Call { callee, .. } = body.expr(call) else {
        return None;
    };
    let def = term(ws, unit, &crate::infer::path_of(body, *callee))?;
    let decl = crate::resolve::declaration(hirs, def)?;
    (decl.kind == DeclKind::Query).then_some((def, decl))
}

/// `query={..}` on a `<stream>`, when written as an expression.
pub fn query_attr(attrs: &[crate::hir::Attr]) -> Option<ExprId> {
    attrs.iter().find_map(|a| match a.value {
        AttrValue::Expr(e) if a.name == "query" => Some(e),
        _ => None,
    })
}

pub fn check(
    ws: &Workspace,
    hirs: &[&Hir],
    sigs: &Signatures,
    unit: usize,
    hir: &Hir,
) -> Vec<Diagnostic> {
    let mut out = Vec::new();
    for (_, decl) in hir.all_decls() {
        if decl.kind == DeclKind::Query && streamed(decl) && decl.policy("timeout").is_none() {
            let at = decl
                .policy("delivery")
                .map_or_else(|| decl.name_span.clone(), |p| p.span.clone());
            out.push(unbounded(decl, at));
        }
        let Some(b) = decl.body else { continue };
        let body = hir.body(b);
        for e in body.walk() {
            match body.expr(e) {
                Expr::Keyword {
                    keyword, modifiers, ..
                } if keyword == "query" => {
                    let Some(name) = modifiers.first() else {
                        continue;
                    };
                    let Some(q) =
                        term(ws, unit, name).and_then(|d| crate::resolve::declaration(hirs, d))
                    else {
                        continue;
                    };
                    if q.kind == DeclKind::Query && streamed(q) {
                        out.push(waited_for(q, body.expr_span(e)));
                    }
                }
                Expr::Template { roots, .. } => {
                    let nodes = body.walk_markup(roots);
                    // What a stream holds directly: where its parts may be.
                    let mut held = std::collections::BTreeSet::new();
                    for n in &nodes {
                        let Node::Element {
                            tag,
                            attrs,
                            children,
                            ..
                        } = body.node(*n)
                        else {
                            continue;
                        };
                        if tag == "stream" {
                            held.extend(children.iter().copied());
                            out.extend(stream(ws, hirs, sigs, unit, body, *n, attrs, children));
                        }
                    }
                    for n in nodes {
                        if let Node::Element { tag, .. } = body.node(n)
                            && PARTS.contains(&tag.as_str())
                            && !held.contains(&n)
                        {
                            out.push(states(
                                body.node_span(n),
                                format!(
                                    "`<{tag}>` is a `<stream>`'s part, and this one is not \
                                     directly inside a `<stream>`"
                                ),
                            ));
                        }
                    }
                }
                _ => {}
            }
        }
    }
    out
}

/// One `<stream>`: its query, and an arm for each state the query can be in.
#[allow(clippy::too_many_arguments)]
fn stream(
    ws: &Workspace,
    hirs: &[&Hir],
    sigs: &Signatures,
    unit: usize,
    body: &Body,
    at: NodeId,
    attrs: &[crate::hir::Attr],
    children: &[NodeId],
) -> Vec<Diagnostic> {
    let mut out = Vec::new();
    let span = body.node_span(at);
    for a in attrs.iter().filter(|a| a.name != "query") {
        out.push(states(
            a.span.clone(),
            format!(
                "a `<stream>` writes no markup of its own, so `{}` would be dropped; put it on \
                 an element inside its parts",
                a.name
            ),
        ));
    }
    let query = match query_attr(attrs) {
        None => {
            out.push(states(
                span.clone(),
                "a `<stream>` names the query it shows: `query={Q(..)}`".to_string(),
            ));
            None
        }
        Some(call) => match stream_query(ws, hirs, unit, body, call) {
            Some(q) => Some(q),
            None => {
                out.push(states(
                    body.expr_span(call),
                    "a `<stream>`'s `query` is a call of a query, whose state it shows".to_string(),
                ));
                None
            }
        },
    };

    // Its parts, each once, and nothing else.
    let mut found: std::collections::BTreeMap<&str, NodeId> = std::collections::BTreeMap::new();
    for c in children {
        match body.node(*c) {
            Node::Text(t) if t.trim().is_empty() => {}
            Node::Element { tag, attrs, .. } if PARTS.contains(&tag.as_str()) => {
                let tag = PARTS
                    .iter()
                    .find(|p| **p == tag)
                    .copied()
                    .unwrap_or_default();
                if found.insert(tag, *c).is_some() {
                    out.push(states(
                        body.node_span(*c),
                        format!("a `<stream>` has one `<{tag}>`, and this is a second"),
                    ));
                }
                for a in attrs {
                    match (tag, a.name.as_str(), &a.value) {
                        ("ready" | "failed", "as", AttrValue::Expr(e))
                            if matches!(body.expr(*e), Expr::Name(_)) => {}
                        ("ready" | "failed", "as", _) => out.push(states(
                            a.span.clone(),
                            format!("`<{tag} as={{..}}>` binds a name: `as={{items}}`"),
                        )),
                        ("placeholder", "as", _) => out.push(states(
                            a.span.clone(),
                            "a `<placeholder>` binds nothing: nothing has answered yet".to_string(),
                        )),
                        _ => out.push(states(
                            a.span.clone(),
                            format!(
                                "`<{tag}>` is a part of its stream, not an element, so `{}` \
                                 would be dropped; put it on an element inside it",
                                a.name
                            ),
                        )),
                    }
                }
            }
            _ => out.push(states(
                body.node_span(*c),
                "a `<stream>` holds only its `<placeholder>`, `<ready>` and `<failed>`; put \
                 this inside one of them"
                    .to_string(),
            )),
        }
    }

    if !found.contains_key("ready") {
        out.push(states(
            span.clone(),
            "a `<stream>` shows what its query answered: it needs a `<ready as={..}>`".to_string(),
        ));
    }
    if !found.contains_key("failed") {
        out.push(states(
            span.clone(),
            "a `<stream>` shows that its query failed, as every query can, its budget spent \
             or its source down: it needs a `<failed>`"
                .to_string(),
        ));
    }
    let Some((def, decl)) = query else {
        return out;
    };
    let name = &decl.name;
    match (streamed(decl), found.get("placeholder")) {
        (true, None) => out.push(states(
            span.clone(),
            format!(
                "`{name}` is declared `delivery streamed`, so the page is sent before it \
                 answers: the `<stream>` needs a `<placeholder>` to show until then"
            ),
        )),
        (false, Some(p)) => out.push(states(
            body.node_span(*p),
            format!(
                "`{name}` is not declared `delivery streamed`, so the page waits for it and \
                 this `<placeholder>` is never shown; declare `delivery streamed` to send the \
                 page first"
            ),
        )),
        _ => {}
    }
    // What the failed arm is given: the query's declared error, or the
    // host's failure. A query that declares none gives it nothing.
    let declares_error = sigs
        .by_def(def)
        .and_then(|s| s.result())
        .and_then(|t| t.as_builtin())
        == Some(crate::resolved::Builtin::Result);
    if !declares_error
        && let Some(f) = found.get("failed")
        && let Node::Element { attrs, .. } = body.node(*f)
        && let Some(a) = attrs.iter().find(|a| a.name == "as")
    {
        out.push(states(
            a.span.clone(),
            format!(
                "`{name}` declares no error, so its failure is the host's and has no value to \
                 bind; write `<failed>`"
            ),
        ));
    }
    out
}

fn diagnostic(code: Code, reason: &'static str, message: String, at: Span) -> Diagnostic {
    Diagnostic {
        code: code.id,
        invariant: code.invariant,
        reason,
        detector: Detector::DeclarationRule,
        severity: Severity::Error,
        message,
        primary_span: at,
        related: Vec::new(),
        explanation: None,
        repairs: Vec::new(),
    }
}

fn waited_for(q: &Decl, at: Span) -> Diagnostic {
    let name = &q.name;
    let mut d = diagnostic(
        crate::codes::STREAMED_READ_OUTSIDE_STREAM,
        "streamed_read_outside_stream",
        format!(
            "`{name}` is declared `delivery streamed`, so a page does not wait for it; read it \
             in a `<stream query={{{name}(..)}}>`"
        ),
        at,
    );
    d.explanation = Some(
        "A query declared `delivery streamed` is slow enough that its page is sent before it \
         answers (ADR-0148). A `let` makes the page wait for its value, which is what the \
         declaration says the page must not do. A `<stream>` shows a placeholder while it is \
         pending, and the server sends what it settles to in the same response."
            .to_string(),
    );
    d.repairs.push(Repair {
        description: format!(
            "show it with `<stream query={{{name}(..)}}>`, holding a `<placeholder>`, a \
             `<ready as={{..}}>` and a `<failed>`"
        ),
        replacement: None,
    });
    d
}

fn states(at: Span, message: String) -> Diagnostic {
    let mut d = diagnostic(crate::codes::STREAM_STATES, "stream_states", message, at);
    d.explanation = Some(
        "A `<stream>` shows its query's state: `<placeholder>` while a query declared \
         `delivery streamed` is pending, `<ready as={v}>` with what it answered, and \
         `<failed as={why}>` when it did not, given `Some(e)` for its declared error and \
         `None` for the host's failure (ADR-0148). An arm missing is a state the region shows \
         nothing for; an arm for a state the query cannot be in is markup no one sees."
            .to_string(),
    );
    d
}

fn unbounded(q: &Decl, at: Span) -> Diagnostic {
    let name = &q.name;
    let mut d = diagnostic(
        crate::codes::STREAMED_WITHOUT_TIMEOUT,
        "streamed_without_timeout",
        format!(
            "`{name}` is declared `delivery streamed` and no `timeout`: a region waiting on it \
             would wait as long as its source does"
        ),
        at,
    );
    d.explanation = Some(
        "The response that carries a streamed region stays open until the region's query \
         settles (ADR-0148). A `timeout` is how long that may be: when it is spent, the region \
         shows its `<failed>` arm and the response ends."
            .to_string(),
    );
    d.repairs.push(Repair {
        description: "declare how long it may take: `timeout 3.seconds`".to_string(),
        replacement: None,
    });
    d
}
