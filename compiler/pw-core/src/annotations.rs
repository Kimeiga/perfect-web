//! Charter §7.1, §7.10, §8.2 — three questions about what a value *is*.
//!
//! Not a type checker. A type checker infers a type for every expression; this
//! answers three specific questions the corpus asks, each from a type the
//! author **wrote down**:
//!
//! - **R-008** uses `Option<Store>` where `Store` is expected. Absence is a
//!   value of type `Option<T>` and never an ambient null, so the only way to
//!   reach the `Store` is to match.
//! - **R-009** casts `Unknown` to `Store`. External data enters as `Unknown`
//!   and there is no cast that skips decoding — that is what makes the
//!   boundary trustworthy rather than merely typed.
//! - **R-022** hands a `PressEvent` handler to `on:submit`.
//!
//! Scoping it this way is deliberate. Inferring types for everything would
//! reach these three and a great deal else, and the "else" is where a partial
//! inference engine reports confident nonsense about programs it does not
//! understand. Every rule here fires only where an annotation exists, so a
//! program without annotations gets silence rather than guesses.
//!
//! What that costs is recorded rather than hidden: `store_name` would still be
//! accepted if the author dropped the `: Option<Store>` annotation. The
//! annotation is the specification, and E9 proper is what replaces it.

use crate::codes;
use crate::diagnostics::{Detector, Diagnostic, Related, Repair, Severity};
use crate::hir::{AttrValue, Body, Decl, Expr, Hir, Node, Span, TypeRefId};
use crate::signatures::Signatures;

/// Where an `on:` attribute's event type is declared. One name, and the only
/// module name the compiler knows.
const EVENTS_MODULE: &str = "events";

pub fn check(hir: &Hir, sigs: &Signatures, out: &mut Vec<Diagnostic>) {
    for (id, decl) in hir.all_decls() {
        let Some(body_id) = decl.body else { continue };
        let body = hir.body(body_id);
        let at = hir.decl_span(id);
        let module = module_of(hir, id);
        let types = crate::infer::Types::of_decl(sigs, hir, id, body);
        optional_used_as_present(hir, &types, body, decl, &at, out);
        unchecked_cast(hir, sigs, &types, body, decl, &at, out);
        handler_matches_event(hir, body, sigs, decl, module, &at, out);
        results_are_handled(&types, body, decl, &at, out);
    }
}

// --- ADR-0099: a failure is handled ------------------------------------------

/// **A `Result` whose value nothing uses** is a failure dropped: the first
/// `Carts.add(..)` of two, as a statement, failed and the command went on.
/// The charter's "unhandled ADT variant", one step earlier: neither case of
/// the value is handled. `?` passes the failure on, `match` handles it, and
/// a binding that says so, `let _cleared = ..`, discards it by name.
///
/// Which values are used: a statement's is not, nor a `for` body's, nor a
/// body declared `-> ()`'s last. A lambda's body is its result, used by
/// whoever calls it: a handler's command answer is the runtime's to drop
/// (KNOWN_LIMITATIONS).
fn results_are_handled(
    types: &crate::infer::Types<'_>,
    body: &Body,
    decl: &Decl,
    at: &Span,
    out: &mut Vec<Diagnostic>,
) {
    let returns_unit = decl
        .ret
        .as_ref()
        .is_none_or(|t| matches!(t.written().as_str(), "()" | ""));
    let mut dropped = Vec::new();
    unused(body, body.root, !returns_unit, &mut dropped);
    for e in dropped {
        let Some(ty) = types.of(body, e) else {
            continue;
        };
        if ty.as_builtin() != Some(crate::resolved::Builtin::Result) {
            continue;
        }
        out.push(Diagnostic {
            code: codes::RESULT_DROPPED.id,
            invariant: codes::RESULT_DROPPED.invariant,
            reason: "result_dropped",
            detector: Detector::PatternMatrix,
            severity: Severity::Error,
            message: format!(
                "this `{}` is dropped, and its failure with it",
                ty.display_name()
            ),
            primary_span: body.expr_span(e),
            related: vec![Related {
                span: at.clone(),
                label: format!("inside `{}`", decl.name),
            }],
            explanation: Some(
                "A `Result` carries a failure, and a value nothing uses handles neither \
                 of its cases: the program goes on as though it had succeeded. Until \
                 2026-09-26 a statement's `Result` was dropped without a word."
                    .to_string(),
            ),
            repairs: vec![Repair {
                description: "pass the failure on with `?`, handle it with `match`, or \
                              discard it by name, `let _ignored = ..`"
                    .to_string(),
                replacement: None,
            }],
        });
    }
}

/// Is `id` the head of a clause whose value is executable code, written as a
/// statement: `acquire`, or `release(handle)`?
fn executable_head(body: &Body, id: crate::hir::ExprId) -> bool {
    let name = match body.expr(id) {
        Expr::Name(n) => n,
        Expr::Call { callee, .. } => match body.expr(*callee) {
            Expr::Name(n) => n,
            _ => return false,
        },
        _ => return false,
    };
    crate::policy::domain_of(name) == Some(crate::policy::Domain::Body)
}

/// The expressions under `id` whose value nothing uses, where `used` says
/// whether `id`'s own is. A block, an `if` and a `match` pass their value
/// on; everything else uses what it contains.
fn unused(body: &Body, id: crate::hir::ExprId, used: bool, out: &mut Vec<crate::hir::ExprId>) {
    match body.expr(id) {
        Expr::Block { stmts } => {
            let last = stmts.len().saturating_sub(1);
            for (i, s) in stmts.iter().enumerate() {
                // `acquire { .. }`, `release(h) { .. }`: a block after a clause
                // whose value is code is that clause's value, which the
                // platform takes (the handle `acquire` produces).
                let clause = i > 0 && executable_head(body, stmts[i - 1]);
                // `return x` is two statements, and `x` is the value returned.
                let returned =
                    i > 0 && matches!(body.expr(stmts[i - 1]), Expr::Name(n) if n == "return");
                unused(body, *s, clause || returned || (used && i == last), out);
            }
        }
        Expr::If { cond, then, els } => {
            unused(body, *cond, true, out);
            unused(body, *then, used, out);
            if let Some(e) = els {
                unused(body, *e, used, out);
            }
        }
        Expr::Match { scrutinee, arms } => {
            unused(body, *scrutinee, true, out);
            for a in arms {
                unused(body, a.body, used, out);
            }
        }
        Expr::For {
            iterable, body: b, ..
        } => {
            unused(body, *iterable, true, out);
            unused(body, *b, false, out);
        }
        Expr::Lambda { body: b, .. } => unused(body, *b, true, out),
        other => {
            let _ = other;
            if !used {
                out.push(id);
            }
            for c in body.children(id) {
                unused(body, c, true, out);
            }
        }
    }
}

/// The module a declaration belongs to.
///
/// A handler named in `on:submit={on_press}` is a declaration in THIS module,
/// so it is looked up by its qualified path. Falling back to the unique-member
/// rule made the check depend on the name being unique across the whole
/// program — `resolve_corpus` caught it: adding a sibling that declared
/// `on_press` too made the lookup ambiguous, and the rule went silent instead
/// of going wrong, which is the harder failure to notice.
fn module_of(hir: &Hir, decl: crate::hir::DeclId) -> Option<&str> {
    hir.modules
        .iter()
        .find(|(_, m, _)| m.decls.contains(&decl))
        .map(|(_, m, _)| m.name.as_str())
}

// --- R-008: a value that may be absent, used as though it is present --------

fn optional_used_as_present(
    hir: &Hir,
    types: &crate::infer::Types<'_>,
    body: &Body,
    decl: &Decl,
    at: &Span,
    out: &mut Vec<Diagnostic>,
) {
    // Bindings that hold an `Option<T>`, with the inner type.
    //
    // From the ANNOTATION where the author wrote one, and otherwise from the
    // initialiser's declared return type. The second source is what makes this
    // a rule about the program rather than about how much the author chose to
    // write down: `let store = Stores.find(id)` is an `Option<Store>` because
    // `find` says so, and dropping the annotation used to make the defect
    // disappear.
    let mut optional: Vec<(String, String, String, Span)> = Vec::new();
    for id in body.walk() {
        let Expr::Let {
            pat: Some(pat),
            ty,
            init,
        } = body.expr(id)
        else {
            continue;
        };
        let crate::hir::Pattern::Bind { name, .. } = body.pat(*pat) else {
            continue;
        };
        let _ = (ty, init);
        let Some(ty) = types.binding(crate::lexical::Binder::Pattern(*pat)) else {
            continue;
        };
        if ty.as_builtin() != Some(crate::resolved::Builtin::Option) {
            continue;
        }
        let Some(inner) = ty.args().first() else {
            continue;
        };
        let (written, inner) = (ty.display_name(), inner.display_name());
        optional.push((name.clone(), written, inner, body.expr_span(id)));
    }
    if optional.is_empty() {
        return;
    }

    for id in body.walk() {
        // A field access is the case the corpus writes. `Option<T>` has no
        // fields of `T`; `store.name` is asking the container for something
        // only its contents have.
        let Expr::Field { base, name: field } = body.expr(id) else {
            continue;
        };
        let Expr::Name(base_name) = body.expr(*base) else {
            continue;
        };
        let Some((_, written, inner, origin)) = optional.iter().find(|(n, ..)| n == base_name)
        else {
            continue;
        };
        out.push(Diagnostic {
            code: codes::OPTION_USED_AS_VALUE.id,
            invariant: codes::OPTION_USED_AS_VALUE.invariant,
            reason: "optional_used_without_matching",
            detector: Detector::PatternMatrix,
            severity: Severity::Error,
            message: format!("`{written}` cannot be used where `{inner}` is expected"),
            primary_span: body.expr_span(id),
            related: vec![
                Related {
                    span: origin.clone(),
                    label: format!("`{base_name}` is `{written}` here"),
                },
                Related {
                    span: at.clone(),
                    label: format!("`{}` assumes the lookup succeeded", decl.name),
                },
            ],
            explanation: Some(format!(
                "`.{field}` is a field of `{inner}`, and `{base_name}` is not an \
                 `{inner}` — it is a value that may or may not be one. There is no \
                 ambient `null` in this language; match on `Some`/`None`, and the \
                 branch where the value is absent becomes something you decided \
                 rather than something that happens at three in the morning."
            )),
            repairs: vec![Repair {
                description: format!("match {base_name} {{ Some(v) => v.{field}, None => … }}"),
                replacement: None,
            }],
        });
        let _ = hir;
    }
}

// --- R-009: a cast where a decode belongs -----------------------------------

#[allow(clippy::too_many_arguments)]
fn unchecked_cast(
    hir: &Hir,
    sigs: &Signatures,
    types: &crate::infer::Types<'_>,
    body: &Body,
    decl: &Decl,
    at: &Span,
    out: &mut Vec<Diagnostic>,
) {
    for id in body.walk() {
        let Expr::Cast { value, ty } = body.expr(id) else {
            continue;
        };
        // The operand's INFERRED type, so a rebinding does not launder it.
        // Reading only parameters and annotated `let`s meant `let same = raw`
        // produced a value the rule no longer recognised as external — the
        // same narrowness privacy labels had, in a different analysis.
        if !types.of(body, *value).is_some_and(|t| {
            sigs.language_type("decode", "Unknown")
                .is_some_and(|unknown| t.same_as(&unknown))
        }) {
            continue;
        }
        let name = match body.expr(*value) {
            Expr::Name(n) => n.clone(),
            _ => "this value".to_string(),
        };
        let target = written(body, *ty);
        out.push(Diagnostic {
            code: codes::UNCHECKED_EXTERNAL_CAST.id,
            invariant: codes::UNCHECKED_EXTERNAL_CAST.invariant,
            reason: "cast_from_unknown",
            detector: Detector::PatternMatrix,
            severity: Severity::Error,
            message: format!("`Unknown` cannot be cast to `{target}`"),
            primary_span: body.expr_span(id),
            related: vec![Related {
                span: at.clone(),
                label: format!("`{}` receives external data", decl.name),
            }],
            explanation: Some(format!(
                "`{name}` came from outside the program, so nothing has checked it \
                 yet. A cast asserts the shape without looking; external values must \
                 be decoded with a `Decoder<{target}>` returning `Result`, so the \
                 case where the outside world sent something else is a value you \
                 handle rather than a crash you find later. That is what makes the \
                 boundary trustworthy rather than merely typed."
            )),
            repairs: vec![Repair {
                description: format!("decode.run(decoder_for_{}, {name})", lower_head(&target)),
                replacement: None,
            }],
        });
        let _ = hir;
    }
}

// --- R-022: a handler that does not accept what it is given -----------------

#[allow(clippy::too_many_arguments)]
fn handler_matches_event(
    hir: &Hir,
    body: &Body,
    sigs: &Signatures,
    decl: &Decl,
    module: Option<&str>,
    at: &Span,
    out: &mut Vec<Diagnostic>,
) {
    let mut roots = Vec::new();
    for id in body.walk() {
        if let Expr::Template { roots: r, .. } = body.expr(id) {
            roots.extend(r.iter().copied());
        }
    }

    for n in body.walk_markup(&roots) {
        let Node::Element { attrs, .. } = body.node(n) else {
            continue;
        };
        for a in attrs {
            let Some(event) = a.name.strip_prefix("on:") else {
                continue;
            };
            // The event an attribute delivers is a platform fact:
            // `events.submit(event: SubmitEvent)`. The MODULE is a language
            // binding, in the same way `measure` names a frame phase — `on:`
            // attributes resolve in `events`, and that is stated here rather
            // than reached by "exactly one module declares a `submit`", which
            // would make the rule depend on an accident of the program.
            // An event the platform does not declare is not an event
            // (ADR-0093). A program with no `events` module declares none,
            // and there is nothing to hold the attribute to.
            if sigs.by_path(&format!("{EVENTS_MODULE}.{event}")).is_none() {
                let mut declared: Vec<&str> = sigs
                    .iter()
                    .filter_map(|(path, _)| path.strip_prefix(EVENTS_MODULE)?.strip_prefix('.'))
                    .collect();
                declared.sort();
                if !declared.is_empty() {
                    out.push(unknown_event(decl, at, a, event, &declared));
                }
                continue;
            }
            let Some(expected) = sigs
                .by_path(&format!("{EVENTS_MODULE}.{event}"))
                .and_then(|s| s.params.first())
                .and_then(Option::as_ref)
                .and_then(crate::resolved::TypeResolution::resolved)
            else {
                continue;
            };
            let AttrValue::Expr(e) = a.value else {
                continue;
            };
            let Expr::Name(handler) = body.expr(e) else {
                continue;
            };
            let Some(actual) = sigs
                .in_module(module, handler)
                .and_then(|s| s.params.first())
                .and_then(Option::as_ref)
                .and_then(crate::resolved::TypeResolution::resolved)
            else {
                continue;
            };
            if actual.same_as(expected) {
                continue;
            }
            out.push(Diagnostic {
                code: codes::HANDLER_SIGNATURE_MISMATCH.id,
                invariant: codes::HANDLER_SIGNATURE_MISMATCH.invariant,
                reason: "handler_takes_the_wrong_event",
                detector: Detector::PatternMatrix,
                severity: Severity::Error,
                message: format!(
                    "`on:{event}` expects a handler taking `{expected}`, found one \
                     taking `{actual}`"
                ),
                primary_span: body.expr_span(e),
                related: vec![Related {
                    span: at.clone(),
                    label: format!("`{}` binds the handler here", decl.name),
                }],
                explanation: Some(format!(
                    "Form submission and pointer activation are different events, and \
                     `{handler}` is written for `{actual}`. Without this check the \
                     handler would simply never fire — which looks like a bug in the \
                     form rather than a mismatch at the binding."
                )),
                repairs: vec![Repair {
                    description: format!(
                        "give `{handler}` a `{expected}` parameter, or bind a handler \
                         that already takes one"
                    ),
                    replacement: None,
                }],
            });
        }
    }
    let _ = hir;
}

/// `on:clik={go}`: an event the platform does not declare (ADR-0093).
fn unknown_event(
    decl: &Decl,
    at: &Span,
    attr: &crate::hir::Attr,
    event: &str,
    declared: &[&str],
) -> Diagnostic {
    Diagnostic {
        code: codes::UNKNOWN_EVENT.id,
        invariant: codes::UNKNOWN_EVENT.invariant,
        reason: "unknown_event",
        detector: Detector::PatternMatrix,
        severity: Severity::Error,
        message: format!(
            "`on:{event}` names no event the platform declares: {}",
            crate::check::listed(declared)
        ),
        primary_span: attr.span.clone(),
        related: vec![Related {
            span: at.clone(),
            label: format!("`{}` binds a handler to it here", decl.name),
        }],
        explanation: Some(
            "Which event an `on:` attribute delivers is declared by the platform, in \
             `events`. An attribute naming no declared event binds its handler to nothing. \
             The runtime listens for a click whatever the name, so until 2026-09-26 such \
             a handler ran by accident, and a runtime that listens for the named event \
             would never run it."
                .to_string(),
        ),
        repairs: vec![Repair {
            description: "name a declared event, or declare it in the platform's `events`"
                .to_string(),
            replacement: None,
        }],
    }
}

// --- shared ------------------------------------------------------------------

/// A type as the author wrote it: `Option<Store>`, not `Option`.
fn written(body: &Body, id: TypeRefId) -> String {
    let Some(t) = body.types.get(id.index()) else {
        return String::new();
    };
    if t.args.is_empty() {
        return t.path.clone();
    }
    let args: Vec<String> = t.args.iter().map(|a| written(body, *a)).collect();
    format!("{}<{}>", t.path, args.join(", "))
}

fn lower_head(t: &str) -> String {
    t.split('<').next().unwrap_or(t).to_lowercase()
}
