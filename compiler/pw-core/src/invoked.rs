//! **A command the platform invokes, which no reader presses** (NEXT 6c, the
//! payments track's; PW64xx).
//!
//! `invoked_by Payments` says which command a payment's outcome invokes. The
//! host calls it once it has its provider's word for the outcome, a signed
//! event or the reader's return, each read again from the provider; no page
//! sends it, and a press naming it is answered as a command that is not
//! there. Convex's internal functions are the same boundary ("can only be
//! called by other functions and cannot be called directly from a Convex
//! client"), as is a WASI handler export, the platform's to call.
//!
//! - PW6401: it declares no `requires`: no reader acts, so no predicate has
//!   anyone to hold.
//! - PW6402: it is given one `payments.Settled`, what the host has to give.
//! - PW6403: one command answers a payment's outcome in a program.
//! - PW6404: no page's handler sends it.

use std::collections::BTreeMap;

use crate::codes;
use crate::diagnostics::{Detector, Diagnostic, Related, Repair, Severity};
use crate::hir::{DeclId, DeclKind, Hir, Span};
use crate::resolve::{DefId, Namespace, Resolution, Workspace};

/// The platform's invokers, by the word a command names them with.
pub const INVOKERS: &[&str] = &["Payments"];

/// **The command each invoker invokes**, in source order across the program:
/// `(unit, declaration)` of each command that names it.
pub fn invoked(hirs: &[&Hir]) -> BTreeMap<String, Vec<(usize, DeclId)>> {
    let mut out: BTreeMap<String, Vec<(usize, DeclId)>> = BTreeMap::new();
    for (unit, hir) in hirs.iter().enumerate() {
        for (id, decl) in hir.all_decls() {
            if decl.kind != DeclKind::Command {
                continue;
            }
            if let Some(p) = decl.policy("invoked_by") {
                out.entry(p.value.trim().to_string())
                    .or_default()
                    .push((unit, id));
            }
        }
    }
    out
}

/// **What `unit` writes wrong of a command the platform invokes.**
pub fn check(
    hirs: &[&Hir],
    unit: usize,
    ws: &Workspace,
    sent: &BTreeMap<DefId, Vec<(usize, Span)>>,
) -> Vec<Diagnostic> {
    let mut out = Vec::new();
    let hir = hirs[unit];
    let all = invoked(hirs);
    for (id, decl) in hir.all_decls() {
        if decl.kind != DeclKind::Command {
            continue;
        }
        let Some(by) = decl.policy("invoked_by") else {
            continue;
        };
        let invoker = by.value.trim();
        let at = hir.decl_span(id);
        if let Some(requires) = decl.policy("requires") {
            out.push(diagnostic(
                codes::INVOKED_COMMAND_REQUIRES,
                "invoked_command_requires",
                format!(
                    "`{}` is invoked by {invoker}, not by a reader: `requires {}` has no one to hold",
                    decl.name,
                    requires.value.trim()
                ),
                requires.span.clone(),
                vec![Related::new(at.clone(), format!("`invoked_by {invoker}`"))],
                "A command the platform invokes runs on the platform's word, the provider's \
                 for a payment, which the host has checked before it calls it. No reader \
                 presses it, so a predicate over the reader has nobody to hold; what a reader \
                 may do is held where the reader acts, on the command that began the payment.",
                format!("remove `requires` from `{}`", decl.name),
            ));
        }
        if !given_settled(unit, ws, decl) {
            let written: Vec<String> = decl
                .params
                .iter()
                .map(|p| match &p.ty {
                    Some(t) => format!("{}: {}", p.name, t.written()),
                    None => p.name.clone(),
                })
                .collect();
            out.push(diagnostic(
                codes::INVOKED_COMMAND_PARAMETERS,
                "invoked_command_parameters",
                format!(
                    "`{}` is invoked by {invoker} with a `payments.Settled`, and takes ({})",
                    decl.name,
                    written.join(", ")
                ),
                at.clone(),
                Vec::new(),
                "The host invokes the command with what it has from the provider: which \
                 payment, the order it was begun for, and where it now stands, a \
                 `payments.Settled`. A parameter it has nothing to give would be a value \
                 nobody chose.",
                format!(
                    "declare `{}(settled: Settled)`, importing `payments.{{ Settled }}`",
                    decl.name
                ),
            ));
        }
        // The first, in source order across the program, is the one; every
        // other is refused where it is written, naming it.
        if let Some(each) = all.get(invoker)
            && let Some(&(first_unit, first)) = each.first()
            && (first_unit, first) != (unit, id)
        {
            out.push(diagnostic(
                codes::INVOKED_TWICE,
                "invoked_twice",
                format!(
                    "`{}` and `{}` are both invoked by {invoker}",
                    hirs[first_unit].decl(first).name,
                    decl.name
                ),
                at.clone(),
                vec![Related::in_unit(
                    first_unit,
                    hirs[first_unit].decl_span(first),
                    format!(
                        "`{}` is invoked by {invoker} first",
                        hirs[first_unit].decl(first).name
                    ),
                )],
                "The host invokes one command with a payment's outcome. With two, the \
                 outcome would have two answers, and which ran would be the host's choice, \
                 not the program's.",
                "keep one command `invoked_by` it, and call the other's work from it".to_string(),
            ));
        }
        let def = DefId { unit, decl: id.0 };
        if let Some(calls) = sent.get(&def) {
            out.push(diagnostic(
                codes::INVOKED_COMMAND_PRESSED,
                "invoked_command_pressed",
                format!(
                    "`{}` is invoked by {invoker}, and a page's handler sends it",
                    decl.name
                ),
                at.clone(),
                calls
                    .iter()
                    .map(|(at, span)| {
                        Related::in_unit(
                            *at,
                            span.clone(),
                            "a press sends it from here".to_string(),
                        )
                    })
                    .collect(),
                "A command the platform invokes is answered on the platform's word: a \
                 payment's outcome on the provider's, read again from the provider. A press \
                 is a reader's word, and would settle a payment the provider never took; the \
                 host answers one as a command that is not there.",
                format!(
                    "send the command that begins the payment, not `{}`",
                    decl.name
                ),
            ));
        }
    }
    out
}

/// Is `decl` given exactly one parameter, of the platform's `payments.Settled`?
fn given_settled(unit: usize, ws: &Workspace, decl: &crate::hir::Decl) -> bool {
    let [param] = decl.params.as_slice() else {
        return false;
    };
    let Some(ty) = &param.ty else {
        return false;
    };
    let written = ty.written();
    let named = match written.contains('.') {
        true => ws.resolve_path_in(unit, Namespace::Type, &written),
        false => ws.resolve_in(unit, Namespace::Type, &written),
    };
    match named {
        Resolution::Local(d) | Resolution::Imported { def: d, .. } => {
            ws.module_of(d.unit).is_some_and(|m| {
                m.name == "payments"
                    && m.defines.get(&(Namespace::Type, "Settled".to_string())) == Some(&d)
            })
        }
        _ => false,
    }
}

fn diagnostic(
    code: codes::Code,
    reason: &'static str,
    message: String,
    span: Span,
    related: Vec<Related>,
    explanation: &str,
    repair: String,
) -> Diagnostic {
    Diagnostic {
        code: code.id,
        invariant: code.invariant,
        reason,
        detector: Detector::DeclarationRule,
        severity: Severity::Error,
        message,
        primary_span: span,
        related,
        explanation: Some(explanation.to_string()),
        repairs: vec![Repair {
            description: repair,
            replacement: None,
        }],
    }
}
