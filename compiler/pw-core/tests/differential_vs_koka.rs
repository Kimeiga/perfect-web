//! The differential test that justifies E1A's existence.
//!
//! E0 measured (`docs/evidence/M0/spike-koka-js-interop.txt` §2) that Koka
//! **compiles** this program:
//!
//! ```koka
//! pub fun describe-order( s : order-state ) : exn string
//!   match s
//!     Draft   -> "draft"
//!     Pricing -> "pricing"
//!   // `Cancelled` omitted — accepted, because the row admits `exn`
//! ```
//!
//! and fails only at runtime with `pattern match failure`.
//!
//! ADR-0011 moved the rule to `pw`. This test asserts the *difference*: the same
//! shape, expressed in `pw-core`'s pattern matrix, is rejected — and rejected
//! identically whether or not the enclosing function may raise.
//!
//! If this test ever passes vacuously (because the checker stopped rejecting),
//! the project has silently regressed to Koka's guarantee.

use pw_core::exhaust::{Arm, Pattern, check_match};
use pw_core::types::{Ctor, Program, Type};

/// The `order-state` from the Koka spike, with `Cancelled` present.
fn order_state() -> (Program, Type) {
    let mut p = Program::new();
    let id = p.declare_adt(
        "OrderState",
        vec![
            Ctor {
                name: "Draft".into(),
                fields: vec![],
            },
            Ctor {
                name: "Pricing".into(),
                fields: vec![],
            },
            Ctor {
                name: "Cancelled".into(),
                fields: vec![Type::Str],
            },
        ],
    );
    (p, Type::Adt(id))
}

fn arm(p: Pattern) -> Arm {
    Arm {
        pattern: p,
        span: 0..0,
    }
}

/// The exact program Koka accepts under `exn`.
fn koka_accepted_arms() -> Vec<Arm> {
    vec![arm(Pattern::unit(0)), arm(Pattern::unit(1))]
}

#[test]
fn pw_rejects_the_match_koka_accepts_under_exn() {
    let (p, ty) = order_state();
    let report = check_match(&p, &ty, &koka_accepted_arms());
    assert!(
        report.outcome().is_violation(),
        "REGRESSION: pw accepted the non-exhaustive match that Koka only accepts \
         because the effect row admits exn. E1A's reason to exist has gone."
    );
    let names: Vec<String> = report
        .missing
        .iter()
        .map(|w| pw_core::exhaust::render_witness(&p, &ty, w))
        .collect();
    assert_eq!(names, vec!["Cancelled(String)".to_string()]);
}

/// The property that makes this a *language* rule rather than a lint: the answer
/// must not depend on the effect row.
///
/// `check_match`'s signature contains no effect row at all, so this is enforced
/// structurally. The test pins it so a future refactor cannot quietly thread one
/// in — which is exactly how Koka's version of this rule became conditional.
#[test]
fn the_result_does_not_depend_on_any_effect_row() {
    let (p, ty) = order_state();
    let arms = koka_accepted_arms();

    // Whatever a caller believes about effects, the same arms give the same
    // report. There is no parameter to vary.
    let a = check_match(&p, &ty, &arms);
    let b = check_match(&p, &ty, &arms);
    assert_eq!(a.missing.len(), b.missing.len());
    assert!(a.outcome().is_violation() && b.outcome().is_violation());

    // And adding the missing arm fixes it, in every context equally.
    let mut complete = arms.clone();
    complete.push(arm(Pattern::ctor(2, vec![Pattern::Wildcard])));
    assert!(!check_match(&p, &ty, &complete).outcome().is_violation());
}

/// A check that cannot fail is not a check (`docs/RISK_QUEUE.md`). This proves
/// the differential test above can go red: if the checker were replaced by one
/// that always reports "exhaustive", the assertion below would catch it.
#[test]
fn the_checker_can_distinguish_exhaustive_from_not() {
    let (p, ty) = order_state();
    let complete = vec![
        arm(Pattern::unit(0)),
        arm(Pattern::unit(1)),
        arm(Pattern::ctor(2, vec![Pattern::Wildcard])),
    ];
    assert!(!check_match(&p, &ty, &complete).outcome().is_violation());
    assert!(
        check_match(&p, &ty, &koka_accepted_arms())
            .outcome()
            .is_violation()
    );
}

/// Koka's other measured gap: `Nothing` and `Nil` are both `null`, so a decoder
/// working from the payload alone cannot tell an empty Option from an empty
/// List. `pw`'s decoder is type-directed and therefore never faces the question.
#[test]
fn option_and_list_stay_distinct_where_koka_conflates_them() {
    use pw_core::abi::{Decoder, Mode};
    use serde_json::json;

    let mut p = Program::new();
    let opt = p.declare_option(Type::Int);
    let list = p.declare_list(Type::Int);
    let d = Decoder::new(&p, Mode::Untyped);

    let none = d
        .decode("$", &Type::Adt(opt), &json!({"$case":"None"}))
        .unwrap();
    let nil = d
        .decode("$", &Type::Adt(list), &json!({"$case":"Nil"}))
        .unwrap();
    assert_ne!(none, nil);

    // Neither accepts a bare `null` — the value Koka would have produced for both.
    assert!(d.decode("$", &Type::Adt(opt), &json!(null)).is_err());
    assert!(d.decode("$", &Type::Adt(list), &json!(null)).is_err());
}

/// Koka erases single-field value structs, so `Money_usd(350)` *is* `350` and a
/// USD amount is indistinguishable from a EUR amount at the boundary.
#[test]
fn nominal_wrappers_stay_distinct_where_koka_erases_them() {
    use pw_core::abi::{Decoder, Mode};
    use serde_json::json;

    let mut p = Program::new();
    let usd = p.declare_opaque("Money<USD>", Type::Int);
    let eur = p.declare_opaque("Money<EUR>", Type::Int);

    // Even on the erased fast path, the decoded values carry their nominal type.
    let d = Decoder::new(&p, Mode::Trusted);
    let a = d.decode("$", &Type::Opaque(usd), &json!(350)).unwrap();
    let b = d.decode("$", &Type::Opaque(eur), &json!(350)).unwrap();
    assert_ne!(a, b, "two currencies must not decode to the same value");

    // In debug mode a cross-wired payload is actively detected.
    let dbg = Decoder::new(&p, Mode::Debug);
    assert!(
        dbg.decode(
            "$",
            &Type::Opaque(usd),
            &json!({"$type":"Money<EUR>","value":350})
        )
        .is_err()
    );
}
