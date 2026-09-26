//! Privacy labels as a property of **values**, not of names.
//!
//! # Why this replaced what was there
//!
//! Labels used to be a map from binding name to restriction, filled in by
//! looking at `let` initialisers. A counterexample broke it in one line:
//!
//! ```text
//! let token = secrets.payments()   // Secret<Payments>
//! let same  = token                // unlabelled  <- wrong
//! ```
//!
//! Iterating the `let` pass fixed that case and left the shape of the mistake
//! intact — the label was still attached to *how a value was reached* rather
//! than to the value. A field of a secret, a branch that returns one, a record
//! built around one, an interpolation holding one: each would have needed its
//! own patch, and the rule would have been about spelling.
//!
//! Architect ruling, 2026-08-06:
//!
//! > The sink examines the value's inferred label, not how the expression is
//! > spelled.
//!
//! # The lattice
//!
//! [`crate::privacy::Label`] is a **set of restrictions**, joined by union
//! (charter §7.8: labels are not a total order, so this is not a max). That
//! gives the branch behaviour directly:
//!
//! ```text
//! Public         ⊔ Session<S>  =  Session<S>
//! Session<S>     ⊔ Secret<C>   =  {Session<S>, Secret<C>}
//! ```
//!
//! A join never loses a restriction, so a value that is secret on one path is
//! secret at the merge.
//!
//! # Where a label comes from
//!
//! Only from declarations: a signature's return type (`Secret<Payments>`), a
//! parameter's written type, or a declaration's scope modifier. Never from a
//! function's name. That is E2C's rule and the reason the checker has no table
//! of accessors.
//!
//! # Where a binding's label comes from
//!
//! A binding is labelled by where it is bound, never by its name (ADR-0063):
//! a `let` by its initialiser, a match arm's names by the scrutinee, and a
//! name bound over the elements of a collection by the collection. A `for`
//! loop's name, a `{#each}` block's, and a lambda's parameters where the
//! lambda is passed to a call, whose parameters take the call's other
//! arguments' labels: an element of a labelled list carries the list's label.
//! Until 2026-09-26 those were unlabelled, and `for t in tokens {
//! log.public("{t}") }` over a list of secrets passed `pw check`; and the
//! bindings were keyed by name, so a name bound twice carried its first
//! binding's label at both.
//!
//! # Through a call
//!
//! A declared call's result is the declaration's label, joined with the
//! label of each argument whose declared type mentions a type parameter the
//! result mentions (ADR-0064): `List.get(xs, 0)` over a list of secrets is
//! secret, and `List.length(xs)` is what `length` declares. A lambda is
//! labelled by what it computes, and a call no declaration answers by all
//! that goes into it, its receiver included.
//!
//! # What it does not do
//!
//! Track an implicit flow: an element chosen by a secret index is its list's
//! label, not the index's. A label is a value's (charter §7.8).

use std::collections::BTreeMap;

use crate::hir::{Body, Decl, Expr, ExprId, Node, Pattern as HPat, PatternId, Span};
use crate::lexical::Binder;
use crate::privacy::Label;
use crate::signatures::Signatures;

/// Labels for the values in one body.
pub struct Labels<'a> {
    sigs: &'a Signatures,
    /// The module this body is in, and the modules it imports. What an
    /// unqualified name may resolve to, and nothing wider.
    module: Option<&'a str>,
    types: crate::infer::Types<'a>,
    /// Each binding's label and where it acquired one, by where it is bound
    /// (ADR-0063): two bindings of one name are two entries.
    bindings: BTreeMap<Binder, (Label, Span)>,
    /// The value each `|>` feeds, keyed by the call on its right: that
    /// call's first argument.
    piped: BTreeMap<ExprId, ExprId>,
}

impl<'a> Labels<'a> {
    /// `module` and `imports` say what an unqualified name may resolve to.
    ///
    /// Passed in rather than searched for, because "every module in the
    /// program" is the answer this replaced.
    pub fn of_body(
        sigs: &'a Signatures,
        decl: &Decl,
        body: &Body,
        module: Option<&'a str>,
        imports: &'a [String],
    ) -> Labels<'a> {
        let types = crate::infer::Types::of_body(sigs, decl, body, module);
        Labels::with(sigs, decl, body, module, imports, types, BTreeMap::new())
    }

    /// **The same, for the declaration `id` of `hir`, where it may be nested
    /// in another** (ADR-0066). A binding around a nested declaration carries
    /// the label it has in the enclosing declaration: a secret the enclosing
    /// body holds is a secret where the nested function reads it. Until
    /// 2026-09-26 it was unlabelled there, and a nested function logged it
    /// publicly with nothing reported.
    pub fn of_decl(
        sigs: &'a Signatures,
        hir: &'a crate::hir::Hir,
        id: crate::hir::DeclId,
        body: &Body,
        imports: &'a [String],
    ) -> Labels<'a> {
        let decl = hir.decl(id);
        let module = hir.module_of(id);
        let types = crate::infer::Types::of_decl(sigs, hir, id, body);
        let parent = crate::lexical::enclosing(hir, id).and_then(|p| {
            let b = hir.decl(p).body?;
            Some(Labels::of_decl(sigs, hir, p, hir.body(b), imports))
        });
        let mut outer = BTreeMap::new();
        for (i, (_, b)) in types.lexical().outer_bindings().enumerate() {
            if let Some(found) = parent.as_ref().and_then(|l| l.bindings.get(&b)) {
                outer.insert(Binder::Outer(i as u32), found.clone());
            }
        }
        Labels::with(sigs, decl, body, module, imports, types, outer)
    }

    fn with(
        sigs: &'a Signatures,
        decl: &Decl,
        body: &Body,
        module: Option<&'a str>,
        _imports: &'a [String],
        types: crate::infer::Types<'a>,
        outer: BTreeMap<Binder, (Label, Span)>,
    ) -> Labels<'a> {
        let mut me = Labels {
            sigs,
            module,
            types,
            bindings: outer,
            piped: BTreeMap::new(),
        };

        // A label comes from the resolved annotation in its real lexical
        // context. A same-spelled type in another module is not a qualifier.
        for (i, p) in decl.params.iter().enumerate() {
            if let Some(written) = &p.ty
                && let Some(ty) = sigs
                    .resolve_type(module, decl, written, p.span.clone())
                    .resolved()
            {
                let label = sigs.label(ty);
                if !label.is_public() {
                    me.bindings
                        .insert(Binder::Param(i), (label, p.span.clone()));
                }
            }
        }
        for id in body.walk() {
            if let Expr::Binary {
                op: crate::hir::BinOp::Pipe,
                lhs,
                rhs,
            } = body.expr(id)
            {
                me.piped.insert(*rhs, *lhs);
            }
        }
        let piped = me.piped.clone();

        // Bindings, to a fixed point. A binding's label is the label of its
        // initialiser, and an initialiser may mention an earlier binding, so
        // one pass in source order is not enough for every shape — a `use`
        // block or a nested block can bind out of order.
        for _ in 0..6 {
            let before = me.bindings.len();
            for id in body.walk() {
                let (binder, init, span) = match body.expr(id) {
                    Expr::Let {
                        pat: Some(pat),
                        init: Some(init),
                        ty,
                    } => {
                        let HPat::Bind { .. } = body.pat(*pat) else {
                            continue;
                        };
                        let binder = Binder::Pattern(*pat);
                        if let Some(written) =
                            ty.and_then(|t| crate::resolved::written_in_body(body, t))
                            && let Some(resolved) = sigs
                                .resolve_type(module, decl, &written, body.expr_span(id))
                                .resolved()
                        {
                            let label = sigs.label(resolved);
                            if !label.is_public() {
                                me.bindings.insert(binder, (label, body.expr_span(id)));
                                continue;
                            }
                        }
                        (binder, *init, body.expr_span(id))
                    }
                    Expr::Keyword {
                        keyword,
                        modifiers,
                        args,
                        ..
                    } if keyword == "use" => match (modifiers.first(), args.first()) {
                        (Some(_), Some(init)) => (Binder::Use(id), *init, body.expr_span(id)),
                        _ => continue,
                    },
                    _ => continue,
                };
                if me.bindings.contains_key(&binder) {
                    continue;
                }
                let l = me.label(body, init);
                if !l.is_public() {
                    me.bindings.insert(binder, (l, span));
                }
            }

            // A pattern binding inherits its scrutinee's label: matching on a
            // secret and naming the payload does not make the payload public.
            //
            // And a name bound over a collection's elements inherits the
            // collection's label: a `for` loop's, and a lambda's parameters
            // where it is passed to a call, which inherit the call's other
            // arguments' (ADR-0063).
            for id in body.walk() {
                let (from, pats): (Label, Vec<PatternId>) = match body.expr(id) {
                    Expr::Match { scrutinee, arms } => (
                        me.label(body, *scrutinee),
                        arms.iter().map(|a| a.pat).collect(),
                    ),
                    Expr::For {
                        pat: Some(p),
                        iterable,
                        ..
                    } => (me.label(body, *iterable), vec![*p]),
                    Expr::Call { args, .. } => {
                        for (i, a) in args.iter().enumerate() {
                            let Expr::Lambda { params, .. } = body.expr(a.value) else {
                                continue;
                            };
                            let l = args
                                .iter()
                                .enumerate()
                                .filter(|(j, _)| *j != i)
                                .map(|(_, o)| o.value)
                                .chain(piped.get(&id).copied())
                                .fold(Label::public(), |acc, v| acc.join(&me.label(body, v)));
                            if l.is_public() {
                                continue;
                            }
                            for p in params {
                                for (q, span) in bound_binders(body, *p) {
                                    me.bindings
                                        .entry(Binder::Pattern(q))
                                        .or_insert((l.clone(), span));
                                }
                            }
                        }
                        continue;
                    }
                    _ => continue,
                };
                if from.is_public() {
                    continue;
                }
                for p in pats {
                    for (q, span) in bound_binders(body, p) {
                        me.bindings
                            .entry(Binder::Pattern(q))
                            .or_insert((from.clone(), span));
                    }
                }
            }

            // A `release(h) { .. }` clause's `h` carries what its resource
            // acquired.
            let released: Vec<_> = me.types.lexical().released().collect();
            for (clause, acquired) in released {
                let l = me.label(body, acquired);
                if !l.is_public() {
                    me.bindings
                        .entry(Binder::Clause(clause, 0))
                        .or_insert((l, body.expr_span(clause)));
                }
            }

            // A stream's parts carry its query's label (ADR-0066).
            let parts: Vec<_> = me.types.lexical().stream_parts().collect();
            for (n, query, _) in parts {
                let l = me.label(body, query);
                if !l.is_public() {
                    me.bindings
                        .entry(Binder::Stream(n))
                        .or_insert((l, body.node_span(n)));
                }
            }

            // A template's names: an `{#each}` block's by its collection, a
            // `{#match}` arm's by the block's subject.
            let eaches: Vec<_> = me
                .types
                .lexical()
                .each_blocks()
                .map(|(n, c, h)| (n, c.to_string(), h))
                .collect();
            for (n, collection, head) in eaches {
                let l = me.collection_label(&collection, head);
                if !l.is_public() {
                    me.bindings
                        .entry(Binder::Each(n))
                        .or_insert((l, body.node_span(n)));
                }
            }
            let arms: Vec<_> = me.types.lexical().template_arms().collect();
            for (n, subject) in arms {
                let l = me.label(body, subject);
                let Node::Branch { arm: Some(arm), .. } = body.node(n) else {
                    continue;
                };
                if l.is_public() {
                    continue;
                }
                for i in 0..arm.bindings.len() {
                    me.bindings
                        .entry(Binder::Arm(n, i))
                        .or_insert((l.clone(), body.node_span(n)));
                }
            }

            if me.bindings.len() == before {
                break;
            }
        }
        me
    }

    /// A declaration by its bare name, WITHIN THIS MODULE AND ITS IMPORTS.
    ///
    /// `query Cart(session)` names `Cart`, not `cart.queries.Cart` — the
    /// dependency is on the declaration, and the module it lives in is what
    /// the import resolved.
    ///
    /// It used to search every module in the program, returning `None` on
    /// ambiguity. Unique-or-nothing is safer than picking one, but it still
    /// answered with a declaration from a module this unit never imported —
    /// and a unique wrong answer is harder to notice than a contested one.
    /// Classified `forbidden` in `last-segment-audit.txt` and repaired here.
    ///
    /// The qualified path is tried first, because a module's own declaration
    /// is what an unqualified name means inside it.
    fn declaration_named(&self, name: &str) -> Option<&crate::signatures::Signature> {
        self.sigs.in_module(self.module, name)
    }

    /// Where the binding a name means where `use_` writes it acquired its
    /// label, for the diagnostic's origin span.
    pub fn origin(&self, use_: ExprId) -> Option<&(Label, Span)> {
        self.bindings.get(&self.types.lexical().binder(use_)?)
    }

    /// The label of a collection a `{#each}` directive names, as written: its
    /// first name's binding's, and each field read from it on top, as a
    /// field read is labelled.
    fn collection_label(&self, collection: &str, head: Option<Binder>) -> Label {
        let Some(b) = head else {
            return Label::public();
        };
        let mut l = self
            .bindings
            .get(&b)
            .map(|(l, _)| l.clone())
            .unwrap_or_else(Label::public);
        let mut ty = self.types.binding(b).cloned();
        for field in collection.split('.').skip(1) {
            let Some(sig) = ty
                .as_ref()
                .and_then(|t| self.sigs.member_of(t, field.trim()))
            else {
                break;
            };
            l = l.join(&sig.label);
            ty = sig.result().cloned();
        }
        l
    }

    /// The label of a value.
    ///
    /// Every construct that can carry a value forward joins the labels of what
    /// it carries. A restriction is never dropped, so this errs toward saying a
    /// value is private.
    pub fn label(&self, body: &Body, id: ExprId) -> Label {
        match body.expr(id) {
            // The binding the name means here (ADR-0063). A declaration
            // named as a value is labelled by what calling it makes, as a
            // lambda is by its body (ADR-0079): `let f = secrets.payments`
            // holds a secret's source. Until 2026-09-26 it was public, and
            // `f()` was too.
            Expr::Name(n) => match self.types.lexical().binder(id) {
                Some(b) => self
                    .bindings
                    .get(&b)
                    .map(|(l, _)| l.clone())
                    .unwrap_or_else(Label::public),
                None => self
                    .declaration_named(n)
                    .map(|s| s.label.clone())
                    .unwrap_or_else(Label::public),
            },

            // A field of a secret record is secret. The record's own label is
            // the floor; a field with its own declared label joins on top.
            Expr::Field { base, name } => {
                // `secrets.payments` named as a value: a declaration, as a
                // name is (ADR-0079).
                if let Some(sig) = self.declaration_named(&crate::infer::path_of(body, id)) {
                    return sig.label.clone();
                }
                let mut l = self.label(body, *base);
                if let Some(sig) = self
                    .types
                    .of(body, *base)
                    .and_then(|t| self.sigs.member_of(&t, name))
                {
                    l = l.join(&sig.label);
                }
                l
            }

            Expr::Call { callee, args } => {
                match self.types.callee(body, *callee) {
                    // Declared: its return label is the contract, joined with
                    // what it is given (ADR-0085). A parameter declared with a
                    // label of its own, `key: Secret<Payments>`, receives the
                    // value by that contract, and the declaration says what
                    // comes out: a receipt, not the key. Any other parameter
                    // says nothing about labels, and what the call makes
                    // carries what it was given, as the charter's data flow
                    // asks (§7.8). Until 2026-09-26 only an argument that
                    // brought in a type parameter the result mentions was
                    // carried (ADR-0064), so `String.trim` over a secret
                    // string made a public one.
                    Some(sig) => {
                        let mut l = sig.label.clone();
                        // What each parameter is given: a piped value, or a
                        // method call's receiver, first.
                        let first = self.piped.get(&id).copied().or(match body.expr(*callee) {
                            Expr::Field { base, .. }
                                if self
                                    .declaration_named(&crate::infer::path_of(body, *callee))
                                    .is_none() =>
                            {
                                Some(*base)
                            }
                            _ => None,
                        });
                        // Each argument, to the parameter it is given to: a
                        // named one to the parameter of its name (ADR-0081).
                        // A call whose names do not arrange is PW0617's, and
                        // carries every argument's label meanwhile.
                        let leading = usize::from(first.is_some());
                        let written: Vec<Option<&str>> =
                            args.iter().map(|a| a.name.as_deref()).collect();
                        let Ok(order) = crate::signatures::arrange(&sig.names, leading, &written)
                        else {
                            return args
                                .iter()
                                .map(|a| a.value)
                                .chain(first)
                                .fold(l, |acc, v| acc.join(&self.label(body, v)));
                        };
                        let mut given: Vec<Option<ExprId>> = vec![None; sig.params.len()];
                        if let (Some(f), Some(slot)) = (first, given.get_mut(0)) {
                            *slot = Some(f);
                        }
                        for (a, i) in args.iter().zip(order) {
                            if let Some(slot) = given.get_mut(i) {
                                *slot = Some(a.value);
                            }
                        }
                        for (param, value) in sig.params.iter().zip(given) {
                            let Some(value) = value else { continue };
                            let states_a_label = param
                                .as_ref()
                                .and_then(crate::resolved::TypeResolution::resolved)
                                .is_some_and(|t| !self.sigs.label(t).is_public());
                            if !states_a_label {
                                l = l.join(&self.label(body, value));
                            }
                        }
                        l
                    }
                    // Undeclared: conservative. A call whose contract this
                    // program does not state carries whatever went into it,
                    // which is what stops `wrap(token)` from laundering: its
                    // arguments, a piped value, and a method call's receiver.
                    // The callee was left out until 2026-09-26: `f()`, where `f`
                    // holds `secrets.payments`, and `b.f()` through a record
                    // field holding it, made a public value (ADR-0079). A
                    // method call's callee, `tokens.get`, carries its
                    // receiver's label, which ADR-0064 joined by hand.
                    None => args
                        .iter()
                        .map(|a| a.value)
                        .chain(self.piped.get(&id).copied())
                        .chain(std::iter::once(*callee))
                        .fold(Label::public(), |acc, v| acc.join(&self.label(body, v))),
                }
            }

            // Branch join, per charter §7.8: union, not max.
            Expr::If { then, els, .. } => {
                let l = self.label(body, *then);
                match els {
                    Some(e) => l.join(&self.label(body, *e)),
                    None => l,
                }
            }
            Expr::Match { arms, .. } => arms.iter().fold(Label::public(), |acc, a| {
                acc.join(&self.label(body, a.body))
            }),

            // `x |> f(..)` is the call, with `x` its first argument: labelled
            // as the call labels what it is given (ADR-0085), so a secret
            // piped to a parameter declared to receive it keeps the call's
            // contract, as it does passed in place.
            Expr::Binary {
                op: crate::hir::BinOp::Pipe,
                rhs,
                ..
            } if matches!(body.expr(*rhs), Expr::Call { .. }) => self.label(body, *rhs),
            Expr::Binary { lhs, rhs, .. } => self.label(body, *lhs).join(&self.label(body, *rhs)),
            Expr::Unary { operand, .. } => self.label(body, *operand),

            // A string carrying a secret in a hole IS the secret.
            Expr::Interpolated { parts, .. } => parts
                .iter()
                .fold(Label::public(), |acc, p| acc.join(&self.label(body, *p))),

            // Aggregates carry what is put into them.
            Expr::Record { fields, .. } => {
                fields.iter().fold(Label::public(), |acc, f| match f.value {
                    Some(v) => acc.join(&self.label(body, v)),
                    None => acc,
                })
            }
            Expr::List { items } => items
                .iter()
                .fold(Label::public(), |acc, i| acc.join(&self.label(body, *i))),

            // A block's value is its last statement's.
            Expr::Block { stmts } => stmts
                .last()
                .map(|s| self.label(body, *s))
                .unwrap_or_else(Label::public),

            Expr::Cast { value, .. } => self.label(body, *value),
            // A function is labelled by what it computes: `t => "{t}"`,
            // mapped over secrets, makes secrets (ADR-0064).
            Expr::Lambda { body: inner, .. } => self.label(body, *inner),
            // The success value carries what the whole value carried.
            Expr::Try { value } => self.label(body, *value),

            // `query Cart(session)` DECLARES a dependency. The query runs at
            // its own placement, so its effects are not this body's — but its
            // result is this body's value, so its LABEL is. Splitting those
            // two is the whole point of the declaration: a page may depend on
            // a session-scoped query without itself reaching the database.
            Expr::Keyword {
                keyword, modifiers, ..
            } if matches!(keyword.as_str(), "query" | "command" | "subscription") => modifiers
                .first()
                .and_then(|name| self.declaration_named(name))
                .map(|s| s.label.clone())
                .unwrap_or_else(Label::public),

            _ => Label::public(),
        }
    }
}

/// Where a pattern binds each of its names, with their spans.
fn bound_binders(body: &Body, pat: PatternId) -> Vec<(PatternId, Span)> {
    let mut out = Vec::new();
    let mut stack = vec![pat];
    while let Some(p) = stack.pop() {
        match body.pat(p) {
            HPat::Bind { .. } => out.push((p, body.pat_span(p))),
            HPat::Ctor { args, .. } => stack.extend(args.iter().copied()),
            HPat::Or(alts) => stack.extend(alts.iter().copied()),
            _ => {}
        }
    }
    out
}

/// The names a pattern binds, with their spans.
pub(crate) fn bound_names(body: &Body, pat: crate::hir::PatternId) -> Vec<(String, Span)> {
    let mut out = Vec::new();
    let mut stack = vec![pat];
    while let Some(p) = stack.pop() {
        match body.pat(p) {
            HPat::Bind { name, .. } => out.push((name.clone(), body.pat_span(p))),
            HPat::Ctor { args, .. } => stack.extend(args.iter().copied()),
            HPat::Or(alts) => stack.extend(alts.iter().copied()),
            _ => {}
        }
    }
    out
}

/// The modules a unit imports, for scoping unqualified names.
///
/// An `import` declaration's own name is the module it names; the `imports`
/// field beside it is the list of symbols taken from it, which is a different
/// question.
pub fn imported_modules(hir: &crate::hir::Hir) -> Vec<String> {
    let mut out: Vec<String> = hir
        .all_decls()
        .filter(|(_, d)| d.kind == crate::hir::DeclKind::Import)
        .map(|(_, d)| d.name.clone())
        .collect();
    out.sort();
    out.dedup();
    out
}
