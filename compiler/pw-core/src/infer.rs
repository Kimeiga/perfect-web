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
        let mut bindings: BTreeMap<String, String> = BTreeMap::new();

        for p in &decl.params {
            if let Some(t) = &p.ty {
                bindings.insert(p.name.clone(), t.written());
            }
        }

        // `List<MenuItem>` for a parameter, so the `#each` rule below can ask
        // what an element of it is.
        let mut element_of: BTreeMap<String, String> = BTreeMap::new();
        for p in &decl.params {
            if let Some(head) = &p.ty
                && let Some(elem) = element_of_written(head.constructor_head_only(), head.args())
            {
                element_of.insert(p.name.clone(), elem);
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

        // A CALLBACK's parameter takes the element type of the collection the
        // callback is applied to.
        //
        // `items |> List.map(fn(el) el.getBoundingClientRect().width)` gives
        // `el` the type `ElementRef`, so `getBoundingClientRect` resolves
        // through the RECEIVER rather than through a spelling.
        //
        // Architect ruling, 2026-08-07:
        //
        // > `el.getBoundingClientRect()` should resolve because
        // > `type(el) = Element` and Element's declared member set contains
        // > `getBoundingClientRect` — not because some globally unique
        // > declaration has that spelling.
        //
        // The same shape `{#each}` already relies on, one level down. It is
        // deliberately narrow: the callback's FIRST parameter, and only when a
        // sibling argument is a collection whose element type is known. A
        // callback over something that is not a collection binds nothing,
        // because there is nothing to be right about.
        // `xs |> f(cb)` keeps its pipe: the HIR is `Binary { Pipe, xs, f(cb) }`
        // rather than a call with `xs` prepended. So the piped value is
        // collected here and offered to the call on its right, which is what
        // `|>` means.
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
            let Expr::Call { args, .. } = body.expr(id) else {
                continue;
            };
            // The element type any sibling argument offers — including the
            // piped receiver.
            let element = args
                .iter()
                .map(|a| a.value)
                .chain(piped.get(&id).copied())
                .find_map(|value| {
                    let name = path_of(body, value);
                    element_of
                        .get(&name)
                        .cloned()
                        .or_else(|| types.element_type(body, &name))
                });
            let Some(element) = element else { continue };

            for a in args {
                let Expr::Lambda { params, .. } = body.expr(a.value) else {
                    continue;
                };
                let Some(first) = params.first() else {
                    continue;
                };
                // `fn(el)` arrives as a constructor-shaped wrapper around the
                // binding — the parameter LIST is call-shaped — so the name is
                // found by walking rather than matched at the top.
                if let Some(name) = first_bound_name(body, *first) {
                    types.bindings.entry(name).or_insert(element.clone());
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
    fn element_type(&self, body: &Body, name: &str) -> Option<String> {
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
                return element_of_written(sig.returns.as_deref()?, &sig.returns_args);
            }
        }
        None
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
    /// Every name this body binds to a known type.
    ///
    /// Exposed so the effect checker's member lookup uses the SAME environment
    /// the type inference built, rather than assembling a narrower one from
    /// declaration parameters alone. A callback parameter typed here — `el` in
    /// `items |> List.map(fn(el) ..)` — is invisible to a second construction,
    /// and `el.getBoundingClientRect()` then resolves through nothing.
    pub fn bindings(&self) -> &BTreeMap<String, String> {
        &self.bindings
    }

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
/// The first name a pattern binds, however it is wrapped.
///
/// A lambda parameter list is call-shaped, so `fn(el)` lowers to a constructor
/// pattern with an empty path around the binding. Matching `Bind` at the top
/// finds nothing, and finding nothing here is indistinguishable from a callback
/// over something untyped.
fn first_bound_name(body: &Body, pat: crate::hir::PatternId) -> Option<String> {
    use crate::hir::Pattern;
    match body.pat(pat) {
        Pattern::Bind { name, .. } => Some(name.clone()),
        Pattern::Ctor { args, .. } => args.iter().find_map(|a| first_bound_name(body, *a)),
        _ => None,
    }
}

fn element_of_written(head: &str, args: &[String]) -> Option<String> {
    match head {
        "List" => args.first().map(|a| strip_args(a)),
        "Result" | "Option" => {
            let inner = args.first()?;
            let (h, rest) = split_written(inner);
            element_of_written(&h, &rest)
        }
        _ => None,
    }
}

/// `List<MenuItem>` -> `("List", ["MenuItem"])`.
fn split_written(t: &str) -> (String, Vec<String>) {
    let t = t.trim();
    match t.split_once('<') {
        Some((head, rest)) => {
            let inner = rest.strip_suffix('>').unwrap_or(rest);
            // One level of nesting: split on commas outside angle brackets.
            let mut args = Vec::new();
            let (mut depth, mut start) = (0i32, 0usize);
            for (i, c) in inner.char_indices() {
                match c {
                    '<' => depth += 1,
                    '>' => depth -= 1,
                    ',' if depth == 0 => {
                        args.push(inner[start..i].trim().to_string());
                        start = i + 1;
                    }
                    _ => {}
                }
            }
            args.push(inner[start..].trim().to_string());
            (head.trim().to_string(), args)
        }
        None => (t.to_string(), Vec::new()),
    }
}

fn strip_args(t: &str) -> String {
    t.split('<').next().unwrap_or(t).trim().to_string()
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
