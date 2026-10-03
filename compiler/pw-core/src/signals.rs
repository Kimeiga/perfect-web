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

/// The three rules over every declaration of one unit. `captured` is which
/// parameters each view's handlers capture (ADR-0136).
pub fn check(
    hir: &Hir,
    sigs: &Signatures,
    at: usize,
    captured: &BTreeMap<crate::resolve::DefId, BTreeSet<String>>,
    out: &mut Vec<Diagnostic>,
) {
    for (_, decl) in hir.all_decls() {
        let Some(b) = decl.body else { continue };
        let body = hir.body(b);
        check_body(decl, body, sigs, at, captured, out);
    }
}

fn check_body(
    decl: &Decl,
    body: &Body,
    sigs: &Signatures,
    at: usize,
    captured: &BTreeMap<crate::resolve::DefId, BTreeSet<String>>,
    out: &mut Vec<Diagnostic>,
) {
    let handlers = handlers(body);
    // A `<dialog>` is the dialog rule's, signals or none (ADR-0141).
    let dialog = body.walk().into_iter().any(|id| match body.expr(id) {
        Expr::Template { roots, .. } => body
            .walk_markup(roots)
            .into_iter()
            .any(|n| matches!(body.node(n), Node::Element { tag, .. } if tag == "dialog")),
        _ => false,
    });
    if body.signals.is_empty() && handlers.is_empty() && !dialog {
        return;
    }
    let places = places(body, &handlers);
    // Each prop a composed view's handler captures (ADR-0136). A handler
    // reaches a signal through its context, and reads it as it is when the
    // handler runs; one given a signal as a prop would capture it as it was
    // rendered, and read that value however the signal changed after.
    let given: Vec<(ExprId, String, String)> = if body.signals.is_empty() {
        Vec::new()
    } else {
        crate::resume::captured_props(body, sigs.workspace(), at, captured)
    };
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

    // A `<dialog>` a signal shows (ADR-0141).
    dialogs(body, &lexical, &signals, out);
    // What an input binds its value to (ADR-0142).
    bindings(body, &lexical, &signals, out);

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
                // A binding of the body, changed by a handler. One `bind:value`
                // wrote is PW5304's to say (ADR-0142).
                if let Place::Handler(handler) = place
                    && !body.bound.contains(&handler)
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
                } else if let Some((_, tag, param)) =
                    given.iter().find(|(e, ..)| within(body, *e, id))
                {
                    out.push(captured_through_a_view(body, id, n, *declared, tag, param));
                }
            }
            _ => {}
        }
    }
}

/// **A modal dialog is shown by a signal's block, and says what closing it
/// does** (ADR-0141). A `<dialog>` written without `open` is the browser's
/// modal dialog while a block a signal decides renders it. Escape closes it
/// by itself, and the standard lets a page stop that only sometimes, so it
/// handles `close`, and the signal that shows it hears of it.
fn dialogs(
    body: &Body,
    lexical: &Lexical,
    signals: &BTreeMap<Binder, (String, ExprId)>,
    out: &mut Vec<Diagnostic>,
) {
    // The signal a subject is read from: `open` in `{#if open}`.
    let signal_of = |e: ExprId| {
        let mut root = e;
        while let Expr::Field { base, .. } = body.expr(root) {
            root = *base;
        }
        lexical
            .binder(root)
            .and_then(|b| signals.get(&b))
            .map(|(name, _)| name.clone())
    };
    fn walk(
        body: &Body,
        lexical: &Lexical,
        signals: &BTreeMap<Binder, (String, ExprId)>,
        signal_of: &dyn Fn(ExprId) -> Option<String>,
        n: crate::hir::NodeId,
        shown_by: Option<&str>,
        out: &mut Vec<Diagnostic>,
    ) {
        match body.node(n) {
            Node::Element {
                tag,
                attrs,
                children,
                ..
            } => {
                if tag == "dialog" && !attrs.iter().any(|a| a.name == "open") {
                    let closes = attrs
                        .iter()
                        .any(|a| a.event().is_some_and(|(e, _)| e == "close"));
                    match shown_by {
                        None => out.push(dialog_shown_by_nothing(body, n)),
                        Some(signal) if !closes => {
                            out.push(dialog_close_unheard(body, n, signal));
                        }
                        Some(_) => {}
                    }
                }
                for c in children {
                    walk(body, lexical, signals, signal_of, *c, shown_by, out);
                }
            }
            Node::Block {
                subject, children, ..
            } => {
                let each = lexical
                    .each_blocks()
                    .find(|(node, ..)| *node == n)
                    .and_then(|(_, _, head)| head)
                    .and_then(|b| signals.get(&b))
                    .map(|(name, _)| name.clone());
                let decided = subject.and_then(signal_of).or(each);
                let shown = shown_by.map(str::to_string).or(decided);
                for c in children {
                    walk(body, lexical, signals, signal_of, *c, shown.as_deref(), out);
                }
            }
            _ => {}
        }
    }
    for id in body.walk() {
        let Expr::Template { roots, .. } = body.expr(id) else {
            continue;
        };
        for r in roots {
            walk(body, lexical, signals, &signal_of, *r, None, out);
        }
    }
}

fn dialog_shown_by_nothing(body: &Body, at: crate::hir::NodeId) -> Diagnostic {
    let code = crate::codes::MODAL_DIALOG;
    Diagnostic {
        code: code.id,
        invariant: code.invariant,
        reason: "dialog_shown_by_nothing",
        detector: Detector::DeclarationRule,
        severity: Severity::Error,
        message: "this `<dialog>` is shown by nothing: a modal dialog is shown while a \
                  signal's block renders it"
            .to_string(),
        primary_span: body.node_span(at),
        related: Vec::new(),
        explanation: Some(
            "A `<dialog>` written without `open` is hidden, and nothing in a template \
             shows one but a block a signal decides, which the browser renders as its \
             modal dialog (ADR-0141)."
                .to_string(),
        ),
        repairs: vec![Repair {
            description: "render it in a block a signal decides, `{#if open}<dialog ..>..\
                          </dialog>{/if}`, or write `open` for a dialog shown in place"
                .to_string(),
            replacement: None,
        }],
    }
}

fn dialog_close_unheard(body: &Body, at: crate::hir::NodeId, signal: &str) -> Diagnostic {
    let code = crate::codes::MODAL_DIALOG;
    Diagnostic {
        code: code.id,
        invariant: code.invariant,
        reason: "dialog_close_unheard",
        detector: Detector::DeclarationRule,
        severity: Severity::Error,
        message: format!(
            "this `<dialog>` is shown while `{signal}` renders it, and its closing is \
             heard by nothing"
        ),
        primary_span: body.node_span(at),
        related: Vec::new(),
        explanation: Some(format!(
            "Escape closes a modal dialog by itself, and the HTML standard lets a page \
             stop that only sometimes. Without a `close` handler, `{signal}` would still \
             say the dialog is shown after it closed, and the control that opens it could \
             not open it again."
        )),
        repairs: vec![Repair {
            description: format!("handle its closing: `on:close={{() => {signal} = ..}}`"),
            replacement: None,
        }],
    }
}

/// **An input binds its value to a signal of `String`, by its name**
/// (ADR-0142). `bind:value={s}` was lowered to `value={s}` and a handler
/// setting `s`; this reads each such handler, and each `bind:` the lowering
/// left as written.
fn bindings(
    body: &Body,
    lexical: &Lexical,
    signals: &BTreeMap<Binder, (String, ExprId)>,
    out: &mut Vec<Diagnostic>,
) {
    let mut roots = Vec::new();
    for id in body.walk() {
        if let Expr::Template { roots: r, .. } = body.expr(id) {
            roots.extend(r.iter().copied());
        }
    }
    for n in body.walk_markup(&roots) {
        let Node::Element { tag, attrs, .. } = body.node(n) else {
            continue;
        };
        for a in attrs {
            // A `bind:` the lowering left: not `value`, or not a name.
            if let Some(("bind", what)) = a.namespace() {
                out.push(bound_value(
                    a.span.clone(),
                    format!("`bind:{what}` binds `value`, to a signal's name: `bind:value={{s}}`"),
                ));
                continue;
            }
            let AttrValue::Expr(handler) = &a.value else {
                continue;
            };
            if !body.bound.contains(handler) {
                continue;
            }
            if !matches!(tag.as_str(), "input" | "textarea" | "select") {
                out.push(bound_value(
                    a.span.clone(),
                    format!(
                        "`<{tag}>` has no value a person types: `bind:value` binds an \
                         `<input>`, a `<textarea>` or a `<select>`"
                    ),
                ));
                continue;
            }
            // The handler sets the bound name: `s = event.value`.
            let Expr::Lambda { body: set, .. } = body.expr(*handler) else {
                continue;
            };
            let Expr::Binary { lhs, .. } = body.expr(*set) else {
                continue;
            };
            let Expr::Name(name) = body.expr(*lhs) else {
                continue;
            };
            let signal = lexical.binder(*lhs).and_then(|b| signals.get(&b));
            let Some((_, declared)) = signal else {
                out.push(bound_value(
                    a.span.clone(),
                    format!(
                        "`bind:value` binds `{name}`, which is not a signal: what is typed \
                         would change nothing the browser holds"
                    ),
                ));
                continue;
            };
            // As written: `String`, `Int`, `List<String>`.
            let written = match body.expr(*declared) {
                Expr::Let { ty: Some(t), .. } => body
                    .types
                    .get(t.index())
                    .map(|r| match r.args.is_empty() {
                        true => r.path.clone(),
                        false => format!("{}<..>", r.path),
                    })
                    .unwrap_or_default(),
                _ => String::new(),
            };
            if written != "String" {
                out.push(bound_value(
                    a.span.clone(),
                    format!(
                        "`bind:value` binds `{name}`, a signal of `{written}`: a signal of \
                         another type than `String` needs a codec, which is not built \
                         (ADR-0131)"
                    ),
                ));
            }
        }
    }
}

fn bound_value(at: crate::hir::Span, message: String) -> Diagnostic {
    let code = crate::codes::BOUND_VALUE;
    Diagnostic {
        code: code.id,
        invariant: code.invariant,
        reason: "bound_value",
        detector: Detector::DeclarationRule,
        severity: Severity::Error,
        message,
        primary_span: at,
        related: Vec::new(),
        explanation: Some(
            "`bind:value={s}` shows the signal `s` in a field and sets it to what is typed \
             (ADR-0142). The browser holds a page's signals, so a value the server holds \
             would not change, and text becomes another type only through a codec, a \
             parse and a format back (ADR-0131)."
                .to_string(),
        ),
        repairs: vec![Repair {
            description: "declare `signal s: String = \"\"` and write `bind:value={s}`".to_string(),
            replacement: None,
        }],
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

/// A signal given to a view whose handler captures it (ADR-0136).
fn captured_through_a_view(
    body: &Body,
    at: ExprId,
    name: &str,
    declared: ExprId,
    tag: &str,
    param: &str,
) -> Diagnostic {
    let code = crate::codes::SIGNAL_READ_WHERE_IT_CANNOT_CHANGE;
    Diagnostic {
        code: code.id,
        invariant: code.invariant,
        reason: "signal_captured_through_a_view",
        detector: Detector::DeclarationRule,
        severity: Severity::Error,
        message: format!(
            "`{name}` is a signal, and `<{tag}>`'s handler captures it as `{param}`, \
             as it was rendered"
        ),
        primary_span: body.expr_span(at),
        related: vec![Related {
            span: body.expr_span(declared),
            label: format!("`{name}` is declared a signal here"),
        }],
        explanation: Some(format!(
            "A handler reads a signal through its context, as it is when the handler \
             runs. A view's handler that captures `{param}` reads the value the \
             document was rendered with, and pressing it after `{name}` changed would \
             act on the old value. A view is given a signal to change or to read in a \
             handler when signals are provided to views (ADR-0130, step 3)."
        )),
        repairs: vec![Repair {
            description: format!(
                "write `<{tag}>`'s handler in this page, where it reads `{name}` itself"
            ),
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
