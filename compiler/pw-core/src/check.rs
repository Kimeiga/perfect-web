//! Running `pw-core`'s algorithms over real `.pw` source.
//!
//! `docs/NEXT.md` item 7. The algorithms in [`crate::exhaust`],
//! [`crate::types`], [`crate::scope`] and [`crate::abi`] were written and
//! tested against hand-built inputs; this is what feeds them HIR built from
//! source. It consumes ids and spans only — never syntax nodes (ADR-0012).
//!
//! # Scope of what is checked
//!
//! Deliberately narrow, and narrow in a direction that cannot produce false
//! positives: a `match` is checked **only** when its scrutinee type is known
//! from a signature or an annotated binding *in the same program*. Anything
//! else is skipped in silence. A checker that guessed a type in order to
//! report more would be reporting fiction, and the corpus exists to catch
//! exactly that.

use std::collections::{BTreeMap, BTreeSet};

use crate::diagnostics::{Detector, Diagnostic, Related, Repair, Severity};
use crate::exhaust::{self, Arm, Pattern as EPat};
use crate::hir::{self, AttrValue, Body, Decl, DeclKind, Expr, ExprId, Hir, Node, Pattern as HPat};
use crate::placement::{ALL_WORLDS, Demand, Placements, World, solve};
use crate::privacy::{Label, Restriction};
use crate::scope::{HandleKind, Op, ScopeGraph, ScopeKind, ScopeViolation};
use crate::signatures::Signatures;
use crate::types::{Ctor, Program, Type};

/// One source file that has been parsed and lowered.
pub struct Unit {
    pub path: String,
    pub src: String,
    pub hir: Hir,
}

/// The declared types of every unit being checked together.
///
/// **The set of files passed to one invocation is treated as one program.**
/// Import-based visibility is not enforced yet — that is name resolution, and
/// it does not exist. The corpus depends on this: `R-007` matches on
/// `OrderState`, which is declared in `A-002`. Recorded as assumption A-009.
pub struct Env {
    program: Program,
    /// ADT name → its `AdtId`.
    adts: BTreeMap<String, usize>,
    /// ADT name → constructor names, in declaration order.
    ctor_names: BTreeMap<String, Vec<String>>,
}

impl Env {
    pub fn build(units: &[Unit]) -> Env {
        let mut program = Program::new();
        let mut adts = BTreeMap::new();
        let mut ctor_names = BTreeMap::new();

        // Two passes: every ADT is declared before any field type is resolved,
        // so mutually recursive types work and declaration order does not
        // change the result.
        let mut pending: Vec<(String, Vec<hir::VariantDef>)> = Vec::new();
        for u in units {
            for (_, d) in u.hir.all_decls() {
                if let Some(vs) = &d.variants {
                    pending.push((d.name.clone(), vs.clone()));
                }
                if let Some(rep) = &d.opaque_of {
                    program.declare_opaque(&d.name, primitive(rep).unwrap_or(Type::Str));
                }
            }
        }
        for (name, _) in &pending {
            let id = program.declare_adt(name, Vec::new());
            adts.insert(name.clone(), id);
        }
        // A field type that is neither primitive nor declared here — usually
        // one reached through an `import` — is nominal-but-unknown, which is
        // exactly an opaque type. Recording it by name rather than collapsing
        // it to a placeholder is what lets a witness read
        // `Cancelled(CancellationReason)` instead of `Cancelled(String)`, and
        // R-007 declares that exact string as its expected error.
        let mut unresolved: BTreeMap<String, usize> = BTreeMap::new();
        for (name, vs) in &pending {
            let id = adts[name];
            let ctors: Vec<Ctor> = vs
                .iter()
                .map(|v| Ctor {
                    name: v.name.clone(),
                    fields: v
                        .fields
                        .iter()
                        .map(|f| {
                            if let Some(t) = primitive(f) {
                                return t;
                            }
                            if let Some(i) = adts.get(f) {
                                return Type::Adt(*i);
                            }
                            let oid = *unresolved
                                .entry(f.clone())
                                .or_insert_with(|| program.declare_opaque(f, Type::Str));
                            Type::Opaque(oid)
                        })
                        .collect(),
                })
                .collect();
            program.adts[id].ctors = ctors;
            ctor_names.insert(name.clone(), vs.iter().map(|v| v.name.clone()).collect());
        }

        Env {
            program,
            adts,
            ctor_names,
        }
    }

    pub fn program(&self) -> &Program {
        &self.program
    }

    fn adt_of(&self, ty_name: &str) -> Option<usize> {
        self.adts.get(ty_name).copied()
    }
}

fn primitive(name: &str) -> Option<Type> {
    match name {
        "Bool" => Some(Type::Bool),
        "Int" => Some(Type::Int),
        "String" | "Str" => Some(Type::Str),
        _ => None,
    }
}

/// Check every unit against the shared environment.
pub fn check_units(units: &[Unit]) -> Vec<(String, Vec<Diagnostic>)> {
    let env = Env::build(units);

    // E2B: one workspace module graph, built before any semantic analysis.
    // Resolution failures are reported per unit, so a file importing a module
    // that does not exist says so instead of failing later in a checker that
    // cannot explain why.
    let hirs: Vec<&Hir> = units.iter().map(|u| &u.hir).collect();
    let workspace = crate::resolve::Workspace::build(&hirs);
    // E2C: one set of resolved signatures, shared by every analysis.
    let sigs = Signatures::build(&workspace, &hirs);
    // E2D: effects inferred over the whole program, so a helper declared in
    // another module still contributes to its caller's row.
    let mut inference = crate::effects::Inference::new(&sigs, &workspace);
    inference.run(&hirs);
    // E7: what the whole program says about a type — is it a resource, and is
    // it produced only by a scoped declaration. Both are needed before any one
    // file can be asked what its handlers may capture.
    let manifest = crate::boundary::TypeFacts::build(&hirs, &sigs);
    // E8 step 2: what a capability's type argument may name, PER UNIT.
    //
    // Whole-program until 2026-08-07, which was assumption A-017 and is now
    // retired: an argument resolves from where it was written, exactly as a
    // type in an expression does.
    let visible_types: Vec<std::collections::BTreeSet<String>> = (0..units.len())
        .map(|u| crate::ontology::argument_names_visible_from(&workspace, u, &hirs))
        .collect();
    // E8 slice 2: what each effect a row names actually IS. One ontology for
    // the whole program, beside the signatures, because both answer questions
    // about the same rows.
    let ontology = crate::ontology::Ontology::build_with(&hirs, &workspace);
    // E6: every route the program declares, so a link can be checked against
    // what exists rather than against a naming convention.
    let routes = crate::routes::table(&hirs);
    // E6: the resource dependency graph, over every unit at once. A fragment
    // in one module depends on a query in another, so a per-file graph would
    // report a dangling edge for exactly the case the milestone is about.
    let graph = crate::graph::Graph::build(&hirs, &workspace);
    let mut resolution: BTreeMap<usize, Vec<Diagnostic>> = BTreeMap::new();
    for e in &workspace.errors {
        resolution
            .entry(e.unit)
            .or_default()
            .push(resolve_diagnostic(e));
    }
    for (i, u) in units.iter().enumerate() {
        let per_unit = resolution.entry(i).or_default();
        per_unit.extend(unresolved_uses(&workspace, i, &u.hir));
        // Every effect row, against the declarations. Reported beside the
        // resolution failures because that is what it is: a name in a row that
        // resolves to nothing is the same class of mistake as a name in an
        // expression that does.
        crate::ontology::check_effect_rows(&ontology, &workspace, i, &u.hir, per_unit);
        // And the declarations themselves: an impact condition naming nothing
        // makes a facet silently inapplicable.
        crate::ontology::check_effect_declarations(&ontology, per_unit, i);
        // E10, ADR-0025: an optimistic transition is a separate execution root
        // with its own effect row, and its context permits none.
        named_roots_respect_their_context(&inference, i, &u.hir, per_unit);
        optimistic_transitions_agree_with_their_target(&sigs, &workspace, &hirs, i, per_unit);
    }

    // Declaration name → its privacy label, across every unit. A `page` in one
    // file renders a `session query` declared in another, and that is the
    // corpus's canonical private-in-shared-cache case (assumption A-009).
    // Keyed by RESOLVED IDENTITY, not by spelling.
    //
    // It was `BTreeMap<String, Label>` keyed by bare declaration name, so a
    // page rendering `Cart` was given the join of EVERY `Cart` in the program.
    // Joining is the safe direction — an unrelated declaration could only make
    // a page look more private, never less — which is exactly why it never
    // produced a visibly wrong answer and stayed in place. It is still
    // `docs/RISK_QUEUE.md` 34's shape: meaning resolved from a spelling.
    let mut labels: BTreeMap<crate::resolve::DefId, Label> = BTreeMap::new();
    for (unit, u) in units.iter().enumerate() {
        for (id, d) in u.hir.all_decls() {
            let l = label_of(d);
            if !l.is_public() {
                labels.insert(crate::resolve::DefId { unit, decl: id.0 }, l);
            }
        }
    }
    units
        .iter()
        .enumerate()
        .map(|(i, u)| {
            let mut out = resolution.remove(&i).unwrap_or_default();
            out.extend(check_unit_with(
                &env,
                &labels,
                &sigs,
                &inference,
                &manifest,
                &routes,
                &graph,
                &ontology,
                &workspace,
                i,
                &visible_types[i],
                u,
            ));
            out.sort_by_key(|d| d.primary_span.start);
            (u.path.clone(), out)
        })
        .collect()
}

/// Uses of a name nothing declares.
///
/// Import failures alone are not enough. `R-004` materializes a `session query
/// Cart` declared in a file it never imports: with only import checking, that
/// is silence — and silence is what let the fixture look uncaught rather than
/// unresolved.
///
/// Deliberately conservative about what counts as a use: a bare name that is
/// not a local binding, and a qualified path whose head names a module. A
/// method call on a value (`line.item_name`) is a field access, not a path, and
/// is left alone until types exist.
fn unresolved_uses(
    workspace: &crate::resolve::Workspace,
    unit: usize,
    hir: &Hir,
) -> Vec<Diagnostic> {
    use crate::resolve::Resolution;

    let mut out = Vec::new();
    for (_, decl) in hir.all_decls() {
        let Some(body_id) = decl.body else { continue };
        let body = hir.body(body_id);

        let mut in_scope = crate::resolve::local_bindings(body);
        in_scope.extend(decl.params.iter().map(|p| p.name.clone()));
        // A declaration can call itself and its siblings.
        in_scope.extend(hir.all_decls().map(|(_, d)| d.name.clone()));

        // **The body, PLUS every embedded transition.**
        //
        // Architect ruling, 2026-08-10:
        //
        // > Being inside a policy never exempts an executable term from
        // > resolution. […] There should not be a separate
        // > "optimistic-expression resolver."
        //
        // A transition is not reachable from `body.root` — deliberately, so
        // the declaration's effect row does not absorb code that runs in
        // another execution context — so it is added here by name rather than
        // found by walking. It is the SAME rule below, not a second one.
        let mut reachable = body.walk();
        for (policy, root) in decl.term_roots() {
            reachable.extend(body.walk_from(root.root));

            // **Every name inside a named root must come from somewhere.**
            //
            // Architect ruling, 2026-08-10, locking the semantic requirement:
            //
            // > Every identifier visible inside optimistic/rollback code must
            // > arise from ordinary lexical scope or from an explicit binder in
            // > the construct itself. No special-name lookup.
            //
            // The binders are the ROOT's own, so they scope to it and nothing
            // else. An optimistic clause's target is a separate root with no
            // binders — it is evaluated before the binding exists, so
            // `Cart(cart)` names nothing, which is correct.
            let mut scope = in_scope.clone();
            scope.extend(root.binders.iter().map(|(n, _)| n.clone()));
            scope.extend(crate::resolve::local_bindings_from(body, root.root));

            for nid in body.walk_from(root.root) {
                let Expr::Name(n) = body.expr(nid) else {
                    continue;
                };
                if scope.contains(n)
                    || crate::resolve::INTRINSIC_CALLS.contains(&n.as_str())
                    || !matches!(
                        workspace.resolve(unit, n),
                        crate::resolve::Resolution::Unresolved
                    )
                {
                    continue;
                }
                out.push(unresolved_in_policy_term(
                    hir,
                    decl,
                    body,
                    nid,
                    n,
                    &policy.name,
                ));
            }
        }

        for id in reachable {
            let Expr::Call { callee, .. } = body.expr(id) else {
                continue;
            };
            let path = path_of(body, *callee);
            let Some((head, _)) = path.split_once('.') else {
                // **A BARE call.**
                //
                // Examined here since 2026-08-10, and not before — which is why
                // four accepted programs called names that do not exist and
                // checked clean since E4. Every analysis produces an answer for
                // a call it cannot resolve: inference contributes no effects,
                // privacy no label, placement no constraint, the contract no
                // capability. Each of those is indistinguishable from *this
                // call is harmless*.
                //
                // The subject is narrow on purpose. Two earlier attempts at
                // this rule reported sixteen and then eleven corpus errors and
                // were reverted rather than landed over-broad, because a
                // call-shaped construct can have real meaning that resolution
                // does not supply: a language constructor, a policy operator, a
                // CSS value function. `tests/semantic_ownership.rs` is the gate
                // that made the class small enough to report — it names every
                // owner, and the residue it leaves is what this reports.
                if bare_call_is_unowned(workspace, unit, &path, &in_scope) {
                    out.push(unresolved_bare_call(hir, decl, body, id, &path));
                }
                continue;
            };
            // Only a head that looks like a module: either the workspace has
            // it, or it is capitalised and is not a local. Anything else is a
            // field access on a value.
            let known_module = workspace.sees_module(unit, head);
            let module_shaped = head.chars().next().is_some_and(char::is_uppercase)
                || matches!(
                    head,
                    "secrets" | "style" | "clock" | "log" | "database" | "dom"
                );
            if in_scope.contains(head) || (!known_module && !module_shaped) {
                continue;
            }
            // `StoreError.DecodeFailed` is a CONSTRUCTOR on a type in scope,
            // not a member of a module. Both are written `Head.member`, and
            // treating the second as the first reported every union constructor
            // in the corpus as undeclared.
            if !known_module
                && matches!(
                    workspace.resolve(unit, head),
                    Resolution::Local(_) | Resolution::Imported { .. }
                )
            {
                continue;
            }
            if workspace.resolve_path(unit, &path) != Resolution::Unresolved {
                continue;
            }

            out.push(Diagnostic {
                code: crate::codes::UNRESOLVED_NAME.id,
                invariant: crate::codes::UNRESOLVED_NAME.invariant,
                reason: "unresolved_use",
                detector: Detector::DeclarationRule,
                severity: Severity::Error,
                message: format!("`{path}` does not resolve"),
                primary_span: body.expr_span(id),
                related: vec![Related {
                    span: hir.decl_span(decl_id_of(hir, decl)),
                    label: format!("used inside `{}`", decl.name),
                }],
                explanation: Some(format!(
                    "`{head}` is not a module this file can see. Names are visible \
                     through lexical scope, module membership, or an explicit import \
                     — external implementation is allowed, a missing declaration is \
                     not."
                )),
                repairs: vec![Repair {
                    description: format!("add `import {head}`, or declare it"),
                    replacement: None,
                }],
            });
        }
    }
    out
}

/// **An optimistic transition performs nothing.**
///
/// Architect ruling, 2026-08-11 (ADR-0025):
///
/// > The desired shape is `Cart → Cart`, not `Cart → arbitrary client program
/// > with arbitrary effects`. […] That gives automatic rollback meaningful
/// > semantics. An arbitrary externally visible effect cannot generally be
/// > undone by restoring the resource value.
///
/// The transition is its own execution root, so its row is inferred
/// independently of the declaration's body — `infer_rooted`. Neither absorbs
/// the other's effects, which is what makes this checkable at all: a
/// transition merged into the command's row would be indistinguishable from
/// the command performing them itself.
fn named_roots_respect_their_context(
    inference: &crate::effects::Inference<'_>,
    unit: usize,
    hir: &Hir,
    out: &mut Vec<Diagnostic>,
) {
    for (_, decl) in hir.all_decls() {
        let Some(body_id) = decl.body else { continue };
        let body = hir.body(body_id);
        // **Every root, asked its own context's question.** Not a rule keyed on
        // the policy's spelling: the context says whether effects are permitted
        // there, and `optimistic` is currently the only one that says no.
        for (policy, root) in decl.term_roots() {
            if root.context.permits_effects() {
                continue;
            }
            let inferred = inference.infer_rooted(unit, body, root.root);
            if inferred.effects.is_empty() {
                continue;
            }
            let performed: Vec<String> = inferred.effects.iter().cloned().collect();
            out.push(Diagnostic {
                code: crate::codes::OPTIMISTIC_NOT_PURE.id,
                invariant: crate::codes::OPTIMISTIC_NOT_PURE.invariant,
                reason: "optimistic_transition_performs_effects",
                detector: Detector::DeclarationRule,
                severity: Severity::Error,
                message: format!(
                    "`{}`'s optimistic transition performs {}",
                    decl.name,
                    performed.join(", ")
                ),
                primary_span: body.expr_span(root.root),
                related: vec![Related {
                    span: policy.span.clone(),
                    label: "declared here".to_string(),
                }],
                explanation: Some(
                    "An optimistic transition is a pure function from a resource's current \
                     value to its speculative one. It runs on the client, before the round \
                     trip, and is discarded if the command fails — and the platform discards \
                     it by restoring the value it held. An externally visible effect cannot \
                     be undone that way, so a transition that performs one has no defined \
                     behaviour on rejection."
                        .to_string(),
                ),
                repairs: vec![Repair {
                    description: "move the effect into the command's body, which runs once and \
                                  authoritatively"
                        .to_string(),
                    replacement: None,
                }],
            });
        }
    }
}

/// **The transition produces the value type of the resource it targets.**
///
/// Architect ruling, 2026-08-11, treating this as blocking before codegen:
///
/// ```text
/// target              ResourceEntry<Cart>
/// binder `cart`       Cart
/// transition result   Cart
/// ```
///
/// # Target selection and state transformation are different computations
///
/// The ruling is explicit that these must not be conflated:
///
/// > The purity requirement belongs to the transformation. The target selector
/// > may legitimately need context to identify the entry — `current_session()`
/// > is the obvious example — without making the actual state transformation
/// > effectful. So don't accidentally implement
/// > `effects(target) ∪ effects(transition) must be {}`.
///
/// So `optimistic_transitions_are_pure` infers from `t.body` alone, and this
/// function does not look at the target's effects at all. It asks the target
/// one question — which resource, and what is its value type — and then asks
/// the transition whether it produces that.
fn optimistic_transitions_agree_with_their_target(
    sigs: &Signatures,
    workspace: &crate::resolve::Workspace,
    hirs: &[&Hir],
    unit: usize,
    out: &mut Vec<Diagnostic>,
) {
    let hir = hirs[unit];
    for (_, decl) in hir.all_decls() {
        let Some(body_id) = decl.body else { continue };
        let body = hir.body(body_id);
        let module = hir.module_of(decl_id_of(hir, decl)).map(str::to_string);
        for (policy, target, transition) in decl.optimistic_clauses() {
            // The target names a resource. `Cart(current_session())` — the
            // callee, not the argument.
            let Expr::Call { callee, .. } = body.expr(target.root) else {
                out.push(target_is_not_a_resource_entry(
                    decl,
                    body,
                    target,
                    policy,
                    "it is not a resource applied to a key",
                ));
                continue;
            };
            let path = path_of(body, *callee);
            let Some(value_ty) = resource_value_type(sigs, workspace, hirs, unit, &path) else {
                out.push(target_is_not_a_resource_entry(
                    decl,
                    body,
                    target,
                    policy,
                    &format!("`{path}` is not a resource this file can see"),
                ));
                continue;
            };

            // The binder IS the target's value type; the transition must
            // produce it. Seeded rather than inferred — nothing in the body
            // says what `cart` is, because the clause's header does.
            let mut types = crate::infer::Types::of_body(sigs, decl, body, module.as_deref());
            for (name, _) in &transition.binders {
                types = types.with_binding(name, &value_ty);
            }
            let Some(produced) = types.of(body, transition.root) else {
                // No answer is not a violation. `docs/RISK_QUEUE.md`: an
                // analysis that could not run must not be read as a proof, and
                // it must not be read as a refutation either.
                continue;
            };
            if produced == value_ty {
                continue;
            }
            out.push(Diagnostic {
                code: crate::codes::OPTIMISTIC_TARGET_MISMATCH.id,
                invariant: crate::codes::OPTIMISTIC_TARGET_MISMATCH.invariant,
                reason: "optimistic_transition_type_mismatch",
                detector: Detector::DeclarationRule,
                severity: Severity::Error,
                message: format!(
                    "`{}`'s optimistic transition produces `{produced}`, but it targets a \
                     resource whose value is `{value_ty}`",
                    decl.name
                ),
                primary_span: body.expr_span(transition.root),
                related: vec![Related {
                    span: body.expr_span(target.root),
                    label: format!("this resource holds `{value_ty}`"),
                }],
                explanation: Some(format!(
                    "An optimistic transition replaces the value the client is displaying, and \
                     the platform abandons it by restoring the value it held. Both are \
                     `{value_ty}`. A transition producing `{produced}` would put something else \
                     in that entry, and there would be nothing meaningful to restore."
                )),
                repairs: vec![Repair {
                    description: format!(
                        "produce a `{value_ty}` from `{}`",
                        transition
                            .binders
                            .first()
                            .map(|(n, _)| n.as_str())
                            .unwrap_or("the bound value")
                    ),
                    replacement: None,
                }],
            });
        }
    }
}

/// The VALUE a resource holds: `Cart` for `query Cart(..) -> Result<Cart, E>`.
///
/// The `Ok` side, because that is what the client displays and what an
/// optimistic transition replaces. A `Result` in the entry would make the
/// speculative value a different shape from the held one.
fn resource_value_type(
    sigs: &Signatures,
    workspace: &crate::resolve::Workspace,
    hirs: &[&Hir],
    unit: usize,
    path: &str,
) -> Option<String> {
    use crate::resolve::{Namespace, Resolution};
    // **A bare name resolves in the TERM namespace**, not in whichever
    // namespace answers first.
    //
    // A resource is a data operation and lives there. `examples/store/app.pw`
    // declares `session query Cart(..)` and also imports `domain.{ Cart }` —
    // the type — so a namespace-agnostic lookup finds the type and reports the
    // program's own resource as one the file cannot see. Two declarations, one
    // spelling, two namespaces: exactly what `Namespace` exists for.
    //
    // A QUALIFIED path keeps its qualifier. Taking the last segment here would
    // have been `docs/RISK_QUEUE.md` 34 in a new place: `Resources.Cart` and
    // `store.page.Cart` are two resources, and an optimistic clause targeting
    // one must not be typechecked against the other.
    let resolution = match path.contains('.') {
        true => workspace.resolve_path(unit, path),
        false => workspace.resolve_in(unit, Namespace::Term, path),
    };
    let def = match resolution {
        Resolution::Local(def) | Resolution::Imported { def, .. } => def,
        _ => return None,
    };
    let _ = sigs;
    // The WHOLE program. `A-005` writes `optimistic Cart(..)` and imports
    // `Resources.{ Cart }`, so a lookup restricted to this unit would report
    // every cross-module resource as one the file cannot see — which is the
    // shape of half the defects this milestone found.
    let decl = crate::resolve::declaration(hirs, def)?;
    if !matches!(
        decl.kind,
        DeclKind::Query | DeclKind::Resource | DeclKind::Subscription
    ) {
        return None;
    }
    match decl.ret.as_deref() {
        Some("Result") => decl.ret_args.first().cloned(),
        Some(other) => Some(other.to_string()),
        None => None,
    }
}

fn target_is_not_a_resource_entry(
    decl: &Decl,
    body: &Body,
    target: &crate::hir::TermRoot,
    policy: &crate::hir::Policy,
    why: &str,
) -> Diagnostic {
    Diagnostic {
        code: crate::codes::OPTIMISTIC_TARGET_MISMATCH.id,
        invariant: crate::codes::OPTIMISTIC_TARGET_MISMATCH.invariant,
        reason: "optimistic_target_is_not_a_resource_entry",
        detector: Detector::DeclarationRule,
        severity: Severity::Error,
        message: format!(
            "`{}`'s optimistic clause targets no resource: {why}",
            decl.name
        ),
        primary_span: body.expr_span(target.root),
        related: vec![Related {
            span: policy.span.clone(),
            label: "declared here".to_string(),
        }],
        explanation: Some(
            "An optimistic clause names the resource ENTRY it speculatively updates — \
             `Cart(current_session())`, not `Cart`. Two entries can have the same type, so a \
             clause that named only a type would not say which one it changed."
                .to_string(),
        ),
        repairs: vec![Repair {
            description: "name a declared resource and the key that selects its entry".to_string(),
            replacement: None,
        }],
    }
}

/// A name inside an `optimistic` or `rollback` term that nothing binds.
fn unresolved_in_policy_term(
    hir: &Hir,
    decl: &Decl,
    body: &Body,
    id: ExprId,
    name: &str,
    policy: &str,
) -> Diagnostic {
    Diagnostic {
        code: crate::codes::UNRESOLVED_NAME.id,
        invariant: crate::codes::UNRESOLVED_NAME.invariant,
        reason: "unresolved_in_policy_term",
        detector: Detector::DeclarationRule,
        severity: Severity::Error,
        message: format!("`{name}` does not resolve"),
        primary_span: body.expr_span(id),
        related: vec![Related {
            span: hir.decl_span(decl_id_of(hir, decl)),
            label: format!("written in `{policy}` on `{}`", decl.name),
        }],
        explanation: Some(format!(
            "An `{policy}` value is executable code: it runs on the client, and \
             its names come from lexical scope like any other term's. `{name}` \
             is not a parameter, not a local, and not a declaration this file \
             can see.\n\nThis value was kept as unparsed text until \
             2026-08-10, so nothing had ever asked. Making it a term means it \
             is checked like a term — no ambient binding is supplied because \
             the policy is spelled `{policy}`."
        )),
        repairs: vec![Repair {
            description: format!(
                "bind `{name}` explicitly, or write a value whose names are in scope"
            ),
            replacement: None,
        }],
    }
}

/// Does this bare call name something nothing in the language owns?
///
/// The owners, in the order `tests/semantic_ownership.rs` establishes them:
/// lexical scope, a declaration the unit can see, language syntax, a policy
/// head or operator, a value domain. A name none of those claims is a name the
/// author wrote and the compiler never gave meaning to.
fn bare_call_is_unowned(
    workspace: &crate::resolve::Workspace,
    unit: usize,
    path: &str,
    in_scope: &std::collections::BTreeSet<String>,
) -> bool {
    use crate::resolve::Resolution;

    if in_scope.contains(path)
        || crate::resolve::INTRINSIC_CALLS.contains(&path)
        || crate::resolve::VALUE_DOMAIN_CALLS.contains(&path)
    {
        return false;
    }
    // A policy head, or an operator under one. `merge_by_field(..)` and
    // `content_address(..)` lower into the body as ordinary calls today; the
    // `PolicyExpr` split is what will stop them doing so, and until it lands
    // reporting them would be reporting a policy value as a missing function.
    if crate::policy::domain_of(path).is_some()
        || ["retry", "reconnect", "conflict", "identity"]
            .iter()
            .any(|h| crate::policy::operator(h, path).is_some())
    {
        return false;
    }
    // A privacy label constructor: `privacy User(consumer)`.
    if matches!(path, "Session" | "User" | "Organization" | "Device") {
        return false;
    }
    matches!(workspace.resolve(unit, path), Resolution::Unresolved)
}

fn unresolved_bare_call(hir: &Hir, decl: &Decl, body: &Body, id: ExprId, path: &str) -> Diagnostic {
    Diagnostic {
        code: crate::codes::UNRESOLVED_NAME.id,
        invariant: crate::codes::UNRESOLVED_NAME.invariant,
        reason: "unresolved_bare_call",
        detector: Detector::DeclarationRule,
        severity: Severity::Error,
        message: format!("`{path}` does not resolve"),
        primary_span: body.expr_span(id),
        related: vec![Related {
            span: hir.decl_span(decl_id_of(hir, decl)),
            label: format!("called inside `{}`", decl.name),
        }],
        explanation: Some(format!(
            "`{path}` names nothing this file can see. Assumption A-009: term \
             names are not ambient — only the Effect namespace is — so a \
             function must be declared here or brought in by an explicit \
             import.\n\nA call that resolves to nothing is not caught by \
             anything downstream. Inference contributes no effects for it, \
             privacy no label, placement no constraint and the contract no \
             capability, and each of those answers is what a harmless call \
             produces too."
        )),
        repairs: vec![Repair {
            description: format!("import the module that declares `{path}`, or declare it here"),
            replacement: None,
        }],
    }
}

/// A resolution failure, as a diagnostic.
fn resolve_diagnostic(e: &crate::resolve::ResolveError) -> Diagnostic {
    use crate::codes;
    use crate::resolve::ResolveErrorKind as K;

    let (code, repair) = match &e.kind {
        K::UnresolvedModule { module } => (
            codes::UNRESOLVED_MODULE,
            format!("declare `module {module}`, or import a module that exists"),
        ),
        K::UnresolvedName { module, name } => (
            codes::UNRESOLVED_NAME,
            format!("declare `{name}` in `{module}`, or import a name it has"),
        ),
        K::AmbiguousName { name, .. } => (
            codes::AMBIGUOUS_NAME,
            format!("import `{name}` from one module, or qualify each use"),
        ),
        K::DuplicateDeclaration { name } => (
            codes::DUPLICATE_DECLARATION,
            format!("rename one of them; `{name}` can name one thing per namespace"),
        ),
        K::PrivateAccess { module, name } => (
            codes::PRIVATE_ACCESS,
            format!("make `{name}` public in `{module}`, or stop importing it"),
        ),
        K::ImportCycle { .. } => (
            codes::IMPORT_CYCLE,
            "extract the shared declarations into a third module".to_string(),
        ),
    };

    Diagnostic {
        code: code.id,
        invariant: code.invariant,
        reason: "name_resolution",
        detector: Detector::DeclarationRule,
        severity: Severity::Error,
        message: e.kind.message(),
        primary_span: e.span.clone(),
        related: vec![Related {
            span: e.span.clone(),
            label: "resolved against the workspace module graph".to_string(),
        }],
        explanation: Some(
            "Names are visible through lexical scope, module membership, or an \
             explicit import — and through nothing else. External implementation \
             is allowed; a missing declaration is not."
                .to_string(),
        ),
        repairs: vec![Repair {
            description: repair,
            replacement: None,
        }],
    }
}

pub fn check_unit(env: &Env, unit: &Unit) -> Vec<Diagnostic> {
    let sigs = Signatures::default();
    // One unit's own workspace, so a call to a sibling in the same file still
    // resolves. Narrower than `check_units` by construction — see below.
    let own = crate::resolve::Workspace::build(&[&unit.hir]);
    let inference = crate::effects::Inference::new(&sigs, &own);
    let manifest = crate::boundary::TypeFacts::default();
    let routes = std::collections::BTreeSet::new();
    // One unit's own graph. Enough for a single-file caller, and honestly
    // narrower than the whole-program one: an edge to another file's query is
    // dangling here, which is why `check_units` is the entry point every real
    // caller uses.
    let ws = crate::resolve::Workspace::build(&[&unit.hir]);
    let graph = crate::graph::Graph::build(&[&unit.hir], &ws);
    check_unit_with(
        env,
        &BTreeMap::new(),
        &sigs,
        &inference,
        &manifest,
        &routes,
        &graph,
        &crate::ontology::Ontology::build_with(&[&unit.hir], &ws),
        &ws,
        // This entry point builds a workspace from ONE unit, so the unit it
        // resolves in is 0. `check_units` is what real callers use.
        0,
        &crate::ontology::argument_names_visible_from(&ws, 0, &[&unit.hir]),
        unit,
    )
}

#[allow(clippy::too_many_arguments)]
fn check_unit_with(
    env: &Env,
    labels: &BTreeMap<crate::resolve::DefId, Label>,
    sigs: &Signatures,
    inference: &crate::effects::Inference<'_>,
    manifest: &crate::boundary::TypeFacts,
    routes: &std::collections::BTreeSet<String>,
    graph: &crate::graph::Graph,
    // What each effect DECLARES about itself, and the graph its arguments
    // resolve through. Phase legality reads an effect's semantic facets, which
    // only its declaration knows.
    ontology: &crate::ontology::Ontology,
    ws: &crate::resolve::Workspace,
    // Which unit this is, so a call to a SIBLING resolves rather than being
    // matched by spelling — `docs/RISK_QUEUE.md` 34.
    at: usize,
    // What a capability argument may name FROM THIS UNIT: the types and
    // modules it can see. Per-unit since A-017 was retired — the ambient
    // whole-program set is gone.
    types: &std::collections::BTreeSet<String>,
    unit: &Unit,
) -> Vec<Diagnostic> {
    let mut out = Vec::new();

    // Placement is inherited: a `fn` inside a `component placement browser`
    // runs in the browser too. Checking each declaration in isolation misses
    // exactly the case the charter opens with — a database read inside a
    // browser-placed component — because the effect and the placement are on
    // different declarations.
    let mut inherited: BTreeMap<u32, World> = BTreeMap::new();
    for (id, decl) in unit.hir.all_decls() {
        if let Some(w) = declared_world(&unit.hir, decl) {
            for child in &decl.children {
                inherited.insert(child.0, w);
            }
            let _ = id;
        }
    }

    // Charter §7.5A relations: an ordering inside one frame, an observation
    // that feeds itself, a property the compositor cannot animate, a subtree
    // that is not as independent as it claims. None is an effect-row
    // violation, so none belongs in `effect_rows`.
    crate::layout::check(&unit.hir, sigs, &mut out);

    // Charter §7.1, §7.10, §8.2: three questions about what a value is, each
    // answered from a type the author wrote down.
    crate::annotations::check(&unit.hir, sigs, &mut out);

    // Affine resources: consumed exactly once, in the scope that acquired it.
    crate::affine::check(&unit.hir, sigs, &mut out);

    // Charter §8.5: the resume manifest ships with the document.
    crate::resume::check(&unit.hir, sigs, manifest, &mut out);

    // E7 generator: the resume manifest and the handler artifact, derived by
    // two different walks and compared. A disagreement within one build is a
    // build error; disagreement ACROSS builds is `runtime/pw-resume`'s job and
    // is not a source diagnostic at all.
    crate::resume_artifacts::check(
        &unit.src,
        &unit.hir,
        sigs,
        crate::resume_artifacts::BUILD,
        &mut out,
    );

    // Charter §8.2: an internal link names a route the program declares.
    crate::routes::check(&unit.hir, routes, &mut out);
    crate::graph::check(&unit.hir, graph, &mut out);

    for (id, decl) in unit.hir.all_decls() {
        privacy_and_placement(
            &unit.hir,
            &unit.src,
            labels,
            inference,
            // Where each effect is meaningful, as the program declares it.
            // There was a hard-coded family→world table in `placement.rs`
            // answering this; the declaration is now the only source.
            ontology,
            at,
            id,
            decl,
            inherited.get(&id.0).copied(),
            &mut out,
        );
        privacy_flow(&unit.hir, sigs, id, decl, &mut out);
        privacy_sinks(&unit.hir, sigs, id, decl, &mut out);
        call_arity(&unit.hir, sigs, ws, at, decl, &mut out);
        effect_rows(
            &unit.hir, sigs, inference, ontology, ws, at, id, decl, &mut out,
        );
        crate::capability::capability_arguments(&unit.hir, decl, types, &mut out);
        markup_rules(&unit.hir, decl, &mut out);
        let Some(body_id) = decl.body else { continue };
        let body = unit.hir.body(body_id);

        // Local type facts: a parameter's declared type, and any `let x: T`.
        let mut locals: BTreeMap<String, String> = BTreeMap::new();
        for p in &decl.params {
            if let Some(t) = &p.ty {
                locals.insert(p.name.clone(), t.written());
            }
        }

        for id in body.walk() {
            if let Expr::Match { scrutinee, arms } = body.expr(id) {
                exhaustiveness(env, unit, body, id, *scrutinee, arms, &locals, &mut out);
            }
        }
        scopes(decl, body, &mut out);
    }
    out.sort_by_key(|d| d.primary_span.start);
    out
}

/// **What the exhaustiveness analysis concluded about one `match`.**
///
/// Architect ruling, 2026-08-10:
///
/// > I would make diagnostics a **projection of an analysis result**, not the
/// > only observable result. […] Now the audit can assert **positive proof**
/// > rather than infer it from silence.
///
/// The hole this closes, found by `tests/evidence_reachability.rs`: A-002 and
/// A-012 are exhaustiveness fixtures, and the only thing observable about them
/// from outside was the *absence* of a diagnostic — which is also what a
/// checker that never ran produces. Both were classified `Unqueryable`.
///
/// `Blocked` is the variant that makes the difference. It is not a violation
/// and emphatically not a proof: it says the analysis produced no answer, and
/// it is what every early return in `analyse_match` became.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum MatchOutcome {
    /// The analysis ran and every value is covered.
    Proven,
    /// The analysis ran and these values are not.
    NonExhaustive { missing: Vec<String> },
    /// The analysis did not run. Never read this as coverage.
    Blocked { reason: String },
}

/// One `match`, with what the analysis concluded and where it is.
#[derive(Debug, Clone)]
pub struct MatchAnalysis {
    /// The declaration the match is written in.
    pub declaration: String,
    /// The scrutinee's type as the program declares it, when it has one.
    pub scrutinee_type: Option<String>,
    pub span: crate::hir::Span,
    pub outcome: MatchOutcome,
}

/// **Every `match` in the program, and what was concluded about it.**
///
/// The same walk `check_unit` does, so a `match` that reaches the checker
/// reaches this and vice versa. A separate walk here would let the two disagree
/// about which matches exist, and the disagreement would be invisible: an
/// audit would report a proof for a match no rule examined.
pub fn match_analysis(units: &[Unit]) -> Vec<MatchAnalysis> {
    let env = Env::build(units);
    let mut out = Vec::new();
    for unit in units {
        for (_, decl) in unit.hir.all_decls() {
            let Some(body_id) = decl.body else { continue };
            let body = unit.hir.body(body_id);
            let mut locals: BTreeMap<String, String> = BTreeMap::new();
            for p in &decl.params {
                if let Some(t) = &p.ty {
                    locals.insert(p.name.clone(), t.written());
                }
            }
            for id in body.walk() {
                if let Expr::Match { scrutinee, arms } = body.expr(id) {
                    out.push(analyse_match(
                        &env, &decl.name, body, id, *scrutinee, arms, &locals,
                    ));
                }
            }
        }
    }
    out
}

/// The analysis, with no diagnostics in it.
#[allow(clippy::too_many_arguments)]
fn analyse_match(
    env: &Env,
    declaration: &str,
    body: &Body,
    match_id: ExprId,
    scrutinee: ExprId,
    arms: &[hir::MatchArm],
    locals: &BTreeMap<String, String>,
) -> MatchAnalysis {
    let span = body.expr_span(match_id);
    let blocked = |reason: &str, ty: Option<String>| MatchAnalysis {
        declaration: declaration.to_string(),
        scrutinee_type: ty,
        span: span.clone(),
        outcome: MatchOutcome::Blocked {
            reason: reason.to_string(),
        },
    };

    // Only a bare name whose type is declared. No guessing (see module docs) —
    // and each of these was a bare `return` until 2026-08-10, which is exactly
    // the silence the audit could not tell from a proof.
    let Expr::Name(n) = body.expr(scrutinee) else {
        return blocked(
            "the scrutinee is not a bare name, so its type is unknown here",
            None,
        );
    };
    let Some(ty_name) = locals.get(n) else {
        return blocked("the scrutinee's type is not declared in this body", None);
    };
    let Some(adt_id) = env.adt_of(ty_name) else {
        return blocked(
            "the scrutinee's type is not an algebraic data type this program declares",
            Some(ty_name.clone()),
        );
    };
    let ctors = &env.ctor_names[ty_name];
    let lowered: Vec<Arm> = arms
        .iter()
        .map(|a| Arm {
            pattern: to_exhaust_pattern(body, a.pat, ctors),
            span: body.pat_span(a.pat),
        })
        .collect();

    let report = exhaust::check_match(env.program(), &Type::Adt(adt_id), &lowered);
    let outcome = match report.outcome() {
        crate::outcome::Outcome::Proven(_) => MatchOutcome::Proven,
        crate::outcome::Outcome::Blocked(bs) => MatchOutcome::Blocked {
            reason: format!("{bs:?}"),
        },
        crate::outcome::Outcome::Violation(_) => MatchOutcome::NonExhaustive {
            missing: report
                .missing
                .iter()
                .map(|w| exhaust::render_witness(env.program(), &Type::Adt(adt_id), w))
                .collect(),
        },
    };
    MatchAnalysis {
        declaration: declaration.to_string(),
        scrutinee_type: Some(ty_name.clone()),
        span,
        outcome,
    }
}

#[allow(clippy::too_many_arguments)]
fn exhaustiveness(
    env: &Env,
    unit: &Unit,
    body: &Body,
    match_id: ExprId,
    scrutinee: ExprId,
    arms: &[hir::MatchArm],
    locals: &BTreeMap<String, String>,
    out: &mut Vec<Diagnostic>,
) {
    // **The verdict comes from `analyse_match`, not from a second run.**
    //
    // Architect ruling, 2026-08-10: a diagnostic is a projection of an analysis
    // result. Deriving it twice would let the report an audit reads and the
    // error a developer reads disagree, and the disagreement would be silent —
    // which is `docs/RISK_QUEUE.md`'s most common shape.
    let analysis = analyse_match(env, "", body, match_id, scrutinee, arms, locals);

    // Only a bare name whose type is declared. No guessing (see module docs).
    let Expr::Name(n) = body.expr(scrutinee) else {
        return;
    };
    let Some(ty_name) = locals.get(n) else { return };
    let Some(adt_id) = env.adt_of(ty_name) else {
        return;
    };
    let ctors = &env.ctor_names[ty_name];

    let lowered: Vec<Arm> = arms
        .iter()
        .map(|a| Arm {
            pattern: to_exhaust_pattern(body, a.pat, ctors),
            span: body.pat_span(a.pat),
        })
        .collect();

    // Validate BEFORE analysing. A pattern whose arity disagrees with its
    // constructor is a defect in its own right, and it is also what
    // desynchronizes the pattern row from the type list inside the
    // exhaustiveness algorithm. Reporting it here means the author is told
    // what is actually wrong, rather than being told about a missing variant
    // that follows from it.
    //
    // `exhaust.rs` defends against the mismatch anyway. Two layers, because
    // "upstream validated it" is the assumption that produced the panic.
    let program = env.program();
    for (arm, lowered) in arms.iter().zip(&lowered) {
        let exhaust::Pattern::Ctor { ctor, args } = &lowered.pattern else {
            continue;
        };
        let Some(declared) = program
            .ctors_of(&Type::Adt(adt_id))
            .and_then(|cs| cs.get(*ctor).cloned())
        else {
            continue;
        };
        if args.len() == declared.fields.len() {
            continue;
        }
        out.push(Diagnostic {
            code: crate::codes::CONSTRUCTOR_ARITY.id,
            invariant: crate::codes::CONSTRUCTOR_ARITY.invariant,
            reason: "constructor_pattern_arity_mismatch",
            detector: Detector::PatternMatrix,
            severity: Severity::Error,
            message: format!(
                "`{}` binds {} field(s) but declares {}",
                declared.name,
                args.len(),
                declared.fields.len()
            ),
            primary_span: body.pat_span(arm.pat),
            related: vec![Related {
                span: body.expr_span(match_id),
                label: format!("matching on `{ty_name}`"),
            }],
            explanation: Some(format!(
                "A constructor pattern binds its constructor's fields, so the count                  is not a style choice — `{}` carries {} of them. Writing a different                  number leaves the compiler with a pattern that does not describe any                  value of this type.",
                declared.name,
                declared.fields.len()
            )),
            repairs: vec![Repair {
                description: if args.len() < declared.fields.len() {
                    format!(
                        "bind the remaining field(s), or write `{}(_)`-style wildcards",
                        declared.name
                    )
                } else {
                    format!("`{}` takes {}", declared.name, declared.fields.len())
                },
                replacement: None,
            }],
        });
    }

    // A `Blocked` analysis produced NO ANSWER. The reason is already reported
    // by whichever phase found it — `PW0603` above, for the arity case — and
    // saying it twice would turn one defect into two. What must not happen is
    // treating it as a proof of exhaustiveness, which is what `is_exhaustive()`
    // did before `Outcome` existed.
    let MatchOutcome::NonExhaustive { missing } = &analysis.outcome else {
        return;
    };

    let span = body.expr_span(match_id);
    out.push(Diagnostic {
        code: "PW0305",
        invariant: "a match must cover every value its scrutinee can take",
        reason: "non_exhaustive_match",
        detector: Detector::PatternMatrix,
        severity: Severity::Error,
        message: format!("match on `{ty_name}` is not exhaustive"),
        primary_span: span.clone(),
        related: vec![Related {
            span: body.expr_span(scrutinee),
            label: format!("this has type `{ty_name}`"),
        }],
        explanation: Some(format!(
            "`{ty_name}` has {} constructor(s); {} of them {} unmatched. \
             Adding a variant to a type must break every match on it at compile \
             time, which is why a wildcard arm is not the default repair.",
            ctors.len(),
            missing.len(),
            if missing.len() == 1 { "is" } else { "are" },
        )),
        repairs: missing
            .iter()
            .map(|m| Repair {
                description: format!("add an arm for `{m}`"),
                replacement: None,
            })
            .chain(std::iter::once(Repair {
                description: "or add `_ =>` if the remaining cases are genuinely \
                              interchangeable — this silences future variants too"
                    .to_string(),
                replacement: None,
            }))
            .collect(),
    });

    let _ = (unit, &lowered);
}

/// HIR pattern → the usefulness algorithm's pattern.
///
/// The load-bearing case: a nullary constructor has no parentheses, so the
/// grammar cannot tell `Draft` from a binding and emits `Pattern::Bind`. Read
/// as a binding it matches everything, which makes the first arm cover the
/// scrutinee and reports a non-exhaustive match as exhaustive — the failure is
/// silent and favourable, which is the shape `docs/RISK_QUEUE.md` tracks.
fn to_exhaust_pattern(body: &Body, id: hir::PatternId, ctors: &[String]) -> EPat {
    match body.pat(id) {
        HPat::Wild | HPat::Literal(_) | HPat::Error => EPat::Wildcard,
        HPat::Bind { name, .. } => match ctors.iter().position(|c| c == name) {
            Some(i) => EPat::unit(i),
            None => EPat::Wildcard,
        },
        HPat::Ctor { path, args } => {
            // `DecodeError.Invalid` and `Invalid` name the same constructor.
            let short = path.rsplit('.').next().unwrap_or(path);
            match ctors.iter().position(|c| c == short) {
                Some(i) => EPat::ctor(
                    i,
                    args.iter()
                        .map(|a| to_exhaust_pattern(body, *a, ctors))
                        .collect(),
                ),
                None => EPat::Wildcard,
            }
        }
        HPat::Or(ps) => EPat::Or(
            ps.iter()
                .map(|p| to_exhaust_pattern(body, *p, ctors))
                .collect(),
        ),
    }
}

/// Parse, lower and check a set of files together.
pub fn check_sources(files: &[(String, String)]) -> Vec<(String, Vec<Diagnostic>)> {
    let units: Vec<Unit> = files
        .iter()
        .map(|(path, src)| {
            let parsed = pw_syntax::parse_tree(src);
            Unit {
                path: path.clone(),
                src: src.clone(),
                hir: crate::lower::lower_file(src, &parsed.green),
            }
        })
        .collect();
    check_units(&units)
}

/// Declarations that define a type, for callers that need the environment.
pub fn declares_a_type(kind: DeclKind) -> bool {
    matches!(kind, DeclKind::Type | DeclKind::Opaque)
}

// --- structured concurrency (E2A-S) --------------------------------------

/// The ambient scope a declaration's body runs in.
///
/// A `view` and a `component` live for as long as their component; a `page` for
/// a route; a `query` or `command` for one request. Getting this wrong in the
/// permissive direction would silently accept an escape, so anything
/// unrecognised is treated as the *narrowest* scope.
fn ambient_scope(decl: &Decl) -> ScopeKind {
    match decl.kind {
        DeclKind::View | DeclKind::Component => ScopeKind::Component,
        DeclKind::Page => ScopeKind::Route,
        DeclKind::Query | DeclKind::Command => ScopeKind::Request,
        DeclKind::Subscription => ScopeKind::Session,
        _ => ScopeKind::Block,
    }
}

fn named_scope(name: &str) -> Option<ScopeKind> {
    Some(match name {
        "application" => ScopeKind::Application,
        "session" => ScopeKind::Session,
        "route" => ScopeKind::Route,
        "request" => ScopeKind::Request,
        "component" => ScopeKind::Component,
        _ => return None,
    })
}

/// Feed `crate::scope`'s graph from a real body (`docs/NEXT.md` item 7).
///
/// The graph and its rules were written and tested against hand-built inputs in
/// E2A-S. Nothing here re-implements them: this only builds the graph and
/// converts what comes back.
fn scopes(decl: &Decl, body: &Body, out: &mut Vec<Diagnostic>) {
    let mut g = ScopeGraph::new();
    // The lifetime hierarchy every declaration sits inside. Built in full each
    // time so `outlives` has real parent links to walk.
    let app = g.scope("application", ScopeKind::Application, None);
    let session = g.scope("session", ScopeKind::Session, Some(app));
    let route = g.scope("route", ScopeKind::Route, Some(session));
    let request = g.scope("request", ScopeKind::Request, Some(route));
    let component = g.scope("component", ScopeKind::Component, Some(route));
    let here = match ambient_scope(decl) {
        ScopeKind::Application => app,
        ScopeKind::Session => session,
        ScopeKind::Route => route,
        ScopeKind::Request => request,
        ScopeKind::Component | ScopeKind::Block => component,
    };
    let by_kind = move |k: ScopeKind| match k {
        ScopeKind::Application => app,
        ScopeKind::Session => session,
        ScopeKind::Route => route,
        ScopeKind::Request => request,
        ScopeKind::Component | ScopeKind::Block => component,
    };

    let mut declared: Vec<(usize, usize, crate::hir::Span, String)> = Vec::new();

    for id in body.walk() {
        match body.expr(id) {
            // `task.spawn(detached) { .. }` — a detach with no durable
            // capability. `durable.spawn` is the legal path and is not this.
            Expr::Call { callee, args } => {
                let path = path_of(body, *callee);
                if path != "task.spawn" {
                    continue;
                }
                // The `detached` argument is the violation; the call is where
                // the handle came from. Charter §16.3 wants both, and they must
                // be different spans or the second underline says nothing.
                let Some(at) = args
                    .iter()
                    .find(|a| matches!(body.expr(a.value), Expr::Name(n) if n == "detached"))
                    .map(|a| body.expr_span(a.value))
                else {
                    continue;
                };
                let h = g.spawn("task.spawn", HandleKind::Task, here, body.expr_span(id));
                g.op(Op::Detach {
                    handle: h,
                    span: at,
                });
            }

            // `observe intersection(self) -> Bool { scope application }` — a
            // subscription declaring a scope that outlives the one it is
            // created in.
            Expr::Keyword {
                keyword,
                modifiers,
                block,
                ..
            } if keyword == "observe" || keyword == "subscribe" => {
                let Some(block) = block else { continue };
                let Some((scope_name, at)) = block_scope(body, *block) else {
                    continue;
                };
                let Some(kind) = named_scope(&scope_name) else {
                    continue;
                };
                let label = match modifiers.first() {
                    Some(m) => format!("{keyword}.{m}"),
                    None => keyword.clone(),
                };
                let h = g.spawn(&label, HandleKind::Subscription, here, body.expr_span(id));
                declared.push((h, by_kind(kind), at.clone(), format!("scope {scope_name}")));
            }

            _ => {}
        }
    }

    let mut found = g.check();
    let mut clauses: Vec<Option<String>> = vec![None; found.len()];
    for (h, scope, span, clause) in declared {
        if let Some(v) = g.check_declared_scope(h, scope, span) {
            found.push(v);
            // The clause as written, so the diagnostic can quote `scope
            // application` rather than paraphrasing it.
            clauses.push(Some(clause));
        }
    }
    out.extend(
        found
            .into_iter()
            .zip(clauses)
            .map(|(v, c)| to_diagnostic(v, c)),
    );
}

/// `scope application` inside a block: the scope's name, and the span covering
/// the pair so a diagnostic can underline the clause rather than the statement.
fn block_scope(body: &Body, block: ExprId) -> Option<(String, crate::hir::Span)> {
    name_pair(body, block, "scope")
}

/// A callee's dotted path as written: `task.spawn`, `Analytics.record_view`.
fn path_of(body: &Body, id: ExprId) -> String {
    match body.expr(id) {
        Expr::Name(n) => n.clone(),
        Expr::Field { base, name } => format!("{}.{}", path_of(body, *base), name),
        _ => String::new(),
    }
}

fn to_diagnostic(v: ScopeViolation, clause: Option<String>) -> Diagnostic {
    Diagnostic {
        code: match v.code {
            "PW2001" => "PW2001",
            "PW2002" => "PW2002",
            "PW2003" => "PW2003",
            _ => "PW2004",
        },
        invariant: match v.code {
            "PW2001" => "a handle cannot outlive the scope that owns it",
            "PW2002" => "an ordinary task cannot be detached from its scope",
            "PW2003" => "a handle cannot be used after its owning scope has exited",
            _ => "a subscription cannot declare a scope that outlives its owner",
        },
        reason: match v.code {
            "PW2001" => "handle_escape",
            "PW2002" => "ordinary_task_detached",
            "PW2003" => "use_after_scope",
            _ => "declared_scope_outlives_owner",
        },
        detector: Detector::ScopeGraph,
        severity: Severity::Error,
        message: v.message,
        primary_span: v.span,
        related: vec![Related {
            span: v.origin_span,
            label: "the handle is created here".to_string(),
        }],
        // The explanation says WHY the invariant exists; the repair says what
        // to do instead. Emitting the same sentence twice reads as a bug.
        explanation: Some(match v.code {
            "PW2001" => "A handle is cancelled when its owning scope ends. One that has \
                         escaped is a reference to work that may already be gone."
                .to_string(),
            "PW2002" => "Structured concurrency has no fire-and-forget: every ordinary task \
                         belongs to a scope and is cancelled with it. Work that must survive \
                         the scope is a different kind of work, and says so."
                .to_string(),
            "PW2003" => "A result arriving after its owning scope has exited must not be \
                         committed — there is nothing left to commit it to."
                .to_string(),
            _ => format!(
                "A subscription keeps pushing updates for as long as its declared \
                 scope lives. `{}` outlives the component that created it, so the \
                 subscription would push into something that no longer exists.",
                clause.as_deref().unwrap_or("the declared scope")
            ),
        }),
        repairs: vec![Repair {
            description: v.help,
            replacement: None,
        }],
    }
}

// --- privacy, cache safety and placement (E5) ------------------------------

/// A declaration's privacy label, from its visibility keyword.
///
/// Conservative on purpose: an unlabelled declaration is treated as public, so
/// the checker can only *under*-restrict a value it was never told about. It
/// cannot invent a restriction and reject a legal program.
pub(crate) fn label_of(decl: &Decl) -> Label {
    // The parameters are the charter §7.8 type names — `Session<SessionId>`,
    // not `Session<Cart>`. A label names *what* a value is scoped to, not which
    // declaration produced it, and the corpus declares the former as the text
    // the developer must be shown.
    match decl.visibility.as_deref() {
        Some("session") => Label::session("SessionId"),
        Some("private") => Label::user("UserId"),
        _ => Label::public(),
    }
}

/// The world a declaration pins, from either place it can be written.
///
/// A `query` puts `placement origin` in its policy block, before the brace. A
/// `component` writes `placement browser` inside its body. Reading only the
/// policy block missed every component — which is the charter's opening
/// example, a database read inside a browser-placed component.
pub(crate) fn declared_world(hir: &Hir, decl: &Decl) -> Option<World> {
    if let Some(p) = decl.policy("placement") {
        let v = p.value.trim();
        if let Some(w) = ALL_WORLDS.iter().copied().find(|w| w.name() == v) {
            return Some(w);
        }
    }
    let body = hir.body(decl.body?);
    let (name, _) = name_pair(body, body.root, "placement")?;
    ALL_WORLDS.iter().copied().find(|w| w.name() == name)
}

/// `<keyword> <value>` as two adjacent name statements in a block, with the
/// span covering the pair.
fn name_pair(body: &Body, block: ExprId, keyword: &str) -> Option<(String, crate::hir::Span)> {
    let Expr::Block { stmts } = body.expr(block) else {
        return None;
    };
    let mut it = stmts.iter().peekable();
    while let Some(s) = it.next() {
        if !matches!(body.expr(*s), Expr::Name(n) if n == keyword) {
            continue;
        }
        let Some(next) = it.peek() else { continue };
        if let Expr::Name(v) = body.expr(**next) {
            let span = body.expr_span(*s).start..body.expr_span(**next).end;
            return Some((v.clone(), span));
        }
    }
    None
}

/// E5's checks over one declaration.
///
/// Each is a *flow* question answered by the algebra in `crate::privacy` and
/// `crate::placement`, not a pattern match on syntax. That is what lets a
/// diagnostic name the value and the boundary rather than reporting that a
/// keyword was in the wrong place (charter §14 M5 gate).
#[allow(clippy::too_many_arguments)]
fn privacy_and_placement(
    hir: &Hir,
    src: &str,
    labels: &BTreeMap<crate::resolve::DefId, Label>,
    inference: &crate::effects::Inference<'_>,
    declared: &dyn crate::placement::Placements,
    at: usize,
    id: crate::hir::DeclId,
    decl: &Decl,
    inherited: Option<World>,
    out: &mut Vec<Diagnostic>,
) {
    // The declaration's own visibility, joined with everything its body reads.
    // A `page` that is itself unlabelled but renders a `session query` is a
    // session materialization — which is the corpus's canonical case, and is
    // invisible to a rule that only reads the header.
    let (read, read_from) = reads_label_with_source(hir, labels, inference, at, decl);
    let label = label_of(decl).join(&read);

    // 1. A non-public value in a shared cache. Charter §7.8's canonical case.
    if let Some((cache_value, cache_span)) =
        declared_cache(hir, decl).filter(|(v, _)| v == "shared" && !label.safe_in_shared_cache())
    {
        {
            let cache = crate::hir::Policy {
                name: "cache".to_string(),
                value: cache_value,
                roots: Vec::new(),
                span: cache_span,
            };
            let needed = label.required_cache_partitions();
            out.push(Diagnostic {
                code: "PW5001",
                invariant: "a value that is not public cannot live in a shared cache",
                reason: "private_value_in_shared_cache",
                detector: Detector::DeclarationRule,
                severity: Severity::Error,
                message: format!("`{}` is {label} and declares a shared cache", decl.name),
                primary_span: cache.span.clone(),
                related: vec![Related {
                    span: hir.decl_span(decl_id_of(hir, decl)),
                    label: format!("`{}` is labelled {label} here", decl.name),
                }],
                explanation: Some(format!(
                    "A shared cache may contain only `Public` values, because every \
                     user reads it. The cause chain: `{}` materializes {}, which is \
                     labeled `{label}`, and `{label}` does not flow into `Public` — \
                     so one user's value would be served to another.",
                    decl.name,
                    read_from
                        .as_deref()
                        .map(|v| format!("`{v}`"))
                        .unwrap_or_else(|| "a non-public value".to_string()),
                )),
                repairs: vec![
                    Repair {
                        description: format!(
                            "partition the cache by {} so each one gets its own entry",
                            if needed.is_empty() {
                                "the restriction".to_string()
                            } else {
                                needed.join(" and ")
                            }
                        ),
                        replacement: None,
                    },
                    Repair {
                        description: "or make the cache private, which stores per session"
                            .to_string(),
                        replacement: Some((cache.span.clone(), "cache private".to_string())),
                    },
                ],
            });
        }
    }

    // 2. Placement. The demand is what this definition EFFECTIVELY needs, plus
    // the label; the solver answers which worlds can satisfy it.
    //
    // `effective_effects`, not the declared row. Architect ruling, 2026-08-07:
    //
    // > Placement and E8 contract generation should use the same central
    // > effective-effects function. No caller should independently decide
    // > whether to look at declarations or inference.
    //
    // Two answers to "what does this need" is how one semantic declaration ends
    // up with two answers about where it can run — the pattern this project has
    // spent its life deleting. It also means an over-declared row no longer
    // narrows placement: `fn f() !{ database.read, trace } { trace("hi") }`
    // needs `trace`, and a host must not be asked for database access because
    // an annotation permitted it.
    let effects: Vec<String> = inference.effective_effects(at, hir, id);
    if effects.is_empty() && label.is_public() {
        return;
    }

    let world = declared_world(hir, decl).or(inherited);

    // An effect the row does NOT cover, at a world the author declared, is
    // `DECLARED_PLACEMENT_CANNOT_GRANT`'s to report — against the call that
    // performs it, naming the worlds that could. Reporting it here as well
    // would deliver one defect twice in two vocabularies, which is what
    // `every_rejected_fixture_emits_only_its_own_defect` exists to prevent.
    //
    // Covers, not equals: R-026 declares `secret<Payments>` and its body
    // performs `secret.read`. The row covers it, so this rule owns it.
    let row: Vec<String> = decl
        .declared_effects
        .as_deref()
        .unwrap_or_default()
        .iter()
        .map(|e| e.path.clone())
        .collect();
    if let Some(w) = world
        && effects.iter().any(|e| {
            !crate::effects::row_covers(&row, e)
                && w.grants(e, declared) == crate::placement::Grant::No
        })
    {
        return;
    }

    let demand = Demand {
        effects: effects.clone(),
        label: label.clone(),
        declared: world,
    };
    let solution = solve(&demand, declared);
    if solution.is_satisfiable() {
        return;
    }
    // An effect that names nothing has no placement, and this rule has nothing
    // to say about it. `check_effect_rows` reports the row — with the family it
    // could not find and the nearest spelling — and reporting "nowhere to run"
    // here as well would name the declaration's placement as the problem while
    // the problem is a word in its row.
    if solution.is_blocked() {
        return;
    }

    // The effect AS WRITTEN, type arguments included. `database.read` and
    // `database.read<Stores>` are the same capability but not the same text,
    // and the corpus declares which one the developer must be shown.
    //
    // From the ROW's source span where a row exists, and from the effective set
    // otherwise. The row is what the author wrote and what the corpus declares
    // it must be shown: R-026 writes `secret<Payments>` and performs
    // `secret.read`, and being told about `secret.read` names something that
    // appears nowhere in the file.
    let mut written: Vec<String> = decl
        .declared_effects
        .as_deref()
        .unwrap_or_default()
        .iter()
        .map(|e| {
            src.get(e.span.clone())
                .unwrap_or(&e.path)
                .trim()
                .to_string()
        })
        .collect();
    if written.is_empty() {
        written = effects.clone();
    }

    // Nowhere can run this. Name every reason, for every world — that is the
    // cause chain charter §14 M5 task 5 asks for.
    let mut chain: Vec<String> = Vec::new();
    for &w in ALL_WORLDS {
        let reasons: Vec<String> = solution
            .why_not(w)
            .iter()
            .map(|r| r.reason.to_string())
            .collect();
        if !reasons.is_empty() {
            chain.push(format!("  {w}: {}", reasons.join("; ")));
        }
    }

    // Point at the effect that cannot be granted, which is what the author has
    // to change. The declaration's own span is the boundary, and goes in
    // `related` — charter §16.3 wants both.
    let span = decl
        .declared_effects
        .as_deref()
        .unwrap_or_default()
        .iter()
        .find(|e| {
            effects.iter().any(|f| f == &e.path || f == &e.written)
                && world.is_some_and(|w| w.grants(&e.path, declared) == crate::placement::Grant::No)
        })
        .map(|e| e.span.clone())
        .or_else(|| decl.policy("placement").map(|p| p.span.clone()))
        .unwrap_or_else(|| hir.decl_span(decl_id_of(hir, decl)));

    out.push(Diagnostic {
        code: "PW5002",
        invariant: "every declaration must have somewhere it can run",
        reason: "no_feasible_placement",
        detector: Detector::CapabilityAudit,
        severity: Severity::Error,
        message: format!(
            "`{}` cannot run in any world: it requires {}",
            decl.name,
            written.join(", ")
        ),
        primary_span: span,
        related: vec![Related {
            span: hir.decl_span(decl_id_of(hir, decl)),
            label: if written.is_empty() {
                format!("labelled {label}")
            } else {
                format!("requires {}", written.join(", "))
            },
        }],
        explanation: Some(format!(
            "Placement is derived from what a body does, not chosen. Each world \
             was ruled out:\n{}",
            chain.join("\n")
        )),
        repairs: vec![Repair {
            description: "split the work: keep the privileged effect where it is \
                          granted and pass a value across the boundary"
                .to_string(),
            replacement: None,
        }],
    });
}

/// The id of a declaration, by identity of its span.
fn decl_id_of(hir: &Hir, decl: &Decl) -> crate::hir::DeclId {
    hir.all_decls()
        .find(|(_, d)| std::ptr::eq(*d, decl))
        .map(|(id, _)| id)
        .expect("the declaration came from this Hir")
}

/// Every restriction a declaration picks up by *calling* another declaration.
///
/// A `page` that renders a `session query` holds session data, whether or not
/// the page says so. Reading it from the program — rather than from the table
/// of library accessors below — is real label propagation: it follows the
/// author's own declarations, and it improves as the program does.
/// The same, plus the binding that carried the restriction.
///
/// A diagnostic that says "this page is Session" is true and unhelpful; the
/// author needs to know it was `cart`. The corpus declares that name as text it
/// expects to see, which is how the gap was found.
/// **The label a DECLARATION carries: its own, joined with everything its body
/// reads.**
///
/// Distinct from `body_label` below, which is a resume-manifest question about
/// one body's captures.
///
/// One derivation, two readers. `check.rs` uses it to refuse a non-public value
/// in a shared cache; `contract.rs` uses it to build the placement demand. It
/// passed `Label::public()` there until 2026-08-11 — see E10-P in
/// `docs/EVIDENCE_LEDGER.md` — four lines under a comment saying the solver
/// weighs privacy labels.
///
/// The deferral was deliberate and is now discharged: the label is the join of
/// what a body READS, and until policy values left the executable body tree a
/// body walk could not tell one from a term. Wiring it in first would have
/// given the contract's placement a second channel from policy values.
pub(crate) fn declaration_label(
    hir: &Hir,
    labels: &BTreeMap<crate::resolve::DefId, Label>,
    inference: &crate::effects::Inference<'_>,
    at: usize,
    decl: &Decl,
) -> Label {
    let (read, _) = reads_label_with_source(hir, labels, inference, at, decl);
    label_of(decl).join(&read)
}

fn reads_label_with_source(
    hir: &Hir,
    labels: &BTreeMap<crate::resolve::DefId, Label>,
    inference: &crate::effects::Inference<'_>,
    at: usize,
    decl: &Decl,
) -> (Label, Option<String>) {
    let Some(body_id) = decl.body else {
        return (Label::public(), None);
    };
    let body = hir.body(body_id);
    let mut label = Label::public();
    let mut source: Option<String> = None;

    // Bindings, so a labelled call can be attributed to the name it was bound
    // to rather than to the call itself.
    let mut bound_at: BTreeMap<usize, String> = BTreeMap::new();
    for id in body.walk() {
        let Expr::Let { pat, .. } = body.expr(id) else {
            continue;
        };
        if let Some(HPat::Bind { name, .. }) = pat.map(|p| body.pat(p)) {
            bound_at.insert(body.expr_span(id).start, name.clone());
        }
    }

    for id in body.walk() {
        // `query Cart(session)` parses as a keyword statement; `Cart(session)`
        // as a call. Both name a declaration.
        let called = match body.expr(id) {
            Expr::Call { callee, .. } => path_of(body, *callee),
            Expr::Keyword {
                keyword, modifiers, ..
            } if keyword == "query" || keyword == "subscribe" => {
                modifiers.first().cloned().unwrap_or_default()
            }
            _ => continue,
        };
        if called.is_empty() {
            continue;
        }
        // RESOLVED. `Cart` in one module and `Cart` in another are two
        // declarations with two labels, and taking the join of both made a
        // page's privacy depend on what an unrelated module happened to call
        // its own query.
        //
        // A name that resolves to nothing contributes nothing — which is the
        // honest answer, and narrower than the join it replaces. A page reading
        // a query it never imported is a resolution error, reported as one.
        let Some(def) = inference.called_from(at, &called) else {
            continue;
        };
        let Some(found) = labels.get(&def).cloned() else {
            continue;
        };
        if found.is_public() {
            continue;
        }
        let short = called.rsplit('.').next().unwrap_or(&called);
        label = label.join(&found);
        if source.is_none() {
            // The nearest enclosing binding, by start offset.
            let at = body.expr_span(id).start;
            source = bound_at
                .range(..=at)
                .next_back()
                .map(|(_, n)| n.clone())
                .or_else(|| Some(short.to_string()));
        }
    }
    (label, source)
}

/// Every restriction a body picks up by calling something.
///
/// Reads the callee's **declared return label** from its signature. There is no
/// table of accessor names here any more: a function carries a secret because
/// it returns `Secret<C>`, not because it is spelled `secrets.something`
/// (E2C, architect ruling 2026-08-06).
fn body_label(body: &Body, sigs: &Signatures) -> Label {
    let mut label = Label::public();
    for id in body.walk() {
        let Expr::Call { callee, .. } = body.expr(id) else {
            continue;
        };
        if let Some(sig) = sigs.by_path(&path_of(body, *callee)) {
            label = label.join(&sig.label);
        }
    }
    label
}

/// E5 rules that need the body's label, not only the declaration header.
/// The cache partition a declaration asks for, from either place it can be
/// written: a `query`'s policy block, or a `page`'s body.
/// The cache policy a declaration names, from either place it can be written.
///
/// A `query` puts `cache private` in its policy block, before the brace; a
/// `page` writes it inside its body. `pub(crate)` because `resume.rs` needs the
/// same answer to know what scope a manifest lands in, and reading it a second
/// way there is how the two would come to disagree about what a private page is.
pub(crate) fn declared_cache(hir: &Hir, decl: &Decl) -> Option<(String, crate::hir::Span)> {
    if let Some(p) = decl.policy("cache") {
        return Some((p.value.trim().to_string(), p.span.clone()));
    }
    let body = hir.body(decl.body?);
    name_pair(body, body.root, "cache")
}

/// **PW0604 — a call passes exactly the arguments its callee declares.**
///
/// The first check of an ordinary call site. Until 2026-08-20 there was none:
/// not the arity, not the argument types, not the result, while
/// `docs/MILESTONES.md` recorded E9 — *permanent value type checker* — as
/// complete and charter §14 M9A listed `unification-based inference` among its
/// contents. `a_call_site_is_not_type_checked` pinned it; this is the first
/// piece of the repair.
///
/// **Arity first, deliberately.** It needs no inference at all — the callee's
/// declaration says how many parameters it has and the call says how many
/// arguments it passes — so it can be correct today, on the whole corpus,
/// without waiting for a type comparison to exist. Argument TYPES are the next
/// piece and are not here.
///
/// # What it refuses to decide
///
/// Only a call whose callee resolves **by path** to a declaration with a
/// signature. A callee that does not resolve is not an arity error and must not
/// be reported as one — it is either a name this build cannot see or a
/// different defect with its own code. Silence here is the three-valued
/// discipline the project uses everywhere else: *wrong*, *right*, and *this
/// analysis has nothing to say*.
///
/// Constructors are excluded for the same reason: `Cart(1)` resolves to a type
/// declaration, whose "parameters" are a variant's fields, and `PW0603` already
/// owns that relation.
fn call_arity(
    hir: &Hir,
    sigs: &Signatures,
    ws: &crate::resolve::Workspace,
    at: crate::resolve::UnitId,
    decl: &Decl,
    out: &mut Vec<Diagnostic>,
) {
    let Some(body_id) = decl.body else { return };
    let body = hir.body(body_id);

    // **`|>` supplies an argument the call does not carry.** `items |>
    // List.map(f)` is `List.map(items, f)`: the pipeline's left side becomes
    // the call's first argument, so counting only `args` reports every
    // pipelined call as one argument short.
    //
    // The corpus found this on the rule's first run — five call sites across
    // A-001 and A-016, all correct code. Built the same way `infer.rs` builds
    // it, because "which call receives a piped value" must have one answer.
    let mut piped: std::collections::BTreeSet<crate::hir::ExprId> =
        std::collections::BTreeSet::new();
    for id in body.walk() {
        if let Expr::Binary {
            op: crate::hir::BinOp::Pipe,
            rhs,
            ..
        } = body.expr(id)
        {
            piped.insert(*rhs);
        }
    }

    for id in body.walk() {
        let Expr::Call { callee, args } = body.expr(id) else {
            continue;
        };
        let supplied = args.len() + usize::from(piped.contains(&id));
        // **By resolved identity, never by spelling.** `two(1)` and
        // `other.two(1)` are different callees, and a lookup keyed on the
        // written path would answer for whichever declaration happened to be
        // spelled that way. The resolution is the same one the backend uses to
        // decide which declaration a call reaches.
        let path = path_of(body, *callee);
        let def = match path.contains('.') {
            true => ws.resolve_path(at, &path),
            false => ws.resolve_in(at, crate::resolve::Namespace::Term, &path),
        };
        // `Ambiguous` and `Unresolved` are deliberately silent: neither is an
        // arity error, and reporting one as such would be a right-shaped
        // verdict from the wrong mechanism.
        let def = match def {
            crate::resolve::Resolution::Local(d) => d,
            crate::resolve::Resolution::Imported { def, .. } => def,
            _ => continue,
        };
        let Some(sig) = sigs.by_def(def) else {
            continue;
        };
        // **A declaration that is not a callable is excluded by having no
        // signature**, not by a list of kinds this function keeps. `Cart(1)`
        // reaches a type, whose fields are `PW0603`'s relation and which
        // `Signatures` does not describe.
        //
        // The first version did enumerate kinds, and it needed the callee's
        // Decl to read `kind` — which it looked up in the CURRENT unit only, so
        // every cross-module call fell through and the rule silently did not
        // apply to them. `a_call_across_modules_is_checked_by_resolved_identity`
        // is what caught it, and the repair is to stop asking the question.
        // A declaration with no parameter list of its own — a type, an event —
        // has nothing to say about arity here.
        if sig.params.is_empty() && supplied == 0 {
            continue;
        }
        if supplied == sig.params.len() {
            continue;
        }
        let (n, m) = (supplied, sig.params.len());
        let word = |k: usize| if k == 1 { "argument" } else { "arguments" };
        out.push(
            Diagnostic::error(
                crate::codes::CALL_ARITY.id,
                crate::codes::CALL_ARITY.invariant,
                Detector::Signature,
                format!("`{path}` declares {m} {} and this call passes {n}", word(m)),
                body.expr_span(id),
            )
            .reason("call_arity_disagrees_with_declaration")
            .explain(
                "the callee's declaration fixes how many arguments a call supplies; a call \
                 that passes a different number is not the call the declaration describes",
            )
            .repair(match n < m {
                true => "pass the missing arguments",
                false => "remove the extra arguments",
            }),
        );
    }
}

/// Charter §7.8: a sink accepts only what its declared privacy level admits.
///
/// The level is read from the signature, not from the function's name.
/// `log.public` carries `!{ log<Public> }`, and it is the type argument that
/// makes the sink checkable — a rule that recognised the identifier `public`
/// would be inventing the vocabulary it then enforces, and would miss the next
/// sink somebody declares.
///
/// Deliberately outside `privacy_flow`, which returns early when the enclosing
/// declaration's own label is public. R-006's `trace_capture` returns `()`, so
/// its body label IS public; the defect is in what it passes along, not in what
/// it returns.
fn privacy_sinks(
    hir: &Hir,
    sigs: &Signatures,
    id: crate::hir::DeclId,
    decl: &Decl,
    out: &mut Vec<Diagnostic>,
) {
    let Some(body_id) = decl.body else { return };
    let body = hir.body(body_id);
    let imports = crate::labels::imported_modules(hir);
    // Labels belong to VALUES. `labels.rs` explains why this is not a map from
    // binding name to restriction any more.
    let labels = crate::labels::Labels::of_body(sigs, decl, body, hir.module_of(id), &imports);

    for id in body.walk() {
        let Expr::Call { callee, args } = body.expr(id) else {
            continue;
        };
        let Some(level) = sink_level(sigs, body, *callee) else {
            continue;
        };
        // Only `Public` is checked, because only `Public` is what the corpus
        // specifies. A rule for the other levels would be a rule nothing has
        // ever exercised.
        if level.0 != "Public" {
            continue;
        }
        for arg in args {
            // The argument's own label, whatever it is spelled as: a name, a
            // field of one, a branch that returns one, a string with one in a
            // hole. Not a search for names the body happened to bind.
            let label = labels.label(body, arg.value);
            if label.is_public() {
                continue;
            }
            for r in label.restrictions() {
                let (name, span, origin) = blame(body, &labels, arg.value);
                let Restriction::Secret(cap) = r else {
                    continue;
                };
                let origin = origin.unwrap_or_else(|| span.clone());
                out.push(Diagnostic {
                    code: crate::codes::VALUE_EXCEEDS_SINK_LEVEL.id,
                    invariant: crate::codes::VALUE_EXCEEDS_SINK_LEVEL.invariant,
                    reason: "value_exceeds_sink_privacy_level",
                    detector: Detector::PatternMatrix,
                    severity: Severity::Error,
                    message: format!(
                        "cannot log a `Secret<{cap}>` value at privacy level `Public`"
                    ),
                    primary_span: span,
                    related: vec![
                        Related {
                            span: origin.clone(),
                            label: format!("`{name}` becomes Secret<{cap}> here"),
                        },
                        Related {
                            span: level.1.clone(),
                            label: format!("`{}` is declared here", level.0),
                        },
                    ],
                    explanation: Some(format!(
                        "`log<Public>` accepts only `Public` values, and a \
                         `Secret<{cap}>` is not one. Logging is an effect \
                         parameterized by privacy level precisely so that this is a \
                         compile error rather than an incident found in a log \
                         archive months later. The row already says \
                         `secret<{cap}>`, which permits the value to EXIST here — \
                         it does not permit it to leave."
                    )),
                    repairs: vec![Repair {
                        description: "log a non-secret correlate — an identifier or a \
                                      hash — or use a sink whose level admits the value"
                            .to_string(),
                        replacement: None,
                    }],
                });
            }
        }
    }
}

/// What to blame in the diagnostic: the innermost labelled name inside the
/// argument, its span, and where it acquired the label.
fn blame(
    body: &Body,
    labels: &crate::labels::Labels<'_>,
    value: ExprId,
) -> (String, crate::hir::Span, Option<crate::hir::Span>) {
    for id in body.walk_from(value) {
        if let Expr::Name(n) = body.expr(id)
            && let Some((_, origin)) = labels.origin(n)
        {
            return (n.clone(), body.expr_span(id), Some(origin.clone()));
        }
    }
    ("this value".to_string(), body.expr_span(value), None)
}

/// The privacy level a call's sink declares, with the span of the effect that
/// declares it.
fn sink_level(
    sigs: &Signatures,
    body: &Body,
    callee: ExprId,
) -> Option<(String, crate::hir::Span)> {
    // By path only. A sink is named — `log.public` — and resolving it by "the
    // one declaration spelled `public`" would make the privacy rule depend on
    // no other module having a `public`.
    let sig = sigs.by_path(&path_of(body, callee))?;
    sig.effects.iter().find_map(|e| {
        let (_, rest) = e.split_once('<')?;
        let level = rest.strip_suffix('>')?;
        (!level.is_empty()).then(|| (level.to_string(), body.expr_span(callee)))
    })
}

fn privacy_flow(
    hir: &Hir,
    sigs: &Signatures,
    id: crate::hir::DeclId,
    decl: &Decl,
    out: &mut Vec<Diagnostic>,
) {
    let Some(body_id) = decl.body else { return };
    let body = hir.body(body_id);
    let imports = crate::labels::imported_modules(hir);
    let label = body_label(body, sigs);
    if label.is_public() {
        return;
    }

    // 1. A secret reaching markup. Markup renders in the browser, and a secret
    //    never leaves the origin (charter §7.8, corpus R-003).
    //
    // The VALUE's label, via `labels.rs`. This used to read a map from binding
    // name to restriction, so `let shown = if dry_run { "none" } else { key }`
    // rendered a secret that the rule could not see — the same narrowness the
    // sink rule had, in the one place that had not been migrated with it.
    let labels = crate::labels::Labels::of_body(sigs, decl, body, hir.module_of(id), &imports);
    for id in body.walk() {
        let Expr::Template { parts, .. } = body.expr(id) else {
            continue;
        };
        for part in parts {
            let value_label = labels.label(body, *part);
            let Some(Restriction::Secret(cap)) = value_label.restrictions().next() else {
                continue;
            };
            let (name, origin) = body
                .walk_from(*part)
                .into_iter()
                .find_map(|e| match body.expr(e) {
                    Expr::Name(n) => labels.origin(n).map(|(_, o)| (n.clone(), o.clone())),
                    _ => None,
                })
                .unwrap_or_else(|| ("this value".to_string(), body.expr_span(*part)));
            let origin = &origin;
            out.push(Diagnostic {
                code: "PW5003",
                invariant: "a secret cannot be rendered to the browser",
                reason: "secret_crosses_to_browser",
                detector: Detector::DeclarationRule,
                severity: Severity::Error,
                message: format!("`{name}` is Secret<{cap}> and is rendered into markup"),
                primary_span: body.expr_span(*part),
                related: vec![Related {
                    span: origin.clone(),
                    label: format!("`{name}` becomes Secret<{cap}> here"),
                }],
                explanation: Some(format!(
                    "Markup is sent to the browser. Secret<{cap}> may exist only in \
                     the origin world, so rendering it moves it across a boundary \
                     it may not cross — the value is in the HTML whether or not \
                     anything reads it."
                )),
                repairs: vec![Repair {
                    description: "perform the privileged operation on the origin and \
                                  render only its result"
                        .to_string(),
                    replacement: None,
                }],
            });
        }
    }

    // 2. A shared cache keyed without every partition the value needs
    //    (charter §14 M5 task 4, corpus R-005).
    let shared = decl
        .policy("cache")
        .is_some_and(|c| c.value.trim() == "shared");
    if !shared {
        return;
    }
    let key_policy = decl.policy("key");
    let key_text = key_policy.map(|k| k.value.clone()).unwrap_or_default();
    let missing: Vec<String> = label
        .required_cache_partitions()
        .into_iter()
        .filter(|p| !key_text.contains(p.as_str()))
        .collect();
    if missing.is_empty() {
        return;
    }

    let span = key_policy
        .map(|k| k.span.clone())
        .or_else(|| decl.policy("cache").map(|c| c.span.clone()))
        .unwrap_or_else(|| hir.decl_span(decl_id_of(hir, decl)));

    out.push(Diagnostic {
        code: "PW5004",
        invariant: "a shared cache key must carry every partition its value depends on",
        reason: "cache_key_omits_partition",
        detector: Detector::DeclarationRule,
        severity: Severity::Error,
        message: format!(
            "`{}` is {label} but its shared cache key omits {}",
            decl.name,
            missing.join(" and ")
        ),
        primary_span: span,
        related: vec![Related {
            span: hir.decl_span(decl_id_of(hir, decl)),
            label: format!("the result depends on {label}"),
        }],
        explanation: Some(format!(
            "Two callers with different {} produce different results, and this key \
             cannot tell them apart — so the first caller's value is served to the \
             second. That is a cross-tenant leak, not a stale read.",
            missing.join(" and ")
        )),
        repairs: vec![Repair {
            description: format!(
                "add {} to the key, or make the cache private",
                missing.join(" and ")
            ),
            replacement: None,
        }],
    });
}

// --- markup rules (E3/E5, no inference required) ---------------------------

/// Elements whose children HTML constrains. Charter §8.2 asks for semantic
/// HTML; a `<div>` inside a `<ul>` is invalid markup that browsers silently
/// reparse, so the DOM stops matching the source.
fn permitted_children(tag: &str) -> Option<&'static [&'static str]> {
    Some(match tag {
        "ul" | "ol" => &["li", "script", "template"],
        "dl" => &["dt", "dd", "div", "script", "template"],
        "table" => &["caption", "colgroup", "thead", "tbody", "tfoot", "tr"],
        "thead" | "tbody" | "tfoot" => &["tr"],
        "tr" => &["td", "th"],
        "select" => &["option", "optgroup"],
        _ => return None,
    })
}

/// Structural rules over a declaration's markup.
///
/// None of these needs effect inference: they are properties of the tree the
/// author wrote. They are grouped because they share the walk, not because they
/// share an invariant — each pushes its own code.
fn markup_rules(hir: &Hir, decl: &Decl, out: &mut Vec<Diagnostic>) {
    let Some(body_id) = decl.body else { return };
    let body = hir.body(body_id);

    // The escape hatch must justify itself (charter §14 M5 task 6).
    //
    // A declaration that declares an audited `unsafe capability … because "…"`
    // covers the uses of it inside: A-024 justifies the capability once and
    // then calls `unsafe.imperative` twice, and demanding a fresh string at
    // every use would make the justification boilerplate rather than a reason.
    // A complete audit record is BOTH halves: a reason, and an owner answerable
    // for the consequence. A reason nobody is attributed for is a comment.
    let audited = body.walk().iter().any(|id| {
        matches!(
            body.expr(*id),
            Expr::Keyword {
                keyword,
                justification: Some(_),
                attribution: Some(_),
                ..
            } if keyword.starts_with("unsafe")
        )
    });
    for id in body.walk() {
        let Expr::Keyword {
            keyword,
            justification,
            attribution,
            ..
        } = body.expr(id)
        else {
            continue;
        };
        if !keyword.starts_with("unsafe") {
            continue;
        }
        // An attribution naming a declaration nobody can find is worse than
        // none: it reads as reviewed. Separate invariant, separate code —
        // and checked BEFORE the `audited` short-circuit, because a record
        // with both halves present is exactly what that short-circuit treats
        // as complete.
        if let Some(target) = attribution {
            let resolves = hir.all_decls().any(|(_, d)| &d.name == target) || decl.name == *target;
            if justification.is_some() && !resolves {
                out.push(Diagnostic {
                    code: crate::codes::UNSAFE_ATTRIBUTION_INVALID.id,
                    invariant: crate::codes::UNSAFE_ATTRIBUTION_INVALID.invariant,
                    reason: "attribution_target_unresolved",
                    detector: Detector::DeclarationRule,
                    severity: Severity::Error,
                    message: format!(
                        "`{keyword}` is attributed to `{target}`, which names no declaration"
                    ),
                    primary_span: body.expr_span(id),
                    related: vec![Related {
                        span: hir.decl_span(decl_id_of(hir, decl)),
                        label: format!("`{}` opens the escape hatch here", decl.name),
                    }],
                    explanation: Some(
                        "An attribution target names who is answerable for the \
                         consequence. One that resolves to nothing reads as reviewed \
                         and is not — which is worse than an escape hatch with no \
                         attribution at all."
                            .to_string(),
                    ),
                    repairs: vec![Repair {
                        description: "name a declaration in this package, or an ADR".to_string(),
                        replacement: None,
                    }],
                });
                continue;
            }
        }
        if audited {
            continue;
        }
        let missing: Vec<&str> = [
            justification
                .is_none()
                .then_some("a `because` justification"),
            attribution.is_none().then_some("an attribution target"),
        ]
        .into_iter()
        .flatten()
        .collect();
        if missing.is_empty() {
            continue;
        }
        out.push(Diagnostic {
            code: crate::codes::UNSAFE_AUDIT_INCOMPLETE.id,
            invariant: crate::codes::UNSAFE_AUDIT_INCOMPLETE.invariant,
            reason: "unsafe_audit_incomplete",
            detector: Detector::DeclarationRule,
            severity: Severity::Error,
            message: format!("`{keyword}` requires {}", missing.join(" and ")),
            primary_span: body.expr_span(id),
            related: vec![Related {
                span: hir.decl_span(decl_id_of(hir, decl)),
                label: format!("`{}` opens an escape hatch here", decl.name),
            }],
            explanation: Some(
                "An escape hatch is a promise that the compiler's rule is wrong here \
                 and someone checked. The audit record has two halves: a written \
                 reason, so there is something to review and something to delete when \
                 the reason stops being true, and an attribution target, so the cost \
                 lands on a named owner rather than on the frame at large."
                    .to_string(),
            ),
            repairs: vec![Repair {
                description: format!(
                    "write `{keyword} because \"…\" attributes_forced_layout_to Owner`"
                ),
                replacement: None,
            }],
        });
    }

    // Everything else is about the element tree.
    let mut roots: Vec<crate::hir::NodeId> = Vec::new();
    for id in body.walk() {
        if let Expr::Template { roots: r, .. } = body.expr(id) {
            roots.extend(r.iter().copied());
        }
    }

    for id in body.walk_markup(&roots) {
        match body.node(id) {
            // A list without a key cannot preserve identity across a reorder.
            Node::Block { directive, .. } => {
                let d = directive.trim();
                if !d.starts_with("{#each") || d.contains('(') {
                    continue;
                }
                out.push(Diagnostic {
                    code: "PW5011",
                    invariant: "a list over a mutable collection needs a stable key",
                    reason: "unkeyed_each",
                    detector: Detector::DeclarationRule,
                    severity: Severity::Error,
                    message: "`#each` over a mutable collection requires a key expression"
                        .to_string(),
                    primary_span: body.node_span(id),
                    related: vec![Related {
                        span: hir.decl_span(decl_id_of(hir, decl)),
                        label: format!("`{}` renders this list", decl.name),
                    }],
                    explanation: Some(
                        "Without a stable key, reordering cannot preserve element identity, \
                         focus, or state: the renderer matches by position, so the third row \
                         keeps the second row's open menu and cursor."
                            .to_string(),
                    ),
                    repairs: vec![Repair {
                        description: "add a key: `{#each items as item (item.id)}`".to_string(),
                        replacement: None,
                    }],
                });
            }

            Node::Element {
                tag,
                attrs,
                children,
                ..
            } => {
                // Invalid nesting.
                if let Some(allowed) = permitted_children(tag) {
                    for c in children {
                        let Node::Element { tag: child, .. } = body.node(*c) else {
                            continue;
                        };
                        if allowed.contains(&child.as_str()) {
                            continue;
                        }
                        out.push(Diagnostic {
                            code: "PW5012",
                            invariant: "an element may only contain the children HTML permits",
                            reason: "invalid_nesting",
                            detector: Detector::DeclarationRule,
                            severity: Severity::Error,
                            message: format!(
                                "`<{child}>` is not permitted as a child of `<{tag}>`"
                            ),
                            primary_span: body.node_span(*c),
                            related: vec![Related {
                                span: body.node_span(id),
                                label: format!("`<{tag}>` starts here"),
                            }],
                            explanation: Some(format!(
                                "`<{tag}>` accepts only {}. A browser silently reparses \
                                 invalid nesting, so the DOM stops matching the source and \
                                 every selector, test and assistive technology sees a \
                                 different tree than the author wrote.",
                                allowed
                                    .iter()
                                    .map(|t| format!("`<{t}>`"))
                                    .collect::<Vec<_>>()
                                    .join(", ")
                            )),
                            repairs: vec![Repair {
                                description: format!(
                                    "wrap it in `<{}>`, or move it outside `<{tag}>`",
                                    allowed[0]
                                ),
                                replacement: None,
                            }],
                        });
                    }
                }

                // Interactive behaviour on a non-interactive element.
                let handler = attrs.iter().find(|a| a.name.starts_with("on:"));
                // `<form on:submit>` is the normal way to submit a form, and
                // omitting `form` here made R-022 — a handler *type* mismatch —
                // report as an accessibility defect instead. Right file, wrong
                // rule, and the count would have looked one better than it was.
                let interactive = matches!(
                    tag.as_str(),
                    "button"
                        | "a"
                        | "input"
                        | "select"
                        | "textarea"
                        | "summary"
                        | "label"
                        | "option"
                        | "form"
                        | "details"
                        | "dialog"
                );
                let has_role = attrs.iter().any(|a| a.name == "role");
                let has_tabindex = attrs.iter().any(|a| a.name == "tabindex");
                if let Some(h) = handler.filter(|_| !interactive && !(has_role && has_tabindex)) {
                    {
                        out.push(Diagnostic {
                            code: "PW5013",
                            invariant: "interactive behaviour belongs on an element that can \
                                        receive it",
                            reason: "handler_on_inert_element",
                            detector: Detector::DeclarationRule,
                            severity: Severity::Error,
                            message: format!(
                                "`<{tag}>` with an `{}` handler is not keyboard accessible",
                                h.name
                            ),
                            primary_span: h.span.clone(),
                            related: vec![Related {
                                span: body.node_span(id),
                                label: format!("`<{tag}>` is not an interactive element"),
                            }],
                            explanation: Some(
                                "A pointer handler on an inert element is unreachable by \
                                 keyboard and invisible to assistive technology. The element \
                                 looks like a control and is not one."
                                    .to_string(),
                            ),
                            repairs: vec![
                                Repair {
                                    description: "use `<button>`, which is focusable and \
                                                  activates on Enter and Space"
                                        .to_string(),
                                    replacement: None,
                                },
                                Repair {
                                    description: "or supply `role`, `tabindex` and keyboard \
                                                  activation"
                                        .to_string(),
                                    replacement: None,
                                },
                            ],
                        });
                    }
                }

                // A form control with nothing naming it.
                if matches!(tag.as_str(), "input" | "select" | "textarea") {
                    let typ =
                        attrs
                            .iter()
                            .find(|a| a.name == "type")
                            .and_then(|a| match &a.value {
                                AttrValue::Static(v) => Some(v.trim_matches('"').to_string()),
                                _ => None,
                            });
                    // `hidden` and `submit` inputs are not labelled controls.
                    if matches!(
                        typ.as_deref(),
                        Some("hidden") | Some("submit") | Some("button")
                    ) {
                        continue;
                    }
                    let named = attrs.iter().any(|a| {
                        matches!(a.name.as_str(), "aria-label" | "aria-labelledby" | "id")
                    });
                    if named {
                        continue;
                    }
                    out.push(Diagnostic {
                        code: "PW5014",
                        invariant: "a form control must have something that names it",
                        reason: "control_without_label",
                        detector: Detector::DeclarationRule,
                        severity: Severity::Error,
                        message: format!("`<{tag}>` has no associated label"),
                        primary_span: body.node_span(id),
                        related: vec![Related {
                            span: hir.decl_span(decl_id_of(hir, decl)),
                            label: format!("`{}` renders this control", decl.name),
                        }],
                        explanation: Some(
                            "A control with no accessible name is announced as \"edit text\" \
                             and nothing else. `name` is submitted to the server; it is not \
                             read to the user."
                                .to_string(),
                        ),
                        repairs: vec![Repair {
                            description: "associate a `<label for=...>`, or supply \
                                          `aria-label`/`aria-labelledby`"
                                .to_string(),
                            replacement: None,
                        }],
                    });
                }
            }

            _ => {}
        }
    }
}

// --- effect rows (E2D) ------------------------------------------------------

/// Does a declaration do what its effect row says?
///
/// Two questions, two codes. An **undeclared** effect is a row that understates
/// what the body does. A **forbidden** effect is one that no row could permit
/// here — a `view` reaching the database is not fixed by declaring it.
#[allow(clippy::too_many_arguments)]
fn effect_rows(
    hir: &Hir,
    sigs: &Signatures,
    inference: &crate::effects::Inference<'_>,
    // Phase legality is decided on an effect's semantic FACETS, which only its
    // declaration knows — see `ontology::Facet`.
    ontology: &crate::ontology::Ontology,
    ws: &crate::resolve::Workspace,
    at: usize,
    id: crate::hir::DeclId,
    decl: &Decl,
    out: &mut Vec<Diagnostic>,
) {
    use crate::effects::{Reuse, forbidden_in, forbidden_in_phase};

    let Some(body_id) = decl.body else { return };
    let body = hir.body(body_id);
    // The SAME type environment the inference built, not a narrower one
    // assembled here.
    //
    // It used to be declaration parameters plus annotated `let`s, and a lambda
    // parameter — having no annotation — fell through to a by-spelling rule
    // that the architect ordered deleted on 2026-08-07. `Types::of_body` types
    // a callback's parameter from the collection it is applied to, so `el` in
    // `items |> List.map(fn(el) ..)` is an `ElementRef` here and
    // `el.getBoundingClientRect()` resolves through its RECEIVER.
    //
    // Two constructions of one environment is how the narrower one silently
    // wins, which is exactly what happened.
    let mut types: BTreeMap<String, String> =
        crate::infer::Types::of_body(sigs, decl, body, hir.module_of(id))
            .bindings()
            .clone();
    for id in body.walk() {
        let Expr::Let {
            pat: Some(pat),
            ty: Some(t),
            ..
        } = body.expr(id)
        else {
            continue;
        };
        if let (HPat::Bind { name, .. }, Some(ty)) = (body.pat(*pat), body.types.get(t.index())) {
            types.insert(name.clone(), ty.path.clone());
        }
    }
    let mut found = inference.infer_in_at(at, body, &types);

    // **Plus the named roots that ARE this declaration's work.**
    //
    // A painter's `draw` block and a resource's `acquire`/`release` blocks
    // became named roots on 2026-08-11 so their headers could bind. They are
    // not reachable from `body.root`, and every rule below reads `found` — so
    // without this, `R-041`'s `dom.mutate` inside a painter stopped being seen
    // by the rule whose whole subject it is.
    //
    // An optimistic clause is excluded by `contributes_to_declaration`: it runs
    // on the client at a different time.
    for (_, root) in decl.term_roots() {
        if !root.context.contributes_to_declaration() {
            continue;
        }
        found
            .sources
            .extend(inference.infer_rooted(at, body, root.root).sources);
    }

    // Work that runs somewhere else does not contribute its effects here.
    // ONE model (`contexts.rs`) rather than a list of syntax exceptions: a
    // query dependency, a named declaration, a streamed region, a handler and
    // a later frame phase are the same fact seen six ways.
    let regions = crate::contexts::elsewhere(body);
    found
        .sources
        .retain(|s| crate::contexts::effects_belong_here(&regions, &s.span));
    found.effects = found.sources.iter().map(|s| s.effect.clone()).collect();
    let found = found;
    // Work inside an event handler, a streamed region or a later frame phase is
    // not done *during render*, so the render-time restriction does not apply
    // to it. Charter §7.5A — and it is the same question `regions` already
    // answers, asked from the other side.
    let at_render_time = |span: &crate::hir::Span| crate::contexts::at_render_time(&regions, span);

    // Forbidden first: it is the stronger statement, and reporting both for one
    // call would say the same thing twice.
    let mut reported: BTreeSet<String> = BTreeSet::new();

    // The frame phase an effect happens in decides what it may do, and that is
    // an ordering question rather than a question of which effects exist.
    for source in &found.sources {
        // EVERY enclosing phase, not just the innermost. A `measure { .. }`
        // opened inside `post_paint { .. }` is still in the post-paint frame,
        // and R-042 is exactly that program — caught until now only because a
        // synthesized effect happened to be recorded at the outer phase's span.
        let Some((phase, why)) = crate::effects::phases_at(body, &source.span)
            .into_iter()
            .find_map(|p| {
                forbidden_in_phase(&p, &ontology.facets_of(ws, at, &source.effect)).map(|w| (p, w))
            })
        else {
            continue;
        };
        if !reported.insert(source.effect.clone()) {
            continue;
        }
        out.push(Diagnostic {
            code: crate::codes::WRONG_FRAME_PHASE.id,
            invariant: crate::codes::WRONG_FRAME_PHASE.invariant,
            reason: "effect_in_wrong_phase",
            detector: Detector::PatternMatrix,
            severity: Severity::Error,
            message: format!(
                "`{}` happens in the `{phase}` phase, which may not do it",
                source.effect
            ),
            primary_span: source.span.clone(),
            related: vec![Related {
                span: hir.decl_span(decl_id_of(hir, decl)),
                label: format!("`{}` declares this frame", decl.name),
            }],
            explanation: Some(format!("{why}. The chain: {}.", source.via.describe())),
            repairs: vec![Repair {
                description: "move it to the phase that owns it — read in `measure`, \
                              write in `mutate`"
                    .to_string(),
                replacement: None,
            }],
        });
    }

    // Charter §7.9, §9.4: is this declaration's output computed once and
    // reused, or recomputed for its reader? A wall-clock read is only a defect
    // in the first case.
    let reuse = reuse_of(hir, decl);

    for source in &found.sources {
        let Some(why) = forbidden_in(
            decl,
            reuse,
            declared_world(hir, decl),
            ontology,
            &source.effect,
        ) else {
            continue;
        };
        if !at_render_time(&source.span) {
            continue;
        }
        if !reported.insert(source.effect.clone()) {
            continue;
        }
        out.push(Diagnostic {
            code: crate::codes::FORBIDDEN_EFFECT.id,
            invariant: crate::codes::FORBIDDEN_EFFECT.invariant,
            reason: "effect_forbidden_in_context",
            detector: Detector::PatternMatrix,
            severity: Severity::Error,
            message: match reuse {
                // Named for what makes it wrong. "which a page may not do" is
                // false — a page may read the clock; a page generated once at
                // build time may not.
                Reuse::Build => format!(
                    "a `Build`-placed {} may not use effect `{}`",
                    kind_noun(decl.kind),
                    source.effect
                ),
                Reuse::SharedPartition => format!(
                    "materialization `{}` may not depend on `{}`",
                    decl.name, source.effect
                ),
                Reuse::PerReader => format!(
                    "`{}` performs `{}`, which a {} may not do",
                    decl.name,
                    source.effect,
                    context_noun(decl)
                ),
            },
            primary_span: source.span.clone(),
            related: vec![Related {
                span: hir.decl_span(decl_id_of(hir, decl)),
                label: format!("`{}` is a {}", decl.name, context_noun(decl)),
            }],
            explanation: Some(format!(
                "{why}. The chain: {}. Declaring the effect would not help — the \
                 restriction is about where this runs, not about what it admits to.",
                source.via.describe()
            )),
            repairs: vec![Repair {
                description: "move the work into a query or command and pass its \
                              result in"
                    .to_string(),
                replacement: None,
            }],
        });
    }

    // Charter §8.2, §17.5: an escape hatch needs an audit record, and a
    // declaration that has none is the most incomplete record there is.
    //
    // The rule in `markup_rules` checks a record the author WROTE for
    // completeness. This is the other half: `raw_html` is not a name the
    // compiler recognises, it is a function whose row says `unsafe.raw_html`,
    // so declaring a new escape hatch in a platform package brings its own
    // audit requirement with it.
    if !has_audit_record(body) {
        for source in &found.sources {
            if crate::effects::family_of(&source.effect) != "unsafe"
                || !reported.insert(source.effect.clone())
            {
                continue;
            }
            out.push(Diagnostic {
                code: crate::codes::UNSAFE_AUDIT_INCOMPLETE.id,
                invariant: crate::codes::UNSAFE_AUDIT_INCOMPLETE.invariant,
                reason: "unsafe_effect_without_audit_record",
                detector: Detector::PatternMatrix,
                severity: Severity::Error,
                message: format!(
                    "`{}` requires the `{}` capability and a justification",
                    source.via.callee(),
                    source.effect
                ),
                primary_span: source.span.clone(),
                related: vec![Related {
                    span: hir.decl_span(decl_id_of(hir, decl)),
                    label: format!("`{}` declares no capability", decl.name),
                }],
                explanation: Some(format!(
                    "Escaping is the default, so reaching `{}` is a decision rather \
                     than a detail — untrusted input reaching `{}` is an XSS sink. \
                     {}. Adding the effect to the row would not help: the row says \
                     what happens, and the capability says who decided it was \
                     necessary and why.",
                    source.via.callee(),
                    source.via.callee(),
                    source.via.describe()
                )),
                repairs: vec![Repair {
                    description: format!(
                        "write `unsafe capability {} because \"…\" \
                         attributes_forced_layout_to Owner`, or escape the value with \
                         `html.text`",
                        source.effect
                    ),
                    replacement: None,
                }],
            });
        }
    }

    // The row as written, by path.
    let declared: Option<Vec<String>> = decl
        .declared_effects
        .as_ref()
        .map(|r| r.iter().map(|e| e.written.clone()).collect());

    // A placement the author NAMED must be able to grant what the body needs.
    //
    // `rules.rs` already checks this against the DECLARED row. That is not
    // enough on its own, and R-025 is why: its query names `placement origin`
    // and declares no row at all, so there was nothing to compare and a
    // registered rule sat silent. The effects are inferred; the placement is
    // declared; the check is the intersection.
    if let Some(world) = declared_world(hir, decl) {
        let row = declared.as_deref().unwrap_or(&[]);
        let mut said: BTreeSet<String> = BTreeSet::new();
        for source in &found.sources {
            // Leave anything the row COVERS to `rules.rs`, which reports it
            // against the row the author wrote and at that row's span.
            //
            // Covers, not equals. R-026 declares `secret<Payments>` and the
            // inferred effect is `secret.read`; an exact match reported it
            // here as well, so one defect arrived twice in two vocabularies
            // and the fixture stopped being a single-defect test.
            // `Grant::Blocked` continues too, and for a different reason than
            // `Grant::Yes`: the effect names nothing, `check_effect_rows` owns
            // that, and "not available at placement origin" would confirm a
            // typo as an effect while blaming the placement.
            if crate::effects::row_covers(row, &source.effect)
                || world.grants(&source.effect, ontology) != crate::placement::Grant::No
                || !said.insert(source.effect.clone())
            {
                continue;
            }
            let family = crate::effects::family_of(&source.effect);
            // Where this effect IS meaningful, from its declaration. It came
            // from `World::worlds_for(family)`, a hard-coded family→world
            // table, and the two answers differ in a way worth having: the
            // table could only speak per family, so `secret<Payments>` — no
            // dot, no family — got an empty list and the explanation said
            // "only  can", with nothing in the gap.
            let elsewhere = match ontology.placement_of(&source.effect) {
                crate::placement::PlacementLookup::Known(worlds) => worlds
                    .iter()
                    .map(|w| format!("`{w:?}`"))
                    .collect::<Vec<_>>()
                    .join(" or "),
                _ => String::new(),
            };
            out.push(Diagnostic {
                code: crate::codes::DECLARED_PLACEMENT_CANNOT_GRANT.id,
                invariant: crate::codes::DECLARED_PLACEMENT_CANNOT_GRANT.invariant,
                reason: "inferred_effect_outside_declared_placement",
                detector: Detector::PatternMatrix,
                severity: Severity::Error,
                message: format!(
                    "`{}` is not available at placement {world:?}",
                    source.effect
                ),
                primary_span: source.span.clone(),
                related: vec![Related {
                    span: hir.decl_span(decl_id_of(hir, decl)),
                    label: format!("`{}` is placed in {world:?}", decl.name),
                }],
                explanation: Some(format!(
                    "The {world} world grants no {family} capabilities; only {} can. \
                     {}. The row does not name this effect, so nothing said so \
                     until the body was read.",
                    if elsewhere.is_empty() {
                        "no world"
                    } else {
                        elsewhere.as_str()
                    },
                    source.via.describe()
                )),
                repairs: vec![Repair {
                    description: format!(
                        "move this into a declaration placed in {elsewhere}, and pass \
                         its result in"
                    ),
                    replacement: None,
                }],
            });
        }
    }
    for source in found.undeclared(declared.as_deref()) {
        // Same distinction: an event handler's effects and a streamed region's
        // are not the enclosing view's row. The handler is a separate body that
        // runs later, and charging its effects to the render is what reported
        // three correct accepted programs as violations.
        if reported.contains(&source.effect) || !at_render_time(&source.span) {
            continue;
        }
        out.push(Diagnostic {
            code: crate::codes::UNDECLARED_EFFECT.id,
            invariant: crate::codes::UNDECLARED_EFFECT.invariant,
            reason: "effect_not_in_row",
            detector: Detector::PatternMatrix,
            severity: Severity::Error,
            message: format!(
                "`{}` performs `{}` but does not declare it",
                decl.name, source.effect
            ),
            primary_span: source.span.clone(),
            related: vec![Related {
                span: hir.decl_span(decl_id_of(hir, decl)),
                label: format!("`{}` declares its effects here", decl.name),
            }],
            explanation: Some(format!(
                "An effect row is a claim about what a body does, and a caller \
                 relies on it. {}. An effect that reaches a caller through a \
                 helper or a callback is still an effect the caller pays for.",
                source.via.describe()
            )),
            repairs: vec![Repair {
                description: format!("add `{}` to the row", source.effect),
                replacement: None,
            }],
        });
    }
}

/// Does this declaration carry an audit record for an escape hatch?
///
/// Deliberately not "does it declare the matching capability by name". The
/// record's job is to say that a human decided the compiler's rule is wrong
/// here and why; `markup_rules` then checks that record is complete. Requiring
/// a name match as well would report the same missing record twice, in two
/// vocabularies.
fn has_audit_record(body: &Body) -> bool {
    body.walk().into_iter().any(|id| {
        matches!(
            body.expr(id),
            Expr::Keyword { keyword, justification, .. }
                if keyword.starts_with("unsafe") && justification.is_some()
        )
    })
}

/// Is this declaration's output computed once and reused, or recomputed for
/// each reader?
///
/// Two spellings reach the same place. `placement build` says the output is a
/// file produced before any request exists; `partition public` says one cache
/// entry serves every reader. Charter §7.9 and §9.4 give them separate words
/// because they are separate mechanisms — but the same thing goes wrong in
/// both, so the check asks one question.
fn reuse_of(hir: &Hir, decl: &Decl) -> crate::effects::Reuse {
    use crate::effects::Reuse;
    if declared_world(hir, decl) == Some(World::Build) {
        return Reuse::Build;
    }
    // The policy first, the body scan second — the same order as
    // `declared_world` and `declared_cache`. A `materialize` block's clauses
    // are policies; a `page` writes `cache private` among its statements and
    // the scan is what reads those.
    if decl
        .policy("partition")
        .is_some_and(|p| p.value.trim() == "public")
    {
        return Reuse::SharedPartition;
    }
    if let Some(body_id) = decl.body {
        let body = hir.body(body_id);
        if let Some((v, _)) = name_pair(body, body.root, "partition")
            && v == "public"
        {
            return Reuse::SharedPartition;
        }
    }
    Reuse::PerReader
}

/// What to call this declaration in a diagnostic.
///
/// A painter has no `DeclKind` of its own — `paint X(..) !{ paint.custom }`
/// lowers as an ordinary declaration — so the row is what names it. Saying
/// "which a declaration may not do" would be true and useless.
fn context_noun(decl: &Decl) -> &'static str {
    if decl
        .declared_effects
        .as_ref()
        .is_some_and(|r| r.iter().any(|e| e.path == "paint.custom"))
    {
        return "painter";
    }
    kind_noun(decl.kind)
}

fn kind_noun(kind: DeclKind) -> &'static str {
    match kind {
        DeclKind::View => "view",
        DeclKind::Component => "component",
        DeclKind::Page => "page",
        DeclKind::Query => "query",
        DeclKind::Command => "command",
        _ => "declaration",
    }
}
