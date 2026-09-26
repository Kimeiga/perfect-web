//! **Every name resolves** (ADR-0047).
//!
//! `PW0021` was reported for a call whose callee resolves to nothing, for a
//! qualified call whose head is a module, and for a name inside a policy
//! term. A name used as a value was not examined: `let x = nothing` and
//! `{nothing.here}` in a template passed `pw check`, and every analysis read
//! the name as unknown, which each of them treats as "nothing to report".
//!
//! This walk follows lexical scope, in order:
//! - a `let` binds its names for the statements after it in its block, not
//!   for its own initialiser and not before it;
//! - a lambda's parameters, a loop's pattern and a match arm's pattern bind
//!   for their bodies only;
//! - in a template, `{#each xs as x}` binds `x` for the block's children, and
//!   `{:Some(x)}` binds `x` for its own arm.
//!
//! A name no scope binds must name a declaration this module can see (local,
//! imported or from a prelude), a constructor of a type it can see, or one of
//! the language's own values (`true`, `None`, `self`). A field path's head is
//! held to the same rule, and a path through a module to that module's
//! members. Names in call position keep their existing rule
//! (`check::unresolved_uses`), which knows the call-shaped owners: policy
//! operators, CSS value functions, privacy labels.
//!
//! # Clauses the grammar keeps as names
//!
//! Inside a body, a clause is written as statements: `scope component` is two
//! names, `acquire { .. }` a name and a block, `release(h) { .. }` a call and
//! a block, and a page's `view { .. }` a name and a block. The analyses that
//! give them meaning read them in that shape (`check::name_pair`), and the
//! policy table says what each clause's value is (`policy::domain_of`). So:
//! - a statement naming a clause, followed by its value on the same line or
//!   by a block, is syntax and not a use;
//! - its value is a word of the clause's domain (`component`, from `scope`'s
//!   closed set), or code where the table says the clause carries terms;
//! - a block after it is code, and `release(h)` binds `h` in its block.
//!
//! What a name refers to past its head is not this walk's question: `box.x`
//! on a type with no `x` is the member relation's (ADR-0048).

use std::collections::{BTreeMap, BTreeSet};

use crate::diagnostics::{Detector, Diagnostic, Related, Repair, Severity};
use crate::hir::{AttrValue, Body, Decl, DeclId, Expr, ExprId, Hir, Node, NodeId, Pattern, Span};
use crate::resolve::{DefId, Resolution, UnitId, Workspace};

/// Names the language gives a meaning to wherever the program has not
/// declared one: its literals, its own variants, and the statement word
/// `return`, which the grammar keeps as a name.
const LANGUAGE_VALUES: &[&str] = &["true", "false", "return"];

/// Heads of the platform's effect paths, as `check::unresolved_uses` reads
/// them in a call: `clock.now()`, `secrets.payments`. The path's resolution
/// is that rule's; a value path with one of these heads is left to it.
const EFFECT_HEADS: &[&str] = &["secrets", "style", "clock", "log", "database", "dom"];

/// The structured-concurrency forms, which the scope graph reads by path
/// (`check::scope_violations`): `task.spawn(detached) { .. }`,
/// `task.spawn(scope = component) { .. }`. A bare word among their arguments
/// is the form's (`detached`, a scope's name), not a use.
const SPAWN_FORMS: &[&str] = &["task.spawn", "durable.spawn"];

/// Every use of a name that resolves to nothing, in `unit`.
pub fn check(workspace: &Workspace, hirs: &[&Hir], unit: UnitId, src: &str) -> Vec<Diagnostic> {
    let hir = hirs[unit];
    let constructors = visible_constructors(workspace, hirs, unit);
    // A declaration nested in another (`fn load()` inside a component) sees
    // the enclosing declarations' parameters and bindings, and any
    // declaration of this file is visible by name, as a call to it is.
    let mut parent = std::collections::BTreeMap::new();
    for (id, decl) in hir.all_decls() {
        for c in &decl.children {
            parent.insert(*c, id);
        }
    }
    // An `import` is a declaration too, and binds nothing by its name: a
    // module path through it is resolved below, member by member.
    let declared: BTreeSet<String> = hir
        .all_decls()
        .filter(|(_, d)| d.kind != crate::hir::DeclKind::Import)
        .map(|(_, d)| d.name.clone())
        .collect();
    // A module-level `let mut` may be assigned from a body (ADR-0051).
    let module_mutable: BTreeSet<String> = hir
        .all_decls()
        .filter(|(_, d)| d.mutable)
        .map(|(_, d)| d.name.clone())
        .collect();
    let mut out = Vec::new();
    for (id, decl) in hir.all_decls() {
        let Some(body_id) = decl.body else { continue };
        let body = hir.body(body_id);
        let mut outer = declared.clone();
        let mut at = id;
        while let Some(&p) = parent.get(&at) {
            let enclosing = hir.decl(p);
            outer.extend(enclosing.params.iter().map(|p| p.name.clone()));
            if let Some(b) = enclosing.body {
                outer.extend(crate::resolve::local_bindings(hir.body(b)));
            }
            at = p;
        }
        let mut walk = Walk {
            body,
            src,
            workspace,
            unit,
            constructors: &constructors,
            scopes: vec![outer, decl.params.iter().map(|p| p.name.clone()).collect()],
            mutable: vec![module_mutable.clone(), BTreeSet::new()],
            immutable: Vec::new(),
            visited: BTreeSet::new(),
            found: Vec::new(),
            ambiguous: Vec::new(),
        };
        walk.expr(body.root);
        for (span, name) in walk.found {
            out.push(diagnostic(hir, id, decl, span, &name));
        }
        for (span, name, types) in walk.ambiguous {
            out.push(ambiguous_case(hir, id, decl, span, &name, &types));
        }
        for (span, name) in walk.immutable {
            out.push(immutable_target(hir, id, decl, span, &name));
        }
    }
    out
}

/// The constructors of every sum type `unit` can see: its own, and those of
/// the types it imports, each with the types that declare it, by identity.
fn visible_constructors(
    workspace: &Workspace,
    hirs: &[&Hir],
    unit: UnitId,
) -> BTreeMap<String, BTreeMap<DefId, String>> {
    let mut out: BTreeMap<String, BTreeMap<DefId, String>> = BTreeMap::new();
    for def in workspace.visible_types(unit) {
        let Some(decl) = crate::resolve::declaration(hirs, def) else {
            continue;
        };
        for v in decl.variants.iter().flatten() {
            out.entry(v.name.clone())
                .or_default()
                .insert(def, decl.name.clone());
        }
    }
    out
}

struct Walk<'a> {
    body: &'a Body,
    src: &'a str,
    workspace: &'a Workspace,
    unit: UnitId,
    constructors: &'a BTreeMap<String, BTreeMap<DefId, String>>,
    /// Innermost last. A `let` adds to the innermost.
    scopes: Vec<BTreeSet<String>>,
    /// The names bound `let mut`, in the same scopes (ADR-0051).
    mutable: Vec<BTreeSet<String>>,
    /// Assignments to a binding that is not `let mut`.
    immutable: Vec<(Span, String)>,
    /// Every expression walked, so a template part reached through a nested
    /// region is not walked again in the outer template's scope.
    visited: BTreeSet<ExprId>,
    found: Vec<(Span, String)>,
    /// A bare case two visible types declare, and those types (ADR-0059).
    ambiguous: Vec<(Span, String, Vec<String>)>,
}

impl Walk<'_> {
    fn bound(&self, name: &str) -> bool {
        self.scopes.iter().any(|s| s.contains(name))
    }

    fn bind(&mut self, names: impl IntoIterator<Item = String>) {
        self.scopes
            .last_mut()
            .expect("a walk always has its parameters' scope")
            .extend(names);
    }

    fn scoped(&mut self, names: BTreeSet<String>, f: impl FnOnce(&mut Self)) {
        self.scopes.push(names);
        self.mutable.push(BTreeSet::new());
        f(self);
        self.scopes.pop();
        self.mutable.pop();
    }

    /// Is the binding `name` refers to here a `let mut` one? The innermost
    /// scope that binds it decides.
    fn is_mutable(&self, name: &str) -> bool {
        self.scopes
            .iter()
            .zip(&self.mutable)
            .rev()
            .find(|(s, _)| s.contains(name))
            .is_some_and(|(_, m)| m.contains(name))
    }

    /// Does a name no scope binds mean something here?
    fn resolves(&self, name: &str) -> bool {
        LANGUAGE_VALUES.contains(&name)
            || crate::resolve::INTRINSIC_CALLS.contains(&name)
            || self.constructors.contains_key(name)
            || !matches!(
                self.workspace.resolve(self.unit, name),
                Resolution::Unresolved
            )
    }

    fn name(&mut self, name: &str, span: Span) {
        if self.bound(name) {
            return;
        }
        if !self.resolves(name) {
            self.found.push((span, name.to_string()));
            return;
        }
        // `Empty` alone, where two types it sees each declare a case of that
        // name and nothing else has it: it names neither (ADR-0059).
        if let Some(types) = self.constructors.get(name)
            && types.len() > 1
            && !LANGUAGE_VALUES.contains(&name)
            && !crate::resolve::INTRINSIC_CALLS.contains(&name)
            && matches!(
                self.workspace.resolve(self.unit, name),
                Resolution::Unresolved
            )
        {
            self.ambiguous
                .push((span, name.to_string(), types.values().cloned().collect()));
        }
    }

    fn pattern_names(&self, p: crate::hir::PatternId, out: &mut BTreeSet<String>) {
        match self.body.pat(p) {
            Pattern::Bind { name, .. } => {
                out.insert(name.clone());
            }
            Pattern::Ctor { args, .. } => {
                for a in args {
                    self.pattern_names(*a, out);
                }
            }
            Pattern::Or(ps) => {
                for a in ps {
                    self.pattern_names(*a, out);
                }
            }
            Pattern::Wild | Pattern::Literal(_) | Pattern::Error => {}
        }
    }

    fn expr(&mut self, id: ExprId) {
        self.visited.insert(id);
        match self.body.expr(id) {
            Expr::Name(n) => self.name(n, self.body.expr_span(id)),
            Expr::Field { .. } => self.path(id),
            Expr::Call { callee, args } => {
                // The callee is `check::unresolved_uses`'s, which knows the
                // call-shaped owners. A callee that is itself a computed value
                // (`f(x)(y)`, `make().run()`) is walked for what it contains.
                let spawn =
                    SPAWN_FORMS.contains(&crate::infer::path_of(self.body, *callee).as_str());
                match self.body.expr(*callee) {
                    Expr::Name(_) => {}
                    Expr::Field { .. } if spawn => {}
                    Expr::Field { .. } => self.callee_path(*callee),
                    _ => self.expr(*callee),
                }
                for a in args.clone() {
                    if spawn && matches!(self.body.expr(a.value), Expr::Name(_)) {
                        self.visited.insert(a.value);
                        continue;
                    }
                    self.expr(a.value);
                }
            }
            Expr::Block { stmts } => {
                let stmts = stmts.clone();
                self.scoped(BTreeSet::new(), |w| w.statements(&stmts));
            }
            Expr::Let { pat, init, .. } => {
                if let Some(init) = *init {
                    self.expr(init);
                }
                if let Some(p) = *pat {
                    let mut names = BTreeSet::new();
                    self.pattern_names(p, &mut names);
                    // `let x` rebinds `x`: a `mut` from an earlier binding of
                    // the name in this scope is not this one's.
                    let innermost = self.mutable.last_mut().expect("a scope");
                    for n in &names {
                        innermost.remove(n);
                    }
                    if let Pattern::Bind {
                        name,
                        mutable: true,
                    } = self.body.pat(p)
                    {
                        innermost.insert(name.clone());
                    }
                    self.bind(names);
                }
            }
            // `x = e` (ADR-0051): `x` must be a `let mut` binding in scope. An
            // unbound `x` is the name rule's to report.
            Expr::Binary {
                op: crate::hir::BinOp::Assign,
                lhs,
                rhs,
            } => {
                let (lhs, rhs) = (*lhs, *rhs);
                if let Expr::Name(n) = self.body.expr(lhs) {
                    let n = n.clone();
                    self.visited.insert(lhs);
                    if self.bound(&n) || self.resolves(&n) {
                        if !self.is_mutable(&n) {
                            self.immutable.push((self.body.expr_span(lhs), n));
                        }
                    } else {
                        self.name(&n, self.body.expr_span(lhs));
                    }
                } else {
                    self.expr(lhs);
                }
                self.expr(rhs);
            }
            Expr::Lambda {
                descriptor,
                params,
                body,
            } => {
                if let Some(d) = *descriptor {
                    self.expr(d);
                }
                let mut names = BTreeSet::new();
                for p in params.clone() {
                    self.pattern_names(p, &mut names);
                }
                let body = *body;
                self.scoped(names, |w| w.expr(body));
            }
            Expr::Match { scrutinee, arms } => {
                self.expr(*scrutinee);
                for arm in arms.clone() {
                    let mut names = BTreeSet::new();
                    self.pattern_names(arm.pat, &mut names);
                    self.scoped(names, |w| w.expr(arm.body));
                }
            }
            Expr::For {
                pat,
                iterable,
                body,
            } => {
                self.expr(*iterable);
                let mut names = BTreeSet::new();
                if let Some(p) = *pat {
                    self.pattern_names(p, &mut names);
                }
                let body = *body;
                self.scoped(names, |w| w.expr(body));
            }
            Expr::Record { fields, .. } => {
                for f in fields.clone() {
                    match f.value {
                        Some(v) => self.expr(v),
                        // `Point { x, y }` reads `x` and `y` from scope.
                        None => self.name(&f.name, f.span.clone()),
                    }
                }
            }
            // `use key: T = acquire()` binds `key` for the statements after it,
            // and for its own block. `observe resize` binds nothing, and a
            // modifier bound needlessly can only make this walk quieter.
            Expr::Keyword {
                modifiers,
                args,
                block,
                ..
            } => {
                let first: BTreeSet<String> = modifiers.first().cloned().into_iter().collect();
                for a in args.clone() {
                    self.expr(a);
                }
                if let Some(b) = *block {
                    self.scoped(first.clone(), |w| w.expr(b));
                }
                self.bind(first);
            }
            Expr::Template { parts, roots } => {
                let (parts, roots) = (parts.clone(), roots.clone());
                for r in roots {
                    self.node(r);
                }
                // A part no node holds is still a use, in the template's scope.
                for p in parts {
                    if !self.visited.contains(&p) {
                        self.expr(p);
                    }
                }
            }
            _ => {
                for c in self.body.children(id) {
                    self.expr(c);
                }
            }
        }
    }

    /// A block's statements, in order, with the clauses the grammar keeps as
    /// names read as clauses (see the module header).
    fn statements(&mut self, stmts: &[ExprId]) {
        let mut i = 0;
        while let Some(&s) = stmts.get(i) {
            let next = stmts.get(i + 1).copied();
            match self.body.expr(s) {
                Expr::Name(head) if self.is_clause(head, s, next) => {
                    let head = head.clone();
                    self.visited.insert(s);
                    i += 1;
                    // The value: what follows on the head's line. A block is
                    // the clause's body, walked as the next statement.
                    while let Some(&v) = stmts.get(i) {
                        if !self.same_line(s, v) || matches!(self.body.expr(v), Expr::Block { .. })
                        {
                            break;
                        }
                        self.clause_value(&head, v);
                        i += 1;
                    }
                    continue;
                }
                // `release(h) { .. }`: the parameters bind in the block.
                Expr::Call { callee, args }
                    if matches!(self.body.expr(*callee), Expr::Name(h)
                        if matches!(crate::policy::domain_of(h), Some(crate::policy::Domain::Body)))
                        && next
                            .is_some_and(|n| matches!(self.body.expr(n), Expr::Block { .. })) =>
                {
                    let (callee, args) = (*callee, args.clone());
                    self.visited.insert(s);
                    self.visited.insert(callee);
                    let mut names = BTreeSet::new();
                    for a in args {
                        match self.body.expr(a.value) {
                            Expr::Name(n) => {
                                self.visited.insert(a.value);
                                names.insert(n.clone());
                            }
                            _ => self.expr(a.value),
                        }
                    }
                    let block = next.expect("matched above");
                    self.scoped(names, |w| w.expr(block));
                    i += 2;
                    continue;
                }
                _ => {}
            }
            self.expr(s);
            i += 1;
        }
    }

    /// Is the statement `s`, the bare name `head`, a clause: a policy head
    /// with a value on its line or a block after it, or a UI section such as
    /// a page's `view { .. }`? A name some scope binds is a use, whatever its
    /// spelling.
    fn is_clause(&self, head: &str, s: ExprId, next: Option<ExprId>) -> bool {
        let Some(next) = next else { return false };
        if self.bound(head) {
            return false;
        }
        let block_follows = matches!(self.body.expr(next), Expr::Block { .. });
        if pw_syntax::UI_NOUNS.contains(&head) {
            return block_follows;
        }
        crate::policy::domain_of(head).is_some() && (block_follows || self.same_line(s, next))
    }

    /// A clause's value: code where the policy table says the clause carries
    /// terms; otherwise words of the clause's own domain, which the analysis
    /// that reads the clause judges (`scope application` is the scope
    /// graph's), and never names to resolve.
    fn clause_value(&mut self, head: &str, v: ExprId) {
        if crate::policy::carries_terms(head) {
            return self.expr(v);
        }
        for e in self.body.walk_from(v) {
            self.visited.insert(e);
        }
    }

    /// Do `a` and `b` start on one line: is there no line break between the
    /// end of `a` and the start of `b`?
    fn same_line(&self, a: ExprId, b: ExprId) -> bool {
        let (a, b) = (self.body.expr_span(a), self.body.expr_span(b));
        self.src
            .get(a.end..b.start)
            .is_some_and(|between| !between.contains('\n'))
    }

    /// `a.b.c` used as a value: its head must resolve, and a head that names
    /// a module must have the member.
    fn path(&mut self, id: ExprId) {
        let mut head = id;
        let mut members = Vec::new();
        while let Expr::Field { base, name } = self.body.expr(head) {
            members.push(name.clone());
            head = *base;
        }
        let Expr::Name(h) = self.body.expr(head) else {
            // `f(x).y`: the base is a value computed some other way.
            return self.expr(head);
        };
        let h = h.clone();
        if self.bound(&h) || EFFECT_HEADS.contains(&h.as_str()) {
            return;
        }
        if self.workspace.sees_module(self.unit, &h) {
            let member = members.last().expect("a field has a member");
            let path = format!("{h}.{member}");
            if matches!(
                self.workspace.resolve_path(self.unit, &path),
                Resolution::Unresolved
            ) && !self.constructors.contains_key(member)
            {
                self.found.push((self.body.expr_span(id), path));
            }
            return;
        }
        self.name(&h, self.body.expr_span(head));
    }

    /// `x.method(..)` and `Module.f(..)`: the qualified call is
    /// `check::unresolved_uses`'s, but a receiver no scope binds is a use of
    /// a name like any other.
    fn callee_path(&mut self, id: ExprId) {
        let mut head = id;
        while let Expr::Field { base, .. } = self.body.expr(head) {
            head = *base;
        }
        match self.body.expr(head) {
            Expr::Name(h) => {
                let h = h.clone();
                let module_shaped = h.chars().next().is_some_and(char::is_uppercase)
                    || EFFECT_HEADS.contains(&h.as_str())
                    || self.workspace.sees_module(self.unit, &h);
                if !module_shaped {
                    self.name(&h, self.body.expr_span(head));
                }
            }
            _ => self.expr(head),
        }
    }

    fn node(&mut self, id: NodeId) {
        match self.body.node(id).clone() {
            Node::Element {
                tag,
                attrs,
                children,
                ..
            } => {
                // `<ready as={items}>` and `<failed as={e}>`, a `<stream>`'s
                // parts, bind the name for their children (as `marko.rs`
                // reads them).
                let mut scope = BTreeSet::new();
                for a in attrs {
                    let AttrValue::Expr(e) = a.value else {
                        continue;
                    };
                    if a.name == "as"
                        && matches!(tag.as_str(), "ready" | "failed")
                        && let Expr::Name(n) = self.body.expr(e)
                    {
                        scope.insert(n.clone());
                        self.visited.insert(e);
                        continue;
                    }
                    self.expr(e);
                }
                self.scoped(scope, |w| {
                    for c in children {
                        w.node(c);
                    }
                });
            }
            Node::Interpolation(e) => self.expr(e),
            Node::Block {
                directive,
                children,
                subject,
                ..
            } => {
                if let Some(s) = subject {
                    self.expr(s);
                }
                let mut scope = BTreeSet::new();
                if let Some((collection, binding)) = each(&directive) {
                    // A list written as a path starts with a name. A computed
                    // one, `same(xs)`, is not a name: until 2026-09-26 it was
                    // reported as one that does not resolve, and the build now
                    // refuses it for what it is (ADR-0074).
                    let segments: Vec<&str> = collection.split('.').map(str::trim).collect();
                    let written = segments.iter().all(|s| {
                        s.starts_with(|c: char| c.is_alphabetic() || c == '_')
                            && s.chars().all(|c| c.is_alphanumeric() || c == '_')
                    });
                    let head = segments[0];
                    if written && !self.bound(head) && !self.resolves(head) {
                        self.found.push((self.body.node_span(id), head.to_string()));
                    }
                    scope.insert(binding);
                }
                self.scoped(scope, |w| {
                    // Each `{:Some(x)}` arm binds for the nodes after it, up
                    // to the next marker; a condition is read in the block's
                    // scope, not the arm before it.
                    let mut in_arm = false;
                    for c in children {
                        if let Node::Branch {
                            condition,
                            arm: case,
                            ..
                        } = w.body.node(c).clone()
                        {
                            if in_arm {
                                w.scopes.pop();
                            }
                            if let Some(e) = condition {
                                w.expr(e);
                            }
                            w.scopes
                                .push(case.into_iter().flat_map(|a| a.bindings).collect());
                            in_arm = true;
                            continue;
                        }
                        w.node(c);
                    }
                    if in_arm {
                        w.scopes.pop();
                    }
                });
            }
            Node::Branch { condition, .. } => {
                if let Some(e) = condition {
                    self.expr(e);
                }
            }
            Node::Text(_) => {}
        }
    }
}

/// `{#each xs as x (x.id)}` -> `("xs", "x")`.
fn each(directive: &str) -> Option<(String, String)> {
    let inner = directive.strip_prefix("{#each")?.strip_suffix('}')?.trim();
    let (collection, rest) = inner.split_once(" as ")?;
    let binding = rest.split(['(', ' ']).next()?.trim();
    (!binding.is_empty()).then(|| (collection.trim().to_string(), binding.to_string()))
}

/// `x = e` where `x` is not `let mut` (PW0611, ADR-0051).
fn immutable_target(hir: &Hir, id: DeclId, decl: &Decl, span: Span, name: &str) -> Diagnostic {
    Diagnostic {
        code: crate::codes::ASSIGN_IMMUTABLE.id,
        invariant: crate::codes::ASSIGN_IMMUTABLE.invariant,
        reason: "assignment_to_immutable_binding",
        detector: Detector::DeclarationRule,
        severity: Severity::Error,
        message: format!("`{name}` is assigned, and it is not a `let mut` binding"),
        primary_span: span,
        related: vec![Related {
            span: hir.decl_span(id),
            label: format!("in `{}`", decl.name),
        }],
        explanation: Some(
            "a binding holds one value unless it is declared `let mut`; a parameter, a \
             pattern's name and a declaration are never assigned"
                .to_string(),
        ),
        repairs: vec![Repair {
            description: format!("declare it `let mut {name} = ..`, or bind a new name"),
            replacement: None,
        }],
    }
}

/// **PW0022**: a bare case that more than one visible type declares
/// (ADR-0059).
fn ambiguous_case(
    hir: &Hir,
    id: DeclId,
    decl: &Decl,
    span: Span,
    name: &str,
    types: &[String],
) -> Diagnostic {
    let qualified: Vec<String> = types.iter().map(|t| format!("`{t}.{name}`")).collect();
    Diagnostic {
        code: crate::codes::AMBIGUOUS_NAME.id,
        invariant: crate::codes::AMBIGUOUS_NAME.invariant,
        reason: "ambiguous_case",
        detector: Detector::DeclarationRule,
        severity: Severity::Error,
        message: format!(
            "`{name}` is a case of {}",
            types
                .iter()
                .map(|t| format!("`{t}`"))
                .collect::<Vec<_>>()
                .join(" and ")
        ),
        primary_span: span,
        related: vec![Related {
            span: hir.decl_span(id),
            label: format!("used inside `{}`", decl.name),
        }],
        explanation: Some(format!(
            "A case written alone is the case of that name of the one sum type in \
             scope that declares it. Here {} types declare `{name}`, and nothing \
             says which is meant.",
            types.len()
        )),
        repairs: vec![Repair {
            description: format!("write it through its type: {}", qualified.join(" or ")),
            replacement: None,
        }],
    }
}

fn diagnostic(hir: &Hir, id: DeclId, decl: &Decl, span: Span, name: &str) -> Diagnostic {
    Diagnostic {
        code: crate::codes::UNRESOLVED_NAME.id,
        invariant: crate::codes::UNRESOLVED_NAME.invariant,
        reason: "unresolved_name",
        detector: Detector::DeclarationRule,
        severity: Severity::Error,
        message: format!("`{name}` does not resolve"),
        primary_span: span,
        related: vec![Related {
            span: hir.decl_span(id),
            label: format!("used inside `{}`", decl.name),
        }],
        explanation: Some(format!(
            "Nothing in scope here is called `{name}`: no parameter, no `let` or \
             pattern before this use, no declaration of this module, and no \
             import. A name must come from lexical scope, the module, or an \
             explicit import."
        )),
        repairs: vec![Repair {
            description: format!("bind `{name}` before this use, declare it, or import it"),
            replacement: None,
        }],
    }
}
