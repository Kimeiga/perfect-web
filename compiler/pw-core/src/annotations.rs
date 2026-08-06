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
        let types = crate::infer::Types::of_body(sigs, decl, body).in_module(module);
        optional_used_as_present(hir, sigs, body, decl, &at, out);
        unchecked_cast(hir, &types, body, decl, &at, out);
        handler_matches_event(hir, body, sigs, decl, module, &at, out);
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
    sigs: &Signatures,
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
        let annotated = ty.and_then(|t| body.types.get(t.index())).and_then(|t| {
            (t.path == "Option")
                .then(|| t.args.first().and_then(|a| body.types.get(a.index())))
                .flatten()
                // Internal invariant: this closure only runs inside
                // `ty.and_then(..)`, so `ty` is `Some` by construction.
                .map(|inner| (written(body, ty.expect("annotated")), inner.path.clone()))
        });
        let inferred = || {
            let init = (*init)?;
            let sig = sigs.by_path(&crate::infer::path_of(body, callee_of(body, init)?))?;
            if sig.returns.as_deref() != Some("Option") {
                return None;
            }
            let inner = sig.returns_args.first()?.clone();
            Some((format!("Option<{inner}>"), inner))
        };
        let Some((written, inner)) = annotated.or_else(inferred) else {
            continue;
        };
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

fn unchecked_cast(
    hir: &Hir,
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
        if types.of(body, *value).as_deref() != Some("Unknown") {
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
            let Some(expected) = sigs
                .by_path(&format!("{EVENTS_MODULE}.{event}"))
                .and_then(|s| s.params.first().cloned())
                .flatten()
            else {
                continue;
            };
            let AttrValue::Expr(e) = a.value else {
                continue;
            };
            let Expr::Name(handler) = body.expr(e) else {
                continue;
            };
            let Some(actual) = module
                .and_then(|m| sigs.by_path(&format!("{m}.{handler}")))
                .and_then(|s| s.params.first().cloned())
                .flatten()
            else {
                continue;
            };
            if actual == expected {
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

/// The callee of a call expression, if this is one.
fn callee_of(body: &Body, id: crate::hir::ExprId) -> Option<crate::hir::ExprId> {
    match body.expr(id) {
        Expr::Call { callee, .. } => Some(*callee),
        _ => None,
    }
}

fn lower_head(t: &str) -> String {
    t.split('<').next().unwrap_or(t).to_lowercase()
}
