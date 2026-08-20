//! **Backend IR → core Wasm.**
//!
//! E10-A's second stage. Architect ruling, 2026-08-19, opening it:
//!
//! > The pre-codegen contract is finally strong enough that the encoder can be
//! > treated as a translation problem rather than another semantic analysis.
//! > […] **No semantic rediscovery in the encoder.** `DefId`,
//! > `Type::Nominal(DefId)`, `CapabilityId`, existing ABI/type facts in; bytes
//! > out. No name lookup, effect lookup, placement decisions, privacy
//! > inference, or "helpful" fallback.
//!
//! # What this file is NOT allowed to decide
//!
//! ```text
//! which calls need authority     the IR already says: HostCall vs Call
//! which interface serves one     Program::imports, from the CONTRACT
//! what a name resolves to        every callee is a DefId
//! what a type is                 every type is a Type, nominals by DefId
//! where anything runs            not this layer's question at all
//! ```
//!
//! The one thing it decides is **bytes**: which core instruction represents
//! which IR instruction, and how a value is held. Everything a decision depends
//! on arrives already answered, and `the_encoder_never_decides_from_a_name` is
//! the structural guard — the same one `lower.rs` carries, one layer down.
//!
//! # Core Wasm only
//!
//! > Keep Canonical ABI/component wrapping separate from core Wasm lowering
//! > […] so component-format plumbing doesn't infect expression lowering.
//!
//! So this file emits a **core module**: types, imports, functions, code,
//! exports. It does not know what a component is, does not lift or lower across
//! the Canonical ABI, and does not name a world. Those are the next stage's,
//! and keeping them out is why this one is small enough to read.
//!
//! # How a value is held: the invocation region
//!
//! Every value that is not a core scalar is an `i32` **handle** — an offset
//! into a region the host allocates for one invocation and reclaims when it
//! ends.
//!
//! > Invocation-region memory stays explicitly provisional. Establish the
//! > allocator/lifetime interface now, but don't let the fact that E10-A can
//! > reclaim everything at invocation end become the language's permanent
//! > escape/lifetime model.
//!
//! [`Repr`] is that interface, and it is deliberately one function: the day a
//! value must outlive its invocation, this is the single place that has to
//! learn about it. Nothing else in the encoder asks how a value is stored.

use std::collections::BTreeMap;

use wasm_encoder::{
    CodeSection, ExportKind, ExportSection, Function, FunctionSection, ImportSection, Instruction,
    Module, TypeSection, ValType,
};

use super::ir::{CapabilityId, Const, Instr, Program, Terminator, Type, ValueId};

/// **What an encoding produced, and never silently nothing.**
///
/// The same three-valued discipline `Lowering` carries, for the same reason and
/// with more at stake. Architect ruling, 2026-08-19:
///
/// > **Unsupported IR is a hard backend result.** Never emit `nop`, zero, empty
/// > block, dummy result, etc. for an instruction you haven't implemented.
///
/// A construct encoded to `nop` produces a module that validates, links,
/// instantiates and does the wrong thing — and every check downstream of here
/// reads bytes, so nothing would notice.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Encoding<T> {
    Encoded(T),
    /// The encoder does not represent this construct yet, and says which.
    Unsupported {
        construct: &'static str,
        reason: String,
    },
    /// An upstream stage did not produce what this one needs. Not a violation
    /// of anything, and emphatically not something to fill in.
    Blocked {
        why: String,
    },
}

impl<T> Encoding<T> {
    pub fn is_encoded(&self) -> bool {
        matches!(self, Encoding::Encoded(_))
    }
}

impl<T> std::fmt::Display for Encoding<T> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Encoding::Encoded(_) => f.write_str("encoded"),
            Encoding::Unsupported { construct, reason } => {
                write!(f, "`{construct}` does not encode: {reason}")
            }
            Encoding::Blocked { why } => write!(f, "blocked upstream: {why}"),
        }
    }
}

/// **How a value of this type is held in core Wasm.**
///
/// The invocation-region interface, and the only place in the encoder that
/// knows a value has a representation at all.
///
/// `Handle` is an `i32` offset into the region the host allocates for one
/// invocation. It is provisional by construction: the region is reclaimed when
/// the invocation ends, so nothing here can express a value outliving one — and
/// that limitation is a property of this function rather than an assumption
/// spread through the encoder.
///
/// It is deliberately NOT a decision about the type. `Type` came from the
/// checker; this says how many bytes of stack a core function needs to pass it.
pub fn repr(t: &Type) -> ValType {
    match t {
        // The core scalars, which need no region at all.
        Type::Int => ValType::I64,
        Type::Float => ValType::F64,
        Type::Bool => ValType::I32,
        // Everything else is a handle into the invocation region: a string, a
        // record, a variant, a `Result`, an `Option`, a list, or an opaque
        // type's representation.
        Type::Str
        | Type::Unit
        | Type::Nominal(_)
        | Type::Result(_, _)
        | Type::Option(_)
        | Type::List(_) => ValType::I32,
    }
}

/// **Encode a whole program as one core module.**
///
/// Returns the bytes and, beside them, every function that did NOT encode with
/// the reason. A caller that wants only the successes still has to look at the
/// refusals to know what it is missing — the same shape `lower::program`
/// returns, and for the same reason.
pub fn module(p: &Program) -> (Vec<u8>, Vec<Encoding<()>>) {
    let mut m = Module::new();
    let mut refusals = Vec::new();

    // **The import table, in the order `Program::imports` gives it.**
    //
    // Index `i` of that list is core function index `i`, which is what makes
    // `HostCall` encodable at all: the capability names an entry the contract
    // put there. A missing capability is a refusal below, never index 0.
    let mut import_of: BTreeMap<String, u32> = BTreeMap::new();
    let mut types = TypeSection::new();
    let mut imports = ImportSection::new();
    let mut type_of_sig: BTreeMap<(Vec<ValType>, Vec<ValType>), u32> = BTreeMap::new();
    let mut next_type = 0u32;

    // A host import's signature is not knowable from the capability alone —
    // that is the Canonical ABI's answer and it lives in the next stage. Here
    // every host function takes the handles its call site passes and returns
    // one, which is what the invocation-region model makes true.
    let mut ambiguous: BTreeMap<String, Vec<usize>> = BTreeMap::new();
    // Counted rather than taken from `p.imports.len()`: a refused import is not
    // emitted, and a function index computed from the declared count would name
    // whatever sits at that slot. The validator caught exactly that — "exported
    // function index out of bounds".
    let mut emitted = 0usize;
    for (i, imp) in p.imports.iter().enumerate() {
        let arities = host_arities(p, &imp.capability);
        if arities.len() > 1 {
            // **A capability is not a function, and this is where that shows.**
            //
            // `database.write<Carts>` is reached through `Carts.add(s, item,
            // qty)` and `Carts.clear(s)` — two Pleris functions, one authority,
            // two argument lists. A core import has ONE signature, so there is
            // no honest number to put here.
            //
            // Refused rather than encoded against the first arity found, which
            // is what produced a module the validator rejected with "expected
            // i32 but nothing on stack". See `docs/RISK_QUEUE.md`.
            ambiguous.insert(imp.capability.name(), arities);
            continue;
        }
        let arity = arities.first().copied().unwrap_or(0);
        let params: Vec<ValType> = vec![ValType::I32; arity];
        let results = vec![ValType::I32];
        let key = (params.clone(), results.clone());
        let ty = *type_of_sig.entry(key).or_insert_with(|| {
            types.ty().function(params.clone(), results.clone());
            let t = next_type;
            next_type += 1;
            t
        });
        imports.import(
            &imp.interface,
            &imp.name,
            wasm_encoder::EntityType::Function(ty),
        );
        import_of.insert(imp.capability.name(), emitted as u32);
        emitted += 1;
        let _ = i;
    }
    for (name, arities) in &ambiguous {
        refusals.push(Encoding::Blocked {
            why: format!(
                "`{name}` is called with {arities:?} arguments at different sites. A capability \
                 names AUTHORITY, not a function: `Instr::HostCall` carries the capability the \
                 enclosing contract requires and the arguments of the Pleris function that \
                 needed it, and two functions can need one capability. A core import has one \
                 signature"
            ),
        });
    }

    // Then the program's own functions, each with its own signature.
    let mut funcs = FunctionSection::new();
    let mut code = CodeSection::new();
    let mut exports = ExportSection::new();
    let mut next_func = emitted as u32;

    for f in &p.functions {
        let params: Vec<ValType> = f.params.iter().map(|(_, t)| repr(t)).collect();
        let results = vec![repr(&f.ret)];
        match body(f, &import_of) {
            Encoding::Encoded(fun) => {
                let key = (params.clone(), results.clone());
                let ty = *type_of_sig.entry(key).or_insert_with(|| {
                    types.ty().function(params.clone(), results.clone());
                    let t = next_type;
                    next_type += 1;
                    t
                });
                funcs.function(ty);
                code.function(&fun);
                exports.export(&f.export, ExportKind::Func, next_func);
                next_func += 1;
            }
            other => refusals.push(match other {
                Encoding::Unsupported { construct, reason } => {
                    Encoding::Unsupported { construct, reason }
                }
                Encoding::Blocked { why } => Encoding::Blocked { why },
                Encoding::Encoded(_) => unreachable!("matched above"),
            }),
        }
    }

    m.section(&types);
    m.section(&imports);
    m.section(&funcs);
    m.section(&exports);
    m.section(&code);
    (m.finish(), refusals)
}

/// **Every distinct argument count this capability is called with.**
///
/// More than one means the program cannot be encoded, and the encoder says so
/// rather than picking one. Read off the IR rather than decided — a signature
/// invented here would be a second answer to a question the front end is
/// supposed to have settled, and the point of this function is that the front
/// end has not settled it.
fn host_arities(p: &Program, cap: &CapabilityId) -> Vec<usize> {
    let mut out: Vec<usize> = p
        .functions
        .iter()
        .flat_map(|f| f.blocks.iter())
        .flat_map(|b| b.instrs.iter())
        .filter_map(|i| match i {
            Instr::HostCall {
                capability, args, ..
            } if capability == cap => Some(args.len()),
            _ => None,
        })
        .collect();
    out.sort_unstable();
    out.dedup();
    out
}

/// One function's body.
fn body(f: &super::ir::Function, import_of: &BTreeMap<String, u32>) -> Encoding<Function> {
    let Some(entry) = f.entry() else {
        return Encoding::Blocked {
            why: format!("`{}` has no entry block", f.export),
        };
    };
    if f.blocks.len() > 1 {
        return Encoding::Unsupported {
            construct: "a function with more than one block",
            reason: format!(
                "`{}` has {} blocks; E10-A encodes straight-line bodies, which \
                 is what `add_to_cart` needs",
                f.export,
                f.blocks.len()
            ),
        };
    }

    // **One local per IR value, so a `ValueId` is a slot.**
    //
    // Parameters are locals 0..n by the core calling convention; every
    // instruction result gets the next slot. `ValueId` is dense and assigned in
    // order by `lower.rs`, but this does not assume that — the map is built
    // from what is actually there.
    let mut slot: BTreeMap<ValueId, u32> = BTreeMap::new();
    for (i, (v, _)) in f.params.iter().enumerate() {
        slot.insert(*v, i as u32);
    }
    let mut extra: Vec<ValType> = Vec::new();
    for i in &entry.instrs {
        let v = i.result();
        if slot.contains_key(&v) {
            continue;
        }
        slot.insert(v, (f.params.len() + extra.len()) as u32);
        extra.push(repr(instr_type(i)));
    }

    // `Function::new` takes run-length-encoded local groups.
    let mut groups: Vec<(u32, ValType)> = Vec::new();
    for t in extra {
        match groups.last_mut() {
            Some((n, seen)) if *seen == t => *n += 1,
            _ => groups.push((1, t)),
        }
    }
    let mut fun = Function::new(groups);

    for i in &entry.instrs {
        match i {
            Instr::HostCall {
                result,
                capability,
                args,
                ..
            } => {
                let Some(&idx) = import_of.get(&capability.name()) else {
                    // The capability is in the function's list and nothing put
                    // an import there. Refused rather than encoded against
                    // index 0, which would call whatever happened to be first.
                    return Encoding::Blocked {
                        why: format!(
                            "`{}` calls `{}` and the program declares no import for it",
                            f.export,
                            capability.name()
                        ),
                    };
                };
                for a in args {
                    match slot.get(a) {
                        Some(&s) => {
                            fun.instructions().local_get(s);
                        }
                        None => {
                            return Encoding::Blocked {
                                why: format!(
                                    "`{}` passes {a:?} to `{}` and nothing defines it",
                                    f.export,
                                    capability.name()
                                ),
                            };
                        }
                    }
                }
                fun.instructions().call(idx);
                fun.instructions().local_set(slot[result]);
            }
            Instr::Const { result, value, .. } => match value {
                Const::Int(n) => {
                    fun.instructions().i64_const(*n);
                    fun.instructions().local_set(slot[result]);
                }
                Const::Bool(b) => {
                    fun.instructions().i32_const(i32::from(*b));
                    fun.instructions().local_set(slot[result]);
                }
                other => {
                    // A float or a string needs the region, and writing one
                    // into it is the Canonical ABI's job — the next stage.
                    // Refused by name rather than encoded as zero.
                    return Encoding::Unsupported {
                        construct: "a constant that does not fit a core scalar",
                        reason: format!(
                            "`{}` holds {other:?}, which needs the invocation region and the \
                             ABI layer that writes into it",
                            f.export
                        ),
                    };
                }
            },
            Instr::Call { callee, .. } => {
                return Encoding::Unsupported {
                    construct: "a call to another Pleris declaration",
                    reason: format!(
                        "`{}` calls {callee:?}; E10-A encodes the host-call path, which is what \
                         `add_to_cart` needs",
                        f.export
                    ),
                };
            }
            Instr::Construct { .. } => {
                return Encoding::Unsupported {
                    construct: "building a record or variant",
                    reason: format!(
                        "`{}` constructs a value, which needs the invocation region",
                        f.export
                    ),
                };
            }
            Instr::Project { .. } => {
                return Encoding::Unsupported {
                    construct: "reading a field",
                    reason: format!(
                        "`{}` projects a field, which needs the region's layout",
                        f.export
                    ),
                };
            }
        }
    }

    match &entry.terminator {
        Terminator::Return(v) => match slot.get(v) {
            Some(&s) => {
                fun.instructions().local_get(s);
                fun.instructions().end();
            }
            None => {
                return Encoding::Blocked {
                    why: format!("`{}` returns {v:?} and nothing defines it", f.export),
                };
            }
        },
        other => {
            return Encoding::Unsupported {
                construct: "a terminator other than `return`",
                reason: format!("`{}` ends in {other:?}", f.export),
            };
        }
    }

    Encoding::Encoded(fun)
}

/// The type an instruction's result has, as the IR recorded it.
fn instr_type(i: &Instr) -> &Type {
    match i {
        Instr::Const { ty, .. }
        | Instr::Call { ty, .. }
        | Instr::HostCall { ty, .. }
        | Instr::Construct { ty, .. }
        | Instr::Project { ty, .. } => ty,
    }
}

/// Unused today, kept because the import section needs it once a host function
/// takes something other than handles.
#[allow(dead_code)]
fn _unused(_: Instruction<'_>) {}
