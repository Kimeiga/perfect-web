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
    for (_, decl) in unit.hir.all_decls() {
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
    let Expr::Block { stmts } = body.expr(block) else {
        return None;
    };
    let mut it = stmts.iter().peekable();
    while let Some(s) = it.next() {
        if !matches!(body.expr(*s), Expr::Name(n) if n == "scope") {
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
