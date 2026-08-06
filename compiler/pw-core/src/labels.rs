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
//! # What it does not do
//!
//! Track a label *into* a collection and back out — `List.push(xs, secret)`
//! then `List.first(xs)` loses it, because the element type is not inferred.
//! A witness for that belongs in `examples/generality/` when someone writes
//! one; it is not claimed here.

use std::collections::BTreeMap;

use crate::hir::{Body, Decl, Expr, ExprId, Pattern as HPat, Span};
use crate::privacy::{Label, Restriction};
use crate::signatures::Signatures;

/// Labels for the values in one body.
pub struct Labels<'a> {
    sigs: &'a Signatures,
    /// Binding name → its label and where it acquired one.
    bindings: BTreeMap<String, (Label, Span)>,
}

impl<'a> Labels<'a> {
    pub fn of_body(sigs: &'a Signatures, decl: &Decl, body: &Body) -> Labels<'a> {
        let mut me = Labels {
            sigs,
            bindings: BTreeMap::new(),
        };

        // A parameter whose written type carries a restriction.
        for p in &decl.params {
            if let Some(ty) = &p.ty
                && let Some(l) = label_of_type(ty, &[])
            {
                me.bindings.insert(p.name.clone(), (l, p.span.clone()));
            }
        }

        // Bindings, to a fixed point. A binding's label is the label of its
        // initialiser, and an initialiser may mention an earlier binding, so
        // one pass in source order is not enough for every shape — a `use`
        // block or a nested block can bind out of order.
        for _ in 0..6 {
            let before = me.bindings.len();
            for id in body.walk() {
                let (name, init, span) = match body.expr(id) {
                    Expr::Let {
                        pat: Some(pat),
                        init: Some(init),
                        ty,
                    } => {
                        let HPat::Bind { name, .. } = body.pat(*pat) else {
                            continue;
                        };
                        // A written annotation wins: `let k: Secret<Payments>`
                        // names the capability exactly.
                        if let Some(l) = ty
                            .and_then(|t| body.types.get(t.index()))
                            .and_then(|t| label_of_written(body, t))
                        {
                            me.bindings.insert(name.clone(), (l, body.expr_span(id)));
                            continue;
                        }
                        (name.clone(), *init, body.expr_span(id))
                    }
                    Expr::Keyword {
                        keyword,
                        modifiers,
                        args,
                        ..
                    } if keyword == "use" => match (modifiers.first(), args.first()) {
                        (Some(n), Some(init)) => (n.clone(), *init, body.expr_span(id)),
                        _ => continue,
                    },
                    _ => continue,
                };
                if me.bindings.contains_key(&name) {
                    continue;
                }
                let l = me.label(body, init);
                if !l.is_public() {
                    me.bindings.insert(name, (l, span));
                }
            }

            // A pattern binding inherits its scrutinee's label: matching on a
            // secret and naming the payload does not make the payload public.
            for id in body.walk() {
                let Expr::Match { scrutinee, arms } = body.expr(id) else {
                    continue;
                };
                let l = me.label(body, *scrutinee);
                if l.is_public() {
                    continue;
                }
                for arm in arms {
                    for (name, span) in bound_names(body, arm.pat) {
                        me.bindings.entry(name).or_insert((l.clone(), span));
                    }
                }
            }

            if me.bindings.len() == before {
                break;
            }
        }
        me
    }

    /// A declaration by its bare name, across every module.
    ///
    /// `query Cart(session)` names `Cart`, not `cart.queries.Cart` — the
    /// dependency is on the declaration, and the module it lives in is what
    /// the import resolved. Ambiguity here resolves to nothing rather than to
    /// a guess.
    fn declaration_named(&self, name: &str) -> Option<&crate::signatures::Signature> {
        let mut found = None;
        for (path, sig) in self.sigs.iter() {
            if path.rsplit('.').next() == Some(name) {
                if found.is_some() {
                    return None;
                }
                found = Some(sig);
            }
        }
        found
    }

    /// Where a binding acquired its label, for the diagnostic's origin span.
    pub fn origin(&self, name: &str) -> Option<&(Label, Span)> {
        self.bindings.get(name)
    }

    /// The label of a value.
    ///
    /// Every construct that can carry a value forward joins the labels of what
    /// it carries. A restriction is never dropped, so this errs toward saying a
    /// value is private.
    pub fn label(&self, body: &Body, id: ExprId) -> Label {
        match body.expr(id) {
            Expr::Name(n) => self
                .bindings
                .get(n)
                .map(|(l, _)| l.clone())
                .unwrap_or_else(Label::public),

            // A field of a secret record is secret. The record's own label is
            // the floor; a field with its own declared label joins on top.
            Expr::Field { base, name } => {
                let mut l = self.label(body, *base);
                if let Some(sig) = crate::infer::Types::of_body(self.sigs, &dummy(), body, None)
                    .of(body, *base)
                    .and_then(|t| self.sigs.member_of(&t, name))
                {
                    l = l.join(&sig.label);
                }
                l
            }

            Expr::Call { callee, args } => {
                let path = crate::infer::path_of(body, *callee);
                match self.sigs.by_path(&path) {
                    // Declared: its return label is the contract. E2C's whole
                    // model is that one declaration answers this, so a helper
                    // that launders a secret is a library-contract defect and
                    // not something to second-guess here.
                    Some(sig) => sig.label.clone(),
                    // Undeclared: conservative. A call whose contract this
                    // program does not state carries whatever went into it,
                    // which is what stops `wrap(token)` from laundering.
                    None => args.iter().fold(Label::public(), |acc, a| {
                        acc.join(&self.label(body, a.value))
                    }),
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

/// The names a pattern binds, with their spans.
fn bound_names(body: &Body, pat: crate::hir::PatternId) -> Vec<(String, Span)> {
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

/// The restriction a written type name carries: `Secret<Payments>`.
fn label_of_type(head: &str, args: &[String]) -> Option<Label> {
    let arg = || args.first().cloned().unwrap_or_else(|| "?".to_string());
    Some(match head {
        "Secret" => Label::of(Restriction::Secret(arg())),
        "Session" => Label::of(Restriction::Session(arg())),
        "User" => Label::of(Restriction::User(arg())),
        "Organization" => Label::of(Restriction::Organization(arg())),
        _ => return None,
    })
}

fn label_of_written(body: &Body, t: &crate::hir::TypeRef) -> Option<Label> {
    let args: Vec<String> = t
        .args
        .iter()
        .filter_map(|a| body.types.get(a.index()).map(|t| t.path.clone()))
        .collect();
    label_of_type(&t.path, &args)
}

/// A declaration with no parameters, for the places that only need `Types` to
/// resolve a member chain and have no declaration of their own to offer.
fn dummy() -> Decl {
    Decl {
        name: String::new(),
        kind: crate::hir::DeclKind::Fn,
        params: vec![],
        ret: None,
        ret_args: vec![],
        variants: None,
        fields: None,
        policies: vec![],
        imports: vec![],
        visibility: None,
        opaque_of: None,
        declared_effects: None,
        body: None,
        children: vec![],
    }
}
