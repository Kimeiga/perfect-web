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
//!   it changes: a template part, a handler, or a page query's key. A read in
//!   the page's body is the server reading the first value once; nothing would
//!   read it again. A page's `let found = query Search(id, term)` is read
//!   again, for the new key, when `term` changes (ADR-0152), so it holds the
//!   two rules of a key: PW5308 and PW5309.
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
use crate::hir::{
    AttrValue, BinOp, Body, Decl, DeclKind, Expr, ExprId, Hir, Node, NodeId, Pattern,
};
use crate::lexical::{Binder, Lexical};
use crate::resolve::{DefId, Resolution};
use crate::resolved::Primitive;
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

/// The rules over every declaration of one unit. `captured` is which
/// parameters each view's handlers capture (ADR-0136), and `provision` what
/// each view needs provided and which views hold a signal (ADR-0144).
pub fn check(
    hir: &Hir,
    sigs: &Signatures,
    at: usize,
    captured: &BTreeMap<crate::resolve::DefId, BTreeSet<String>>,
    provision: &Provision,
    out: &mut Vec<Diagnostic>,
) {
    for (_, decl) in hir.all_decls() {
        let Some(b) = decl.body else { continue };
        let body = hir.body(b);
        provided(decl, body, sigs, at, provision, out);
        check_body(decl, body, sigs, at, captured, provision, out);
    }
}

fn check_body(
    decl: &Decl,
    body: &Body,
    sigs: &Signatures,
    at: usize,
    captured: &BTreeMap<crate::resolve::DefId, BTreeSet<String>>,
    provision: &Provision,
    out: &mut Vec<Diagnostic>,
) {
    let lexical = Lexical::build(decl, body);
    // The module signals it names, provided to it (ADR-0144).
    let named = module_signals_named(sigs, at, &lexical, body);
    // Where a form control's value is written (ADR-0221), signals or none.
    form_controls(body, &lexical, &named, out);
    let handlers = handlers(body);
    // A `<dialog>` is the dialog rule's, signals or none (ADR-0141).
    let dialog = body.walk().into_iter().any(|id| match body.expr(id) {
        Expr::Template { roots, .. } => body
            .walk_markup(roots)
            .into_iter()
            .any(|n| matches!(body.node(n), Node::Element { tag, .. } if tag == "dialog")),
        _ => false,
    });
    if body.signals.is_empty() && named.is_empty() && handlers.is_empty() && !dialog {
        return;
    }
    let places = places(body, &handlers);
    // Each prop a composed view's handler captures (ADR-0136). A handler
    // reaches a signal through its context, and reads it as it is when the
    // handler runs; one given a signal as a prop would capture it as it was
    // rendered, and read that value however the signal changed after.
    let given: Vec<(ExprId, String, String)> = if body.signals.is_empty() && named.is_empty() {
        Vec::new()
    } else {
        crate::resume::captured_props(body, sigs.workspace(), at, captured)
    };

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

    // **Each signal a page's query is given** (ADR-0152): the argument, the
    // binding, and the query. The browser reads the binding again, for the
    // new key, when the signal changes.
    let mut keys: BTreeMap<ExprId, (String, DefId)> = BTreeMap::new();
    // A layout's too (ADR-0303): its bindings are its pages'.
    if matches!(decl.kind, DeclKind::Page | DeclKind::Layout) {
        for (binding, query, args) in crate::page_values::query_bindings(sigs.workspace(), at, body)
        {
            for arg in args {
                if matches!(body.expr(arg), Expr::Name(_))
                    && lexical
                        .binder(arg)
                        .is_some_and(|b| signals.contains_key(&b))
                {
                    keys.insert(arg, (binding.clone(), query));
                }
            }
        }
    }

    // A module's signal, by where it is named (ADR-0144).
    let module = |e: ExprId| named.iter().find(|(_, at)| *at == e).map(|(def, _)| *def);
    // A `<dialog>` a signal shows (ADR-0141).
    dialogs(body, &lexical, &signals, &|e| module(e).is_some(), out);
    // What an input binds its value to (ADR-0142).
    bindings(body, &lexical, &signals, sigs, &module, out);

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
                let place = places.get(&id).copied().unwrap_or(Place::Body);
                let Some(binder) = lexical.binder(target) else {
                    // A module's signal: changed by a handler, and given its
                    // value by a `provide`, which is PW5306's (ADR-0144).
                    if !body.provides.contains(&id)
                        && module(target).is_some()
                        && !matches!(place, Place::Handler(_))
                        && let Expr::Name(name) = body.expr(target)
                    {
                        out.push(written_outside(body, id, name, None));
                    }
                    continue;
                };
                if let Some((name, declared)) = signals.get(&binder) {
                    if !matches!(place, Place::Handler(_)) {
                        out.push(written_outside(body, id, name, Some(*declared)));
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
                // A signal this body declares, or a module's (ADR-0144).
                let declared = match lexical.binder(id) {
                    Some(binder) => match signals.get(&binder) {
                        Some((_, declared)) => Some(*declared),
                        None => continue,
                    },
                    None if module(id).is_some() => None,
                    None => continue,
                };
                // The assignment's own target is a write, not a read.
                if is_assigned(body, id) {
                    continue;
                }
                if let Some((binding, query)) = keys.get(&id) {
                    key_rules(body, id, n, declared, binding, *query, provision, sigs, out);
                } else if places.get(&id).copied().unwrap_or(Place::Body) == Place::Body {
                    out.push(read_where_it_cannot_change(body, id, n, declared));
                } else if let Some((_, tag, param)) =
                    given.iter().find(|(e, ..)| within(body, *e, id))
                {
                    out.push(captured_through_a_view(body, id, n, declared, tag, param));
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
    module: &dyn Fn(ExprId) -> bool,
    out: &mut Vec<Diagnostic>,
) {
    // The signal a subject is read from: `open` in `{#if open}`, the body's
    // own or a module's (ADR-0144).
    let signal_of = |e: ExprId| {
        let mut root = e;
        while let Expr::Field { base, .. } = body.expr(root) {
            root = *base;
        }
        match lexical.binder(root) {
            Some(b) => signals.get(&b).map(|(name, _)| name.clone()),
            None => match body.expr(root) {
                Expr::Name(n) if module(root) => Some(n.clone()),
                _ => None,
            },
        }
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
    sigs: &Signatures,
    module: &dyn Fn(ExprId) -> Option<crate::resolve::DefId>,
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
            // A `<select>`'s is the option it marks, which is PW5036's to
            // say (ADR-0221).
            if !matches!(tag.as_str(), "input" | "textarea" | "select") {
                out.push(bound_value(
                    a.span.clone(),
                    format!(
                        "`<{tag}>` has no value a person types: `bind:value` binds an \
                         `<input>` or a `<textarea>`"
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
            // A module's signal, declared `String` where it is declared
            // (ADR-0144).
            if lexical.binder(*lhs).is_none()
                && let Some(def) = module(*lhs)
            {
                let declared = sigs.signal_type(def).and_then(|t| t.resolved());
                if !declared.is_some_and(|t| t.as_primitive() == Some(Primitive::Str)) {
                    let written = declared.map(|t| t.display_name()).unwrap_or_default();
                    out.push(bound_value(
                        a.span.clone(),
                        format!(
                            "`bind:value` binds `{name}`, a signal of `{written}`: a signal \
                             of another type than `String` needs a codec, which is not built \
                             (ADR-0131)"
                        ),
                    ));
                }
                continue;
            }
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

/// **A form control's value is written where HTML reads it** (ADR-0221,
/// PW5036). A `<textarea>` reads its value from its text, so its `value` is
/// written there: a signal, which the browser sets in place, or text, and
/// never both a value and text. A `<select>` reads its value from the
/// `<option>` it marks `selected`, so a `value` on it is refused, `bind:value`
/// too, until a page computes which option that is. Until ADR-0221 each was
/// written as an attribute HTML ignores.
fn form_controls(
    body: &Body,
    lexical: &Lexical,
    named: &[(DefId, ExprId)],
    out: &mut Vec<Diagnostic>,
) {
    let mut roots = Vec::new();
    for id in body.walk() {
        if let Expr::Template { roots: r, .. } = body.expr(id) {
            roots.extend(r.iter().copied());
        }
    }
    // The body's signals by their binders, and a module's by where they are
    // named.
    let declared: BTreeSet<Binder> = body
        .walk()
        .into_iter()
        .filter_map(|id| match body.expr(id) {
            Expr::Let { pat: Some(p), .. } if body.signals.contains(&id) => {
                Some(Binder::Pattern(*p))
            }
            _ => None,
        })
        .collect();
    let is_signal = |e: ExprId| {
        lexical.binder(e).is_some_and(|b| declared.contains(&b))
            || named.iter().any(|(_, at)| *at == e)
    };
    for n in body.walk_markup(&roots) {
        let Node::Element {
            tag,
            attrs,
            children,
            ..
        } = body.node(n)
        else {
            continue;
        };
        let control = tag.to_ascii_lowercase();
        let value = attrs
            .iter()
            .find(|a| a.name.eq_ignore_ascii_case("value") && a.event().is_none());
        // `bind:value` wrote a handler beside the value (ADR-0142).
        let bound = attrs
            .iter()
            .any(|a| matches!(&a.value, AttrValue::Expr(h) if body.bound.contains(h)));
        let written = if bound { "`bind:value`" } else { "`value`" };
        let message = match (control.as_str(), value) {
            ("select", Some(_)) => Some(format!(
                "{written} on a `<select>` is written as an attribute HTML does not read: a \
                 select's value is the `<option>` it marks `selected`"
            )),
            ("textarea", Some(_)) if !children.is_empty() => Some(
                "a `<textarea>` has a value and text, and its text is its value: write one"
                    .to_string(),
            ),
            ("textarea", Some(a)) => match &a.value {
                AttrValue::Expr(e) if matches!(body.expr(*e), Expr::Name(_)) && is_signal(*e) => {
                    None
                }
                AttrValue::Expr(_) => Some(format!(
                    "{written} on a `<textarea>` is a value no change sets again: a textarea's \
                     value is a signal, which the browser sets in place, or text"
                )),
                AttrValue::Static(_) | AttrValue::None => None,
            },
            ("textarea", None)
                if children
                    .iter()
                    .any(|c| !matches!(body.node(*c), Node::Text(_))) =>
            {
                Some(
                    "a `<textarea>`'s text holds a value, which would be written between \
                     markers it shows as text: its value is `value={s}`, a signal"
                        .to_string(),
                )
            }
            _ => None,
        };
        let Some(message) = message else { continue };
        // Where it is written, and its boundary (charter §16.3): the element
        // it is written on, or the text a value meets.
        let element = body.node_span(n);
        let (primary, related) = match value {
            Some(a) if control == "textarea" && !children.is_empty() => (
                a.span.clone(),
                Related {
                    span: body.node_span(children[0]),
                    label: "its text".to_string(),
                },
            ),
            Some(a) => (
                a.span.clone(),
                Related {
                    span: element,
                    label: format!("the `<{control}>` it is written on"),
                },
            ),
            None => (
                children
                    .iter()
                    .find(|c| !matches!(body.node(**c), Node::Text(_)))
                    .map(|c| body.node_span(*c))
                    .unwrap_or_else(|| element.clone()),
                Related {
                    span: element,
                    label: "the `<textarea>` whose text it is in".to_string(),
                },
            ),
        };
        let code = crate::codes::FORM_CONTROL_VALUE;
        out.push(Diagnostic {
            code: code.id,
            invariant: code.invariant,
            reason: "form_control_value",
            detector: Detector::DeclarationRule,
            severity: Severity::Error,
            message,
            primary_span: primary,
            related: vec![related],
            explanation: Some(
                "HTML reads a `<textarea>`'s value from its text and a `<select>`'s from the \
                 `<option>` it marks `selected`; neither has a `value` attribute. A value                  written as one shows nothing until a script sets it, and nothing with                  scripts off. A textarea's value is written as its text, from a signal the                  browser sets in place as it is typed, or as text."
                    .to_string(),
            ),
            repairs: vec![Repair {
                description: match control.as_str() {
                    "select" => "mark the chosen `<option>` with `selected={..}`".to_string(),
                    _ => "write `bind:value={s}`, `s` a signal, or the text alone".to_string(),
                },
                replacement: None,
            }],
        });
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

/// Where a signal is declared, when it is in the same body: a module's is
/// declared in its module (ADR-0144).
fn declared_here(body: &Body, name: &str, declared: Option<ExprId>) -> Vec<Related> {
    declared
        .map(|d| Related {
            span: body.expr_span(d),
            label: format!("`{name}` is declared a signal here"),
        })
        .into_iter()
        .collect()
}

fn written_outside(body: &Body, at: ExprId, name: &str, declared: Option<ExprId>) -> Diagnostic {
    let code = crate::codes::SIGNAL_WRITTEN_OUTSIDE_HANDLER;
    Diagnostic {
        code: code.id,
        invariant: code.invariant,
        reason: "signal_written_outside_handler",
        detector: Detector::DeclarationRule,
        severity: Severity::Error,
        message: format!("`{name}` is a signal, and it is changed outside a handler"),
        primary_span: body.expr_span(at),
        related: declared_here(body, name, declared),
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
    declared: Option<ExprId>,
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
        related: declared_here(body, name, declared),
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

/// **The two rules of a key a signal gives a query** (ADR-0152). PW5308: the
/// signal is a `String`, an `Int` or a `Bool`, since a key crosses to the
/// server and is compared exactly. PW5309: the query says what its stale work
/// does when the key changes while the old key's read is in flight.
#[allow(clippy::too_many_arguments)]
fn key_rules(
    body: &Body,
    at: ExprId,
    name: &str,
    declared: Option<ExprId>,
    binding: &str,
    query: DefId,
    provision: &Provision,
    sigs: &Signatures,
    out: &mut Vec<Diagnostic>,
) {
    // As written where it is declared: `String`, `Int`, `List<String>`.
    let written = declared
        .and_then(|d| match body.expr(d) {
            Expr::Let { ty: Some(t), .. } => {
                body.types.get(t.index()).map(|r| match r.args.is_empty() {
                    true => r.path.clone(),
                    false => format!("{}<..>", r.path),
                })
            }
            _ => None,
        })
        .unwrap_or_default();
    if !matches!(written.as_str(), "String" | "Int" | "Bool") {
        let code = crate::codes::SIGNAL_KEY_TYPE;
        out.push(Diagnostic {
            code: code.id,
            invariant: code.invariant,
            reason: "signal_key_type",
            detector: Detector::DeclarationRule,
            severity: Severity::Error,
            message: format!(
                "`{name}` keys `{binding}`, and is a signal of `{written}`: a key is a \
                 `String`, an `Int` or a `Bool`"
            ),
            primary_span: body.expr_span(at),
            related: declared_here(body, name, declared),
            explanation: Some(
                "The browser reads a query a signal keys again when the signal changes, \
                 and sends the new key to the server, which compares it with the keys it \
                 holds. A `String`, an `Int` and a `Bool` cross as they are and compare \
                 exactly; a record or a list would need a codec, which is not built \
                 (ADR-0131)."
                    .to_string(),
            ),
            repairs: vec![Repair {
                description: "key the query by a signal of `String`, `Int` or `Bool`".to_string(),
                replacement: None,
            }],
        });
    }
    if provision
        .stale_work
        .get(&query)
        .is_some_and(Option::is_none)
    {
        let code = crate::codes::SIGNAL_KEY_STALE_WORK;
        let path = sigs.path_of(query).unwrap_or_default();
        out.push(Diagnostic {
            code: code.id,
            invariant: code.invariant,
            reason: "signal_key_stale_work",
            detector: Detector::DeclarationRule,
            severity: Severity::Error,
            message: format!("`{name}` keys `{binding}`, and `{path}` declares no `on_key_change`"),
            primary_span: body.expr_span(at),
            related: declared_here(body, name, declared),
            explanation: Some(
                "When the signal changes while the old key's read is in flight, that \
                 read is stale work. The query says what it does: `cancel` stops it, \
                 `supersede` lets it finish and drops its answer, and `keep` lets it \
                 finish and asks for the new key after it. In none is an old key's \
                 answer shown for the new one (ADR-0152)."
                    .to_string(),
            ),
            repairs: vec![Repair {
                description: format!(
                    "declare `on_key_change cancel | supersede | keep` on `{path}`"
                ),
                replacement: None,
            }],
        });
    }
}

/// A signal given to a view whose handler captures it (ADR-0136).
fn captured_through_a_view(
    body: &Body,
    at: ExprId,
    name: &str,
    declared: Option<ExprId>,
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
        related: declared_here(body, name, declared),
        explanation: Some(format!(
            "A handler reads a signal through its context, as it is when the handler \
             runs. A view's handler that captures `{param}` reads the value the \
             document was rendered with, and pressing it after `{name}` changed would \
             act on the old value. A view that changes or reads a signal in a handler \
             names it: its own, or one a module declares and a page provides (ADR-0144)."
        )),
        repairs: vec![
            Repair {
                description: format!(
                    "declare `{name}` in a module, `signal {name}: Type`, `provide` it in this \
                     page, and name it in `<{tag}>`"
                ),
                replacement: None,
            },
            Repair {
                description: format!(
                    "or write `<{tag}>`'s handler in this page, where it reads `{name}` itself"
                ),
                replacement: None,
            },
        ],
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

// --- provided signals (ADR-0144) ---------------------------------------------

/// **What each view needs provided, and which views hold a signal**
/// (ADR-0144), across the program. A page checks its own needs against it,
/// and every body checks the views it uses in a loop's row.
#[derive(Debug, Default)]
pub struct Provision {
    /// Each module signal a view names, or a view it composes needs, that it
    /// does not provide itself: with the views the need comes through,
    /// nearest first.
    pub needs: BTreeMap<DefId, BTreeMap<DefId, Vec<String>>>,
    /// The views that hold a signal or provide one, themselves or through a
    /// view they compose: each use of one is an instance of its own.
    pub holds: BTreeSet<DefId>,
    /// Each module signal's name, as it is declared, for a message.
    pub names: BTreeMap<DefId, String>,
    /// **Each query's `on_key_change`**, as declared, or `None` (ADR-0152): a
    /// page that keys one by a signal reads it wherever it is declared.
    pub stale_work: BTreeMap<DefId, Option<String>>,
}

impl Provision {
    pub fn of(hirs: &[&Hir], sigs: &Signatures) -> Provision {
        let mut p = Provision::default();
        for (unit, hir) in hirs.iter().enumerate() {
            for (id, decl) in hir.all_decls() {
                if decl.kind == DeclKind::Signal {
                    p.names
                        .insert(DefId { unit, decl: id.0 }, decl.name.clone());
                }
                if decl.kind == DeclKind::Query {
                    p.stale_work.insert(
                        DefId { unit, decl: id.0 },
                        decl.policy("on_key_change")
                            .map(|policy| policy.value.trim().to_string()),
                    );
                }
            }
        }
        for (unit, hir) in hirs.iter().enumerate() {
            for (id, decl) in hir.all_decls() {
                if decl.kind == DeclKind::View {
                    let def = DefId { unit, decl: id.0 };
                    needs_of(hirs, sigs, def, &mut p, &mut Vec::new());
                }
            }
        }
        p
    }
}

fn needs_of(
    hirs: &[&Hir],
    sigs: &Signatures,
    def: DefId,
    p: &mut Provision,
    within: &mut Vec<DefId>,
) -> BTreeMap<DefId, Vec<String>> {
    if let Some(found) = p.needs.get(&def) {
        return found.clone();
    }
    // A view that contains itself is refused (PW5020), and not followed.
    if within.contains(&def) {
        return BTreeMap::new();
    }
    let Some(decl) = crate::resolve::declaration(hirs, def) else {
        return BTreeMap::new();
    };
    let Some(body) = decl.body.map(|b| hirs[def.unit].body(b)) else {
        return BTreeMap::new();
    };
    within.push(def);
    let lexical = Lexical::build(decl, body);
    let gives: BTreeSet<DefId> = provided_in(sigs, def.unit, body)
        .into_iter()
        .map(|(signal, _)| signal)
        .collect();
    let mut needs: BTreeMap<DefId, Vec<String>> = BTreeMap::new();
    for (signal, _) in module_signals_named(sigs, def.unit, &lexical, body) {
        if !gives.contains(&signal) {
            needs.entry(signal).or_default();
        }
    }
    let mut holds = !body.signals.is_empty() || !gives.is_empty();
    for (_, view, tag, _) in composed(sigs, def.unit, body) {
        let inner = needs_of(hirs, sigs, view, p, within);
        holds |= p.holds.contains(&view);
        for (signal, through) in inner {
            if gives.contains(&signal) {
                continue;
            }
            needs
                .entry(signal)
                .or_insert_with(|| std::iter::once(tag.clone()).chain(through).collect());
        }
    }
    within.pop();
    if holds {
        p.holds.insert(def);
    }
    p.needs.insert(def, needs.clone());
    needs
}

/// The module signal `name` resolves to, where it is written (ADR-0144).
fn module_signal(sigs: &Signatures, unit: usize, name: &str) -> Option<DefId> {
    match sigs.workspace().resolve(unit, name) {
        Resolution::Local(d) | Resolution::Imported { def: d, .. }
            if sigs.kind_of(d) == Some(DeclKind::Signal) =>
        {
            Some(d)
        }
        _ => None,
    }
}

/// **The module signals a body names**, each where it is named, that no
/// binding of the body hides.
fn module_signals_named(
    sigs: &Signatures,
    unit: usize,
    lexical: &Lexical,
    body: &Body,
) -> Vec<(DefId, ExprId)> {
    let mut out = Vec::new();
    for (id, e, _) in body.exprs() {
        let Expr::Name(n) = e else { continue };
        if lexical.binder(id).is_some() {
            continue;
        }
        if let Some(def) = module_signal(sigs, unit, n) {
            out.push((def, id));
        }
    }
    out
}

/// What a body provides: each `provide` of a module signal, by the signal.
fn provided_in(sigs: &Signatures, unit: usize, body: &Body) -> Vec<(DefId, ExprId)> {
    body.provides
        .iter()
        .filter_map(|id| {
            let Expr::Binary { lhs, .. } = body.expr(*id) else {
                return None;
            };
            let Expr::Name(n) = body.expr(*lhs) else {
                return None;
            };
            module_signal(sigs, unit, n).map(|def| (def, *id))
        })
        .collect()
}

/// **The views a body's markup uses**: each element naming one, the view,
/// the tag as written, and whether it is inside a loop's row.
fn composed(sigs: &Signatures, unit: usize, body: &Body) -> Vec<(NodeId, DefId, String, bool)> {
    fn walk(
        sigs: &Signatures,
        unit: usize,
        body: &Body,
        n: NodeId,
        rows: u32,
        out: &mut Vec<(NodeId, DefId, String, bool)>,
    ) {
        match body.node(n) {
            Node::Element { tag, children, .. } => {
                if tag.starts_with(|c: char| c.is_ascii_uppercase())
                    && let Resolution::Local(d) | Resolution::Imported { def: d, .. } =
                        sigs.workspace().resolve(unit, tag)
                    && sigs.kind_of(d) == Some(DeclKind::View)
                {
                    out.push((n, d, tag.clone(), rows > 0));
                }
                for c in children {
                    walk(sigs, unit, body, *c, rows, out);
                }
            }
            Node::Block {
                directive,
                children,
                ..
            } => {
                let rows = rows + u32::from(directive.trim_start().starts_with("{#each"));
                for c in children {
                    walk(sigs, unit, body, *c, rows, out);
                }
            }
            _ => {}
        }
    }
    let mut out = Vec::new();
    for id in body.walk() {
        if let Expr::Template { roots, .. } = body.expr(id) {
            for r in roots {
                walk(sigs, unit, body, *r, 0, &mut out);
            }
        }
    }
    out
}

/// **The rules on provided signals** (ADR-0144), over one body:
/// - PW5306: it provides a module signal, once, and only in a page's or
///   view's body;
/// - PW5307: a view it uses in a loop's row holds no signal;
/// - PW5305: in a page, each signal it or a view it uses needs is provided.
fn provided(
    decl: &Decl,
    body: &Body,
    sigs: &Signatures,
    at: usize,
    provision: &Provision,
    out: &mut Vec<Diagnostic>,
) {
    let ui = matches!(
        decl.kind,
        DeclKind::Page | DeclKind::View | DeclKind::Layout
    );
    let mut given: BTreeMap<DefId, ExprId> = BTreeMap::new();
    for id in &body.provides {
        let Expr::Binary { lhs, .. } = body.expr(*id) else {
            continue;
        };
        let Expr::Name(name) = body.expr(*lhs) else {
            continue;
        };
        if !ui {
            out.push(provide_problem(
                body,
                *id,
                format!(
                    "`{}` is not a page or a view, and a `provide` gives `{name}` to the markup \
                     of the one it is written in",
                    decl.name
                ),
            ));
            continue;
        }
        let Some(signal) = module_signal(sigs, at, name) else {
            // A name that resolves to nothing is the name rule's (PW0020).
            if !matches!(sigs.workspace().resolve(at, name), Resolution::Unresolved) {
                out.push(provide_problem(
                    body,
                    *id,
                    format!(
                        "`{name}` is not a signal a module declares: a `provide` gives one \
                         its value, `signal {name}: Type` at module level"
                    ),
                ));
            }
            continue;
        };
        if let Some(first) = given.get(&signal) {
            let mut d = provide_problem(
                body,
                *id,
                format!("`{name}` is provided twice in `{}`", decl.name),
            );
            d.related.push(Related {
                span: body.expr_span(*first),
                label: "it is provided here first".to_string(),
            });
            out.push(d);
            continue;
        }
        given.insert(signal, *id);
    }
    if !ui {
        return;
    }

    let uses = composed(sigs, at, body);
    // A view used in a loop's row holds no signal of its own yet.
    for (node, view, tag, in_row) in &uses {
        if *in_row && provision.holds.contains(view) {
            out.push(signal_in_a_row(body, *node, tag));
        }
    }

    // A page provides what it, and every view it uses, needs; and a layout
    // what it and its views need (ADR-0303), since nothing is provided
    // around it.
    if !matches!(decl.kind, DeclKind::Page | DeclKind::Layout) {
        return;
    }
    let name = |d: DefId| {
        (
            provision.names.get(&d).cloned().unwrap_or_default(),
            sigs.path_of(d).unwrap_or_default().to_string(),
        )
    };
    let lexical = Lexical::build(decl, body);
    let mut reported: BTreeSet<DefId> = BTreeSet::new();
    for (signal, e) in module_signals_named(sigs, at, &lexical, body) {
        if given.contains_key(&signal) || !reported.insert(signal) {
            continue;
        }
        out.push(not_provided(
            body.expr_span(e),
            name(signal),
            &decl.name,
            &format!("`{}` reads it here", decl.name),
        ));
    }
    for (node, view, tag, _) in &uses {
        let Some(needs) = provision.needs.get(view) else {
            continue;
        };
        for (signal, through) in needs {
            if given.contains_key(signal) || !reported.insert(*signal) {
                continue;
            }
            let chain: String = through
                .iter()
                .map(|t| format!(", through `<{t}>`"))
                .collect();
            out.push(not_provided(
                body.node_span(*node),
                name(*signal),
                &decl.name,
                &format!("`<{tag}>` needs it{chain}"),
            ));
        }
    }
}

fn not_provided(
    at: crate::hir::Span,
    (name, path): (String, String),
    page: &str,
    why: &str,
) -> Diagnostic {
    let code = crate::codes::SIGNAL_NOT_PROVIDED;
    Diagnostic {
        code: code.id,
        invariant: code.invariant,
        reason: "signal_not_provided",
        detector: Detector::DeclarationRule,
        severity: Severity::Error,
        message: format!("nothing provides `{path}` to `{page}`, and {why}"),
        primary_span: at,
        related: Vec::new(),
        explanation: Some(
            "A signal declared in a module has no value of its own. A page or view gives it \
             one with `provide`, for everything that body contains, and a view that names it \
             reads the nearest one around it (ADR-0144). Without one, the view would read a \
             value nobody gave; a default value would hide that."
                .to_string(),
        ),
        repairs: vec![Repair {
            description: format!(
                "write `provide {name} = ..` in `{page}`, or in a view around the one that \
                 needs it"
            ),
            replacement: None,
        }],
    }
}

fn provide_problem(body: &Body, at: ExprId, message: String) -> Diagnostic {
    let code = crate::codes::PROVIDE;
    Diagnostic {
        code: code.id,
        invariant: code.invariant,
        reason: "provide",
        detector: Detector::DeclarationRule,
        severity: Severity::Error,
        message,
        primary_span: body.expr_span(at),
        related: Vec::new(),
        explanation: Some(
            "`provide name = value` gives a signal declared in a module, `signal name: Type`, \
             its value for everything the page or view it is written in contains (ADR-0144). \
             A nearer `provide` shadows it for what that one contains; two in one body would \
             leave which one holds unsaid."
                .to_string(),
        ),
        repairs: vec![Repair {
            description: "declare the signal at module level, `signal name: Type`, and provide \
                          it once, in the page or view whose markup uses it"
                .to_string(),
            replacement: None,
        }],
    }
}

fn signal_in_a_row(body: &Body, at: NodeId, tag: &str) -> Diagnostic {
    let code = crate::codes::SIGNAL_IN_A_ROW;
    Diagnostic {
        code: code.id,
        invariant: code.invariant,
        reason: "signal_in_a_row",
        detector: Detector::DeclarationRule,
        severity: Severity::Error,
        message: format!(
            "`<{tag}>` holds a signal, and a view used in a loop's row holds none yet: each row \
             would need its own"
        ),
        primary_span: body.node_span(at),
        related: Vec::new(),
        explanation: Some(
            "Each use of a view that holds a signal is an instance of its own (ADR-0144). In a \
             loop's row, each row would need one, kept by the row's key as rows are added, \
             removed and reordered, and the browser holds one instance per use."
                .to_string(),
        ),
        repairs: vec![Repair {
            description: "hold the state in a signal the page provides, keyed by the row: a \
                          `signal open: Option<ItemId>` the rows read and set"
                .to_string(),
            replacement: None,
        }],
    }
}
