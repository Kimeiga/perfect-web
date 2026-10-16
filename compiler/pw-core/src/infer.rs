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
//! - a field access, from the signature of the member on the base's type;
//! - what `e?` takes out of `e`'s `Result` or `Option`, a block's last
//!   statement, and branches or arms that agree (ADR-0250).
//!
//! That chain is what makes `self.style.set_padding(4.px)` resolvable:
//! `self` is an `ElementRef`, `.style` is a member of `ElementRef` returning
//! `Style`, and `.set_padding` is a member of `Style`. No step asks whether a
//! name is unique.
//!
//! A lambda parameter is typed by the value solver where its callee's declared
//! function type says: `el` in `List.map(items, el => ..)` is an
//! element of `items`, because `map` takes a `List<T>` and a `fn(T) -> U`.
//!
//! # What it reads from the value relations' typer
//!
//! **A type it never answers: a call's uninstantiated callee parameter**
//! (ADR-XXXX, the generic-effects track). A generic call's
//! declared result names the callee's own parameters: `List.reverse(given)`
//! returns `List<T>`, and which `T` is the call's to say. Until ADR-XXXX this
//! module answered `List<T>` as written, so a member read through the
//! result, `x.label` over its elements, looked `label` up on `T`, found
//! nothing, and every analysis reading members through these types (the
//! effect inference, the labels, the resumed captures) saw nothing there: a
//! view declared `!{}` read the database unseen.
//!
//! So where its own rules, which read declarations, leave an expression with
//! no type, or with a callee's parameter in it, [`Types::of`] answers what the
//! value relations' typer (`values.rs`) solves: the call instantiated per
//! call, its parameters unified with its arguments' types, and the same for a
//! `for` loop's binder, a `match` arm's and a lambda's parameter over a list
//! a call computes, which this module's own rules never typed. One solver for
//! a call, not two that disagree (`values::solve_body`). What neither knows
//! is `None`, and it is each caller's decision whether to fall back or to
//! stop; a member read on such a value is refused (ADR-XXXX).

use std::collections::BTreeMap;

use crate::hir::{Body, Decl, DeclId, DeclKind, Expr, ExprId, Hir, Node};
use crate::lexical::{Binder, Lexical};
use crate::resolve::DefId;
use crate::resolved::{self, Builtin, ResolvedType};
use crate::signatures::Signatures;
use crate::values::{SolvedBody, Ty};

/// The types known inside one declaration's body.
pub struct Types<'a> {
    sigs: &'a Signatures,
    /// The binding each local name means (ADR-0063): the rule the value
    /// relations and the backend read names by.
    lexical: Lexical,
    /// Every binding with a type the program states, by where it is bound,
    /// never by its name: two bindings of one name are two entries. Until
    /// 2026-09-26 this was keyed by name, the last binding of a name won, and
    /// a handler's capture was typed by another binding of its name
    /// (ADR-0063).
    bindings: BTreeMap<Binder, ResolvedType>,
    /// `self`, in a UI declaration: its own element.
    self_type: Option<ResolvedType>,
    /// The module this body lives in, so a bare name resolves to a sibling
    /// declaration. Without it `shrink(self)` looked up `shrink` and found
    /// nothing, because signatures are stored module-qualified — so a rule
    /// following a helper stopped at the module boundary of its own file.
    module: Option<String>,
    /// **What the value relations' typer solves in this body** (ADR-XXXX):
    /// read where this module's own rules leave a hole. Solved once per
    /// check, and shared by every analysis's `Types` ([`Signatures::solved`]).
    solved: std::rc::Rc<SolvedBody>,
    /// The declarations whose type parameters are in scope here: this one,
    /// and each it is nested in. A parameter of any other declaration in a
    /// type is a callee's, unsolved, and never an answer.
    own: Vec<DefId>,
}

impl<'a> Types<'a> {
    /// Gather what this declaration's own text says about its bindings.
    /// The module is a **parameter**, not a builder step.
    ///
    /// It used to be `Types::of_body(..).in_module(m)`, and that made whether
    /// a rule could resolve a sibling declaration depend on whether its caller
    /// remembered a second method call. The `#each` rule below is the case
    /// that exposed it: it resolves `query Menu(..)` during construction, so
    /// with a post-hoc module it silently saw none and every loop binding came
    /// out untyped. Four of the seven callers had never called `in_module`.
    pub fn of_body(
        sigs: &'a Signatures,
        decl: &Decl,
        body: &Body,
        module: Option<&str>,
    ) -> Types<'a> {
        let lexical = Lexical::build(decl, body);
        let solved = solved(sigs, module, body, false, || {
            crate::values::solve_body(sigs, module, decl, body, lexical.clone(), Vec::new())
        });
        let own = own_binder(sigs, module, decl).into_iter().collect();
        Types::with(sigs, decl, body, module, lexical, Vec::new(), solved, own)
    }

    /// **The same, for the declaration `id` of `hir`, where it may be nested
    /// in another** (ADR-0066): the bindings around it are typed as its
    /// enclosing declaration types them.
    pub fn of_decl(sigs: &'a Signatures, hir: &Hir, id: DeclId, body: &Body) -> Types<'a> {
        let decl = hir.decl(id);
        let module = hir.module_of(id);
        let Some(lexical) = Lexical::build_in(hir, id) else {
            return Types::of_body(sigs, decl, body, module);
        };
        let parent = crate::lexical::enclosing(hir, id).and_then(|p| {
            let b = hir.decl(p).body?;
            Some(Types::of_decl(sigs, hir, p, hir.body(b)))
        });
        let outer = lexical
            .outer_bindings()
            .map(|(_, b)| parent.as_ref().and_then(|t| t.bindings.get(&b).cloned()))
            .collect();
        // The same bindings, as the value relations' typer typed them in the
        // enclosing declaration, for the one solving this body.
        let solved = solved(sigs, module, body, true, || {
            let around = lexical
                .outer_bindings()
                .map(|(_, b)| {
                    parent
                        .as_ref()
                        .map_or(Ty::Unknown, |t| t.solved.local(b).clone())
                })
                .collect();
            crate::values::solve_body(sigs, module, decl, body, lexical.clone(), around)
        });
        let mut own: Vec<DefId> = own_binder(sigs, module, decl).into_iter().collect();
        own.extend(parent.iter().flat_map(|t| t.own.iter().copied()));
        Types::with(sigs, decl, body, module, lexical, outer, solved, own)
    }

    #[allow(clippy::too_many_arguments)]
    fn with(
        sigs: &'a Signatures,
        decl: &Decl,
        body: &Body,
        module: Option<&str>,
        lexical: Lexical,
        outer: Vec<Option<ResolvedType>>,
        solved: std::rc::Rc<SolvedBody>,
        own: Vec<DefId>,
    ) -> Types<'a> {
        let mut bindings: BTreeMap<Binder, ResolvedType> = BTreeMap::new();
        for (i, t) in outer.into_iter().enumerate() {
            if let Some(t) = t {
                bindings.insert(Binder::Outer(i as u32), t);
            }
        }

        for (i, p) in decl.params.iter().enumerate() {
            if let Some(t) = &p.ty
                && let Some(ty) = sigs
                    .resolve_type(module, decl, t, p.span.clone())
                    .resolved()
            {
                bindings.insert(Binder::Param(i), ty.clone());
            }
        }

        // Charter §7.5A writes `self.style.set_padding(..)` inside a component.
        // `self` is the declaration's own element, and that is a language fact
        // about UI declarations rather than something a library declares.
        let self_type = match decl.kind {
            DeclKind::View | DeclKind::Component | DeclKind::Page | DeclKind::Layout => {
                sigs.language_type("browser", "ElementRef")
            }
            _ => None,
        };

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
            if let (crate::hir::Pattern::Bind { .. }, Some(t)) =
                (body.pat(*pat), resolved::written_in_body(body, *ty))
                && let Some(ty) = sigs.resolve_type(module, decl, &t, 0..0).resolved()
            {
                bindings.insert(Binder::Pattern(*pat), ty.clone());
            }
        }

        // **A page's query binding is the query's value** (ADR-0146): its
        // declared result's `Ok` value, as the host gives it and every
        // template read takes it. Until 2026-10-02 such a binding had no
        // type, so a `{#match}` over it, and a loop over its arm's binding,
        // typed nothing.
        for id in body.walk() {
            let Expr::Let {
                pat: Some(pat),
                ty: None,
                init: Some(init),
            } = body.expr(id)
            else {
                continue;
            };
            let Expr::Keyword {
                keyword, modifiers, ..
            } = body.expr(*init)
            else {
                continue;
            };
            if !matches!(keyword.as_str(), "query" | "subscription") {
                continue;
            }
            let Some(result) = modifiers
                .first()
                .and_then(|m| sigs.in_module(module, m))
                .and_then(|sig| sig.result())
            else {
                continue;
            };
            let value = match result.as_builtin() {
                Some(Builtin::Result) => result.args().first().cloned(),
                _ => Some(result.clone()),
            };
            if let (crate::hir::Pattern::Bind { .. }, Some(v)) = (body.pat(*pat), value) {
                bindings.entry(Binder::Pattern(*pat)).or_insert(v);
            }
        }

        // A lambda's written parameter type, `(e: InputEvent) => ..`, and an
        // `on:` handler's event where none is written: the record its event
        // gives it (ADR-0138).
        for (pat, ty) in &body.param_types {
            if let (crate::hir::Pattern::Bind { .. }, Some(t)) =
                (body.pat(*pat), resolved::written_in_body(body, *ty))
                && let Some(ty) = sigs.resolve_type(module, decl, &t, 0..0).resolved()
            {
                bindings.insert(Binder::Pattern(*pat), ty.clone());
            }
        }
        for (event, lambda) in crate::resume::handler_events(body) {
            let Expr::Lambda { params, .. } = body.expr(lambda) else {
                continue;
            };
            if let [p] = crate::values::lambda_params(body, params).as_slice()
                && let crate::hir::Pattern::Bind { .. } = body.pat(*p)
                && let Some(t) = crate::annotations::event_type(sigs, &event)
            {
                bindings
                    .entry(Binder::Pattern(*p))
                    .or_insert_with(|| t.clone());
            }
        }

        let mut types = Types {
            sigs,
            lexical,
            bindings,
            self_type,
            module: module.map(str::to_string),
            solved,
            own,
        };

        // A REAL type-propagation rule, not adapter knowledge:
        //
        //     List<MenuItem>
        //         | each
        //     item : MenuItem
        //
        // Without it a loop binding has no type, so a resumable handler inside
        // a loop has no capture schema and `PW5016` refuses to generate one.
        // The alternative — letting the adapter assume `item` "probably means
        // MenuItem" — would make serialization, nominal identity, privacy and
        // handler compatibility all rest on a guess.
        //
        // An E9 slice, pulled forward because E7 genuinely requires it:
        // milestone numbering must not force knowingly unsound semantics.
        //
        // A collection named by a binding: a parameter's, a `let`'s, a
        // stream part's. The binding is the one the name means where the
        // block is written.
        //
        // A stream's `<ready as={x}>` is its query's success, and
        // `<failed as={e}>` its failure (ADR-0066): `Some` of its declared
        // error, or `None` for the host's (ADR-0148).
        // A `release(h) { .. }` clause's `h` is what its resource's
        // `acquire { .. }` produced, its success where it can fail.
        let released: Vec<_> = types.lexical.released().collect();
        for (clause, acquired) in released {
            let last = match body.expr(acquired) {
                Expr::Block { stmts } => stmts.last().copied(),
                _ => Some(acquired),
            };
            let Some(t) = last.and_then(|e| types.of(body, e)) else {
                continue;
            };
            let value = match t.as_builtin() {
                Some(Builtin::Result) => t.args().first().cloned(),
                _ => Some(t),
            };
            if let Some(v) = value {
                types.bindings.insert(Binder::Clause(clause, 0), v);
            }
        }
        let parts: Vec<_> = types.lexical.stream_parts().collect();
        for (n, query, ok) in parts {
            let Some(t) = types.of(body, query) else {
                continue;
            };
            let part = match (t.as_builtin(), ok) {
                (Some(Builtin::Result), true) => t.args().first().cloned(),
                (Some(Builtin::Result), false) => {
                    t.args().get(1).cloned().map(ResolvedType::optional)
                }
                (_, true) => Some(t),
                (_, false) => None,
            };
            if let Some(p) = part {
                types.bindings.insert(Binder::Stream(n), p);
            }
        }
        // Loops and arms, typed together until nothing more is learned. A
        // loop's list can be an arm's binding, `{:Ok(items)} {#each items ..}`,
        // and an arm's subject a loop's item, `{#each orders as o} {#match
        // o.status}`, so neither order alone types both. Until 2026-10-02
        // loops were typed first, and an item over an arm's binding had no
        // type (PW5016).
        // A loop's list is a name, or a path inside its value, `cart.lines`
        // (ADR-0172). Until 2026-10-03 a path was not typed, and a handler
        // in its row could not be resumed (PW5016).
        let eaches: Vec<(crate::hir::NodeId, Option<Binder>, Vec<String>)> = types
            .lexical
            .each_blocks()
            .filter(|(_, collection, _)| !collection.contains('('))
            .map(|(n, collection, head)| {
                let fields = collection
                    .split('.')
                    .skip(1)
                    .map(|f| f.trim().to_string())
                    .collect();
                (n, head, fields)
            })
            .collect();
        let arms: Vec<(crate::hir::NodeId, ExprId)> = types.lexical.template_arms().collect();
        loop {
            let learned = types.bindings.len();
            for (n, head, fields) in &eaches {
                if types.bindings.contains_key(&Binder::Each(*n)) {
                    continue;
                }
                if let Some(elem) = head.and_then(|b| types.element_at(body, b, fields)) {
                    types.bindings.insert(Binder::Each(*n), elem);
                }
            }

            // The same rule for a `{#match}` arm (ADR-0042):
            //
            //     Option<Entry>          Result<T, E>
            //         | {:Some(e)}           | {:Ok(v)}  {:Err(x)}
            //     e : Entry              v : T     x : E
            for (n, s) in &arms {
                let (n, s) = (*n, *s);
                if types.bindings.contains_key(&Binder::Arm(n, 0)) {
                    continue;
                }
                let Node::Branch { arm: Some(arm), .. } = body.node(n) else {
                    continue;
                };
                let Some(ty) = types.of(body, s) else {
                    continue;
                };
                let payload = match (ty.as_builtin(), arm.short()) {
                    (Some(Builtin::Option), "Some") | (Some(Builtin::Result), "Ok") => {
                        ty.args().first()
                    }
                    (Some(Builtin::Result), "Err") => ty.args().get(1),
                    _ => None,
                };
                if let (Some(p), [_]) = (payload.cloned(), arm.bindings.as_slice()) {
                    types.bindings.insert(Binder::Arm(n, 0), p);
                    continue;
                }
                // A declared case's fields (ADR-0061), where the type has no
                // arguments a field could mention.
                if let Some(def) = ty.def_id()
                    && ty.args().is_empty()
                    && let Some((_, fields)) = sigs
                        .type_decl(def)
                        .and_then(|t| t.variants.as_ref())
                        .and_then(|cases| cases.iter().find(|(c, _)| c == arm.short()))
                    && fields.len() == arm.bindings.len()
                {
                    for (i, f) in fields.iter().enumerate() {
                        if let Some(t) = f.resolved() {
                            types.bindings.insert(Binder::Arm(n, i), t.clone());
                        }
                    }
                }
            }
            if types.bindings.len() == learned {
                break;
            }
        }

        // Callback parameters are read from the value solver below, along
        // with loop and match binders. Keeping the former list-only callback
        // approximation here created a second answer for a solved call.

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
                let crate::hir::Pattern::Bind { .. } = body.pat(*pat) else {
                    continue;
                };
                let b = Binder::Pattern(*pat);
                if types.bindings.contains_key(&b) {
                    continue;
                }
                if let Some(t) = types.of(body, *init) {
                    types.bindings.insert(b, t);
                    added = true;
                }
            }
            if !added {
                break;
            }
        }
        // **Every binding the value relations' typer solves** that these
        // rules left untyped (ADR-XXXX): a `for` loop's binder, a `match`
        // arm's, a lambda's parameter over a list a call computes. A name
        // reads its binding through [`Types::of`] either way; this is for a
        // reader of [`Types::binding`].
        let solved: Vec<(Binder, ResolvedType)> = types
            .solved
            .locals()
            .filter(|(b, _)| !types.bindings.contains_key(b))
            .filter_map(|(b, t)| Some((b, types.resolved_from(t, 0..0)?)))
            .collect();
        types.bindings.extend(solved);
        types
    }

    /// **`t`, where every part of it is known** (ADR-XXXX): a type the value
    /// relations' typer solved, as a resolved type, each declaration it
    /// names by its own name. `None` for one with a hole, a value of any
    /// type, or a type parameter, none of which a resolved type holds.
    fn resolved_from(&self, t: &Ty, span: crate::hir::Span) -> Option<ResolvedType> {
        let key = t.key()?;
        ResolvedType::inferred(
            &key,
            &|def| {
                self.sigs
                    .path_of(def)
                    .map(|p| p.rsplit('.').next().unwrap_or(p).to_string())
            },
            &span,
        )
    }

    /// **Does `t` name a type parameter no declaration in scope here binds**
    /// (ADR-XXXX): a callee's, which a call instantiates, and which as written
    /// is no type a value here has.
    fn unsolved(&self, t: &ResolvedType) -> bool {
        t.parameter_binding()
            .is_some_and(|(binder, _)| !self.own.contains(&binder))
            || t.args().iter().any(|a| self.unsolved(a))
    }

    /// **The element of a list inside a binding's value** (ADR-0172):
    /// `cart.lines`'s, the binding's value seen through its `Result` or its
    /// `Option`, then each field. With no field, the binding's own list.
    fn element_at(&self, body: &Body, b: Binder, fields: &[String]) -> Option<ResolvedType> {
        let mut ty = self.binder_type(body, b)?;
        for field in fields {
            while matches!(ty.as_builtin(), Some(Builtin::Result | Builtin::Option)) {
                ty = ty.args().first()?.clone();
            }
            ty = self.member_result(&ty, field)?;
        }
        element_of_type(&ty).cloned()
    }

    /// The type a binding holds, where a declaration says: its own, or its
    /// initialiser's, a query's or a call's result.
    fn binder_type(&self, body: &Body, b: Binder) -> Option<ResolvedType> {
        if let Some(t) = self.bindings.get(&b) {
            return Some(t.clone());
        }
        let Binder::Pattern(p) = b else {
            return None;
        };
        // A binding whose initialiser is a declaration returning `List<T>`.
        for id in body.walk() {
            let bound = match body.expr(id) {
                Expr::Let {
                    pat: Some(pat),
                    init: Some(init),
                    ..
                } if *pat == p => Some(*init),
                _ => None,
            };
            let Some(init) = bound else { continue };
            match body.expr(init) {
                // `query Menu(..)` names a declaration.
                Expr::Keyword {
                    keyword, modifiers, ..
                } if matches!(keyword.as_str(), "query" | "subscription") => {
                    if let Some(sig) = modifiers.first().and_then(|m| self.by_path(m)) {
                        return sig.result().filter(|t| !self.unsolved(t)).cloned();
                    }
                }
                // Anything else as it is typed: a call's result solved
                // (ADR-XXXX), where it was the callee's declared result, its
                // type parameters as written; and a name a `let` aliases,
                // which was nothing.
                _ => return self.of(body, init),
            }
        }
        None
    }

    /// **The member `name` of a value of type `ty`, as this body's module
    /// sees it** (ADR-0254): of several declarations, the one it imports or
    /// declares.
    pub fn member(
        &self,
        ty: &ResolvedType,
        name: &str,
    ) -> Option<&'a crate::signatures::Signature> {
        self.sigs.member_in(self.module.as_deref(), ty, name)
    }

    /// **The value `name` read from a value of type `ty` holds** (ADR-XXXX):
    /// its member's declared result under what `ty` gives its type
    /// parameters, by the value relations' one rule
    /// ([`crate::values::member_value`]). A generic record's field `v: T`
    /// read from a `Box<Item>` is an `Item`; it was `T`, and a member read on
    /// it named nothing.
    pub fn member_result(&self, ty: &ResolvedType, name: &str) -> Option<ResolvedType> {
        let sig = self.member(ty, name)?;
        // A callable with arguments beyond its receiver is a function value,
        // not a property holding the result of a call (ADR-0031, A-021).
        if self.sigs.by_def(sig.definition).is_some() && sig.params.len() != 1 {
            return None;
        }
        let result = sig.result()?;
        if !mentions_binder(result, sig.definition) {
            return Some(result.clone());
        }
        let solved = crate::values::member_value(self.sigs, sig, &Ty::of(ty));
        self.resolved_from(&solved, ty.span())
    }

    /// **Is `base.name` a member several modules declare, and this body's
    /// module cannot tell which** (ADR-0254)? PW0628's, and nothing else's to
    /// read as a call to something.
    pub fn ambiguous_member(&self, body: &Body, base: ExprId, name: &str) -> bool {
        self.of(body, base).is_some_and(|t| {
            !self
                .sigs
                .member_choices_of(self.module.as_deref(), &t, name)
                .is_empty()
        })
    }

    /// A declaration by path, trying this module first.
    fn by_path(&self, path: &str) -> Option<&'a crate::signatures::Signature> {
        self.sigs.in_module(self.module.as_deref(), path)
    }

    /// The type of an expression, or `None` when nothing declared says.
    ///
    /// `None` is a real answer and callers must treat it as one. It means "this
    /// program does not say", not "look it up some other way".
    /// Every name this body binds to a known type.
    ///
    /// Exposed so the effect checker's member lookup uses the SAME environment
    /// the type inference built, rather than assembling a narrower one from
    /// declaration parameters alone. A callback parameter typed here — `el` in
    /// `items |> List.map(fn(el) ..)` — is invisible to a second construction,
    /// and `el.getBoundingClientRect()` then resolves through nothing.
    /// Introduce a binding this body's own text does not declare.
    ///
    /// For a named execution root whose binder comes from its header rather
    /// than from a `let` or a parameter: `optimistic Cart(..) as cart => ..`
    /// binds `cart` to the target resource's VALUE type, and nothing in the
    /// body says so. Added rather than inferred, because the type comes from
    /// the resource the clause targets — see ADR-0025.
    pub fn with_binding(mut self, b: Binder, ty: &ResolvedType) -> Types<'a> {
        self.bindings.insert(b, ty.clone());
        self
    }

    pub fn stable_type(&self, ty: &ResolvedType) -> Option<crate::resolved::StableTypeId> {
        self.sigs.stable_type(ty)
    }

    /// The type a binding holds, where the program states it.
    pub fn binding(&self, b: Binder) -> Option<&ResolvedType> {
        self.bindings.get(&b)
    }

    /// The binding each local name means in this body.
    pub fn lexical(&self) -> &Lexical {
        &self.lexical
    }

    ///
    /// **Never a type naming a type parameter no declaration in scope binds**
    /// (ADR-XXXX): what this module's rules give, where that is a type a value
    /// here has; else what the value relations' typer solves; else `None`.
    pub fn of(&self, body: &Body, id: ExprId) -> Option<ResolvedType> {
        match self.declared(body, id) {
            Some(t) if !self.unsolved(&t) => Some(t),
            _ => self.resolved_from(self.solved.expr(id), body.expr_span(id)),
        }
    }

    /// **What the declarations say `id` is**, by this module's own rules: a
    /// call's declared result, as declared, a callee's type parameters
    /// included, which [`Types::of`] reads as unsolved.
    fn declared(&self, body: &Body, id: ExprId) -> Option<ResolvedType> {
        match body.expr(id) {
            // The binding the name means here (ADR-0063), or `self`.
            Expr::Name(n) => match self.lexical.binder(id) {
                Some(b) => self.bindings.get(&b).cloned(),
                None if n == "self" => self.self_type.clone(),
                None => None,
            },
            Expr::Field { base, name } => {
                let receiver = self.of(body, *base)?;
                self.member_result(&receiver, name)
            }
            Expr::Call { callee, .. } => {
                let sig = self.callee(body, *callee)?;
                let result = sig.result()?;
                // Even when this callee is the enclosing declaration (or
                // this function itself), its parameters are instantiated at
                // THIS call. Its declared T is not the caller's rigid T:
                // first<T>(Item { .. }) can return an Item within first<T>.
                // The shared solver, never the declaration, owns that type.
                if names_parameter(result, sig.definition) {
                    return None;
                }
                // A command answers its caller whether it committed, and its
                // declared error if not: never the value (ADR-0157).
                if self.sigs.kind_of(sig.definition) == Some(DeclKind::Command) {
                    return Some(ResolvedType::answered(result));
                }
                Some(result.clone())
            }
            // **`e?`, a block, and branches** (ADR-0250): what `e`'s `Result`
            // or `Option` holds; the last statement's value; and the
            // branches' or arms' type, where each says the same. Until
            // ADR-0250 `let h = Maps.create(..)?` held a value of no type,
            // and `h.destroy()` named no member.
            Expr::Try { value } => {
                let t = self.of(body, *value)?;
                match t.as_builtin() {
                    Some(Builtin::Result | Builtin::Option) => t.args().first().cloned(),
                    _ => None,
                }
            }
            Expr::Block { stmts } => self.of(body, *stmts.last()?),
            Expr::If {
                then, els: Some(e), ..
            } => {
                let t = self.of(body, *then)?;
                self.of(body, *e)
                    .is_some_and(|u| u.same_as(&t))
                    .then_some(t)
            }
            Expr::Match { arms, .. } => {
                let (first, rest) = arms.split_first()?;
                let t = self.of(body, first.body)?;
                rest.iter()
                    .all(|a| self.of(body, a.body).is_some_and(|u| u.same_as(&t)))
                    .then_some(t)
            }
            _ => None,
        }
    }

    /// **The function or command an `on:` value names** (ADR-0199): `save`
    /// or `forms.save`, which is `(e) => save(e)`, named by its path where no
    /// local binds its first segment. A local's value, or a declaration of
    /// another kind, names none, and the checker refuses it as a handler.
    pub fn named_handler(
        &self,
        body: &Body,
        e: ExprId,
    ) -> Option<&'a crate::signatures::Signature> {
        let root = crate::resume::root_name(body, e)?;
        if self.lexical.binder(root).is_some() {
            return None;
        }
        self.by_path(&path_of(body, e)).filter(|s| {
            matches!(
                self.sigs.kind_of(s.definition),
                Some(DeclKind::Fn | DeclKind::Command)
            )
        })
    }

    /// The signature a call resolves to, **only** when the receiver's type is
    /// known. Never guesses from a name.
    pub fn callee(&self, body: &Body, callee: ExprId) -> Option<&'a crate::signatures::Signature> {
        if let Some(sig) = self.by_path(&path_of(body, callee)) {
            return Some(sig);
        }
        match self.member_read(body, callee) {
            Some(MemberRead::Member(sig)) => sig,
            Some(_) => None,
            None => self.by_path(&path_of(body, callee)),
        }
    }

    /// **What `id`, a `.name` read, reads** (ADR-XXXX): the one reading of a
    /// member read that the effect inference, the labels and the checker
    /// share, so none skips what another refuses. `None` where `id` is no
    /// field read.
    ///
    /// By the receiver's type, whole where it is known, or by its
    /// constructor where only that is: a `Box<?>` has `Box`'s members, and
    /// which one `name` is does not depend on the hole.
    pub fn member_read(&self, body: &Body, id: ExprId) -> Option<MemberRead<'a>> {
        let Expr::Field { base, name } = body.expr(id) else {
            return None;
        };
        // A resolved module path is a path even if a declaration in this
        // module shares its head's spelling (List.reverse beside fn List).
        // A lexical receiver, however, is always that binding's value.
        if crate::resume::root_name(body, id)
            .is_some_and(|head| self.lexical.binder(head).is_none())
            && self.by_path(&path_of(body, id)).is_some()
        {
            return Some(MemberRead::Path);
        }
        if !self.is_value(body, *base) {
            return Some(MemberRead::Path);
        }
        if let Some(t) = self.of(body, *base) {
            return Some(match t.parameter_binding() {
                Some(_) => MemberRead::Parameter,
                None => MemberRead::Member(self.member(&t, name)),
            });
        }
        Some(match self.solved.expr(*base) {
            Ty::Any => MemberRead::Never,
            Ty::Parameter { .. } => MemberRead::Parameter,
            t => match t.receiver() {
                Some(r) => {
                    MemberRead::Member(self.sigs.member_by_in(self.module.as_deref(), r, name))
                }
                None => MemberRead::Unknown,
            },
        })
    }

    /// **Each member read in `body` whose receiver's type nothing states**
    /// (ADR-XXXX): a `.name` read through a value that neither the
    /// declarations nor the value relations' typer give a type, so no
    /// analysis can read which member it is, what it performs or what it
    /// carries. The effect rows refuse each (PW0408); until then each was
    /// skipped unseen. Every expression of the body's arena: its roots' and
    /// its markup's too.
    pub fn untyped_receivers(&self, body: &Body) -> Vec<ExprId> {
        body.exprs()
            .map(|(id, _, _)| id)
            .filter(|id| matches!(self.member_read(body, *id), Some(MemberRead::Unknown)))
            .collect()
    }

    /// **Is `e` a value** rather than a module's or a type's path: a local,
    /// `self` where it is the element, a declaration or a module's signal
    /// named as a value, or anything computed. A dimensioned literal's
    /// number, `900` in `900.px`, is no receiver either.
    fn is_value(&self, body: &Body, e: ExprId) -> bool {
        use crate::resolve::{Namespace, Resolution};
        let term = |path: &str| {
            let Some(unit) = self.sigs.unit_of(self.module.as_deref()) else {
                return false;
            };
            let ws = self.sigs.workspace();
            let found = match path.contains('.') {
                true => ws.resolve_path_in(unit, Namespace::Term, path),
                false => ws.resolve_in(unit, Namespace::Term, path),
            };
            matches!(found, Resolution::Local(_) | Resolution::Imported { .. })
        };
        match body.expr(e) {
            Expr::Name(n) => {
                self.lexical.binder(e).is_some()
                    || (n == "self" && self.self_type.is_some())
                    || term(n)
            }
            Expr::Field { base, .. } => term(&path_of(body, e)) || self.is_value(body, *base),
            Expr::Literal(crate::hir::Literal::Int(_) | crate::hir::Literal::Float(_)) => false,
            _ => true,
        }
    }
}

/// **What a member read reads** (ADR-XXXX, the generic-effects track), as
/// [`Types::member_read`] tells it. Every analysis that reads members reads
/// this, and each case is either read, refused by a rule that names it, or a
/// read no run makes: never a value whose member is skipped unseen.
#[derive(Debug, Clone, Copy)]
pub enum MemberRead<'s> {
    /// A member of the receiver's type: its declaration, or none where the
    /// type has none (PW0610) or several modules declare one this module
    /// cannot tell apart (PW0628), each refused by the value relations.
    Member(Option<&'s crate::signatures::Signature>),
    /// No read through a value: a module's path, `List.map`, a type's,
    /// `Shape.Circle`, or a dimensioned literal, `900.px`.
    Path,
    /// A value of any type, which no run holds: `todo`'s, or an element of
    /// `[]`.
    Never,
    /// A value of a type parameter, which has no member (PW0610).
    Parameter,
    /// A value whose type nothing states and nothing solves (PW0408).
    Unknown,
}

/// The element type of a written type, seeing through the carriers.
///
/// ```text
/// List<MenuItem>                  -> MenuItem
/// Result<List<MenuItem>, Error>   -> MenuItem
/// Option<List<MenuItem>>          -> MenuItem
/// MenuItem                        -> None, it is not a collection
/// ```
///
/// `Result` and `Option` are seen through because a query returns one and the
/// loop iterates what is inside. That is not the same as ignoring them: a
/// `Result` still has to be handled, and `option_used_as_value` is the rule
/// that says so. This answers a different question — what one element IS.
/// Does this type mention a type parameter `binder` binds?
fn mentions_binder(ty: &ResolvedType, binder: DefId) -> bool {
    ty.parameter_binding().is_some_and(|(b, _)| b == binder)
        || ty.args().iter().any(|a| mentions_binder(a, binder))
}

/// **`body` as the value relations' typer solves it**, once per check
/// (ADR-XXXX): `nested` where it is solved inside the declaration it is
/// nested in, which types the bindings around it. Nothing where `module`
/// names no unit.
fn solved(
    sigs: &Signatures,
    module: Option<&str>,
    body: &Body,
    nested: bool,
    solve: impl FnOnce() -> SolvedBody,
) -> std::rc::Rc<SolvedBody> {
    let Some(unit) = sigs.unit_of(module) else {
        return std::rc::Rc::new(SolvedBody::default());
    };
    let key = (
        unit,
        body.owner,
        body as *const Body as usize,
        body.root,
        nested,
    );
    sigs.solved(key, solve)
}

/// **The declaration whose type parameters `decl`'s written types name**:
/// the binder [`Signatures::resolve_type`] gives them, so a parameter of
/// `decl`'s own is in scope in its body, and no other's is.
fn own_binder(sigs: &Signatures, module: Option<&str>, decl: &Decl) -> Option<DefId> {
    use crate::resolve::{Namespace, Resolution};
    let unit = sigs.unit_of(module)?;
    let ns = Namespace::of(decl.kind)?;
    match sigs.workspace().resolve_in(unit, ns, &decl.name) {
        Resolution::Local(def) | Resolution::Imported { def, .. } => Some(def),
        _ => None,
    }
}

pub fn element_of_type(ty: &ResolvedType) -> Option<&ResolvedType> {
    match ty.as_builtin()? {
        Builtin::List => ty.args().first(),
        Builtin::Result | Builtin::Option => element_of_type(ty.args().first()?),
        // A map or a set is read through `Map.keys` or `Set.to_list`.
        Builtin::Function | Builtin::Map | Builtin::Set => None,
    }
}

pub fn path_of(body: &Body, id: ExprId) -> String {
    match body.expr(id) {
        Expr::Name(n) => n.clone(),
        Expr::Field { base, name } => format!("{}.{}", path_of(body, *base), name),
        _ => String::new(),
    }
}

/// Whether a declared result contains this callee's own type parameters.
/// Binding identity alone cannot distinguish its recursive invocation from
/// the enclosing generic's rigid parameters; the call must instantiate it.
fn names_parameter(t: &ResolvedType, binder: DefId) -> bool {
    t.parameter_binding().is_some_and(|(b, _)| b == binder)
        || t.args().iter().any(|a| names_parameter(a, binder))
}
