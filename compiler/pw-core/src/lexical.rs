//! **Which binding each local name means** (ADR-0063).
//!
//! A body binds names in scopes:
//! - a declaration's parameters, for its whole body;
//! - a `let` or a `use`, for the statements after it in its block;
//! - a lambda's parameters, a match arm's names and a `for` loop's, for the
//!   body they head;
//! - a template's `{#each xs as x}`, for the block's children;
//! - a `{:Some(x)}` arm, for the children up to the next marker;
//! - a policy term's binders, for its own tree.
//!
//! Until 2026-09-26 the value relations kept one flat environment per body,
//! and a name bound at two sites was unknown wherever it was used. So `x + 1`
//! over an `Option<String>`'s `Some(x)` passed `pw check` wherever another
//! arm, lambda or `let` also bound an `x`. [`Lexical`] gives each use the one
//! binding it means, by the scopes the backend lowers it with
//! (`backend::lower`), so the binding the checker typed is the binding that
//! runs.
//!
//! A name no binding in scope has is not a local: it is resolved as the
//! program's own, as before.

use std::collections::BTreeMap;

use crate::hir::{AttrValue, Body, Decl, Expr, ExprId, Node, NodeId, Pattern, PatternId};
use crate::resolve::{Namespace, Resolution, UnitId};
use crate::signatures::Signatures;

/// **Is a name written alone in a pattern a case rather than a binding?**
/// The language's `true`, `false` or `None`, or a case of a type `at` sees
/// (ADR-0038). Such a pattern tests; it binds nothing. `at` is `None` for a
/// file that declares no module, which sees no declared type.
///
/// The one rule, with every reader of [`Lexical`]: a pattern the value
/// relations read as a test and another analysis read as a binding would give
/// one name two meanings.
pub fn names_a_case(sigs: &Signatures, at: Option<UnitId>, name: &str) -> bool {
    let ws = sigs.workspace();
    let own = at.is_none_or(|at| {
        matches!(
            ws.resolve_in(at, Namespace::Term, name),
            Resolution::Unresolved
        )
    });
    (own && matches!(name, "true" | "false" | "None"))
        || at.is_some_and(|at| {
            ws.visible_types(at).into_iter().any(|def| {
                sigs.type_decl(def)
                    .and_then(|t| t.variants.as_ref())
                    .is_some_and(|cs| cs.iter().any(|(n, _)| n == name))
            })
        })
}

/// **Where a local name is bound.**
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum Binder {
    /// The declaration's parameter, by position.
    Param(usize),
    /// A name in a pattern: a `let`'s, a lambda's parameter, a match arm's,
    /// a `for` loop's.
    Pattern(PatternId),
    /// `use x = e`: the statement.
    Use(ExprId),
    /// `{#each xs as x}`: the block.
    Each(NodeId),
    /// The `i`th name a `{#match}` arm binds, by its marker: `w` and `h` in
    /// `{:Rect(w, h)}`.
    Arm(NodeId, usize),
    /// A policy term's `j`th binder, by the term's position among the
    /// declaration's term roots: `cart` in `optimistic .. as cart => ..`.
    Term(usize, usize),
}

/// **The binding each use of a local name means, in one body.**
#[derive(Debug, Default)]
pub struct Lexical {
    /// Each `Expr::Name` that names a local, and the binding it names.
    uses: BTreeMap<ExprId, Binder>,
    /// `Point { x }`: a record's shorthand field, by the record and the
    /// field's position, and the binding its name means.
    shorthand: BTreeMap<(ExprId, usize), Binder>,
    /// `{#each xs as x}`: the collection as the directive writes it, and the
    /// binding its first name means where a local's does.
    each: BTreeMap<NodeId, (String, Option<Binder>)>,
    /// A `{#match}` arm's marker, and the expression its block matches.
    arm_subject: BTreeMap<NodeId, ExprId>,
}

/// The names in scope, innermost last.
type Scope = Vec<(String, Binder)>;

impl Lexical {
    /// Resolve every local name in `decl`'s body, a body of unit `at`, by
    /// [`names_a_case`].
    pub fn build(sigs: &Signatures, at: Option<UnitId>, decl: &Decl, body: &Body) -> Lexical {
        Lexical::of(decl, body, &|name| names_a_case(sigs, at, name))
    }

    /// Resolve every local name in `decl`'s body and policy terms.
    ///
    /// `is_case` says whether a name written alone in a pattern is a case
    /// rather than a binding: `None`, `true`, or a case of a type the unit
    /// sees (ADR-0038). Such a pattern binds nothing.
    pub fn of(decl: &Decl, body: &Body, is_case: &dyn Fn(&str) -> bool) -> Lexical {
        let mut out = Lexical::default();
        let params: Scope = decl
            .params
            .iter()
            .enumerate()
            .map(|(i, p)| (p.name.clone(), Binder::Param(i)))
            .collect();
        let mut walk = Walk {
            body,
            is_case,
            out: &mut out,
        };
        walk.expr(body.root, &mut params.clone());
        // A term root is its own tree, reached from no statement: it sees the
        // declaration's parameters and its own binders, and nothing the body
        // binds.
        for (i, (_, root)) in decl.term_roots().enumerate() {
            let mut scope = params.clone();
            scope.extend(
                root.binders
                    .iter()
                    .enumerate()
                    .map(|(j, (name, _))| (name.clone(), Binder::Term(i, j))),
            );
            walk.expr(root.root, &mut scope);
        }
        out
    }

    /// The binding a name means where `use_` writes it, or `None` where no
    /// binding in scope has the name.
    pub fn binder(&self, use_: ExprId) -> Option<Binder> {
        self.uses.get(&use_).copied()
    }

    /// The binding a record's shorthand field names: `x` in `Point { x }`.
    pub fn shorthand(&self, record: ExprId, field: usize) -> Option<Binder> {
        self.shorthand.get(&(record, field)).copied()
    }

    /// Every `{#each}` block: the collection as written, and the binding its
    /// first name means where a local's does.
    pub fn each_blocks(&self) -> impl Iterator<Item = (NodeId, &str, Option<Binder>)> {
        self.each
            .iter()
            .map(|(n, (collection, head))| (*n, collection.as_str(), *head))
    }

    /// Every `{#match}` arm's marker, and the expression its block matches.
    pub fn template_arms(&self) -> impl Iterator<Item = (NodeId, ExprId)> + '_ {
        self.arm_subject.iter().map(|(n, s)| (*n, *s))
    }
}

struct Walk<'a> {
    body: &'a Body,
    is_case: &'a dyn Fn(&str) -> bool,
    out: &'a mut Lexical,
}

fn find(scope: &Scope, name: &str) -> Option<Binder> {
    scope.iter().rev().find(|(n, _)| n == name).map(|(_, b)| *b)
}

impl Walk<'_> {
    fn expr(&mut self, id: ExprId, scope: &mut Scope) {
        match self.body.expr(id) {
            Expr::Name(n) => {
                if let Some(b) = find(scope, n) {
                    self.out.uses.insert(id, b);
                }
            }
            Expr::Block { stmts } => {
                let depth = scope.len();
                for s in stmts {
                    self.statement(*s, scope);
                }
                scope.truncate(depth);
            }
            // A `let` outside a block binds for nothing after it.
            Expr::Let { init, .. } => {
                if let Some(init) = init {
                    self.expr(*init, scope);
                }
            }
            Expr::Lambda {
                descriptor,
                params,
                body,
            } => {
                // `resumable(captures = { item })` names what is around the
                // lambda, not its parameters.
                if let Some(d) = descriptor {
                    self.expr(*d, scope);
                }
                let depth = scope.len();
                for p in params {
                    self.pattern(*p, scope);
                }
                self.expr(*body, scope);
                scope.truncate(depth);
            }
            Expr::Match { scrutinee, arms } => {
                self.expr(*scrutinee, scope);
                for arm in arms {
                    let depth = scope.len();
                    self.pattern(arm.pat, scope);
                    self.expr(arm.body, scope);
                    scope.truncate(depth);
                }
            }
            Expr::For {
                pat,
                iterable,
                body,
            } => {
                self.expr(*iterable, scope);
                let depth = scope.len();
                if let Some(p) = pat {
                    self.pattern(*p, scope);
                }
                self.expr(*body, scope);
                scope.truncate(depth);
            }
            Expr::Record { fields, .. } => {
                for (i, f) in fields.iter().enumerate() {
                    match f.value {
                        Some(v) => self.expr(v, scope),
                        None => {
                            if let Some(b) = find(scope, &f.name) {
                                self.out.shorthand.insert((id, i), b);
                            }
                        }
                    }
                }
            }
            // The markup, not the flat list of its parts: a name a block or
            // an arm binds is in scope in its own children only.
            Expr::Template { roots, .. } => {
                let roots = roots.clone();
                self.markup(&roots, None, scope);
            }
            _ => {
                for c in self.body.children(id) {
                    self.expr(c, scope);
                }
            }
        }
    }

    /// One statement of a block: a `let` or a `use` binds for the rest of it.
    fn statement(&mut self, id: ExprId, scope: &mut Scope) {
        match self.body.expr(id) {
            Expr::Let { pat, init, .. } => {
                if let Some(init) = init {
                    self.expr(*init, scope);
                }
                if let Some(p) = pat {
                    self.pattern(*p, scope);
                }
            }
            Expr::Keyword {
                keyword,
                modifiers,
                args,
                block,
                ..
            } if keyword == "use" => {
                for a in args {
                    self.expr(*a, scope);
                }
                if let Some(b) = block {
                    self.expr(*b, scope);
                }
                if let Some(name) = modifiers.first() {
                    scope.push((name.clone(), Binder::Use(id)));
                }
            }
            _ => self.expr(id, scope),
        }
    }

    /// The names a pattern binds, pushed in order. A name that is a case is
    /// a test, not a binding. An or-pattern binds its first alternative's
    /// names, which every alternative must bind alike.
    fn pattern(&mut self, p: PatternId, scope: &mut Scope) {
        match self.body.pat(p) {
            Pattern::Bind { name, .. } => {
                if !(self.is_case)(name) {
                    scope.push((name.clone(), Binder::Pattern(p)));
                }
            }
            Pattern::Ctor { args, .. } => {
                for a in args.clone() {
                    self.pattern(a, scope);
                }
            }
            Pattern::Or(alternatives) => {
                if let Some(first) = alternatives.first().copied() {
                    self.pattern(first, scope);
                }
            }
            Pattern::Wild | Pattern::Literal(_) | Pattern::Error => {}
        }
    }

    /// A sequence of sibling nodes. Inside a block, `subject` is the
    /// expression the block matches, and a `{:..}` marker ends the previous
    /// arm's names and begins its own.
    fn markup(&mut self, nodes: &[NodeId], subject: Option<ExprId>, scope: &mut Scope) {
        let depth = scope.len();
        for n in nodes {
            match self.body.node(*n) {
                Node::Element {
                    attrs, children, ..
                } => {
                    let values: Vec<ExprId> = attrs
                        .iter()
                        .filter_map(|a| match a.value {
                            AttrValue::Expr(e) => Some(e),
                            _ => None,
                        })
                        .collect();
                    for v in values {
                        self.expr(v, scope);
                    }
                    let children = children.clone();
                    self.markup(&children, None, scope);
                }
                Node::Interpolation(e) => self.expr(*e, scope),
                Node::Text(_) => {}
                Node::Block {
                    directive,
                    children,
                    subject: own,
                    ..
                } => {
                    if let Some(s) = own {
                        self.expr(*s, scope);
                    }
                    let inner = scope.len();
                    if let Some((name, collection)) = crate::infer::each_binding(directive) {
                        let head = collection
                            .split(['.', '('])
                            .next()
                            .map(str::trim)
                            .and_then(|h| find(scope, h));
                        self.out.each.insert(*n, (collection, head));
                        scope.push((name, Binder::Each(*n)));
                    }
                    let (children, own) = (children.clone(), *own);
                    self.markup(&children, own, scope);
                    scope.truncate(inner);
                }
                Node::Branch { condition, arm, .. } => {
                    scope.truncate(depth);
                    if let Some(c) = condition {
                        self.expr(*c, scope);
                    }
                    if let Some(arm) = arm {
                        if let Some(s) = subject {
                            self.out.arm_subject.insert(*n, s);
                        }
                        for (i, name) in arm.bindings.iter().enumerate() {
                            scope.push((name.clone(), Binder::Arm(*n, i)));
                        }
                    }
                }
            }
        }
        scope.truncate(depth);
    }
}
