//! Effect-independent exhaustiveness checking.
//!
//! **This is the rule Koka provably does not enforce.** E0 measured that a
//! non-exhaustive match compiles cleanly in Koka whenever the enclosing
//! function's effect row admits `exn`, and fails only at runtime with
//! `pattern match failure`. ADR-0011 therefore moved the rule to `pw`:
//!
//! > `pw` performs exhaustiveness checking on its own typed pattern matrix,
//! > before lowering to Koka. It rejects an incomplete match **regardless of the
//! > function's effect row**.
//!
//! The algorithm is Maranget's usefulness relation ("Warnings for pattern
//! matching", JFP 2007), extended to build *witnesses* — concrete counterexample
//! patterns — because the required diagnostic names the missing cases:
//!
//! ```text
//! PW1004 Non-exhaustive match
//! Missing cases:
//! - Cancelled(CancellationReason)
//! - Failed(OrderFailure)
//! ```
//!
//! Two related properties fall out of the same machinery and are also reported:
//! unreachable arms, and (via witnesses) which constructor is missing rather
//! than merely that something is.

use crate::types::{Program, Type};

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Pattern {
    /// `_`, or a binding like `x` — both match anything and neither
    /// discriminates, so they are the same thing to this algorithm.
    Wildcard,
    /// `Ctor(p1, .., pn)`. `ctor` indexes the constructor list of the scrutinee
    /// type as returned by [`Program::ctors_of`].
    Ctor { ctor: usize, args: Vec<Pattern> },
    /// `p1 | p2`
    Or(Vec<Pattern>),
}

impl Pattern {
    pub fn ctor(ctor: usize, args: Vec<Pattern>) -> Self {
        Pattern::Ctor { ctor, args }
    }
    pub fn unit(ctor: usize) -> Self {
        Pattern::Ctor { ctor, args: vec![] }
    }
}

/// One arm of a `match`.
#[derive(Debug, Clone)]
pub struct Arm {
    pub pattern: Pattern,
    /// Byte range in the source, for diagnostics.
    pub span: std::ops::Range<usize>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Witness(pub Pattern);

#[derive(Debug, Clone)]
pub struct MatchReport {
    /// Concrete values the match does not cover. Empty means exhaustive.
    pub missing: Vec<Witness>,
    /// Arms that can never be reached because earlier arms already cover them.
    pub unreachable: Vec<usize>,
}

impl MatchReport {
    pub fn is_exhaustive(&self) -> bool {
        self.missing.is_empty()
    }
}

type Row = Vec<Pattern>;

/// Expand or-patterns so the core algorithm only sees wildcards and ctors.
fn expand(row: &Row) -> Vec<Row> {
    let mut out: Vec<Row> = vec![Vec::new()];
    for p in row {
        match p {
            Pattern::Or(alts) => {
                let mut next = Vec::new();
                for base in &out {
                    for a in alts {
                        let mut r = base.clone();
                        r.push(a.clone());
                        next.push(r);
                    }
                }
                out = next;
            }
            other => {
                for r in out.iter_mut() {
                    r.push(other.clone());
                }
            }
        }
    }
    out
}

/// Maranget's `S(c, P)` — keep rows whose head matches constructor `c`, and
/// splice that constructor's arguments into the front of the row.
fn specialize(matrix: &[Row], ctor: usize, arity: usize) -> Vec<Row> {
    let mut out = Vec::new();
    for row in matrix {
        // A row with nothing left in it constrains nothing, so it drops out.
        // See `is_useful` for why these are `else { continue }` and not
        // `expect`: a `.pw` program reached them and panicked the compiler.
        let Some((head, rest)) = row.split_first() else {
            continue;
        };
        match head {
            Pattern::Wildcard => {
                let mut r = vec![Pattern::Wildcard; arity];
                r.extend_from_slice(rest);
                out.push(r);
            }
            Pattern::Ctor { ctor: c, args } if *c == ctor => {
                // Padded to the arity the caller is specializing at, for the
                // same reason: the row and the type list must stay aligned
                // even when the program's pattern does not match the
                // constructor's declared shape.
                let mut r = args.clone();
                r.resize(arity, Pattern::Wildcard);
                r.extend_from_slice(rest);
                out.push(r);
            }
            Pattern::Ctor { .. } => {}
            Pattern::Or(_) => unreachable!("or-patterns are expanded before this point"),
        }
    }
    out
}

/// Maranget's `D(P)` — keep only rows whose head is a wildcard, dropping it.
fn default_matrix(matrix: &[Row]) -> Vec<Row> {
    let mut out = Vec::new();
    for row in matrix {
        let Some((head, rest)) = row.split_first() else {
            continue;
        };
        if matches!(head, Pattern::Wildcard) {
            out.push(rest.to_vec());
        }
    }
    out
}

/// Which constructor indices appear at the head of some row.
fn head_ctors(matrix: &[Row]) -> Vec<usize> {
    let mut seen = Vec::new();
    for row in matrix {
        if let Some(Pattern::Ctor { ctor, .. }) = row.first()
            && !seen.contains(ctor)
        {
            seen.push(*ctor);
        }
    }
    seen.sort_unstable();
    seen
}

/// Core recursion: values of `types` that no row of `matrix` matches.
///
/// Returns at most `limit` witnesses so a wide unmatched type does not produce
/// an unreadable diagnostic.
fn missing_witnesses(
    program: &Program,
    matrix: &[Row],
    types: &[Type],
    limit: usize,
) -> Vec<Vec<Pattern>> {
    if types.is_empty() {
        // No columns left: if any row survived, everything here is covered.
        return if matrix.is_empty() {
            vec![vec![]]
        } else {
            vec![]
        };
    }

    let head_ty = &types[0];
    let rest_ty = &types[1..];
    let used = head_ctors(matrix);

    match program.ctors_of(head_ty) {
        // Finite, enumerable constructor set.
        Some(all) if !head_ty.is_infinite() => {
            let complete = all.len() == used.len() && (0..all.len()).all(|i| used.contains(&i));

            if complete {
                // Every constructor is present: recurse into each.
                let mut out = Vec::new();
                for (idx, c) in all.iter().enumerate() {
                    let sub = specialize(matrix, idx, c.fields.len());
                    let mut sub_types = c.fields.clone();
                    sub_types.extend_from_slice(rest_ty);
                    for w in missing_witnesses(program, &sub, &sub_types, limit) {
                        let (args, tail) = w.split_at(c.fields.len());
                        let mut row = vec![Pattern::ctor(idx, args.to_vec())];
                        row.extend_from_slice(tail);
                        out.push(row);
                        if out.len() >= limit {
                            return out;
                        }
                    }
                }
                out
            } else {
                // At least one constructor is absent. Every absent constructor is
                // a witness — this is what makes the diagnostic able to say
                // "Missing cases: Cancelled(..), Failed(..)" instead of just
                // "not exhaustive".
                let sub = default_matrix(matrix);
                let tails = missing_witnesses(program, &sub, rest_ty, limit);
                if tails.is_empty() {
                    return vec![];
                }
                let mut out = Vec::new();
                for (idx, c) in all.iter().enumerate() {
                    if used.contains(&idx) {
                        continue;
                    }
                    for tail in &tails {
                        let mut row =
                            vec![Pattern::ctor(idx, vec![Pattern::Wildcard; c.fields.len()])];
                        row.extend_from_slice(tail);
                        out.push(row);
                        if out.len() >= limit {
                            return out;
                        }
                    }
                }
                out
            }
        }
        // Infinite or opaque: a complete signature is impossible, so coverage
        // requires a wildcard row.
        _ => {
            let sub = default_matrix(matrix);
            let tails = missing_witnesses(program, &sub, rest_ty, limit);
            tails
                .into_iter()
                .take(limit)
                .map(|tail| {
                    let mut row = vec![Pattern::Wildcard];
                    row.extend_from_slice(&tail);
                    row
                })
                .collect()
        }
    }
}

/// Is `row` useful with respect to `matrix` — does it match anything the matrix
/// does not already match? An arm that is not useful is unreachable.
fn is_useful(program: &Program, matrix: &[Row], row: &Row, types: &[Type]) -> bool {
    if types.is_empty() {
        return matrix.is_empty();
    }
    let head_ty = &types[0];
    let rest_ty = &types[1..];
    // A row shorter than the type list matches nothing, so it is not useful.
    // This used to be `.expect("useful on empty row")`, and a generality
    // counterexample reached it — the compiler PANICKED on a `.pw` program.
    // A crash is a worse failure than a missed diagnostic: it takes every
    // other rule down with it and reports nothing at all.
    let Some((head, tail)) = row.split_first() else {
        return false;
    };

    match head {
        Pattern::Ctor { ctor, args } => {
            let ctors = program.ctors_of(head_ty).unwrap_or_default();
            let field_types = ctors
                .get(*ctor)
                .map(|c| c.fields.clone())
                .unwrap_or_else(|| vec![Type::Bool; args.len()]);
            // The pattern's arity and the constructor's declared field count
            // can disagree — `Cancelled` written without its argument, or a
            // constructor resolved to the wrong index. The row and the type
            // list must stay the same length whatever the program says, so
            // the pattern is padded with wildcards or truncated to fit.
            let arity = field_types.len();
            let mut args = args.clone();
            args.resize(arity, Pattern::Wildcard);

            let sub = specialize(matrix, *ctor, arity);
            let mut sub_row = args;
            sub_row.extend_from_slice(tail);
            let mut sub_types = field_types;
            sub_types.extend_from_slice(rest_ty);
            is_useful(program, &sub, &sub_row, &sub_types)
        }
        Pattern::Wildcard => {
            let used = head_ctors(matrix);
            match program.ctors_of(head_ty) {
                Some(all) if !head_ty.is_infinite() && all.len() == used.len() => {
                    // Complete signature: a wildcard is useful only if it is
                    // useful under some constructor.
                    for (idx, c) in all.iter().enumerate() {
                        let sub = specialize(matrix, idx, c.fields.len());
                        let mut sub_row = vec![Pattern::Wildcard; c.fields.len()];
                        sub_row.extend_from_slice(tail);
                        let mut sub_types = c.fields.clone();
                        sub_types.extend_from_slice(rest_ty);
                        if is_useful(program, &sub, &sub_row, &sub_types) {
                            return true;
                        }
                    }
                    false
                }
                _ => is_useful(program, &default_matrix(matrix), &tail.to_vec(), rest_ty),
            }
        }
        Pattern::Or(_) => unreachable!("or-patterns are expanded before this point"),
    }
}

/// Check a single-scrutinee `match`.
///
/// Note the signature: there is no effect row anywhere in it. That is the point
/// — the result cannot depend on whether the enclosing function may raise.
pub fn check_match(program: &Program, scrutinee: &Type, arms: &[Arm]) -> MatchReport {
    check_match_multi(program, std::slice::from_ref(scrutinee), arms)
}

/// Check a match over a tuple of scrutinees (each arm's pattern must be a
/// `Ctor` of the synthetic tuple constructor, or a wildcard).
pub fn check_match_multi(program: &Program, scrutinees: &[Type], arms: &[Arm]) -> MatchReport {
    let types: Vec<Type> = scrutinees.to_vec();

    let mut matrix: Vec<Row> = Vec::new();
    let mut unreachable = Vec::new();

    for (i, arm) in arms.iter().enumerate() {
        let rows = expand(&vec![arm.pattern.clone()]);
        let mut any_useful = false;
        for r in &rows {
            if is_useful(program, &matrix, r, &types) {
                any_useful = true;
            }
        }
        if !any_useful {
            unreachable.push(i);
        }
        matrix.extend(rows);
    }

    let missing = missing_witnesses(program, &matrix, &types, 8)
        .into_iter()
        .filter_map(|mut w| w.pop().or(None).map(Witness).or(None))
        .collect::<Vec<_>>();

    // `missing_witnesses` returns one pattern per column; with a single
    // scrutinee that is one pattern per witness.
    let missing = if scrutinees.len() == 1 {
        missing
    } else {
        missing_witnesses(program, &matrix, &types, 8)
            .into_iter()
            .map(|w| Witness(Pattern::Ctor { ctor: 0, args: w }))
            .collect()
    };

    MatchReport {
        missing,
        unreachable,
    }
}

/// Render a witness the way it should appear in a diagnostic.
pub fn render_witness(program: &Program, ty: &Type, w: &Witness) -> String {
    fn go(program: &Program, ty: &Type, p: &Pattern) -> String {
        match p {
            Pattern::Wildcard => "_".to_string(),
            Pattern::Or(_) => "_".to_string(),
            Pattern::Ctor { ctor, args } => {
                let ctors = program.ctors_of(ty).unwrap_or_default();
                let Some(c) = ctors.get(*ctor) else {
                    return "_".to_string();
                };
                if args.is_empty() {
                    return c.name.clone();
                }
                let rendered: Vec<String> = args
                    .iter()
                    .zip(c.fields.iter())
                    .map(|(a, t)| match a {
                        // A wildcard argument is shown as its TYPE, which reads
                        // better in a missing-cases list: `Cancelled(Reason)`
                        // rather than `Cancelled(_)`.
                        Pattern::Wildcard => program.type_name(t),
                        other => go(program, t, other),
                    })
                    .collect();
                format!("{}({})", c.name, rendered.join(", "))
            }
        }
    }
    go(program, ty, &w.0)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::types::Ctor;

    fn order_state(p: &mut Program) -> Type {
        let reason = p.declare_adt(
            "CancellationReason",
            vec![
                Ctor {
                    name: "OutOfStock".into(),
                    fields: vec![],
                },
                Ctor {
                    name: "UserCancelled".into(),
                    fields: vec![],
                },
            ],
        );
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
                    name: "Confirmed".into(),
                    fields: vec![Type::Str],
                },
                Ctor {
                    name: "Cancelled".into(),
                    fields: vec![Type::Adt(reason)],
                },
                Ctor {
                    name: "Failed".into(),
                    fields: vec![Type::Str],
                },
            ],
        );
        Type::Adt(id)
    }

    fn arm(p: Pattern) -> Arm {
        Arm {
            pattern: p,
            span: 0..0,
        }
    }

    #[test]
    fn complete_match_is_exhaustive() {
        let mut p = Program::new();
        let ty = order_state(&mut p);
        let arms = vec![
            arm(Pattern::unit(0)),
            arm(Pattern::unit(1)),
            arm(Pattern::ctor(2, vec![Pattern::Wildcard])),
            arm(Pattern::ctor(3, vec![Pattern::Wildcard])),
            arm(Pattern::ctor(4, vec![Pattern::Wildcard])),
        ];
        let r = check_match(&p, &ty, &arms);
        assert!(r.is_exhaustive(), "{:?}", r.missing);
        assert!(r.unreachable.is_empty());
    }

    #[test]
    fn a_pattern_whose_arity_disagrees_with_its_constructor_does_not_crash() {
        // Found by a generality counterexample, which panicked the whole
        // compiler rather than reporting anything. `Cancelled` is declared
        // with one field and written here with none; the row and the type
        // list must stay the same length whatever the program says.
        let mut p = Program::new();
        let reason = p.declare_adt(
            "Reason",
            vec![Ctor {
                name: "Late".into(),
                fields: vec![],
            }],
        );
        let state = p.declare_adt(
            "State",
            vec![
                Ctor {
                    name: "Placed".into(),
                    fields: vec![],
                },
                Ctor {
                    name: "Cancelled".into(),
                    fields: vec![Type::Adt(reason)],
                },
            ],
        );
        let report = check_match(
            &p,
            &Type::Adt(state),
            &[
                arm(Pattern::Ctor {
                    ctor: 0,
                    args: vec![],
                }),
                // One field declared, none written.
                arm(Pattern::Ctor {
                    ctor: 1,
                    args: vec![],
                }),
            ],
        );
        // The point is that this returns at all. Both constructors are
        // mentioned, so nothing is missing.
        assert!(report.missing.is_empty(), "{:?}", report.missing);

        // And the opposite mismatch — more arguments than fields.
        let report = check_match(
            &p,
            &Type::Adt(state),
            &[arm(Pattern::Ctor {
                ctor: 0,
                args: vec![Pattern::Wildcard, Pattern::Wildcard],
            })],
        );
        assert!(!report.missing.is_empty(), "`Cancelled` is still missing");
    }

    #[test]
    fn missing_variants_are_named_not_merely_counted() {
        // The exact case from the architect's PW1004 example.
        let mut p = Program::new();
        let ty = order_state(&mut p);
        let arms = vec![
            arm(Pattern::unit(0)),
            arm(Pattern::unit(1)),
            arm(Pattern::ctor(2, vec![Pattern::Wildcard])),
        ];
        let r = check_match(&p, &ty, &arms);
        assert!(!r.is_exhaustive());
        let names: Vec<String> = r
            .missing
            .iter()
            .map(|w| render_witness(&p, &ty, w))
            .collect();
        assert!(
            names.contains(&"Cancelled(CancellationReason)".to_string()),
            "{names:?}"
        );
        assert!(names.contains(&"Failed(String)".to_string()), "{names:?}");
        assert_eq!(names.len(), 2);
    }

    #[test]
    fn wildcard_makes_any_match_exhaustive() {
        let mut p = Program::new();
        let ty = order_state(&mut p);
        let arms = vec![arm(Pattern::unit(0)), arm(Pattern::Wildcard)];
        assert!(check_match(&p, &ty, &arms).is_exhaustive());
    }

    #[test]
    fn nested_patterns_find_nested_gaps() {
        // `Cancelled(OutOfStock)` handled, `Cancelled(UserCancelled)` not.
        let mut p = Program::new();
        let ty = order_state(&mut p);
        let arms = vec![
            arm(Pattern::unit(0)),
            arm(Pattern::unit(1)),
            arm(Pattern::ctor(2, vec![Pattern::Wildcard])),
            arm(Pattern::ctor(3, vec![Pattern::unit(0)])),
            arm(Pattern::ctor(4, vec![Pattern::Wildcard])),
        ];
        let r = check_match(&p, &ty, &arms);
        assert!(!r.is_exhaustive());
        let names: Vec<String> = r
            .missing
            .iter()
            .map(|w| render_witness(&p, &ty, w))
            .collect();
        assert_eq!(names, vec!["Cancelled(UserCancelled)".to_string()]);
    }

    #[test]
    fn unreachable_arms_are_reported() {
        let mut p = Program::new();
        let ty = order_state(&mut p);
        let arms = vec![
            arm(Pattern::Wildcard),
            arm(Pattern::unit(0)), // dead: the wildcard already covered it
        ];
        let r = check_match(&p, &ty, &arms);
        assert!(r.is_exhaustive());
        assert_eq!(r.unreachable, vec![1]);
    }

    #[test]
    fn or_patterns_count_toward_coverage() {
        let mut p = Program::new();
        let ty = order_state(&mut p);
        let arms = vec![
            arm(Pattern::Or(vec![Pattern::unit(0), Pattern::unit(1)])),
            arm(Pattern::ctor(2, vec![Pattern::Wildcard])),
            arm(Pattern::ctor(3, vec![Pattern::Wildcard])),
            arm(Pattern::ctor(4, vec![Pattern::Wildcard])),
        ];
        assert!(check_match(&p, &ty, &arms).is_exhaustive());
    }

    #[test]
    fn bool_needs_both_arms() {
        let p = Program::new();
        let only_true = vec![Arm {
            pattern: Pattern::unit(1),
            span: 0..0,
        }];
        let r = check_match(&p, &Type::Bool, &only_true);
        assert!(!r.is_exhaustive());
        assert_eq!(render_witness(&p, &Type::Bool, &r.missing[0]), "false");
    }

    #[test]
    fn infinite_types_always_need_a_wildcard() {
        let p = Program::new();
        // No finite constructor set exists for Int, so an arm list without a
        // wildcard can never be exhaustive.
        let r = check_match(&p, &Type::Int, &[]);
        assert!(!r.is_exhaustive());
        let r2 = check_match(
            &p,
            &Type::Int,
            &[Arm {
                pattern: Pattern::Wildcard,
                span: 0..0,
            }],
        );
        assert!(r2.is_exhaustive());
    }

    #[test]
    fn opaque_types_cannot_be_matched_structurally() {
        // A StoreId is a String at runtime, but matching it as one is not
        // possible, so only a wildcard covers it.
        let mut p = Program::new();
        let sid = p.declare_opaque("StoreId", Type::Str);
        let ty = Type::Opaque(sid);
        assert!(!check_match(&p, &ty, &[]).is_exhaustive());
        assert!(
            check_match(
                &p,
                &ty,
                &[Arm {
                    pattern: Pattern::Wildcard,
                    span: 0..0
                }]
            )
            .is_exhaustive()
        );
    }

    #[test]
    fn empty_arm_list_over_a_finite_type_reports_every_constructor() {
        let mut p = Program::new();
        let ty = order_state(&mut p);
        let r = check_match(&p, &ty, &[]);
        assert_eq!(r.missing.len(), 5);
    }

    #[test]
    fn recursive_list_type_terminates_and_reports_cons() {
        let mut p = Program::new();
        let list = p.declare_list(Type::Int);
        let ty = Type::Adt(list);
        // Only `Nil` handled.
        let arms = vec![Arm {
            pattern: Pattern::unit(0),
            span: 0..0,
        }];
        let r = check_match(&p, &ty, &arms);
        assert!(!r.is_exhaustive());
        let names: Vec<String> = r
            .missing
            .iter()
            .map(|w| render_witness(&p, &ty, w))
            .collect();
        assert_eq!(names, vec!["Cons(Int, List<Int>)".to_string()]);
    }
}
