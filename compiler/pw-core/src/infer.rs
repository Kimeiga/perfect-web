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
//! A lambda parameter is typed where its callee's declared function type
//! says, without solving the call: `el` in `List.map(items, el => ..)` is an
//! element of `items`, because `map` takes a `List<T>` and a `fn(T) -> U`.
//!
//! # What it does not infer
//!
//! A lambda parameter whose type needs the call solved, such as `fold`'s
//! accumulator `A`, which the seed argument instantiates. `values.rs` solves
//! calls. Where this module cannot say, [`Types::of`] returns `None`, and it is
//! each caller's decision whether to fall back or to stop.
//! `docs/evidence/P0/readiness.txt` records which ones still fall back.

use std::collections::BTreeMap;

use crate::hir::{Body, Decl, DeclKind, Expr, ExprId};
use crate::resolved::{self, Builtin, ResolvedType};
use crate::signatures::Signatures;

/// The types known inside one declaration's body.
pub struct Types<'a> {
    sigs: &'a Signatures,
    bindings: BTreeMap<String, ResolvedType>,
    /// The module this body lives in, so a bare name resolves to a sibling
    /// declaration. Without it `shrink(self)` looked up `shrink` and found
    /// nothing, because signatures are stored module-qualified — so a rule
    /// following a helper stopped at the module boundary of its own file.
    module: Option<String>,
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
        let mut bindings: BTreeMap<String, ResolvedType> = BTreeMap::new();

        for p in &decl.params {
            if let Some(t) = &p.ty
                && let Some(ty) = sigs
                    .resolve_type(module, decl, t, p.span.clone())
                    .resolved()
            {
                bindings.insert(p.name.clone(), ty.clone());
            }
        }

        // `List<MenuItem>` for a parameter, so the `#each` rule below can ask
        // what an element of it is.
        let element_of: BTreeMap<String, ResolvedType> = bindings
            .iter()
            .filter_map(|(name, ty)| element_of_type(ty).map(|t| (name.clone(), t.clone())))
            .collect();

        // Charter §7.5A writes `self.style.set_padding(..)` inside a component.
        // `self` is the declaration's own element, and that is a language fact
        // about UI declarations rather than something a library declares.
        if matches!(
            decl.kind,
            DeclKind::View | DeclKind::Component | DeclKind::Page
        ) && let Some(ty) = sigs.language_type("browser", "ElementRef")
        {
            bindings.insert("self".to_string(), ty);
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
                (body.pat(*pat), resolved::written_in_body(body, *ty))
                && let Some(ty) = sigs.resolve_type(module, decl, &t, 0..0).resolved()
            {
                bindings.insert(name.clone(), ty.clone());
            }
        }

        let mut types = Types {
            sigs,
            bindings,
            module: module.map(str::to_string),
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
        let mut roots = Vec::new();
        for e in body.walk() {
            if let Expr::Template { roots: r, .. } = body.expr(e) {
                roots.extend(r.iter().copied());
            }
        }
        for n in body.walk_markup(&roots) {
            let crate::hir::Node::Block { directive, .. } = body.node(n) else {
                continue;
            };
            let Some((binding, collection)) = each_binding(directive) else {
                continue;
            };
            if let Some(elem) = element_of
                .get(&collection)
                .cloned()
                .or_else(|| types.element_type(body, &collection))
            {
                types.bindings.insert(binding, elem);
            }
        }

        // The same rule for a `{#match}` arm (ADR-0042):
        //
        //     Option<Entry>          Result<T, E>
        //         | {:Some(e)}           | {:Ok(v)}  {:Err(x)}
        //     e : Entry              v : T     x : E
        for n in body.walk_markup(&roots) {
            let crate::hir::Node::Block {
                subject: Some(s),
                children,
                ..
            } = body.node(n)
            else {
                continue;
            };
            let Some(ty) = types.of(body, *s) else {
                continue;
            };
            for c in children {
                let crate::hir::Node::Branch {
                    arm: Some((case, Some(name))),
                    ..
                } = body.node(*c)
                else {
                    continue;
                };
                let payload = match (ty.as_builtin(), case.as_str()) {
                    (Some(Builtin::Option), "Some") | (Some(Builtin::Result), "Ok") => {
                        ty.args().first()
                    }
                    (Some(Builtin::Result), "Err") => ty.args().get(1),
                    _ => None,
                };
                if let Some(p) = payload.cloned() {
                    types.bindings.insert(name.clone(), p);
                }
            }
        }

        // A CALLBACK's parameter takes the type its callee declares for it.
        //
        // `items |> List.map(fn(el) el.getBoundingClientRect().width)` gives
        // `el` the type `ElementRef`: `map` takes a `List<T>` and a
        // `fn(T) -> U`, and `items` is a `List<ElementRef>`. So
        // `getBoundingClientRect` resolves through the RECEIVER rather than
        // through a spelling.
        //
        // Architect ruling, 2026-08-07:
        //
        // > `el.getBoundingClientRect()` should resolve because
        // > `type(el) = Element` and Element's declared member set contains
        // > `getBoundingClientRect` — not because some globally unique
        // > declaration has that spelling.
        //
        // A parameter is typed where the callee's function type gives it a
        // type that mentions no type parameter, or exactly the type parameter
        // a list argument's element instantiates. Anything else needs the
        // call solved, which `values.rs` does.
        //
        // Until 2026-09-25 the rule was "the FIRST parameter takes the element
        // type of any list beside it", whatever the callee declared. `fold`'s
        // callback is `fn(A, T) -> A`, whose first parameter is the
        // accumulator, so `List.fold(ws, 0, (t, w) => t + w.score)` typed `t`
        // as a `Word` and was refused (ADR-0053).
        //
        // `xs |> f(cb)` keeps its pipe: the HIR is `Binary { Pipe, xs, f(cb) }`
        // rather than a call with `xs` prepended. So the piped value is
        // collected here and offered to the call on its right as its first
        // argument, which is what `|>` means.
        let mut piped: BTreeMap<ExprId, ExprId> = BTreeMap::new();
        for id in body.walk() {
            if let Expr::Binary {
                op: crate::hir::BinOp::Pipe,
                lhs,
                rhs,
            } = body.expr(id)
            {
                piped.insert(*rhs, *lhs);
            }
        }

        for id in body.walk() {
            let Expr::Call { callee, args } = body.expr(id) else {
                continue;
            };
            // A named argument's position is not its parameter's.
            if args.iter().any(|a| a.name.is_some()) {
                continue;
            }
            let Some(sig) = types.callee(body, *callee) else {
                continue;
            };
            let values: Vec<(usize, ExprId)> = piped
                .get(&id)
                .copied()
                .into_iter()
                .chain(args.iter().map(|a| a.value))
                .enumerate()
                .collect();
            let declared = |i: usize| {
                sig.params
                    .get(i)
                    .and_then(Option::as_ref)
                    .and_then(resolved::TypeResolution::resolved)
            };
            // The element each list argument gives its type parameter:
            // `items: List<T>` passed a `List<Word>` makes `T` a `Word`.
            let mut elements = BTreeMap::new();
            for (i, value) in &values {
                let Some(param) = declared(*i)
                    .and_then(element_of_type)
                    .and_then(ResolvedType::parameter_binding)
                else {
                    continue;
                };
                let name = path_of(body, *value);
                if let Some(e) = element_of
                    .get(&name)
                    .cloned()
                    .or_else(|| types.element_type(body, &name))
                {
                    elements.entry(param).or_insert(e);
                }
            }
            for (i, value) in &values {
                let Expr::Lambda { params, .. } = body.expr(*value) else {
                    continue;
                };
                let Some(f) = declared(*i).filter(|t| t.as_builtin() == Some(Builtin::Function))
                else {
                    continue;
                };
                let Some((_, wanted)) = f.args().split_last() else {
                    continue;
                };
                let Some(names) = crate::values::lambda_names(body, params) else {
                    continue;
                };
                if names.len() != wanted.len() {
                    continue;
                }
                for (name, w) in names.into_iter().zip(wanted) {
                    let ty = match w.parameter_binding() {
                        Some(p) => elements.get(&p).cloned(),
                        None if !mentions_parameter(w) => Some(w.clone()),
                        None => None,
                    };
                    if let Some(t) = ty {
                        types.bindings.entry(name).or_insert(t);
                    }
                }
            }
        }

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

    /// The element type of a collection-valued binding or call.
    ///
    /// `menu` bound from `query Menu(..)` whose declaration returns
    /// `List<MenuItemId>` gives `MenuItemId`.
    fn element_type(&self, body: &Body, name: &str) -> Option<ResolvedType> {
        // A binding whose initialiser is a declaration returning `List<T>`.
        for id in body.walk() {
            let bound = match body.expr(id) {
                Expr::Let {
                    pat: Some(pat),
                    init: Some(init),
                    ..
                } => match body.pat(*pat) {
                    crate::hir::Pattern::Bind { name: n, .. } if n == name => Some(*init),
                    _ => None,
                },
                _ => None,
            };
            let Some(init) = bound else { continue };
            let sig = match body.expr(init) {
                // `query Menu(..)` names a declaration.
                Expr::Keyword {
                    keyword, modifiers, ..
                } if matches!(keyword.as_str(), "query" | "subscription") => {
                    modifiers.first().and_then(|m| self.by_path(m))
                }
                Expr::Call { callee, .. } => self.callee(body, *callee),
                _ => None,
            };
            if let Some(sig) = sig {
                return element_of_type(sig.result()?).cloned();
            }
        }
        None
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
    pub fn with_binding(mut self, name: &str, ty: &ResolvedType) -> Types<'a> {
        self.bindings.insert(name.to_string(), ty.clone());
        self
    }

    pub fn stable_type(&self, ty: &ResolvedType) -> Option<crate::resolved::StableTypeId> {
        self.sigs.stable_type(ty)
    }

    pub fn bindings(&self) -> &BTreeMap<String, ResolvedType> {
        &self.bindings
    }

    pub fn of(&self, body: &Body, id: ExprId) -> Option<ResolvedType> {
        match body.expr(id) {
            Expr::Name(n) => self.bindings.get(n).cloned(),
            Expr::Field { base, name } => {
                let receiver = self.of(body, *base)?;
                self.sigs
                    .member_of(&receiver, name)
                    .and_then(|s| s.result().cloned())
            }
            Expr::Call { callee, .. } => {
                self.callee(body, *callee).and_then(|s| s.result().cloned())
            }
            _ => None,
        }
    }

    /// The signature a call resolves to, **only** when the receiver's type is
    /// known. Never guesses from a name.
    pub fn callee(&self, body: &Body, callee: ExprId) -> Option<&'a crate::signatures::Signature> {
        if let Some(sig) = self.by_path(&path_of(body, callee)) {
            return Some(sig);
        }
        match body.expr(callee) {
            Expr::Field { base, name } => {
                let receiver = self.of(body, *base)?;
                self.sigs.member_of(&receiver, name)
            }
            _ => self.by_path(&path_of(body, callee)),
        }
    }
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
/// Does this type mention a type parameter anywhere?
fn mentions_parameter(ty: &ResolvedType) -> bool {
    ty.parameter_binding().is_some() || ty.args().iter().any(mentions_parameter)
}

pub fn element_of_type(ty: &ResolvedType) -> Option<&ResolvedType> {
    match ty.as_builtin()? {
        Builtin::List => ty.args().first(),
        Builtin::Result | Builtin::Option => element_of_type(ty.args().first()?),
        // A map or a set is read through `Map.keys` or `Set.to_list`.
        Builtin::Function | Builtin::Map | Builtin::Set => None,
    }
}

/// `{#each menu as item (item.id)}` gives `("item", "menu")`.
///
/// Read from the directive's text because that is where the parser leaves it —
/// a markup block keeps its directive verbatim. The shape is fixed by the
/// grammar, so this is reading a known form rather than guessing at one.
fn each_binding(directive: &str) -> Option<(String, String)> {
    let d = directive.trim();
    let inner = d.strip_prefix("{#each")?.strip_suffix('}')?;
    let (collection, rest) = inner.trim().split_once(" as ")?;
    let binding = rest
        .trim()
        .split(|c: char| c == '(' || c.is_whitespace())
        .find(|s| !s.is_empty())?;
    Some((binding.to_string(), collection.trim().to_string()))
}

pub fn path_of(body: &Body, id: ExprId) -> String {
    match body.expr(id) {
        Expr::Name(n) => n.clone(),
        Expr::Field { base, name } => format!("{}.{}", path_of(body, *base), name),
        _ => String::new(),
    }
}
