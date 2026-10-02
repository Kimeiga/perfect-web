//! **Where UI state may be read and written** (ADR-0130).
//!
//! A signal (`signal open: Bool = false`) lives in the browser and changes
//! when a person acts. Three rules follow, each a question about where an
//! expression runs:
//!
//! - **PW5300.** A signal is assigned only in a handler. A render's effect
//!   row is empty (charter §7.4), and the body of a page runs on the server,
//!   where no person has acted yet.
//! - **PW5301.** A signal is read only where the browser reads it again when
//!   it changes: a template part, or a handler. A read in the page's body is
//!   the server reading the first value once; nothing would read it again.
//! - **PW5302.** A handler assigns a signal, not a binding of the body it is
//!   written in. A body's `let mut` changed by a press is UI state written the
//!   other way, and there is one way (ADR-0130's fourth ruling). A binding the
//!   handler declares itself is its own local, and stays legal.
//!
//! Where an expression runs is decided from the template: an `on:` attribute's
//! value is a handler, every other template part is read by the renderer, and
//! everything else is the body.

use std::collections::{BTreeMap, BTreeSet};

use crate::diagnostics::{Detector, Diagnostic, Related, Repair, Severity};
use crate::hir::{AttrValue, BinOp, Body, Decl, Expr, ExprId, Hir, Node, Pattern};
use crate::lexical::{Binder, Lexical};
use crate::signatures::Signatures;

/// Where an expression runs.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Place {
    /// In a handler, in the browser, after a person acted.
    Handler(ExprId),
    /// In a template part, read by the renderer and again when what it reads
    /// changes.
    Part,
    /// In the body, once, on the server.
    Body,
}

/// The three rules over every declaration of one unit.
pub fn check(hir: &Hir, sigs: &Signatures, at: usize, out: &mut Vec<Diagnostic>) {
    for (_, decl) in hir.all_decls() {
        let Some(b) = decl.body else { continue };
        let body = hir.body(b);
        check_body(decl, body, sigs, at, out);
    }
}

fn check_body(decl: &Decl, body: &Body, sigs: &Signatures, at: usize, out: &mut Vec<Diagnostic>) {
    let handlers = handlers(body);
    if body.signals.is_empty() && handlers.is_empty() {
        return;
    }
    let places = places(body, &handlers);
    let lexical = Lexical::build(sigs, Some(at), decl, body);

    // Each signal's binder, and each `let`'s, by its pattern: where it was
    // declared decides whose binding it is.
    let mut signals: BTreeMap<Binder, (String, ExprId)> = BTreeMap::new();
    let mut lets: BTreeMap<Binder, ExprId> = BTreeMap::new();
    for id in body.walk() {
        let Expr::Let { pat: Some(p), .. } = body.expr(id) else {
            continue;
        };
        lets.insert(Binder::Pattern(*p), id);
        if body.signals.contains(&id)
            && let Pattern::Bind { name, .. } = body.pat(*p)
        {
            signals.insert(Binder::Pattern(*p), (name.clone(), id));
        }
    }

    for id in body.walk() {
        match body.expr(id) {
            Expr::Binary {
                op: BinOp::Assign,
                lhs,
                ..
            } => {
                let mut target = *lhs;
                while let Expr::Field { base, .. } = body.expr(target) {
                    target = *base;
                }
                let Some(binder) = lexical.binder(target) else {
                    continue;
                };
                let place = places.get(&id).copied().unwrap_or(Place::Body);
                if let Some((name, declared)) = signals.get(&binder) {
                    if !matches!(place, Place::Handler(_)) {
                        out.push(written_outside(body, id, name, *declared));
                    }
                    continue;
                }
                // A binding of the body, changed by a handler.
                if let Place::Handler(handler) = place
                    && let Some(declared) = lets.get(&binder)
                    && !within(body, handler, *declared)
                    && let Expr::Name(name) = body.expr(target)
                {
                    out.push(not_a_signal(body, id, name, *declared));
                }
            }
            Expr::Name(n) => {
                let Some(binder) = lexical.binder(id) else {
                    continue;
                };
                let Some((_, declared)) = signals.get(&binder) else {
                    continue;
                };
                // The assignment's own target is a write, not a read.
                if is_assigned(body, id) {
                    continue;
                }
                if places.get(&id).copied().unwrap_or(Place::Body) == Place::Body {
                    out.push(read_where_it_cannot_change(body, id, n, *declared));
                }
            }
            _ => {}
        }
    }
}

/// Each handler: an `on:` attribute's value.
fn handlers(body: &Body) -> BTreeSet<ExprId> {
    let mut out = BTreeSet::new();
    for id in body.walk() {
        let Expr::Template { roots, .. } = body.expr(id) else {
            continue;
        };
        for n in body.walk_markup(roots) {
            let Node::Element { attrs, .. } = body.node(n) else {
                continue;
            };
            for a in attrs {
                if let (Some(("on", _)), AttrValue::Expr(e)) = (a.namespace(), &a.value) {
                    out.insert(*e);
                }
            }
        }
    }
    out
}

/// Where each expression runs: under a handler, under another template part,
/// or in the body.
fn places(body: &Body, handlers: &BTreeSet<ExprId>) -> BTreeMap<ExprId, Place> {
    let mut out = BTreeMap::new();
    for id in body.walk() {
        let Expr::Template { parts, .. } = body.expr(id) else {
            continue;
        };
        for part in parts {
            let place = if handlers.contains(part) {
                Place::Handler(*part)
            } else {
                Place::Part
            };
            for e in body.walk_from(*part) {
                out.insert(e, place);
            }
        }
    }
    out
}

/// Is `inner` inside the expression `outer`?
fn within(body: &Body, outer: ExprId, inner: ExprId) -> bool {
    body.walk_from(outer).contains(&inner)
}

/// Is `name` the target of an assignment?
fn is_assigned(body: &Body, name: ExprId) -> bool {
    body.walk().into_iter().any(|id| match body.expr(id) {
        Expr::Binary {
            op: BinOp::Assign,
            lhs,
            ..
        } => {
            let mut target = *lhs;
            while let Expr::Field { base, .. } = body.expr(target) {
                target = *base;
            }
            target == name
        }
        _ => false,
    })
}

fn written_outside(body: &Body, at: ExprId, name: &str, declared: ExprId) -> Diagnostic {
    let code = crate::codes::SIGNAL_WRITTEN_OUTSIDE_HANDLER;
    Diagnostic {
        code: code.id,
        invariant: code.invariant,
        reason: "signal_written_outside_handler",
        detector: Detector::DeclarationRule,
        severity: Severity::Error,
        message: format!("`{name}` is a signal, and it is changed outside a handler"),
        primary_span: body.expr_span(at),
        related: vec![Related {
            span: body.expr_span(declared),
            label: format!("`{name}` is declared a signal here"),
        }],
        explanation: Some(
            "A signal is UI state: it changes when a person acts, in the browser. \
             Outside a handler this runs on the server while the page is made, or \
             while it renders, and a render changes nothing (charter §7.4)."
                .to_string(),
        ),
        repairs: vec![Repair {
            description: "change it in an `on:` handler, or give the signal this \
                          value as its first one"
                .to_string(),
            replacement: None,
        }],
    }
}

fn read_where_it_cannot_change(
    body: &Body,
    at: ExprId,
    name: &str,
    declared: ExprId,
) -> Diagnostic {
    let code = crate::codes::SIGNAL_READ_WHERE_IT_CANNOT_CHANGE;
    Diagnostic {
        code: code.id,
        invariant: code.invariant,
        reason: "signal_read_where_it_cannot_change",
        detector: Detector::DeclarationRule,
        severity: Severity::Error,
        message: format!(
            "`{name}` is a signal, and this reads it once, on the server, where it \
             never changes"
        ),
        primary_span: body.expr_span(at),
        related: vec![Related {
            span: body.expr_span(declared),
            label: format!("`{name}` is declared a signal here"),
        }],
        explanation: Some(
            "The page's body runs on the server, before anyone has pressed \
             anything, so it would read the signal's first value and nothing \
             would read it again. A template part and a handler run in the \
             browser, where the signal changes."
                .to_string(),
        ),
        repairs: vec![Repair {
            description: "read the signal in the template, or in a handler".to_string(),
            replacement: None,
        }],
    }
}

fn not_a_signal(body: &Body, at: ExprId, name: &str, declared: ExprId) -> Diagnostic {
    let code = crate::codes::UI_STATE_NOT_A_SIGNAL;
    Diagnostic {
        code: code.id,
        invariant: code.invariant,
        reason: "ui_state_not_a_signal",
        detector: Detector::DeclarationRule,
        severity: Severity::Error,
        message: format!("this handler changes `{name}`, a binding of the body it is written in"),
        primary_span: body.expr_span(at),
        related: vec![Related {
            span: body.expr_span(declared),
            label: format!("`{name}` is declared here, outside the handler"),
        }],
        explanation: Some(
            "A value a handler changes is UI state, and UI state is a signal: \
             declared with its type, read where the browser reads it again, and \
             nothing else (ADR-0130). A binding the handler declares itself is \
             its own, and may change."
                .to_string(),
        ),
        repairs: vec![Repair {
            description: format!("declare it `signal {name}: Type = ..`"),
            replacement: None,
        }],
    }
}
