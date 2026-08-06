//! Diagnostics for E1A, in the shape ADR-0009 fixed and §16.3 requires.
//!
//! The `PW1004` wording is prescribed by the architect ruling:
//!
//! ```text
//! PW1004 Non-exhaustive match
//!
//! Missing cases:
//! - Cancelled(CancellationReason)
//! - Failed(OrderFailure)
//!
//! The presence of `failure`, `exception`, or any other effect does not
//! permit an incomplete domain-state match.
//! ```
//!
//! That last sentence is load-bearing. It is the difference between this
//! checker and the backend it lowers to.

use annotate_snippets::{AnnotationKind, Level, Renderer, Snippet};

use crate::exhaust::{MatchReport, render_witness};
use crate::types::{Program, Type};

pub struct MatchSite<'a> {
    pub source: &'a str,
    pub path: &'a str,
    /// Span of the whole `match` expression.
    pub span: std::ops::Range<usize>,
    /// Span of the scrutinee, so the diagnostic can point at what is being matched.
    pub scrutinee_span: Option<std::ops::Range<usize>>,
}

/// Render `PW1004` for a non-exhaustive match, or `None` if it is exhaustive.
pub fn render_non_exhaustive(
    program: &Program,
    scrutinee: &Type,
    report: &MatchReport,
    site: &MatchSite<'_>,
    styled: bool,
) -> Option<String> {
    if !report.outcome().is_violation() {
        return None;
    }

    let ty_name = program.type_name(scrutinee);
    let cases: Vec<String> = report
        .missing
        .iter()
        .map(|w| render_witness(program, scrutinee, w))
        .collect();

    let title = format!("[PW1004] non-exhaustive match on `{ty_name}`");
    let mut snippet = Snippet::source(site.source)
        .path(site.path)
        .line_start(1)
        .annotation(
            AnnotationKind::Primary
                .span(site.span.clone())
                .label(format!("this match does not cover every `{ty_name}`")),
        );
    if let Some(s) = &site.scrutinee_span {
        snippet = snippet.annotation(
            AnnotationKind::Context
                .span(s.clone())
                .label(format!("scrutinee has type `{ty_name}`")),
        );
    }

    let missing_list = cases
        .iter()
        .map(|c| format!("- {c}"))
        .collect::<Vec<_>>()
        .join("\n");

    let group = Level::ERROR
        .primary_title(title)
        .element(snippet)
        .element(Level::NOTE.message(format!("missing cases:\n{missing_list}")))
        // The sentence that distinguishes `pw` from its temporary backend.
        .element(Level::NOTE.message(
            "the presence of `panic`, `exception`, or any other effect does not permit an incomplete domain-state match",
        ))
        .element(Level::HELP.message(
            "add the missing arms, or — if the case is genuinely impossible — use `unreachable(proof)`; `unsafe.partial_match` is audited and carries the `panic` effect",
        ));

    let renderer = if styled {
        Renderer::styled()
    } else {
        Renderer::plain()
    };
    Some(renderer.render(&[group]))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::exhaust::{Arm, Pattern, check_match};
    use crate::types::Ctor;

    fn order_program() -> (Program, Type) {
        let mut p = Program::new();
        let reason = p.declare_adt(
            "CancellationReason",
            vec![Ctor {
                name: "OutOfStock".into(),
                fields: vec![],
            }],
        );
        let failure = p.declare_adt(
            "OrderFailure",
            vec![Ctor {
                name: "PaymentDeclined".into(),
                fields: vec![],
            }],
        );
        let id = p.declare_adt(
            "OrderState",
            vec![
                Ctor {
                    name: "Draft".into(),
                    fields: vec![],
                },
                Ctor {
                    name: "Confirmed".into(),
                    fields: vec![Type::Str],
                },
                Ctor {
                    name: "Cancelled".into(),
                    fields: vec![Type::Adt(reason)],
                },
                Ctor {
                    name: "Failed".into(),
                    fields: vec![Type::Adt(failure)],
                },
            ],
        );
        (p, Type::Adt(id))
    }

    #[test]
    fn pw1004_names_every_missing_case_and_the_effect_rule() {
        let (p, ty) = order_program();
        let src = "fn label(s: OrderState) -> String !{panic} {\n    match s {\n        Draft => \"draft\",\n        Confirmed(c) => c,\n    }\n}\n";
        let arms = vec![
            Arm {
                pattern: Pattern::unit(0),
                span: 0..0,
            },
            Arm {
                pattern: Pattern::ctor(1, vec![Pattern::Wildcard]),
                span: 0..0,
            },
        ];
        let report = check_match(&p, &ty, &arms);
        let site = MatchSite {
            source: src,
            path: "order/label.pw",
            span: src.find("match").unwrap()..src.rfind('}').unwrap(),
            scrutinee_span: Some(
                src.find("s: OrderState").unwrap()..src.find("s: OrderState").unwrap() + 1,
            ),
        };
        let text = render_non_exhaustive(&p, &ty, &report, &site, false)
            .expect("should be non-exhaustive");

        assert!(text.contains("PW1004"), "{text}");
        assert!(text.contains("Cancelled(CancellationReason)"), "{text}");
        assert!(text.contains("Failed(OrderFailure)"), "{text}");
        // The load-bearing sentence.
        assert!(
            text.contains("does not permit an incomplete domain-state match"),
            "{text}"
        );
        assert!(text.contains("unreachable(proof)"), "{text}");
        assert!(
            !text.contains('\u{1b}'),
            "plain rendering must be ANSI-free"
        );
    }

    #[test]
    fn an_exhaustive_match_renders_nothing() {
        let (p, ty) = order_program();
        let arms = vec![Arm {
            pattern: Pattern::Wildcard,
            span: 0..0,
        }];
        let report = check_match(&p, &ty, &arms);
        let site = MatchSite {
            source: "x",
            path: "x.pw",
            span: 0..1,
            scrutinee_span: None,
        };
        assert!(render_non_exhaustive(&p, &ty, &report, &site, false).is_none());
    }
}
