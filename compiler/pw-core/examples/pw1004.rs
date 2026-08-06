fn main() {
    use pw_core::diag::{MatchSite, render_non_exhaustive};
    use pw_core::exhaust::{Arm, Pattern, check_match};
    use pw_core::types::{Ctor, Program, Type};
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
    let ty = Type::Adt(id);
    let src = "fn label(state: OrderState) -> String !{panic} {\n    match state {\n        Draft => \"draft\",\n        Confirmed(ref) => ref,\n    }\n}\n";
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
    let mstart = src.find("match").unwrap();
    let mend = src.rfind("    }").unwrap() + 5;
    let sstart = src.find("state: OrderState").unwrap();
    let site = MatchSite {
        source: src,
        path: "order/label.pw",
        span: mstart..mend,
        scrutinee_span: Some(sstart..sstart + "state: OrderState".len()),
    };
    print!(
        "{}",
        render_non_exhaustive(&p, &ty, &report, &site, false).unwrap()
    );
}
