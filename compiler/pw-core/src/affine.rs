//! Affine resources — a value that must be consumed exactly once, in the scope
//! that acquired it.
//!
//! Two corpus fixtures, one invariant seen from opposite sides:
//!
//! - **R-011** opens a transaction and returns early on one path, so it is
//!   consumed zero times on that path.
//! - **R-012** binds a map handle with `use` and stores it in module state, so
//!   it is consumed — if at all — somewhere the acquiring scope cannot see.
//!
//! # What makes a value affine
//!
//! Its producer's effect row. `Database.begin()` declares
//! `!{ resource.acquire<DatabaseTransaction> }` and `Database.commit(tx)`
//! declares `!{ resource.release<DatabaseTransaction> }`. The compiler holds no
//! list of functions that open things and no list of functions that close them
//! — declaring a new resource is a change to a library file, and the diagnostic
//! names the release functions by reading them back out of the signature table.
//!
//! # Every path, counted
//!
//! Each path from the acquisition to where the value leaves its scope (the end
//! of the block that holds it, a `return`, a failing `?`) must release it
//! exactly once, or move it to the caller as the body's value. A release inside
//! a loop or a function value runs any number of times. A declaration whose
//! row says `resource.release<T>` owes the same to its `T` parameter. A `use`
//! binding is released by its block and must only not escape. The analysis is
//! over the statement tree: `pw` has no `goto`, no labelled `break` and no loop
//! that can carry a resource out. Until 2026-09-25 it asked only whether a
//! release came before each `return`, so a value never released, one live
//! across a `?`, and one released twice all passed.

use std::collections::BTreeSet;

use crate::codes;
use crate::diagnostics::{Detector, Diagnostic, Related, Repair, Severity};
use crate::hir::{BinOp, Body, Decl, Expr, ExprId, Hir, Span};
use crate::signatures::Signatures;

/// A value bound in this body whose producer declared it affine.
struct Acquired {
    name: String,
    /// The resource type, from `resource.acquire<T>`.
    ty: String,
    span: Span,
    /// `use x = ..` scopes the value to the block; a plain `let` does not.
    scoped: bool,
    /// A parameter the declaration's row promises to release.
    parameter: bool,
}

pub fn check(hir: &Hir, sigs: &Signatures, out: &mut Vec<Diagnostic>) {
    // Names declared outside any body: module-level `let mut`. A value stored
    // into one of these has left every scope in the file.
    let module_state: Vec<&str> = hir
        .all_decls()
        .filter(|(_, d)| d.kind == crate::hir::DeclKind::Let)
        .map(|(_, d)| d.name.as_str())
        .collect();

    for (id, decl) in hir.all_decls() {
        let Some(body_id) = decl.body else { continue };
        let body = hir.body(body_id);
        // A `todo` body is not written yet, so it promises nothing.
        if matches!(body.expr(body.root), Expr::Block { stmts }
            if matches!(stmts.as_slice(), [s] if matches!(body.expr(*s), Expr::Name(n) if n == "todo")))
        {
            continue;
        }
        let at = hir.decl_span(id);
        let types = crate::infer::Types::of_decl(sigs, hir, id, body);
        let mut owned = acquisitions(body, sigs, &types);
        owned.extend(released_parameters(hir, sigs, id, decl, body));
        for a in owned {
            let releases = releases_of(body, sigs, &types, &a);
            if let Some(escape) = escape_of(body, &a, &module_state) {
                report_escape(hir, decl, &a, escape, &at, out);
            } else if a.scoped {
                // A `use` block releases its value when it ends.
                continue;
            } else if let Some(fault) = fault_of(body, &a, &releases) {
                report_unconsumed(hir, sigs, decl, &a, fault, &at, out);
            }
        }
    }
}

/// **A parameter the declaration promises to release** (2026-09-25).
///
/// A declaration whose row says `resource.release<T>` takes a `T` in order to
/// end it, so its body must, exactly once on every path, as `Database.commit`
/// promises. Until 2026-09-25 only a value acquired in the same body was
/// followed, and a helper that took a transaction to commit it and did not was
/// never checked.
fn released_parameters(
    hir: &Hir,
    sigs: &Signatures,
    id: crate::hir::DeclId,
    decl: &Decl,
    body: &Body,
) -> Vec<Acquired> {
    let path = match hir.module_of(id) {
        Some(m) => format!("{m}.{}", decl.name),
        None => decl.name.clone(),
    };
    let Some(sig) = sigs.by_path(&path) else {
        return Vec::new();
    };
    let released: Vec<String> = sig
        .effects
        .iter()
        .filter_map(|e| type_argument(e, "resource.release"))
        .collect();
    decl.params
        .iter()
        .filter_map(|p| {
            let ty = p.ty.as_ref()?.written();
            released.contains(&ty).then(|| Acquired {
                name: p.name.clone(),
                ty,
                span: body.expr_span(body.root),
                scoped: false,
                parameter: true,
            })
        })
        .collect()
}

/// Bindings whose initialiser declares `resource.acquire<T>`.
fn acquisitions<'a>(
    body: &Body,
    sigs: &'a Signatures,
    types: &crate::infer::Types<'a>,
) -> Vec<Acquired> {
    let mut out = Vec::new();
    for id in body.walk() {
        let (name, init, scoped) = match body.expr(id) {
            Expr::Let {
                pat: Some(pat),
                init: Some(init),
                ..
            } => match body.pat(*pat) {
                crate::hir::Pattern::Bind { name, .. } => (name.clone(), *init, false),
                _ => continue,
            },
            // `use handle = Maps.create(..)` — the scoped form.
            Expr::Keyword {
                keyword,
                modifiers,
                args,
                ..
            } if keyword == "use" => match (modifiers.first(), args.first()) {
                (Some(name), Some(init)) => (name.clone(), *init, true),
                _ => continue,
            },
            _ => continue,
        };
        let Expr::Call { callee, .. } = body.expr(init) else {
            continue;
        };
        let Some(ty) = signature_of(sigs, types, body, *callee).and_then(|s| {
            s.effects
                .iter()
                .find_map(|e| type_argument(e, "resource.acquire"))
        }) else {
            continue;
        };
        out.push(Acquired {
            name,
            ty,
            span: body.expr_span(id),
            scoped,
            parameter: false,
        });
    }
    out
}

/// Calls in this body that release the value, by span.
fn releases_of<'a>(
    body: &Body,
    sigs: &'a Signatures,
    types: &crate::infer::Types<'a>,
    a: &Acquired,
) -> Vec<Span> {
    let mut out = Vec::new();
    for id in body.walk() {
        let Expr::Call { callee, args } = body.expr(id) else {
            continue;
        };
        let Some(sig) = signature_of(sigs, types, body, *callee) else {
            continue;
        };
        if !sig
            .effects
            .iter()
            .any(|e| type_argument(e, "resource.release").as_deref() == Some(a.ty.as_str()))
        {
            continue;
        }
        // Either `tx.commit()` — the value is the receiver — or
        // `Database.commit(tx)`, where it is an argument.
        let receiver = matches!(body.expr(*callee), Expr::Field { base, .. }
            if matches!(body.expr(*base), Expr::Name(n) if *n == a.name));
        let passed = args
            .iter()
            .any(|arg| matches!(body.expr(arg.value), Expr::Name(n) if *n == a.name));
        if receiver || passed {
            out.push(body.expr_span(id));
        }
    }
    out
}

/// How many times a path has released the value: 0, 1, or 2 for "more than
/// once".
type Count = u8;

fn plus(a: Count, b: Count) -> Count {
    (a + b).min(2)
}

/// What happens to the value along the paths through one construct.
#[derive(Debug, Clone)]
struct Flow {
    /// The release counts of the paths that continue past the construct.
    /// Empty when none does: every path left the body.
    through: BTreeSet<Count>,
    /// Paths that leave the body inside the construct: where, and having
    /// released how many times.
    exits: Vec<(Span, Count)>,
    /// A release inside a loop or a function value, which runs any number of
    /// times.
    repeated: Option<Span>,
}

impl Flow {
    /// Nothing happens.
    fn identity() -> Flow {
        Flow::releasing(0)
    }

    fn releasing(n: Count) -> Flow {
        Flow {
            through: BTreeSet::from([n]),
            exits: Vec::new(),
            repeated: None,
        }
    }

    /// Every path leaves the body here.
    fn exit(at: Span) -> Flow {
        Flow {
            through: BTreeSet::new(),
            exits: vec![(at, 0)],
            repeated: None,
        }
    }

    /// This construct, then `next`.
    fn then(self, next: Flow) -> Flow {
        let mut exits = self.exits;
        for c in &self.through {
            exits.extend(next.exits.iter().map(|(s, k)| (s.clone(), plus(*c, *k))));
        }
        let through = self
            .through
            .iter()
            .flat_map(|c| next.through.iter().map(move |k| plus(*c, *k)))
            .collect();
        Flow {
            through,
            exits,
            repeated: self.repeated.or(next.repeated),
        }
    }

    /// This construct or `other`.
    fn or(self, other: Flow) -> Flow {
        Flow {
            through: self.through.union(&other.through).copied().collect(),
            exits: [self.exits, other.exits].concat(),
            repeated: self.repeated.or(other.repeated),
        }
    }
}

/// **Every path through a scope, counted** (2026-09-25).
///
/// PW2005 says "exactly once", and until 2026-09-25 the check asked only
/// whether a release came before each `return`. A transaction never released,
/// one live across a failing `?`, and one committed twice all passed. Each
/// construct now answers how many times each of its paths releases, over the
/// statement tree: `pw` has no `goto`, no labelled `break` and no loop that can
/// carry a resource out, so the tree is enough.
struct Paths<'a> {
    body: &'a Body,
    name: &'a str,
    releases: &'a [Span],
    /// Where the body's value is produced. The value named there moves to the
    /// caller, which is its one consumption.
    tails: BTreeSet<ExprId>,
}

impl Paths<'_> {
    fn seq(&self, stmts: &[ExprId]) -> Flow {
        let mut flow = Flow::identity();
        let mut i = 0;
        while i < stmts.len() && !flow.through.is_empty() {
            let s = stmts[i];
            // `return e` is two statements: the value, then the exit. Nothing
            // after it on this path runs.
            if matches!(self.body.expr(s), Expr::Name(n) if n == "return") {
                let value = match stmts.get(i + 1) {
                    Some(e) if matches!(self.body.expr(*e), Expr::Name(n) if n == self.name) => {
                        Flow::releasing(1)
                    }
                    Some(e) => self.expr(*e),
                    None => Flow::identity(),
                };
                return flow.then(value).then(Flow::exit(self.body.expr_span(s)));
            }
            flow = flow.then(self.expr(s));
            i += 1;
        }
        flow
    }

    fn expr(&self, id: ExprId) -> Flow {
        let span = self.body.expr_span(id);
        if self.releases.contains(&span) {
            // Its arguments run first; then it releases.
            return self.children(id).then(Flow::releasing(1));
        }
        match self.body.expr(id) {
            Expr::Block { stmts } => self.seq(stmts),
            Expr::If { cond, then, els } => {
                let e = els.map_or_else(Flow::identity, |e| self.expr(e));
                self.expr(*cond).then(self.expr(*then).or(e))
            }
            Expr::Match { scrutinee, arms } => {
                let taken = arms
                    .iter()
                    .map(|a| self.expr(a.body))
                    .reduce(Flow::or)
                    .unwrap_or_else(Flow::identity);
                self.expr(*scrutinee).then(taken)
            }
            // `e?` leaves the body when `e` fails, with what it has released.
            Expr::Try { value } => self
                .expr(*value)
                .then(Flow::exit(span.clone()).or(Flow::identity())),
            // A loop's body, or a function value's, runs any number of times.
            Expr::For { .. } | Expr::Lambda { .. } => Flow {
                through: BTreeSet::from([0]),
                exits: Vec::new(),
                repeated: self
                    .body
                    .walk_from(id)
                    .into_iter()
                    .map(|e| self.body.expr_span(e))
                    .find(|s| self.releases.contains(s)),
            },
            Expr::Name(n) if n == self.name && self.tails.contains(&id) => Flow::releasing(1),
            _ => self.children(id),
        }
    }

    fn children(&self, id: ExprId) -> Flow {
        self.body
            .children(id)
            .into_iter()
            .fold(Flow::identity(), |f, c| f.then(self.expr(c)))
    }
}

/// The expressions whose value is the body's.
fn tails(body: &Body, id: ExprId, out: &mut BTreeSet<ExprId>) {
    match body.expr(id) {
        Expr::Block { stmts } => {
            if let Some(last) = stmts.last() {
                tails(body, *last, out);
            }
        }
        Expr::If {
            then, els: Some(e), ..
        } => {
            tails(body, *then, out);
            tails(body, *e, out);
        }
        Expr::Match { arms, .. } => {
            for a in arms {
                tails(body, a.body, out);
            }
        }
        _ => {
            out.insert(id);
        }
    }
}

/// What the paths from an acquisition do, and what is wrong with them, if
/// anything: the first problem, in order of what a reader meets.
enum Fault {
    /// A path leaves `at`, or ends the scope, not having released.
    Unreleased(Span),
    /// A path releases twice before `at`.
    Twice(Span),
    /// A release inside a loop or a function value.
    Repeated(Span),
}

/// The scope `a` is acquired in: the statements after its binding, in the
/// block that holds it, or the whole body for a parameter.
fn fault_of(body: &Body, a: &Acquired, releases: &[Span]) -> Option<Fault> {
    let mut tail = BTreeSet::new();
    tails(body, body.root, &mut tail);
    let paths = Paths {
        body,
        name: &a.name,
        releases,
        tails: tail,
    };
    let (flow, end) = if a.parameter {
        (paths.expr(body.root), body.expr_span(body.root))
    } else {
        let (stmts, at) = body.walk().into_iter().find_map(|b| match body.expr(b) {
            Expr::Block { stmts } => stmts
                .iter()
                .position(|s| body.expr_span(*s) == a.span)
                .map(|i| (stmts[i + 1..].to_vec(), body.expr_span(b))),
            _ => None,
        })?;
        (paths.seq(&stmts), at)
    };
    if let Some(r) = flow.repeated {
        return Some(Fault::Repeated(r));
    }
    for (at, count) in &flow.exits {
        match count {
            0 => return Some(Fault::Unreleased(at.clone())),
            2 => return Some(Fault::Twice(at.clone())),
            _ => {}
        }
    }
    if flow.through.contains(&0) {
        return Some(Fault::Unreleased(end));
    }
    flow.through.contains(&2).then_some(Fault::Twice(end))
}

/// An assignment that puts the value somewhere the acquiring scope cannot see.
fn escape_of(body: &Body, a: &Acquired, module_state: &[&str]) -> Option<Span> {
    body.walk().into_iter().find_map(|id| {
        let Expr::Binary {
            op: BinOp::Assign,
            lhs,
            rhs,
        } = body.expr(id)
        else {
            return None;
        };
        let Expr::Name(target) = body.expr(*lhs) else {
            return None;
        };
        if !module_state.contains(&target.as_str()) {
            return None;
        }
        // The value need not be assigned bare: `Some(handle)` still stores it.
        body.walk_from(*rhs)
            .into_iter()
            .any(|e| matches!(body.expr(e), Expr::Name(n) if *n == a.name))
            .then(|| body.expr_span(id))
    })
}

fn report_escape(
    hir: &Hir,
    decl: &Decl,
    a: &Acquired,
    escape: Span,
    at: &Span,
    out: &mut Vec<Diagnostic>,
) {
    let scope = if a.scoped {
        "the `use` block that acquired it"
    } else {
        "the scope that acquired it"
    };
    out.push(Diagnostic {
        code: codes::AFFINE_NOT_CONSUMED_ONCE.id,
        invariant: codes::AFFINE_NOT_CONSUMED_ONCE.invariant,
        reason: "affine_value_escapes_its_scope",
        detector: Detector::PatternMatrix,
        severity: Severity::Error,
        message: format!(
            "affine resource `{}: {}` escapes the scope that acquired it",
            a.name, a.ty
        ),
        primary_span: escape,
        related: vec![
            Related {
                span: a.span.clone(),
                label: format!("`{}` is acquired here", a.name),
            },
            Related {
                span: at.clone(),
                label: format!("`{}` owns it", decl.name),
            },
        ],
        explanation: Some(format!(
            "An affine value may not outlive its acquiring scope. Storing `{}` \
             outside {scope} means the release that {scope} performs either does \
             not happen or happens to a value something else still holds — and \
             which of those it is cannot be determined by reading this function.",
            a.name
        )),
        repairs: vec![Repair {
            description: "keep the value inside the scope and store a value derived \
                          from it, or move the whole lifetime up to where the state lives"
                .to_string(),
            replacement: None,
        }],
    });
    let _ = hir;
}

#[allow(clippy::too_many_arguments)]
fn report_unconsumed(
    hir: &Hir,
    sigs: &Signatures,
    decl: &Decl,
    a: &Acquired,
    fault: Fault,
    at: &Span,
    out: &mut Vec<Diagnostic>,
) {
    // Named by reading the signature table, so declaring a third way to end a
    // transaction updates the diagnostic.
    let ways = release_functions(sigs, &a.ty);
    let ways = if ways.is_empty() {
        "a function that releases it".to_string()
    } else {
        ways.iter()
            .map(|w| format!("`{w}`"))
            .collect::<Vec<_>>()
            .join(" or ")
    };
    let (reason, message, primary, repair) = match fault {
        Fault::Unreleased(span) => (
            "affine_value_not_consumed_on_every_path",
            format!(
                "affine resource `{}: {}` is not consumed on every path",
                a.name, a.ty
            ),
            span,
            format!("end `{}` on this path, with {ways}", a.name),
        ),
        Fault::Twice(span) => (
            "affine_value_consumed_twice",
            format!(
                "affine resource `{}: {}` is released twice on one path",
                a.name, a.ty
            ),
            span,
            format!("end `{}` once on each path", a.name),
        ),
        Fault::Repeated(span) => (
            "affine_value_released_repeatedly",
            format!(
                "affine resource `{}: {}` is released inside a loop or a function value, \
                 which may run any number of times",
                a.name, a.ty
            ),
            span,
            format!("end `{}` once, outside the loop", a.name),
        ),
    };
    let acquired = if a.parameter {
        format!(
            "`{}` is a parameter `{}` promises to release",
            a.name, decl.name
        )
    } else {
        format!("`{}` is acquired here", a.name)
    };
    out.push(Diagnostic {
        code: codes::AFFINE_NOT_CONSUMED_ONCE.id,
        invariant: codes::AFFINE_NOT_CONSUMED_ONCE.invariant,
        reason,
        detector: Detector::PatternMatrix,
        severity: Severity::Error,
        message,
        primary_span: primary,
        related: vec![
            Related {
                span: a.span.clone(),
                label: acquired,
            },
            Related {
                span: at.clone(),
                label: format!("`{}` must end it exactly once on every path", decl.name),
            },
        ],
        explanation: Some(format!(
            "A {} must end exactly once, with {ways}. Not ending it holds whatever \
             it locked until something else times out, and ending it twice is a \
             different bug that this same rule is what makes visible.",
            a.ty
        )),
        repairs: vec![Repair {
            description: repair,
            replacement: None,
        }],
    });
    let _ = hir;
}

/// Every function the program declares as releasing this resource type.
fn release_functions(sigs: &Signatures, ty: &str) -> Vec<String> {
    let mut out: Vec<String> = sigs
        .iter()
        .filter(|(_, s)| {
            s.effects
                .iter()
                .any(|e| type_argument(e, "resource.release").as_deref() == Some(ty))
        })
        .map(|(path, _)| path.rsplit('.').next().unwrap_or(path).to_string())
        .collect();
    out.sort();
    out.dedup();
    out
}

/// `resource.acquire<MapHandle>` with prefix `resource.acquire` gives
/// `MapHandle`.
fn type_argument(effect: &str, prefix: &str) -> Option<String> {
    let rest = effect.strip_prefix(prefix)?;
    let inner = rest.strip_prefix('<')?.strip_suffix('>')?;
    (!inner.is_empty()).then(|| inner.to_string())
}

/// The signature a call resolves to: a module path, or a member of a receiver
/// whose type is known. Nothing else.
fn signature_of<'a>(
    sigs: &'a Signatures,
    types: &crate::infer::Types<'a>,
    body: &Body,
    callee: ExprId,
) -> Option<&'a crate::signatures::Signature> {
    sigs.by_path(&path_of(body, callee))
        .or_else(|| types.callee(body, callee))
}

use crate::infer::path_of;
