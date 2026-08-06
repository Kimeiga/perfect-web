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
//! # What this is not
//!
//! Not a control-flow analysis. A real one builds a graph and asks whether
//! every path from the acquisition reaches a release; this asks whether a
//! `return` sits textually between the acquisition and the first release. That
//! is exactly R-011's shape and it is honest about the cases it misses: a
//! release inside one branch of an `if` and a return in the other would pass
//! here. E9C proper is the CFG; this is the corpus's specification met with the
//! structure that exists.

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
        let at = hir.decl_span(id);
        let types = crate::infer::Types::of_body(sigs, decl, body, hir.module_of(id));
        for a in acquisitions(body, sigs, &types) {
            let releases = releases_of(body, sigs, &types, &a);
            if let Some(escape) = escape_of(body, &a, &module_state) {
                report_escape(hir, decl, &a, escape, &at, out);
            } else if let Some(early) = unreleased_path(body, &a, &releases) {
                report_unconsumed(hir, sigs, decl, &a, early, &at, out);
            }
        }
    }
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

/// What happens to the resource along the paths through one construct.
#[derive(Debug, Default, Clone)]
struct Flow {
    /// EVERY path through this construct releases before leaving it.
    released: bool,
    /// Paths that leave the enclosing body without having released.
    escapes: Vec<Span>,
}

/// Does every path from the acquisition reach a release?
///
/// This replaced a textual scan — "is there a `return` between the acquisition
/// and the first release" — which met R-011 and missed the shape where the
/// release is written first:
///
/// ```text
/// if empty { rollback(tx) } else { return Ok(()) }
/// ```
///
/// Here the release precedes the return in the text and every path is *not*
/// covered. The witness for that gap lived in
/// `examples/generality/affine_not_consumed_once/slips-through.pw`; this is
/// what promoted it.
///
/// The analysis is over the statement tree rather than a basic-block graph,
/// which is enough because `pw` has no `goto`, no labelled break and no loop
/// that can carry a resource out. Each construct answers two questions: do all
/// my paths release, and which of my paths leave the body without releasing.
fn unreleased_path(body: &Body, a: &Acquired, releases: &[Span]) -> Option<Span> {
    let flow = flow_of(body, body.root, a, releases, &mut false);
    flow.escapes.first().cloned()
}

fn flow_of(body: &Body, id: ExprId, a: &Acquired, releases: &[Span], live: &mut bool) -> Flow {
    let span = body.expr_span(id);

    // The acquisition itself: the resource is live from here on.
    if span == a.span {
        *live = true;
        return Flow::default();
    }

    // A release. Everything after it on this path is covered.
    if releases.contains(&span) {
        return Flow {
            released: true,
            escapes: Vec::new(),
        };
    }

    match body.expr(id) {
        Expr::Block { stmts } => {
            let mut out = Flow::default();
            for s in stmts {
                if out.released {
                    // Already released on this path; nothing later can leak it.
                    break;
                }
                let f = flow_of(body, *s, a, releases, live);
                out.released |= f.released;
                if !out.released {
                    out.escapes.extend(f.escapes);
                }
            }
            out
        }
        Expr::If { cond, then, els } => {
            let _ = flow_of(body, *cond, a, releases, live);
            let t = flow_of(body, *then, a, releases, live);
            let e = els.map(|e| flow_of(body, e, a, releases, live));
            let mut escapes = t.escapes;
            let mut released = t.released;
            match e {
                // Both branches must release for the whole `if` to.
                Some(e) => {
                    released &= e.released;
                    escapes.extend(e.escapes);
                }
                // No else: the fall-through path does not release here.
                None => released = false,
            }
            Flow { released, escapes }
        }
        Expr::Match { arms, .. } => {
            let mut released = !arms.is_empty();
            let mut escapes = Vec::new();
            for arm in arms {
                let f = flow_of(body, arm.body, a, releases, live);
                released &= f.released;
                escapes.extend(f.escapes);
            }
            Flow { released, escapes }
        }
        // A `return` while the resource is live and unreleased on this path.
        Expr::Name(n) if n == "return" && *live => Flow {
            released: false,
            escapes: vec![span],
        },
        _ => Flow::default(),
    }
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
    early: Span,
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
    out.push(Diagnostic {
        code: codes::AFFINE_NOT_CONSUMED_ONCE.id,
        invariant: codes::AFFINE_NOT_CONSUMED_ONCE.invariant,
        reason: "affine_value_not_consumed_on_every_path",
        detector: Detector::PatternMatrix,
        severity: Severity::Error,
        message: format!(
            "affine resource `{}: {}` is not consumed on every path",
            a.name, a.ty
        ),
        primary_span: early,
        related: vec![
            Related {
                span: a.span.clone(),
                label: format!("`{}` is acquired here", a.name),
            },
            Related {
                span: at.clone(),
                label: format!("`{}` must end it before every exit", decl.name),
            },
        ],
        explanation: Some(format!(
            "This path leaves `{}` open. A transaction must end exactly once, in \
             {ways} — not ending it holds whatever it locked until something else \
             times out, and ending it twice is a different bug that this same rule \
             is what makes visible.",
            a.name
        )),
        repairs: vec![Repair {
            description: format!("end `{}` on this path before returning", a.name),
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
