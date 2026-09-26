//! **E9-V — the ordinary value relations.**
//!
//! Architect ruling, 2026-08-21, reopening E9:
//!
//! > A milestone called *permanent value type checker* cannot honestly remain
//! > complete while `takes_str(42)`, `fn wrong_return() -> String { 42 }` and
//! > `takes_store(makes_cart())` all pass. That is not an edge case. It is a
//! > missing central relation.
//!
//! This module is that relation. It answers, for every place a value meets a
//! declared type, whether the value has that type:
//!
//! ```text
//! Arity      a call supplies exactly the arguments its callee declares  PW0604
//! Argument   each argument has its parameter's type                     PW0605
//! Field      each constructed field has its declared type               PW0605
//! Return     a body produces its declared result                         PW0606
//! Binding    `let x: T = e` initialises `x` with a `T`                  PW0607
//! Annotation every written type names a type visible from where it is    PW0026
//! ```
//!
//! # Three-valued, like every analysis here
//!
//! A relation is [`Outcome::Agree`], [`Outcome::Disagree`] or
//! [`Outcome::Undecided`]. Undecided is not agreement and is never reported as
//! a violation: it says the program does not state enough for this analysis to
//! decide. Diagnostics are a **projection** of [`relations`], so an audit can
//! count what was decided rather than infer it from silence — the hole
//! `MatchOutcome::Blocked` closed for exhaustiveness.
//!
//! # Identity, never representation
//!
//! `opaque type Tag = String` in two modules is two types, and a `Tag` is not
//! a `String`. Every comparison goes through the resolved identity
//! ([`crate::resolved::TypeKey`]): a DefId for a declaration, the binder and
//! position for a type parameter. Nothing here reads a spelling to decide.
//!
//! # Generic callees are instantiated per call
//!
//! `fn first<T>(items: List<T>) -> Option<T>` has one signature and a fresh
//! `T` at every call. The callee's own parameters become inference variables,
//! the arguments are unified against them left to right, and the result is the
//! declared result under that substitution. A variable no argument determined
//! leaves the result's corresponding part unknown — never guessed.
//!
//! Inside a generic body its own parameters are **rigid**: `fn id<T>(x: T) ->
//! T { 42 }` is refused, because nothing makes `42` a `T`.
//!
//! # What it deliberately does not decide
//!
//! Recorded in `docs/KNOWN_LIMITATIONS.md` rather than approximated here:
//! function types do not exist, so a lambda's type is unknown and a callback
//! parameter is Undecided; a sum type's variant constructors are not resolved
//! by the workspace, so `Circle(3)` has no type; a call with named arguments is
//! Undecided because a signature does not carry parameter names.

use std::cell::RefCell;
use std::collections::{BTreeMap, BTreeSet};

use crate::diagnostics::{Detector, Diagnostic};
use crate::hir::{
    BinOp, Body, Decl, DeclId, DeclKind, Expr, ExprId, Hir, Literal, Pattern, Span, UnOp,
};
use crate::infer::Types;
use crate::resolve::{DefId, Namespace, Resolution, UnitId, Workspace};
use crate::resolved::{Builtin, Primitive, ResolvedType, TypeKey, TypeResolution};
use crate::signatures::{Receiver, Signature, Signatures};

// --- the type inference holds -------------------------------------------------

/// **A type as inference knows it**: a resolved identity, with holes where
/// the program does not say.
///
/// Not a second semantic type. Every non-hole node is a projection of a
/// [`ResolvedType`]'s [`TypeKey`] — the same identity `same_as` compares —
/// and [`Ty::Unknown`] marks exactly the parts no declaration determined.
/// `Result<Int, ?>` is what `Ok(1)` is, and forcing it into a `ResolvedType`
/// would mean inventing the `?`.
///
/// No derived comparison, for `ResolvedType`'s reason: [`unify`] is the only
/// relation, and a second one would be free to disagree with it.
#[derive(Debug, Clone)]
pub enum Ty {
    Primitive(Primitive),
    Builtin(Builtin, Vec<Ty>),
    Nominal(DefId, Vec<Ty>),
    /// A type parameter, rigid: the enclosing declaration's own `T`.
    Parameter {
        binder: DefId,
        index: u32,
    },
    /// An inference variable, alive only while one call is being solved.
    Var(u32),
    /// The program does not say.
    Unknown,
}

impl Ty {
    /// Does this mention a type parameter of a declaration other than `own`:
    /// a callee's `T` that no call instantiated?
    fn mentions_foreign_parameter(&self, own: Option<DefId>) -> bool {
        match self {
            Ty::Parameter { binder, .. } => Some(*binder) != own,
            Ty::Builtin(_, args) | Ty::Nominal(_, args) => {
                args.iter().any(|a| a.mentions_foreign_parameter(own))
            }
            _ => false,
        }
    }

    pub fn of(t: &ResolvedType) -> Ty {
        Ty::from_key(&t.semantic_key())
    }

    fn from_key(k: &TypeKey) -> Ty {
        match k {
            TypeKey::Primitive(p) => Ty::Primitive(*p),
            TypeKey::Builtin(b, args) => Ty::Builtin(*b, args.iter().map(Ty::from_key).collect()),
            TypeKey::Nominal(d, args) => Ty::Nominal(*d, args.iter().map(Ty::from_key).collect()),
            TypeKey::Parameter { binder, index } => Ty::Parameter {
                binder: *binder,
                index: *index,
            },
        }
    }

    fn is_unknown(&self) -> bool {
        matches!(self, Ty::Unknown)
    }

    /// The constructor a member lookup keys on. A hole in an argument does not
    /// change which members a `List<?>` has.
    fn receiver(&self) -> Option<Receiver> {
        match self {
            Ty::Primitive(p) => Some(Receiver::Primitive(*p)),
            Ty::Builtin(b, _) => Some(Receiver::Builtin(*b)),
            Ty::Nominal(d, _) => Some(Receiver::Nominal(*d)),
            _ => None,
        }
    }

    /// Replace the given binder's parameters with fresh variables numbered by
    /// position. Every other node is kept.
    fn instantiate(&self, binder: DefId) -> Ty {
        match self {
            Ty::Parameter { binder: b, index } if *b == binder => Ty::Var(*index),
            Ty::Builtin(c, args) => {
                Ty::Builtin(*c, args.iter().map(|a| a.instantiate(binder)).collect())
            }
            Ty::Nominal(d, args) => {
                Ty::Nominal(*d, args.iter().map(|a| a.instantiate(binder)).collect())
            }
            other => other.clone(),
        }
    }
}

// --- unification ---------------------------------------------------------------

/// The verdict of one relation between an expected and an actual type.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Verdict {
    Agree,
    Disagree,
    /// Some part the relation depends on is unknown. Not agreement.
    Undecided,
}

impl Verdict {
    fn and(self, other: Verdict) -> Verdict {
        match (self, other) {
            (Verdict::Disagree, _) | (_, Verdict::Disagree) => Verdict::Disagree,
            (Verdict::Undecided, _) | (_, Verdict::Undecided) => Verdict::Undecided,
            _ => Verdict::Agree,
        }
    }
}

/// One call's substitution. Created per call and dropped after it, so a
/// variable can never leak into another expression's type.
#[derive(Debug, Default)]
struct Subst {
    bound: BTreeMap<u32, Ty>,
}

impl Subst {
    fn resolve(&self, t: &Ty) -> Ty {
        match t {
            Ty::Var(v) => match self.bound.get(v) {
                Some(b) => self.resolve(b),
                None => Ty::Var(*v),
            },
            other => other.clone(),
        }
    }

    /// Apply the substitution, turning every variable nothing determined into
    /// a hole. The only way a type leaves a call.
    fn close(&self, t: &Ty) -> Ty {
        match self.resolve(t) {
            Ty::Var(_) => Ty::Unknown,
            Ty::Builtin(c, args) => Ty::Builtin(c, args.iter().map(|a| self.close(a)).collect()),
            Ty::Nominal(d, args) => Ty::Nominal(d, args.iter().map(|a| self.close(a)).collect()),
            other => other,
        }
    }

    fn occurs(&self, v: u32, t: &Ty) -> bool {
        match self.resolve(t) {
            Ty::Var(w) => w == v,
            Ty::Builtin(_, args) | Ty::Nominal(_, args) => args.iter().any(|a| self.occurs(v, a)),
            _ => false,
        }
    }
}

/// **The value relation**: may a value of `actual` stand where `expected` is
/// declared?
///
/// By identity, recursively through arguments. Two nominal types are the same
/// type only when they are the same declaration applied to the same
/// arguments; a primitive is only itself; a rigid type parameter is only
/// itself. A variable is bound by the first thing it meets.
fn unify(s: &mut Subst, expected: &Ty, actual: &Ty) -> Verdict {
    let (e, a) = (s.resolve(expected), s.resolve(actual));
    match (&e, &a) {
        (Ty::Var(v), Ty::Var(w)) if v == w => Verdict::Agree,
        (Ty::Var(v), t) | (t, Ty::Var(v)) => {
            if t.is_unknown() || s.occurs(*v, t) {
                return Verdict::Undecided;
            }
            s.bound.insert(*v, t.clone());
            Verdict::Agree
        }
        (Ty::Unknown, _) | (_, Ty::Unknown) => Verdict::Undecided,
        (
            Ty::Parameter {
                binder: b1,
                index: i1,
            },
            Ty::Parameter {
                binder: b2,
                index: i2,
            },
        ) => match b1 == b2 && i1 == i2 {
            true => Verdict::Agree,
            false => Verdict::Disagree,
        },
        (Ty::Primitive(p), Ty::Primitive(q)) => match p == q {
            true => Verdict::Agree,
            false => Verdict::Disagree,
        },
        (Ty::Builtin(c1, a1), Ty::Builtin(c2, a2)) => {
            if c1 != c2 || a1.len() != a2.len() {
                return Verdict::Disagree;
            }
            all(s, a1, a2)
        }
        (Ty::Nominal(d1, a1), Ty::Nominal(d2, a2)) => {
            if d1 != d2 {
                return Verdict::Disagree;
            }
            // One declaration applied to a different number of arguments is
            // a malformed type, which resolution refuses; reaching here with
            // one means an upstream stage did not, and that is not this
            // relation's verdict to give.
            if a1.len() != a2.len() {
                return Verdict::Undecided;
            }
            all(s, a1, a2)
        }
        _ => Verdict::Disagree,
    }
}

fn all(s: &mut Subst, expected: &[Ty], actual: &[Ty]) -> Verdict {
    let mut verdict = Verdict::Agree;
    for (e, a) in expected.iter().zip(actual) {
        verdict = verdict.and(unify(s, e, a));
        if verdict == Verdict::Disagree {
            break;
        }
    }
    verdict
}

/// The information in both, or a hole where they conflict. For the branches
/// of an `if` or a `match`, whose value is whichever one ran.
fn join(a: Ty, b: Ty) -> Ty {
    match (a, b) {
        (Ty::Unknown, t) | (t, Ty::Unknown) => t,
        (Ty::Primitive(p), Ty::Primitive(q)) if p == q => Ty::Primitive(p),
        (Ty::Builtin(c1, a1), Ty::Builtin(c2, a2)) if c1 == c2 && a1.len() == a2.len() => {
            Ty::Builtin(
                c1,
                a1.into_iter().zip(a2).map(|(x, y)| join(x, y)).collect(),
            )
        }
        (Ty::Nominal(d1, a1), Ty::Nominal(d2, a2)) if d1 == d2 && a1.len() == a2.len() => {
            Ty::Nominal(
                d1,
                a1.into_iter().zip(a2).map(|(x, y)| join(x, y)).collect(),
            )
        }
        (
            Ty::Parameter {
                binder: b1,
                index: i1,
            },
            Ty::Parameter {
                binder: b2,
                index: i2,
            },
        ) if b1 == b2 && i1 == i2 => Ty::Parameter {
            binder: b1,
            index: i1,
        },
        _ => Ty::Unknown,
    }
}

// --- what the analysis reports ---------------------------------------------------

/// Which relation a record is about.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum RelationKind {
    Arity,
    Argument,
    Field,
    Return,
    Binding,
    Annotation,
    /// An operator's operand, or an `if`'s condition, against the type it
    /// takes (PW0609).
    Operand,
    /// `x = e`: `e` against the type `x` holds (PW0607, ADR-0051).
    Assignment,
    /// `value.name`: the member a read or a call names, against the members
    /// the value's type has (PW0610, ADR-0048).
    Member,
}

/// A member relation's expectation when the member is an opaque type's
/// representation, read outside the module that declares the type.
const PRIVATE_REPRESENTATION: &str = "its representation, which only its own module reads";

/// A member relation's expectation when the member is the contents' of an
/// `Option` or a `Result`, read from the container (PW0600).
const OPTIONAL_CONTENTS: &str = "its contents, taken apart first";

/// Why a relation was not decided.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum Undecided {
    /// Part of the value's type is not stated by anything the program declares.
    Unknown,
    /// The declared type did not resolve; the relation refuses to run on it.
    ExpectedUnresolved,
    /// The call names its arguments, and a signature does not carry names.
    NamedArguments,
    /// The call has the wrong number of arguments; `Arity` owns that.
    ArityMismatch,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Outcome {
    Agree,
    /// What was declared and what was found, as a diagnostic shows them.
    Disagree {
        expected: String,
        actual: String,
    },
    Undecided(Undecided),
}

/// **One place a value meets a declared type**, and what was concluded.
#[derive(Debug, Clone)]
pub struct ValueRelation {
    /// The declaration the relation is written in.
    pub declaration: String,
    pub kind: RelationKind,
    /// Where the value (or the annotation) is.
    pub span: Span,
    /// What it is checked against: a callee, a field, a declaration.
    pub target: String,
    /// The argument or field position, where there is one.
    pub index: Option<usize>,
    pub outcome: Outcome,
    /// Where the expectation comes from, when it has a place in source: the
    /// unit it is written in, and where. A span is only meaningful against its
    /// own file, and a diagnostic's related spans are rendered against the
    /// file the diagnostic is in.
    pub declared_at: Option<(UnitId, Span)>,
    /// The boundary the relation sits on, in this unit: the call, the
    /// declaration whose result is checked, the binding. Charter §16.3 asks
    /// every diagnostic for one, and it is always in the file the diagnostic
    /// is about, where a callee's own declaration may not be.
    pub boundary: (Span, String),
}

// --- the typer ---------------------------------------------------------------------

/// What a call resolves to.
pub(crate) enum Target<'s> {
    /// `f(a, b)` and `m.f(a, b)`: every supplied argument is a parameter.
    Callable(&'s Signature),
    /// `x.f(a)`: `x` is the first parameter.
    Member(&'s Signature, ExprId),
    /// `Cart(lines, total)`: a record's fields, positionally.
    Record(DefId),
    /// `PositiveInt(1)`: an opaque type built from its representation.
    Opaque(DefId),
}

struct Typer<'a> {
    sigs: &'a Signatures,
    ws: &'a Workspace,
    at: UnitId,
    module: Option<&'a str>,
    decl: &'a Decl,
    body: &'a Body,
    /// Every name bound to a known type in this body. A cell, because typing
    /// a lambda against the function type it is passed as binds its
    /// parameters for the duration of that one question.
    locals: RefCell<BTreeMap<String, Ty>>,
    /// Names bound at more than one site. A flat environment cannot say which
    /// binding a use refers to, so their uses are unknown rather than guessed.
    shadowed: BTreeSet<String>,
    /// The expression each `|>` feeds, keyed by the call on its right.
    piped: BTreeMap<ExprId, ExprId>,
}

/// What a resolved call is checked against: its parameters, where each is
/// declared, and its result, all under the binder whose type parameters the
/// call instantiates.
struct Callee {
    name: String,
    binder: DefId,
    params: Vec<Option<TypeResolution>>,
    declared: Vec<Option<(UnitId, Span)>>,
    result: Option<Ty>,
}

/// What resolving and solving one call produced.
struct Solved {
    result: Ty,
    relations: Vec<ValueRelation>,
}

/// **The type the value relations give one expression, and its name.**
///
/// For an analysis that needs a type where no annotation says one: the
/// exhaustiveness check's scrutinee, when it is a call rather than a declared
/// name (`match get(w) { .. }`). `Ty::Unknown` where the typer does not know,
/// never a guess.
pub(crate) fn type_of(
    sigs: &Signatures,
    ws: &Workspace,
    at: UnitId,
    module: Option<&str>,
    decl: &Decl,
    body: &Body,
    e: ExprId,
) -> (Ty, String) {
    let typer = Typer::new(sigs, ws, at, module, decl, body);
    let around = typer.arms_around(e);
    let t = typer.with_bindings(&around, || typer.of(e));
    let name = typer.display(&t);
    (t, name)
}

/// What a call's written path names.
pub(crate) enum Named<'s> {
    /// The callee is not a path, or its path names nothing. `x.f(a)` may still
    /// be a member call.
    Nothing,
    /// It names something ambiguously, or names something a call cannot
    /// target.
    Refused,
    Target(Target<'s>),
}

/// **Which declaration a call's path names.** The one rule, with two readers:
/// the typer, and the handler backend (`backend::js`). A handler compiled to
/// call a different declaration than the one the checker checked would run
/// unchecked code, and nothing downstream would notice.
///
/// A dotted path resolves as a path. A bare name is a term first, and a type
/// only where no term has that name: `PositiveInt(1)` builds an opaque value,
/// and `Store(id)` calls the query `Store` even where a type `Store` exists
/// too (A-003).
pub(crate) fn named<'s>(
    sigs: &'s Signatures,
    ws: &Workspace,
    at: UnitId,
    body: &Body,
    callee: ExprId,
) -> Named<'s> {
    let path = path_of(body, callee);
    if path.is_empty() {
        return Named::Nothing;
    }
    let term = match path.contains('.') {
        true => ws.resolve_path(at, &path),
        false => ws.resolve_in(at, Namespace::Term, &path),
    };
    let def = match term {
        Resolution::Local(d) | Resolution::Imported { def: d, .. } => d,
        // Ambiguity is its own diagnostic; never a type verdict.
        Resolution::Ambiguous(_) => return Named::Refused,
        Resolution::Unresolved if !path.contains('.') => {
            match ws.resolve_in(at, Namespace::Type, &path) {
                Resolution::Local(d) | Resolution::Imported { def: d, .. } => d,
                _ => return Named::Nothing,
            }
        }
        Resolution::Unresolved => return Named::Nothing,
    };
    if let Some(sig) = sigs.by_def(def) {
        return Named::Target(Target::Callable(sig));
    }
    let Some(facts) = sigs.type_decl(def) else {
        return Named::Refused;
    };
    if facts.record.is_some() {
        return Named::Target(Target::Record(def));
    }
    if facts.representation.is_some() {
        return Named::Target(Target::Opaque(def));
    }
    Named::Refused
}

impl<'a> Typer<'a> {
    fn new(
        sigs: &'a Signatures,
        ws: &'a Workspace,
        at: UnitId,
        module: Option<&'a str>,
        decl: &'a Decl,
        body: &'a Body,
    ) -> Typer<'a> {
        let types = Types::of_body(sigs, decl, body, module);

        let mut sites: BTreeMap<String, usize> = BTreeMap::new();
        for p in &decl.params {
            *sites.entry(p.name.clone()).or_default() += 1;
        }
        for (_, pat, _) in body.pats.iter() {
            if let Pattern::Bind { name, .. } = pat {
                *sites.entry(name.clone()).or_default() += 1;
            }
        }
        let shadowed: BTreeSet<String> = sites
            .into_iter()
            .filter(|(_, n)| *n > 1)
            .map(|(name, _)| name)
            .collect();

        let mut piped = BTreeMap::new();
        for id in body.walk() {
            if let Expr::Binary {
                op: BinOp::Pipe,
                lhs,
                rhs,
            } = body.expr(id)
            {
                piped.insert(*rhs, *lhs);
            }
        }

        let typer = Typer {
            sigs,
            ws,
            at,
            module,
            decl,
            body,
            locals: RefCell::new(
                types
                    .bindings()
                    .iter()
                    .map(|(name, t)| (name.clone(), Ty::of(t)))
                    .collect(),
            ),
            shadowed,
            piped,
        };

        // **A binding typed with a callee's own `T` is no type.** `infer.rs`
        // types `let ys = List.filter(xs, ..)` by `filter`'s declared result,
        // `List<T>`, with nothing instantiating `T`; and the loop below skipped
        // every name already bound. So `ys` was `List<type parameter 0>`, and
        // a correct `List.sort_by(ys, compare)` was refused (PW0605) for a
        // mismatch the typer had made. Found 2026-09-25, writing kiokun's
        // ranking in Pleris. Such a binding is dropped, and the call is solved
        // below.
        let own = typer.own_def();
        typer
            .locals
            .borrow_mut()
            .retain(|_, t| !t.mentions_foreign_parameter(own));

        // An unannotated `let` takes its initialiser's type. Iterated, because
        // `let a = f()` then `let b = g(a)` needs `a` first; bounded, because
        // each round can only add bindings.
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
                let Pattern::Bind { name, .. } = body.pat(*pat) else {
                    continue;
                };
                if typer.locals.borrow().contains_key(name) {
                    continue;
                }
                let t = typer.of(*init);
                if !t.is_unknown() {
                    typer.locals.borrow_mut().insert(name.clone(), t);
                    added = true;
                }
            }
            if !added {
                break;
            }
        }
        typer
    }

    /// **The type of an expression**, holes included.
    fn of(&self, id: ExprId) -> Ty {
        match self.body.expr(id) {
            Expr::Literal(l) => Ty::Primitive(match l {
                Literal::Int(_) => Primitive::Int,
                Literal::Float(_) => Primitive::Float,
                Literal::Str(_) | Literal::UnterminatedStr(_) => Primitive::Str,
            }),
            Expr::Interpolated { .. } => Ty::Primitive(Primitive::Str),
            Expr::Name(n) => self.name(n),
            Expr::Field { base, name } => self.field(id, *base, name),
            Expr::Call { .. } => self.call(id).result,
            // `let store = query Store(id)` binds what the query produces when
            // it succeeds: the page renders `{store.name}`, and its loading and
            // failure states are the page's to render, not the binding's. The
            // same reading `infer.rs` gives `{#each}` over a query. Recorded
            // in docs/ASSUMPTIONS.md.
            Expr::Keyword {
                keyword, modifiers, ..
            } if matches!(keyword.as_str(), "query" | "subscription") && !modifiers.is_empty() => {
                match self.call(id).result {
                    Ty::Builtin(Builtin::Result, mut args) if args.len() == 2 => {
                        args.swap_remove(0)
                    }
                    other => other,
                }
            }
            Expr::Binary { op, lhs, rhs } => match op {
                BinOp::Cmp(_) | BinOp::And | BinOp::Or => Ty::Primitive(Primitive::Bool),
                BinOp::Add | BinOp::Sub | BinOp::Mul | BinOp::Div | BinOp::Rem => {
                    match (self.of(*lhs), self.of(*rhs)) {
                        (Ty::Primitive(p), Ty::Primitive(q))
                            if p == q && matches!(p, Primitive::Int | Primitive::Float) =>
                        {
                            Ty::Primitive(p)
                        }
                        _ => Ty::Unknown,
                    }
                }
                BinOp::Pipe => self.of(*rhs),
                BinOp::Transition | BinOp::Assign => Ty::Unknown,
            },
            Expr::Unary { op, operand } => match op {
                UnOp::Not => Ty::Primitive(Primitive::Bool),
                UnOp::Neg => match self.of(*operand) {
                    Ty::Primitive(p) if matches!(p, Primitive::Int | Primitive::Float) => {
                        Ty::Primitive(p)
                    }
                    _ => Ty::Unknown,
                },
            },
            // `e?` is `e`'s success value.
            Expr::Try { value } => match self.of(*value) {
                Ty::Builtin(Builtin::Result, mut args) if args.len() == 2 => args.swap_remove(0),
                Ty::Builtin(Builtin::Option, mut args) if args.len() == 1 => args.swap_remove(0),
                _ => Ty::Unknown,
            },
            Expr::Cast { ty, .. } => crate::resolved::written_in_body(self.body, *ty)
                .and_then(|t| {
                    self.sigs
                        .resolve_type(self.module, self.decl, &t, self.body.expr_span(id))
                        .resolved()
                        .map(Ty::of)
                })
                .unwrap_or(Ty::Unknown),
            Expr::Block { stmts } => match stmts.last() {
                Some(last) if !matches!(self.body.expr(*last), Expr::Let { .. }) => self.of(*last),
                _ => Ty::Unknown,
            },
            Expr::If {
                then, els: Some(e), ..
            } => join(self.of(*then), self.of(*e)),
            // Each arm's body, with its pattern's names bound to the payload
            // types the scrutinee's type gives them: `entry` in
            // `Some(entry) => ..` is the option's `T`.
            Expr::Match { scrutinee, arms } => {
                let st = self.of(*scrutinee);
                arms.iter()
                    .map(|a| self.with_bindings(&self.arm_bindings(&st, a.pat), || self.of(a.body)))
                    .reduce(join)
                    .unwrap_or(Ty::Unknown)
            }
            Expr::Record { name: Some(_), .. } => self.construct(id).result,
            Expr::List { items } => Ty::Builtin(
                Builtin::List,
                vec![
                    items
                        .iter()
                        .map(|i| self.of(*i))
                        .reduce(join)
                        .unwrap_or(Ty::Unknown),
                ],
            ),
            _ => Ty::Unknown,
        }
    }

    fn name(&self, n: &str) -> Ty {
        if self.shadowed.contains(n) {
            return Ty::Unknown;
        }
        if let Some(t) = self.locals.borrow().get(n) {
            return t.clone();
        }
        // A declared callable named as a value: `List.map(xs, line_total)`.
        let term = self.ws.resolve_in(self.at, Namespace::Term, n);
        if let Resolution::Local(d) | Resolution::Imported { def: d, .. } = term
            && let Some(sig) = self.sigs.by_def(d)
        {
            return function_type(sig);
        }
        // The language's own values, only where the program has not declared
        // something of that name.
        let own = matches!(term, Resolution::Unresolved);
        match n {
            "true" | "false" if own => Ty::Primitive(Primitive::Bool),
            "None" if own => Ty::Builtin(Builtin::Option, vec![Ty::Unknown]),
            _ => Ty::Unknown,
        }
    }

    /// The names an arm's pattern binds, typed from the scrutinee's type:
    /// `Some(x)` binds the option's `T`; `Ok(x)` and `Err(e)` the result's two
    /// sides. Only a name bound directly under one of the language's own
    /// cases, where the program declares nothing of that name; a declared
    /// variant's payload, and any nested pattern, bind nothing here and stay
    /// unknown, which every relation reads as undecided.
    fn arm_bindings(&self, scrutinee: &Ty, pat: crate::hir::PatternId) -> Vec<(String, Ty)> {
        let Pattern::Ctor { path, args } = self.body.pat(pat) else {
            return Vec::new();
        };
        let [inner] = args.as_slice() else {
            return Vec::new();
        };
        let Pattern::Bind { name, .. } = self.body.pat(*inner) else {
            return Vec::new();
        };
        let own = matches!(
            self.ws.resolve_in(self.at, Namespace::Term, path),
            Resolution::Unresolved
        );
        let payload = match (path.as_str(), scrutinee) {
            ("Some", Ty::Builtin(Builtin::Option, a)) if own => a.first(),
            ("Ok", Ty::Builtin(Builtin::Result, a)) if own => a.first(),
            ("Err", Ty::Builtin(Builtin::Result, a)) if own => a.get(1),
            _ => None,
        };
        match payload {
            Some(t) if !self.shadowed.contains(name) => vec![(name.clone(), t.clone())],
            _ => Vec::new(),
        }
    }

    /// Every name bound by a match arm that encloses `target`, outermost
    /// first, each typed in the scope its own match sees.
    fn arms_around(&self, target: ExprId) -> Vec<(String, Ty)> {
        let mut parent: BTreeMap<ExprId, ExprId> = BTreeMap::new();
        for e in self.body.walk() {
            for c in self.body.children(e) {
                parent.insert(c, e);
            }
        }
        let mut path = vec![target];
        while let Some(p) = parent.get(path.last().expect("non-empty")) {
            path.push(*p);
        }
        path.reverse();
        let mut bound: Vec<(String, Ty)> = Vec::new();
        for pair in path.windows(2) {
            let (outer, inner) = (pair[0], pair[1]);
            let Expr::Match { scrutinee, arms } = self.body.expr(outer) else {
                continue;
            };
            let Some(arm) = arms.iter().find(|a| a.body == inner) else {
                continue;
            };
            let st = self.with_bindings(&bound, || self.of(*scrutinee));
            bound.extend(self.arm_bindings(&st, arm.pat));
        }
        bound
    }

    /// Run `f` with these names bound, restoring what they meant before.
    fn with_bindings<R>(&self, binds: &[(String, Ty)], f: impl FnOnce() -> R) -> R {
        let mut saved = Vec::new();
        for (name, ty) in binds {
            let old = self.locals.borrow_mut().insert(name.clone(), ty.clone());
            saved.push((name.clone(), old));
        }
        let result = f();
        for (name, old) in saved.into_iter().rev() {
            match old {
                Some(t) => self.locals.borrow_mut().insert(name, t),
                None => self.locals.borrow_mut().remove(&name),
            };
        }
        result
    }

    /// **A policy's term roots**: the target and transition of an optimistic
    /// clause, a resource's `acquire` and `release`, a painter. They are
    /// separate trees in the body's arena, reached from no statement, so the
    /// body walk does not see them — `optimistic Cart(..) as cart =>
    /// Carts.with_line(cart, item, quantity)` is an ordinary call, and it was
    /// checked by nothing until this.
    ///
    /// A binder takes the type its header gives it where the header gives
    /// one: an optimistic transition's binder is the target's value
    /// (ADR-0025), a `release(h)`'s is what the resource declares it
    /// produces. Any other binder is bound as unknown, so it can never borrow
    /// the type of a parameter that happens to share its name.
    fn term_relations(&self, own_result: Option<Ty>, out: &mut Vec<ValueRelation>) {
        use crate::hir::ExecutionContext as Cx;
        let success = |t: Ty| match t {
            Ty::Builtin(Builtin::Result, mut args) if args.len() == 2 => args.swap_remove(0),
            other => other,
        };
        for (policy, root) in self.decl.term_roots() {
            let bound = match root.context {
                Cx::OptimisticTransition => policy
                    .roots
                    .iter()
                    .find(|r| r.context == Cx::TargetSelection)
                    .map(|t| success(self.of(t.root)))
                    .unwrap_or(Ty::Unknown),
                Cx::Release => own_result.clone().map(success).unwrap_or(Ty::Unknown),
                _ => Ty::Unknown,
            };
            let binds: Vec<(String, Ty)> = root
                .binders
                .iter()
                .map(|(name, _)| (name.clone(), bound.clone()))
                .collect();
            self.with_bindings(&binds, || self.relations_from(root.root, out));
        }
    }

    /// A lambda, typed against the function type it is passed as: its
    /// parameters take that type's parameter types, and its type is theirs
    /// with its body's type as the result. Without an expected function type
    /// a lambda has no type here — an unannotated parameter is not a claim.
    fn lambda(&self, params: &[crate::hir::PatternId], body: ExprId, expected: &[Ty]) -> Ty {
        let Some(names) = lambda_names(self.body, params) else {
            return Ty::Unknown;
        };
        let Some((_, expected_params)) = expected.split_last() else {
            return Ty::Unknown;
        };
        if names.len() != expected_params.len() {
            // A callback of the wrong arity is a different function type.
            let mut shape = vec![Ty::Unknown; names.len()];
            shape.push(Ty::Unknown);
            return Ty::Builtin(Builtin::Function, shape);
        }
        let mut saved = Vec::new();
        for (name, ty) in names.iter().zip(expected_params) {
            let old = self.locals.borrow_mut().insert(name.clone(), ty.clone());
            saved.push((name.clone(), old));
        }
        let result = self.of(body);
        for (name, old) in saved {
            match old {
                Some(t) => self.locals.borrow_mut().insert(name, t),
                None => self.locals.borrow_mut().remove(&name),
            };
        }
        let mut shape = expected_params.to_vec();
        shape.push(result);
        Ty::Builtin(Builtin::Function, shape)
    }

    /// `base.name`: a record field, read through the receiver's type.
    fn field(&self, id: ExprId, base: ExprId, name: &str) -> Ty {
        // A module path: `decode.string` named as a value is that function.
        if self.resolves_as_path(id) {
            let path = path_of(self.body, id);
            return match self.ws.resolve_path_in(self.at, Namespace::Term, &path) {
                Resolution::Local(d) | Resolution::Imported { def: d, .. } => self
                    .sigs
                    .by_def(d)
                    .map(function_type)
                    .unwrap_or(Ty::Unknown),
                _ => Ty::Unknown,
            };
        }
        let receiver = self.of(base);
        let Some(sig) = receiver
            .receiver()
            .and_then(|r| self.sigs.member_by(r, name))
        else {
            // An opaque type's `.value`, read in its own module (ADR-0048).
            return match self.representation(&receiver, name) {
                Some((def, rep)) if def.unit == self.at => rep,
                _ => Ty::Unknown,
            };
        };
        // A callable member whose only parameter is the receiver is read as a
        // property — `self.style`, `snapshot.value` — exactly as `infer.rs`
        // resolves it. One with more parameters, named without a call, is a
        // function value, which has no type here.
        if self.sigs.by_def(sig.definition).is_some() {
            let [Some(TypeResolution::Resolved(recv))] = sig.params.as_slice() else {
                return Ty::Unknown;
            };
            let Some(TypeResolution::Resolved(result)) = &sig.returns else {
                return Ty::Unknown;
            };
            let mut s = Subst::default();
            let binder = sig.definition;
            if unify(&mut s, &Ty::of(recv).instantiate(binder), &receiver) == Verdict::Disagree {
                return Ty::Unknown;
            }
            return s.close(&Ty::of(result).instantiate(binder));
        }
        let (Some(Some(TypeResolution::Resolved(recv))), Some(result)) =
            (sig.params.first(), sig.result())
        else {
            return Ty::Unknown;
        };
        // `Box<Int>.v` where `v: T`: the field's type under the receiver's
        // arguments, by unifying the declared receiver with the actual one.
        let mut s = Subst::default();
        let owner = sig.definition;
        unify(&mut s, &Ty::of(recv).instantiate(owner), &receiver);
        s.close(&Ty::of(result).instantiate(owner))
    }

    fn resolves_as_path(&self, id: ExprId) -> bool {
        let path = path_of(self.body, id);
        !path.is_empty()
            && path.contains('.')
            && !matches!(self.ws.resolve_path(self.at, &path), Resolution::Unresolved)
    }

    /// Resolve a call's target by identity. `None` when it reaches nothing
    /// this analysis can describe — which is silence, not a verdict.
    fn target(&self, call: ExprId, callee: ExprId) -> Option<Target<'a>> {
        match named(self.sigs, self.ws, self.at, self.body, callee) {
            Named::Target(t) => return Some(t),
            Named::Refused => return None,
            Named::Nothing => {}
        }
        // `x.f(a)`, where `x` is a value whose type declares `f`.
        let Expr::Field { base, name } = self.body.expr(callee) else {
            return None;
        };
        if self.piped.contains_key(&call) {
            return None;
        }
        let receiver = self.of(*base).receiver()?;
        let sig = self.sigs.member_by(receiver, name)?;
        self.sigs.by_def(sig.definition)?;
        Some(Target::Member(sig, *base))
    }

    /// **Resolve, instantiate and solve one call.**
    fn call(&self, id: ExprId) -> Solved {
        let unknown = Solved {
            result: Ty::Unknown,
            relations: Vec::new(),
        };
        let callee_span = match self.body.expr(id) {
            Expr::Call { callee, .. } => self.body.expr_span(*callee),
            _ => self.body.expr_span(id),
        };
        let (target, args): (Option<Target<'a>>, Vec<(Option<String>, ExprId)>) =
            match self.body.expr(id) {
                Expr::Call { callee, args } => {
                    if let Some(t) = self.intrinsic(*callee, args) {
                        return Solved {
                            result: t,
                            relations: Vec::new(),
                        };
                    }
                    (
                        self.target(id, *callee),
                        args.iter().map(|a| (a.name.clone(), a.value)).collect(),
                    )
                }
                // `query Menu(id)` invokes the query `Menu`.
                Expr::Keyword {
                    modifiers, args, ..
                } => {
                    let target = match self.ws.resolve_in(self.at, Namespace::Term, &modifiers[0]) {
                        Resolution::Local(d) | Resolution::Imported { def: d, .. } => {
                            self.sigs.by_def(d).map(Target::Callable)
                        }
                        _ => None,
                    };
                    (target, args.iter().map(|a| (None, *a)).collect())
                }
                _ => return unknown,
            };
        let Some(target) = target else {
            return unknown;
        };

        // What is supplied, in parameter order: the receiver of a member call,
        // then a piped value, then the written arguments.
        let mut supplied: Vec<(Option<String>, ExprId)> = Vec::new();
        if let Target::Member(_, base) = &target {
            supplied.push((None, *base));
        }
        if let Some(p) = self.piped.get(&id) {
            supplied.push((None, *p));
        }
        supplied.extend(args);

        let Callee {
            name,
            binder,
            params,
            declared,
            result,
        } = match &target {
            Target::Callable(sig) | Target::Member(sig, _) => Callee {
                name: sig.path.clone(),
                binder: sig.definition,
                params: sig.params.clone(),
                declared: sig
                    .params
                    .iter()
                    .map(|p| {
                        p.as_ref()
                            .and_then(TypeResolution::resolved)
                            .map(|t| (sig.definition.unit, t.span()))
                    })
                    .collect(),
                // No annotation: the call's value is not stated.
                result: sig.returns.as_ref().map(|r| match r {
                    TypeResolution::Resolved(t) => Ty::of(t),
                    _ => Ty::Unknown,
                }),
            },
            Target::Record(def) => {
                let fields = self
                    .sigs
                    .type_decl(*def)
                    .and_then(|f| f.record.clone())
                    .unwrap_or_default();
                Callee {
                    name: self.display_def(*def),
                    binder: *def,
                    params: fields.iter().map(|(_, r)| Some(r.clone())).collect(),
                    declared: fields
                        .iter()
                        .map(|(_, r)| r.resolved().map(|t| (def.unit, t.span())))
                        .collect(),
                    result: Some(self.applied(*def)),
                }
            }
            Target::Opaque(def) => {
                let rep = self
                    .sigs
                    .type_decl(*def)
                    .and_then(|f| f.representation.clone());
                let span = rep
                    .as_ref()
                    .and_then(|r| r.resolved().map(|t| (def.unit, t.span())));
                Callee {
                    name: self.display_def(*def),
                    binder: *def,
                    params: vec![rep],
                    declared: vec![span],
                    result: Some(self.applied(*def)),
                }
            }
        };
        let field_names: Option<Vec<String>> = match &target {
            Target::Record(def) => self
                .sigs
                .type_decl(*def)
                .and_then(|f| f.record.as_ref())
                .map(|r| r.iter().map(|(n, _)| n.clone()).collect()),
            _ => None,
        };
        let kind = match &target {
            Target::Record(_) | Target::Opaque(_) => RelationKind::Field,
            _ => RelationKind::Argument,
        };

        let mut relations = Vec::new();
        let mut s = Subst::default();
        let result = result.map(|r| r.instantiate(binder));
        let span = self.body.expr_span(id);

        // A declaration with no parameters and a call with no arguments has
        // nothing to relate — a type, an event.
        if params.is_empty() && supplied.is_empty() {
            return Solved {
                result: result.map(|r| s.close(&r)).unwrap_or(Ty::Unknown),
                relations,
            };
        }
        let arity_ok = supplied.len() == params.len();
        relations.push(ValueRelation {
            declaration: self.decl.name.clone(),
            kind: RelationKind::Arity,
            span: span.clone(),
            target: name.clone(),
            index: None,
            outcome: match arity_ok {
                true => Outcome::Agree,
                false => Outcome::Disagree {
                    expected: params.len().to_string(),
                    actual: supplied.len().to_string(),
                },
            },
            declared_at: None,
            boundary: (
                callee_span.clone(),
                format!(
                    "`{name}` is declared with {} parameter{}",
                    params.len(),
                    if params.len() == 1 { "" } else { "s" }
                ),
            ),
        });
        let named = supplied.iter().any(|(n, _)| n.is_some());

        for (i, (_, value)) in supplied.iter().enumerate() {
            let expected = params.get(i).cloned().flatten();
            let outcome = if !arity_ok {
                Outcome::Undecided(Undecided::ArityMismatch)
            } else if named {
                Outcome::Undecided(Undecided::NamedArguments)
            } else {
                match &expected {
                    Some(TypeResolution::Resolved(t)) => {
                        let e = Ty::of(t).instantiate(binder);
                        // A lambda is typed against the function type it is
                        // passed as, under what earlier arguments determined.
                        let a = match (self.body.expr(*value), s.resolve(&e)) {
                            (
                                Expr::Lambda {
                                    descriptor: None,
                                    params,
                                    body,
                                },
                                Ty::Builtin(Builtin::Function, fargs),
                            ) => {
                                let known: Vec<Ty> = fargs.iter().map(|f| s.close(f)).collect();
                                self.lambda(params, *body, &known)
                            }
                            _ => self.of(*value),
                        };
                        match unify(&mut s, &e, &a) {
                            Verdict::Agree => Outcome::Agree,
                            Verdict::Undecided => Outcome::Undecided(Undecided::Unknown),
                            Verdict::Disagree => Outcome::Disagree {
                                expected: self.display(&s.close(&e)),
                                actual: self.display(&a),
                            },
                        }
                    }
                    Some(_) => Outcome::Undecided(Undecided::ExpectedUnresolved),
                    // A parameter the author left unannotated is not a claim.
                    None => Outcome::Undecided(Undecided::Unknown),
                }
            };
            relations.push(ValueRelation {
                declaration: self.decl.name.clone(),
                kind,
                span: self.body.expr_span(*value),
                target: match (&field_names, kind) {
                    (Some(names), RelationKind::Field) => {
                        format!("{name}.{}", names.get(i).map(String::as_str).unwrap_or("?"))
                    }
                    _ => name.clone(),
                },
                index: Some(i),
                outcome,
                declared_at: declared.get(i).cloned().flatten(),
                boundary: (span.clone(), format!("in this call to `{name}`")),
            });
        }
        Solved {
            result: result.map(|r| s.close(&r)).unwrap_or(Ty::Unknown),
            relations,
        }
    }

    /// `Ok(x)`, `Err(e)`, `Some(x)` — the language's own constructors, where the
    /// program has not declared a term of that name.
    fn intrinsic(&self, callee: ExprId, args: &[crate::hir::Arg]) -> Option<Ty> {
        let Expr::Name(n) = self.body.expr(callee) else {
            return None;
        };
        if !matches!(n.as_str(), "Ok" | "Err" | "Some")
            || !matches!(
                self.ws.resolve_in(self.at, Namespace::Term, n),
                Resolution::Unresolved
            )
        {
            return None;
        }
        let [arg] = args else {
            return Some(Ty::Unknown);
        };
        let v = self.of(arg.value);
        Some(match n.as_str() {
            "Ok" => Ty::Builtin(Builtin::Result, vec![v, Ty::Unknown]),
            "Err" => Ty::Builtin(Builtin::Result, vec![Ty::Unknown, v]),
            _ => Ty::Builtin(Builtin::Option, vec![v]),
        })
    }

    /// A type declaration applied to fresh variables for its parameters.
    fn applied(&self, def: DefId) -> Ty {
        let arity = self.ws.type_arity(def).unwrap_or(0);
        Ty::Nominal(
            def,
            (0..arity as u32)
                .map(|index| Ty::Parameter { binder: def, index })
                .collect(),
        )
    }

    /// `Cart { lines: .., total: .. }`: a record built by field name.
    fn construct(&self, id: ExprId) -> Solved {
        let unknown = Solved {
            result: Ty::Unknown,
            relations: Vec::new(),
        };
        let Expr::Record {
            name: Some(name),
            fields,
        } = self.body.expr(id)
        else {
            return unknown;
        };
        let found = match name.contains('.') {
            true => self.ws.resolve_path_in(self.at, Namespace::Type, name),
            false => self.ws.resolve_in(self.at, Namespace::Type, name),
        };
        let def = match found {
            Resolution::Local(d) | Resolution::Imported { def: d, .. } => d,
            _ => return unknown,
        };
        let Some(declared) = self.sigs.type_decl(def).and_then(|f| f.record.as_ref()) else {
            return unknown;
        };
        let mut s = Subst::default();
        let mut relations = Vec::new();
        for (i, init) in fields.iter().enumerate() {
            let Some((index, (_, expected))) = declared
                .iter()
                .enumerate()
                .find(|(_, (n, _))| *n == init.name)
            else {
                continue;
            };
            let _ = i;
            let actual = match init.value {
                Some(v) => self.of(v),
                // `Point { x, y }` — the shorthand names a binding.
                None => self.name(&init.name),
            };
            let outcome = match expected {
                TypeResolution::Resolved(t) => {
                    let e = Ty::of(t).instantiate(def);
                    match unify(&mut s, &e, &actual) {
                        Verdict::Agree => Outcome::Agree,
                        Verdict::Undecided => Outcome::Undecided(Undecided::Unknown),
                        Verdict::Disagree => Outcome::Disagree {
                            expected: self.display(&s.close(&e)),
                            actual: self.display(&actual),
                        },
                    }
                }
                _ => Outcome::Undecided(Undecided::ExpectedUnresolved),
            };
            relations.push(ValueRelation {
                declaration: self.decl.name.clone(),
                kind: RelationKind::Field,
                span: match init.value {
                    Some(v) => self.body.expr_span(v),
                    None => init.span.clone(),
                },
                target: format!("{}.{}", self.display_def(def), init.name),
                index: Some(index),
                outcome,
                declared_at: expected.resolved().map(|t| (def.unit, t.span())),
                boundary: (
                    self.body.expr_span(id),
                    format!("in this construction of `{}`", self.display_def(def)),
                ),
            });
        }
        Solved {
            result: s.close(&self.applied(def).instantiate(def)),
            relations,
        }
    }

    // --- display ---------------------------------------------------------------------

    /// A type as a diagnostic shows it. Short names, qualified only where two
    /// declarations in one message would otherwise read the same — the case
    /// this checker exists to tell apart.
    fn display(&self, t: &Ty) -> String {
        match t {
            Ty::Primitive(p) => p.name().to_string(),
            Ty::Builtin(Builtin::Function, args) => {
                let (result, params) = match args.split_last() {
                    Some((r, p)) => (self.display(r), p),
                    None => ("?".to_string(), &args[..]),
                };
                format!(
                    "fn({}) -> {result}",
                    params
                        .iter()
                        .map(|a| self.display(a))
                        .collect::<Vec<_>>()
                        .join(", ")
                )
            }
            Ty::Builtin(b, args) => format!(
                "{}<{}>",
                b.name(),
                args.iter()
                    .map(|a| self.display(a))
                    .collect::<Vec<_>>()
                    .join(", ")
            ),
            Ty::Nominal(d, args) => {
                let base = self.display_def(*d);
                match args.is_empty() {
                    true => base,
                    false => format!(
                        "{base}<{}>",
                        args.iter()
                            .map(|a| self.display(a))
                            .collect::<Vec<_>>()
                            .join(", ")
                    ),
                }
            }
            Ty::Parameter { binder, index } => {
                let name = self
                    .decl
                    .type_params
                    .get(*index as usize)
                    .filter(|_| self.own_def() == Some(*binder))
                    .cloned();
                name.unwrap_or_else(|| format!("type parameter {index}"))
            }
            Ty::Var(_) | Ty::Unknown => "?".to_string(),
        }
    }

    /// The declaration's qualified path. The qualification is kept: two
    /// `SessionId`s are the reason this checker exists, and a message showing
    /// `SessionId` against `SessionId` would describe a defect it cannot name.
    fn display_def(&self, d: DefId) -> String {
        self.sigs
            .path_of(d)
            .map(str::to_string)
            .unwrap_or_else(|| "?".to_string())
    }

    fn own_def(&self) -> Option<DefId> {
        let ns = Namespace::of(self.decl.kind)?;
        match self.ws.resolve_in(self.at, ns, &self.decl.name) {
            Resolution::Local(d) => Some(d),
            _ => None,
        }
    }

    // --- the relations -----------------------------------------------------------------

    /// Every place in this body a value meets a declared type.
    fn relations(&self, out: &mut Vec<ValueRelation>) {
        self.relations_from(self.body.root, out);
    }

    /// The same relations, over one tree: the body, or a policy's term root.
    fn relations_from(&self, root: ExprId, out: &mut Vec<ValueRelation>) {
        for id in self.body.walk_from(root) {
            match self.body.expr(id) {
                Expr::Call { .. } => out.extend(self.call(id).relations),
                Expr::Keyword {
                    keyword, modifiers, ..
                } if matches!(keyword.as_str(), "query" | "subscription")
                    && !modifiers.is_empty() =>
                {
                    out.extend(self.call(id).relations)
                }
                Expr::Record { name: Some(_), .. } => out.extend(self.construct(id).relations),
                Expr::Field { base, name } => out.extend(self.member(id, *base, name)),
                // An operator's operands, and a condition. Until 2026-09-25 a
                // comparison was typed `Bool` whatever it compared, so
                // `1 == "a"` checked, and the backend was the first to refuse.
                Expr::Binary { op, lhs, rhs } => out.extend(self.operands(id, op, *lhs, *rhs)),
                Expr::Unary { op, operand } => {
                    let (what, expected) = match op {
                        UnOp::Not => ("the operand of `!`", Some(Ty::Primitive(Primitive::Bool))),
                        UnOp::Neg => ("the operand of `-`", None),
                    };
                    out.push(match expected {
                        Some(t) => self.operand(id, *operand, what, &t),
                        None => self.number(id, *operand, what),
                    });
                }
                Expr::If { cond, .. } => out.push(self.operand(
                    id,
                    *cond,
                    "the condition of `if`",
                    &Ty::Primitive(Primitive::Bool),
                )),
                Expr::Let {
                    pat: Some(pat),
                    ty: Some(ty),
                    init: Some(init),
                } => {
                    let Some(written) = crate::resolved::written_in_body(self.body, *ty) else {
                        continue;
                    };
                    let span = self.body.expr_span(*init);
                    let declared =
                        self.sigs
                            .resolve_type(self.module, self.decl, &written, span.clone());
                    let name = match self.body.pat(*pat) {
                        Pattern::Bind { name, .. } => name.clone(),
                        _ => "_".to_string(),
                    };
                    let boundary = (
                        self.body.expr_span(id),
                        format!("`{name}` is annotated in this binding"),
                    );
                    out.push(self.relate(
                        RelationKind::Binding,
                        &declared,
                        self.of(*init),
                        name,
                        span,
                        boundary,
                    ));
                }
                _ => {}
            }
        }
    }

    /// **`value.name` names a member the value's type has** (PW0610,
    /// ADR-0048): a record's field, or a declaration whose first parameter
    /// takes the type, read as a property or called as a method. Until
    /// 2026-09-25 a member no type had was unknown, so A-015 read `box.x`
    /// from a snapshot of a `Rect`, which has no `x`, and every analysis
    /// after it read nothing.
    ///
    /// Not a member: a path through a module (`List.map`, a name, ADR-0047's),
    /// and a unit on a numeric literal (`900.px`, `30.seconds`), which is a
    /// dimensioned literal. A value whose type is not known is undecided.
    fn member(&self, id: ExprId, base: ExprId, name: &str) -> Option<ValueRelation> {
        if self.resolves_as_path(id)
            || matches!(
                self.body.expr(base),
                Expr::Literal(Literal::Int(_) | Literal::Float(_))
            )
        {
            return None;
        }
        let receiver = self.of(base);
        let outcome = match receiver.receiver() {
            None => Outcome::Undecided(Undecided::Unknown),
            Some(r) if self.sigs.member_by(r, name).is_some() => Outcome::Agree,
            Some(_) => match self.representation(&receiver, name) {
                Some((def, _)) if def.unit == self.at => Outcome::Agree,
                Some(_) => Outcome::Disagree {
                    expected: PRIVATE_REPRESENTATION.to_string(),
                    actual: self.display(&receiver),
                },
                // `store.name` on an `Option<Store>`: the contents have the
                // member and the container does not. PW0600's invariant, an
                // optional value used as its contents.
                None if self.contents_have(&receiver, name) => Outcome::Disagree {
                    expected: OPTIONAL_CONTENTS.to_string(),
                    actual: self.display(&receiver),
                },
                None => Outcome::Disagree {
                    expected: format!("a member `{name}`"),
                    actual: self.display(&receiver),
                },
            },
        };
        Some(ValueRelation {
            declaration: self.decl.name.clone(),
            kind: RelationKind::Member,
            span: self.body.expr_span(id),
            target: name.to_string(),
            index: None,
            outcome,
            declared_at: None,
            boundary: (self.body.expr_span(base), "the value read".to_string()),
        })
    }

    /// Does the `Some` or `Ok` side of an `Option` or a `Result` have the
    /// member `name`?
    fn contents_have(&self, receiver: &Ty, name: &str) -> bool {
        let Ty::Builtin(Builtin::Option | Builtin::Result, args) = receiver else {
            return false;
        };
        args.first()
            .and_then(Ty::receiver)
            .is_some_and(|r| self.sigs.member_by(r, name).is_some())
    }

    /// **An opaque type's representation, as `.value`** (ADR-0048): the
    /// declaring type and the representation under the receiver's
    /// arguments, when `receiver` is an opaque type that declares no member
    /// of that name. Who may read it is the caller's question: only the
    /// module that declares the type, for which it is not opaque.
    fn representation(&self, receiver: &Ty, name: &str) -> Option<(DefId, Ty)> {
        let Ty::Nominal(def, _) = receiver else {
            return None;
        };
        if name != "value" {
            return None;
        }
        let TypeResolution::Resolved(rep) = self.sigs.type_decl(*def)?.representation.as_ref()?
        else {
            return None;
        };
        let mut s = Subst::default();
        unify(&mut s, &self.applied(*def).instantiate(*def), receiver);
        Some((*def, s.close(&Ty::of(rep).instantiate(*def))))
    }

    /// **An operator's operands** (PW0609). A logical operator takes `Bool`s;
    /// a comparison, two values of one type; arithmetic, two `Int`s or two
    /// `Float`s. Which operators a type supports beyond that is the backend's
    /// to refuse, not a type disagreement.
    fn operands(&self, id: ExprId, op: &BinOp, lhs: ExprId, rhs: ExprId) -> Vec<ValueRelation> {
        let sym = match op {
            BinOp::Add => "+",
            BinOp::Sub => "-",
            BinOp::Mul => "*",
            BinOp::Div => "/",
            BinOp::Rem => "%",
            BinOp::Cmp(c) => c.as_str(),
            BinOp::And => "&",
            BinOp::Or => "|",
            BinOp::Assign => return self.assignment(id, lhs, rhs),
            BinOp::Pipe | BinOp::Transition => return Vec::new(),
        };
        let left = format!("the left side of `{sym}`");
        let right = format!("the right side of `{sym}`, like its left,");
        match op {
            BinOp::And | BinOp::Or => {
                let b = Ty::Primitive(Primitive::Bool);
                vec![
                    self.operand(id, lhs, &left, &b),
                    self.operand(id, rhs, &format!("the right side of `{sym}`"), &b),
                ]
            }
            BinOp::Cmp(_) => vec![self.operand(id, rhs, &right, &self.of(lhs))],
            _ => vec![
                self.number(id, lhs, &left),
                self.operand(id, rhs, &right, &self.of(lhs)),
            ],
        }
    }

    /// **`x = e`**: `e` has the type `x` holds (ADR-0051). An assignment to a
    /// field, or to a name whose type is not known, relates nothing here.
    fn assignment(&self, id: ExprId, lhs: ExprId, rhs: ExprId) -> Vec<ValueRelation> {
        let Expr::Name(x) = self.body.expr(lhs) else {
            return Vec::new();
        };
        let declared = self.name(x);
        let actual = self.of(rhs);
        let mut s = Subst::default();
        let outcome = match unify(&mut s, &declared, &actual) {
            Verdict::Agree => Outcome::Agree,
            Verdict::Undecided => Outcome::Undecided(Undecided::Unknown),
            Verdict::Disagree => Outcome::Disagree {
                expected: self.display(&declared),
                actual: self.display(&actual),
            },
        };
        vec![ValueRelation {
            declaration: self.decl.name.clone(),
            kind: RelationKind::Assignment,
            span: self.body.expr_span(rhs),
            target: x.clone(),
            index: None,
            outcome,
            declared_at: None,
            boundary: (self.body.expr_span(id), format!("`{x}` is assigned here")),
        }]
    }

    /// One operand against the type it takes.
    fn operand(&self, id: ExprId, value: ExprId, what: &str, expected: &Ty) -> ValueRelation {
        let actual = self.of(value);
        let mut s = Subst::default();
        let outcome = match unify(&mut s, expected, &actual) {
            Verdict::Agree => Outcome::Agree,
            Verdict::Undecided => Outcome::Undecided(Undecided::Unknown),
            Verdict::Disagree => Outcome::Disagree {
                expected: self.display(expected),
                actual: self.display(&actual),
            },
        };
        self.operand_relation(id, value, what, outcome)
    }

    /// An operand arithmetic takes: an `Int` or a `Float`.
    fn number(&self, id: ExprId, value: ExprId, what: &str) -> ValueRelation {
        let actual = self.of(value);
        let outcome = match &actual {
            Ty::Primitive(Primitive::Int | Primitive::Float) => Outcome::Agree,
            Ty::Unknown | Ty::Parameter { .. } | Ty::Var(_) => {
                Outcome::Undecided(Undecided::Unknown)
            }
            // A list, a record or a string is never a number, whatever its
            // arguments are.
            other => Outcome::Disagree {
                expected: "Int or Float".to_string(),
                actual: self.display(other),
            },
        };
        self.operand_relation(id, value, what, outcome)
    }

    fn operand_relation(
        &self,
        id: ExprId,
        value: ExprId,
        what: &str,
        outcome: Outcome,
    ) -> ValueRelation {
        ValueRelation {
            declaration: self.decl.name.clone(),
            kind: RelationKind::Operand,
            span: self.body.expr_span(value),
            target: what.to_string(),
            index: None,
            outcome,
            declared_at: None,
            boundary: (self.body.expr_span(id), "the operator".to_string()),
        }
    }

    /// Relate one value to one declared type, outside any call.
    #[allow(clippy::too_many_arguments)]
    fn relate(
        &self,
        kind: RelationKind,
        declared: &TypeResolution,
        actual: Ty,
        target: String,
        span: Span,
        boundary: (Span, String),
    ) -> ValueRelation {
        let outcome = match declared {
            TypeResolution::Resolved(t) => {
                let e = Ty::of(t);
                let a = actual;
                let mut s = Subst::default();
                match unify(&mut s, &e, &a) {
                    Verdict::Agree => Outcome::Agree,
                    Verdict::Undecided => Outcome::Undecided(Undecided::Unknown),
                    Verdict::Disagree => Outcome::Disagree {
                        expected: self.display(&e),
                        actual: self.display(&a),
                    },
                }
            }
            _ => Outcome::Undecided(Undecided::ExpectedUnresolved),
        };
        ValueRelation {
            declaration: self.decl.name.clone(),
            kind,
            span,
            target,
            index: None,
            outcome,
            declared_at: declared.resolved().map(|t| (self.at, t.span())),
            boundary,
        }
    }

    /// **Every expression whose value the body returns.** The tail, seen
    /// through blocks and the branches of an `if`/`match`, and each `return`.
    ///
    /// `return e` lowers as two statements, `Name("return")` then `e`, so the
    /// statement after one is a returned value. A `return` inside a lambda
    /// returns from the lambda and is not collected.
    fn result_sites(&self) -> Vec<ExprId> {
        let mut out = Vec::new();
        self.tail_sites(self.body.root, &mut out);
        let mut lambdas: BTreeSet<ExprId> = BTreeSet::new();
        for id in self.body.walk() {
            if let Expr::Lambda { body, .. } = self.body.expr(id) {
                lambdas.extend(self.body.walk_from(*body));
            }
        }
        for id in self.body.walk() {
            if lambdas.contains(&id) {
                continue;
            }
            let Expr::Block { stmts } = self.body.expr(id) else {
                continue;
            };
            for pair in stmts.windows(2) {
                if matches!(self.body.expr(pair[0]), Expr::Name(n) if n == "return") {
                    self.tail_sites(pair[1], &mut out);
                }
            }
        }
        out.sort();
        out.dedup();
        out
    }

    /// Every `?` that returns from this declaration — not from a lambda.
    fn propagations(&self) -> Vec<ExprId> {
        let mut lambdas: BTreeSet<ExprId> = BTreeSet::new();
        for id in self.body.walk() {
            if let Expr::Lambda { body, .. } = self.body.expr(id) {
                lambdas.extend(self.body.walk_from(*body));
            }
        }
        self.body
            .walk()
            .into_iter()
            .filter(|id| !lambdas.contains(id) && matches!(self.body.expr(*id), Expr::Try { .. }))
            .collect()
    }

    fn tail_sites(&self, id: ExprId, out: &mut Vec<ExprId>) {
        match self.body.expr(id) {
            Expr::Block { stmts } => {
                if let Some(last) = stmts.last() {
                    self.tail_sites(*last, out);
                }
            }
            Expr::If {
                then, els: Some(e), ..
            } => {
                self.tail_sites(*then, out);
                self.tail_sites(*e, out);
            }
            Expr::Match { arms, .. } => {
                for a in arms {
                    self.tail_sites(a.body, out);
                }
            }
            // `return` as the last statement returns nothing — not a value.
            Expr::Name(n) if n == "return" => {}
            _ => out.push(id),
        }
    }
}

pub fn path_of(body: &Body, id: ExprId) -> String {
    crate::infer::path_of(body, id)
}

/// A declared callable's type as a value: `fn(params..) -> result`. A
/// parameter or result the author left unannotated is a hole, and so is every
/// part that mentions the callable's own type parameters — a generic function
/// named as a value is not instantiated by anything here.
fn function_type(sig: &Signature) -> Ty {
    let mut shape: Vec<Ty> = sig
        .params
        .iter()
        .map(|p| match p {
            Some(TypeResolution::Resolved(t)) => Ty::of(t),
            _ => Ty::Unknown,
        })
        .collect();
    shape.push(match &sig.returns {
        Some(TypeResolution::Resolved(t)) => Ty::of(t),
        _ => Ty::Unknown,
    });
    let s = Subst::default();
    s.close(&Ty::Builtin(Builtin::Function, shape).instantiate(sig.definition))
}

/// A lambda's parameter names, in order, when every parameter is a plain
/// binding. `fn(a, b) e` lowers its list as one call-shaped pattern around the
/// bindings; `x => e` as the binding itself.
/// A lambda's parameter names: `x => ..`, `fn(a, b) ..` and `(a, b) => ..`,
/// the last two lowered as one unnamed constructor pattern. The backend reads
/// parameters through this too (ADR-0040), so the two cannot disagree.
pub(crate) fn lambda_names(body: &Body, params: &[crate::hir::PatternId]) -> Option<Vec<String>> {
    let bind = |p: crate::hir::PatternId| match body.pat(p) {
        Pattern::Bind { name, .. } => Some(name.clone()),
        _ => None,
    };
    if let [only] = params
        && let Pattern::Ctor { path, args } = body.pat(*only)
        && path.is_empty()
    {
        return args.iter().map(|a| bind(*a)).collect();
    }
    params.iter().map(|p| bind(*p)).collect()
}

// --- running it -----------------------------------------------------------------------

/// **Every value relation in one unit.** The single walk both the checker and
/// [`analysis`] use, so the two cannot disagree about which relations exist.
pub fn relations(hir: &Hir, sigs: &Signatures, ws: &Workspace, at: UnitId) -> Vec<ValueRelation> {
    let mut out = Vec::new();
    for (id, decl) in hir.all_decls() {
        annotations(hir, sigs, ws, at, id, decl, &mut out);
        let Some(body_id) = decl.body else { continue };
        let body = hir.body(body_id);
        let typer = Typer::new(sigs, ws, at, hir.module_of(id), decl, body);
        typer.relations(&mut out);
        let own_result = sigs
            .by_def(DefId {
                unit: at,
                decl: id.0,
            })
            .and_then(Signature::result)
            .map(Ty::of);
        typer.term_relations(own_result, &mut out);
        returns(&typer, sigs, at, id, decl, &mut out);
    }
    out
}

/// The declared result against every value the body returns.
fn returns(
    typer: &Typer<'_>,
    sigs: &Signatures,
    at: UnitId,
    id: DeclId,
    decl: &Decl,
    out: &mut Vec<ValueRelation>,
) {
    if !returns_its_body(decl.kind) {
        return;
    }
    let def = DefId {
        unit: at,
        decl: id.0,
    };
    let Some(sig) = sigs.by_def(def) else { return };
    let Some(declared) = &sig.returns else { return };
    // A body that returns nothing declares `()`: its last statement is a
    // statement, not a value — the language has no statement terminator with
    // which to discard one. Recorded in docs/ASSUMPTIONS.md.
    if declared
        .resolved()
        .and_then(ResolvedType::as_primitive)
        .is_some_and(|p| p == Primitive::Unit)
    {
        return;
    }
    let boundary = || {
        (
            decl.name_span.clone(),
            format!("`{}` declares its result here", decl.name),
        )
    };
    for site in typer.result_sites() {
        out.push(typer.relate(
            RelationKind::Return,
            declared,
            typer.of(site),
            sig.path.clone(),
            typer.body.expr_span(site),
            boundary(),
        ));
    }
    // Each `e?` returns `e`'s failure early: an `Err` of the same error type,
    // or a `None`. Its success type is not returned, so it is a hole.
    for site in typer.propagations() {
        let Expr::Try { value } = typer.body.expr(site) else {
            continue;
        };
        let returned = match typer.of(*value) {
            Ty::Builtin(Builtin::Result, args) if args.len() == 2 => {
                Ty::Builtin(Builtin::Result, vec![Ty::Unknown, args[1].clone()])
            }
            Ty::Builtin(Builtin::Option, _) => Ty::Builtin(Builtin::Option, vec![Ty::Unknown]),
            _ => Ty::Unknown,
        };
        out.push(typer.relate(
            RelationKind::Return,
            declared,
            returned,
            sig.path.clone(),
            typer.body.expr_span(site),
            boundary(),
        ));
    }
}

/// The declarations whose body's value IS their result.
///
/// A `view`, `component` or `page` renders markup; a `materialize`,
/// `subscription` or `resource` body declares a lifecycle. None is checked
/// against a result here, because none says its body is one.
fn returns_its_body(kind: DeclKind) -> bool {
    matches!(
        kind,
        DeclKind::Fn | DeclKind::Query | DeclKind::Command | DeclKind::Task
    )
}

/// **Every written type resolves, or the program is told.** E9-V5.
///
/// `fn f(x: Stroe) -> Lisst<Int>` passed `pw check` with nothing to say until
/// 2026-09-24: the signature held `Unresolved` for both, every relation
/// declined to run on them — correctly — and nothing reported why. A relation
/// that refuses an unresolved type is only half of V5; the other half is that
/// the refusal reaches the author.
fn annotations(
    hir: &Hir,
    sigs: &Signatures,
    ws: &Workspace,
    at: UnitId,
    id: DeclId,
    decl: &Decl,
    out: &mut Vec<ValueRelation>,
) {
    let module = hir.module_of(id);
    let mut check = |written: &crate::hir::DeclaredType, span: Span, what: String| {
        let r = sigs.resolve_type(module, decl, written, span.clone());
        out.push(ValueRelation {
            declaration: decl.name.clone(),
            kind: RelationKind::Annotation,
            span,
            target: what,
            index: None,
            outcome: match &r {
                TypeResolution::Resolved(_) => Outcome::Agree,
                // An import that failed already says why: `PW0020` for a
                // module that is not there, `PW0021` for a name it does not
                // declare. A second diagnostic about the same cause would make
                // the count say two things were wrong.
                TypeResolution::Unresolved { name, .. }
                    if owned_by_a_failed_import(ws, at, name) =>
                {
                    Outcome::Undecided(Undecided::ExpectedUnresolved)
                }
                TypeResolution::Unresolved { name, written } => Outcome::Disagree {
                    expected: why_unresolved(ws, at, name, written),
                    actual: written.clone(),
                },
                TypeResolution::Blocked { .. } => Outcome::Undecided(Undecided::ExpectedUnresolved),
            },
            declared_at: None,
            boundary: (
                decl.name_span.clone(),
                format!("in the declaration of `{}`", decl.name),
            ),
        });
    };
    for p in &decl.params {
        if let Some(t) = &p.ty {
            check(t, p.span.clone(), format!("parameter `{}`", p.name));
        }
    }
    if let Some(t) = &decl.ret {
        check(t, decl.name_span.clone(), "the declared result".to_string());
    }
    for f in decl.fields.iter().flatten() {
        if let Some(t) = &f.ty {
            check(t, f.span.clone(), format!("field `{}`", f.name));
        }
    }
    let Some(body_id) = decl.body else { return };
    let body = hir.body(body_id);
    for e in body.walk() {
        let (ty, what) = match body.expr(e) {
            Expr::Let { ty: Some(ty), .. } => (*ty, "this binding's annotation"),
            Expr::Cast { ty, .. } => (*ty, "this cast's target"),
            _ => continue,
        };
        if let Some(written) = crate::resolved::written_in_body(body, ty) {
            check(&written, body.expr_span(e), what.to_string());
        }
    }
}

/// Is `name` something this unit imports from a module that does not exist,
/// or by name from a module that does not declare it? Either import is itself
/// a diagnostic, and it is the cause.
fn owned_by_a_failed_import(ws: &Workspace, at: UnitId, name: &str) -> bool {
    let Some(m) = ws.module_of(at) else {
        return false;
    };
    let (module, member) = match name.rsplit_once('.') {
        Some((m, n)) => (Some(m), n),
        None => (None, name),
    };
    m.imports.iter().any(|imp| {
        let names_it = match module {
            Some(q) => imp.module == q,
            None => imp.names.iter().any(|n| n == member),
        };
        names_it
            && match ws.modules.iter().find(|t| t.name == imp.module) {
                None => true,
                Some(t) => !t
                    .defines
                    .contains_key(&(Namespace::Type, member.to_string())),
            }
    })
}

/// What a diagnostic says about an unresolved written type: that a name is not
/// a type here, or that a type is applied to the wrong number of arguments.
fn why_unresolved(ws: &Workspace, at: UnitId, name: &str, written: &str) -> String {
    let found = match name.contains('.') {
        true => ws.resolve_path_in(at, Namespace::Type, name),
        false => ws.resolve_in(at, Namespace::Type, name),
    };
    match found {
        Resolution::Local(d) | Resolution::Imported { def: d, .. } => {
            let n = ws.type_arity(d).unwrap_or(0);
            format!(
                "`{name}` takes {n} type argument{}, and `{written}` applies it to a \
                 different number",
                if n == 1 { "" } else { "s" }
            )
        }
        Resolution::Ambiguous(_) => format!("`{name}` names more than one type here"),
        Resolution::Unresolved => match crate::resolved::Builtin::of(name) {
            Some(_) => format!("`{written}` does not apply `{name}` to its parameters"),
            None => format!("`{name}` is not a type visible here"),
        },
    }
}

/// **Diagnostics, as a projection of the relations.**
pub fn diagnostics(relations: &[ValueRelation], at: UnitId) -> Vec<Diagnostic> {
    let mut out = Vec::new();
    for r in relations {
        let Outcome::Disagree { expected, actual } = &r.outcome else {
            continue;
        };
        let d: Diagnostic = match r.kind {
            RelationKind::Arity => {
                let (m, n) = (expected, actual);
                let word = |k: &str| if k == "1" { "argument" } else { "arguments" };
                Diagnostic::error(
                    crate::codes::CALL_ARITY.id,
                    crate::codes::CALL_ARITY.invariant,
                    Detector::Signature,
                    format!(
                        "`{}` declares {m} {} and this call passes {n}",
                        r.target,
                        word(m)
                    ),
                    r.span.clone(),
                )
                .reason("call_arity_disagrees_with_declaration")
                .explain(
                    "the callee's declaration fixes how many arguments a call supplies; a call \
                     that passes a different number is not the call the declaration describes",
                )
                .repair(match n.parse::<usize>().ok() < m.parse::<usize>().ok() {
                    true => "pass the missing arguments",
                    false => "remove the extra arguments",
                })
            }
            RelationKind::Argument | RelationKind::Field => {
                let position = match (r.kind, r.index) {
                    (RelationKind::Field, _) => format!("the field `{}`", r.target),
                    (_, Some(i)) => format!("argument {} of `{}`", i + 1, r.target),
                    _ => format!("an argument of `{}`", r.target),
                };
                let mut d = Diagnostic::error(
                    crate::codes::ARGUMENT_TYPE.id,
                    crate::codes::ARGUMENT_TYPE.invariant,
                    Detector::Signature,
                    format!("{position} is declared `{expected}` and this is `{actual}`"),
                    r.span.clone(),
                )
                .reason("argument_type_disagrees_with_declaration")
                .explain(
                    "a value passed to a declared parameter or field must have the declared \
                     type; two types with one representation are still two types",
                )
                .repair(format!("pass a `{expected}`, or change the declaration"));
                if let Some((unit, span)) = &r.declared_at
                    && *unit == at
                {
                    d = d.related(span.clone(), format!("declared `{expected}` here"));
                }
                d
            }
            RelationKind::Return => Diagnostic::error(
                crate::codes::RETURN_TYPE.id,
                crate::codes::RETURN_TYPE.invariant,
                Detector::Signature,
                format!(
                    "`{}` declares its result `{expected}` and this produces `{actual}`",
                    r.target
                ),
                r.span.clone(),
            )
            .reason("result_disagrees_with_declaration")
            .explain(
                "every value a body returns — its last expression, each branch of it, and \
                 each `return` — must have the result type its signature declares",
            )
            .repair(format!(
                "produce a `{expected}` here, or change the declared result"
            )),
            RelationKind::Binding => Diagnostic::error(
                crate::codes::BINDING_TYPE.id,
                crate::codes::BINDING_TYPE.invariant,
                Detector::Signature,
                format!(
                    "`{}` is declared `{expected}` and initialised with `{actual}`",
                    r.target
                ),
                r.span.clone(),
            )
            .reason("binding_type_disagrees_with_annotation")
            .explain("an annotated binding must be initialised with a value of its declared type")
            .repair(format!(
                "initialise it with a `{expected}`, or change the annotation"
            )),
            RelationKind::Operand => Diagnostic::error(
                crate::codes::OPERAND_TYPE.id,
                crate::codes::OPERAND_TYPE.invariant,
                Detector::Signature,
                format!("{} must be `{expected}`, and this is `{actual}`", r.target),
                r.span.clone(),
            )
            .reason("operand_type_disagrees_with_operator")
            .explain(
                "a comparison's sides share a type, arithmetic takes two `Int`s or two \
                 `Float`s, and `&`, `|`, `!` and an `if` take `Bool`s; two types with one \
                 representation are still two types",
            )
            .repair(format!("make it a `{expected}`")),
            RelationKind::Assignment => Diagnostic::error(
                crate::codes::BINDING_TYPE.id,
                crate::codes::BINDING_TYPE.invariant,
                Detector::Signature,
                format!(
                    "`{}` holds `{expected}` and is assigned `{actual}`",
                    r.target
                ),
                r.span.clone(),
            )
            .reason("assignment_disagrees_with_binding")
            .explain("a mutable binding holds values of the type it was first bound with")
            .repair(format!("assign a `{expected}`, or bind a new name")),
            RelationKind::Member if expected == OPTIONAL_CONTENTS => Diagnostic::error(
                crate::codes::OPTION_USED_AS_VALUE.id,
                crate::codes::OPTION_USED_AS_VALUE.invariant,
                Detector::Signature,
                format!(
                    "`{actual}` has no member `{}`: what it may hold does, and must be taken \
                     out first",
                    r.target
                ),
                r.span.clone(),
            )
            .reason("member_of_optional_contents")
            .explain(
                "an `Option` or a `Result` is a value that may be absent or failed; its \
                 contents' members are reached by taking it apart, which says what happens \
                 when there are none",
            )
            .repair("`match` it, and read the member in the `Some` or `Ok` arm"),
            RelationKind::Member if expected == PRIVATE_REPRESENTATION => Diagnostic::error(
                crate::codes::UNKNOWN_MEMBER.id,
                crate::codes::UNKNOWN_MEMBER.invariant,
                Detector::Signature,
                format!(
                    "`{actual}` is opaque here: its `.value` is read only in the module that \
                     declares it"
                ),
                r.span.clone(),
            )
            .reason("opaque_representation_outside_its_module")
            .explain(
                "an opaque type's representation is its own module's; everywhere else the \
                 type has only the members that module declares for it",
            )
            .repair(format!(
                "declare an accessor for `{actual}` in its module, and read that"
            )),
            RelationKind::Member => Diagnostic::error(
                crate::codes::UNKNOWN_MEMBER.id,
                crate::codes::UNKNOWN_MEMBER.invariant,
                Detector::Signature,
                format!("`{actual}` has no member `{}`", r.target),
                r.span.clone(),
            )
            .reason("member_not_declared_by_type")
            .explain(
                "a value's members are its type's fields and the declarations whose first \
                 parameter takes that type; a name the type does not have reads nothing",
            )
            .repair(format!(
                "read a member `{actual}` declares, or declare `{}` for it",
                r.target
            )),
            RelationKind::Annotation => Diagnostic::error(
                crate::codes::UNRESOLVED_TYPE.id,
                crate::codes::UNRESOLVED_TYPE.invariant,
                Detector::Signature,
                format!("{} names `{actual}`: {expected}", r.target),
                r.span.clone(),
            )
            .reason("written_type_does_not_resolve")
            .explain(
                "a written type must name a type declaration visible from where it is written, \
                 applied to exactly the parameters that declaration binds; an annotation that \
                 resolves to nothing checks nothing",
            )
            .repair("import the type, correct its spelling, or supply its type arguments"),
        };
        out.push(d.related(r.boundary.0.clone(), r.boundary.1.clone()));
    }
    out
}

/// **Every value relation in the program, and what was concluded.** The audit
/// view: the same walk the checker projects diagnostics from.
pub fn analysis(units: &[crate::check::Unit]) -> Vec<(String, Vec<ValueRelation>)> {
    let hirs: Vec<&Hir> = units.iter().map(|u| &u.hir).collect();
    let ws = Workspace::build(&hirs);
    let sigs = Signatures::build(&ws, &hirs);
    units
        .iter()
        .enumerate()
        .map(|(i, u)| (u.path.clone(), relations(&u.hir, &sigs, &ws, i)))
        .collect()
}
