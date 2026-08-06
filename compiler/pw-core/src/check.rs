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

use std::collections::BTreeMap;

use crate::diagnostics::{Detector, Diagnostic, Related, Repair, Severity};
use crate::exhaust::{self, Arm, Pattern as EPat};
use crate::hir::{self, Body, Decl, DeclKind, Expr, ExprId, Hir, Pattern as HPat};
use crate::placement::{ALL_WORLDS, Demand, World, solve};
use crate::privacy::{Label, Restriction};
use crate::scope::{HandleKind, Op, ScopeGraph, ScopeKind, ScopeViolation};
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
    units
        .iter()
        .map(|u| (u.path.clone(), check_unit(&env, u)))
        .collect()
}

pub fn check_unit(env: &Env, unit: &Unit) -> Vec<Diagnostic> {
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

    for (id, decl) in unit.hir.all_decls() {
        privacy_and_placement(
            &unit.hir,
            &unit.src,
            decl,
            inherited.get(&id.0).copied(),
            &mut out,
        );
        privacy_flow(&unit.hir, decl, &mut out);
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

    let report = exhaust::check_match(env.program(), &Type::Adt(adt_id), &lowered);
    if report.is_exhaustive() {
        return;
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

    let mut declared: Vec<(usize, usize, crate::hir::Span)> = Vec::new();

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
            } if keyword == "observe" => {
                let Some(block) = block else { continue };
                let Some((scope_name, at)) = block_scope(body, *block) else {
                    continue;
                };
                let Some(kind) = named_scope(&scope_name) else {
                    continue;
                };
                let label = match modifiers.first() {
                    Some(m) => format!("observe.{m}"),
                    None => "observe".to_string(),
                };
                let h = g.spawn(&label, HandleKind::Subscription, here, body.expr_span(id));
                declared.push((h, by_kind(kind), at));
            }

            _ => {}
        }
    }

    let mut found = g.check();
    for (h, scope, span) in declared {
        found.extend(g.check_declared_scope(h, scope, span));
    }
    out.extend(found.into_iter().map(to_diagnostic));
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

fn to_diagnostic(v: ScopeViolation) -> Diagnostic {
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
            _ => "A subscription keeps pushing updates for as long as its declared scope \
                  lives. Declaring a longer scope than the owner means pushing into \
                  something that no longer exists."
                .to_string(),
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
    match decl.visibility.as_deref() {
        Some("session") => Label::session(&decl.name),
        Some("private") => Label::user(&decl.name),
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
fn privacy_and_placement(
    hir: &Hir,
    src: &str,
    decl: &Decl,
    inherited: Option<World>,
    out: &mut Vec<Diagnostic>,
) {
    let label = label_of(decl);

    // 1. A non-public value in a shared cache. Charter §7.8's canonical case.
    if let Some(cache) = decl
        .policy("cache")
        .filter(|c| c.value.trim() == "shared" && !label.safe_in_shared_cache())
    {
        {
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
                    "A shared cache is read by every user. {label} is not, so one \
                     user's value would be served to another. The cause chain is \
                     the whole point: `{}` is {label}, a shared cache is public, \
                     and {label} does not flow into public.",
                    decl.name
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

    // 2. Placement. The demand is the declared effect row plus the label; the
    // solver answers which worlds can satisfy it.
    let effects: Vec<String> = decl
        .declared_effects
        .as_deref()
        .unwrap_or_default()
        .iter()
        .map(|e| e.path.clone())
        .collect();
    if effects.is_empty() && label.is_public() {
        return;
    }

    let world = declared_world(hir, decl).or(inherited);
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
    let written: Vec<String> = decl
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
        .find(|e| world.is_some_and(|w| !w.grants(&e.path)))
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

/// Standard-library accessors that introduce a privacy label.
///
/// **A stand-in for signatures that do not exist yet.** With a real library the
/// checker would read `current_organization()`'s declared return label instead
/// of consulting a table. The table is here so the rule can be built and tested
/// now; it is the thing to delete when the library lands, not the rule.
fn introduced_restriction(path: &str) -> Option<Restriction> {
    Some(match path {
        // The parameters are the charter §7.8 type names, because that is what
        // a developer sees and what the corpus declares it expects. With a real
        // library these come from the accessor's return label.
        "current_session" | "session.current" => Restriction::Session("SessionId".into()),
        "current_user" | "user.current" => Restriction::User("UserId".into()),
        "current_organization" | "organization.current" => {
            Restriction::Organization("OrganizationId".into())
        }
        "device.id" => Restriction::Device,
        p if p.starts_with("secrets.") => {
            Restriction::Secret(capitalize(p.trim_start_matches("secrets.")))
        }
        _ => return None,
    })
}

fn capitalize(s: &str) -> String {
    let mut c = s.chars();
    match c.next() {
        Some(f) => f.to_uppercase().collect::<String>() + c.as_str(),
        None => String::new(),
    }
}

/// Every restriction a body picks up by calling a label-introducing accessor.
fn body_label(body: &Body) -> Label {
    let mut label = Label::public();
    for id in body.walk() {
        let Expr::Call { callee, .. } = body.expr(id) else {
            continue;
        };
        if let Some(r) = introduced_restriction(&path_of(body, *callee)) {
            label = label.join(&Label::of(r));
        }
    }
    label
}

/// Names bound to a label-introducing call, so a later use can be traced back.
fn labelled_bindings(body: &Body) -> BTreeMap<String, (Restriction, crate::hir::Span)> {
    let mut out = BTreeMap::new();
    for id in body.walk() {
        let Expr::Let { pat, init, ty } = body.expr(id) else {
            continue;
        };
        let (Some(pat), Some(init)) = (pat, init) else {
            continue;
        };
        let HPat::Bind { name, .. } = body.pat(*pat) else {
            continue;
        };
        // Either the initialiser calls an accessor, or the binding is annotated
        // `Secret<..>` directly.
        // A written annotation is the most precise source: `let key:
        // Secret<Payments>` names the capability exactly, where the accessor's
        // name only implies it.
        let annotated = ty.and_then(|t| {
            let t = body.types.get(t.index())?;
            (t.path == "Secret")
                .then(|| t.args.first().and_then(|a| body.types.get(a.index())))
                .flatten()
                .map(|arg| Restriction::Secret(arg.path.clone()))
        });
        let restriction = annotated.or_else(|| match body.expr(*init) {
            Expr::Call { callee, .. } => introduced_restriction(&path_of(body, *callee)),
            _ => None,
        });
        if let Some(r) = restriction {
            out.insert(name.clone(), (r, body.expr_span(id)));
        }
    }
    out
}

/// E5 rules that need the body's label, not only the declaration header.
fn privacy_flow(hir: &Hir, decl: &Decl, out: &mut Vec<Diagnostic>) {
    let Some(body_id) = decl.body else { return };
    let body = hir.body(body_id);
    let label = body_label(body);
    if label.is_public() {
        return;
    }

    // 1. A secret reaching markup. Markup renders in the browser, and a secret
    //    never leaves the origin (charter §7.8, corpus R-003).
    let bound = labelled_bindings(body);
    for id in body.walk() {
        let Expr::Template { parts, .. } = body.expr(id) else {
            continue;
        };
        for part in parts {
            let name = match body.expr(*part) {
                Expr::Name(n) => n.clone(),
                _ => continue,
            };
            let Some((Restriction::Secret(cap), origin)) = bound.get(&name) else {
                continue;
            };
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
