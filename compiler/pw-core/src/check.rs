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

/// **Every name some type in the program has as a constructor**: each
/// declared sum type's cases, and the language's own `Some`, `None`, `Ok`
/// and `Err`. A bare name in a pattern that is one of these is a constructor,
/// never a fresh binding (ADR-0038).
///
/// It held an ADT per declaration, read from spellings, until 2026-09-26.
/// Each match now types its own subject's constructors from the resolved
/// declarations (`Instances`, ADR-0060), and this is what is left: the names.
///
/// **The set of files passed to one invocation is treated as one program.**
/// Recorded as assumption A-009.
pub struct Env {
    constructors: BTreeSet<String>,
}

impl Env {
    pub fn build(units: &[Unit]) -> Env {
        let mut constructors: BTreeSet<String> = ["Some", "None", "Ok", "Err"]
            .into_iter()
            .map(str::to_string)
            .collect();
        for u in units {
            for (_, d) in u.hir.all_decls() {
                for v in d.variants.iter().flatten() {
                    constructors.insert(v.name.clone());
                }
            }
        }
        Env { constructors }
    }

    /// The names the program knows as constructors, sorted.
    pub fn constructors(&self) -> impl Iterator<Item = &str> {
        self.constructors.iter().map(String::as_str)
    }

    /// Whether any type this program knows has a constructor of this name.
    fn names_a_constructor(&self, name: &str) -> bool {
        self.constructors.contains(name)
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
        per_unit.extend(unresolved_uses(&workspace, &sigs, &hirs, i, &u.hir));
        // ADR-0072: an element named with a capital letter is a view.
        per_unit.extend(view_elements(&workspace, &hirs, i, &u.hir));
        // ADR-0047: a name used as a value resolves too, in lexical scope.
        per_unit.extend(crate::names::check(&workspace, &hirs, i, &u.src));
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
    sigs: &Signatures,
    hirs: &[&Hir],
    unit: usize,
    hir: &Hir,
) -> Vec<Diagnostic> {
    use crate::resolve::Resolution;

    let mut out = Vec::new();
    // A declaration can call itself and its siblings.
    let declared: std::collections::BTreeSet<String> =
        hir.all_decls().map(|(_, d)| d.name.clone()).collect();
    for (decl_id, decl) in hir.all_decls() {
        let Some(body_id) = decl.body else { continue };
        let body = hir.body(body_id);
        // Which binding a callee's name means where the call is written: a
        // call to a function value out of its binding's scope names nothing
        // (ADR-0066). The set below held every name the body bound anywhere,
        // so such a call passed.
        let lexical = crate::lexical::Lexical::build_in(sigs, Some(unit), hir, decl_id);

        let mut in_scope = crate::resolve::local_bindings(body);
        in_scope.extend(decl.params.iter().map(|p| p.name.clone()));
        in_scope.extend(declared.iter().cloned());

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
                let unowned = match &lexical {
                    // A local in scope where the call is written is the value
                    // it holds; any other name must be a declaration's.
                    Some(lx) if matches!(body.expr(*callee), Expr::Name(_)) => {
                        lx.binder(*callee).is_none()
                            && bare_call_is_unowned(workspace, unit, &path, &declared)
                    }
                    _ => bare_call_is_unowned(workspace, unit, &path, &in_scope),
                };
                if unowned {
                    // `Circle(3)`: a case, which is built through its type
                    // (ADR-0059). The types it sees that declare one.
                    let owners: Vec<String> = workspace
                        .visible_types(unit)
                        .into_iter()
                        .filter_map(|def| crate::resolve::declaration(hirs, def))
                        .filter(|d| {
                            d.variants
                                .as_ref()
                                .is_some_and(|vs| vs.iter().any(|v| v.name == path))
                        })
                        .map(|d| d.name.clone())
                        .collect();
                    out.push(unresolved_bare_call(hir, decl, body, id, &path, &owners));
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

/// **An element named with a capital letter** (ADR-0072).
///
/// HTML lowercases every tag it parses, so `<Money>` is never HTML. The
/// charter writes a view used in another view that way:
/// `<Money value={item.price} />` (§8.1). Until 2026-09-26 nothing read such a
/// tag. It built as an unknown element named `Money`: the view's markup was
/// never rendered, and its props were checked by nothing. A view used in
/// another view is not compiled yet, so it is refused, and so is a tag that
/// names no view.
fn view_elements(
    workspace: &crate::resolve::Workspace,
    hirs: &[&Hir],
    unit: usize,
    hir: &Hir,
) -> Vec<Diagnostic> {
    use crate::resolve::Resolution;

    let mut out = Vec::new();
    for (decl_id, decl) in hir.all_decls() {
        let Some(body_id) = decl.body else { continue };
        let body = hir.body(body_id);
        let mut roots = Vec::new();
        for e in body.walk() {
            if let Expr::Template { roots: r, .. } = body.expr(e) {
                roots.extend(r.iter().copied());
            }
        }
        for n in body.walk_markup(&roots) {
            let Node::Element { tag, .. } = body.node(n) else {
                continue;
            };
            if !tag.starts_with(|c: char| c.is_ascii_uppercase()) {
                continue;
            }
            let named = match workspace.resolve(unit, tag) {
                Resolution::Local(def) | Resolution::Imported { def, .. } => {
                    crate::resolve::declaration(hirs, def).map(|d| d.kind)
                }
                Resolution::Unresolved | Resolution::Ambiguous(_) => None,
            };
            let (message, repair) = match named {
                Some(DeclKind::View | DeclKind::Component | DeclKind::Page) => (
                    format!(
                        "`<{tag}>` uses `{tag}` inside another view, and a view used in \
                         another view is not compiled yet"
                    ),
                    format!("write `{tag}`'s markup here until views compose"),
                ),
                Some(_) => (
                    format!("`<{tag}>` names a declaration that is not a view"),
                    "an element is an HTML element, written in lowercase, or a view".to_string(),
                ),
                None => (
                    format!("`<{tag}>` names no view in scope"),
                    format!("import `{tag}`, declare it, or write an HTML element in lowercase"),
                ),
            };
            out.push(Diagnostic {
                code: crate::codes::VIEW_ELEMENT.id,
                invariant: crate::codes::VIEW_ELEMENT.invariant,
                reason: "view_element",
                detector: Detector::DeclarationRule,
                severity: Severity::Error,
                message,
                primary_span: body.node_span(n),
                related: vec![Related {
                    span: hir.decl_span(decl_id),
                    label: format!("`{}` renders this", decl.name),
                }],
                explanation: Some(
                    "HTML lowercases every tag it parses, so an element named with a capital \
                     letter is a view. Until 2026-09-26 one built as an unknown HTML element: \
                     the view's markup was never rendered."
                        .to_string(),
                ),
                repairs: vec![Repair {
                    description: repair,
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
            let mut types = crate::infer::Types::of_decl(sigs, hir, decl_id_of(hir, decl), body);
            // Each binder by where it is bound: its term's place among the
            // declaration's (ADR-0063).
            let term = decl
                .term_roots()
                .position(|(_, r)| std::ptr::eq(r, transition));
            for j in 0..transition.binders.len() {
                if let Some(i) = term {
                    types = types.with_binding(crate::lexical::Binder::Term(i, j), &value_ty);
                }
            }
            let Some(produced) = types.of(body, transition.root) else {
                // No answer is not a violation. `docs/RISK_QUEUE.md`: an
                // analysis that could not run must not be read as a proof, and
                // it must not be read as a refutation either.
                continue;
            };
            if produced.same_as(&value_ty) {
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
) -> Option<crate::resolved::ResolvedType> {
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
    let ty = sigs.by_def(def)?.result()?;
    match ty.as_builtin() {
        Some(crate::resolved::Builtin::Result) => ty.args().first().cloned(),
        _ => Some(ty.clone()),
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

fn unresolved_bare_call(
    hir: &Hir,
    decl: &Decl,
    body: &Body,
    id: ExprId,
    path: &str,
    case_of: &[String],
) -> Diagnostic {
    // A case with a payload, written alone: what it is, and how it is built.
    if !case_of.is_empty() {
        let qualified: Vec<String> = case_of
            .iter()
            .map(|t| format!("`{t}.{path}(..)`"))
            .collect();
        return Diagnostic {
            code: crate::codes::UNRESOLVED_NAME.id,
            invariant: crate::codes::UNRESOLVED_NAME.invariant,
            reason: "unqualified_case",
            detector: Detector::DeclarationRule,
            severity: Severity::Error,
            message: format!("`{path}` does not resolve"),
            primary_span: body.expr_span(id),
            related: vec![Related {
                span: hir.decl_span(decl_id_of(hir, decl)),
                label: format!("called inside `{}`", decl.name),
            }],
            explanation: Some(format!(
                "`{path}` is a case, not a function: a case with a payload is \
                 built through its type, as the case's own declaration is \
                 reached through it (ADR-0059). No term is called `{path}`."
            )),
            repairs: vec![Repair {
                description: format!("write {}", qualified.join(" or ")),
                replacement: None,
            }],
        };
    }
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

    // E9-V: every place a value meets a declared type — call arity and
    // argument types, constructed fields, declared results, annotated
    // bindings, and written types that resolve to nothing. The diagnostics
    // are a projection of `values::relations`, which an audit can query.
    out.extend(crate::values::diagnostics(
        &crate::values::relations(&unit.hir, sigs, ws, at),
        at,
    ));

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
        effect_rows(
            &unit.hir, sigs, inference, ontology, ws, at, id, decl, &mut out,
        );
        crate::capability::capability_arguments(&unit.hir, decl, types, &mut out);
        markup_rules(&unit.hir, decl, &mut out);
        loop_keys(&unit.hir, id, decl, &mut out);
        template_blocks(&unit.hir, sigs, id, decl, &mut out);
        let Some(body_id) = decl.body else { continue };
        let body = unit.hir.body(body_id);

        // Local type facts: a parameter's declared type, and any `let x: T`.
        // The DECLARED type, not a rendering of it — `List<MenuItem>` written
        // back into a string is a type a consumer has to re-parse, and
        // re-parsing is where arguments get dropped.
        let mut locals: BTreeMap<String, hir::DeclaredType> = BTreeMap::new();
        for p in &decl.params {
            if let Some(t) = &p.ty {
                locals.insert(p.name.clone(), t.clone());
            }
        }

        let site = MatchSite {
            env,
            ws,
            sigs,
            at,
            module: unit.hir.module_of(id),
            decl,
            body,
            locals: &locals,
        };
        for match_id in body.walk() {
            if let Expr::Match { scrutinee, arms } = body.expr(match_id) {
                exhaustiveness(&site, match_id, *scrutinee, arms, &mut out);
            }
        }
        scopes(decl, body, &mut out);
    }
    // **One finding, one diagnostic.** Two rules can reach one defect: a field
    // read through an `Option` bound by `let` is PW0600 to `annotations` and to
    // the member relation (ADR-0048). The first reported is kept; the order
    // above puts the more specific rule first.
    let mut seen = BTreeSet::new();
    out.retain(|d| seen.insert((d.code, d.primary_span.start, d.primary_span.end)));
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
    /// Each arm no value reaches, by its pattern: the arms before it take
    /// every value it matches (ADR-0076). Empty where the analysis did not
    /// run.
    pub unreachable: Vec<crate::hir::Span>,
}

/// **Every `match` in the program, and what was concluded about it.**
///
/// The same walk `check_unit` does, so a `match` that reaches the checker
/// reaches this and vice versa. A separate walk here would let the two disagree
/// about which matches exist, and the disagreement would be invisible: an
/// audit would report a proof for a match no rule examined.
pub fn match_analysis(units: &[Unit]) -> Vec<MatchAnalysis> {
    let env = Env::build(units);
    // One workspace, because a scrutinee's type must be RESOLVED before the
    // environment is consulted — `Status` alone does not say whose.
    let hirs: Vec<&Hir> = units.iter().map(|u| &u.hir).collect();
    let ws = crate::resolve::Workspace::build(&hirs);
    // The value relations type a scrutinee no annotation types.
    let sigs = crate::signatures::Signatures::build(&ws, &hirs);
    let mut out = Vec::new();
    for (at, unit) in units.iter().enumerate() {
        for (decl_id, decl) in unit.hir.all_decls() {
            let Some(body_id) = decl.body else { continue };
            let body = unit.hir.body(body_id);
            let mut locals: BTreeMap<String, hir::DeclaredType> = BTreeMap::new();
            for p in &decl.params {
                if let Some(t) = &p.ty {
                    locals.insert(p.name.clone(), t.clone());
                }
            }
            let site = MatchSite {
                env: &env,
                ws: &ws,
                sigs: &sigs,
                at,
                module: unit.hir.module_of(decl_id),
                decl,
                body,
                locals: &locals,
            };
            for id in body.walk() {
                if let Expr::Match { scrutinee, arms } = body.expr(id) {
                    out.push(analyse_match(&site, &decl.name, id, *scrutinee, arms).0);
                }
            }
        }
    }
    out
}

/// **What a match is over**: its type as the analysis reads it, the
/// program that type's constructors live in, and its name, found once and
/// read by both the analysis and its diagnostic.
struct Subject {
    ty: Type,
    /// The analysis's types for this match (ADR-0060): each instance a
    /// pattern can take apart, as an ADT of its own.
    program: Program,
    /// Which declared sum type each of `program`'s ADTs is an instance of,
    /// for a pattern's qualifier.
    defs: BTreeMap<crate::types::AdtId, crate::resolve::DefId>,
    /// The opaque types that stand for a type the program does not state: a
    /// pattern against one is not read, where against a known type no
    /// pattern takes apart it is an error.
    unknown: BTreeSet<crate::types::OpaqueId>,
    /// The type as written or inferred, for messages.
    name: String,
}

/// Where a match is, and what it can read: the context both callers share.
struct MatchSite<'a> {
    env: &'a Env,
    ws: &'a crate::resolve::Workspace,
    sigs: &'a crate::signatures::Signatures,
    at: usize,
    module: Option<&'a str>,
    decl: &'a Decl,
    body: &'a Body,
    locals: &'a BTreeMap<String, hir::DeclaredType>,
}

/// **The type a match is over**, or why the analysis cannot say.
///
/// A bare name with a declared type reads the declaration. Anything else (a
/// call: `match Entries.get(word) { .. }`) is typed by the value relations,
/// which type calls already. Until 2026-09-25 a scrutinee that was not a bare
/// declared name, and every match over `Option` or `Result`, was Blocked, and
/// a match with its `None` arm missing passed `pw check`. The kiokun slice's
/// backend refused one, which is how it was found.
///
/// A parameter's declared type is read only where the name means the
/// parameter. A body that binds the name again (`Some(x) => match x { .. }`
/// in a function with a parameter `x`) is left to the value relations, which
/// read a name bound at two sites as unknown rather than guess which one is
/// meant. Until 2026-09-25 the parameter's type was read, and a match over the
/// arm's value was proven against the parameter's constructors.
fn subject(site: &MatchSite<'_>, scrutinee: ExprId) -> Result<Subject, (String, Option<String>)> {
    let MatchSite {
        env,
        ws,
        sigs,
        at,
        module,
        decl,
        body,
        locals,
    } = site;
    let span = body.expr_span(scrutinee);
    if let Expr::Name(n) = body.expr(scrutinee)
        && let Some(declared) = locals.get(n)
        && !rebinds(body, n)
    {
        // **Resolve, then look up.** `Status` alone does not say whose, and
        // two modules may declare one. The environment is handed an identity;
        // it never discovers one.
        let written = declared.written();
        let resolution = crate::resolved::resolve(ws, *at, None, &[], declared, span);
        let Some(t) = resolution.resolved() else {
            return Err((
                "the scrutinee's type does not resolve to a declaration".into(),
                Some(written),
            ));
        };
        return typed_subject(env, sigs, &crate::values::Ty::of(t), written);
    }
    match crate::values::type_of(sigs, ws, *at, *module, decl, body, scrutinee) {
        (crate::values::Ty::Unknown | crate::values::Ty::Var(_) | crate::values::Ty::Any, _) => {
            Err(("the scrutinee's type is unknown here".into(), None))
        }
        (ty, name) => typed_subject(env, sigs, &ty, name),
    }
}

/// A subject of a type a match takes apart: a sum type, the language's
/// `Option` or `Result`, a `Bool`, or an `Int` or a `String` by its
/// literals (ADR-0060).
fn typed_subject(
    _env: &Env,
    sigs: &Signatures,
    ty: &crate::values::Ty,
    name: String,
) -> Result<Subject, (String, Option<String>)> {
    let mut instances = Instances {
        sigs,
        program: Program::new(),
        memo: BTreeMap::new(),
        defs: BTreeMap::new(),
        unknown: BTreeSet::new(),
    };
    let t = instances.of(ty);
    match &t {
        Type::Adt(id) if instances.program.adt(*id).ctors.is_empty() => Err((
            "the scrutinee's type declares no constructors".into(),
            Some(name),
        )),
        Type::Opaque(id) if instances.unknown.contains(id) => {
            Err(("the scrutinee's type is unknown here".into(), Some(name)))
        }
        // A record, a list, a `Float`: known, and taken apart by no pattern,
        // so only `_` or a name matches one, and anything else is an error.
        _ => Ok(Subject {
            ty: t,
            program: instances.program,
            defs: instances.defs,
            unknown: instances.unknown,
            name,
        }),
    }
}

/// **The analysis's types for one match** (ADR-0060): each type a pattern
/// can take apart, declared as an ADT of its own instance, every
/// constructor's fields typed. `Option<Option<Int>>` and `Option<Int>` are
/// two ADTs, and a declared case's fields are its type's, under the
/// arguments it is applied to. Until 2026-09-26 the payloads of `Some`, `Ok`
/// and `Err` were one opaque type and a declared case's fields were read by
/// their spelling, so a pattern nested under either blocked the analysis.
struct Instances<'a> {
    sigs: &'a Signatures,
    program: Program,
    memo: BTreeMap<String, Type>,
    defs: BTreeMap<crate::types::AdtId, crate::resolve::DefId>,
    /// The opaque types standing for a type the program does not state.
    unknown: BTreeSet<crate::types::OpaqueId>,
}

impl Instances<'_> {
    fn of(&mut self, ty: &crate::values::Ty) -> Type {
        use crate::resolved::{Builtin, Primitive};
        use crate::values::Ty;
        match ty {
            Ty::Primitive(Primitive::Int) => Type::Int,
            Ty::Primitive(Primitive::Str) => Type::Str,
            Ty::Primitive(Primitive::Bool) => Type::Bool,
            Ty::Builtin(Builtin::Option, a) if a.len() == 1 => {
                let inner = a[0].clone();
                self.adt(ty, None, move |me| {
                    vec![
                        ("Some".to_string(), vec![me.of(&inner)]),
                        ("None".to_string(), Vec::new()),
                    ]
                })
            }
            Ty::Builtin(Builtin::Result, a) if a.len() == 2 => {
                let (ok, err) = (a[0].clone(), a[1].clone());
                self.adt(ty, None, move |me| {
                    vec![
                        ("Ok".to_string(), vec![me.of(&ok)]),
                        ("Err".to_string(), vec![me.of(&err)]),
                    ]
                })
            }
            Ty::Nominal(def, args)
                if self
                    .sigs
                    .type_decl(*def)
                    .is_some_and(|t| t.variants.is_some()) =>
            {
                let cases = self
                    .sigs
                    .type_decl(*def)
                    .and_then(|t| t.variants.clone())
                    .unwrap_or_default();
                let (def, args) = (*def, args.clone());
                self.adt(ty, Some(def), move |me| {
                    cases
                        .iter()
                        .map(|(name, fields)| {
                            let fields = fields
                                .iter()
                                .map(|f| match f.resolved() {
                                    Some(t) => {
                                        me.of(&crate::values::substituted(&Ty::of(t), def, &args))
                                    }
                                    None => me.opaque(&Ty::Unknown),
                                })
                                .collect();
                            (name.clone(), fields)
                        })
                        .collect()
                })
            }
            other => self.opaque(other),
        }
    }

    /// An instance's ADT, declared once: before its constructors are
    /// computed, so a type that contains itself finds its own.
    fn adt(
        &mut self,
        ty: &crate::values::Ty,
        def: Option<crate::resolve::DefId>,
        ctors: impl FnOnce(&mut Self) -> Vec<(String, Vec<Type>)>,
    ) -> Type {
        let key = format!("{ty:?}");
        if let Some(t) = self.memo.get(&key) {
            return t.clone();
        }
        let id = self.program.declare_adt(&self.name(ty), Vec::new());
        if let Some(d) = def {
            self.defs.insert(id, d);
        }
        self.memo.insert(key, Type::Adt(id));
        let ctors = ctors(self);
        self.program.adts[id].ctors = ctors
            .into_iter()
            .map(|(name, fields)| Ctor { name, fields })
            .collect();
        Type::Adt(id)
    }

    /// A type no pattern takes apart: a record, an opaque type, a `Float`,
    /// a list, a type the program does not state.
    fn opaque(&mut self, ty: &crate::values::Ty) -> Type {
        let key = format!("{ty:?}");
        if let Some(t) = self.memo.get(&key) {
            return t.clone();
        }
        let id = self.program.declare_opaque(&self.name(ty), Type::Str);
        if matches!(
            ty,
            crate::values::Ty::Unknown | crate::values::Ty::Var(_) | crate::values::Ty::Any
        ) {
            self.unknown.insert(id);
        }
        let t = Type::Opaque(id);
        self.memo.insert(key, t.clone());
        t
    }

    /// A type as a witness names it: `Option<Int>`, `Shape`, `Maybe<String>`.
    fn name(&self, ty: &crate::values::Ty) -> String {
        use crate::values::Ty;
        let args = |xs: &[Ty]| match xs.is_empty() {
            true => String::new(),
            false => format!(
                "<{}>",
                xs.iter()
                    .map(|x| self.name(x))
                    .collect::<Vec<_>>()
                    .join(", ")
            ),
        };
        match ty {
            Ty::Primitive(p) => p.name().to_string(),
            Ty::Builtin(b, xs) => format!("{}{}", b.name(), args(xs)),
            Ty::Nominal(def, xs) => {
                let path = self.sigs.path_of(*def).unwrap_or("?");
                let bare = path.rsplit('.').next().unwrap_or(path);
                format!("{bare}{}", args(xs))
            }
            Ty::Parameter { .. } | Ty::Var(_) | Ty::Unknown | Ty::Any => "_".to_string(),
        }
    }
}

/// Whether a pattern in this body binds `name`, as a `let` or an arm does.
fn rebinds(body: &Body, name: &str) -> bool {
    body.pats
        .iter()
        .any(|(_, p, _)| matches!(p, HPat::Bind { name: n, .. } if n == name))
}

/// A constructor pattern naming a constructor its type does not have.
struct Foreign {
    constructor: String,
    /// The type the pattern is read against, and that type's constructors.
    ty: String,
    ctors: Vec<String>,
    /// What a pattern against a type without constructors may be, said
    /// instead of the list (ADR-0060).
    note: Option<String>,
    span: hir::Span,
}

/// A constructor pattern binding a different number of fields than its
/// constructor declares.
struct Arity {
    ctor: String,
    written: usize,
    declared: usize,
    span: hir::Span,
}

/// What a match's diagnostics are read from, beside its verdict.
enum Found {
    /// No subject. The audit says why, and there is nothing to report.
    Nothing,
    /// The subject, and each arm as the analysis read it, or why it could
    /// not. Kept per arm, so one arm's fault never hides another's.
    Arms(Box<Subject>, Vec<Result<Arm, PatternFault>>),
}

/// The analysis, with no diagnostics in it, and what they are read from.
fn analyse_match(
    site: &MatchSite<'_>,
    declaration: &str,
    match_id: ExprId,
    scrutinee: ExprId,
    arms: &[hir::MatchArm],
) -> (MatchAnalysis, Found) {
    let (env, body) = (site.env, site.body);
    let span = body.expr_span(match_id);
    let blocked = |reason: &str, ty: Option<String>| MatchAnalysis {
        declaration: declaration.to_string(),
        scrutinee_type: ty,
        span: span.clone(),
        outcome: MatchOutcome::Blocked {
            reason: reason.to_string(),
        },
        unreachable: Vec::new(),
    };
    let subject = match subject(site, scrutinee) {
        Ok(s) => s,
        Err((reason, ty)) => return (blocked(&reason, ty), Found::Nothing),
    };
    // `Shape.Circle(r)`: the type a qualified pattern names, resolved where
    // the match is written (ADR-0059).
    let qualifier = |q: &str| {
        use crate::resolve::{Namespace, Resolution};
        let found = match q.contains('.') {
            true => site.ws.resolve_path_in(site.at, Namespace::Type, q),
            false => site.ws.resolve_in(site.at, Namespace::Type, q),
        };
        match found {
            Resolution::Local(d) | Resolution::Imported { def: d, .. } => Some(d),
            _ => None,
        }
    };
    // Each literal the match's patterns write, as a constructor of its
    // type's own (ADR-0060): `1` and `2` are two, and no list of them is
    // every `Int`.
    let literals = std::cell::RefCell::new(BTreeMap::new());
    let read: Vec<Result<Arm, PatternFault>> = arms
        .iter()
        .map(|a| {
            to_exhaust_pattern(
                env,
                &subject,
                body,
                &qualifier,
                &literals,
                a.pat,
                &subject.ty,
                &subject.name,
            )
            .map(|pattern| Arm {
                pattern,
                span: body.pat_span(a.pat),
            })
        })
        .collect();
    // The audit's reason is the first fault, in the order an author would
    // repair them: a constructor of another type, then a field count, then
    // what the analysis does not read.
    let fault = read
        .iter()
        .filter_map(|r| r.as_ref().err())
        .min_by_key(|f| match f {
            PatternFault::Foreign(_) => 0,
            PatternFault::Arity(_) => 1,
            PatternFault::Unread(_) => 2,
        });
    if let Some(f) = fault {
        let why = match f {
            PatternFault::Foreign(f) => {
                format!("`{}` is not a constructor of `{}`", f.constructor, f.ty)
            }
            PatternFault::Arity(a) => format!(
                "`{}` binds {} field(s) but declares {}",
                a.ctor, a.written, a.declared
            ),
            PatternFault::Unread(why) => why.clone(),
        };
        let name = subject.name.clone();
        return (
            blocked(&why, Some(name)),
            Found::Arms(Box::new(subject), read),
        );
    }
    let lowered: Vec<Arm> = read.into_iter().filter_map(Result::ok).collect();

    let report = exhaust::check_match(&subject.program, &subject.ty, &lowered);
    // Which arms no value reaches, where the analysis ran. `exhaust.rs` found
    // them all along; until 2026-09-26 nothing read them (ADR-0076).
    let unreachable: Vec<crate::hir::Span> = match report.blocked.is_empty() {
        true => report
            .unreachable
            .iter()
            .filter_map(|&i| lowered.get(i).map(|a| a.span.clone()))
            .collect(),
        false => Vec::new(),
    };
    let outcome = match report.outcome() {
        crate::outcome::Outcome::Proven(_) => MatchOutcome::Proven,
        crate::outcome::Outcome::Blocked(bs) => MatchOutcome::Blocked {
            reason: format!("{bs:?}"),
        },
        crate::outcome::Outcome::Violation(_) => MatchOutcome::NonExhaustive {
            missing: report
                .missing
                .iter()
                .map(|w| exhaust::render_witness(&subject.program, &subject.ty, w))
                .collect(),
        },
    };
    (
        MatchAnalysis {
            declaration: declaration.to_string(),
            scrutinee_type: Some(subject.name.clone()),
            span,
            outcome,
            unreachable,
        },
        Found::Arms(Box::new(subject), lowered.into_iter().map(Ok).collect()),
    )
}

fn exhaustiveness(
    site: &MatchSite<'_>,
    match_id: ExprId,
    scrutinee: ExprId,
    arms: &[hir::MatchArm],
    out: &mut Vec<Diagnostic>,
) {
    // **The verdict and its subject come from `analyse_match`.**
    //
    // Architect ruling, 2026-08-10: a diagnostic is a projection of an analysis
    // result. Deriving it twice would let the report an audit reads and the
    // error a developer reads disagree, and the disagreement would be silent —
    // which is `docs/RISK_QUEUE.md`'s most common shape. Until 2026-09-25 this
    // function re-derived the scrutinee's type for its messages; it now reads
    // the one `analyse_match` found.
    let body = site.body;
    let (analysis, found) = analyse_match(site, "", match_id, scrutinee, arms);
    let Found::Arms(subject, read) = found else {
        return;
    };
    let ty_name = &subject.name;

    // Each arm's own defect, where it is. A pattern whose arity disagrees with
    // its constructor, or that names a constructor its type lacks, is a
    // defect in its own right, and reporting it means the author is told what
    // is actually wrong, rather than being told about a missing variant that
    // follows from it. Both are found at any depth, and are never a reason to
    // stop reading the other arms.
    //
    // `exhaust.rs` defends against an arity mismatch anyway. Two layers,
    // because "upstream validated it" is the assumption that produced the
    // panic.
    for fault in read.into_iter().filter_map(Result::err) {
        match fault {
            PatternFault::Foreign(f) => out.push(foreign_constructor(body, match_id, &subject, f)),
            PatternFault::Arity(a) => out.push(constructor_arity(body, match_id, &subject, a)),
            PatternFault::Unread(_) => {}
        }
    }

    // **An arm no value reaches** (ADR-0076): the arms before it take every
    // value it matches, so the code in it is dead and the value it was
    // written for goes to an earlier arm. Until 2026-09-26 the analysis found
    // these and nothing reported them: `_ => 0` before `Circle(r) => r`
    // checked, the backend refused a case matched twice, and a literal
    // matched twice built with its second arm dead.
    for arm in &analysis.unreachable {
        out.push(Diagnostic {
            code: crate::codes::UNREACHABLE_ARM.id,
            invariant: crate::codes::UNREACHABLE_ARM.invariant,
            reason: "unreachable_arm",
            detector: Detector::PatternMatrix,
            severity: Severity::Error,
            message: format!(
                "no `{ty_name}` reaches this arm: the arms before it take every value it matches"
            ),
            primary_span: arm.clone(),
            related: vec![Related {
                span: body.expr_span(scrutinee),
                label: format!("this has type `{ty_name}`"),
            }],
            explanation: Some(
                "A match tries its arms in order, so an arm whose values the arms before it \
                 already take never runs."
                    .to_string(),
            ),
            repairs: vec![Repair {
                description: "remove the arm, or move it before the arm that takes its values"
                    .to_string(),
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
        explanation: Some(match subject.program.ctors_of(&subject.ty) {
            Some(ctors) if !subject.ty.is_infinite() => format!(
                "`{ty_name}` has {} constructor(s), and {} value(s) no arm takes. \
                 Adding a variant to a type must break every match on it at compile \
                 time, which is why a wildcard arm is not the default repair.",
                ctors.len(),
                missing.len(),
            ),
            // `Int` and `String`: no list of literals is every value (ADR-0060).
            _ => format!(
                "A `{ty_name}` has more values than any list of literals names, so \
                 a match over one needs an arm that takes the rest."
            ),
        }),
        repairs: match subject.ty.is_infinite() {
            // No list of literals is every `Int`: the rest is one arm.
            true => vec![Repair {
                description: "add an arm `_ =>` for the values no literal names".to_string(),
                replacement: None,
            }],
            false => missing
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
        },
    });
}

/// **PW0603**: a constructor pattern binding a different number of fields
/// than its constructor declares.
fn constructor_arity(body: &Body, match_id: ExprId, subject: &Subject, a: Arity) -> Diagnostic {
    Diagnostic {
        code: crate::codes::CONSTRUCTOR_ARITY.id,
        invariant: crate::codes::CONSTRUCTOR_ARITY.invariant,
        reason: "constructor_pattern_arity_mismatch",
        detector: Detector::PatternMatrix,
        severity: Severity::Error,
        message: format!(
            "`{}` binds {} field(s) but declares {}",
            a.ctor, a.written, a.declared
        ),
        primary_span: a.span,
        related: vec![Related {
            span: body.expr_span(match_id),
            label: format!("matching on `{}`", subject.name),
        }],
        explanation: Some(format!(
            "A constructor pattern binds its constructor's fields, so the count \
             is not a style choice — `{}` carries {} of them. Writing a different \
             number leaves the compiler with a pattern that does not describe any \
             value of this type.",
            a.ctor, a.declared
        )),
        repairs: vec![Repair {
            description: if a.written < a.declared {
                format!(
                    "bind the remaining field(s), or write `{}(_)`-style wildcards",
                    a.ctor
                )
            } else {
                format!("`{}` takes {}", a.ctor, a.declared)
            },
            replacement: None,
        }],
    }
}

/// **PW0608**: a constructor pattern whose constructor its type does not have.
fn foreign_constructor(body: &Body, match_id: ExprId, subject: &Subject, f: Foreign) -> Diagnostic {
    Diagnostic {
        code: crate::codes::PATTERN_CONSTRUCTOR.id,
        invariant: crate::codes::PATTERN_CONSTRUCTOR.invariant,
        reason: "pattern_names_a_constructor_its_type_lacks",
        detector: Detector::PatternMatrix,
        severity: Severity::Error,
        message: format!("`{}` is not a constructor of `{}`", f.constructor, f.ty),
        primary_span: f.span,
        related: vec![Related {
            span: body.expr_span(match_id),
            label: format!("matching on `{}`", subject.name),
        }],
        explanation: Some(format!(
            "A constructor pattern matches the values its constructor builds, and \
             no value of `{}` is built by `{}`. The arm can never match. Read as \
             a binding, it would match everything and hide the cases it misses.",
            f.ty, f.constructor
        )),
        repairs: vec![Repair {
            description: match f.note {
                // A type without constructors (ADR-0060).
                Some(note) => note,
                None => format!(
                    "`{}`'s constructors are {}",
                    f.ty,
                    f.ctors
                        .iter()
                        .map(|c| format!("`{c}`"))
                        .collect::<Vec<_>>()
                        .join(", ")
                ),
            },
            replacement: None,
        }],
    }
}

/// Why a pattern has no reading in the analysis.
enum PatternFault {
    /// A type error, reported as PW0608.
    Foreign(Foreign),
    /// A field count its constructor does not declare, reported as PW0603.
    Arity(Arity),
    /// Not an error, and not analysed. The analysis is Blocked, with this
    /// reason.
    Unread(String),
}

/// HIR pattern → the usefulness algorithm's pattern.
///
/// The load-bearing case: a nullary constructor has no parentheses, so the
/// grammar cannot tell `Draft` from a binding and emits `Pattern::Bind`. Read
/// as a binding it matches everything, which makes the first arm cover the
/// scrutinee and reports a non-exhaustive match as exhaustive — the failure is
/// silent and favourable, which is the shape `docs/RISK_QUEUE.md` tracks.
///
/// **Each pattern is read against its own type.** Until 2026-09-25 every
/// pattern, at any depth, was read against the constructors of the
/// scrutinee's type, and a constructor that type does not have read as a
/// wildcard. So `Ok(x)` and `Err(e)` arms proved a match over an `Option`
/// exhaustive, `Circle(Draft)` read as `Circle(_)`, and a literal covered
/// every value: each a proof of something false, of the same favourable shape
/// as above. Now a nested pattern is read against its field's type, a
/// constructor its type does not have is PW0608, and what the analysis cannot
/// read blocks it, with the reason: a literal, or a constructor pattern
/// against a type whose constructors it does not know, such as `Some`'s
/// payload.
#[allow(clippy::too_many_arguments)]
fn to_exhaust_pattern(
    env: &Env,
    subject: &Subject,
    body: &Body,
    qualifier: &dyn Fn(&str) -> Option<crate::resolve::DefId>,
    literals: &std::cell::RefCell<BTreeMap<String, usize>>,
    id: hir::PatternId,
    ty: &Type,
    ty_name: &str,
) -> Result<EPat, PatternFault> {
    let program = &subject.program;
    let ctors = program.ctors_of(ty);
    let unread = || {
        PatternFault::Unread(format!(
            "a pattern against `{ty_name}` is not analysed: the analysis does not know \
             that type's constructors"
        ))
    };
    let foreign = |name: &str, cs: &[Ctor]| {
        PatternFault::Foreign(Foreign {
            constructor: name.to_string(),
            ty: ty_name.to_string(),
            ctors: cs.iter().map(|c| c.name.clone()).collect(),
            note: None,
            span: body.pat_span(id),
        })
    };
    // A type without constructors: an `Int` or a `String` is matched by its
    // literals, and a record, a list or a `Float` by no pattern at all. A
    // type the program does not state is not read (ADR-0060).
    let untakeable = |name: &str| match ty {
        Type::Opaque(o) if subject.unknown.contains(o) => unread(),
        _ => PatternFault::Foreign(Foreign {
            constructor: name.to_string(),
            ty: ty_name.to_string(),
            ctors: Vec::new(),
            note: Some(match ty {
                Type::Int | Type::Str => {
                    format!("a pattern against `{ty_name}` is one of its literals, or `_`")
                }
                _ => format!(
                    "a `{ty_name}` is taken apart by no pattern: match it with `_` or a name"
                ),
            }),
            span: body.pat_span(id),
        }),
    };
    match body.pat(id) {
        HPat::Wild | HPat::Error => Ok(EPat::Wildcard),
        // A literal is a constructor of its type's own (ADR-0060). An `Int`
        // or a `String` has more of them than any match writes, so only an
        // arm taking the rest covers one; the analysis says which values
        // the literals leave, as `_`.
        HPat::Literal(l) => {
            let key = match (ty, l) {
                (Type::Int, hir::Literal::Int(n)) => n
                    .replace('_', "")
                    .parse::<i64>()
                    .ok()
                    .map(|n| format!("i:{n}")),
                (Type::Str, hir::Literal::Str(_)) => l.string_value().map(|v| format!("s:{v}")),
                _ => None,
            };
            match key {
                Some(k) => {
                    let mut seen = literals.borrow_mut();
                    let next = seen.len();
                    Ok(EPat::unit(*seen.entry(k).or_insert(next)))
                }
                // A `Float` literal: equality on floats decides no case.
                None if matches!(l, hir::Literal::Float(_)) && ty_name == "Float" => Err(
                    PatternFault::Unread("a `Float` literal pattern is not analysed".to_string()),
                ),
                None => {
                    let written = match l {
                        hir::Literal::Int(t)
                        | hir::Literal::Float(t)
                        | hir::Literal::Str(t)
                        | hir::Literal::UnterminatedStr(t) => t.clone(),
                    };
                    Err(match &ctors {
                        Some(cs) => foreign(&written, cs),
                        None => untakeable(&written),
                    })
                }
            }
        }
        HPat::Bind { name, .. } => {
            if let Some(cs) = &ctors
                && let Some(i) = cs.iter().position(|c| &c.name == name)
            {
                // `Some` alone, for a constructor that carries a field.
                return match cs[i].fields.len() {
                    0 => Ok(EPat::unit(i)),
                    declared => Err(PatternFault::Arity(Arity {
                        ctor: name.clone(),
                        written: 0,
                        declared,
                        span: body.pat_span(id),
                    })),
                };
            }
            // Another type's constructor is not a fresh binding: `None`
            // against a `Status` names `Option`'s case, and `true` against
            // an `Int` names a `Bool`.
            if env.names_a_constructor(name) || matches!(name.as_str(), "true" | "false") {
                return Err(match &ctors {
                    Some(cs) => foreign(name, cs),
                    None => untakeable(name),
                });
            }
            Ok(EPat::Wildcard)
        }
        HPat::Ctor { path, args } => {
            // `DecodeError.Invalid` and `Invalid` name the same constructor,
            // where the qualifier names the type matched (ADR-0059). Read by
            // its last segment alone, `Other.Invalid` matched a `DecodeError`.
            let short = path.rsplit('.').next().unwrap_or(path);
            let Some(cs) = ctors else {
                return Err(untakeable(path));
            };
            if let Some((q, _)) = path.rsplit_once('.')
                && !matches!(ty, Type::Adt(t)
                    if qualifier(q).is_some_and(|d| subject.defs.get(t) == Some(&d)))
            {
                return Err(foreign(path, &cs));
            }
            let Some(i) = cs.iter().position(|c| c.name == short) else {
                return Err(foreign(short, &cs));
            };
            let fields = &cs[i].fields;
            if args.len() != fields.len() {
                return Err(PatternFault::Arity(Arity {
                    ctor: short.to_string(),
                    written: args.len(),
                    declared: fields.len(),
                    span: body.pat_span(id),
                }));
            }
            let args = args
                .iter()
                .zip(fields)
                .map(|(a, t)| {
                    to_exhaust_pattern(
                        env,
                        subject,
                        body,
                        qualifier,
                        literals,
                        *a,
                        t,
                        &program.type_name(t),
                    )
                })
                .collect::<Result<Vec<_>, _>>()?;
            Ok(EPat::ctor(i, args))
        }
        HPat::Or(ps) => ps
            .iter()
            .map(|p| to_exhaust_pattern(env, subject, body, qualifier, literals, *p, ty, ty_name))
            .collect::<Result<Vec<_>, _>>()
            .map(EPat::Or),
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
    let labels = crate::labels::Labels::of_decl(sigs, hir, id, body, &imports);

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
            && let Some((_, origin)) = labels.origin(id)
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
    let labels = crate::labels::Labels::of_decl(sigs, hir, id, body, &imports);
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
                    Expr::Name(n) => labels.origin(e).map(|(_, o)| (n.clone(), o.clone())),
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
/// **A template block's markers, read** (ADR-0042).
///
/// Until 2026-09-25 nothing read them. `{:else}` was dropped, so both of an
/// `if`'s branches rendered together; `{#if a} .. {/each}` closed the `if`;
/// and an unknown directive passed `pw check`, refused only when rendered.
/// **A loop's key is its element, or a field read from it** (ADR-0073).
///
/// `{#each rs as x (x.id)}` keys each instance on its element's `id`. Until
/// 2026-09-26 the key's head was not read: the template IR kept the key's last
/// segment and keyed on that field of the element. So `(item.id)` in a loop
/// over `x` keyed on `x.id`, and `(k.r.id)` on `k.id`, silently.
fn loop_keys(hir: &Hir, id: crate::hir::DeclId, decl: &Decl, out: &mut Vec<Diagnostic>) {
    let Some(body_id) = decl.body else { return };
    let body = hir.body(body_id);
    let mut roots = Vec::new();
    for e in body.walk() {
        if let Expr::Template { roots: r, .. } = body.expr(e) {
            roots.extend(r.iter().copied());
        }
    }
    for n in body.walk_markup(&roots) {
        let Node::Block { directive, .. } = body.node(n) else {
            continue;
        };
        let Some((binding, _, Some(key))) = crate::template_ir::each_parts(directive) else {
            continue;
        };
        let head = key.split('.').next().unwrap_or_default().trim();
        if head == binding {
            continue;
        }
        out.push(Diagnostic {
            code: crate::codes::LOOP_KEY.id,
            invariant: crate::codes::LOOP_KEY.invariant,
            reason: "loop_key",
            detector: Detector::DeclarationRule,
            severity: Severity::Error,
            message: format!(
                "a loop's key is `{binding}` or a field read from it, and this is `{key}`"
            ),
            primary_span: body.node_span(n),
            related: vec![Related {
                span: hir.decl_span(id),
                label: format!("`{}` renders this", decl.name),
            }],
            explanation: Some(
                "A key identifies each element of the list, so it is read from the element. \
                 Until 2026-09-26 a key whose head was another name keyed on a field of the \
                 element, silently."
                    .to_string(),
            ),
            repairs: vec![Repair {
                description: format!("key the loop on `{binding}`, or on a field read from it"),
                replacement: None,
            }],
        });
    }
}

fn template_blocks(
    hir: &Hir,
    sigs: &Signatures,
    id: crate::hir::DeclId,
    decl: &Decl,
    out: &mut Vec<Diagnostic>,
) {
    use crate::resolved::Builtin;
    let Some(body_id) = decl.body else { return };
    let body = hir.body(body_id);
    let mut roots = Vec::new();
    for e in body.walk() {
        if let Expr::Template { roots: r, .. } = body.expr(e) {
            roots.extend(r.iter().copied());
        }
    }
    if roots.is_empty() {
        return;
    }
    let types = crate::infer::Types::of_decl(sigs, hir, id, body);
    let at = hir.decl_span(id);
    let related = || {
        vec![Related {
            span: at.clone(),
            label: format!("`{}` renders this", decl.name),
        }]
    };
    let malformed = |span: hir::Span, message: String| Diagnostic {
        code: crate::codes::MALFORMED_TEMPLATE_BLOCK.id,
        invariant: crate::codes::MALFORMED_TEMPLATE_BLOCK.invariant,
        reason: "malformed_template_block",
        detector: Detector::DeclarationRule,
        severity: Severity::Error,
        message,
        primary_span: span,
        related: related(),
        explanation: Some(
            "A block's markers decide what renders. A marker the block does not take \
             is not ignored safely: until 2026-09-25 a dropped `{:else}` rendered both \
             of an `if`'s branches at once."
                .to_string(),
        ),
        repairs: Vec::new(),
    };

    let mut inside = BTreeSet::new();
    for n in body.walk_markup(&roots) {
        let Node::Block {
            directive,
            children,
            subject,
            close,
        } = body.node(n)
        else {
            continue;
        };
        let d = directive.trim();
        let span = body.node_span(n);
        // Each branch marker with what it carries: as written, whether it has
        // a condition, and the arm it names.
        type Marker<'b> = (hir::NodeId, String, bool, &'b Option<hir::TemplateArm>);
        let branches: Vec<Marker<'_>> = children
            .iter()
            .filter_map(|c| match body.node(*c) {
                Node::Branch {
                    marker,
                    condition,
                    arm,
                } => Some((*c, marker.trim().to_string(), condition.is_some(), arm)),
                _ => None,
            })
            .collect();
        inside.extend(branches.iter().map(|b| b.0));
        // A stray closer, which recovery keeps as a block of its own.
        if d.starts_with("{/") {
            out.push(malformed(span, format!("`{d}` closes no block")));
            continue;
        }
        let name: String = d
            .trim_start_matches("{#")
            .chars()
            .take_while(|c| c.is_alphanumeric() || *c == '_')
            .collect();
        if !matches!(name.as_str(), "if" | "each" | "match") {
            out.push(malformed(
                span,
                format!(
                    "`{d}` is not a template block: the blocks are `{{#if}}`, `{{#each}}` \
                     and `{{#match}}`"
                ),
            ));
            continue;
        }
        let closer = close
            .trim()
            .trim_start_matches("{/")
            .trim_end_matches('}')
            .trim();
        if close.is_empty() {
            out.push(malformed(
                span.clone(),
                format!("`{{#{name}}}` is never closed"),
            ));
        } else if closer != name {
            out.push(malformed(
                span.clone(),
                format!(
                    "`{{#{name}}}` closes with `{}`, not `{{/{name}}}`",
                    close.trim()
                ),
            ));
        }
        match name.as_str() {
            "each" => {
                if let Some((b, marker, ..)) = branches.first() {
                    out.push(malformed(
                        body.node_span(*b),
                        format!(
                            "`{{#each}}` takes no `{marker}`; `{{#if xs}}` around the list \
                             says what renders when it is empty"
                        ),
                    ));
                }
            }
            "if" => {
                for (i, (b, marker, else_if, _)) in branches.iter().enumerate() {
                    let last = i + 1 == branches.len();
                    if !(*else_if || (marker == "{:else}" && last)) {
                        out.push(malformed(
                            body.node_span(*b),
                            format!(
                                "an `{{#if}}` takes `{{:else if c}}` markers and one \
                                 `{{:else}}`, last; not `{marker}` here"
                            ),
                        ));
                    }
                }
                // An `Option` or a `Result` is taken apart, not tested.
                if let Some(s) = subject
                    && let Some(ty) = types.of(body, *s)
                    && matches!(ty.as_builtin(), Some(Builtin::Option | Builtin::Result))
                {
                    let written = crate::infer::path_of(body, *s);
                    out.push(Diagnostic {
                        code: crate::codes::OPTION_USED_AS_VALUE.id,
                        invariant: crate::codes::OPTION_USED_AS_VALUE.invariant,
                        reason: "optional_tested_by_if",
                        detector: Detector::PatternMatrix,
                        severity: Severity::Error,
                        message: format!(
                            "`{{#if {written}}}` tests a value that may be absent; \
                             `{{#match {written}}}` takes it apart"
                        ),
                        primary_span: span.clone(),
                        related: related(),
                        explanation: None,
                        repairs: vec![Repair {
                            description: format!(
                                "write `{{#match {written}}}{{:Some(x)}} .. {{:None}} .. {{/match}}`"
                            ),
                            replacement: None,
                        }],
                    });
                }
            }
            "match" => {
                // `{#match}`: nothing before the first arm.
                let stray = children
                    .iter()
                    .take_while(|c| !matches!(body.node(**c), Node::Branch { .. }))
                    .any(|c| !matches!(body.node(*c), Node::Text(t) if t.trim().is_empty()));
                if stray {
                    out.push(malformed(
                        span.clone(),
                        "a `{#match}` holds only its arms: nothing may come before the first"
                            .to_string(),
                    ));
                }
                // What the subject is, and its cases with each one's field
                // count: the language's `Option` or `Result`, or a declared
                // sum type (ADR-0061). A subject of unknown type takes its
                // family from its first arm, if that arm is the language's.
                // A subject's family: its name, each case with its field
                // count, and the declaration of a declared one.
                type Family = (String, Vec<(String, usize)>, Option<crate::resolve::DefId>);
                let builtin = |family: &str| -> Family {
                    let cases = match family {
                        "Option" => vec![("Some".to_string(), 1), ("None".to_string(), 0)],
                        _ => vec![("Ok".to_string(), 1), ("Err".to_string(), 1)],
                    };
                    (family.to_string(), cases, None)
                };
                let family_of = |case: &str| match case {
                    "Some" | "None" => Some("Option"),
                    "Ok" | "Err" => Some("Result"),
                    _ => None,
                };
                let subject_ty = subject.and_then(|s| types.of(body, s));
                let mut cases: Option<Family> = match &subject_ty {
                    Some(ty) => match ty.as_builtin() {
                        Some(Builtin::Option) => Some(builtin("Option")),
                        Some(Builtin::Result) => Some(builtin("Result")),
                        _ => {
                            let declared = ty.def_id().and_then(|d| {
                                let vs = sigs.type_decl(d)?.variants.as_ref()?;
                                Some((d, vs))
                            });
                            let Some((def, vs)) = declared else {
                                out.push(malformed(
                                    span.clone(),
                                    format!(
                                        "a `{{#match}}` takes an `Option`, a `Result` or a \
                                             sum type apart, and its subject is `{ty}`"
                                    ),
                                ));
                                continue;
                            };
                            Some((
                                ty.to_string(),
                                vs.iter().map(|(n, fs)| (n.clone(), fs.len())).collect(),
                                Some(def),
                            ))
                        }
                    },
                    None => None,
                };
                let unit = sigs.unit_of(hir.module_of(id));
                let mut seen: Vec<String> = Vec::new();
                for (b, marker, _, arm) in &branches {
                    let Some(arm) = arm else {
                        out.push(malformed(
                            body.node_span(*b),
                            format!(
                                "`{marker}` is not an arm: a `{{#match}}` arm is a case, \
                                 `{{:Some(x)}}`, `{{:None}}` or a declared case like \
                                 `{{:Circle(r)}}`"
                            ),
                        ));
                        continue;
                    };
                    let short = arm.short().to_string();
                    if cases.is_none() {
                        match family_of(&short) {
                            Some(f) => cases = Some(builtin(f)),
                            None => {
                                out.push(malformed(
                                    body.node_span(*b),
                                    format!(
                                        "`{short}` is a declared type's case, and the \
                                         subject's type is not known here"
                                    ),
                                ));
                                continue;
                            }
                        }
                    }
                    let Some((ty_name, declared, def)) = cases.as_ref() else {
                        continue;
                    };
                    let not_a_case = |why: &str| Diagnostic {
                        code: crate::codes::PATTERN_CONSTRUCTOR.id,
                        invariant: crate::codes::PATTERN_CONSTRUCTOR.invariant,
                        reason: "pattern_names_a_constructor_its_type_lacks",
                        detector: Detector::PatternMatrix,
                        severity: Severity::Error,
                        message: format!("`{why}` is not a constructor of `{ty_name}`"),
                        primary_span: body.node_span(*b),
                        related: related(),
                        explanation: None,
                        repairs: vec![Repair {
                            description: format!(
                                "`{ty_name}`'s constructors are {}",
                                declared
                                    .iter()
                                    .map(|(c, _)| format!("`{c}`"))
                                    .collect::<Vec<_>>()
                                    .join(", ")
                            ),
                            replacement: None,
                        }],
                    };
                    // `{:Shape.Circle(r)}`: the qualifier names the type
                    // matched.
                    if let Some((q, _)) = arm.case.rsplit_once('.') {
                        use crate::resolve::{Namespace, Resolution};
                        let named = unit.map(|u| match q.contains('.') {
                            true => sigs.workspace().resolve_path_in(u, Namespace::Type, q),
                            false => sigs.workspace().resolve_in(u, Namespace::Type, q),
                        });
                        let same = matches!(
                            (named, def),
                            (Some(Resolution::Local(d) | Resolution::Imported { def: d, .. }), Some(t))
                                if d == *t
                        );
                        if !same {
                            out.push(not_a_case(&arm.case));
                            continue;
                        }
                    }
                    let Some((_, fields)) = declared.iter().find(|(c, _)| *c == short) else {
                        out.push(not_a_case(&short));
                        continue;
                    };
                    // A payload's fields, bound all or not at all.
                    if !arm.bindings.is_empty() && arm.bindings.len() != *fields {
                        out.push(Diagnostic {
                            code: crate::codes::CONSTRUCTOR_ARITY.id,
                            invariant: crate::codes::CONSTRUCTOR_ARITY.invariant,
                            reason: "constructor_pattern_arity_mismatch",
                            detector: Detector::PatternMatrix,
                            severity: Severity::Error,
                            message: format!(
                                "`{short}` binds {} field(s) but declares {fields}",
                                arm.bindings.len()
                            ),
                            primary_span: body.node_span(*b),
                            related: related(),
                            explanation: None,
                            repairs: vec![Repair {
                                description: format!(
                                    "bind each of `{short}`'s {fields} field(s), or none"
                                ),
                                replacement: None,
                            }],
                        });
                    }
                    // A declared case named like the language's own reaches a
                    // template as that name, which is the language's.
                    if def.is_some() && family_of(&short).is_some() {
                        out.push(malformed(
                            body.node_span(*b),
                            format!(
                                "`{short}` is a declared case named like the language's own, \
                                 which a template cannot tell apart"
                            ),
                        ));
                    }
                    if seen.contains(&short) {
                        out.push(malformed(
                            body.node_span(*b),
                            format!("a second `{{:{short}}}` arm can never render"),
                        ));
                    }
                    seen.push(short);
                }
                let Some((_, declared, _)) = &cases else {
                    continue;
                };
                let missing: Vec<&str> = declared
                    .iter()
                    .map(|(c, _)| c.as_str())
                    .filter(|c| !seen.iter().any(|s| s == c))
                    .collect();
                if !missing.is_empty() {
                    out.push(Diagnostic {
                        code: "PW0305",
                        invariant: "a match must cover every value its scrutinee can take",
                        reason: "non_exhaustive_match",
                        detector: Detector::PatternMatrix,
                        severity: Severity::Error,
                        message: format!("`{d}` does not cover `{}`", missing.join("` or `")),
                        primary_span: span.clone(),
                        related: related(),
                        explanation: Some(
                            "A template match is exhaustive, as every match is (ADR-0011): \
                             an arm that renders nothing is written, not implied."
                                .to_string(),
                        ),
                        repairs: missing
                            .iter()
                            .map(|m| Repair {
                                description: format!("add a `{{:{m}}}` arm"),
                                replacement: None,
                            })
                            .collect(),
                    });
                }
            }
            // Any other directive was refused above.
            _ => {}
        }
    }
    // A marker no block holds.
    for n in body.walk_markup(&roots) {
        if let Node::Branch { marker, .. } = body.node(n)
            && !inside.contains(&n)
        {
            out.push(malformed(
                body.node_span(n),
                format!("`{}` stands outside any block", marker.trim()),
            ));
        }
    }
}

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
    let types = crate::infer::Types::of_decl(sigs, hir, id, body);
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
