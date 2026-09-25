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
    CodeSection, ConstExpr, EntityType, ExportKind, ExportSection, Function, FunctionSection,
    GlobalSection, GlobalType, ImportSection, MemArg, MemorySection, MemoryType, Module,
    TypeSection, ValType,
};
use wit_parser::abi::{AbiVariant, FlatTypes, WasmSignature, WasmType};
use wit_parser::{
    Function as WitFunction, LiftLowerAbi, ManglingAndAbi, Resolve, SizeAlign, Type as WitType,
    TypeDefKind, WasmExport, WasmExportKind, WasmImport, WorldId, WorldItem, WorldKey,
};

use super::ir::{CallableImport, Const, Instr, Terminator, Type, ValueId};

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
    let mut sizes = SizeAlign::default();
    sizes.fill(resolve);

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
    for i in &entry.instrs {
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
    let body = match export_body(
        resolve,
        &sizes,
        function,
        &export_fn,
        &export_sig,
        &import_index,
        realloc_index,
    ) {
        Encoding::Encoded(b) => b,
        other => return other.map(|_| unreachable!()),
    };

    let mut funcs = FunctionSection::new();
    funcs.function(realloc_ty);
    funcs.function(export_ty);
    funcs.function(post_ty);

    let mut memories = MemorySection::new();
    memories.memory(MemoryType {
        minimum: 1,
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
        &ConstExpr::i32_const(HEAP_BASE),
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
    code.function(&post_return());

    module.section(&types);
    module.section(&import_section);
    module.section(&funcs);
    module.section(&memories);
    module.section(&globals);
    module.section(&exports);
    module.section(&code);
    Encoding::Encoded(module.finish())
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

/// The instructions of the export's body. Encoded against a scratch list so
/// the locals it needs are known before the `Function` is created.
fn export_body(
    resolve: &Resolve,
    sizes: &SizeAlign,
    function: &super::ir::Function,
    export_fn: &WitFunction,
    export_sig: &WasmSignature,
    imports: &BTreeMap<String, (u32, WasmSignature, WitFunction)>,
    realloc_index: u32,
) -> Encoding<Function> {
    use wasm_encoder::Instruction as I;

    let mut ops: Vec<I<'static>> = Vec::new();
    let mut locals = Locals {
        first: export_sig.params.len() as u32,
        types: Vec::new(),
    };
    let mut held: BTreeMap<ValueId, Held> = BTreeMap::new();

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
        held.insert(
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
    for instr in &entry.instrs {
        match instr {
            Instr::ImportCall {
                result,
                import,
                args,
                ..
            } => {
                let Some((index, sig, func)) = imports.get(&import.qualified()) else {
                    blocked!("`{}` has no core import", import.qualified());
                };
                if args.len() != func.params.len() {
                    blocked!(
                        "`{}` passes {} arguments to `{}`, whose world function takes {}",
                        function.export,
                        args.len(),
                        import.qualified(),
                        func.params.len()
                    );
                }
                for (a, param) in args.iter().zip(&func.params) {
                    let Some(h) = held.get(a) else {
                        blocked!(
                            "`{}` passes {a:?} to `{}` and nothing defines it",
                            function.export,
                            import.qualified()
                        );
                    };
                    let Some(t) = h.ty() else {
                        blocked!("`{}` passes a call with no result", function.export);
                    };
                    // **The component-level check.** Two positions that
                    // flatten alike are not therefore one type.
                    if !same_type(resolve, t, &param.ty) {
                        blocked!(
                            "`{}` passes a value of another component type as `{}` of `{}`",
                            function.export,
                            param.name,
                            import.qualified()
                        );
                    }
                    match push_flat_values(resolve, sizes, h, &mut ops) {
                        Encoding::Encoded(()) => {}
                        other => return other.map(|_| unreachable!()),
                    }
                }
                let out = match (&func.result, sig.retptr) {
                    (Some(rt), true) => {
                        let area = locals.fresh(ValType::I32);
                        allocate(sizes, rt, realloc_index, area, &mut ops);
                        ops.push(I::LocalGet(area));
                        ops.push(I::Call(*index));
                        Held::Memory { ty: *rt, ptr: area }
                    }
                    (Some(rt), false) => {
                        ops.push(I::Call(*index));
                        let ls: Vec<u32> = sig
                            .results
                            .iter()
                            .map(|t| locals.fresh(core_type(*t)))
                            .collect();
                        for l in ls.iter().rev() {
                            ops.push(I::LocalSet(*l));
                        }
                        Held::Flat {
                            ty: *rt,
                            locals: ls,
                        }
                    }
                    (None, _) => {
                        ops.push(I::Call(*index));
                        Held::Nothing
                    }
                };
                held.insert(*result, out);
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
                        function.export
                    ),
                };
                let l = locals.fresh(vt);
                ops.push(op);
                ops.push(I::LocalSet(l));
                held.insert(
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
                function.export
            ),
            Instr::Construct { .. } => refuse!(
                "building a record or variant",
                "`{}` constructs a value, which needs a layout written into the region",
                function.export
            ),
            Instr::Project { .. } => refuse!(
                "reading a field",
                "`{}` projects a field of a value it does not hold flat",
                function.export
            ),
        }
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
    let Some(h) = held.get(v) else {
        blocked!("`{}` returns {v:?} and nothing defines it", function.export);
    };
    match (&export_fn.result, h) {
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
                (true, Held::Memory { ptr, .. }) => ops.push(I::LocalGet(*ptr)),
                (true, Held::Nothing) => {
                    blocked!("`{}` returns a call with no result", function.export)
                }
                (true, Held::Flat { .. }) => refuse!(
                    "a flat value returned by pointer",
                    "`{}` would have to store its result into the region first",
                    function.export
                ),
                (false, h) => match push_flat_values(resolve, sizes, h, &mut ops) {
                    Encoding::Encoded(()) => {}
                    other => return other.map(|_| unreachable!()),
                },
            }
        }
    }
    ops.push(I::End);

    let mut groups: Vec<(u32, ValType)> = Vec::new();
    for t in &locals.types {
        match groups.last_mut() {
            Some((n, seen)) if seen == t => *n += 1,
            _ => groups.push((1, *t)),
        }
    }
    let mut f = Function::new(groups);
    for op in &ops {
        f.instruction(op);
    }
    Encoding::Encoded(f)
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
/// invocation region is reclaimed.
fn post_return() -> Function {
    use wasm_encoder::Instruction as I;
    let mut f = Function::new([]);
    f.instruction(&I::I32Const(HEAP_BASE));
    f.instruction(&I::GlobalSet(0));
    f.instruction(&I::End);
    f
}
