//! Local type inference — enough to resolve a member by its **receiver's
//! type** rather than by that member's name being unique in the program.
//!
//! # Why this exists
//!
//! `Signatures::member` had a fallback: with no receiver type, resolve to the
//! one declaration with that member name, if exactly one exists. It was
//! conservative in the sense that it never resolved ambiguously — and unsafe in
//! a way conservatism does not fix, because *whether a correctness check runs
//! at all* depended on a global accident. `resolve_corpus` demonstrated it: a
//! sibling module declaring a second `on_press` made the name ambiguous, and
//! the handler rule went **silent**. Not wrong — silent, which is worse,
//! because nothing is reported and the count does not move.
//!
//! The architect's ruling is the right one:
//!
//! > A correctness analysis should never mean "use this member because it
//! > happens to be the only one with this spelling."
//!
//! # What it infers
//!
//! A type for an expression, from four sources, all of them declarations:
//!
//! - a parameter's or `let`'s written annotation;
//! - `self` inside a UI declaration, which is that declaration's element;
//! - a call's return type, from the callee's signature;
//! - a field access, from the signature of the member on the base's type.
//!
//! That chain is what makes `self.style.set_padding(4.px)` resolvable:
//! `self` is an `ElementRef`, `.style` is a member of `ElementRef` returning
//! `Style`, and `.set_padding` is a member of `Style`. No step asks whether a
//! name is unique.
//!
//! # What it does not infer
//!
//! Unannotated lambda parameters. `List.map(items, el => el.offsetWidth())`
//! gives `el` no type, because that needs the generic's instantiation. Callers
//! are told so — [`Types::of`] returns `None` — and it is each caller's
//! decision whether to fall back or to stop. `docs/evidence/P0/readiness.txt`
//! records which ones still fall back.

use std::collections::BTreeMap;

use crate::hir::{Body, Decl, DeclKind, Expr, ExprId};
use crate::signatures::Signatures;

/// The types known inside one declaration's body.
pub struct Types<'a> {
    sigs: &'a Signatures,
    bindings: BTreeMap<String, String>,
    /// The module this body lives in, so a bare name resolves to a sibling
    /// declaration. Without it `shrink(self)` looked up `shrink` and found
    /// nothing, because signatures are stored module-qualified — so a rule
    /// following a helper stopped at the module boundary of its own file.
    module: Option<String>,
}

impl<'a> Types<'a> {
    /// Gather what this declaration's own text says about its bindings.
    pub fn of_body(sigs: &'a Signatures, decl: &Decl, body: &Body) -> Types<'a> {
        let mut bindings: BTreeMap<String, String> = BTreeMap::new();

        for p in &decl.params {
            if let Some(t) = &p.ty {
                bindings.insert(p.name.clone(), t.clone());
            }
        }

        // Charter §7.5A writes `self.style.set_padding(..)` inside a component.
        // `self` is the declaration's own element, and that is a language fact
        // about UI declarations rather than something a library declares.
        if matches!(
            decl.kind,
            DeclKind::View | DeclKind::Component | DeclKind::Page
        ) {
            bindings.insert("self".to_string(), "ElementRef".to_string());
        }

        // Annotated bindings first, then inferred ones — a written annotation
        // is the most precise thing available and must not be overwritten by a
        // guess about its initialiser.
        for id in body.walk() {
            let Expr::Let {
                pat: Some(pat),
                ty: Some(ty),
                ..
            } = body.expr(id)
            else {
                continue;
            };
            if let (crate::hir::Pattern::Bind { name, .. }, Some(t)) =
                (body.pat(*pat), body.types.get(ty.index()))
            {
                bindings.insert(name.clone(), t.path.clone());
            }
        }

        let mut types = Types {
            sigs,
            bindings,
            module: None,
        };

        // A binding whose initialiser has a knowable type. Iterated, so
        // `let a = f()` then `let b = a.g()` both resolve; bounded because each
        // round can only add bindings and there are finitely many.
        for _ in 0..4 {
            let mut added = false;
            for id in body.walk() {
                let Expr::Let {
                    pat: Some(pat),
                    ty: None,
                    init: Some(init),
                } = body.expr(id)
                else {
                    continue;
                };
                let crate::hir::Pattern::Bind { name, .. } = body.pat(*pat) else {
                    continue;
                };
                if types.bindings.contains_key(name) {
                    continue;
                }
                if let Some(t) = types.of(body, *init) {
                    types.bindings.insert(name.clone(), t);
                    added = true;
                }
            }
            if !added {
                break;
            }
        }
        types
    }

    /// Resolve bare names against this module's own declarations.
    pub fn in_module(mut self, module: Option<&str>) -> Self {
        self.module = module.map(str::to_string);
        self
    }

    /// A declaration by path, trying this module first.
    fn by_path(&self, path: &str) -> Option<&'a crate::signatures::Signature> {
        self.module
            .as_deref()
            .and_then(|m| self.sigs.by_path(&format!("{m}.{path}")))
            .or_else(|| self.sigs.by_path(path))
    }

    /// The type of an expression, or `None` when nothing declared says.
    ///
    /// `None` is a real answer and callers must treat it as one. It means "this
    /// program does not say", not "look it up some other way".
    pub fn of(&self, body: &Body, id: ExprId) -> Option<String> {
        match body.expr(id) {
            Expr::Name(n) => self.bindings.get(n).cloned(),
            Expr::Field { base, name } => {
                let receiver = self.of(body, *base)?;
                self.sigs
                    .member_of(&receiver, name)
                    .and_then(|s| s.returns.clone())
            }
            Expr::Call { callee, .. } => match body.expr(*callee) {
                // `el.style()` and `el.style` are the same member; the parser
                // keeps the distinction and the type does not depend on it.
                Expr::Field { base, name } => {
                    let receiver = self.of(body, *base)?;
                    self.sigs
                        .member_of(&receiver, name)
                        .and_then(|s| s.returns.clone())
                }
                _ => self
                    .by_path(&path_of(body, *callee))
                    .and_then(|s| s.returns.clone()),
            },
            _ => None,
        }
    }

    /// The signature a call resolves to, **only** when the receiver's type is
    /// known. Never guesses from a name.
    pub fn callee(&self, body: &Body, callee: ExprId) -> Option<&'a crate::signatures::Signature> {
        match body.expr(callee) {
            Expr::Field { base, name } => {
                let receiver = self.of(body, *base)?;
                self.sigs.member_of(&receiver, name)
            }
            _ => self.by_path(&path_of(body, callee)),
        }
    }
}

pub fn path_of(body: &Body, id: ExprId) -> String {
    match body.expr(id) {
        Expr::Name(n) => n.clone(),
        Expr::Field { base, name } => format!("{}.{}", path_of(body, *base), name),
        _ => String::new(),
    }
}
