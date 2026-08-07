//! Charter §8.5, §7.8 — what a resumable handler may capture.
//!
//! A `resumable(captures = { .. })` handler does not close over the browser's
//! memory; its captures are **serialized into the resume manifest**, which
//! ships with the document. Two things follow, and the corpus writes one
//! fixture for each:
//!
//! - **R-010** captures a live `DatabaseConnection`. Serializing it is not
//!   merely lossy, it is impossible — the value *is* the open socket.
//! - **R-030** captures a `Cart`, which only a `session query` produces.
//!   Anything in the manifest inherits the document's cacheability, so a
//!   session-scoped value in it is served to whoever the shell is served to.
//!
//! Both are read from declarations. A type is unserializable because some
//! function declares `resource.acquire<T>` for it, and a type is
//! session-scoped because the query producing it is declared `session`. There
//! is no list of unserializable types in the compiler, and adding a resource
//! to a library brings both consequences with it.
//!
//! # This module is a POLICY, not an analysis
//!
//! Architect ruling, 2026-08-07:
//!
//! > Remote-capable is not equivalent to resume-serializable. But they are two
//! > policies over the same underlying semantic fact: whether a typed value can
//! > safely cross a boundary. Don't build a second `is_remote_capable_type()`
//! > beside `is_serializable_capture()`.
//!
//! So `boundary.rs` owns [`crate::boundary::TypeFacts`] and
//! [`crate::boundary::can_cross`]; this module supplies
//! `Boundary::ResumeCapture` and turns the verdict into the two diagnostics the
//! corpus names. `binding.rs` supplies `Boundary::RemoteCall` over exactly the
//! same facts.
//!
//! The maps `TypeFacts` holds were declared here until 2026-08-07 and moved
//! rather than copied — one analysis, or the two would agree until the day one
//! of them changed.

use crate::boundary::{
    Blocked, Boundary, BoundaryContext, Crossing, Direction, Violation, can_cross,
};
use crate::codes;
use crate::diagnostics::{Detector, Diagnostic, Related, Repair, Severity};
use crate::hir::{Decl, Expr, ExprId, Hir, Span};
use crate::privacy::Restriction;
use crate::signatures::Signatures;

/// The captures with their spans, for a diagnostic that points at one.
fn captures_with_spans(
    body: &crate::hir::Body,
    types: &crate::infer::Types<'_>,
    descriptor: ExprId,
) -> Vec<(String, Span, Option<String>)> {
    captures(body, descriptor)
        .into_iter()
        .map(|(name, span, expr)| (name, span, types.of(body, expr)))
        .collect()
}

/// The capture schema a handler declares: (name, type) per capture.
///
/// E7V's build-time half. The runtime compares content hashes ACROSS
/// deployments; within one build there is no version skew, so the question is
/// narrower — does every capture have a type the manifest can name? A capture
/// whose type this build cannot determine yields a schema hash derived from a
/// guess, and a guessed hash matches nothing, so every resume would fail after
/// deployment for a reason nobody could diagnose from the deployment.
/// Is this descriptor a `resumable(..)` call at all?
///
/// Separate from "does it capture anything". The two were one test — an empty
/// capture list meant no resume artifact — and that silently denied an identity
/// to `resumable() => clear_cart()`, a handler that legitimately needs nothing
/// from the document. Without an identity it cannot be authorised, so the
/// button rendered, existed, and could never work; nothing said so.
pub(crate) fn is_resumable(body: &crate::hir::Body, descriptor: ExprId) -> bool {
    let Expr::Call { callee, .. } = body.expr(descriptor) else {
        return false;
    };
    matches!(body.expr(*callee), Expr::Name(n) if n == "resumable")
}

pub(crate) fn capture_names_and_types(
    body: &crate::hir::Body,
    types: &crate::infer::Types<'_>,
    descriptor: ExprId,
) -> Vec<(String, Option<String>)> {
    captures(body, descriptor)
        .into_iter()
        .map(|(name, _, expr)| (name, types.of(body, expr)))
        .collect()
}

pub fn check(
    hir: &Hir,
    sigs: &Signatures,
    facts: &crate::boundary::TypeFacts,
    out: &mut Vec<Diagnostic>,
) {
    for (id, decl) in hir.all_decls() {
        let Some(body_id) = decl.body else { continue };
        let body = hir.body(body_id);
        let at = hir.decl_span(id);

        // The two questions are answered from two different facts, because they
        // are different questions and a value can pass one and fail the other:
        //
        //   can this value be serialized?      -> its TYPE (is it a resource)
        //   may it cross this boundary?        -> its LABEL (is it private)
        //
        // Both are now `boundary::can_cross`'s, and the split above is the
        // reason `TransferProfile` is per-type while the label travels in the
        // `BoundaryContext`.
        //
        // Reading a capture's type from the declaration's parameter list was
        // the narrow version: it saw `connection: DatabaseConnection` and
        // nothing else. `Types` follows a chain and `Labels` follows a value,
        // so a capture that is a field, a rebinding or a branch is answered
        // the same way as a bare parameter.
        let types = crate::infer::Types::of_body(sigs, decl, body, hir.module_of(id));
        let imports = crate::labels::imported_modules(hir);
        let labels = crate::labels::Labels::of_body(sigs, decl, body, hir.module_of(id), &imports);

        for lambda in body.walk() {
            let Expr::Lambda {
                descriptor: Some(d),
                ..
            } = body.expr(lambda)
            else {
                continue;
            };
            // Build-time artifact agreement: every capture must have a type
            // the manifest can name.
            //
            // `Crossing::Blocked` is exactly this — the analysis has no basis
            // to decide — and asking for it rather than testing `ty.is_some()`
            // keeps one definition of "this build could not determine the
            // type". The diagnostic stays here because what a BLOCKED crossing
            // means for a resume manifest (a schema hash derived from a guess)
            // is this policy's to say.
            for (name, span, ty) in captures_with_spans(body, &types, *d) {
                if !matches!(
                    can_cross(
                        &facts.profile(ty.as_deref()),
                        &BoundaryContext {
                            boundary: Boundary::ResumeCapture,
                            direction: Direction::Outbound,
                            label: crate::privacy::Label::public(),
                        },
                    ),
                    Crossing::Blocked(Blocked::UndeterminedSchema)
                ) {
                    continue;
                }
                out.push(Diagnostic {
                    code: codes::RESUME_CAPTURE_SCHEMA_UNNAMEABLE.id,
                    invariant: codes::RESUME_CAPTURE_SCHEMA_UNNAMEABLE.invariant,
                    reason: "capture_has_no_nameable_type",
                    detector: Detector::PatternMatrix,
                    severity: Severity::Error,
                    message: format!("`{name}` has no type the resume manifest can name"),
                    primary_span: span,
                    related: vec![Related {
                        span: at.clone(),
                        label: format!("`{}` declares the handler", decl.name),
                    }],
                    explanation: Some(format!(
                        "A resume manifest carries a schema hash derived from what a \
                         handler captures, and the running code compares it against \
                         the schema its own build produced. A capture whose type this \
                         build cannot determine yields a hash derived from a guess, \
                         which matches nothing — so every resume of `{}` would fail \
                         after deployment, for a reason nobody could diagnose from the \
                         deployment. Name the type here, where it is still a compile \
                         error.",
                        decl.name
                    )),
                    repairs: vec![Repair {
                        description: format!(
                            "annotate `{name}`, or capture a value whose type is declared"
                        ),
                        replacement: None,
                    }],
                });
            }

            // **One call, two diagnostics.** The union of label and producer
            // scope, the resource check and their ordering all live in
            // `boundary.rs` now; what is left here is which message each
            // verdict earns, which is what makes this a policy rather than a
            // second analysis.
            for (name, span, expr) in captures(body, *d) {
                let ty = types.of(body, expr);
                let verdict = can_cross(
                    &facts.profile(ty.as_deref()),
                    &BoundaryContext {
                        boundary: Boundary::ResumeCapture,
                        direction: Direction::Outbound,
                        label: labels.label(body, expr),
                    },
                );
                match verdict {
                    Crossing::Violation(Violation::Resource { ty, producer }) => {
                        out.push(unserializable(decl, &name, &ty, &producer, span, &at));
                    }
                    Crossing::Violation(Violation::Private { restriction }) => {
                        out.push(private_value(decl, &name, &restriction, span, &at));
                    }
                    // Reported above, against the capture's own span, with the
                    // schema-hash explanation this boundary needs.
                    Crossing::Blocked(_) | Crossing::Proven => {}
                }
            }
        }
    }
}

/// The values inside `resumable(captures = { a, b.c })`.
///
/// Returns the expression as well as its name, because what is captured may be
/// a chain — and it is the chain's type and label that decide, not the root
/// binding's.
fn captures(body: &crate::hir::Body, descriptor: ExprId) -> Vec<(String, Span, ExprId)> {
    let Expr::Call { callee, args } = body.expr(descriptor) else {
        return Vec::new();
    };
    if !matches!(body.expr(*callee), Expr::Name(n) if n == "resumable") {
        return Vec::new();
    }
    let mut out = Vec::new();
    for a in args
        .iter()
        .filter(|a| a.name.as_deref() == Some("captures"))
    {
        let Expr::Block { stmts } = body.expr(a.value) else {
            continue;
        };
        for s in stmts {
            let name = match body.expr(*s) {
                Expr::Name(n) => n.clone(),
                Expr::Field { .. } => crate::infer::path_of(body, *s),
                _ => continue,
            };
            out.push((name, body.expr_span(*s), *s));
        }
    }
    out
}

fn unserializable(
    decl: &Decl,
    name: &str,
    ty: &str,
    producer: &str,
    span: Span,
    at: &Span,
) -> Diagnostic {
    Diagnostic {
        code: codes::UNSERIALIZABLE_CAPTURE.id,
        invariant: codes::UNSERIALIZABLE_CAPTURE.invariant,
        reason: "resumable_handler_captures_a_resource",
        detector: Detector::PatternMatrix,
        severity: Severity::Error,
        message: format!("handler captures `{name}: {ty}`, which is not serializable"),
        primary_span: span,
        related: vec![Related {
            span: at.clone(),
            label: format!("`{}` declares the handler", decl.name),
        }],
        explanation: Some(format!(
            "A resumable handler may capture only serializable immutable values and \
             resource keys. `{ty}` is a resource — `{producer}` declares \
             `resource.acquire<{ty}>` — so the value IS the thing held open, not a \
             description of it. Serializing it cannot fail informatively at runtime; \
             it produces a manifest that is simply wrong."
        )),
        repairs: vec![Repair {
            description: format!(
                "capture the key that identifies what `{name}` points at, and acquire \
                 the resource again on the other side"
            ),
            replacement: None,
        }],
    }
}

fn private_value(decl: &Decl, name: &str, r: &Restriction, span: Span, at: &Span) -> Diagnostic {
    let written = match r {
        Restriction::Session(s) => format!("Session<{s}>"),
        Restriction::User(u) => format!("User<{u}>"),
        Restriction::Organization(o) => format!("Organization<{o}>"),
        Restriction::Secret(c) => format!("Secret<{c}>"),
        Restriction::Device => "Device".to_string(),
    };
    Diagnostic {
        code: codes::PRIVATE_IN_RESUME_MANIFEST.id,
        invariant: codes::PRIVATE_IN_RESUME_MANIFEST.invariant,
        reason: "resumable_handler_captures_a_private_value",
        detector: Detector::PatternMatrix,
        severity: Severity::Error,
        message: format!(
            "resume manifest for `{}` would contain a `{written}` value",
            decl.name
        ),
        primary_span: span,
        related: vec![Related {
            span: at.clone(),
            label: format!("`{}` declares the handler", decl.name),
        }],
        explanation: Some(format!(
            "The resume manifest is served with the public shell and may contain only \
             `Public` values. `{name}` is `{written}`, and anything in the manifest \
             inherits the document's cacheability — so this is not a question of who \
             reads the page, it is a question of what a cache is allowed to keep."
        )),
        repairs: vec![Repair {
            description: format!(
                "capture an identifier and re-read `{name}` on the server when the \
                 handler runs"
            ),
            replacement: None,
        }],
    }
}
