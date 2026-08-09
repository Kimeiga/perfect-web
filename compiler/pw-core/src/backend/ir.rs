//! **The Backend IR — what every backend lowers from, and what none of them
//! may re-derive.**
//!
//! Architect ruling, 2026-08-09, scoping E10-A:
//!
//! > Do not lower directly from HIR into Wasm instructions. […] E10 will later
//! > want multiple backends. The stable division should be:
//! >
//! > ```text
//! > Pleris semantics
//! >       ↓
//! > Backend IR
//! >     /       \
//! >  Wasm       future native backend
//! > ```
//!
//! and the rule that shapes every type in this file:
//!
//! > The backend consumes semantics already established upstream. It must not
//! > rediscover effects, capabilities, placement, privacy, or ABI types.
//!
//! # Nothing here is a name
//!
//! > `DefId`, not names. `DeclaredType`, not reconstructed strings.
//! > `CapabilityId`, not `"database.write"`.
//!
//! This project has spent its life deleting spelling-based resolution —
//! `docs/RISK_QUEUE.md` is mostly instances of it — and a backend that took a
//! string would be reintroducing it at the point where the answer becomes
//! machine code. So [`Type::Nominal`] holds a `DefId`, a call names a `DefId`,
//! and a host call names a [`CapabilityId`] the contract already derived.
//!
//! If a lowering finds itself formatting a name to decide something, that is
//! the bug this module is shaped to prevent.
//!
//! # Three outcomes, never a silent one
//!
//! > Lowering should return `Lowered | Unsupported(reason) | Blocked(upstream
//! > error)`, not turn an unknown HIR node into no instructions.
//!
//! [`Lowering`] is that. A construct the backend does not support yet produces
//! a diagnostic naming it; an upstream error produces `Blocked`, because a
//! program that did not check is not a program with a backend problem. Neither
//! is an empty instruction list, which is what "silently lower differently"
//! looks like from the outside — and which would produce a component that runs
//! and does the wrong thing.

use std::fmt;

use crate::hir::Span;
use crate::resolve::DefId;

/// One value, in one function. SSA: assigned once, at the instruction that
/// produces it.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct ValueId(pub u32);

/// One basic block, in one function.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct BlockId(pub u32);

/// **A capability, as the contract already derived it.**
///
/// A newtype over `contract::Capability` rather than a second derivation: that
/// type is ADR-0020's frozen boundary, and E9's ontology pull-forward exists
/// precisely so this identity survives changes to inference, row representation
/// and syntax. A backend inventing its own capability name would be the second
/// answer the pull-forward was meant to prevent.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub struct CapabilityId(pub crate::contract::Capability);

impl CapabilityId {
    /// The canonical text, for a host import name. Read from the contract's own
    /// spelling so the two cannot drift.
    pub fn name(&self) -> String {
        self.0.name()
    }
}

/// **A fully resolved type.**
///
/// No strings, and no `DeclaredType` either — that is the SOURCE form, and
/// carrying it here would make every backend re-resolve `Cart` against a
/// workspace it does not have. A nominal type is a `DefId`.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum Type {
    Int,
    Float,
    Bool,
    Str,
    /// Nothing crosses. A command with no declared return.
    Unit,
    /// A record, variant or opaque type, by the declaration that defines it.
    Nominal(DefId),
    Result(Box<Type>, Box<Type>),
    Option(Box<Type>),
    List(Box<Type>),
}

impl Type {
    /// Every nominal type reachable inside this one.
    ///
    /// The backend needs the transitive set to emit type definitions, and
    /// walking here rather than at each call site is the same lesson
    /// `DeclaredType` encodes: a type read one level deep is not the type.
    pub fn nominals(&self) -> Vec<DefId> {
        let mut out = Vec::new();
        self.walk(&mut |t| {
            if let Type::Nominal(d) = t {
                out.push(*d);
            }
        });
        out
    }

    fn walk(&self, f: &mut impl FnMut(&Type)) {
        f(self);
        match self {
            Type::Result(a, b) => {
                a.walk(f);
                b.walk(f);
            }
            Type::Option(a) | Type::List(a) => a.walk(f),
            _ => {}
        }
    }
}

/// A literal the backend can materialize without calling anything.
#[derive(Debug, Clone, PartialEq)]
pub enum Const {
    Int(i64),
    Float(f64),
    Bool(bool),
    Str(String),
    Unit,
}

/// One instruction. Every one names the value it produces and that value's
/// type, so a consumer never infers either.
#[derive(Debug, Clone, PartialEq)]
pub enum Instr {
    Const {
        result: ValueId,
        value: Const,
        ty: Type,
    },
    /// A call to another Pleris declaration, by resolved identity.
    Call {
        result: ValueId,
        callee: DefId,
        args: Vec<ValueId>,
        ty: Type,
    },
    /// **A call across the capability boundary.**
    ///
    /// Separate from `Call` because it is a different kind of thing, not a
    /// call that happens to be external: the host decides whether it may
    /// happen at all, `Granted` is what makes it linkable, and the E8 audit
    /// compares the artifact's imports against exactly these. Flattening the
    /// two would make "which of my calls need authority" a question somebody
    /// answers by looking at names again.
    HostCall {
        result: ValueId,
        capability: CapabilityId,
        args: Vec<ValueId>,
        ty: Type,
    },
    /// Build a record or a variant case.
    Construct {
        result: ValueId,
        ctor: DefId,
        args: Vec<ValueId>,
        ty: Type,
    },
    /// Read a field, by index. The name was resolved upstream.
    Project {
        result: ValueId,
        of: ValueId,
        field: u32,
        ty: Type,
    },
}

impl Instr {
    pub fn result(&self) -> ValueId {
        match self {
            Instr::Const { result, .. }
            | Instr::Call { result, .. }
            | Instr::HostCall { result, .. }
            | Instr::Construct { result, .. }
            | Instr::Project { result, .. } => *result,
        }
    }

    pub fn ty(&self) -> &Type {
        match self {
            Instr::Const { ty, .. }
            | Instr::Call { ty, .. }
            | Instr::HostCall { ty, .. }
            | Instr::Construct { ty, .. }
            | Instr::Project { ty, .. } => ty,
        }
    }
}

/// How a block ends. Every block has one — there is no falling off the end.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Terminator {
    Return(ValueId),
    Jump(BlockId),
    Branch {
        cond: ValueId,
        then_block: BlockId,
        else_block: BlockId,
    },
    /// Reached only by a program that already failed to check. Emitted so the
    /// IR stays well-formed rather than leaving a block with no terminator,
    /// which is the shape a later pass silently walks off the end of.
    Unreachable,
}

#[derive(Debug, Clone, PartialEq)]
pub struct Block {
    pub id: BlockId,
    pub instrs: Vec<Instr>,
    pub terminator: Terminator,
}

/// One lowered declaration.
#[derive(Debug, Clone, PartialEq)]
pub struct Function {
    /// The declaration this is, by resolved identity.
    pub def: DefId,
    /// Its exported name, for the component's interface. Carried rather than
    /// derived: `contract::contracts` decided it, and a second construction is
    /// how the two come to disagree.
    pub export: String,
    pub params: Vec<(ValueId, Type)>,
    pub ret: Type,
    pub blocks: Vec<Block>,
    /// Every capability this function calls across. **Not re-derived** — it is
    /// the contract's `required_capabilities`, narrowed to this declaration,
    /// and the E8 audit compares the built artifact against it.
    pub capabilities: Vec<CapabilityId>,
}

impl Function {
    pub fn entry(&self) -> Option<&Block> {
        self.blocks.first()
    }
}

/// A whole program, lowered.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct Program {
    pub functions: Vec<Function>,
    /// Nominal types the functions use, in dependency order.
    pub types: Vec<TypeDef>,
}

/// A nominal type's shape, resolved.
#[derive(Debug, Clone, PartialEq)]
pub struct TypeDef {
    pub def: DefId,
    pub name: String,
    pub shape: Shape,
}

#[derive(Debug, Clone, PartialEq)]
pub enum Shape {
    Record {
        fields: Vec<(String, Type)>,
    },
    Variant {
        cases: Vec<(String, Option<Type>)>,
    },
    /// An opaque type, as its representation. A wire has no opacity to offer —
    /// the same decision `wit.rs` records and for the same reason.
    Alias(Box<Type>),
}

/// **What a lowering produced, and never silently nothing.**
///
/// Architect ruling, 2026-08-09: *"Lowering should return `Lowered |
/// Unsupported(reason) | Blocked(upstream error)`, not turn an unknown HIR node
/// into no instructions."*
#[derive(Debug, Clone, PartialEq)]
pub enum Lowering<T> {
    Lowered(T),
    /// The backend does not handle this construct yet.
    ///
    /// A real diagnostic, naming the construct. E10-A supports what
    /// `add_to_cart` needs and nothing more, so this is the ordinary answer for
    /// most of the language — and it must stay loud, because a construct
    /// lowered to nothing produces a component that runs and does the wrong
    /// thing.
    Unsupported {
        construct: &'static str,
        span: Span,
        reason: String,
    },
    /// The program did not check. Not the backend's problem, and not the
    /// backend's diagnostic to invent — reporting it here would deliver one
    /// defect twice in two vocabularies.
    Blocked {
        why: String,
        span: Span,
    },
}

impl<T> Lowering<T> {
    pub fn is_lowered(&self) -> bool {
        matches!(self, Lowering::Lowered(_))
    }

    pub fn lowered(self) -> Option<T> {
        match self {
            Lowering::Lowered(t) => Some(t),
            _ => None,
        }
    }

    /// Map the payload, keeping the refusal.
    pub fn map<U>(self, f: impl FnOnce(T) -> U) -> Lowering<U> {
        match self {
            Lowering::Lowered(t) => Lowering::Lowered(f(t)),
            Lowering::Unsupported {
                construct,
                span,
                reason,
            } => Lowering::Unsupported {
                construct,
                span,
                reason,
            },
            Lowering::Blocked { why, span } => Lowering::Blocked { why, span },
        }
    }
}

impl<T> fmt::Display for Lowering<T> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Lowering::Lowered(_) => f.write_str("lowered"),
            Lowering::Unsupported {
                construct, reason, ..
            } => write!(f, "the backend does not lower {construct} yet: {reason}"),
            Lowering::Blocked { why, .. } => write!(f, "blocked upstream: {why}"),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn def(n: u32) -> DefId {
        DefId { unit: 0, decl: n }
    }

    #[test]
    fn a_type_reports_every_nominal_inside_it() {
        // The lesson `DeclaredType` encodes, in the IR's own vocabulary: a type
        // read one level deep is not the type. `Result<Cart, CartError>` uses
        // two nominals and neither is at the head.
        let t = Type::Result(
            Box::new(Type::Nominal(def(1))),
            Box::new(Type::Nominal(def(2))),
        );
        assert_eq!(t.nominals(), vec![def(1), def(2)]);

        // And through a carrier of a carrier.
        let nested = Type::Option(Box::new(Type::List(Box::new(Type::Nominal(def(3))))));
        assert_eq!(nested.nominals(), vec![def(3)]);

        // The control: a type with no nominal reports none, so "found some" is
        // not what this says about everything.
        assert!(
            Type::Result(Box::new(Type::Int), Box::new(Type::Str))
                .nominals()
                .is_empty()
        );
    }

    #[test]
    fn an_unsupported_construct_is_not_an_empty_lowering() {
        // The distinction the whole enum exists for. Both of these produce no
        // instructions; only one of them is a program the backend understood.
        let empty: Lowering<Vec<Instr>> = Lowering::Lowered(vec![]);
        let refused: Lowering<Vec<Instr>> = Lowering::Unsupported {
            construct: "a pipeline",
            span: 0..1,
            reason: "E10-A lowers what `add_to_cart` needs".into(),
        };
        assert!(empty.is_lowered());
        assert!(!refused.is_lowered());
        assert!(refused.to_string().contains("does not lower a pipeline"));
    }

    #[test]
    fn map_keeps_a_refusal_rather_than_producing_a_default() {
        let refused: Lowering<u32> = Lowering::Blocked {
            why: "the program does not check".into(),
            span: 0..1,
        };
        let mapped = refused.map(|n| n + 1);
        assert!(matches!(mapped, Lowering::Blocked { .. }));
        assert!(mapped.lowered().is_none());
    }

    #[test]
    fn a_host_call_is_not_an_ordinary_call() {
        // They are different kinds of thing, not one with a flag. The E8 audit
        // compares the artifact's imports against exactly the host ones, and a
        // flattened representation would make "which calls need authority" a
        // question somebody answers by reading names.
        let host = Instr::HostCall {
            result: ValueId(0),
            capability: CapabilityId(crate::contract::Capability::parse("database.write<Carts>")),
            args: vec![],
            ty: Type::Unit,
        };
        let ordinary = Instr::Call {
            result: ValueId(1),
            callee: def(7),
            args: vec![],
            ty: Type::Unit,
        };
        assert!(matches!(host, Instr::HostCall { .. }));
        assert!(matches!(ordinary, Instr::Call { .. }));
        let Instr::HostCall { capability, .. } = &host else {
            unreachable!()
        };
        assert_eq!(capability.name(), "database.write<Carts>");
    }
}
