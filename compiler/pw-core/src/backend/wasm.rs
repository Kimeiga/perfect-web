//! **Backend IR → a core module that speaks the Canonical ABI.**
//!
//! E10-I step 4. Architect ruling, 2026-08-20:
//!
//! > Pleris is the ABI authority. […] Do **not** hand-implement the Component
//! > Model binary format. Pleris owns the lowering facts; upstream owns the
//! > component-format encoding.
//!
//! So this file emits a **core module** whose imports and exports are exactly
//! what `wit-component` needs to wrap it as the component a world describes,
//! and it decides nothing about the Canonical ABI itself:
//!
//! ```text
//! which core signature a function has   Resolve::wasm_signature
//! how a value flattens                  Resolve::push_flat
//! a type's size, alignment, offsets     wit_parser::SizeAlign
//! a core import's module and name       Resolve::wasm_import_name (Legacy, Sync)
//! a core export's name                  Resolve::wasm_export_name (Legacy, Sync)
//! ```
//!
//! What it decides is **bytes**: which core instructions connect the values
//! the IR names. And, as before, nothing from a spelling:
//!
//! ```text
//! which calls leave the module   the IR already says: ImportCall vs Call
//! which operation one names      the ImportCall's own ImportId
//! what an operation's ABI is     the world, which the contract generated
//! what the export is             the world's declaration, by DefId
//! ```
//!
//! # How a value is held
//!
//! Every IR value is held one of two ways, and the Canonical ABI decides which:
//!
//! ```text
//! Flat     core values in locals, as the value flattens    a parameter, a scalar
//! Memory   the value's canonical layout at an address      a result returned by pointer
//! ```
//!
//! A value moves between the two only through [`load`], which reads a layout
//! into its flat values using `SizeAlign`'s offsets. Passing a result to a
//! call, or returning it, needs no copy when the two positions have one
//! component type — the case `add_to_cart` is: `carts#add`'s result IS the
//! command's result.
//!
//! # The invocation region
//!
//! Linear memory is one bump region: `cabi_realloc` hands out addresses from a
//! mutable global, growing memory as needed and trapping when it cannot, and
//! the export's post-return function resets the global. Everything a call
//! allocates — its arguments, its callees' results, its own result — is
//! reclaimed once the caller has lifted the result.
//!
//! > Invocation-region memory stays explicitly provisional. […] don't let the
//! > fact that E10-A can reclaim everything at invocation end become the
//! > language's permanent escape/lifetime model.
//!
//! Nothing here can express a value outliving its invocation, and that is a
//! property of this file, stated once, rather than an assumption spread through
//! the encoder.

use std::collections::BTreeMap;

use wasm_encoder::{
    CodeSection, ConstExpr, DataSection, EntityType, ExportKind, ExportSection, Function,
    FunctionSection, GlobalSection, GlobalType, ImportSection, MemArg, MemorySection, MemoryType,
    Module, TypeSection, ValType,
};
use wit_parser::abi::{AbiVariant, FlatTypes, WasmSignature, WasmType};
use wit_parser::{
    Field, Function as WitFunction, LiftLowerAbi, ManglingAndAbi, Record, Resolve, Result_,
    SizeAlign, Type as WitType, TypeDef as WitTypeDef, TypeDefKind, TypeOwner, WasmExport,
    WasmExportKind, WasmImport, WorldId, WorldItem, WorldKey,
};

use super::ir::{
    BinaryOp, BuiltinCase, CallableImport, Const, Instr, Shape, Terminator, Type, TypeDef, UnaryOp,
    ValueId, all_instrs,
};
use crate::resolve::DefId;

/// **What an encoding produced, and never silently nothing.**
///
/// Architect ruling, 2026-08-19:
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

    pub fn encoded(self) -> Option<T> {
        match self {
            Encoding::Encoded(t) => Some(t),
            _ => None,
        }
    }

    fn map<U>(self, f: impl FnOnce(T) -> U) -> Encoding<U> {
        match self {
            Encoding::Encoded(t) => Encoding::Encoded(f(t)),
            Encoding::Unsupported { construct, reason } => {
                Encoding::Unsupported { construct, reason }
            }
            Encoding::Blocked { why } => Encoding::Blocked { why },
        }
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

macro_rules! refuse {
    ($construct:expr, $($reason:tt)*) => {
        return Encoding::Unsupported { construct: $construct, reason: format!($($reason)*) }
    };
}

macro_rules! blocked {
    ($($why:tt)*) => {
        return Encoding::Blocked { why: format!($($why)*) }
    };
}

/// The legacy (`wit-component`) name mangling with the synchronous ABI: the
/// names `wit-component` and `wasm-tools component new` recognise.
const MANGLING: ManglingAndAbi = ManglingAndAbi::Legacy(LiftLowerAbi::Sync);

/// Where the bump region starts. Address 0 is never handed out, so a null
/// pointer cannot alias an allocation.
const HEAP_BASE: i32 = 16;

/// **The core module for one exported function of one world.**
///
/// `export` is the WIT function the world exports for `function`, as
/// `wit::World::functions` names it. `imports` are the program's callables, of
/// which the module imports exactly those the function calls.
pub fn core_module(
    resolve: &Resolve,
    world: WorldId,
    function: &super::ir::Function,
    export: &str,
    imports: &[CallableImport],
) -> Encoding<Vec<u8>> {
    core_module_with(
        resolve,
        world,
        function,
        export,
        imports,
        &[],
        &BTreeMap::new(),
    )
}

/// [`core_module`], with the program's declared types (ADR-0039 §5): their
/// shapes, and the WIT identifier each declaration's type has in the world's
/// `pw:types` interface, as `wit::type_idents` records them. A body may make a
/// value whose type the world never mentions; the encoder gives it a
/// definition in a private copy of the `Resolve`, and the component is
/// wrapped with the world's own.
pub fn core_module_with(
    world_resolve: &Resolve,
    world: WorldId,
    function: &super::ir::Function,
    export: &str,
    imports: &[CallableImport],
    declared: &[TypeDef],
    idents: &BTreeMap<DefId, String>,
) -> Encoding<Vec<u8>> {
    let mut private = world_resolve.clone();
    let wit = internal_types(&mut private, function, declared, idents);
    let resolve = &private;
    let mut sizes = SizeAlign::default();
    sizes.fill(resolve);
    let literals = Literals::of(function);

    // --- the export, by the world's own description --------------------------
    let Some((export_key, export_fn)) = world_function(resolve, world, true, export) else {
        blocked!(
            "the world `{}` exports no function `{export}`",
            resolve.worlds[world].name
        );
    };
    if export_fn.params.len() != function.params.len() {
        blocked!(
            "`{}` has {} parameters and the world's `{export}` has {}",
            function.export,
            function.params.len(),
            export_fn.params.len()
        );
    }

    // --- the imports this function actually calls, in first-use order --------
    let Some(entry) = function.entry() else {
        blocked!("`{}` has no entry block", function.export);
    };
    if function.blocks.len() > 1 {
        refuse!(
            "a function with more than one block",
            "`{}` has {} blocks; the component backend encodes straight-line bodies",
            function.export,
            function.blocks.len()
        );
    }
    let mut used: Vec<(super::ir::ImportId, WorldKey, WitFunction)> = Vec::new();
    // Inside a match arm or an `if` too: until 2026-09-25 only the top of the
    // body was read, so an import called only in an arm had no core import.
    for i in all_instrs(&entry.instrs) {
        let Instr::ImportCall { import, .. } = i else {
            continue;
        };
        if used.iter().any(|(id, _, _)| id == import) {
            continue;
        }
        if !imports.iter().any(|c| &c.id == import) {
            blocked!(
                "`{}` calls `{}` and the program declares no such callable",
                function.export,
                import.qualified()
            );
        }
        let Some((key, func)) = world_import(resolve, world, &import.interface, &import.name)
        else {
            blocked!(
                "`{}` calls `{}` and the world `{}` does not import it",
                function.export,
                import.qualified(),
                resolve.worlds[world].name
            );
        };
        used.push((import.clone(), key, func));
    }

    // --- the module's shape -----------------------------------------------------
    let mut module = Module::new();
    let mut types = TypeSection::new();
    let mut type_index: BTreeMap<(Vec<ValType>, Vec<ValType>), u32> = BTreeMap::new();
    let mut ty = |params: Vec<ValType>, results: Vec<ValType>| -> u32 {
        let next = type_index.len() as u32;
        *type_index
            .entry((params.clone(), results.clone()))
            .or_insert_with(|| {
                types.ty().function(params, results);
                next
            })
    };

    let mut import_section = ImportSection::new();
    let mut import_index: BTreeMap<String, (u32, WasmSignature, WitFunction)> = BTreeMap::new();
    for (n, (id, key, func)) in used.iter().enumerate() {
        let sig = resolve.wasm_signature(AbiVariant::GuestImport, func);
        if sig.indirect_params {
            refuse!(
                "a call whose arguments exceed the flat limit",
                "`{}` takes more than {} flat values, which the Canonical ABI passes \
                 through memory",
                id.qualified(),
                Resolve::MAX_FLAT_PARAMS
            );
        }
        let (module_name, field) = resolve.wasm_import_name(
            MANGLING,
            WasmImport::Func {
                interface: Some(key),
                func,
            },
        );
        let t = ty(core_types(&sig.params), core_types(&sig.results));
        import_section.import(&module_name, &field, EntityType::Function(t));
        import_index.insert(id.qualified(), (n as u32, sig, func.clone()));
    }
    let imported = used.len() as u32;
    let realloc_index = imported;
    let export_index = imported + 1;
    let post_index = imported + 2;

    let export_sig = resolve.wasm_signature(AbiVariant::GuestExport, &export_fn);
    if export_sig.indirect_params {
        refuse!(
            "an export whose parameters exceed the flat limit",
            "`{export}` takes more than {} flat values",
            Resolve::MAX_FLAT_PARAMS
        );
    }

    let realloc_ty = ty(vec![ValType::I32; 4], vec![ValType::I32]);
    let export_ty = ty(
        core_types(&export_sig.params),
        core_types(&export_sig.results),
    );
    let post_ty = ty(core_types(&export_sig.results), vec![]);

    // --- the export's body --------------------------------------------------------
    let mut helpers = Helpers {
        first: post_index + 1,
        used: Vec::new(),
    };
    let body = match export_body(
        Shared {
            resolve,
            sizes: &sizes,
            wit: &wit,
            literals: &literals,
            imports: &import_index,
            realloc_index,
        },
        function,
        &export_fn,
        &export_sig,
        &mut helpers,
    ) {
        Encoding::Encoded(b) => b,
        other => return other.map(|_| unreachable!()),
    };

    let mut funcs = FunctionSection::new();
    funcs.function(realloc_ty);
    funcs.function(export_ty);
    funcs.function(post_ty);
    for h in &helpers.used {
        let (params, results) = h.signature();
        funcs.function(ty(params, results));
    }

    // Enough pages for the literals below the region.
    let heap_base = literals.heap_base();
    let mut memories = MemorySection::new();
    memories.memory(MemoryType {
        minimum: (heap_base as u64).div_ceil(65536).max(1),
        maximum: None,
        memory64: false,
        shared: false,
        page_size_log2: None,
    });

    let mut globals = GlobalSection::new();
    globals.global(
        GlobalType {
            val_type: ValType::I32,
            mutable: true,
            shared: false,
        },
        &ConstExpr::i32_const(heap_base),
    );

    let mut exports = ExportSection::new();
    exports.export(
        &resolve.wasm_export_name(MANGLING, WasmExport::Memory),
        ExportKind::Memory,
        0,
    );
    exports.export(
        &resolve.wasm_export_name(MANGLING, WasmExport::Realloc),
        ExportKind::Func,
        realloc_index,
    );
    exports.export(
        &resolve.wasm_export_name(
            MANGLING,
            WasmExport::Func {
                interface: Some(&export_key),
                func: &export_fn,
                kind: WasmExportKind::Normal,
            },
        ),
        ExportKind::Func,
        export_index,
    );
    exports.export(
        &resolve.wasm_export_name(
            MANGLING,
            WasmExport::Func {
                interface: Some(&export_key),
                func: &export_fn,
                kind: WasmExportKind::PostReturn,
            },
        ),
        ExportKind::Func,
        post_index,
    );

    let mut code = CodeSection::new();
    code.function(&realloc());
    code.function(&body);
    code.function(&post_return(heap_base));
    for h in &helpers.used {
        code.function(&h.body(realloc_index));
    }

    module.section(&types);
    module.section(&import_section);
    module.section(&funcs);
    module.section(&memories);
    module.section(&globals);
    module.section(&exports);
    module.section(&code);
    if !literals.bytes.is_empty() {
        let mut data = DataSection::new();
        data.active(
            0,
            &ConstExpr::i32_const(DATA_BASE),
            literals.bytes.iter().copied(),
        );
        module.section(&data);
    }
    Encoding::Encoded(module.finish())
}

/// Where the string literals start. The region starts after them.
const DATA_BASE: i32 = HEAP_BASE;

/// **Every string constant the body names, at its address** (ADR-0039 §3):
/// one data segment below the invocation region. A body with none has none,
/// and its region starts where it always did.
struct Literals {
    at: BTreeMap<String, u32>,
    bytes: Vec<u8>,
}

impl Literals {
    fn of(function: &super::ir::Function) -> Literals {
        let mut out = Literals {
            at: BTreeMap::new(),
            bytes: Vec::new(),
        };
        let mut add = |s: &str| {
            if !out.at.contains_key(s) {
                out.at
                    .insert(s.to_string(), DATA_BASE as u32 + out.bytes.len() as u32);
                out.bytes.extend_from_slice(s.as_bytes());
            }
        };
        for b in &function.blocks {
            for i in all_instrs(&b.instrs) {
                match i {
                    Instr::Const {
                        value: Const::Str(s),
                        ..
                    } => add(s),
                    // A `Bool` written as text is one of these two.
                    Instr::Format { .. } => {
                        add("true");
                        add("false");
                    }
                    _ => {}
                }
            }
        }
        out
    }

    /// The region's first address: past the literals, 8-aligned.
    fn heap_base(&self) -> i32 {
        let end = DATA_BASE as u32 + self.bytes.len() as u32;
        (end.div_ceil(8) * 8) as i32
    }
}

/// **The WIT type of each IR type the function uses** (ADR-0039 §5).
///
/// A primitive is itself. A declaration that crosses the boundary is the
/// world's own type, found by the identifier `wit::type_idents` records for
/// it; one that does not is defined here, in the private `Resolve`, from its
/// shape. `option`, `result` and `list` are anonymous: `same_type` compares
/// them by their arguments, so one defined here equals the world's.
/// A type with no WIT form here (a declared variant the world does not name)
/// is left out, and the encoder refuses whatever needs it.
fn internal_types(
    resolve: &mut Resolve,
    function: &super::ir::Function,
    declared: &[TypeDef],
    idents: &BTreeMap<DefId, String>,
) -> BTreeMap<Type, WitType> {
    let world_types = resolve.packages.iter().find_map(|(_, p)| {
        (p.name.namespace == "pw" && p.name.name == "types")
            .then(|| p.interfaces.get("types").copied())
            .flatten()
    });
    let mut wanted: Vec<Type> = function.params.iter().map(|(_, t)| t.clone()).collect();
    wanted.push(function.ret.clone());
    for b in &function.blocks {
        for i in all_instrs(&b.instrs) {
            wanted.push(i.ty().clone());
        }
    }
    let mut out = BTreeMap::new();
    let mut cx = TypeCx {
        world_types,
        declared,
        idents,
        out: &mut out,
        visiting: Vec::new(),
    };
    for t in wanted {
        cx.wit(resolve, &t);
    }
    out
}

struct TypeCx<'a> {
    world_types: Option<wit_parser::InterfaceId>,
    declared: &'a [TypeDef],
    idents: &'a BTreeMap<DefId, String>,
    out: &'a mut BTreeMap<Type, WitType>,
    /// Declarations being defined: a record that contains itself has no
    /// canonical layout.
    visiting: Vec<DefId>,
}

impl TypeCx<'_> {
    fn wit(&mut self, resolve: &mut Resolve, t: &Type) -> Option<WitType> {
        if let Some(w) = self.out.get(t) {
            return Some(*w);
        }
        let anonymous = |resolve: &mut Resolve, kind: TypeDefKind| {
            WitType::Id(resolve.types.alloc(WitTypeDef {
                name: None,
                kind,
                owner: TypeOwner::None,
                docs: Default::default(),
                stability: Default::default(),
                span: Default::default(),
                external_id: None,
            }))
        };
        let w = match t {
            Type::Int => WitType::S64,
            Type::Float => WitType::F64,
            Type::Bool => WitType::Bool,
            Type::Str => WitType::String,
            Type::Unit => return None,
            Type::Option(inner) => {
                let inner = self.wit(resolve, inner)?;
                anonymous(resolve, TypeDefKind::Option(inner))
            }
            Type::List(inner) => {
                let inner = self.wit(resolve, inner)?;
                anonymous(resolve, TypeDefKind::List(inner))
            }
            Type::Result(ok, err) => {
                let ok = match **ok {
                    Type::Unit => None,
                    ref t => Some(self.wit(resolve, t)?),
                };
                let err = match **err {
                    Type::Unit => None,
                    ref t => Some(self.wit(resolve, t)?),
                };
                anonymous(resolve, TypeDefKind::Result(Result_ { ok, err }))
            }
            Type::Nominal(def) => {
                // The world's own, when the declaration crosses the boundary.
                if let (Some(iface), Some(ident)) = (self.world_types, self.idents.get(def))
                    && let Some(id) = resolve.interfaces[iface].types.get(ident)
                {
                    WitType::Id(*id)
                } else {
                    let d = self.declared.iter().find(|d| d.def == *def)?;
                    if self.visiting.contains(def) {
                        return None;
                    }
                    self.visiting.push(*def);
                    let defined = match &d.shape {
                        Shape::Alias(of) => self.wit(resolve, of),
                        Shape::Record { fields } => {
                            let mut wit_fields = Vec::new();
                            let mut whole = true;
                            for (name, ft) in fields {
                                match self.wit(resolve, ft) {
                                    Some(ty) => wit_fields.push(Field {
                                        name: crate::wit::ident(name),
                                        ty,
                                        docs: Default::default(),
                                        span: Default::default(),
                                    }),
                                    None => whole = false,
                                }
                            }
                            whole.then(|| {
                                WitType::Id(
                                    resolve.types.alloc(WitTypeDef {
                                        name: Some(
                                            self.idents
                                                .get(def)
                                                .cloned()
                                                .unwrap_or_else(|| crate::wit::ident(&d.name)),
                                        ),
                                        kind: TypeDefKind::Record(Record { fields: wit_fields }),
                                        owner: TypeOwner::None,
                                        docs: Default::default(),
                                        stability: Default::default(),
                                        span: Default::default(),
                                        external_id: None,
                                    }),
                                )
                            })
                        }
                        // A declared variant is built and matched by nothing
                        // here (ADR-0039 §6).
                        Shape::Variant { .. } => None,
                    };
                    self.visiting.pop();
                    defined?
                }
            }
        };
        self.out.insert(t.clone(), w);
        Some(w)
    }
}

/// The function a world imports or exports, by the interface's world key and
/// the function's WIT name. The key is the one `wit::package` generated from
/// the same `ImportId` the IR carries, so this is a lookup of an identity the
/// world was built from, not a guess from a spelling.
fn world_import(
    resolve: &Resolve,
    world: WorldId,
    interface: &str,
    name: &str,
) -> Option<(WorldKey, WitFunction)> {
    resolve.worlds[world]
        .imports
        .iter()
        .find_map(|(key, item)| match item {
            WorldItem::Interface { id, .. } if resolve.name_world_key(key) == interface => resolve
                .interfaces[*id]
                .functions
                .get(name)
                .map(|f| (key.clone(), f.clone())),
            _ => None,
        })
}

/// The exported function named `name`, from whichever exported interface
/// declares it. Exactly one must.
fn world_function(
    resolve: &Resolve,
    world: WorldId,
    exported: bool,
    name: &str,
) -> Option<(WorldKey, WitFunction)> {
    let items = match exported {
        true => &resolve.worlds[world].exports,
        false => &resolve.worlds[world].imports,
    };
    let found: Vec<(WorldKey, WitFunction)> = items
        .iter()
        .filter_map(|(key, item)| match item {
            WorldItem::Interface { id, .. } => resolve.interfaces[*id]
                .functions
                .get(name)
                .map(|f| (key.clone(), f.clone())),
            WorldItem::Function(f) if f.name == name => Some((key.clone(), f.clone())),
            _ => None,
        })
        .collect();
    match found.as_slice() {
        [one] => Some(one.clone()),
        _ => None,
    }
}

fn core_types(ts: &[WasmType]) -> Vec<ValType> {
    ts.iter().map(|t| core_type(*t)).collect()
}

/// A flat Canonical ABI type as a core value type, for 32-bit memories.
fn core_type(t: WasmType) -> ValType {
    match t {
        WasmType::I32 | WasmType::Pointer | WasmType::Length => ValType::I32,
        WasmType::I64 | WasmType::PointerOrI64 => ValType::I64,
        WasmType::F32 => ValType::F32,
        WasmType::F64 => ValType::F64,
    }
}

/// The flat types of one component type, from `wit-parser`.
fn flat(resolve: &Resolve, t: &WitType) -> Option<Vec<WasmType>> {
    let mut storage = [WasmType::I32; Resolve::MAX_FLAT_PARAMS];
    let mut out = FlatTypes::new(&mut storage);
    resolve.push_flat(t, &mut out).then(|| out.to_vec())
}

/// **Do these two positions hold one component type?**
///
/// Aliases are followed to what they name. A NAMED type — a record, a variant,
/// an enum — is its declaration and is equal only to itself: `domain-store`
/// and `domain-cart` flatten identically and are different types, which is
/// the whole reason the component-level audit exists. An ANONYMOUS
/// constructor — `result<..>`, `list<..>`, `option<..>`, a tuple — has no
/// identity of its own, so each interface that writes one gets its own
/// `TypeId`, and it is compared by its arguments.
fn same_type(resolve: &Resolve, a: &WitType, b: &WitType) -> bool {
    let (a, b) = (dealias(resolve, *a), dealias(resolve, *b));
    match (a, b) {
        (WitType::Id(x), WitType::Id(y)) => {
            if x == y {
                return true;
            }
            let (kx, ky) = (&resolve.types[x], &resolve.types[y]);
            if kx.name.is_some() || ky.name.is_some() {
                return false;
            }
            let opt = |p: &Option<WitType>, q: &Option<WitType>| match (p, q) {
                (Some(p), Some(q)) => same_type(resolve, p, q),
                (None, None) => true,
                _ => false,
            };
            match (&kx.kind, &ky.kind) {
                (TypeDefKind::Result(p), TypeDefKind::Result(q)) => {
                    opt(&p.ok, &q.ok) && opt(&p.err, &q.err)
                }
                (TypeDefKind::List(p), TypeDefKind::List(q))
                | (TypeDefKind::Option(p), TypeDefKind::Option(q)) => same_type(resolve, p, q),
                (TypeDefKind::Tuple(p), TypeDefKind::Tuple(q)) => {
                    p.types.len() == q.types.len()
                        && p.types
                            .iter()
                            .zip(&q.types)
                            .all(|(p, q)| same_type(resolve, p, q))
                }
                _ => false,
            }
        }
        (p, q) => p == q,
    }
}

/// Follow `type a = b` to `b`.
fn dealias(resolve: &Resolve, t: WitType) -> WitType {
    let mut t = t;
    while let WitType::Id(id) = t {
        match &resolve.types[id].kind {
            TypeDefKind::Type(inner) => t = *inner,
            _ => break,
        }
    }
    t
}

/// How one IR value is held while the export runs.
#[derive(Debug, Clone)]
enum Held {
    Flat {
        ty: WitType,
        locals: Vec<u32>,
    },
    Memory {
        ty: WitType,
        ptr: u32,
    },
    /// A call with no result.
    Nothing,
}

impl Held {
    fn ty(&self) -> Option<&WitType> {
        match self {
            Held::Flat { ty, .. } | Held::Memory { ty, .. } => Some(ty),
            Held::Nothing => None,
        }
    }
}

/// Locals beyond the parameters, allocated as they are needed.
struct Locals {
    first: u32,
    types: Vec<ValType>,
}

impl Locals {
    fn fresh(&mut self, t: ValType) -> u32 {
        self.types.push(t);
        self.first + self.types.len() as u32 - 1
    }
}

/// What every part of one module's encoding reads.
#[derive(Clone, Copy)]
struct Shared<'a> {
    resolve: &'a Resolve,
    sizes: &'a SizeAlign,
    /// Each IR type's WIT type (ADR-0039 §5).
    wit: &'a BTreeMap<Type, WitType>,
    literals: &'a Literals,
    imports: &'a BTreeMap<String, (u32, WasmSignature, WitFunction)>,
    realloc_index: u32,
}

/// The instructions of the export's body. Encoded against a scratch list so
/// the locals it needs are known before the `Function` is created.
fn export_body(
    shared: Shared<'_>,
    function: &super::ir::Function,
    export_fn: &WitFunction,
    export_sig: &WasmSignature,
    helpers: &mut Helpers,
) -> Encoding<Function> {
    use wasm_encoder::Instruction as I;
    let Shared {
        resolve,
        sizes,
        imports,
        realloc_index,
        ..
    } = shared;

    let mut enc = Enc {
        resolve,
        sizes,
        imports,
        realloc_index,
        wit: shared.wit,
        literals: shared.literals,
        helpers,
        export: &function.export,
        ops: Vec::new(),
        locals: Locals {
            first: export_sig.params.len() as u32,
            types: Vec::new(),
        },
        held: BTreeMap::new(),
        expected: expected_types(resolve, shared.wit, function, export_fn, imports),
    };

    // Parameters arrive flat, in the order the world lists them.
    let mut next_param = 0u32;
    for ((value, _), param) in function.params.iter().zip(&export_fn.params) {
        let Some(flats) = flat(resolve, &param.ty) else {
            refuse!(
                "a parameter that does not flatten",
                "`{}` does not flatten within the core parameter limit",
                param.name
            );
        };
        let ls: Vec<u32> = (next_param..next_param + flats.len() as u32).collect();
        next_param += flats.len() as u32;
        enc.held.insert(
            *value,
            Held::Flat {
                ty: param.ty,
                locals: ls,
            },
        );
    }

    let Some(entry) = function.entry() else {
        blocked!("`{}` has no entry block", function.export);
    };
    if let other @ (Encoding::Unsupported { .. } | Encoding::Blocked { .. }) =
        enc.region(&entry.instrs)
    {
        return other.map(|_| unreachable!());
    }

    // The result: returned flat, or by pointer when it exceeds the flat limit.
    let Terminator::Return(v) = &entry.terminator else {
        refuse!(
            "a terminator other than `return`",
            "`{}` ends in {:?}",
            function.export,
            entry.terminator
        );
    };
    let Some(h) = enc.held.get(v).cloned() else {
        blocked!("`{}` returns {v:?} and nothing defines it", function.export);
    };
    match (&export_fn.result, &h) {
        (None, _) => {}
        (Some(rt), h) => {
            let Some(t) = h.ty() else {
                blocked!("`{}` returns a call with no result", function.export);
            };
            if !same_type(resolve, t, rt) {
                blocked!(
                    "`{}` returns a value of another component type than the world declares",
                    function.export
                );
            }
            match (export_sig.retptr, h) {
                // The value already has its canonical layout at an address:
                // that address IS the result.
                (true, Held::Memory { ptr, .. }) => enc.ops.push(I::LocalGet(*ptr)),
                (true, Held::Nothing) => {
                    blocked!("`{}` returns a call with no result", function.export)
                }
                // A flat value returned by pointer is stored into the region
                // first, the way a constructed variant is.
                (true, Held::Flat { ty, locals: flats }) => {
                    let area = enc.locals.fresh(ValType::I32);
                    allocate(sizes, ty, realloc_index, area, &mut enc.ops);
                    match store(resolve, sizes, ty, area, 0, flats, &mut enc.ops) {
                        Encoding::Encoded(_) => {}
                        other => return other.map(|_| unreachable!()),
                    }
                    enc.ops.push(I::LocalGet(area));
                }
                (false, h) => match push_flat_values(resolve, sizes, h, &mut enc.ops) {
                    Encoding::Encoded(()) => {}
                    other => return other.map(|_| unreachable!()),
                },
            }
        }
    }
    enc.ops.push(I::End);

    let mut groups: Vec<(u32, ValType)> = Vec::new();
    for t in &enc.locals.types {
        match groups.last_mut() {
            Some((n, seen)) if seen == t => *n += 1,
            _ => groups.push((1, *t)),
        }
    }
    let mut f = Function::new(groups);
    for op in &enc.ops {
        f.instruction(op);
    }
    Encoding::Encoded(f)
}

/// **The component type each value must have where it is used.**
///
/// A constructed variant cannot say what it is: `None` is `None` of whatever
/// its use needs. The world fixes the types of the export's result and of
/// every import's parameters, and a match's arms produce its result, so the
/// type flows backwards from those uses. Only a value with no instruction of
/// its own to type it (a variant, and through it its payload) reads this; every
/// other value is typed where it is made and checked with `same_type` where it
/// is used.
fn expected_types(
    resolve: &Resolve,
    wit: &BTreeMap<Type, WitType>,
    function: &super::ir::Function,
    export_fn: &WitFunction,
    imports: &BTreeMap<String, (u32, WasmSignature, WitFunction)>,
) -> BTreeMap<ValueId, WitType> {
    let mut out = BTreeMap::new();
    let Some(entry) = function.entry() else {
        return out;
    };
    if let (Some(rt), Terminator::Return(v)) = (&export_fn.result, &entry.terminator) {
        out.insert(*v, *rt);
    }
    expect_region(resolve, wit, &entry.instrs, imports, &mut out);
    out
}

fn expect_region(
    resolve: &Resolve,
    wit: &BTreeMap<Type, WitType>,
    instrs: &[Instr],
    imports: &BTreeMap<String, (u32, WasmSignature, WitFunction)>,
    out: &mut BTreeMap<ValueId, WitType>,
) {
    // Backwards: a use comes after the value it uses.
    for instr in instrs.iter().rev() {
        match instr {
            Instr::ImportCall { import, args, .. } => {
                if let Some((_, _, func)) = imports.get(&import.qualified()) {
                    for (a, p) in args.iter().zip(&func.params) {
                        out.entry(*a).or_insert(p.ty);
                    }
                }
            }
            Instr::Match { result, arms, .. } => {
                if let Some(t) = out.get(result).copied() {
                    for arm in arms {
                        out.entry(arm.body.value).or_insert(t);
                    }
                }
                for arm in arms {
                    expect_region(resolve, wit, &arm.body.instrs, imports, out);
                }
            }
            Instr::If {
                result, then, els, ..
            } => {
                if let Some(t) = out.get(result).copied() {
                    out.entry(then.value).or_insert(t);
                    out.entry(els.value).or_insert(t);
                }
                expect_region(resolve, wit, &then.instrs, imports, out);
                expect_region(resolve, wit, &els.instrs, imports, out);
            }
            // A record's field fixes the type of what is stored in it: the
            // `None` in `Entry { redirect: None, .. }`.
            Instr::Construct { args, ty, .. } => {
                if let Some(WitType::Id(id)) = wit.get(ty).map(|t| dealias(resolve, *t))
                    && let TypeDefKind::Record(r) = &resolve.types[id].kind
                {
                    for (a, f) in args.iter().zip(&r.fields) {
                        out.entry(*a).or_insert(f.ty);
                    }
                }
            }
            Instr::Variant {
                result,
                case,
                payload: Some(p),
                ..
            } => {
                if let Some(pt) = out
                    .get(result)
                    .and_then(|t| payload_type(resolve, *t, *case))
                {
                    out.entry(*p).or_insert(pt);
                }
            }
            _ => {}
        }
    }
}

/// The payload type of `case` in the variant type `t`, if it has one.
fn payload_type(resolve: &Resolve, t: WitType, case: BuiltinCase) -> Option<WitType> {
    let WitType::Id(id) = dealias(resolve, t) else {
        return None;
    };
    match (&resolve.types[id].kind, case) {
        (TypeDefKind::Option(p), BuiltinCase::Some) => Some(*p),
        (TypeDefKind::Result(r), BuiltinCase::Ok) => r.ok,
        (TypeDefKind::Result(r), BuiltinCase::Err) => r.err,
        _ => None,
    }
}

/// A variant type's discriminant for `case`, its payload type, and where the
/// payload sits: every number from `SizeAlign`. `None` when `t` is not a
/// variant that has this case.
fn case_layout(
    resolve: &Resolve,
    sizes: &SizeAlign,
    t: WitType,
    case: BuiltinCase,
) -> Option<(u32, Option<WitType>, u64)> {
    let WitType::Id(id) = dealias(resolve, t) else {
        return None;
    };
    let (disc, payload, cases): (u32, Option<WitType>, Vec<Option<WitType>>) =
        match (&resolve.types[id].kind, case) {
            (TypeDefKind::Option(p), BuiltinCase::None) => (0, None, vec![None, Some(*p)]),
            (TypeDefKind::Option(p), BuiltinCase::Some) => (1, Some(*p), vec![None, Some(*p)]),
            (TypeDefKind::Result(r), BuiltinCase::Ok) => (0, r.ok, vec![r.ok, r.err]),
            (TypeDefKind::Result(r), BuiltinCase::Err) => (1, r.err, vec![r.ok, r.err]),
            _ => return None,
        };
    // Two cases: an 8-bit discriminant, as the Canonical ABI gives any
    // variant of up to 256 cases.
    let offset = sizes
        .payload_offset(wit_parser::Int::U8, cases.iter().map(Option::as_ref))
        .size_wasm32() as u64;
    Some((disc, payload, offset))
}

/// The encoder's state while one export's body is emitted.
struct Enc<'a> {
    resolve: &'a Resolve,
    sizes: &'a SizeAlign,
    imports: &'a BTreeMap<String, (u32, WasmSignature, WitFunction)>,
    realloc_index: u32,
    wit: &'a BTreeMap<Type, WitType>,
    literals: &'a Literals,
    helpers: &'a mut Helpers,
    export: &'a str,
    ops: Vec<wasm_encoder::Instruction<'static>>,
    locals: Locals,
    held: BTreeMap<ValueId, Held>,
    expected: BTreeMap<ValueId, WitType>,
}

impl Enc<'_> {
    /// Emit a region's instructions in order.
    fn region(&mut self, instrs: &[Instr]) -> Encoding<()> {
        for instr in instrs {
            match self.instr(instr) {
                Encoding::Encoded(()) => {}
                other => return other,
            }
        }
        Encoding::Encoded(())
    }

    /// A fresh local holding `base + offset`.
    fn address(&mut self, base: u32, offset: u64) -> u32 {
        use wasm_encoder::Instruction as I;
        let l = self.locals.fresh(ValType::I32);
        self.ops.push(I::LocalGet(base));
        if offset != 0 {
            self.ops.push(I::I32Const(offset as i32));
            self.ops.push(I::I32Add);
        }
        self.ops.push(I::LocalSet(l));
        l
    }

    fn instr(&mut self, instr: &Instr) -> Encoding<()> {
        use wasm_encoder::Instruction as I;
        let (resolve, sizes) = (self.resolve, self.sizes);
        match instr {
            Instr::ImportCall {
                result,
                import,
                args,
                ..
            } => {
                let Some((index, sig, func)) = self.imports.get(&import.qualified()) else {
                    blocked!("`{}` has no core import", import.qualified());
                };
                if args.len() != func.params.len() {
                    blocked!(
                        "`{}` passes {} arguments to `{}`, whose world function takes {}",
                        self.export,
                        args.len(),
                        import.qualified(),
                        func.params.len()
                    );
                }
                for (a, param) in args.iter().zip(&func.params) {
                    let Some(h) = self.held.get(a).cloned() else {
                        blocked!(
                            "`{}` passes {a:?} to `{}` and nothing defines it",
                            self.export,
                            import.qualified()
                        );
                    };
                    let Some(t) = h.ty() else {
                        blocked!("`{}` passes a call with no result", self.export);
                    };
                    // **The component-level check.** Two positions that
                    // flatten alike are not therefore one type.
                    if !same_type(resolve, t, &param.ty) {
                        blocked!(
                            "`{}` passes a value of another component type as `{}` of `{}`",
                            self.export,
                            param.name,
                            import.qualified()
                        );
                    }
                    match push_flat_values(resolve, sizes, &h, &mut self.ops) {
                        Encoding::Encoded(()) => {}
                        other => return other,
                    }
                }
                let out = match (&func.result, sig.retptr) {
                    (Some(rt), true) => {
                        let area = self.locals.fresh(ValType::I32);
                        allocate(sizes, rt, self.realloc_index, area, &mut self.ops);
                        self.ops.push(I::LocalGet(area));
                        self.ops.push(I::Call(*index));
                        Held::Memory { ty: *rt, ptr: area }
                    }
                    (Some(rt), false) => {
                        self.ops.push(I::Call(*index));
                        let ls: Vec<u32> = sig
                            .results
                            .iter()
                            .map(|t| self.locals.fresh(core_type(*t)))
                            .collect();
                        for l in ls.iter().rev() {
                            self.ops.push(I::LocalSet(*l));
                        }
                        Held::Flat {
                            ty: *rt,
                            locals: ls,
                        }
                    }
                    (None, _) => {
                        self.ops.push(I::Call(*index));
                        Held::Nothing
                    }
                };
                self.held.insert(*result, out);
            }
            Instr::Const {
                result,
                value: Const::Str(text),
                ty: Type::Str,
            } => {
                let Some(at) = self.literals.at.get(text).copied() else {
                    blocked!(
                        "`{}` names a string with no place in the data segment",
                        self.export
                    );
                };
                let (ptr, len) = (
                    self.locals.fresh(ValType::I32),
                    self.locals.fresh(ValType::I32),
                );
                self.ops.push(I::I32Const(at as i32));
                self.ops.push(I::LocalSet(ptr));
                self.ops.push(I::I32Const(text.len() as i32));
                self.ops.push(I::LocalSet(len));
                self.held.insert(
                    *result,
                    Held::Flat {
                        ty: WitType::String,
                        locals: vec![ptr, len],
                    },
                );
            }
            Instr::Const { result, value, ty } => {
                let (wit, op, vt) = match (value, ty) {
                    (Const::Int(n), Type::Int) => (WitType::S64, I::I64Const(*n), ValType::I64),
                    (Const::Bool(b), Type::Bool) => {
                        (WitType::Bool, I::I32Const(i32::from(*b)), ValType::I32)
                    }
                    (Const::Float(x), Type::Float) => {
                        (WitType::F64, I::F64Const((*x).into()), ValType::F64)
                    }
                    (other, _) => refuse!(
                        "a constant that needs the invocation region",
                        "`{}` holds {other:?}; writing it into memory needs a data \
                         segment this encoder does not emit",
                        self.export
                    ),
                };
                let l = self.locals.fresh(vt);
                self.ops.push(op);
                self.ops.push(I::LocalSet(l));
                self.held.insert(
                    *result,
                    Held::Flat {
                        ty: wit,
                        locals: vec![l],
                    },
                );
            }
            Instr::Call { callee, .. } => refuse!(
                "a call to another Pleris declaration",
                "`{}` calls {callee:?}; a component calls another declaration through \
                 an import, and linking two compiled components is not encoded yet",
                self.export
            ),
            Instr::Construct {
                result, args, ty, ..
            } => return self.construct(*result, args, ty),
            Instr::Project {
                result, of, field, ..
            } => return self.project(*result, *of, *field),
            Instr::Variant {
                result,
                case,
                payload,
                ty,
            } => return self.variant(*result, *case, *payload, ty),
            Instr::Match {
                result,
                scrutinee,
                arms,
                ty,
            } => return self.matched(*result, *scrutinee, arms, ty),
            Instr::Binary {
                result,
                op,
                lhs,
                rhs,
                ty,
            } => return self.binary(*result, *op, *lhs, *rhs, ty),
            Instr::Unary {
                result,
                op,
                operand,
                ty,
            } => return self.unary(*result, *op, *operand, ty),
            Instr::If {
                result,
                cond,
                then,
                els,
                ty,
            } => return self.branch(*result, *cond, then, els, ty),
            Instr::Concat { result, parts, .. } => return self.concat(*result, parts),
            Instr::Format { result, value, .. } => return self.format(*result, *value),
        }
        Encoding::Encoded(())
    }

    /// The component type a value must have: what its use fixes, else what
    /// its IR type is (ADR-0039 §5).
    fn type_of(&self, result: ValueId, ty: &Type) -> Option<WitType> {
        self.expected
            .get(&result)
            .copied()
            .or_else(|| self.wit.get(ty).copied())
    }

    /// **A value's flat core values, in locals**: its own when it is held
    /// flat, read from its layout when it is held in memory.
    fn flat_locals(&mut self, v: ValueId) -> Encoding<(WitType, Vec<u32>)> {
        use wasm_encoder::Instruction as I;
        let (resolve, sizes) = (self.resolve, self.sizes);
        match self.held.get(&v).cloned() {
            Some(Held::Flat { ty, locals }) => Encoding::Encoded((ty, locals)),
            Some(Held::Memory { ty, ptr }) => {
                let Some(flats) = flat(resolve, &ty) else {
                    refuse!(
                        "a value that does not flatten",
                        "`{}` reads a value too large to hold in locals",
                        self.export
                    );
                };
                match load(resolve, sizes, &ty, ptr, 0, &mut self.ops) {
                    Encoding::Encoded(()) => {}
                    other => return other.map(|_| unreachable!()),
                }
                let ls: Vec<u32> = flats
                    .iter()
                    .map(|t| self.locals.fresh(core_type(*t)))
                    .collect();
                for l in ls.iter().rev() {
                    self.ops.push(I::LocalSet(*l));
                }
                Encoding::Encoded((ty, ls))
            }
            Some(Held::Nothing) => blocked!("`{}` uses a call with no result", self.export),
            None => blocked!("`{}` uses {v:?} and nothing defines it", self.export),
        }
    }

    /// **Where a value chosen by a branch is put**: locals, for a type that
    /// flattens without variant slots; an address otherwise.
    fn holder(&mut self, ty: WitType) -> Held {
        match flat(self.resolve, &ty) {
            Some(flats) if plain(self.resolve, &ty) => Held::Flat {
                ty,
                locals: flats
                    .iter()
                    .map(|t| self.locals.fresh(core_type(*t)))
                    .collect(),
            },
            _ => Held::Memory {
                ty,
                ptr: self.locals.fresh(ValType::I32),
            },
        }
    }

    /// Emit a region, and move its value into `holder`.
    fn settle(&mut self, region: &super::ir::Region, holder: &Held) -> Encoding<()> {
        match self.region(&region.instrs) {
            Encoding::Encoded(()) => {}
            other => return other,
        }
        self.move_into(region.value, holder)
    }

    /// Move a value into a branch's holder, checked against its type.
    fn move_into(&mut self, value: ValueId, holder: &Held) -> Encoding<()> {
        use wasm_encoder::Instruction as I;
        let (resolve, sizes) = (self.resolve, self.sizes);
        let Some(value) = self.held.get(&value).cloned() else {
            blocked!("`{}`'s branch ends in a value nothing defines", self.export);
        };
        let Some(vt) = value.ty() else {
            blocked!("`{}`'s branch ends in a call with no result", self.export);
        };
        let rt = *holder.ty().expect("a holder has a type");
        if !same_type(resolve, vt, &rt) {
            blocked!(
                "`{}`'s branches produce values of different component types",
                self.export
            );
        }
        match (holder, &value) {
            (Held::Memory { ptr: out, .. }, Held::Memory { ptr: v, .. }) => {
                self.ops.push(I::LocalGet(*v));
                self.ops.push(I::LocalSet(*out));
            }
            (Held::Memory { ptr: out, .. }, Held::Flat { locals: flats, .. }) => {
                allocate(sizes, &rt, self.realloc_index, *out, &mut self.ops);
                match store(resolve, sizes, &rt, *out, 0, flats, &mut self.ops) {
                    Encoding::Encoded(_) => {}
                    other => return other.map(|_| unreachable!()),
                }
            }
            (Held::Flat { locals: outs, .. }, v) => {
                match push_flat_values(resolve, sizes, v, &mut self.ops) {
                    Encoding::Encoded(()) => {}
                    other => return other,
                }
                for l in outs.iter().rev() {
                    self.ops.push(I::LocalSet(*l));
                }
            }
            (_, Held::Nothing) | (Held::Nothing, _) => {
                blocked!("`{}`'s branch ends in a call with no result", self.export)
            }
        }
        Encoding::Encoded(())
    }

    /// **Arithmetic and comparisons** (ADR-0039 §1, §2).
    fn binary(
        &mut self,
        result: ValueId,
        op: BinaryOp,
        lhs: ValueId,
        rhs: ValueId,
        ty: &Type,
    ) -> Encoding<()> {
        use wasm_encoder::Instruction as I;
        let (lt, l) = match self.flat_locals(lhs) {
            Encoding::Encoded(v) => v,
            other => return other.map(|_| unreachable!()),
        };
        let (rt, r) = match self.flat_locals(rhs) {
            Encoding::Encoded(v) => v,
            other => return other.map(|_| unreachable!()),
        };
        if !same_type(self.resolve, &lt, &rt) {
            blocked!(
                "`{}` applies `{op:?}` to two values of different component types",
                self.export
            );
        }
        let Some(out_ty) = self.type_of(result, ty) else {
            blocked!("`{}`'s `{op:?}` has no component type", self.export);
        };
        let out = match dealias(self.resolve, lt) {
            WitType::S64 => self.int_binary(op, l[0], r[0]),
            WitType::F64 => {
                let (code, vt) = match op {
                    BinaryOp::Add => (I::F64Add, ValType::F64),
                    BinaryOp::Sub => (I::F64Sub, ValType::F64),
                    BinaryOp::Mul => (I::F64Mul, ValType::F64),
                    BinaryOp::Div => (I::F64Div, ValType::F64),
                    BinaryOp::Eq => (I::F64Eq, ValType::I32),
                    BinaryOp::Ne => (I::F64Ne, ValType::I32),
                    BinaryOp::Lt => (I::F64Lt, ValType::I32),
                    BinaryOp::Le => (I::F64Le, ValType::I32),
                    BinaryOp::Gt => (I::F64Gt, ValType::I32),
                    BinaryOp::Ge => (I::F64Ge, ValType::I32),
                    BinaryOp::Rem => refuse!(
                        "`%` on a Float",
                        "`{}`: its semantics are not decided",
                        self.export
                    ),
                };
                let out = self.locals.fresh(vt);
                self.ops.push(I::LocalGet(l[0]));
                self.ops.push(I::LocalGet(r[0]));
                self.ops.push(code);
                self.ops.push(I::LocalSet(out));
                out
            }
            WitType::Bool => {
                let code = match op {
                    BinaryOp::Eq => I::I32Eq,
                    BinaryOp::Ne => I::I32Ne,
                    other => refuse!(
                        "an operator on a Bool",
                        "`{}` applies `{other:?}` to a Bool",
                        self.export
                    ),
                };
                let out = self.locals.fresh(ValType::I32);
                self.ops.push(I::LocalGet(l[0]));
                self.ops.push(I::LocalGet(r[0]));
                self.ops.push(code);
                self.ops.push(I::LocalSet(out));
                out
            }
            WitType::String => {
                let (helper, test) = match op {
                    BinaryOp::Eq => (Helper::StrEq, None),
                    BinaryOp::Ne => (Helper::StrEq, Some(I::I32Eqz)),
                    BinaryOp::Lt => (Helper::StrCmp, Some(I::I32LtS)),
                    BinaryOp::Le => (Helper::StrCmp, Some(I::I32LeS)),
                    BinaryOp::Gt => (Helper::StrCmp, Some(I::I32GtS)),
                    BinaryOp::Ge => (Helper::StrCmp, Some(I::I32GeS)),
                    other => refuse!(
                        "an operator on a String",
                        "`{}` applies `{other:?}` to a String; an interpolation joins strings",
                        self.export
                    ),
                };
                let index = self.helpers.index(helper);
                for x in [l[0], l[1], r[0], r[1]] {
                    self.ops.push(I::LocalGet(x));
                }
                self.ops.push(I::Call(index));
                match test {
                    Some(I::I32Eqz) => self.ops.push(I::I32Eqz),
                    Some(cmp) => {
                        self.ops.push(I::I32Const(0));
                        self.ops.push(cmp);
                    }
                    None => {}
                }
                let out = self.locals.fresh(ValType::I32);
                self.ops.push(I::LocalSet(out));
                out
            }
            other => refuse!(
                "an operator on a value of this type",
                "`{}` applies `{op:?}` to {other:?}",
                self.export
            ),
        };
        self.held.insert(
            result,
            Held::Flat {
                ty: out_ty,
                locals: vec![out],
            },
        );
        Encoding::Encoded(())
    }

    /// **`Int` arithmetic that traps rather than wraps**, with Euclidean `/`
    /// and `%` (ADR-0039 §1). Returns the local holding the result.
    fn int_binary(&mut self, op: BinaryOp, a: u32, b: u32) -> u32 {
        use wasm_encoder::Instruction as I;
        let get = |l: u32| I::LocalGet(l);
        if op.compares() {
            let out = self.locals.fresh(ValType::I32);
            let code = match op {
                BinaryOp::Eq => I::I64Eq,
                BinaryOp::Ne => I::I64Ne,
                BinaryOp::Lt => I::I64LtS,
                BinaryOp::Le => I::I64LeS,
                BinaryOp::Gt => I::I64GtS,
                _ => I::I64GeS,
            };
            self.ops.extend([get(a), get(b), code, I::LocalSet(out)]);
            return out;
        }
        let r = self.locals.fresh(ValType::I64);
        let code = match op {
            BinaryOp::Add => I::I64Add,
            BinaryOp::Sub => I::I64Sub,
            BinaryOp::Mul => I::I64Mul,
            // Wasm truncates, and traps on a zero divisor and on MIN / -1.
            BinaryOp::Div => I::I64DivS,
            BinaryOp::Rem => I::I64RemS,
            _ => unreachable!("comparisons returned above"),
        };
        self.ops.extend([get(a), get(b), code, I::LocalSet(r)]);
        let ops = &mut self.ops;
        match op {
            BinaryOp::Add => trap_on_add_overflow(ops, a, b, r),
            BinaryOp::Sub => trap_on_sub_overflow(ops, a, b, r),
            BinaryOp::Mul => trap_on_mul_overflow(ops, a, b, r),
            BinaryOp::Div => euclidean_quotient(ops, a, b, r),
            BinaryOp::Rem => euclidean_remainder(ops, b, r),
            _ => {}
        }
        r
    }

    /// `-x` and `!b`.
    fn unary(&mut self, result: ValueId, op: UnaryOp, operand: ValueId, ty: &Type) -> Encoding<()> {
        use wasm_encoder::Instruction as I;
        let (t, v) = match self.flat_locals(operand) {
            Encoding::Encoded(x) => x,
            other => return other.map(|_| unreachable!()),
        };
        let Some(out_ty) = self.type_of(result, ty) else {
            blocked!("`{}`'s `{op:?}` has no component type", self.export);
        };
        let out = match (op, dealias(self.resolve, t)) {
            (UnaryOp::Neg, WitType::S64) => {
                let out = self.locals.fresh(ValType::I64);
                trap_on_negation_overflow(&mut self.ops, v[0]);
                self.ops.extend([
                    I::I64Const(0),
                    I::LocalGet(v[0]),
                    I::I64Sub,
                    I::LocalSet(out),
                ]);
                out
            }
            (UnaryOp::Neg, WitType::F64) => {
                let out = self.locals.fresh(ValType::F64);
                self.ops
                    .extend([I::LocalGet(v[0]), I::F64Neg, I::LocalSet(out)]);
                out
            }
            (UnaryOp::Not, WitType::Bool) => {
                let out = self.locals.fresh(ValType::I32);
                self.ops
                    .extend([I::LocalGet(v[0]), I::I32Eqz, I::LocalSet(out)]);
                out
            }
            (op, other) => refuse!(
                "an operator on a value of this type",
                "`{}` applies `{op:?}` to {other:?}",
                self.export
            ),
        };
        self.held.insert(
            result,
            Held::Flat {
                ty: out_ty,
                locals: vec![out],
            },
        );
        Encoding::Encoded(())
    }

    /// **`if c { .. } else { .. }`**: each side's value into one holder.
    fn branch(
        &mut self,
        result: ValueId,
        cond: ValueId,
        then: &super::ir::Region,
        els: &super::ir::Region,
        ty: &Type,
    ) -> Encoding<()> {
        use wasm_encoder::Instruction as I;
        let (ct, c) = match self.flat_locals(cond) {
            Encoding::Encoded(x) => x,
            other => return other.map(|_| unreachable!()),
        };
        if dealias(self.resolve, ct) != WitType::Bool {
            blocked!("`{}` branches on a value that is not a Bool", self.export);
        }
        let Some(rt) = self.type_of(result, ty) else {
            refuse!(
                "an `if` whose component type nothing fixes",
                "`{}`'s `if` produces a value with no component type",
                self.export
            );
        };
        let holder = self.holder(rt);
        self.ops.push(I::LocalGet(c[0]));
        self.ops.push(I::If(wasm_encoder::BlockType::Empty));
        match self.settle(then, &holder) {
            Encoding::Encoded(()) => {}
            other => return other,
        }
        self.ops.push(I::Else);
        match self.settle(els, &holder) {
            Encoding::Encoded(()) => {}
            other => return other,
        }
        self.ops.push(I::End);
        self.held.insert(result, holder);
        Encoding::Encoded(())
    }

    /// **Strings joined**: one allocation of their total length, each copied
    /// in after the one before.
    fn concat(&mut self, result: ValueId, parts: &[ValueId]) -> Encoding<()> {
        use wasm_encoder::BlockType::Empty;
        use wasm_encoder::Instruction as I;
        let mut pieces = Vec::new();
        for p in parts {
            match self.flat_locals(*p) {
                Encoding::Encoded((t, ls)) if dealias(self.resolve, t) == WitType::String => {
                    pieces.push((ls[0], ls[1]))
                }
                Encoding::Encoded(_) => {
                    blocked!("`{}` joins a value that is not a String", self.export)
                }
                other => return other.map(|_| unreachable!()),
            }
        }
        // The total, in 64 bits, refused past what one region could hold.
        let total = self.locals.fresh(ValType::I32);
        self.ops.push(I::I64Const(0));
        for (_, len) in &pieces {
            self.ops
                .extend([I::LocalGet(*len), I::I64ExtendI32U, I::I64Add]);
        }
        let wide = self.locals.fresh(ValType::I64);
        self.ops.extend([
            I::LocalTee(wide),
            I::I64Const(i32::MAX as i64),
            I::I64GtU,
            I::If(Empty),
            I::Unreachable,
            I::End,
            I::LocalGet(wide),
            I::I32WrapI64,
            I::LocalSet(total),
        ]);
        let dst = self.locals.fresh(ValType::I32);
        self.ops.extend([
            I::I32Const(0),
            I::I32Const(0),
            I::I32Const(1),
            I::LocalGet(total),
            I::Call(self.realloc_index),
            I::LocalSet(dst),
        ]);
        let cursor = self.locals.fresh(ValType::I32);
        self.ops.extend([I::LocalGet(dst), I::LocalSet(cursor)]);
        for (ptr, len) in pieces {
            self.ops.extend([
                I::LocalGet(cursor),
                I::LocalGet(ptr),
                I::LocalGet(len),
                I::MemoryCopy {
                    src_mem: 0,
                    dst_mem: 0,
                },
                I::LocalGet(cursor),
                I::LocalGet(len),
                I::I32Add,
                I::LocalSet(cursor),
            ]);
        }
        self.held.insert(
            result,
            Held::Flat {
                ty: WitType::String,
                locals: vec![dst, total],
            },
        );
        Encoding::Encoded(())
    }

    /// **A value as text**: an `Int` in decimal, a `Bool` as `true`/`false`.
    fn format(&mut self, result: ValueId, value: ValueId) -> Encoding<()> {
        use wasm_encoder::Instruction as I;
        let (t, v) = match self.flat_locals(value) {
            Encoding::Encoded(x) => x,
            other => return other.map(|_| unreachable!()),
        };
        let (ptr, len) = (
            self.locals.fresh(ValType::I32),
            self.locals.fresh(ValType::I32),
        );
        match dealias(self.resolve, t) {
            WitType::S64 => {
                let index = self.helpers.index(Helper::IntToString);
                self.ops.extend([
                    I::LocalGet(v[0]),
                    I::Call(index),
                    I::LocalSet(len),
                    I::LocalSet(ptr),
                ]);
            }
            WitType::Bool => {
                let (Some(yes), Some(no)) = (
                    self.literals.at.get("true").copied(),
                    self.literals.at.get("false").copied(),
                ) else {
                    blocked!("`{}` formats a Bool with no literals for it", self.export);
                };
                self.ops.extend([
                    I::I32Const(yes as i32),
                    I::I32Const(no as i32),
                    I::LocalGet(v[0]),
                    I::Select,
                    I::LocalSet(ptr),
                    I::I32Const(4),
                    I::I32Const(5),
                    I::LocalGet(v[0]),
                    I::Select,
                    I::LocalSet(len),
                ]);
            }
            other => refuse!(
                "a value of this type as text",
                "`{}` formats {other:?}",
                self.export
            ),
        }
        self.held.insert(
            result,
            Held::Flat {
                ty: WitType::String,
                locals: vec![ptr, len],
            },
        );
        Encoding::Encoded(())
    }

    /// **A record, built in the region**: its canonical layout, each field
    /// stored at the offset `SizeAlign` gives (ADR-0039 §6).
    fn construct(&mut self, result: ValueId, args: &[ValueId], ty: &Type) -> Encoding<()> {
        use wasm_encoder::Instruction as I;
        let (resolve, sizes) = (self.resolve, self.sizes);
        let Some(rt) = self.type_of(result, ty) else {
            refuse!(
                "a record whose component type nothing fixes",
                "`{}` builds a {ty:?}, which has no component type here",
                self.export
            );
        };
        let WitType::Id(id) = dealias(resolve, rt) else {
            blocked!(
                "`{}` builds a record of a type that is not one",
                self.export
            );
        };
        let TypeDefKind::Record(r) = &resolve.types[id].kind else {
            refuse!(
                "building a declared variant",
                "`{}` builds a value of a declared variant, which is not encoded yet",
                self.export
            );
        };
        if r.fields.len() != args.len() {
            blocked!(
                "`{}` builds a record of {} fields with {} values",
                self.export,
                r.fields.len(),
                args.len()
            );
        }
        let area = self.locals.fresh(ValType::I32);
        allocate(sizes, &rt, self.realloc_index, area, &mut self.ops);
        let offsets = sizes.field_offsets(r.fields.iter().map(|f| &f.ty));
        for ((offset, field_ty), a) in offsets.into_iter().zip(args) {
            let offset = offset.size_wasm32() as u64;
            let Some(h) = self.held.get(a).cloned() else {
                blocked!("`{}` stores {a:?} and nothing defines it", self.export);
            };
            let Some(ht) = h.ty() else {
                blocked!("`{}` stores a call with no result", self.export);
            };
            if !same_type(resolve, ht, field_ty) {
                blocked!(
                    "`{}` stores a value of another component type in a record field",
                    self.export
                );
            }
            match h {
                Held::Memory { ptr, .. } => {
                    self.ops.push(I::LocalGet(area));
                    if offset != 0 {
                        self.ops.push(I::I32Const(offset as i32));
                        self.ops.push(I::I32Add);
                    }
                    self.ops.push(I::LocalGet(ptr));
                    self.ops
                        .push(I::I32Const(sizes.size(field_ty).size_wasm32() as i32));
                    self.ops.push(I::MemoryCopy {
                        src_mem: 0,
                        dst_mem: 0,
                    });
                }
                Held::Flat { locals, .. } => {
                    match store(
                        resolve,
                        sizes,
                        field_ty,
                        area,
                        offset,
                        &locals,
                        &mut self.ops,
                    ) {
                        Encoding::Encoded(_) => {}
                        other => return other.map(|_| unreachable!()),
                    }
                }
                Held::Nothing => blocked!("`{}` stores a call with no result", self.export),
            }
        }
        self.held.insert(result, Held::Memory { ty: rt, ptr: area });
        Encoding::Encoded(())
    }

    /// **A field of a record**: an address inside its layout, or a slice of
    /// its flat values.
    fn project(&mut self, result: ValueId, of: ValueId, field: u32) -> Encoding<()> {
        let resolve = self.resolve;
        let Some(h) = self.held.get(&of).cloned() else {
            blocked!(
                "`{}` reads a field of {of:?} and nothing defines it",
                self.export
            );
        };
        let Some(t) = h.ty().copied() else {
            blocked!("`{}` reads a field of a call with no result", self.export);
        };
        let WitType::Id(id) = dealias(resolve, t) else {
            blocked!(
                "`{}` reads a field of a value that is not a record",
                self.export
            );
        };
        let TypeDefKind::Record(r) = &resolve.types[id].kind else {
            blocked!(
                "`{}` reads a field of a value that is not a record",
                self.export
            );
        };
        let Some(field_ty) = r.fields.get(field as usize).map(|f| f.ty) else {
            blocked!(
                "`{}` reads field {field} of a record with {} fields",
                self.export,
                r.fields.len()
            );
        };
        let out = match h {
            Held::Memory { ptr, .. } => {
                let offsets = self.sizes.field_offsets(r.fields.iter().map(|f| &f.ty));
                let offset = offsets[field as usize].0.size_wasm32() as u64;
                Held::Memory {
                    ty: field_ty,
                    ptr: self.address(ptr, offset),
                }
            }
            Held::Flat { locals, .. } => {
                // The fields' flat values, in field order: this one's are the
                // slice after every earlier field's.
                let mut skip = 0;
                for f in &r.fields[..field as usize] {
                    let Some(flats) = flat(resolve, &f.ty) else {
                        refuse!(
                            "a field of a record that does not flatten",
                            "`{}` reads a field past one that does not flatten",
                            self.export
                        );
                    };
                    skip += flats.len();
                }
                let Some(n) = flat(resolve, &field_ty).map(|f| f.len()) else {
                    refuse!(
                        "a field that does not flatten",
                        "`{}` reads a field that does not flatten",
                        self.export
                    );
                };
                Held::Flat {
                    ty: field_ty,
                    locals: locals[skip..skip + n].to_vec(),
                }
            }
            Held::Nothing => blocked!("`{}` reads a field of a call with no result", self.export),
        };
        self.held.insert(result, out);
        Encoding::Encoded(())
    }

    /// **`Some(x)`, `None`, `Ok(x)`, `Err(e)`**, written into the invocation
    /// region: the discriminant, then the payload at the offset `SizeAlign`
    /// gives. The type is the one its use fixes (`expected_types`).
    fn variant(
        &mut self,
        result: ValueId,
        case: BuiltinCase,
        payload: Option<ValueId>,
        ty: &Type,
    ) -> Encoding<()> {
        use wasm_encoder::Instruction as I;
        let (resolve, sizes) = (self.resolve, self.sizes);
        let Some(t) = self.type_of(result, ty) else {
            refuse!(
                "a variant whose component type nothing fixes",
                "`{}` builds `{case:?}`, and no use of it names a type in the world",
                self.export
            );
        };
        let Some((disc, payload_ty, offset)) = case_layout(resolve, sizes, t, case) else {
            blocked!(
                "`{}` builds `{case:?}` where its use needs another type",
                self.export
            );
        };
        let area = self.locals.fresh(ValType::I32);
        allocate(sizes, &t, self.realloc_index, area, &mut self.ops);
        self.ops.push(I::LocalGet(area));
        self.ops.push(I::I32Const(disc as i32));
        self.ops.push(I::I32Store8(MemArg {
            offset: 0,
            align: 0,
            memory_index: 0,
        }));
        match (payload, payload_ty) {
            (None, None) => {}
            (Some(p), Some(pt)) => {
                let Some(h) = self.held.get(&p).cloned() else {
                    blocked!("`{}` wraps {p:?} and nothing defines it", self.export);
                };
                let Some(ht) = h.ty() else {
                    blocked!("`{}` wraps a call with no result", self.export);
                };
                if !same_type(resolve, ht, &pt) {
                    blocked!(
                        "`{}` wraps a value of another component type in `{case:?}`",
                        self.export
                    );
                }
                match h {
                    // The payload's layout is already in the region: copy it.
                    Held::Memory { ptr, .. } => {
                        self.ops.push(I::LocalGet(area));
                        if offset != 0 {
                            self.ops.push(I::I32Const(offset as i32));
                            self.ops.push(I::I32Add);
                        }
                        self.ops.push(I::LocalGet(ptr));
                        self.ops
                            .push(I::I32Const(sizes.size(&pt).size_wasm32() as i32));
                        self.ops.push(I::MemoryCopy {
                            src_mem: 0,
                            dst_mem: 0,
                        });
                    }
                    Held::Flat { locals, .. } => {
                        match store(resolve, sizes, &pt, area, offset, &locals, &mut self.ops) {
                            Encoding::Encoded(_) => {}
                            other => return other.map(|_| unreachable!()),
                        }
                    }
                    Held::Nothing => {
                        blocked!("`{}` wraps a call with no result", self.export)
                    }
                }
            }
            _ => blocked!(
                "`{}` builds `{case:?}` with a payload its type does not have, or without \
                 one it does",
                self.export
            ),
        }
        self.held.insert(result, Held::Memory { ty: t, ptr: area });
        Encoding::Encoded(())
    }

    /// **Choose by a variant's case**: the discriminant, read from the
    /// scrutinee's layout, selects an arm; each arm binds its payload and moves
    /// its value into one holder, checked against the match's component type.
    fn matched(
        &mut self,
        result: ValueId,
        scrutinee: ValueId,
        arms: &[super::ir::MatchArm],
        ty: &Type,
    ) -> Encoding<()> {
        use wasm_encoder::Instruction as I;
        let (resolve, sizes) = (self.resolve, self.sizes);
        let Some(Held::Memory { ty: st, ptr }) = self.held.get(&scrutinee).cloned() else {
            refuse!(
                "a match over a value not held in memory",
                "`{}` matches a value held flat; a variant's flat form joins its \
                 cases' slots, which this encoder does not read",
                self.export
            );
        };
        let Some(rt) = self.type_of(result, ty) else {
            refuse!(
                "a match whose component type nothing fixes",
                "`{}`'s match is used where the world names no type",
                self.export
            );
        };
        let [first, second] = arms else {
            blocked!("`{}` matches with {} arms, not 2", self.export, arms.len());
        };
        // The result's holder: locals for a value that flattens without
        // variant slots, an address for anything else.
        let holder = self.holder(rt);

        // Discriminant 1 selects the second-numbered case: `Some` for an
        // option, `Err` for a result.
        let (one, zero) = {
            let d = |a: &super::ir::MatchArm| case_layout(resolve, sizes, st, a.case);
            match (d(first), d(second)) {
                (Some((1, ..)), Some((0, ..))) => (first, second),
                (Some((0, ..)), Some((1, ..))) => (second, first),
                _ => blocked!(
                    "`{}` matches cases the scrutinee's type does not have",
                    self.export
                ),
            }
        };
        self.ops.push(I::LocalGet(ptr));
        self.ops.push(I::I32Load8U(MemArg {
            offset: 0,
            align: 0,
            memory_index: 0,
        }));
        self.ops.push(I::If(wasm_encoder::BlockType::Empty));
        match self.arm(ptr, st, one, &holder) {
            Encoding::Encoded(()) => {}
            other => return other,
        }
        self.ops.push(I::Else);
        match self.arm(ptr, st, zero, &holder) {
            Encoding::Encoded(()) => {}
            other => return other,
        }
        self.ops.push(I::End);
        self.held.insert(result, holder);
        Encoding::Encoded(())
    }

    /// One arm: bind the payload, emit the region, move its value.
    fn arm(
        &mut self,
        ptr: u32,
        st: WitType,
        arm: &super::ir::MatchArm,
        holder: &Held,
    ) -> Encoding<()> {
        let (resolve, sizes) = (self.resolve, self.sizes);
        let Some((_, payload_ty, offset)) = case_layout(resolve, sizes, st, arm.case) else {
            blocked!(
                "`{}` matches a case its scrutinee does not have",
                self.export
            );
        };
        if let (Some(b), Some(pt)) = (arm.binding, payload_ty) {
            let at = self.address(ptr, offset);
            self.held.insert(b, Held::Memory { ty: pt, ptr: at });
        }
        self.settle(&arm.body, holder)
    }
}

type Ops = Vec<wasm_encoder::Instruction<'static>>;

/// `unreachable` when the `i32` on the stack is not zero.
fn trap_if(ops: &mut Ops) {
    use wasm_encoder::Instruction as I;
    ops.extend([
        I::If(wasm_encoder::BlockType::Empty),
        I::Unreachable,
        I::End,
    ]);
}

/// After `r = a + b`: it overflowed iff both operands' signs differ from the
/// result's.
fn trap_on_add_overflow(ops: &mut Ops, a: u32, b: u32, r: u32) {
    use wasm_encoder::Instruction as I;
    ops.extend([
        I::LocalGet(a),
        I::LocalGet(r),
        I::I64Xor,
        I::LocalGet(b),
        I::LocalGet(r),
        I::I64Xor,
        I::I64And,
        I::I64Const(0),
        I::I64LtS,
    ]);
    trap_if(ops);
}

/// After `r = a - b`: it overflowed iff the operands' signs differ and the
/// result's differs from the left one's.
fn trap_on_sub_overflow(ops: &mut Ops, a: u32, b: u32, r: u32) {
    use wasm_encoder::Instruction as I;
    ops.extend([
        I::LocalGet(a),
        I::LocalGet(b),
        I::I64Xor,
        I::LocalGet(a),
        I::LocalGet(r),
        I::I64Xor,
        I::I64And,
        I::I64Const(0),
        I::I64LtS,
    ]);
    trap_if(ops);
}

/// After `r = a * b`: it is exact iff dividing back gives the other operand.
/// `r / a` traps by itself only for `MIN / -1`, which is `b == MIN`: an
/// overflow either way.
fn trap_on_mul_overflow(ops: &mut Ops, a: u32, b: u32, r: u32) {
    use wasm_encoder::Instruction as I;
    ops.extend([
        I::LocalGet(a),
        I::I64Eqz,
        I::I32Eqz,
        I::If(wasm_encoder::BlockType::Empty),
        I::LocalGet(r),
        I::LocalGet(a),
        I::I64DivS,
        I::LocalGet(b),
        I::I64Ne,
    ]);
    trap_if(ops);
    ops.push(I::End);
}

/// After `q = a / b`, truncated: a negative remainder moves the quotient one
/// step away from the divisor's sign. `-7 / 2` is -4, `-7 / -2` is 4.
fn euclidean_quotient(ops: &mut Ops, a: u32, b: u32, q: u32) {
    use wasm_encoder::BlockType::Empty;
    use wasm_encoder::Instruction as I;
    ops.extend([
        I::LocalGet(a),
        I::LocalGet(b),
        I::I64RemS,
        I::I64Const(0),
        I::I64LtS,
        I::If(Empty),
        I::LocalGet(b),
        I::I64Const(0),
        I::I64GtS,
        I::If(Empty),
        I::LocalGet(q),
        I::I64Const(1),
        I::I64Sub,
        I::LocalSet(q),
        I::Else,
        I::LocalGet(q),
        I::I64Const(1),
        I::I64Add,
        I::LocalSet(q),
        I::End,
        I::End,
    ]);
}

/// After `m = a % b`, truncated: into `[0, |b|)`. For `b == MIN`, `m - b` is
/// exact, because `m` is above MIN.
fn euclidean_remainder(ops: &mut Ops, b: u32, m: u32) {
    use wasm_encoder::BlockType::Empty;
    use wasm_encoder::Instruction as I;
    ops.extend([
        I::LocalGet(m),
        I::I64Const(0),
        I::I64LtS,
        I::If(Empty),
        I::LocalGet(b),
        I::I64Const(0),
        I::I64GtS,
        I::If(Empty),
        I::LocalGet(m),
        I::LocalGet(b),
        I::I64Add,
        I::LocalSet(m),
        I::Else,
        I::LocalGet(m),
        I::LocalGet(b),
        I::I64Sub,
        I::LocalSet(m),
        I::End,
        I::End,
    ]);
}

/// Before `0 - a`: `-MIN` does not fit.
fn trap_on_negation_overflow(ops: &mut Ops, a: u32) {
    use wasm_encoder::Instruction as I;
    ops.extend([I::LocalGet(a), I::I64Const(i64::MIN), I::I64Eq]);
    trap_if(ops);
}

/// **Write flat values into a value's canonical layout**: the inverse of
/// [`load`], with the same offsets from `SizeAlign`. Returns how many flat
/// values it consumed.
fn store(
    resolve: &Resolve,
    sizes: &SizeAlign,
    ty: &WitType,
    base: u32,
    offset: u64,
    flats: &[u32],
    ops: &mut Vec<wasm_encoder::Instruction<'static>>,
) -> Encoding<usize> {
    use wasm_encoder::Instruction as I;
    let mem = |align: u32, extra: u64| MemArg {
        offset: offset + extra,
        align,
        memory_index: 0,
    };
    let one = |ops: &mut Vec<I<'static>>, op: I<'static>| -> Encoding<usize> {
        let Some(v) = flats.first() else {
            blocked!("a store ran out of flat values");
        };
        ops.push(I::LocalGet(base));
        ops.push(I::LocalGet(*v));
        ops.push(op);
        Encoding::Encoded(1)
    };
    match dealias(resolve, *ty) {
        WitType::Bool | WitType::U8 | WitType::S8 => one(ops, I::I32Store8(mem(0, 0))),
        WitType::U16 | WitType::S16 => one(ops, I::I32Store16(mem(1, 0))),
        WitType::U32 | WitType::S32 | WitType::Char => one(ops, I::I32Store(mem(2, 0))),
        WitType::U64 | WitType::S64 => one(ops, I::I64Store(mem(3, 0))),
        WitType::F32 => one(ops, I::F32Store(mem(2, 0))),
        WitType::F64 => one(ops, I::F64Store(mem(3, 0))),
        WitType::String => pointer_and_length(base, offset, flats, ops),
        WitType::Id(id) => match &resolve.types[id].kind {
            TypeDefKind::List(_) => pointer_and_length(base, offset, flats, ops),
            TypeDefKind::Record(r) => {
                let mut used = 0;
                for (field_offset, field_ty) in sizes.field_offsets(r.fields.iter().map(|f| &f.ty))
                {
                    let o = field_offset.size_wasm32() as u64;
                    match store(
                        resolve,
                        sizes,
                        field_ty,
                        base,
                        offset + o,
                        &flats[used..],
                        ops,
                    ) {
                        Encoding::Encoded(n) => used += n,
                        other => return other,
                    }
                }
                Encoding::Encoded(used)
            }
            other => refuse!(
                "writing a value of this kind into memory",
                "{other:?} flattens with joined variant slots, which this encoder does \
                 not write yet"
            ),
        },
        other => refuse!(
            "writing a value of this kind into memory",
            "{other:?} has no store here yet"
        ),
    }
}

/// A string or a list: its pointer and its length, each a 32-bit word.
fn pointer_and_length(
    base: u32,
    offset: u64,
    flats: &[u32],
    ops: &mut Vec<wasm_encoder::Instruction<'static>>,
) -> Encoding<usize> {
    use wasm_encoder::Instruction as I;
    let [ptr, len, ..] = flats else {
        blocked!("a pointer and length need two flat values");
    };
    for (v, extra) in [(ptr, 0), (len, 4)] {
        ops.push(I::LocalGet(base));
        ops.push(I::LocalGet(*v));
        ops.push(I::I32Store(MemArg {
            offset: offset + extra,
            align: 2,
            memory_index: 0,
        }));
    }
    Encoding::Encoded(2)
}

/// Push a held value's flat core values onto the stack.
fn push_flat_values(
    resolve: &Resolve,
    sizes: &SizeAlign,
    h: &Held,
    ops: &mut Vec<wasm_encoder::Instruction<'static>>,
) -> Encoding<()> {
    use wasm_encoder::Instruction as I;
    match h {
        Held::Flat { locals, .. } => {
            for l in locals {
                ops.push(I::LocalGet(*l));
            }
            Encoding::Encoded(())
        }
        Held::Memory { ty, ptr } => load(resolve, sizes, ty, *ptr, 0, ops),
        Held::Nothing => blocked!("a call with no result was used as a value"),
    }
}

/// **Read a value's canonical layout into its flat values.** Offsets and sizes
/// from `SizeAlign`; the order is the one `push_flat` gives.
fn load(
    resolve: &Resolve,
    sizes: &SizeAlign,
    ty: &WitType,
    ptr: u32,
    offset: u64,
    ops: &mut Vec<wasm_encoder::Instruction<'static>>,
) -> Encoding<()> {
    use wasm_encoder::Instruction as I;
    let mem = |align: u32| MemArg {
        offset,
        align,
        memory_index: 0,
    };
    let at = |ops: &mut Vec<I<'static>>| ops.push(I::LocalGet(ptr));
    match dealias(resolve, *ty) {
        WitType::Bool | WitType::U8 => {
            at(ops);
            ops.push(I::I32Load8U(mem(0)));
        }
        WitType::S8 => {
            at(ops);
            ops.push(I::I32Load8S(mem(0)));
        }
        WitType::U16 => {
            at(ops);
            ops.push(I::I32Load16U(mem(1)));
        }
        WitType::S16 => {
            at(ops);
            ops.push(I::I32Load16S(mem(1)));
        }
        WitType::U32 | WitType::S32 | WitType::Char => {
            at(ops);
            ops.push(I::I32Load(mem(2)));
        }
        WitType::U64 | WitType::S64 => {
            at(ops);
            ops.push(I::I64Load(mem(3)));
        }
        WitType::F32 => {
            at(ops);
            ops.push(I::F32Load(mem(2)));
        }
        WitType::F64 => {
            at(ops);
            ops.push(I::F64Load(mem(3)));
        }
        // A string is a pointer and a length, each a 32-bit word.
        WitType::String => {
            at(ops);
            ops.push(I::I32Load(mem(2)));
            at(ops);
            ops.push(I::I32Load(MemArg {
                offset: offset + 4,
                align: 2,
                memory_index: 0,
            }));
        }
        WitType::Id(id) => match &resolve.types[id].kind {
            TypeDefKind::Record(r) => {
                let offsets = sizes.field_offsets(r.fields.iter().map(|f| &f.ty));
                for (field_offset, field_ty) in offsets {
                    let o = field_offset.size_wasm32() as u64;
                    match load(resolve, sizes, field_ty, ptr, offset + o, ops) {
                        Encoding::Encoded(()) => {}
                        other => return other,
                    }
                }
            }
            TypeDefKind::List(_) => {
                at(ops);
                ops.push(I::I32Load(mem(2)));
                at(ops);
                ops.push(I::I32Load(MemArg {
                    offset: offset + 4,
                    align: 2,
                    memory_index: 0,
                }));
            }
            other => refuse!(
                "reading a value of this kind from memory",
                "{other:?} flattens with joined variant slots, which this encoder does \
                 not read yet"
            ),
        },
        other => refuse!(
            "reading a value of this kind from memory",
            "{other:?} has no load here yet"
        ),
    }
    Encoding::Encoded(())
}

/// Allocate a result area for `ty` from the region, into `into`.
fn allocate(
    sizes: &SizeAlign,
    ty: &WitType,
    realloc_index: u32,
    into: u32,
    ops: &mut Vec<wasm_encoder::Instruction<'static>>,
) {
    use wasm_encoder::Instruction as I;
    ops.push(I::I32Const(0));
    ops.push(I::I32Const(0));
    ops.push(I::I32Const(sizes.align(ty).align_wasm32() as i32));
    ops.push(I::I32Const(sizes.size(ty).size_wasm32() as i32));
    ops.push(I::Call(realloc_index));
    ops.push(I::LocalSet(into));
}

/// **`cabi_realloc`: the bump allocator over the invocation region.**
///
/// `(old_ptr, old_size, align, new_size) -> ptr`. Aligns the top of the
/// region, grows memory when the allocation does not fit, and traps rather
/// than returning an address it does not own. A reallocation copies what the
/// old block held.
fn realloc() -> Function {
    use wasm_encoder::Instruction as I;
    // params: 0 old_ptr, 1 old_size, 2 align, 3 new_size; locals: 4 ptr, 5 end
    let mut f = Function::new([(2, ValType::I32)]);
    let ops = [
        // ptr = (top + align - 1) & -align
        I::GlobalGet(0),
        I::LocalGet(2),
        I::I32Add,
        I::I32Const(1),
        I::I32Sub,
        I::I32Const(0),
        I::LocalGet(2),
        I::I32Sub,
        I::I32And,
        I::LocalSet(4),
        // end = ptr + new_size, trapping on wrap-around
        I::LocalGet(4),
        I::LocalGet(3),
        I::I32Add,
        I::LocalTee(5),
        I::LocalGet(4),
        I::I32LtU,
        I::If(wasm_encoder::BlockType::Empty),
        I::Unreachable,
        I::End,
        // grow when end is past the current memory
        I::Block(wasm_encoder::BlockType::Empty),
        I::LocalGet(5),
        I::MemorySize(0),
        I::I32Const(16),
        I::I32Shl,
        I::I32LeU,
        I::BrIf(0),
        I::LocalGet(5),
        I::MemorySize(0),
        I::I32Const(16),
        I::I32Shl,
        I::I32Sub,
        I::I32Const(65535),
        I::I32Add,
        I::I32Const(16),
        I::I32ShrU,
        I::MemoryGrow(0),
        I::I32Const(-1),
        I::I32Ne,
        I::BrIf(0),
        I::Unreachable,
        I::End,
        I::LocalGet(5),
        I::GlobalSet(0),
        // copy the old block's contents on a reallocation
        I::Block(wasm_encoder::BlockType::Empty),
        I::LocalGet(0),
        I::I32Eqz,
        I::BrIf(0),
        I::LocalGet(4),
        I::LocalGet(0),
        I::LocalGet(1),
        I::LocalGet(3),
        I::LocalGet(1),
        I::LocalGet(3),
        I::I32LtU,
        I::Select,
        I::MemoryCopy {
            src_mem: 0,
            dst_mem: 0,
        },
        I::End,
        I::LocalGet(4),
        I::End,
    ];
    for op in &ops {
        f.instruction(op);
    }
    f
}

/// The export's post-return: the caller has lifted the result, so the whole
/// invocation region is reclaimed, down to the literals below it.
fn post_return(heap_base: i32) -> Function {
    use wasm_encoder::Instruction as I;
    let mut f = Function::new([]);
    f.instruction(&I::I32Const(heap_base));
    f.instruction(&I::GlobalSet(0));
    f.instruction(&I::End);
    f
}

/// Does `ty` flatten without a variant's joined slots: primitives, strings,
/// lists, and records of those?
fn plain(resolve: &Resolve, ty: &WitType) -> bool {
    match dealias(resolve, *ty) {
        WitType::Id(id) => match &resolve.types[id].kind {
            TypeDefKind::Record(r) => r.fields.iter().all(|f| plain(resolve, &f.ty)),
            TypeDefKind::List(_) => true,
            TypeDefKind::Tuple(t) => t.types.iter().all(|t| plain(resolve, t)),
            _ => false,
        },
        _ => true,
    }
}

/// **The module's own functions, beyond the three every module has**: small
/// routines a body calls rather than repeating inline. Each is emitted once,
/// after `post_return`, only if a body uses it.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Helper {
    /// `(p1, l1, p2, l2) -> 1 | 0`: byte equality of two strings.
    StrEq,
    /// `(p1, l1, p2, l2) -> -1 | 0 | 1`: byte order of two strings, which is
    /// code point order for UTF-8.
    StrCmp,
    /// `(i64) -> (ptr, len)`: decimal text, allocated in the region.
    IntToString,
}

struct Helpers {
    /// The index the first helper gets.
    first: u32,
    used: Vec<Helper>,
}

impl Helpers {
    fn index(&mut self, h: Helper) -> u32 {
        let at = match self.used.iter().position(|u| *u == h) {
            Some(i) => i,
            None => {
                self.used.push(h);
                self.used.len() - 1
            }
        };
        self.first + at as u32
    }
}

impl Helper {
    fn signature(self) -> (Vec<ValType>, Vec<ValType>) {
        match self {
            Helper::StrEq | Helper::StrCmp => (vec![ValType::I32; 4], vec![ValType::I32]),
            Helper::IntToString => (vec![ValType::I64], vec![ValType::I32, ValType::I32]),
        }
    }

    fn body(self, realloc_index: u32) -> Function {
        use wasm_encoder::BlockType::Empty;
        use wasm_encoder::Instruction as I;
        let byte = |offset: u64| {
            I::I32Load8U(MemArg {
                offset,
                align: 0,
                memory_index: 0,
            })
        };
        let (locals, ops): (Vec<(u32, ValType)>, Vec<I<'static>>) = match self {
            // params: 0 p1, 1 l1, 2 p2, 3 l2; local 4 i
            Helper::StrEq => (
                vec![(1, ValType::I32)],
                vec![
                    I::LocalGet(1),
                    I::LocalGet(3),
                    I::I32Ne,
                    I::If(Empty),
                    I::I32Const(0),
                    I::Return,
                    I::End,
                    I::Block(Empty),
                    I::Loop(Empty),
                    I::LocalGet(4),
                    I::LocalGet(1),
                    I::I32GeU,
                    I::BrIf(1),
                    I::LocalGet(0),
                    I::LocalGet(4),
                    I::I32Add,
                    byte(0),
                    I::LocalGet(2),
                    I::LocalGet(4),
                    I::I32Add,
                    byte(0),
                    I::I32Ne,
                    I::If(Empty),
                    I::I32Const(0),
                    I::Return,
                    I::End,
                    I::LocalGet(4),
                    I::I32Const(1),
                    I::I32Add,
                    I::LocalSet(4),
                    I::Br(0),
                    I::End,
                    I::End,
                    I::I32Const(1),
                    I::End,
                ],
            ),
            // params: 0 p1, 1 l1, 2 p2, 3 l2; locals 4 i, 5 n, 6 a, 7 b
            Helper::StrCmp => (
                vec![(4, ValType::I32)],
                vec![
                    // n = min(l1, l2)
                    I::LocalGet(1),
                    I::LocalGet(3),
                    I::LocalGet(1),
                    I::LocalGet(3),
                    I::I32LtU,
                    I::Select,
                    I::LocalSet(5),
                    I::Block(Empty),
                    I::Loop(Empty),
                    I::LocalGet(4),
                    I::LocalGet(5),
                    I::I32GeU,
                    I::BrIf(1),
                    I::LocalGet(0),
                    I::LocalGet(4),
                    I::I32Add,
                    byte(0),
                    I::LocalSet(6),
                    I::LocalGet(2),
                    I::LocalGet(4),
                    I::I32Add,
                    byte(0),
                    I::LocalSet(7),
                    I::LocalGet(6),
                    I::LocalGet(7),
                    I::I32Ne,
                    I::If(Empty),
                    I::I32Const(-1),
                    I::I32Const(1),
                    I::LocalGet(6),
                    I::LocalGet(7),
                    I::I32LtU,
                    I::Select,
                    I::Return,
                    I::End,
                    I::LocalGet(4),
                    I::I32Const(1),
                    I::I32Add,
                    I::LocalSet(4),
                    I::Br(0),
                    I::End,
                    I::End,
                    // A proper prefix orders first.
                    I::LocalGet(1),
                    I::LocalGet(3),
                    I::I32LtU,
                    I::If(Empty),
                    I::I32Const(-1),
                    I::Return,
                    I::End,
                    I::LocalGet(1),
                    I::LocalGet(3),
                    I::I32GtU,
                    I::End,
                ],
            ),
            // param: 0 v; locals 1 end, 2 pos, 3 neg, 4 u (i64)
            Helper::IntToString => (
                vec![(3, ValType::I32), (1, ValType::I64)],
                vec![
                    // 20 bytes hold "-9223372036854775808".
                    I::I32Const(0),
                    I::I32Const(0),
                    I::I32Const(1),
                    I::I32Const(20),
                    I::Call(realloc_index),
                    I::I32Const(20),
                    I::I32Add,
                    I::LocalTee(1),
                    I::LocalSet(2),
                    I::LocalGet(0),
                    I::I64Const(0),
                    I::I64LtS,
                    I::LocalSet(3),
                    // The magnitude, unsigned: `0 - MIN` wraps to 2^63, which
                    // is MIN's magnitude read unsigned.
                    I::I64Const(0),
                    I::LocalGet(0),
                    I::I64Sub,
                    I::LocalGet(0),
                    I::LocalGet(3),
                    I::Select,
                    I::LocalSet(4),
                    I::Loop(Empty),
                    I::LocalGet(2),
                    I::I32Const(1),
                    I::I32Sub,
                    I::LocalTee(2),
                    I::LocalGet(4),
                    I::I64Const(10),
                    I::I64RemU,
                    I::I32WrapI64,
                    I::I32Const(48),
                    I::I32Add,
                    I::I32Store8(MemArg {
                        offset: 0,
                        align: 0,
                        memory_index: 0,
                    }),
                    I::LocalGet(4),
                    I::I64Const(10),
                    I::I64DivU,
                    I::LocalTee(4),
                    I::I64Const(0),
                    I::I64Ne,
                    I::BrIf(0),
                    I::End,
                    I::LocalGet(3),
                    I::If(Empty),
                    I::LocalGet(2),
                    I::I32Const(1),
                    I::I32Sub,
                    I::LocalTee(2),
                    I::I32Const(45),
                    I::I32Store8(MemArg {
                        offset: 0,
                        align: 0,
                        memory_index: 0,
                    }),
                    I::End,
                    I::LocalGet(2),
                    I::LocalGet(1),
                    I::LocalGet(2),
                    I::I32Sub,
                    I::End,
                ],
            ),
        };
        let mut f = Function::new(locals);
        for op in &ops {
            f.instruction(op);
        }
        f
    }
}
