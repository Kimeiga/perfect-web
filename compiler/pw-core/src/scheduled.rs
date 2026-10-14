//! **A command runs when the clock passes its time** (the scheduled-commands
//! track; PW65xx).
//!
//! `schedule cmd(args) at when` names a command `invoked_by Clock` and the
//! instant its run is due; `schedule cmd(args) after duration`, the time from
//! the scheduling command's commit, which the host reads then, so no program
//! reads the time. `unschedule cmd(args)` cancels the one pending run of that
//! command with those arguments. Each is a command's statement, written with
//! its writes, as Convex's `runAt` and `runAfter` are atomic with their
//! mutation.
//!
//! - PW6501: a `schedule` or an `unschedule` names a command the clock
//!   invokes, written as a call.
//! - PW6502: each is a command's statement: what a page shows schedules
//!   nothing.
//! - PW6503: a run's time is `at` an `Instant` or `after` a `Duration`.
//!
//! The command's arguments are related as a call's (`values.rs`), and
//! `clock.schedule` is charged where each is written (`effects.rs`).

use crate::codes;
use crate::diagnostics::{Detector, Diagnostic, Related, Repair, Severity};
use crate::hir::{DeclId, DeclKind, Expr, Hir, Span};
use crate::resolve::{Namespace, Resolution, Workspace};
use crate::signatures::Signatures;
use crate::values::Ty;

/// **What `unit` writes wrong of a schedule.**
pub fn check(hirs: &[&Hir], unit: usize, ws: &Workspace, sigs: &Signatures) -> Vec<Diagnostic> {
    let hir = hirs[unit];
    let mut out = Vec::new();
    let module = ws
        .modules
        .iter()
        .find(|m| m.unit == unit)
        .map(|m| m.name.as_str());
    for (_, body, _) in hir.bodies.iter() {
        let decl = hir.decl(body.owner);
        let mut types = None;
        for id in body.walk() {
            let Expr::Keyword {
                keyword,
                modifiers,
                args,
                ..
            } = body.expr(id)
            else {
                continue;
            };
            if !matches!(keyword.as_str(), "schedule" | "unschedule") {
                continue;
            }
            let at = body.expr_span(id);
            if decl.kind != DeclKind::Command {
                out.push(diagnostic(
                    codes::SCHEDULED_OUTSIDE_A_COMMAND,
                    "scheduled_outside_a_command",
                    format!(
                        "`{keyword}` in `{}`, which is no command: what a page shows schedules \
                         nothing",
                        decl.name
                    ),
                    at,
                    vec![Related::new(
                        hir.decl_span(body.owner),
                        format!("`{}`", decl.name),
                    )],
                    "A run is scheduled with the writes of the command that decides it, in its \
                     transaction: a command that fails schedules nothing. A page, a view, a \
                     handler or a query writes nothing, so what it would schedule has no \
                     commit to be written with.",
                    "schedule it from the command a reader presses".to_string(),
                ));
                continue;
            }
            let name = modifiers.first().map(String::as_str).unwrap_or_default();
            let def = match ws.resolve_in(unit, Namespace::Term, name) {
                Resolution::Local(d) | Resolution::Imported { def: d, .. } => Some(d),
                _ => None,
            };
            let target = def.map(|d| hirs[d.unit].decl(DeclId(d.decl)));
            let clocked = target.is_some_and(|t| {
                t.kind == DeclKind::Command
                    && t.policy("invoked_by")
                        .is_some_and(|p| p.value.trim() == "Clock")
            });
            // `schedule cmd(args) at when`, `unschedule cmd(args)`: the
            // command's name, then the time's word, and nothing else bare.
            let shaped = match keyword.as_str() {
                "schedule" => {
                    modifiers.len() == 2
                        && matches!(modifiers[1].as_str(), "at" | "after")
                        && !args.is_empty()
                }
                _ => modifiers.len() == 1,
            };
            if !clocked || modifiers.is_empty() || (keyword == "unschedule" && !shaped) {
                out.push(diagnostic(
                    codes::SCHEDULE_NAMES_NO_CLOCKED_COMMAND,
                    "schedule_names_no_clocked_command",
                    match target {
                        _ if modifiers.is_empty() => {
                            format!("`{keyword}` in `{}` names no command", decl.name)
                        }
                        None => format!(
                            "`{keyword} {name}` in `{}` names no command this module sees",
                            decl.name
                        ),
                        Some(t) if t.kind != DeclKind::Command => format!(
                            "`{keyword} {name}` in `{}` names a declaration that is no command",
                            decl.name
                        ),
                        Some(_) if !clocked => format!(
                            "`{keyword} {name}` in `{}` names a command the clock does not \
                             invoke: it declares no `invoked_by Clock`",
                            decl.name
                        ),
                        Some(_) => format!(
                            "`{keyword} {name}` in `{}` is not written as a call, \
                             `{keyword} {name}(..)`",
                            decl.name
                        ),
                    },
                    at,
                    // Where what it names is, or where it is written.
                    match def {
                        Some(d) => vec![Related::in_unit(
                            d.unit,
                            hirs[d.unit].decl_span(DeclId(d.decl)),
                            format!("`{name}` is declared here"),
                        )],
                        None => vec![Related::new(
                            hir.decl_span(body.owner),
                            format!("`{}` schedules it", decl.name),
                        )],
                    },
                    "The clock runs a command declared `invoked_by Clock`, which no reader \
                     presses, on behalf of the reader whose command scheduled it. A schedule \
                     names it and gives it its arguments, as a call does; one run is pending for \
                     each command and its arguments.",
                    match keyword.as_str() {
                        "schedule" => format!("`schedule {name}(..) at when`"),
                        _ => format!("`unschedule {name}(..)`"),
                    },
                ));
                continue;
            }
            if keyword == "unschedule" {
                continue;
            }
            let word = modifiers.get(1).map(String::as_str).unwrap_or_default();
            if !shaped {
                // What it says where its time's word goes, as written:
                // `schedule expire(..) later`.
                let said: String = modifiers[1..].iter().map(|w| format!(" {w}")).collect();
                out.push(time_refused(
                    format!(
                        "`schedule {name}(..){said}` in `{}` says no time: `at` an instant, or \
                         `after` a duration",
                        decl.name
                    ),
                    at,
                    Related::new(hir.decl_span(body.owner), format!("`{}`", decl.name)),
                ));
                continue;
            }
            let Some(&time) = args.last() else { continue };
            let types = types.get_or_insert_with(|| {
                crate::values::BodyTypes::new(sigs, ws, unit, module, decl, body)
            });
            let (ty, shown) = types.of(time);
            let expected = match word {
                "at" => "Instant",
                _ => "Duration",
            };
            // The platform's own type, by its declaration: a program's type
            // spelt the same is another (RISK_QUEUE 34).
            let wanted = ws
                .modules
                .iter()
                .find(|m| m.name == "clock")
                .and_then(|m| m.defines.get(&(Namespace::Type, expected.to_string())))
                .copied();
            let agrees = match (&ty, wanted) {
                (Ty::Nominal(d, _), Some(want)) => *d == want,
                // A type the checker could not read is another relation's to say.
                (Ty::Unknown | Ty::Any | Ty::Var(_), _) => true,
                _ => false,
            };
            if !agrees {
                out.push(time_refused(
                    format!(
                        "`schedule {name}(..) {word}` in `{}` is given a `{shown}`, where it takes \
                         a `clock.{expected}`",
                        decl.name
                    ),
                    body.expr_span(time),
                    Related::new(at, format!("`schedule {name}(..) {word}`")),
                ));
            }
        }
    }
    out
}

fn time_refused(message: String, at: Span, written: Related) -> Diagnostic {
    diagnostic(
        codes::SCHEDULE_TIME,
        "schedule_time",
        message,
        at,
        vec![written],
        "A run is due `at` an `Instant`, an instant the program holds (from its data, or \
         `clock.at(date, time, zone)`), or `after` a `Duration` from the scheduling command's \
         commit (`minutes(30)`), which the host reads then: no program reads the time to \
         compute one.",
        "`at when`, an `Instant`, or `after minutes(n)`".to_string(),
    )
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
