//! **A type the compiler has resolved, and the only thing that may mean one.**
//!
//! Architect ruling, 2026-08-21:
//!
//! > **Written syntax can explain a resolved type. It cannot compete with it.**
//!
//! [`DeclaredType`] is a type *as the program wrote it* — `List<MenuItem>`, a
//! head and some argument spellings. It is what the parser produced and it is
//! not an identity: two modules declaring `opaque type SessionId = String` give
//! one spelling and two declarations, and every consumer that compared
//! spellings treated them as one type. That is what let `add_to_cart` hand the
//! platform's session id to something expecting the application's, with nothing
//! to say about it.
//!
//! A `ResolvedType` is the identity. Its internals are private to this module —
//! within one crate a `pub` field is visible everywhere, so a module boundary
//! is the only thing that makes *you must ask* enforceable — and the same
//! reasoning that produced [`DeclaredType`] applies with more at stake, because
//! this is what type checking, privacy comparison and the contract's semantic
//! signature will all consume.
//!
//! # Resolution is recursive, or it is nothing
//!
//! > There should never be a successful
//! >
//! > ```text
//! > ResolvedType { head: DefId, args: ["MenuItem"] }   // still textual
//! > ```
//!
//! So [`resolve`] resolves the head **and every argument**, transitively, and
//! reports rather than half-succeeding. A partly-semantic representation is the
//! thing this module exists to prevent: it would look resolved at every call
//! site and carry a spelling underneath, which is the original defect wearing a
//! new type.
//!
//! # What it is not
//!
//! Not the checker's [`crate::types::Type`], which is an interned environment
//! keyed by bare NAME — the representation that collapses `alpha.Tag` and
//! `beta.Tag` into one entry. Migrating its consumers is step 3b; this module
//! is what they migrate onto.
//!
//! Not the ABI. `opaque type Tag = String` says how a `Tag` is REPRESENTED and
//! says nothing about whether a `Tag` may be used as a `String`. Representation
//! transparency belongs to ABI lowering; two nominal types with one
//! representation stay two types here.

use serde::{Deserialize, Serialize};

use crate::hir::{DeclaredType, Span};
use crate::resolve::{DefId, Namespace, Resolution, Workspace};

pub use resolved_type::ResolvedType;

/// The primitives, which are types without declarations.
///
/// Deliberately closed and tiny. A name that is not one of these and does not
/// resolve to a declaration is [`Unresolved`](TypeResolution::Unresolved) — the
/// alternative is a placeholder that means *something*, and a placeholder is
/// how `Type::Str` came to stand for every unresolved field.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum Primitive {
    Bool,
    Int,
    Float,
    Str,
    Unit,
}

impl Primitive {
    fn of(name: &str) -> Option<Primitive> {
        Some(match name {
            "Bool" => Primitive::Bool,
            "Int" => Primitive::Int,
            "Float" => Primitive::Float,
            "String" | "Str" => Primitive::Str,
            "Unit" => Primitive::Unit,
            _ => return None,
        })
    }

    /// The name a diagnostic uses. `String`, not `Str` — one spelling in
    /// messages, whichever the source used.
    pub fn name(self) -> &'static str {
        match self {
            Primitive::Bool => "Bool",
            Primitive::Int => "Int",
            Primitive::Float => "Float",
            Primitive::Str => "String",
            Primitive::Unit => "Unit",
        }
    }
}

/// **The type constructors the language provides**, which have no declaration.
///
/// `List`, `Option` and `Result` are not declared anywhere a program can point
/// at, so they cannot be nominal — and they are not primitives either, because
/// they take arguments and `List<MenuItem>` differs from `List<Store>` by them.
/// A third kind rather than a fudge in either direction.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum Builtin {
    List,
    Option,
    Result,
}

impl Builtin {
    fn of(name: &str) -> Option<Builtin> {
        Some(match name {
            "List" => Builtin::List,
            "Option" => Builtin::Option,
            "Result" => Builtin::Result,
            _ => return None,
        })
    }

    pub fn name(self) -> &'static str {
        match self {
            Builtin::List => "List",
            Builtin::Option => "Option",
            Builtin::Result => "Result",
        }
    }
}

/// **What resolving a written type concluded.**
///
/// Three-valued, like `Lowering`, `Encoding` and `MatchOutcome`, and for the
/// same reason: *resolved*, *this name is not a type here*, and *an upstream
/// stage did not give me what I need* are three different facts, and a consumer
/// that cannot tell the second from the third will report a missing import as a
/// type error.
/// No `PartialEq` either, for the reason [`ResolvedType`] has none: a derived
/// one would compare spans.
#[derive(Debug, Clone)]
pub enum TypeResolution {
    Resolved(ResolvedType),
    /// A name in the written type resolves to nothing. Carries the offending
    /// name rather than the whole type, because `List<Stroe>` is one typo and
    /// naming `List<Stroe>` would send the reader to the wrong place.
    Unresolved {
        name: String,
        written: String,
    },
    /// Resolution could not run. Not a program error and emphatically not
    /// something to fill in with a placeholder.
    Blocked {
        why: String,
    },
}

impl TypeResolution {
    pub fn resolved(&self) -> Option<&ResolvedType> {
        match self {
            TypeResolution::Resolved(t) => Some(t),
            _ => None,
        }
    }

    pub fn is_resolved(&self) -> bool {
        matches!(self, TypeResolution::Resolved(_))
    }
}

impl std::fmt::Display for TypeResolution {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            TypeResolution::Resolved(t) => f.write_str(&t.display_name()),
            TypeResolution::Unresolved { name, written } => {
                write!(f, "`{name}` in `{written}` is not a type here")
            }
            TypeResolution::Blocked { why } => write!(f, "blocked: {why}"),
        }
    }
}

/// **Resolve a written type, head and every argument.**
///
/// `params` is the type parameters the enclosing declaration binds — `T` in
/// `fn identity<T>(x: T) -> T` is not a missing declaration, it is a bound
/// variable, and a resolver that did not know the difference would report every
/// generic as unresolved.
///
/// Fails on the FIRST name that does not resolve rather than collecting: a type
/// with one bad argument has one defect, and reporting `List<Stroe>` as two
/// findings teaches the reader to distrust the count.
pub fn resolve(
    ws: &Workspace,
    at: usize,
    binder: Option<DefId>,
    params: &[String],
    written: &DeclaredType,
    span: Span,
) -> TypeResolution {
    let text = written.written();

    // A type parameter is an identity, but not a declaration's.
    // `constructor_head_only` is legitimate here in the one way the allow-list
    // describes: the question IS the constructor, because a parameter binds no
    // arguments.
    let head = written.constructor_head_only();
    if let Some(index) = params.iter().position(|p| p == head) {
        if !written.args().is_empty() {
            return TypeResolution::Blocked {
                why: format!(
                    "`{text}` applies arguments to the type parameter `{head}`, which this \
                     build does not model"
                ),
            };
        }
        // **A parameter without its binder is not an identity.** `T` means
        // nothing on its own — two declarations each binding a `T` bind two
        // different variables — so a caller that cannot say which declaration
        // binds it gets a refusal rather than a name to compare.
        let Some(binder) = binder else {
            return TypeResolution::Blocked {
                why: format!(
                    "`{head}` is a type parameter and no binding declaration was supplied"
                ),
            };
        };
        return TypeResolution::Resolved(ResolvedType::parameter(
            head,
            binder,
            index as u32,
            span,
            written.clone(),
        ));
    }

    let mut args = Vec::new();
    for a in written.args() {
        // An argument is itself a written type. Parsed shallowly here because
        // `DeclaredType` carries arguments as spellings; a nested
        // `List<Option<Store>>` is step 3b's, and saying so is better than
        // half-resolving it.
        if a.contains('<') {
            return TypeResolution::Blocked {
                why: format!(
                    "`{text}` nests a generic argument, which needs a written type that carries \
                     its own arguments"
                ),
            };
        }
        let inner = DeclaredType::new(a.clone(), Vec::new());
        match resolve(ws, at, binder, params, &inner, span.clone()) {
            TypeResolution::Resolved(t) => args.push(t),
            TypeResolution::Unresolved { name, .. } => {
                return TypeResolution::Unresolved {
                    name,
                    written: text,
                };
            }
            blocked => return blocked,
        }
    }

    if let Some(p) = Primitive::of(head) {
        if !args.is_empty() {
            return TypeResolution::Blocked {
                why: format!("`{text}` applies arguments to the primitive `{head}`"),
            };
        }
        return TypeResolution::Resolved(ResolvedType::primitive(p, span, written.clone()));
    }

    if let Some(b) = Builtin::of(head) {
        // A bare `Option` is not a type — it does not say what may be absent.
        // The same refusal `platform_contracts.rs` makes of a signature.
        if args.is_empty() {
            return TypeResolution::Unresolved {
                name: head.to_string(),
                written: text,
            };
        }
        return TypeResolution::Resolved(ResolvedType::builtin(b, args, span, written.clone()));
    }

    // A dotted head is a path — `domain.SessionId` — and a bare one is a name
    // visible from this unit. Both go through the workspace, which is what
    // makes `capability.SessionId` and `domain.SessionId` two answers.
    let found = match head.contains('.') {
        true => ws.resolve_path(at, head),
        false => ws.resolve_in(at, Namespace::Type, head),
    };
    let def = match found {
        Resolution::Local(d) => d,
        Resolution::Imported { def, .. } => def,
        Resolution::Ambiguous(_) | Resolution::Unresolved => {
            return TypeResolution::Unresolved {
                name: head.to_string(),
                written: text,
            };
        }
    };
    TypeResolution::Resolved(ResolvedType::nominal(def, args, span, written.clone()))
}

/// Private internals, so *you must ask* is enforceable rather than encouraged.
mod resolved_type {
    use super::{Builtin, Primitive};
    use crate::hir::{DeclaredType, Span};
    use crate::resolve::DefId;

    /// **No `PartialEq`.** `same_as` is the only comparison, and deriving one
    /// gave a second that disagreed with it: `origin` holds a span, so two uses
    /// of one type at two places were `same_as` and not `==`. A consumer
    /// reaching for `==` got *different types* for the same type — two
    /// authorities for one question, in the module written to delete exactly
    /// that. Found by reading the derive while making an unrelated change.
    #[derive(Debug, Clone)]
    pub struct ResolvedType {
        what: What,
        origin: Origin,
    }

    #[derive(Debug, Clone)]
    enum What {
        /// A declared type, by the identity of its declaration.
        Nominal {
            def: DefId,
            args: Vec<ResolvedType>,
        },
        /// A type parameter, by the declaration that binds it and its
        /// position. The NAME is provenance: `fn id<T>` and `fn id<U>` bind the
        /// same variable, so a comparison on the spelling would make a rename a
        /// semantic change.
        Parameter {
            name: String,
            binder: DefId,
            index: u32,
        },
        Primitive(Primitive),
        /// `List<T>`, `Option<T>`, `Result<T, E>` — a constructor the language
        /// provides, with its arguments resolved.
        Builtin {
            ctor: Builtin,
            args: Vec<ResolvedType>,
        },
    }

    /// **Provenance, never a second semantic type.**
    ///
    /// > Written syntax can explain a resolved type. It cannot compete with it.
    ///
    /// So the spelling is reachable for a diagnostic that wants to echo what
    /// the programmer typed, and is not reachable in a form a consumer could
    /// compare.
    #[derive(Debug, Clone)]
    struct Origin {
        span: Span,
        written: DeclaredType,
    }

    impl ResolvedType {
        pub(super) fn nominal(
            def: DefId,
            args: Vec<ResolvedType>,
            span: Span,
            written: DeclaredType,
        ) -> ResolvedType {
            ResolvedType {
                what: What::Nominal { def, args },
                origin: Origin { span, written },
            }
        }

        pub(super) fn parameter(
            name: &str,
            binder: DefId,
            index: u32,
            span: Span,
            written: DeclaredType,
        ) -> ResolvedType {
            ResolvedType {
                what: What::Parameter {
                    name: name.to_string(),
                    binder,
                    index,
                },
                origin: Origin { span, written },
            }
        }

        pub(super) fn primitive(p: Primitive, span: Span, written: DeclaredType) -> ResolvedType {
            ResolvedType {
                what: What::Primitive(p),
                origin: Origin { span, written },
            }
        }

        pub(super) fn builtin(
            ctor: Builtin,
            args: Vec<ResolvedType>,
            span: Span,
            written: DeclaredType,
        ) -> ResolvedType {
            ResolvedType {
                what: What::Builtin { ctor, args },
                origin: Origin { span, written },
            }
        }

        /// The language-provided constructor this is, if it is one.
        pub fn as_builtin(&self) -> Option<Builtin> {
            match &self.what {
                What::Builtin { ctor, .. } => Some(*ctor),
                _ => None,
            }
        }

        /// The declaration this names, for a nominal type.
        ///
        /// `None` for a primitive and for a type parameter — neither is a
        /// declaration, and answering with one would be inventing an identity.
        pub fn def_id(&self) -> Option<DefId> {
            match &self.what {
                What::Nominal { def, .. } => Some(*def),
                _ => None,
            }
        }

        /// The resolved arguments. Empty for a non-generic type.
        pub fn args(&self) -> &[ResolvedType] {
            match &self.what {
                What::Nominal { args, .. } | What::Builtin { args, .. } => args,
                _ => &[],
            }
        }

        /// The type parameter this is, if it is one.
        /// The parameter's spelling, for a message. Provenance, not identity.
        pub fn type_parameter(&self) -> Option<&str> {
            match &self.what {
                What::Parameter { name, .. } => Some(name),
                _ => None,
            }
        }

        /// **What a type parameter IS**: which declaration binds it, and where
        /// in that declaration's list. This is the identity; the name above is
        /// not.
        pub fn parameter_binding(&self) -> Option<(DefId, u32)> {
            match &self.what {
                What::Parameter { binder, index, .. } => Some((*binder, *index)),
                _ => None,
            }
        }

        /// The primitive this is, if it is one.
        pub fn as_primitive(&self) -> Option<Primitive> {
            match &self.what {
                What::Primitive(p) => Some(*p),
                _ => None,
            }
        }

        /// **Do these two mean the same type?**
        ///
        /// By resolved identity, recursively through arguments. Never by
        /// representation: `alpha.Tag` and `beta.Tag` are both `String`
        /// underneath and are two types, which is the whole reason this module
        /// exists.
        pub fn same_as(&self, other: &ResolvedType) -> bool {
            match (&self.what, &other.what) {
                (What::Nominal { def: a, args: xs }, What::Nominal { def: b, args: ys }) => {
                    a == b && xs.len() == ys.len() && xs.iter().zip(ys).all(|(x, y)| x.same_as(y))
                }
                (What::Builtin { ctor: a, args: xs }, What::Builtin { ctor: b, args: ys }) => {
                    a == b && xs.len() == ys.len() && xs.iter().zip(ys).all(|(x, y)| x.same_as(y))
                }
                // By binder and position, never by spelling — `fn id<T>` and
                // `fn id<U>` bind the same variable.
                (
                    What::Parameter {
                        binder: a,
                        index: i,
                        ..
                    },
                    What::Parameter {
                        binder: b,
                        index: j,
                        ..
                    },
                ) => a == b && i == j,
                (What::Primitive(a), What::Primitive(b)) => a == b,
                _ => false,
            }
        }

        /// **The name a diagnostic shows**, which is what the programmer wrote.
        ///
        /// `expected SessionId` reads better than a qualified path, and the
        /// reader is looking at the spelling in their own file.
        pub fn display_name(&self) -> String {
            self.origin.written.written()
        }

        /// The spelling, for provenance. Not for comparison — that is
        /// `same_as`, and this returns a `String` precisely so that using it as
        /// an identity looks like what it is.
        pub fn written_source(&self) -> String {
            self.origin.written.written()
        }

        pub fn span(&self) -> Span {
            self.origin.span.clone()
        }
    }

    impl std::fmt::Display for ResolvedType {
        fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
            f.write_str(&self.display_name())
        }
    }
}

// --- the artifact's identity -------------------------------------------------

/// **A type identity a host artifact can carry.**
///
/// Architect ruling, 2026-08-21:
///
/// > `DefId` is compiler-process identity and shouldn't become the public host
/// > artifact format. So `ComponentContract.signature` should be a
/// > serialization **derived from** the resolved signature, using a stable
/// > semantic type identity.
///
/// ```text
/// ResolvedType / DefId  →  StableTypeId  →  contract artifact
/// ```
///
/// and never
///
/// ```text
/// source spelling → contract          WIT name → guess what the source was
/// ```
///
/// # What makes it stable
///
/// A `DefId` is a unit index and a declaration index — both artefacts of how
/// this build enumerated files. Compile the same program with its sources in a
/// different order and every `DefId` moves. A `StableTypeId` names the
/// declaring MODULE and the declaration, so it is the same across builds,
/// machines and file orderings. `the_identity_survives_a_different_file_order`
/// is what says so, and it is the whole point of the type existing.
///
/// It is emphatically not the spelling. `SessionId` is one spelling and was two
/// declarations; `capability.SessionId` names one of them.
/// **The declaration that binds a type parameter**, stably.
///
/// `store.page.add_to_cart` — the declaring module and the declaration, the
/// same construction [`StableTypeId::Declared`] uses and for the same reason.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub struct StableDeclId(String);

impl StableDeclId {
    pub fn new(path: impl Into<String>) -> StableDeclId {
        StableDeclId(path.into())
    }

    pub fn path(&self) -> &str {
        &self.0
    }
}

impl std::fmt::Display for StableDeclId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.0)
    }
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum StableTypeId {
    /// `capability.SessionId`, with its arguments resolved.
    Declared {
        /// The declaring module and the declaration: `capability.SessionId`.
        path: String,
        #[serde(default, skip_serializing_if = "Vec::is_empty")]
        args: Vec<StableTypeId>,
    },
    /// `Int`, `String` — a type with no declaration to name.
    Primitive(String),
    /// `List<..>`, `Option<..>`, `Result<..>`.
    Builtin {
        ctor: String,
        args: Vec<StableTypeId>,
    },
    /// **A type parameter, by its binder and its position.**
    ///
    /// Architect ruling, 2026-08-21: these must be α-equivalent —
    ///
    /// ```text
    /// fn id<T>(x: T) -> T
    /// fn id<U>(x: U) -> U
    /// ```
    ///
    /// > Renaming a generic parameter should not change the public semantic
    /// > contract.
    ///
    /// So the spelling is **not** here. It was, for one commit, and it would
    /// have made a rename a contract change — the same principle applied
    /// everywhere else in this module, missed in the one place the identity is
    /// a name by nature: *spelling explains identity; spelling is not
    /// identity.* The spelling stays reachable on the `ResolvedType` this was
    /// derived from.
    Parameter { binder: StableDeclId, index: u32 },
}

impl std::fmt::Display for StableTypeId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let args = |a: &[StableTypeId]| {
            a.iter()
                .map(|x| x.to_string())
                .collect::<Vec<_>>()
                .join(", ")
        };
        match self {
            StableTypeId::Declared { path, args: a } if a.is_empty() => f.write_str(path),
            StableTypeId::Declared { path, args: a } => write!(f, "{path}<{}>", args(a)),
            StableTypeId::Primitive(p) => f.write_str(p),
            StableTypeId::Builtin { ctor, args: a } => write!(f, "{ctor}<{}>", args(a)),
            StableTypeId::Parameter { binder, index } => write!(f, "{binder}#{index}"),
        }
    }
}

/// **Derive the artifact identity from the resolved one.**
///
/// One direction only. Information may be discarded going this way — a span, a
/// spelling — and nothing downstream may recover what an earlier representation
/// failed to know.
///
/// `None` when the declaration a nominal type names cannot be found, which is a
/// program this build cannot describe rather than a type to guess at.
pub fn stable(hirs: &[&crate::hir::Hir], t: &ResolvedType) -> Option<StableTypeId> {
    let args = |xs: &[ResolvedType]| -> Option<Vec<StableTypeId>> {
        xs.iter().map(|x| stable(hirs, x)).collect()
    };
    if let Some(p) = t.as_primitive() {
        return Some(StableTypeId::Primitive(p.name().to_string()));
    }
    if let Some(b) = t.as_builtin() {
        return Some(StableTypeId::Builtin {
            ctor: b.name().to_string(),
            args: args(t.args())?,
        });
    }
    if let Some((binder, index)) = t.parameter_binding() {
        let hir = hirs.get(binder.unit)?;
        let (id, decl) = hir.all_decls().find(|(id, _)| id.0 == binder.decl)?;
        let module = hir.module_of(id).unwrap_or_default();
        let path = match module.is_empty() {
            true => decl.name.clone(),
            false => format!("{module}.{}", decl.name),
        };
        return Some(StableTypeId::Parameter {
            binder: StableDeclId::new(path),
            index,
        });
    }
    let def = t.def_id()?;
    let hir = hirs.get(def.unit)?;
    let (id, decl) = hir.all_decls().find(|(id, _)| id.0 == def.decl)?;
    let module = hir.module_of(id).unwrap_or_default();
    let path = match module.is_empty() {
        true => decl.name.clone(),
        false => format!("{module}.{}", decl.name),
    };
    Some(StableTypeId::Declared {
        path,
        args: args(t.args())?,
    })
}
