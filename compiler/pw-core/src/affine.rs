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
//!
//! # Held, or refused
//!
//! What is followed is a name: `let tx = Database.begin()`, `use handle =
//! ..`, or `let h = Maps.create(..)?`. An acquisition's value is held by
//! such a binding, ended where it is made (`Database.begin().commit()`),
//! given to the caller as the declaration's value, or held by a resource's
//! `acquire` clause, which its `release` clause ends. Anywhere else nothing
//! follows it, and it is refused: bound to `_`, dropped by a statement,
//! given to a call that does not release it, kept in another value, or
//! returned by a function value (ADR-0250). Until ADR-0250 only the first
//! binding was followed, and an acquisition anywhere else passed unread.
//!
//! # A resource's clauses
//!
//! A `resource`'s `acquire { .. }` and `release(h) { .. }` are terms, which
//! no statement reaches, or a statement's clauses in a body. Both are walked:
//! what `acquire` makes, the resource holds; what `release` is given, it ends
//! exactly once on every path, where `acquire` acquires it (ADR-0251). Until
//! ADR-0251 a declaration's clauses were walked by nothing, and a `release`
//! that never ended its handle passed in either form.
//!
//! # Bindings, not names
//!
//! A use of the value is a name whose binding is the acquisition's
//! (`crate::lexical`, ADR-0080). Until 2026-09-26 it was any name spelled
//! like it, so an inner `tx` ended in each branch ended an outer `tx` never
//! ended, and a program ending each once was refused as ending the outer
//! twice. A local bound to a declaration, `let end = Database.rollback`, is
//! that declaration where it is called; a value given to any other function
//! value may be released there, which nothing can count, and is refused.

use std::collections::{BTreeMap, BTreeSet};

use crate::codes;
use crate::diagnostics::{Detector, Diagnostic, Related, Repair, Severity};
use crate::hir::{BinOp, Body, Decl, Expr, ExprId, Hir, Span};
use crate::infer::Types;
use crate::lexical::Binder;
use crate::signatures::Signatures;

/// A value bound in this body whose producer declared it affine.
struct Acquired {
    /// The binding every use of it means (ADR-0080).
    binder: Binder,
    /// Its name, for the diagnostic.
    name: String,
    /// The resource type, from `resource.acquire<T>`.
    ty: String,
    span: Span,
    /// `use x = ..` scopes the value to the block; a plain `let` does not.
    scoped: bool,
    /// A parameter the declaration's row promises to release.
    parameter: bool,
    /// What a `release` clause is given: its block, through which it must
    /// end it (ADR-0251).
    clause: Option<ExprId>,
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
        let gives = gives_its_value(hir, sigs, id, decl);
        let reached = reach(body, decl);
        let resource = resource_roots(body, decl, &reached);
        let (mut owned, unheld) = acquisitions(body, sigs, &types, gives, &reached, &resource);
        for u in unheld {
            report_unheld(sigs, decl, u, &at, out);
        }
        owned.extend(released_parameters(hir, sigs, id, decl, body));
        owned.extend(released_clauses(body, decl, sigs, &types, &reached));
        for a in owned {
            let (releases, given) = releases_of(body, sigs, &types, &a, &reached);
            if let Some(escape) = escape_of(body, &types, &a, &module_state, &reached) {
                report_escape(hir, decl, &a, escape, &at, out);
            } else if let Some((span, to)) = given.into_iter().next() {
                report_unconsumed(hir, sigs, decl, &a, Fault::Given(span, to), &at, out);
            } else if a.scoped {
                // A `use` block releases its value when it ends.
                continue;
            } else if let Some(fault) =
                fault_of(body, &types, &a, &releases, gives, &reached, &resource)
            {
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
        .enumerate()
        .filter_map(|(i, p)| {
            let ty = p.ty.as_ref()?.written();
            released.contains(&ty).then(|| Acquired {
                binder: Binder::Param(i),
                name: p.name.clone(),
                ty,
                span: body.expr_span(body.root),
                scoped: false,
                parameter: true,
                clause: None,
            })
        })
        .collect()
}

/// **What a `release` clause is given, it ends** (ADR-0251): exactly once on
/// every path, where its resource's `acquire` clause acquires it. A
/// declaration's clauses are term roots, `release(h) { .. }` binding `h`; a
/// statement's are `release(h)` and the block after it. Until ADR-0251
/// neither was followed.
fn released_clauses(
    body: &Body,
    decl: &Decl,
    sigs: &Signatures,
    types: &Types<'_>,
    reached: &[ExprId],
) -> Vec<Acquired> {
    use crate::hir::ExecutionContext as Cx;
    let mut out = Vec::new();
    let roots: Vec<&crate::hir::TermRoot> = decl.term_roots().map(|(_, r)| r).collect();
    let made = roots
        .iter()
        .filter(|r| r.context == Cx::Acquire)
        .find_map(|r| acquires(body, sigs, types, r.root));
    for (i, r) in roots.iter().enumerate() {
        if r.context != Cx::Release {
            continue;
        }
        let (Some(ty), Some((name, _))) = (&made, r.binders.first()) else {
            continue;
        };
        out.push(Acquired {
            binder: Binder::Term(i, 0),
            name: name.clone(),
            ty: ty.clone(),
            span: body.expr_span(r.root),
            scoped: false,
            parameter: false,
            clause: Some(r.root),
        });
    }
    for (clause, acquire) in types.lexical().released() {
        let Some(ty) = acquires(body, sigs, types, acquire) else {
            continue;
        };
        let Some(block) = reached.iter().find_map(|b| match body.expr(*b) {
            Expr::Block { stmts } => stmts.windows(2).find(|w| w[0] == clause).map(|w| w[1]),
            _ => None,
        }) else {
            continue;
        };
        let Expr::Call { args, .. } = body.expr(clause) else {
            continue;
        };
        let Some(name) = args.first().map(|a| path_of(body, a.value)) else {
            continue;
        };
        out.push(Acquired {
            binder: Binder::Clause(clause, 0),
            name,
            ty,
            span: body.expr_span(block),
            scoped: false,
            parameter: false,
            clause: Some(block),
        });
    }
    out
}

/// What an `acquire` clause acquires: `T` of a call under it whose row
/// declares `resource.acquire<T>`.
fn acquires(body: &Body, sigs: &Signatures, types: &Types<'_>, root: ExprId) -> Option<String> {
    body.walk_from(root).into_iter().find_map(|e| {
        let Expr::Call { callee, .. } = body.expr(e) else {
            return None;
        };
        signature_of(sigs, types, body, *callee)?
            .effects
            .iter()
            .find_map(|x| type_argument(x, "resource.acquire"))
    })
}

/// **Every expression the declaration runs**: its body's, and each of its
/// term roots', which no statement reaches (ADR-0251), a resource's
/// `acquire` and `release` clauses among them.
fn reach(body: &Body, decl: &Decl) -> Vec<ExprId> {
    std::iter::once(body.root)
        .chain(decl.term_roots().map(|(_, r)| r.root))
        .flat_map(|r| body.walk_from(r))
        .collect()
}

/// What holds a resource's value until its `release` clause ends it: each
/// `acquire` clause, a `resource` declaration's term root or the block after
/// `acquire` in a body (ADR-0251).
fn resource_roots(body: &Body, decl: &Decl, reached: &[ExprId]) -> BTreeSet<ExprId> {
    let mut out: BTreeSet<ExprId> = decl
        .term_roots()
        .filter(|(_, r)| r.context == crate::hir::ExecutionContext::Acquire)
        .map(|(_, r)| r.root)
        .collect();
    for b in reached {
        let Expr::Block { stmts } = body.expr(*b) else {
            continue;
        };
        for w in stmts.windows(2) {
            if matches!(body.expr(w[0]), Expr::Name(n) if n == "acquire")
                && matches!(body.expr(w[1]), Expr::Block { .. })
            {
                out.insert(w[1]);
            }
        }
    }
    out
}

/// **Every call whose row declares `resource.acquire<T>`, and what holds
/// its value** (ADR-0250): the bindings that name one, each once, to be
/// followed; and the acquisitions nothing holds.
fn acquisitions<'a>(
    body: &Body,
    sigs: &'a Signatures,
    types: &Types<'a>,
    gives: bool,
    reached: &[ExprId],
    resource: &BTreeSet<ExprId>,
) -> (Vec<Acquired>, Vec<Unheld>) {
    let parents: BTreeMap<ExprId, ExprId> = reached
        .iter()
        .flat_map(|p| body.children(*p).into_iter().map(move |c| (c, *p)))
        .collect();
    let (mut owned, mut unheld) = (Vec::<Acquired>::new(), Vec::new());
    for &id in reached {
        let Expr::Call { callee, .. } = body.expr(id) else {
            continue;
        };
        let Some(sig) = signature_of(sigs, types, body, *callee) else {
            continue;
        };
        let Some(ty) = sig
            .effects
            .iter()
            .find_map(|e| type_argument(e, "resource.acquire"))
        else {
            continue;
        };
        let walk = Holding {
            body,
            sigs,
            types,
            parents: &parents,
            ty: &ty,
            gives,
            resource,
        };
        match walk.holder(id) {
            Holder::Binding {
                binder,
                name,
                at,
                scoped,
            } => {
                // `let tx = if c { Database.begin() } else { .. }`: two
                // acquisitions, one binding to follow.
                if !owned.iter().any(|a| a.binder == binder) {
                    owned.push(Acquired {
                        binder,
                        name,
                        ty,
                        span: body.expr_span(at),
                        scoped,
                        parameter: false,
                        clause: None,
                    });
                }
            }
            Holder::Kept => {}
            Holder::Nothing(dropped) => unheld.push(Unheld {
                ty,
                span: body.expr_span(id),
                dropped,
                carried: carried(sig),
            }),
        }
    }
    (owned, unheld)
}

/// **Does the declaration give its body's value to a caller** (ADR-0250)?
/// A function, a query, a command or a task does, unless it declares `()`:
/// such a body ends in a statement, whose value nothing reads (`values`,
/// `returns`). A value given to the caller is the caller's to end, since
/// the row that acquired it, which PW0400 has the declaration state, is now
/// the declaration's own. Until ADR-0250 a body declaring `()` gave its
/// last value to the caller too, and a transaction it ended in passed.
fn gives_its_value(hir: &Hir, sigs: &Signatures, id: crate::hir::DeclId, decl: &Decl) -> bool {
    use crate::hir::DeclKind;
    if !matches!(
        decl.kind,
        DeclKind::Fn | DeclKind::Query | DeclKind::Command | DeclKind::Task
    ) {
        return false;
    }
    let path = match hir.module_of(id) {
        Some(m) => format!("{m}.{}", decl.name),
        None => decl.name.clone(),
    };
    sigs.by_path(&path)
        .and_then(|s| s.returns.as_ref())
        .and_then(|r| r.resolved())
        .is_some_and(|r| r.as_primitive() != Some(crate::resolved::Primitive::Unit))
}

/// Does the call answer a `Result` or an `Option` that carries what it
/// acquires, `Maps.create(..)`, so that a name holds it with `?`?
fn carried(sig: &crate::signatures::Signature) -> bool {
    use crate::resolved::Builtin;
    sig.returns
        .as_ref()
        .and_then(|r| r.resolved())
        .map(crate::values::Ty::of)
        .is_some_and(|t| {
            matches!(
                t,
                crate::values::Ty::Builtin(Builtin::Result | Builtin::Option, _)
            )
        })
}

/// What holds an acquired value (ADR-0250).
enum Holder {
    /// `let x = ..` or `use x = ..`: a binding, which `fault_of` follows.
    Binding {
        binder: Binder,
        name: String,
        /// The binding statement.
        at: ExprId,
        scoped: bool,
    },
    /// Ended where it is made, given to the caller, or held by a resource's
    /// `acquire` clause.
    Kept,
    /// Nothing does.
    Nothing(Dropped),
}

/// Where an acquisition nothing holds goes, and the span that says so.
enum Dropped {
    /// `let _ = ..`, ruling 0099-a: Rust's `let _ = mutex.lock()`, which
    /// drops at once, refused rather than guessed at.
    Discarded(Span),
    /// A statement's value, which nothing reads: the statement.
    Statement(Span),
    /// A clause's value, which nothing reads, as a `release` clause's or a
    /// key's (ADR-0251): the clause.
    Clause(Span),
    /// Given to a call that does not release it: the call, and its callee.
    Taken(Span, String),
    /// Matched where it is made: what the arms bind is not followed.
    Matched(Span),
    /// A function value's result, which nothing follows.
    Returned(Span),
    /// Kept in another value, or read for a field: that value.
    Contained(Span),
}

/// An acquisition nothing holds.
struct Unheld {
    ty: String,
    /// The acquiring call.
    span: Span,
    dropped: Dropped,
    carried: bool,
}

/// The walk from an acquiring call up to what holds its value.
struct Holding<'a, 'b> {
    body: &'b Body,
    sigs: &'a Signatures,
    types: &'b Types<'a>,
    parents: &'b BTreeMap<ExprId, ExprId>,
    /// What the call acquires.
    ty: &'b str,
    gives: bool,
    /// What holds a resource's value (`resource_roots`).
    resource: &'b BTreeSet<ExprId>,
}

impl Holding<'_, '_> {
    /// The value's holder: up through what passes a value on, a block's last
    /// statement, a branch, an arm, a `?`, `Ok(..)` and `Some(..)`, to what
    /// keeps or drops it.
    fn holder(&self, call: ExprId) -> Holder {
        let body = self.body;
        let mut at = call;
        loop {
            let Some(&p) = self.parents.get(&at) else {
                // A `resource`'s `acquire` clause: the resource holds its
                // value, and its `release` clause ends it (ADR-0251).
                // Another clause's value nothing reads.
                if self.resource.contains(&at) {
                    return Holder::Kept;
                }
                if at != body.root {
                    return Holder::Nothing(Dropped::Clause(body.expr_span(at)));
                }
                // The body's value.
                return self.given(at);
            };
            match body.expr(p) {
                Expr::Block { stmts } => {
                    let i = stmts.iter().position(|s| *s == at).unwrap_or(0);
                    let before = i.checked_sub(1).map(|j| body.expr(stmts[j]));
                    // `acquire { .. }`: the resource holds the value, and its
                    // `release` clause ends it.
                    if matches!(before, Some(Expr::Name(n)) if n == "acquire")
                        && matches!(body.expr(at), Expr::Block { .. })
                    {
                        return Holder::Kept;
                    }
                    // `release(h) { .. }`, a statement's clause: what its
                    // block ends in nothing reads (ADR-0251), as a
                    // declaration's `release` clause's.
                    if matches!(before, Some(Expr::Call { callee, .. })
                        if matches!(body.expr(*callee), Expr::Name(n) if n == "release"))
                        && matches!(body.expr(at), Expr::Block { .. })
                    {
                        return Holder::Nothing(Dropped::Clause(body.expr_span(at)));
                    }
                    // `return e`: the value of the function the `return` is
                    // in.
                    if matches!(before, Some(Expr::Name(n)) if n == "return") {
                        return match self.enclosing_lambda(p) {
                            Some(l) => Holder::Nothing(Dropped::Returned(body.expr_span(l))),
                            None => self.given(at),
                        };
                    }
                    if i + 1 < stmts.len() {
                        return Holder::Nothing(Dropped::Statement(body.expr_span(at)));
                    }
                }
                Expr::If { cond, .. } if *cond != at => {}
                Expr::Match { scrutinee, .. } if *scrutinee == at => {
                    return Holder::Nothing(Dropped::Matched(body.expr_span(p)));
                }
                Expr::Match { .. } | Expr::Try { .. } => {}
                Expr::Call { .. } if wraps(body, self.types, p) == Some(at) => {}
                Expr::Let { pat, .. } => {
                    return match pat.map(|q| (q, body.pat(q))) {
                        Some((q, crate::hir::Pattern::Bind { name, .. })) => Holder::Binding {
                            binder: Binder::Pattern(q),
                            name: name.clone(),
                            at: p,
                            scoped: false,
                        },
                        Some((_, crate::hir::Pattern::Wild)) => {
                            Holder::Nothing(Dropped::Discarded(body.expr_span(p)))
                        }
                        _ => Holder::Nothing(Dropped::Contained(body.expr_span(p))),
                    };
                }
                // `use handle = Maps.create(..)`, the scoped form.
                Expr::Keyword {
                    keyword,
                    modifiers,
                    args,
                    ..
                } if keyword == "use" && args.first() == Some(&at) => {
                    return match modifiers.first() {
                        Some(name) => Holder::Binding {
                            binder: Binder::Use(p),
                            name: name.clone(),
                            at: p,
                            scoped: true,
                        },
                        None => Holder::Nothing(Dropped::Contained(body.expr_span(p))),
                    };
                }
                Expr::Lambda { .. } => {
                    return Holder::Nothing(Dropped::Returned(body.expr_span(p)));
                }
                Expr::For { .. } => {
                    return Holder::Nothing(Dropped::Statement(body.expr_span(at)));
                }
                // `Database.begin().commit()`: the receiver of a call.
                Expr::Field { base, .. } if *base == at => {
                    return match self.parents.get(&p).map(|c| (*c, body.expr(*c))) {
                        Some((c, Expr::Call { callee, .. })) if *callee == p => self.taken(c, p),
                        _ => Holder::Nothing(Dropped::Contained(body.expr_span(p))),
                    };
                }
                // `Database.commit(Database.begin())`: an argument.
                Expr::Call { callee, .. } if *callee != at => return self.taken(p, *callee),
                _ => return Holder::Nothing(Dropped::Contained(body.expr_span(p))),
            }
            at = p;
        }
    }

    /// The value `at` holds, given to the declaration's caller.
    fn given(&self, at: ExprId) -> Holder {
        if self.gives {
            Holder::Kept
        } else {
            Holder::Nothing(Dropped::Statement(self.body.expr_span(at)))
        }
    }

    /// Given to `call`: ended there if its callee releases what was
    /// acquired; otherwise taken by a callee that does not end it, or kept
    /// in the value a case makes.
    fn taken(&self, call: ExprId, callee: ExprId) -> Holder {
        let span = self.body.expr_span(call);
        match signature_of(self.sigs, self.types, self.body, callee) {
            Some(sig)
                if sig
                    .effects
                    .iter()
                    .any(|e| type_argument(e, "resource.release").as_deref() == Some(self.ty)) =>
            {
                Holder::Kept
            }
            Some(_) => Holder::Nothing(Dropped::Taken(span, path_of(self.body, callee))),
            None if function_value(self.body, self.types, callee) => {
                Holder::Nothing(Dropped::Taken(span, path_of(self.body, callee)))
            }
            None => Holder::Nothing(Dropped::Contained(span)),
        }
    }

    /// The function value `id` is in, if any: a `return` there leaves it.
    fn enclosing_lambda(&self, mut id: ExprId) -> Option<ExprId> {
        loop {
            if matches!(self.body.expr(id), Expr::Lambda { .. }) {
                return Some(id);
            }
            id = *self.parents.get(&id)?;
        }
    }
}

/// `Ok(x)`, `Err(e)`, `Some(x)`: the language's own cases, which hold what
/// they are given. The argument.
fn wraps(body: &Body, types: &Types<'_>, e: ExprId) -> Option<ExprId> {
    let Expr::Call { callee, args } = body.expr(e) else {
        return None;
    };
    let (Expr::Name(n), [arg]) = (body.expr(*callee), args.as_slice()) else {
        return None;
    };
    (matches!(n.as_str(), "Ok" | "Err" | "Some") && types.lexical().binder(*callee).is_none())
        .then_some(arg.value)
}

/// Does `e` mean the value `a` holds: a name whose binding is `a`'s
/// (ADR-0080)?
fn means(body: &Body, types: &Types<'_>, e: ExprId, a: &Acquired) -> bool {
    matches!(body.expr(e), Expr::Name(_)) && types.lexical().binder(e) == Some(a.binder)
}

/// Calls in this body that release the value, by span; and the calls that
/// give it to a function value, which may release it where nothing counts
/// (ADR-0080), by span and callee.
fn releases_of<'a>(
    body: &Body,
    sigs: &'a Signatures,
    types: &Types<'a>,
    a: &Acquired,
    reached: &[ExprId],
) -> (Vec<Span>, Vec<(Span, String)>) {
    let (mut out, mut given) = (Vec::new(), Vec::new());
    for &id in reached {
        let Expr::Call { callee, args } = body.expr(id) else {
            continue;
        };
        // Either `tx.commit()` — the value is the receiver — or
        // `Database.commit(tx)`, where it is an argument.
        let receiver = matches!(body.expr(*callee), Expr::Field { base, .. }
            if means(body, types, *base, a));
        let passed = args.iter().any(|arg| means(body, types, arg.value, a));
        if !(receiver || passed) {
            continue;
        }
        let Some(sig) = signature_of(sigs, types, body, *callee) else {
            // A function value a binding holds: `f(tx)`, where `f` is a
            // parameter, a lambda, or a declaration chosen at run time.
            if function_value(body, types, *callee) {
                given.push((body.expr_span(id), path_of(body, *callee)));
            }
            continue;
        };
        if sig
            .effects
            .iter()
            .any(|e| type_argument(e, "resource.release").as_deref() == Some(a.ty.as_str()))
        {
            out.push(body.expr_span(id));
        }
    }
    (out, given)
}

/// Is `callee` a value a binding in scope holds, `f` or `box.f`, rather than
/// a declaration, a constructor or a language form?
fn function_value(body: &Body, types: &Types<'_>, callee: ExprId) -> bool {
    match body.expr(callee) {
        Expr::Name(_) => types.lexical().binder(callee).is_some(),
        Expr::Field { base, .. } => function_value(body, types, *base),
        _ => false,
    }
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
    types: &'a Types<'a>,
    acquired: &'a Acquired,
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
            // after it on this path runs. The value is the body's, moved to
            // the caller where it is the value named (`tails`).
            if matches!(self.body.expr(s), Expr::Name(n) if n == "return") {
                let value = match stmts.get(i + 1) {
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
            // **A loop's body runs any number of times** (ADR-0211, ruling
            // 0045-a). Its iterable is evaluated once. A path that leaves the
            // body inside a pass, `return` or a failing `?`, leaves it as any
            // other exit does, owing what it has not released: until
            // ADR-0211 a `for` had no exits, and a transaction left open by
            // a `return` in one checked. A release is accepted only on a
            // path that leaves before the pass ends; one on a path that goes
            // round again runs again.
            Expr::For { iterable, body, .. } => {
                let pass = self.expr(*body);
                let repeated = pass.repeated.or_else(|| {
                    pass.through
                        .iter()
                        .any(|c| *c > 0)
                        .then(|| self.first_release(*body))
                        .flatten()
                });
                self.expr(*iterable).then(Flow {
                    through: BTreeSet::from([0]),
                    exits: pass.exits,
                    repeated,
                })
            }
            // A function value's body runs any number of times, and its
            // `return` leaves the function value, not this body.
            Expr::Lambda { .. } => Flow {
                through: BTreeSet::from([0]),
                exits: Vec::new(),
                repeated: self.first_release(id),
            },
            Expr::Name(_)
                if self.tails.contains(&id) && means(self.body, self.types, id, self.acquired) =>
            {
                Flow::releasing(1)
            }
            _ => self.children(id),
        }
    }

    /// The first release inside `id`, where a reader meets it.
    fn first_release(&self, id: ExprId) -> Option<Span> {
        self.body
            .walk_from(id)
            .into_iter()
            .map(|e| self.body.expr_span(e))
            .find(|s| self.releases.contains(s))
    }

    fn children(&self, id: ExprId) -> Flow {
        self.body
            .children(id)
            .into_iter()
            .fold(Flow::identity(), |f, c| f.then(self.expr(c)))
    }
}

/// **The expressions whose value is the body's**, given to the caller: its
/// last statement's, and each `return`'s that is the body's own and not a
/// function value's; through blocks, branches, arms, and `Ok(..)` and
/// `Some(..)`, which hold what they are given. None where the declaration
/// gives its caller nothing (ADR-0250): until then a body declaring `()`
/// moved a transaction it ended in to a caller that never had it.
fn tails(
    body: &Body,
    types: &Types<'_>,
    gives: bool,
    resource: &BTreeSet<ExprId>,
) -> BTreeSet<ExprId> {
    let mut out = BTreeSet::new();
    if gives {
        value_of(body, types, body.root, &mut out);
        returned(body, types, body.root, &mut out);
    }
    // What an `acquire` clause holds at its end moves to the resource, whose
    // `release` clause ends it (ADR-0251).
    for r in resource {
        value_of(body, types, *r, &mut out);
    }
    out
}

/// The expressions whose value is `id`'s.
fn value_of(body: &Body, types: &Types<'_>, id: ExprId, out: &mut BTreeSet<ExprId>) {
    match body.expr(id) {
        Expr::Block { stmts } => {
            if let Some(last) = stmts.last() {
                value_of(body, types, *last, out);
            }
        }
        Expr::If {
            then, els: Some(e), ..
        } => {
            value_of(body, types, *then, out);
            value_of(body, types, *e, out);
        }
        Expr::Match { arms, .. } => {
            for a in arms {
                value_of(body, types, a.body, out);
            }
        }
        _ => {
            out.insert(id);
            if let Some(held) = wraps(body, types, id) {
                value_of(body, types, held, out);
            }
        }
    }
}

/// Each `return e` under `id`, outside a function value: `e`'s value is the
/// body's.
fn returned(body: &Body, types: &Types<'_>, id: ExprId, out: &mut BTreeSet<ExprId>) {
    match body.expr(id) {
        Expr::Lambda { .. } => return,
        Expr::Block { stmts } => {
            for w in stmts.windows(2) {
                if matches!(body.expr(w[0]), Expr::Name(n) if n == "return") {
                    value_of(body, types, w[1], out);
                }
            }
        }
        _ => {}
    }
    for c in body.children(id) {
        returned(body, types, c, out);
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
    /// Given to a function value, `f(tx)`, which may release it where
    /// nothing counts (ADR-0080): the call, and what it calls.
    Given(Span, String),
}

/// The scope `a` is acquired in: the statements after its binding, in the
/// block that holds it, or the whole body for a parameter.
fn fault_of(
    body: &Body,
    types: &Types<'_>,
    a: &Acquired,
    releases: &[Span],
    gives: bool,
    reached: &[ExprId],
    resource: &BTreeSet<ExprId>,
) -> Option<Fault> {
    let paths = Paths {
        body,
        types,
        acquired: a,
        releases,
        tails: tails(body, types, gives, resource),
    };
    let (flow, end) = if let Some(clause) = a.clause {
        // What a `release` clause is given, through its block (ADR-0251).
        (paths.expr(clause), body.expr_span(clause))
    } else if a.parameter {
        (paths.expr(body.root), body.expr_span(body.root))
    } else {
        let (stmts, at) = reached.iter().find_map(|&b| match body.expr(b) {
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
fn escape_of(
    body: &Body,
    types: &Types<'_>,
    a: &Acquired,
    module_state: &[&str],
    reached: &[ExprId],
) -> Option<Span> {
    reached.iter().copied().find_map(|id| {
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
        // Module state, and not a local that shares its name.
        if !module_state.contains(&target.as_str()) || types.lexical().binder(*lhs).is_some() {
            return None;
        }
        // The value need not be assigned bare: `Some(handle)` still stores it.
        body.walk_from(*rhs)
            .into_iter()
            .any(|e| means(body, types, e, a))
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
        Fault::Given(span, to) => (
            "affine_value_given_to_a_function_value",
            format!(
                "affine resource `{}: {}` is given to `{to}`, a function value, which may \
                 release it",
                a.name, a.ty
            ),
            span,
            format!(
                "end `{}` with {ways} here, or bind one of them to a name nothing reassigns",
                a.name
            ),
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
    let acquired = if a.clause.is_some() {
        format!(
            "`{}` is what its resource's `acquire` made, and this `release` must end it",
            a.name
        )
    } else if a.parameter {
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

/// **An acquisition nothing holds** (ADR-0250): it can never be ended.
fn report_unheld(sigs: &Signatures, decl: &Decl, u: Unheld, at: &Span, out: &mut Vec<Diagnostic>) {
    let ways = release_functions(sigs, &u.ty);
    let ways = if ways.is_empty() {
        "a function that releases it".to_string()
    } else {
        ways.iter()
            .map(|w| format!("`{w}`"))
            .collect::<Vec<_>>()
            .join(" or ")
    };
    let ty = &u.ty;
    let (reason, message, site, label) = match u.dropped {
        Dropped::Discarded(span) => (
            "affine_value_discarded",
            format!("affine resource `{ty}` is bound to `_`, and nothing can release it"),
            span,
            "`_` binds nothing".to_string(),
        ),
        Dropped::Statement(span) => (
            "affine_value_dropped",
            format!("affine resource `{ty}` is acquired, and its statement drops it"),
            span,
            "nothing reads this statement's value".to_string(),
        ),
        Dropped::Clause(span) => (
            "affine_value_dropped",
            format!("affine resource `{ty}` is acquired, and its clause drops it"),
            span,
            "nothing reads this clause's value".to_string(),
        ),
        Dropped::Taken(span, to) => (
            "affine_value_taken",
            format!("affine resource `{ty}` is given to `{to}`, which does not release it"),
            span,
            format!("`{to}` takes it, and nothing ends it after"),
        ),
        Dropped::Matched(span) => (
            "affine_value_matched",
            format!(
                "affine resource `{ty}` is matched where it is acquired, and what the arms bind \
                 is not followed"
            ),
            span,
            "nothing follows a value an arm binds".to_string(),
        ),
        Dropped::Returned(span) => (
            "affine_value_returned_by_a_function_value",
            format!(
                "affine resource `{ty}` is returned by a function value, which nothing follows"
            ),
            span,
            "what calls this function value is not followed".to_string(),
        ),
        Dropped::Contained(span) => (
            "affine_value_contained",
            format!("affine resource `{ty}` is kept in a value that nothing follows"),
            span,
            "nothing follows this value".to_string(),
        ),
    };
    let repair = if u.carried {
        format!("bind it to a name with `?`, `let x = ..?`, and end it with {ways}")
    } else {
        format!("bind it to a name, and end it with {ways}")
    };
    out.push(Diagnostic {
        code: codes::AFFINE_NOT_CONSUMED_ONCE.id,
        invariant: codes::AFFINE_NOT_CONSUMED_ONCE.invariant,
        reason,
        detector: Detector::PatternMatrix,
        severity: Severity::Error,
        message,
        primary_span: u.span,
        related: vec![
            Related { span: site, label },
            Related {
                span: at.clone(),
                label: format!("`{}` must end it exactly once on every path", decl.name),
            },
        ],
        explanation: Some(format!(
            "A {ty} must end exactly once, with {ways}. What ends it is followed from \
             a name: a binding, `let x = ..` or `use x = ..`, to where it is released. \
             An acquisition no name holds, and that is neither ended where it is made \
             nor given to the caller, can never be ended."
        )),
        repairs: vec![Repair {
            description: repair,
            replacement: None,
        }],
    });
}

/// Every function the program declares as releasing this resource type and
/// taking one, as `released_parameters` reads a row. A declaration whose row
/// releases one only because its body calls `commit` is not a way to end
/// another's: until ADR-0250 the repair named the function being checked.
fn release_functions(sigs: &Signatures, ty: &str) -> Vec<String> {
    let mut out: Vec<String> = sigs
        .iter()
        .filter(|(_, s)| {
            s.effects
                .iter()
                .any(|e| type_argument(e, "resource.release").as_deref() == Some(ty))
                && s.params.iter().any(|p| {
                    p.as_ref()
                        .and_then(|r| r.resolved())
                        .is_some_and(|t| t.written_source() == ty)
                })
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
/// whose type is known. A local bound to a declaration, `let end =
/// Database.rollback`, is that declaration (ADR-0080). Nothing else.
fn signature_of<'a>(
    sigs: &'a Signatures,
    types: &Types<'a>,
    body: &Body,
    callee: ExprId,
) -> Option<&'a crate::signatures::Signature> {
    if matches!(body.expr(callee), Expr::Name(_)) && types.lexical().binder(callee).is_some() {
        let init = alias_of(body, types, callee)?;
        return sigs
            .by_path(&path_of(body, init))
            .or_else(|| types.callee(body, init));
    }
    sigs.by_path(&path_of(body, callee))
        .or_else(|| types.callee(body, callee))
}

/// What a `let` binding the name holds, where nothing reassigns it: its
/// initialiser.
fn alias_of(body: &Body, types: &Types<'_>, name: ExprId) -> Option<ExprId> {
    let Binder::Pattern(p) = types.lexical().binder(name)? else {
        return None;
    };
    if !matches!(
        body.pat(p),
        crate::hir::Pattern::Bind { mutable: false, .. }
    ) {
        return None;
    }
    body.exprs().find_map(|(_, e, _)| match e {
        Expr::Let {
            pat: Some(q),
            init: Some(init),
            ..
        } if *q == p => Some(*init),
        _ => None,
    })
}

use crate::infer::path_of;
