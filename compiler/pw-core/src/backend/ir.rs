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
    /// **A call to something this component does not contain.**
    ///
    /// Separate from `Call` because it is a different kind of thing, not a call
    /// that happens to be external: the implementation comes from somewhere
    /// else, that somewhere is named by an `ImportId` the artifact carries, and
    /// the E8 audit compares the built thing's imports against exactly these.
    ///
    /// It was `HostCall { capability }` until 2026-08-20, and that was the
    /// model error the encoder found: a capability authorizes an operation and
    /// does not identify one, so two functions sharing an authority had one
    /// import with two ABIs. The authority is now a property of the IMPORT, not
    /// of the call.
    ImportCall {
        result: ValueId,
        import: ImportId,
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
            | Instr::ImportCall { result, .. }
            | Instr::Construct { result, .. }
            | Instr::Project { result, .. } => *result,
        }
    }

    pub fn ty(&self) -> &Type {
        match self {
            Instr::Const { ty, .. }
            | Instr::Call { ty, .. }
            | Instr::ImportCall { ty, .. }
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
    /// **The host functions this program imports, as the CONTRACT names them.**
    ///
    /// Carried rather than derived, for the reason `Function::export` is:
    /// `contract::contracts` decided which interface serves a capability, from
    /// the effect's own declaration, and an encoder that worked it out again
    /// would be the second derivation this project keeps deleting. The E8 audit
    /// compares the built artifact's imports against the contract — so if the
    /// encoder invented the name, the audit would be comparing a guess with
    /// itself.
    pub imports: Vec<CallableImport>,
}

/// **A callable this component depends on, with the ABI to call it.**
///
/// Architect ruling, 2026-08-20, after the encoder proved the previous model
/// wrong:
///
/// > **A capability authorizes an operation. It does not identify the
/// > operation.** So `database.write<Carts>` must never be used as the callable
/// > import identity.
///
/// `Carts.add(s, item, qty)` and `Carts.clear(s)` both require
/// `database.write<Carts>` and have different ABIs. An import keyed on the
/// capability had no signature it could honestly carry, and the first thing the
/// validator said was *"expected i32 but nothing on stack"*.
///
/// Four separate facts, kept separate:
///
/// ```text
/// CapabilityId       authority
/// ImportId           callable operation identity
/// BackendSignature   callable ABI
/// interface          physical/component-model grouping
/// ```
///
/// The grouping is an ABI/package-layout decision and must not determine the
/// capability semantics — the same distinction E8 already draws between
/// semantic component granularity and physical bundling.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CallableImport {
    pub id: ImportId,
    /// The source declaration this is, for provenance. A compiler-internal
    /// identity: deliberately NOT what the component boundary depends on,
    /// because a `DefId` does not survive past this compiler.
    pub callee: DefId,
    /// How the implementation is supplied.
    pub binding: ImportBinding,
    /// The complete checked ABI. **This is what the encoder reads** — a
    /// signature it derived from call sites would be a second answer to a
    /// question the front end settled, and the arity conflict is what happens
    /// when there is no first answer.
    pub signature: BackendSignature,
    /// Authority required to invoke it. A **set**: an operation may need
    /// several, and one capability may authorize many operations — proven by
    /// `Carts.add` and `Carts.clear` sharing `database.write<Carts>`.
    ///
    /// May be empty. An operation can be placement-constrained and need no
    /// authority at all, which is already true of the browser-semantic effects.
    pub required_capabilities: Vec<CapabilityId>,
}

/// A callable import's identity, stable across the component boundary.
///
/// The interface and the operation within it — `pw:host/carts` and `add`. Not a
/// `DefId`: this is what a WIT world and a built artifact name, and it has to
/// mean something to a toolchain that never saw this compiler.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub struct ImportId {
    pub interface: String,
    pub name: String,
}

impl ImportId {
    pub fn qualified(&self) -> String {
        format!("{}#{}", self.interface, self.name)
    }
}

/// How a callable import's implementation is supplied.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ImportBinding {
    /// **The Pleris platform defines it and the host implements it.**
    /// `pw:host/session#read`. Platform ABI-stability expectations apply.
    PlatformHost,
    /// **The deployment supplies it, and the semantics are the
    /// application's.** `store:data/carts#add`.
    ///
    /// Architect ruling, 2026-08-20: *"The host process currently provides the
    /// implementation" is a deployment fact; it doesn't need to collapse their
    /// semantic ownership.* An application repository method is externally
    /// implemented today and is expected to become compiled Pleris over
    /// narrower platform data primitives — recorded in `docs/NEXT.md` so
    /// `pw:host/carts` cannot become the standard library by default.
    External,
    /// Another component exports it.
    Component,
}

/// A callable's ABI, as the checker established it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BackendSignature {
    pub params: Vec<Type>,
    pub result: Type,
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
    fn an_import_call_is_not_an_ordinary_call() {
        // They are different kinds of thing, not one with a flag. The E8 audit
        // compares the artifact's imports against exactly these, and a
        // flattened representation would make "which calls come from outside"
        // a question somebody answers by reading names.
        let imported = Instr::ImportCall {
            result: ValueId(0),
            import: ImportId {
                interface: "pw:host/carts".into(),
                name: "add".into(),
            },
            args: vec![],
            ty: Type::Unit,
        };
        let ordinary = Instr::Call {
            result: ValueId(1),
            callee: def(7),
            args: vec![],
            ty: Type::Unit,
        };
        assert!(matches!(imported, Instr::ImportCall { .. }));
        assert!(matches!(ordinary, Instr::Call { .. }));
        let Instr::ImportCall { import, .. } = &imported else {
            unreachable!()
        };
        assert_eq!(import.qualified(), "pw:host/carts#add");
    }

    /// **A call carries no authority.** The capability is a property of the
    /// IMPORT, and this is the type-level statement of the 2026-08-20 ruling:
    /// a capability authorizes an operation and does not identify one.
    #[test]
    fn authority_lives_on_the_import_and_not_on_the_call() {
        let src = std::fs::read_to_string(
            std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("src/backend/ir.rs"),
        )
        .expect("ir.rs");
        let decl = src
            .split("pub enum Instr {")
            .nth(1)
            .expect("the Instr enum")
            .split("\n}")
            .next()
            .expect("its body");
        assert!(
            !decl.contains("capability: CapabilityId"),
            "no instruction may carry a capability: an import does"
        );
        assert!(decl.contains("import: ImportId"));
    }
}
