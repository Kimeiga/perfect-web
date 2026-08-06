//! Charter §7.5A — the frame as a transaction.
//!
//! E2D's effect inference answers *which* effects a body performs. Three of the
//! layout corpus's defects are not answerable that way, because the effects are
//! individually legal and the defect is in how they relate:
//!
//! - **R-034** measures after writing, in one transaction. Both the write and
//!   the read are permitted; the *order* forces a synchronous reflow.
//! - **R-038** writes a layout-affecting property of the element whose size it
//!   observes. The write is ordinary; that it feeds its own trigger is not.
//! - **R-040** animates a property on the compositor that the compositor cannot
//!   animate. Nothing here is an effect violation — `width` is simply not a
//!   thing that can be composited.
//! - **R-043** declares a subtree independent and then reads a value from
//!   outside it. The declaration is an assertion about the world, and the code
//!   contradicts it.
//!
//! Each is checked against its accepted counterpart (A-018, A-020, A-021,
//! A-023), which differ from the rejected file in exactly the one way the rule
//! is about. That pairing is the negative control `docs/RISK_QUEUE.md` asks
//! for: a rule that banned `observe resize`, or `animate`, or `subtree
//! independent` outright would catch the rejected file and fail its twin.

use std::collections::BTreeSet;

use crate::codes;
use crate::diagnostics::{Detector, Diagnostic, Related, Repair, Severity};
use crate::hir::{Body, Decl, Expr, ExprId, Hir, Span};
use crate::signatures::Signatures;

/// The effect that means "this write can invalidate layout".
///
/// Charter §7.5A spells it with the type argument, and the argument is the
/// whole point: `style.mutate<LayoutAffect>` and `style.mutate` are different
/// effects, so a checker can tell a padding change from a colour change.
const LAYOUT_AFFECTING: &str = "style.mutate<LayoutAffect>";

pub fn check(hir: &Hir, sigs: &Signatures, out: &mut Vec<Diagnostic>) {
    for (id, decl) in hir.all_decls() {
        let Some(body_id) = decl.body else { continue };
        let body = hir.body(body_id);
        let at = hir.decl_span(id);
        frame_transaction_order(body, sigs, decl, &at, out);
        observation_feedback(body, sigs, decl, &at, out);
        compositor_animation(body, sigs, decl, &at, out);
        false_independence(body, decl, &at, out);
    }
}

// --- R-034: a read after a write, inside one frame ---------------------------

/// Charter §7.5A: a frame transaction orders every read before every write.
///
/// This is deliberately *not* a check that the phase keywords appear in a fixed
/// order. A frame that writes twice is fine; a frame that measures twice is
/// fine. What is not fine is measuring geometry that a write earlier in the
/// same transaction has already invalidated, because the browser must then lay
/// out synchronously to answer.
fn frame_transaction_order(
    body: &Body,
    sigs: &Signatures,
    decl: &Decl,
    at: &Span,
    out: &mut Vec<Diagnostic>,
) {
    for frame in body.walk() {
        let Expr::Keyword { keyword, .. } = body.expr(frame) else {
            continue;
        };
        if keyword != "frame" {
            continue;
        }
        let frame_span = body.expr_span(frame);

        // The phase statements of this frame, in source order.
        let mut phases: Vec<(Span, &str)> = Vec::new();
        for id in body.walk() {
            let Expr::Keyword { keyword, .. } = body.expr(id) else {
                continue;
            };
            let s = body.expr_span(id);
            if s == frame_span || s.start < frame_span.start || s.end > frame_span.end {
                continue;
            }
            if matches!(keyword.as_str(), "measure" | "mutate" | "post_paint") {
                phases.push((s, keyword.as_str()));
            }
        }
        phases.sort_by_key(|(s, _)| s.start);

        let mut wrote: Option<Span> = None;
        for (span, keyword) in &phases {
            if *keyword == "mutate" && layout_affecting_write_in(body, sigs, span).is_some() {
                wrote = Some(span.clone());
                continue;
            }
            if *keyword != "measure" {
                continue;
            }
            let Some(write) = wrote.clone() else { continue };
            out.push(Diagnostic {
                code: codes::WRONG_FRAME_PHASE.id,
                invariant: codes::WRONG_FRAME_PHASE.invariant,
                reason: "measure_after_layout_affecting_write",
                detector: Detector::PatternMatrix,
                severity: Severity::Error,
                message: format!(
                    "`layout.measure` after `{LAYOUT_AFFECTING}` in the same transaction"
                ),
                primary_span: span.clone(),
                related: vec![
                    Related {
                        span: write,
                        label: format!("this writes `{LAYOUT_AFFECTING}`"),
                    },
                    Related {
                        span: at.clone(),
                        label: format!("`{}` declares this frame", decl.name),
                    },
                ],
                explanation: Some(
                    "A frame transaction orders every read before every write. Reading \
                     geometry after a write in the same transaction means the browser \
                     cannot answer from the layout it already has: this forces a \
                     synchronous layout; move the measurement to a later frame, or do \
                     every measurement before the first write."
                        .to_string(),
                ),
                repairs: vec![Repair {
                    description: "measure before the first `mutate`, or move this \
                                  measurement into the next frame"
                        .to_string(),
                    replacement: None,
                }],
            });
            break;
        }
    }
}

// --- R-038: an observation that triggers itself ------------------------------

/// Charter §7.5A: warn where a resize observation immediately changes the size
/// it observes.
///
/// The rule is a dependency, not a keyword. A-018 observes `self` and derives a
/// column count from it — no write, no cycle. R-038 observes `self` and writes
/// `self.style.set_width`, so each delivery schedules the next one; E0 recorded
/// that the browser reports an undelivered-notification loop for exactly this.
fn observation_feedback(
    body: &Body,
    sigs: &Signatures,
    decl: &Decl,
    at: &Span,
    out: &mut Vec<Diagnostic>,
) {
    let mut observed: Vec<(String, Span)> = Vec::new();
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
        if keyword != "observe" || !modifiers.iter().any(|m| m == "resize") {
            continue;
        }
        for a in args {
            if let Some(name) = root_name(body, *a) {
                observed.push((name, body.expr_span(id)));
            }
        }
    }
    if observed.is_empty() {
        return;
    }

    for id in body.walk() {
        let Some((target, property)) = layout_affecting_write(body, sigs, id) else {
            continue;
        };
        let Some((_, obs)) = observed.iter().find(|(n, _)| *n == target) else {
            continue;
        };
        out.push(Diagnostic {
            code: codes::OBSERVATION_FEEDBACK_CYCLE.id,
            invariant: codes::OBSERVATION_FEEDBACK_CYCLE.invariant,
            reason: "resize_observation_writes_observed_element",
            detector: Detector::PatternMatrix,
            severity: Severity::Error,
            message: "`observe.resize` handler writes a layout-affecting property \
                      of the observed element"
                .to_string(),
            primary_span: body.expr_span(id),
            related: vec![
                Related {
                    span: obs.clone(),
                    label: format!("`{target}` is observed here"),
                },
                Related {
                    span: at.clone(),
                    label: format!("`{}` declares both", decl.name),
                },
            ],
            explanation: Some(format!(
                "`{property}` writes `{LAYOUT_AFFECTING}` on `{target}`, and `{target}` \
                 is the element whose size is being observed — so the write schedules \
                 the observation that produced it. This is a feedback cycle; the \
                 browser reports an undelivered-notification loop rather than \
                 converging."
            )),
            repairs: vec![Repair {
                description: "write a property that does not affect the observed \
                              dimension, or derive the value instead of writing it back"
                    .to_string(),
                replacement: None,
            }],
        });
    }
}

// --- R-040: animating something the compositor cannot animate ----------------

/// Charter §7.5A: `animation.composite` permits only what the compositor can
/// run without layout.
///
/// Which properties those are is a platform fact, and it lives in
/// `packages/pw-platform-web/style.pw`: a property is compositable exactly when
/// its setter declares `animation.composite`. The compiler holds no list of
/// property names — E2C's deletion gate forbids one — it asks the signature
/// table.
///
/// An animated property is told from an animation *parameter* structurally: the
/// animated ones are written as transitions (`100.px -> 120.px`), and `duration:
/// 180.milliseconds` is not. So `duration` needs no exemption either.
fn compositor_animation(
    body: &Body,
    sigs: &Signatures,
    decl: &Decl,
    at: &Span,
    out: &mut Vec<Diagnostic>,
) {
    for id in body.walk() {
        let Expr::Keyword {
            keyword,
            block: Some(block),
            ..
        } = body.expr(id)
        else {
            continue;
        };
        if keyword != "animate" {
            continue;
        }
        for (property, span) in transitioned_fields(body, *block) {
            if compositable(sigs, &property) {
                continue;
            }
            let permitted = compositor_properties(sigs);
            let permitted = if permitted.is_empty() {
                "nothing this platform declares".to_string()
            } else {
                permitted
                    .iter()
                    .map(|p| format!("`{p}`"))
                    .collect::<Vec<_>>()
                    .join(" and ")
            };
            out.push(Diagnostic {
                code: codes::FORBIDDEN_EFFECT.id,
                invariant: codes::FORBIDDEN_EFFECT.invariant,
                reason: "animated_property_is_not_compositor_only",
                detector: Detector::PatternMatrix,
                severity: Severity::Error,
                message: format!("`{property}` is not a compositor-only property"),
                primary_span: span,
                related: vec![Related {
                    span: at.clone(),
                    label: format!("`{}` declares this animation", decl.name),
                }],
                explanation: Some(format!(
                    "`animation.composite` permits only {permitted} — the properties \
                     whose setters declare it, because they do not invalidate layout. \
                     Animating `{property}` runs layout on every frame, which is the \
                     cost the phase distinction exists to prevent."
                )),
                repairs: vec![Repair {
                    description: format!(
                        "express the change as {permitted}, or drop the \
                         compositor-only claim and accept the layout cost"
                    ),
                    replacement: None,
                }],
            });
        }
    }
}

/// Fields inside an animation block whose value is a transition.
fn transitioned_fields(body: &Body, block: ExprId) -> Vec<(String, Span)> {
    use crate::hir::BinOp;
    let mut out = Vec::new();
    let Expr::Block { stmts } = body.expr(block) else {
        return out;
    };
    for s in stmts {
        let Expr::Record { fields, .. } = body.expr(*s) else {
            continue;
        };
        for f in fields {
            let Some(value) = f.value else { continue };
            if matches!(
                body.expr(value),
                Expr::Binary {
                    op: BinOp::Transition,
                    ..
                }
            ) {
                out.push((f.name.clone(), f.span.clone()));
            }
        }
    }
    out
}

/// Can the compositor animate this property? Answered by the platform package.
fn compositable(sigs: &Signatures, property: &str) -> bool {
    sigs.member(None, &format!("set_{property}"))
        .is_some_and(|s| s.effects.iter().any(|e| e == "animation.composite"))
}

/// Every property the platform declares compositable, for the message.
fn compositor_properties(sigs: &Signatures) -> Vec<String> {
    let mut out: BTreeSet<String> = BTreeSet::new();
    for (path, sig) in sigs.iter() {
        if !sig.effects.iter().any(|e| e == "animation.composite") {
            continue;
        }
        let name = path.rsplit('.').next().unwrap_or(path);
        if let Some(p) = name.strip_prefix("set_") {
            out.insert(p.to_string());
        }
    }
    out.into_iter().collect()
}

// --- R-043: a subtree that is not as independent as it says ------------------

/// Charter §7.5A: containment is a *semantic assertion* by the author, and the
/// compiler may never infer it — measurement showed containment can be slower
/// where it does not belong.
///
/// What the compiler can do is check the assertion. A subtree declared
/// independent may not read a value its ancestors provide, because containment
/// would then change the rendered result rather than merely its cost. A-023 is
/// the same component without the ancestor-provided custom property.
fn false_independence(body: &Body, decl: &Decl, at: &Span, out: &mut Vec<Diagnostic>) {
    use crate::hir::{AttrValue, Node};

    let independent = body.walk().into_iter().find(|id| {
        matches!(body.expr(*id), Expr::Keyword { keyword, modifiers, .. }
            if keyword == "subtree" && modifiers.iter().any(|m| m == "independent"))
    });
    let Some(independent) = independent else {
        return;
    };
    let claim = body.expr_span(independent);

    // Custom properties this component defines itself. One it sets is its own;
    // one it only reads comes from an ancestor.
    let defined: BTreeSet<String> = body
        .walk()
        .into_iter()
        .filter_map(|id| custom_property_arg(body, id, "set_custom"))
        .collect();

    for id in body.walk() {
        let Expr::Template { roots, .. } = body.expr(id) else {
            continue;
        };
        for n in body.walk_markup(roots) {
            let Node::Element { attrs, .. } = body.node(n) else {
                continue;
            };
            for a in attrs {
                // Only a style binding participates in the subtree's own layout;
                // an ordinary attribute does not.
                if !a.name.starts_with("style:") {
                    continue;
                }
                let AttrValue::Expr(e) = a.value else {
                    continue;
                };
                for read in body.walk_from(e) {
                    let Some(property) = custom_property_arg(body, read, "var") else {
                        continue;
                    };
                    if defined.contains(&property) {
                        continue;
                    }
                    out.push(Diagnostic {
                        code: codes::FALSE_INDEPENDENCE.id,
                        invariant: codes::FALSE_INDEPENDENCE.invariant,
                        reason: "independent_subtree_reads_ancestor_value",
                        detector: Detector::PatternMatrix,
                        severity: Severity::Error,
                        message: format!(
                            "subtree declared `independent` but its layout depends on \
                             `{property}` from an ancestor"
                        ),
                        primary_span: body.expr_span(read),
                        related: vec![
                            Related {
                                span: claim.clone(),
                                label: "independence is asserted here".to_string(),
                            },
                            Related {
                                span: at.clone(),
                                label: format!("`{}` declares both", decl.name),
                            },
                        ],
                        explanation: Some(format!(
                            "`{}` binds a layout property to `{property}`, which nothing \
                             in this subtree defines — so the value crosses the \
                             containment boundary the declaration says does not exist. \
                             Containment would change the rendered result, not merely \
                             its cost, which is why it is asserted rather than inferred.",
                            a.name
                        )),
                        repairs: vec![Repair {
                            description: "pass the value in as a parameter, or drop the \
                                          `independent` claim"
                                .to_string(),
                            replacement: None,
                        }],
                    });
                }
            }
        }
    }
}

/// The custom-property name in `var("--x")` or `set_custom(el, "--x", ..)`.
fn custom_property_arg(body: &Body, id: ExprId, callee: &str) -> Option<String> {
    let Expr::Call { callee: c, args } = body.expr(id) else {
        return None;
    };
    let named = match body.expr(*c) {
        Expr::Name(n) => n == callee,
        Expr::Field { name, .. } => name == callee,
        _ => false,
    };
    if !named {
        return None;
    }
    args.iter().find_map(|a| match body.expr(a.value) {
        Expr::Literal(crate::hir::Literal::Str(s)) => {
            let s = s.trim_matches('"');
            s.starts_with("--").then(|| s.to_string())
        }
        _ => None,
    })
}

// --- shared ------------------------------------------------------------------

/// Does this expression call a setter the platform declares layout-affecting?
/// Returns the receiver it writes to and the setter's name.
fn layout_affecting_write(body: &Body, sigs: &Signatures, id: ExprId) -> Option<(String, String)> {
    let Expr::Call { callee, .. } = body.expr(id) else {
        return None;
    };
    let Expr::Field { base, name } = body.expr(*callee) else {
        return None;
    };
    let sig = sigs.member(None, name)?;
    if !sig.effects.iter().any(|e| e == LAYOUT_AFFECTING) {
        return None;
    }
    Some((root_name(body, *base)?, name.clone()))
}

fn layout_affecting_write_in(body: &Body, sigs: &Signatures, span: &Span) -> Option<ExprId> {
    body.walk().into_iter().find(|id| {
        let s = body.expr_span(*id);
        s.start >= span.start
            && s.end <= span.end
            && layout_affecting_write(body, sigs, *id).is_some()
    })
}

/// The name at the root of a field chain: `self.style` and `self` both give
/// `self`. Which element is written is the question the feedback rule asks, and
/// the chain is how the corpus names it.
fn root_name(body: &Body, id: ExprId) -> Option<String> {
    match body.expr(id) {
        Expr::Name(n) => Some(n.clone()),
        Expr::Field { base, .. } => root_name(body, *base),
        Expr::Call { callee, .. } => root_name(body, *callee),
        _ => None,
    }
}
