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
use crate::placement::{ALL_WORLDS, Demand, World, solve};
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
    let manifest = crate::resume::Manifest::build(&hirs, &sigs);
    // E8 step 2: what a capability's type argument may name.
    let declared_types = crate::capability::capability_argument_names(&hirs);
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
        resolution
            .entry(i)
            .or_default()
            .extend(unresolved_uses(&workspace, i, &u.hir));
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
                i,
                &declared_types,
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

        for id in body.walk() {
            let Expr::Call { callee, .. } = body.expr(id) else {
                continue;
            };
            let path = path_of(body, *callee);
            let Some((head, _)) = path.split_once('.') else {
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
    let manifest = crate::resume::Manifest::default();
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
        // This entry point builds a workspace from ONE unit, so the unit it
        // resolves in is 0. `check_units` is what real callers use.
        0,
        &crate::capability::capability_argument_names(&[&unit.hir]),
        unit,
    )
}

#[allow(clippy::too_many_arguments)]
fn check_unit_with(
    env: &Env,
    labels: &BTreeMap<crate::resolve::DefId, Label>,
    sigs: &Signatures,
    inference: &crate::effects::Inference<'_>,
    manifest: &crate::resume::Manifest,
    routes: &std::collections::BTreeSet<String>,
    graph: &crate::graph::Graph,
    // Which unit this is, so a call to a SIBLING resolves rather than being
    // matched by spelling — `docs/RISK_QUEUE.md` 34.
    at: usize,
    // Every type the whole program declares, for resolving capability
    // arguments. Whole-program, because `database.read<Stores>` in one file
    // names a type declared in another.
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
            at,
            id,
            decl,
            inherited.get(&id.0).copied(),
            &mut out,
        );
        privacy_flow(&unit.hir, sigs, id, decl, &mut out);
        privacy_sinks(&unit.hir, sigs, id, decl, &mut out);
        effect_rows(&unit.hir, sigs, inference, at, id, decl, &mut out);
        crate::capability::capability_arguments(&unit.hir, decl, types, &mut out);
        markup_rules(&unit.hir, decl, &mut out);
        let Some(body_id) = decl.body else { continue };
        let body = unit.hir.body(body_id);

        // Local type facts: a parameter's declared type, and any `let x: T`.
        let mut locals: BTreeMap<String, String> = BTreeMap::new();
        for p in &decl.params {
            if let Some(t) = &p.ty {
                locals.insert(p.name.clone(), t.clone());
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

    let report = exhaust::check_match(env.program(), &Type::Adt(adt_id), &lowered);
    match report.outcome() {
        // Nothing missing, and the analysis actually ran.
        crate::outcome::Outcome::Proven(_) => return,
        // No answer. The reason is already reported by whichever phase found
        // it — `PW0603` above, for the arity case — and saying it twice would
        // turn one defect into two. What must NOT happen is returning here as
        // though the match had been proved exhaustive, which is what
        // `is_exhaustive()` did.
        crate::outcome::Outcome::Blocked(_) => return,
        crate::outcome::Outcome::Violation(_) => {}
    }

    let missing: Vec<String> = report
        .missing
        .iter()
        .map(|w| exhaust::render_witness(env.program(), &Type::Adt(adt_id), w))
        .collect();

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

    let _ = unit;
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
fn label_of(decl: &Decl) -> Label {
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
fn declared_world(hir: &Hir, decl: &Decl) -> Option<World> {
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
        && effects
            .iter()
            .any(|e| !crate::effects::row_covers(&row, e) && !w.grants(e))
    {
        return;
    }

    let demand = Demand {
        effects: effects.clone(),
        label: label.clone(),
        declared: world,
    };
    let solution = solve(&demand);
    if solution.is_satisfiable() {
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
                && world.is_some_and(|w| !w.grants(&e.path))
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
fn declared_cache(hir: &Hir, decl: &Decl) -> Option<(String, crate::hir::Span)> {
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
fn effect_rows(
    hir: &Hir,
    sigs: &Signatures,
    inference: &crate::effects::Inference<'_>,
    at: usize,
    id: crate::hir::DeclId,
    decl: &Decl,
    out: &mut Vec<Diagnostic>,
) {
    use crate::effects::{Reuse, forbidden_in, forbidden_in_phase, phase_at};

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
        let Some(phase) = phase_at(body, &source.span) else {
            continue;
        };
        let Some(why) = forbidden_in_phase(&phase, &source.effect) else {
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
        let Some(why) = forbidden_in(decl, reuse, declared_world(hir, decl), &source.effect) else {
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
            if crate::effects::row_covers(row, &source.effect)
                || world.grants(&source.effect)
                || !said.insert(source.effect.clone())
            {
                continue;
            }
            let family = source.effect.split('.').next().unwrap_or(&source.effect);
            let elsewhere = World::worlds_for(family).unwrap_or(&[]);
            let elsewhere = elsewhere
                .iter()
                .map(|w| format!("`{w:?}`"))
                .collect::<Vec<_>>()
                .join(" or ");
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
