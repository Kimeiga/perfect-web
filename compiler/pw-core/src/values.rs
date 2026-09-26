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
//! Recorded in `docs/KNOWN_LIMITATIONS.md` rather than approximated here: a
//! call with named arguments is Undecided because a signature does not carry
//! parameter names. A sum type's case is typed where it is written through its
//! type, `Shape.Circle(3)`, since ADR-0059; the workspace still resolves no
//! case as a term.

use std::cell::RefCell;
use std::collections::{BTreeMap, BTreeSet};

use crate::diagnostics::{Detector, Diagnostic};
use crate::hir::{
    BinOp, Body, Decl, DeclId, DeclKind, Expr, ExprId, Hir, Literal, Pattern, Span, UnOp,
};
use crate::lexical::{Binder, Lexical};
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
    /// **Any type: the value holds at every one** (ADR-0065). `None` is an
    /// `Option` of anything, `[]` a `List` of anything, `todo` any value,
    /// and `Maybe.Nothing` a `Maybe` of anything. It agrees with every type,
    /// where [`Ty::Unknown`], which the program does not state, decides
    /// nothing. Never a generic function's unknown result, which one of its
    /// parameters may link to another: that stays unknown.
    Any,
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

    /// Fully known: no hole and no inference variable anywhere in it.
    fn is_closed(&self) -> bool {
        match self {
            Ty::Unknown | Ty::Var(_) => false,
            Ty::Builtin(_, args) | Ty::Nominal(_, args) => args.iter().all(Ty::is_closed),
            _ => true,
        }
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
    /// Variables met by a value of any type and by nothing that fixes one:
    /// `T` in `List.get([], 0)`. Each closes to [`Ty::Any`] (ADR-0065).
    any: BTreeSet<u32>,
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
            Ty::Var(v) if self.any.contains(&v) => Ty::Any,
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
        // A value of any type fixes no variable: `List.concat([], ["a"])`
        // is a `List<String>`, whichever side comes first (ADR-0065).
        (Ty::Var(v), Ty::Any) | (Ty::Any, Ty::Var(v)) => {
            s.any.insert(*v);
            Verdict::Agree
        }
        (Ty::Var(v), t) | (t, Ty::Var(v)) => {
            if t.is_unknown() || s.occurs(*v, t) {
                return Verdict::Undecided;
            }
            s.bound.insert(*v, t.clone());
            Verdict::Agree
        }
        (Ty::Any, _) | (_, Ty::Any) => Verdict::Agree,
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
        (Ty::Any, t) | (t, Ty::Any) => t,
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
    /// `Shape.Circle`: the case a path names, against the cases its type
    /// declares (PW0608, ADR-0059).
    Case,
    /// `Box { value: 1, label: "a" }`: the fields a record is built with,
    /// against the fields its type declares, each once (PW0612, ADR-0067).
    Fields,
}

/// A fields relation's expectation for a field its type does not declare.
const FIELD_UNDECLARED: &str = "a field its type declares";
/// A fields relation's expectation for a field given more than once.
const FIELD_ONCE: &str = "each field once";
/// A fields relation's expectation for a field left out.
const FIELD_GIVEN: &str = "every field its type declares";

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
    /// `Shape.Circle(3)`: a sum type's case, by its position in the
    /// declaration, built from its payload's fields positionally (ADR-0059).
    Case(DefId, usize),
}

struct Typer<'a> {
    sigs: &'a Signatures,
    ws: &'a Workspace,
    at: UnitId,
    module: Option<&'a str>,
    decl: &'a Decl,
    body: &'a Body,
    /// The binding each local name means (ADR-0063).
    lexical: Lexical,
    /// Every binding given a known type in this body, by where it is bound,
    /// never by its name: two bindings of one name are two entries
    /// (ADR-0063). A cell, because typing a lambda against the function type
    /// it is passed as binds its parameters for the duration of that one
    /// question.
    locals: RefCell<BTreeMap<Binder, Ty>>,
    /// The expression each `|>` feeds, keyed by the call on its right.
    piped: BTreeMap<ExprId, ExprId>,
    /// Lambda parameters given a closed type while their use was solved,
    /// waiting to join `locals` (see [`Typer::solve_lambdas`]).
    solved: RefCell<BTreeMap<Binder, Ty>>,
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
    let lexical = Lexical::build(sigs, Some(at), decl, body);
    let typer = Typer::new(sigs, ws, at, module, decl, body, lexical, Vec::new());
    let t = typer.of(e);
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
        // `Shape.Circle(3)`, a case through its type (ADR-0059). A case its
        // type lacks is refused here and reported by the case relation.
        Resolution::Unresolved => {
            return match case_named(sigs, ws, at, &path) {
                Some(CaseNamed::Case(def, index)) => Named::Target(Target::Case(def, index)),
                Some(CaseNamed::Unknown(..)) => Named::Refused,
                None => Named::Nothing,
            };
        }
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

/// **`t` with `binder`'s type parameters replaced by `args`**: a declared
/// case's field under the arguments its type is applied to. A parameter no
/// argument gives is unknown.
pub(crate) fn substituted(t: &Ty, binder: DefId, args: &[Ty]) -> Ty {
    match t {
        Ty::Parameter { binder: b, index } if *b == binder => {
            args.get(*index as usize).cloned().unwrap_or(Ty::Unknown)
        }
        Ty::Builtin(c, xs) => Ty::Builtin(
            *c,
            xs.iter().map(|x| substituted(x, binder, args)).collect(),
        ),
        Ty::Nominal(d, xs) => Ty::Nominal(
            *d,
            xs.iter().map(|x| substituted(x, binder, args)).collect(),
        ),
        other => other.clone(),
    }
}

/// What a qualified path names when it names a sum type's case (ADR-0059).
pub(crate) enum CaseNamed {
    /// `Shape.Circle`: the type, and the case's position in its declaration.
    Case(DefId, usize),
    /// `Shape.Bogus`: a sum type, and a name none of its cases has.
    Unknown(DefId, String),
}

/// **Which case a qualified path names** (ADR-0059): `Shape.Circle`, or
/// `geometry.Shape.Circle` through a module. The type is resolved in the Type
/// namespace where the path is written, and the case is found among the cases
/// it declares. `None` where the path is not a sum type's: a path that
/// resolves as a term is that term.
///
/// The one rule, with three readers: the typer, the check that a case exists,
/// and the backend (`backend::lower`), so the case the checker typed is the
/// case that is built.
pub(crate) fn case_named(
    sigs: &Signatures,
    ws: &Workspace,
    at: UnitId,
    path: &str,
) -> Option<CaseNamed> {
    let (ty, case) = path.rsplit_once('.')?;
    if !matches!(ws.resolve_path(at, path), Resolution::Unresolved) {
        return None;
    }
    let found = match ty.contains('.') {
        true => ws.resolve_path_in(at, Namespace::Type, ty),
        false => ws.resolve_in(at, Namespace::Type, ty),
    };
    let (Resolution::Local(def) | Resolution::Imported { def, .. }) = found else {
        return None;
    };
    let cases = sigs.type_decl(def)?.variants.as_ref()?;
    Some(match cases.iter().position(|(name, _)| name == case) {
        Some(index) => CaseNamed::Case(def, index),
        None => CaseNamed::Unknown(def, case.to_string()),
    })
}

/// **The case a bare name is** (ADR-0059): `Empty`, where no term has the
/// name, is the case of that name of the one sum type `at` sees with one,
/// in the set ADR-0047 resolves a bare constructor against. `None` where no
/// type has it, or several do, which the name alone cannot choose between.
/// The language's own cases keep their names: a program's `None` is written
/// through its type.
pub(crate) fn bare_case(
    sigs: &Signatures,
    ws: &Workspace,
    at: UnitId,
    name: &str,
) -> Option<(DefId, usize)> {
    if matches!(name, "Some" | "None" | "Ok" | "Err")
        || !matches!(
            ws.resolve_in(at, Namespace::Term, name),
            Resolution::Unresolved
        )
    {
        return None;
    }
    let found: BTreeSet<(DefId, usize)> = ws
        .visible_types(at)
        .into_iter()
        .filter_map(|def| {
            let cases = sigs.type_decl(def)?.variants.as_ref()?;
            Some((def, cases.iter().position(|(n, _)| n == name)?))
        })
        .collect();
    match found.len() {
        1 => found.into_iter().next(),
        _ => None,
    }
}

/// **The type parameters of `binder` no field mentions** (ADR-0065): each is
/// free in a value built from those fields, which holds at every type it
/// could be.
fn unmentioned(ws: &Workspace, binder: DefId, fields: &[Option<TypeResolution>]) -> BTreeSet<u32> {
    fn mentions(t: &ResolvedType, binder: DefId, i: u32) -> bool {
        t.parameter_binding() == Some((binder, i))
            || t.args().iter().any(|a| mentions(a, binder, i))
    }
    let arity = ws.type_arity(binder).unwrap_or(0) as u32;
    (0..arity)
        .filter(|i| {
            !fields.iter().any(|f| {
                f.as_ref()
                    .and_then(TypeResolution::resolved)
                    .is_some_and(|t| mentions(t, binder, *i))
            })
        })
        .collect()
}

/// `t` with every [`Ty::Any`] in it made unknown: a mutable binding holds
/// one type, whatever its first value holds at (ADR-0065).
fn one_type(t: Ty) -> Ty {
    match t {
        Ty::Any => Ty::Unknown,
        Ty::Builtin(c, args) => Ty::Builtin(c, args.into_iter().map(one_type).collect()),
        Ty::Nominal(d, args) => Ty::Nominal(d, args.into_iter().map(one_type).collect()),
        other => other,
    }
}

/// Is this one name, as a directive writes one?
fn is_ident(s: &str) -> bool {
    s.starts_with(|c: char| c.is_alphabetic() || c == '_')
        && s.chars().all(|c| c.is_alphanumeric() || c == '_')
}

/// The rounds the typer's bindings are solved in, at most. Each round adds a
/// binding or completes one's type, or ends the solving; a chain written in
/// order is solved in one.
const ROUNDS: usize = 8;

impl<'a> Typer<'a> {
    /// A typer for `decl`'s body, reading names by `lexical`. `outer` types
    /// the bindings around a nested declaration, in the order the resolution
    /// numbers them (ADR-0066).
    #[allow(clippy::too_many_arguments)]
    fn new(
        sigs: &'a Signatures,
        ws: &'a Workspace,
        at: UnitId,
        module: Option<&'a str>,
        decl: &'a Decl,
        body: &'a Body,
        lexical: Lexical,
        outer: Vec<Ty>,
    ) -> Typer<'a> {
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

        // What the program writes: each parameter's declared type, and each
        // annotated `let`'s. Around a nested declaration, what its enclosing
        // declaration's bindings were typed as there (ADR-0066).
        let mut locals = BTreeMap::new();
        for (i, t) in outer.into_iter().enumerate() {
            if !t.is_unknown() {
                locals.insert(Binder::Outer(i as u32), t);
            }
        }
        for (i, p) in decl.params.iter().enumerate() {
            if let Some(t) = &p.ty
                && let Some(ty) = sigs
                    .resolve_type(module, decl, t, p.span.clone())
                    .resolved()
            {
                locals.insert(Binder::Param(i), Ty::of(ty));
            }
        }
        for id in body.walk() {
            if let Expr::Let {
                pat: Some(pat),
                ty: Some(ty),
                ..
            } = body.expr(id)
                && let Pattern::Bind { .. } = body.pat(*pat)
                && let Some(written) = crate::resolved::written_in_body(body, *ty)
                && let Some(t) = sigs
                    .resolve_type(module, decl, &written, body.expr_span(id))
                    .resolved()
            {
                locals.insert(Binder::Pattern(*pat), Ty::of(t));
            }
        }

        let typer = Typer {
            sigs,
            ws,
            at,
            module,
            decl,
            body,
            lexical,
            locals: RefCell::new(locals),
            piped,
            solved: RefCell::new(BTreeMap::new()),
        };

        // What the program implies: an unannotated `let` or `use` takes its
        // initialiser's type, a lambda's parameters the types its use gives
        // them, a match arm's names its scrutinee's payload, a `for` loop's
        // name its list's element, and a template's names their block's
        // (ADR-0063). Iterated, because `let a = f()` then `let b = g(a)`
        // needs `a` first, and a lambda's call may need a binding typed;
        // bounded, because a round only adds a binding or makes one's type
        // more complete, and a complete type is never changed.
        for _ in 0..ROUNDS {
            let mut added = typer.solve_lambdas();
            added |= typer.solve_bindings();
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
            Expr::Name(n) => self.name(id, n),
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
                        // A value of any type is the number the other side is
                        // (ADR-0065).
                        (Ty::Primitive(p), Ty::Any) | (Ty::Any, Ty::Primitive(p))
                            if matches!(p, Primitive::Int | Primitive::Float) =>
                        {
                            Ty::Primitive(p)
                        }
                        (Ty::Any, Ty::Any) => Ty::Any,
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
            // Without `else` it is a statement, whose value is the unit value
            // whichever way it goes, as the backend has it (ADR-0051). It was
            // of no stated type until 2026-09-26, so one ending a body that
            // declares an `Int` passed (ADR-0067).
            Expr::If { els: None, .. } => Ty::Primitive(Primitive::Unit),
            // Each arm's body. Its pattern's names are bound to the payload
            // types the scrutinee's type gives them when the typer is built:
            // `entry` in `Some(entry) => ..` is the option's `T`.
            Expr::Match { arms, .. } => arms
                .iter()
                .map(|a| self.of(a.body))
                .reduce(join)
                .unwrap_or(Ty::Unknown),
            Expr::Record { name: Some(_), .. } => self.construct(id).result,
            // `[]` is a list of anything (ADR-0065).
            Expr::List { items } => Ty::Builtin(
                Builtin::List,
                vec![
                    items
                        .iter()
                        .map(|i| self.of(*i))
                        .reduce(join)
                        .unwrap_or(Ty::Any),
                ],
            ),
            _ => Ty::Unknown,
        }
    }

    /// **A name where `id` writes it**: the binding it means, or, where no
    /// binding in scope has it, the program's own (ADR-0063).
    fn name(&self, id: ExprId, n: &str) -> Ty {
        match self.lexical.binder(id) {
            Some(b) => self.local(b),
            None => self.global(n),
        }
    }

    /// The type a binding holds, as far as it is known.
    fn local(&self, b: Binder) -> Ty {
        self.locals.borrow().get(&b).cloned().unwrap_or(Ty::Unknown)
    }

    /// A name no local binding has: a declaration, the language's own value,
    /// or a sum type's case.
    fn global(&self, n: &str) -> Ty {
        // A declared callable named as a value: `List.map(xs, line_total)`.
        let term = self.ws.resolve_in(self.at, Namespace::Term, n);
        if let Resolution::Local(d) | Resolution::Imported { def: d, .. } = term
            && let Some(sig) = self.sigs.by_def(d)
        {
            return function_type(sig);
        }
        // The language's own values, only where the program has not declared
        // something of that name; then a sum type's case (ADR-0059).
        let own = matches!(term, Resolution::Unresolved);
        match n {
            "true" | "false" if own => Ty::Primitive(Primitive::Bool),
            "None" if own => Ty::Builtin(Builtin::Option, vec![Ty::Any]),
            // It never returns, so it is a value of every type (ADR-0065).
            "todo" if own => Ty::Any,
            // Charter §7.5A: `self` in a UI declaration is its own element, a
            // language fact rather than something a library declares.
            "self" if own && self.is_ui() => self
                .sigs
                .language_type("browser", "ElementRef")
                .map(|t| Ty::of(&t))
                .unwrap_or(Ty::Unknown),
            _ if own => match bare_case(self.sigs, self.ws, self.at, n) {
                Some((def, index)) => self.case_value(def, index),
                None => Ty::Unknown,
            },
            _ => Ty::Unknown,
        }
    }

    /// **A case named as a value** (ADR-0059): one without a payload is a
    /// value of its type, `Shape.Empty`; one with a payload is the function
    /// that builds it, `Shape.Circle`, which a list operation can be passed.
    fn case_value(&self, def: DefId, index: usize) -> Ty {
        let Some((_, fields)) = self
            .sigs
            .type_decl(def)
            .and_then(|t| t.variants.as_ref())
            .and_then(|cases| cases.get(index))
        else {
            return Ty::Unknown;
        };
        // A parameter the case's fields do not mention holds at every type:
        // `Maybe.Nothing` is a `Maybe` of anything (ADR-0065). One they do
        // mention links the function's parameter to its result, and stays
        // unknown.
        let s = Subst {
            any: unmentioned(
                self.ws,
                def,
                &fields.iter().map(|f| Some(f.clone())).collect::<Vec<_>>(),
            ),
            ..Subst::default()
        };
        let result = s.close(&self.applied(def).instantiate(def));
        if fields.is_empty() {
            return result;
        }
        let mut shape: Vec<Ty> = fields
            .iter()
            .map(|f| match f {
                TypeResolution::Resolved(t) => s.close(&Ty::of(t).instantiate(def)),
                _ => Ty::Unknown,
            })
            .collect();
        shape.push(result);
        Ty::Builtin(Builtin::Function, shape)
    }

    /// The names an arm's pattern binds, each typed from the type the part
    /// of the pattern binding it is read against: `Some(x)` binds the
    /// option's `T`; `Ok(x)` and `Err(e)` the result's two sides, where the
    /// program declares nothing of that name; `Rect(w, h)` each field of a
    /// declared case's payload (ADR-0059); `Some(Circle(r))` its field inside
    /// a case inside the option; and a name alone the whole value, unless it
    /// names a case (ADR-0060). Until 2026-09-26 only a name directly under a
    /// case was bound, and a nested one was unknown to every relation.
    /// Each name an arm's pattern binds, by where it binds it, and the type
    /// the scrutinee's type gives it.
    fn arm_bindings(&self, scrutinee: &Ty, pat: crate::hir::PatternId) -> Vec<(Binder, Ty)> {
        let mut out = Vec::new();
        self.bind_pattern(scrutinee, pat, &mut out);
        out
    }

    fn bind_pattern(&self, ty: &Ty, pat: crate::hir::PatternId, out: &mut Vec<(Binder, Ty)>) {
        match self.body.pat(pat) {
            Pattern::Bind { name, .. } => {
                if !self.names_a_case(name) {
                    out.push((Binder::Pattern(pat), ty.clone()));
                }
            }
            Pattern::Ctor { path, args } => {
                if let Some(fields) = self.pattern_fields(ty, path, args.len()) {
                    for (arg, field) in args.iter().zip(&fields) {
                        self.bind_pattern(field, *arg, out);
                    }
                }
            }
            _ => {}
        }
    }

    /// Is a name written alone in a pattern a case rather than a binding: the
    /// language's `true`, `false` or `None`, or a case of a type this unit
    /// sees (ADR-0038)?
    fn names_a_case(&self, name: &str) -> bool {
        crate::lexical::names_a_case(self.sigs, Some(self.at), name)
    }

    /// The types of the fields a constructor pattern takes apart, read
    /// against `ty`: a builtin case's payload, or a declared case's fields
    /// under the type's arguments (`Tree<Int>`'s `Node(Tree<T>, T)` has a
    /// `Tree<Int>` and an `Int`). `None` for a case `ty` lacks, a qualifier
    /// naming another type, or a field count the case does not declare; the
    /// exhaustiveness check reports each (PW0608, PW0603).
    fn pattern_fields(&self, ty: &Ty, path: &str, arity: usize) -> Option<Vec<Ty>> {
        if let Ty::Nominal(def, args) = ty
            && let Some(cases) = self.sigs.type_decl(*def).and_then(|t| t.variants.as_ref())
        {
            let index = match path.rsplit_once('.') {
                Some(_) => match case_named(self.sigs, self.ws, self.at, path)? {
                    CaseNamed::Case(d, index) if d == *def => index,
                    _ => return None,
                },
                None => cases.iter().position(|(n, _)| n == path)?,
            };
            let (_, fields) = cases.get(index)?;
            if fields.len() != arity {
                return None;
            }
            return Some(
                fields
                    .iter()
                    .map(|f| match f {
                        TypeResolution::Resolved(t) => substituted(&Ty::of(t), *def, args),
                        _ => Ty::Unknown,
                    })
                    .collect(),
            );
        }
        let own = matches!(
            self.ws.resolve_in(self.at, Namespace::Term, path),
            Resolution::Unresolved
        );
        let payload = match (path, ty) {
            ("Some", Ty::Builtin(Builtin::Option, a)) if own => a.first(),
            ("Ok", Ty::Builtin(Builtin::Result, a)) if own => a.first(),
            ("Err", Ty::Builtin(Builtin::Result, a)) if own => a.get(1),
            _ => None,
        }?;
        (arity == 1).then(|| vec![payload.clone()])
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
    fn term_relations(&self, out: &mut Vec<ValueRelation>) {
        for (_, root) in self.decl.term_roots() {
            self.relations_from(root.root, out);
        }
    }

    /// The type a policy term's binders take from its header, where it gives
    /// one (see [`Typer::term_relations`]).
    fn term_binders(&self) -> Vec<(Binder, Ty)> {
        use crate::hir::ExecutionContext as Cx;
        let success = |t: Ty| match t {
            Ty::Builtin(Builtin::Result, mut args) if args.len() == 2 => args.swap_remove(0),
            other => other,
        };
        let own_result = self
            .own_def()
            .and_then(|d| self.sigs.by_def(d))
            .and_then(Signature::result)
            .map(Ty::of);
        let mut out = Vec::new();
        for (i, (policy, root)) in self.decl.term_roots().enumerate() {
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
            out.extend((0..root.binders.len()).map(|j| (Binder::Term(i, j), bound.clone())));
        }
        out
    }

    /// A lambda, typed against the function type it is passed as: its
    /// parameters take that type's parameter types, and its type is theirs
    /// with its body's type as the result. Without an expected function type
    /// a lambda has no type here — an unannotated parameter is not a claim.
    fn lambda(&self, params: &[crate::hir::PatternId], body: ExprId, expected: &[Ty]) -> Ty {
        let Some(params) = lambda_binders(self.body, params) else {
            return Ty::Unknown;
        };
        let Some((_, expected_params)) = expected.split_last() else {
            return Ty::Unknown;
        };
        if params.len() != expected_params.len() {
            // A callback of the wrong arity is a different function type.
            let mut shape = vec![Ty::Unknown; params.len()];
            shape.push(Ty::Unknown);
            return Ty::Builtin(Builtin::Function, shape);
        }
        let own = self.own_def();
        let mut saved = Vec::new();
        for (p, ty) in params.iter().zip(expected_params) {
            let b = Binder::Pattern(*p);
            if ty.is_closed() && !ty.mentions_foreign_parameter(own) {
                self.solved.borrow_mut().insert(b, ty.clone());
            }
            let old = self.locals.borrow_mut().insert(b, ty.clone());
            saved.push((b, old));
        }
        let result = self.of(body);
        for (b, old) in saved {
            match old {
                Some(t) => self.locals.borrow_mut().insert(b, t),
                None => self.locals.borrow_mut().remove(&b),
            };
        }
        let mut shape = expected_params.to_vec();
        shape.push(result);
        Ty::Builtin(Builtin::Function, shape)
    }

    /// `base.name`: a record field, read through the receiver's type.
    fn field(&self, id: ExprId, base: ExprId, name: &str) -> Ty {
        // `Shape.Empty` and `Shape.Circle`, a case through its type
        // (ADR-0059).
        if let Some(c) = case_named(self.sigs, self.ws, self.at, &path_of(self.body, id)) {
            return match c {
                CaseNamed::Case(def, index) => self.case_value(def, index),
                CaseNamed::Unknown(..) => Ty::Unknown,
            };
        }
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
        self.member_type(&self.of(base), name)
    }

    /// The type of `name` read from a value of type `receiver`: a record's
    /// field under its instance, an opaque value's representation in its own
    /// module, or a property.
    fn member_type(&self, receiver: &Ty, name: &str) -> Ty {
        let receiver = receiver.clone();
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
            Target::Case(def, index) => {
                let (case, fields) = self
                    .sigs
                    .type_decl(*def)
                    .and_then(|t| t.variants.as_ref())
                    .and_then(|cases| cases.get(*index))
                    .cloned()
                    .unwrap_or_default();
                Callee {
                    name: format!("{}.{case}", self.display_def(*def)),
                    binder: *def,
                    params: fields.iter().map(|r| Some(r.clone())).collect(),
                    declared: fields
                        .iter()
                        .map(|r| r.resolved().map(|t| (def.unit, t.span())))
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
        // A value built of fields that mention none of its type's `U` holds
        // at every `U`: `Either.Left(1)` is an `Either<Int, _>` (ADR-0065).
        if matches!(
            target,
            Target::Record(_) | Target::Case(..) | Target::Opaque(_)
        ) {
            s.any.extend(unmentioned(self.ws, binder, &params));
        }
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
            "Ok" => Ty::Builtin(Builtin::Result, vec![v, Ty::Any]),
            "Err" => Ty::Builtin(Builtin::Result, vec![Ty::Any, v]),
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
        // The fields it is built with, against the fields its type declares,
        // each once (ADR-0067).
        let record = self.display_def(def);
        let fields_relation = |span: Span, expected: &str, field: &str| ValueRelation {
            declaration: self.decl.name.clone(),
            kind: RelationKind::Fields,
            span,
            target: record.clone(),
            index: None,
            outcome: match expected.is_empty() {
                true => Outcome::Agree,
                false => Outcome::Disagree {
                    expected: expected.to_string(),
                    actual: field.to_string(),
                },
            },
            declared_at: None,
            boundary: (
                self.body.expr_span(id),
                format!("in this construction of `{record}`"),
            ),
        };
        let mut given: BTreeSet<&str> = BTreeSet::new();
        let mut whole = true;
        for (i, init) in fields.iter().enumerate() {
            let Some((index, (_, expected))) = declared
                .iter()
                .enumerate()
                .find(|(_, (n, _))| *n == init.name)
            else {
                relations.push(fields_relation(
                    init.span.clone(),
                    FIELD_UNDECLARED,
                    &init.name,
                ));
                whole = false;
                continue;
            };
            if !given.insert(init.name.as_str()) {
                relations.push(fields_relation(init.span.clone(), FIELD_ONCE, &init.name));
                whole = false;
            }
            let actual = match init.value {
                Some(v) => self.of(v),
                // `Point { x, y }` — the shorthand names a binding.
                None => match self.lexical.shorthand(id, i) {
                    Some(b) => self.local(b),
                    None => self.global(&init.name),
                },
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
        for (name, _) in declared {
            if !given.contains(name.as_str()) {
                relations.push(fields_relation(self.body.expr_span(id), FIELD_GIVEN, name));
                whole = false;
            }
        }
        if whole {
            relations.push(fields_relation(self.body.expr_span(id), "", ""));
        }
        s.any.extend(unmentioned(
            self.ws,
            def,
            &declared
                .iter()
                .map(|(_, r)| Some(r.clone()))
                .collect::<Vec<_>>(),
        ));
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
            Ty::Any => "_".to_string(),
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

    /// **Every binding the body's own shape types** (ADR-0063): an
    /// unannotated `let` or `use` by its initialiser, a match arm's names by
    /// the scrutinee's payload, a `for` loop's name by its list's element, a
    /// template's `{#each}` and `{#match}` arm names by their block's, and a
    /// policy term's binders by its header. Returns whether any binding was
    /// newly typed, or more completely.
    fn solve_bindings(&self) -> bool {
        let mut added = false;
        let mut trees = vec![self.body.root];
        trees.extend(self.decl.term_roots().map(|(_, r)| r.root));
        for id in trees.iter().flat_map(|t| self.body.walk_from(*t)) {
            match self.body.expr(id) {
                Expr::Let {
                    pat: Some(pat),
                    ty: None,
                    init: Some(init),
                } if matches!(self.body.pat(*pat), Pattern::Bind { .. }) => {
                    // A `let mut` holds one type: `let mut xs = []` is a list
                    // of what is assigned to it, not of anything (ADR-0065).
                    let t = match self.body.pat(*pat) {
                        Pattern::Bind { mutable: true, .. } => one_type(self.of(*init)),
                        _ => self.of(*init),
                    };
                    added |= self.bind(Binder::Pattern(*pat), t);
                }
                // `x = e` completes a `let mut`'s type where its first value
                // left it incomplete.
                Expr::Binary {
                    op: BinOp::Assign,
                    lhs,
                    rhs,
                } => {
                    if let Some(b) = self.lexical.binder(*lhs) {
                        added |= self.bind(b, one_type(self.of(*rhs)));
                    }
                }
                Expr::Keyword { keyword, args, .. } if keyword == "use" => {
                    if let Some(init) = args.first() {
                        added |= self.bind(Binder::Use(id), self.of(*init));
                    }
                }
                Expr::Match { scrutinee, arms } => {
                    let st = self.of(*scrutinee);
                    for arm in arms {
                        for (b, t) in self.arm_bindings(&st, arm.pat) {
                            added |= self.bind(b, t);
                        }
                    }
                }
                // `for x in xs` over a `List<T>`: each `x` is a `T`. The loop
                // takes a list and binds one name (ADR-0051).
                Expr::For {
                    pat: Some(pat),
                    iterable,
                    ..
                } if matches!(self.body.pat(*pat), Pattern::Bind { .. }) => {
                    if let Ty::Builtin(Builtin::List, args) = self.of(*iterable)
                        && let [element] = args.as_slice()
                    {
                        added |= self.bind(Binder::Pattern(*pat), element.clone());
                    }
                }
                _ => {}
            }
        }
        for (b, t) in self.term_binders() {
            added |= self.bind(b, t);
        }
        // `{#each xs as x}`: each `x` is an element of `xs`, read from the
        // directive as written: a name, and the fields read from it.
        for (node, collection, head) in self.lexical.each_blocks() {
            let mut segments = collection.split('.').map(str::trim);
            let first = segments.next().unwrap_or_default();
            if !is_ident(first) {
                continue;
            }
            let mut t = match head {
                Some(b) => self.local(b),
                None => self.global(first),
            };
            for segment in segments {
                if !is_ident(segment) {
                    t = Ty::Unknown;
                    break;
                }
                t = self.member_type(&t, segment);
            }
            if let Ty::Builtin(Builtin::List, args) = t
                && let [element] = args.as_slice()
            {
                added |= self.bind(Binder::Each(node), element.clone());
            }
        }
        // `release(h) { .. }`: what the resource's `acquire { .. }` produced,
        // its success where it can fail.
        for (clause, acquired) in self.lexical.released() {
            let t = match self.of(acquired) {
                Ty::Builtin(Builtin::Result, mut args) if args.len() == 2 => args.swap_remove(0),
                t => t,
            };
            added |= self.bind(Binder::Clause(clause, 0), t);
        }
        // `<ready as={x}>`: the stream's answer when it succeeds, and
        // `<failed as={e}>` its failure.
        for (node, query, ok) in self.lexical.stream_parts() {
            let t = match (self.of(query), ok) {
                (Ty::Builtin(Builtin::Result, mut args), true) if args.len() == 2 => {
                    args.swap_remove(0)
                }
                (Ty::Builtin(Builtin::Result, mut args), false) if args.len() == 2 => {
                    args.swap_remove(1)
                }
                (t, true) => t,
                (_, false) => Ty::Unknown,
            };
            added |= self.bind(Binder::Stream(node), t);
        }
        // `{:Some(x)}`, `{:Rect(w, h)}`: each name is its field of the case
        // the block's subject holds, as a match arm's is.
        for (node, subject) in self.lexical.template_arms() {
            let crate::hir::Node::Branch { arm: Some(arm), .. } = self.body.node(node) else {
                continue;
            };
            let st = self.of(subject);
            let Some(fields) = self.pattern_fields(&st, &arm.case, arm.bindings.len()) else {
                continue;
            };
            for (i, f) in fields.into_iter().enumerate() {
                added |= self.bind(Binder::Arm(node, i), f);
            }
        }
        added
    }

    /// **Record the type a binding holds.** A binding keeps the first type
    /// it is given, unless a later round gives it a complete type its
    /// incomplete one agrees with: `List<?>` becomes `List<Int>`, and a
    /// complete type is never changed. So the rounds end.
    ///
    /// Every type recorded is the typer's own, closed over each callee's
    /// parameters. Until ADR-0063 the bindings began as `infer.rs`'s, which
    /// typed `let ys = List.filter(xs, ..)` by `filter`'s declared `List<T>`,
    /// and a correct `List.sort_by(ys, compare)` was refused (PW0605) for a
    /// mismatch the typer had made (found 2026-09-25).
    fn bind(&self, b: Binder, t: Ty) -> bool {
        if t.is_unknown() {
            return false;
        }
        let mut locals = self.locals.borrow_mut();
        match locals.get(&b) {
            None => {
                locals.insert(b, t);
                true
            }
            Some(old)
                if !old.is_closed()
                    && t.is_closed()
                    && unify(&mut Subst::default(), old, &t) != Verdict::Disagree =>
            {
                locals.insert(b, t);
                true
            }
            Some(_) => false,
        }
    }

    /// Is this declaration one whose body renders an element: a view, a
    /// component, a page?
    fn is_ui(&self) -> bool {
        matches!(
            self.decl.kind,
            DeclKind::View | DeclKind::Component | DeclKind::Page
        )
    }

    fn own_def(&self) -> Option<DefId> {
        let ns = Namespace::of(self.decl.kind)?;
        match self.ws.resolve_in(self.at, ns, &self.decl.name) {
            Resolution::Local(d) => Some(d),
            _ => None,
        }
    }

    /// **A lambda's parameters, as its use gives them**: the function type of
    /// the parameter it is passed as, of a `let`'s written type, or of the
    /// declared result it is returned as. Returns whether any parameter was
    /// newly typed.
    ///
    /// The relations walk reads a lambda's body on its own, with the body's
    /// bindings. A parameter was typed only while its call was solved, so in
    /// that walk it was untyped, or typed by `infer.rs`'s narrower rule
    /// (ADR-0053). A parameter keeps the closed type its use gives it, by
    /// where it is bound, whatever else binds its name (ADR-0063).
    fn solve_lambdas(&self) -> bool {
        let is_lambda = |id: ExprId| matches!(self.body.expr(id), Expr::Lambda { .. });
        for id in self.body.walk() {
            match self.body.expr(id) {
                Expr::Call { args, .. } if args.iter().any(|a| is_lambda(a.value)) => {
                    let _ = self.call(id);
                }
                Expr::Let {
                    ty: Some(ty),
                    init: Some(init),
                    ..
                } if is_lambda(*init) => {
                    let Some(written) = crate::resolved::written_in_body(self.body, *ty) else {
                        continue;
                    };
                    let span = self.body.expr_span(*init);
                    if let Some(t) = self
                        .sigs
                        .resolve_type(self.module, self.decl, &written, span)
                        .resolved()
                    {
                        self.lambda_as(*init, &Ty::of(t));
                    }
                }
                _ => {}
            }
        }
        let result = self
            .own_def()
            .and_then(|d| self.sigs.by_def(d))
            .and_then(Signature::result)
            .map(Ty::of);
        if let Some(result) = result {
            for site in self.result_sites() {
                self.lambda_as(site, &result);
            }
        }
        let solved = std::mem::take(&mut *self.solved.borrow_mut());
        let mut added = false;
        for (b, t) in solved {
            added |= self.bind(b, t);
        }
        added
    }

    /// Type a lambda against a function type it is used as, for the
    /// parameters that records.
    fn lambda_as(&self, id: ExprId, expected: &Ty) {
        if let (
            Expr::Lambda {
                descriptor: None,
                params,
                body,
            },
            Ty::Builtin(Builtin::Function, fargs),
        ) = (self.body.expr(id), expected)
        {
            let _ = self.lambda(params, *body, fargs);
        }
    }

    // --- the relations -----------------------------------------------------------------

    /// Every place in this body a value meets a declared type.
    fn relations(&self, out: &mut Vec<ValueRelation>) {
        self.relations_from(self.body.root, out);
    }

    /// The same relations, over one tree: the body, or a policy's term root,
    /// in the order `Body::walk_from` visits it. An arm's names are typed by
    /// where they are bound, so `x` in `Some(x) => x + 1` is the option's `T`
    /// and `r` in `Circle(r) => ..` the case's field (ADR-0059), whatever
    /// else binds an `x` or an `r` (ADR-0063). Until 2026-09-26 every
    /// relation inside an arm read them as unknown, and `Some(x) => x + "a"`
    /// over an `Option<Int>` passed `pw check`.
    fn relations_from(&self, root: ExprId, out: &mut Vec<ValueRelation>) {
        self.relations_at(root, out);
        for c in self.body.children(root) {
            self.relations_from(c, out);
        }
    }

    /// The relations one expression is the site of.
    fn relations_at(&self, id: ExprId, out: &mut Vec<ValueRelation>) {
        match self.body.expr(id) {
            Expr::Call { .. } => out.extend(self.call(id).relations),
            Expr::Keyword {
                keyword, modifiers, ..
            } if matches!(keyword.as_str(), "query" | "subscription") && !modifiers.is_empty() => {
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
                    return;
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
        // `Shape.Circle`: not a member of a value, a case of a type, which the
        // type must declare (PW0608, ADR-0059). Until 2026-09-26 it was
        // undecided, and `Shape.Bogus(1)` passed `pw check`.
        if let Some(c) = case_named(self.sigs, self.ws, self.at, &path_of(self.body, id)) {
            let (def, outcome) = match c {
                CaseNamed::Case(def, _) => (def, Outcome::Agree),
                CaseNamed::Unknown(def, written) => (
                    def,
                    Outcome::Disagree {
                        expected: self
                            .sigs
                            .type_decl(def)
                            .and_then(|t| t.variants.as_ref())
                            .map(|cases| {
                                cases
                                    .iter()
                                    .map(|(n, _)| format!("`{n}`"))
                                    .collect::<Vec<_>>()
                                    .join(", ")
                            })
                            .unwrap_or_default(),
                        actual: written,
                    },
                ),
            };
            return Some(ValueRelation {
                declaration: self.decl.name.clone(),
                kind: RelationKind::Case,
                span: self.body.expr_span(id),
                target: self.display_def(def),
                index: None,
                outcome,
                declared_at: None,
                boundary: (self.body.expr_span(base), "the type named".to_string()),
            });
        }
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
        let declared = self.name(lhs, x);
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
            Ty::Primitive(Primitive::Int | Primitive::Float) | Ty::Any => Outcome::Agree,
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
    lambda_binders(body, params)?
        .into_iter()
        .map(|p| match body.pat(p) {
            Pattern::Bind { name, .. } => Some(name.clone()),
            _ => None,
        })
        .collect()
}

/// **Where a lambda binds each of its parameters**: `(t, w) => ..` is one
/// parenthesised pattern holding two names. `None` where a parameter is not
/// a name.
pub(crate) fn lambda_binders(
    body: &Body,
    params: &[crate::hir::PatternId],
) -> Option<Vec<crate::hir::PatternId>> {
    let named = |p: &crate::hir::PatternId| matches!(body.pat(*p), Pattern::Bind { .. });
    if let [only] = params
        && let Pattern::Ctor { path, args } = body.pat(*only)
        && path.is_empty()
    {
        return args.iter().all(named).then(|| args.clone());
    }
    params.iter().all(named).then(|| params.to_vec())
}

// --- running it -----------------------------------------------------------------------

/// **Every value relation in one unit.** The single walk both the checker and
/// [`analysis`] use, so the two cannot disagree about which relations exist.
pub fn relations(hir: &Hir, sigs: &Signatures, ws: &Workspace, at: UnitId) -> Vec<ValueRelation> {
    let mut out = Vec::new();
    // Each declaration's bindings as typed, for the declarations nested in
    // it, which come after it (ADR-0066).
    let mut typed: BTreeMap<DeclId, BTreeMap<Binder, Ty>> = BTreeMap::new();
    for (id, decl) in hir.all_decls() {
        annotations(hir, sigs, ws, at, id, decl, &mut out);
        let Some(body_id) = decl.body else { continue };
        let body = hir.body(body_id);
        let lexical = Lexical::build_in(sigs, Some(at), hir, id)
            .unwrap_or_else(|| Lexical::build(sigs, Some(at), decl, body));
        let outer: Vec<Ty> = lexical
            .outer_bindings()
            .map(|(d, b)| {
                typed
                    .get(&d)
                    .and_then(|l| l.get(&b))
                    .cloned()
                    .unwrap_or(Ty::Unknown)
            })
            .collect();
        let typer = Typer::new(sigs, ws, at, hir.module_of(id), decl, body, lexical, outer);
        typer.relations(&mut out);
        typer.term_relations(&mut out);
        returns(&typer, sigs, at, id, decl, &mut out);
        typed.insert(id, typer.locals.borrow().clone());
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
    // or a `None`. Its success type is not returned, so it is any type
    // (ADR-0065).
    for site in typer.propagations() {
        let Expr::Try { value } = typer.body.expr(site) else {
            continue;
        };
        let returned = match typer.of(*value) {
            Ty::Builtin(Builtin::Result, args) if args.len() == 2 => {
                Ty::Builtin(Builtin::Result, vec![Ty::Any, args[1].clone()])
            }
            Ty::Builtin(Builtin::Option, _) => Ty::Builtin(Builtin::Option, vec![Ty::Any]),
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
            RelationKind::Case => Diagnostic::error(
                crate::codes::PATTERN_CONSTRUCTOR.id,
                crate::codes::PATTERN_CONSTRUCTOR.invariant,
                Detector::Signature,
                format!("`{actual}` is not a constructor of `{}`", r.target),
                r.span.clone(),
            )
            .reason("path_names_a_case_its_type_lacks")
            .explain(
                "a sum type's values are built by the cases it declares; a case it does not \
                 declare builds nothing, and a value of the type is not one of them",
            )
            .repair(format!("`{}`'s constructors are {expected}", r.target)),
            RelationKind::Fields => Diagnostic::error(
                crate::codes::RECORD_FIELDS.id,
                crate::codes::RECORD_FIELDS.invariant,
                Detector::Signature,
                match expected.as_str() {
                    FIELD_UNDECLARED => format!("`{}` has no field `{actual}`", r.target),
                    FIELD_ONCE => format!("`{}` is given its field `{actual}` twice", r.target),
                    _ => format!("`{}` is built without its field `{actual}`", r.target),
                },
                r.span.clone(),
            )
            .reason("record_built_with_other_fields")
            .explain(
                "a record's value holds each field its type declares, so it is built with each \
                 of them, once, and with nothing else",
            )
            .repair(match expected.as_str() {
                FIELD_UNDECLARED => format!("remove `{actual}`, or declare it on `{}`", r.target),
                FIELD_ONCE => format!("give `{actual}` once"),
                _ => format!("give `{actual}` a value"),
            }),
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
