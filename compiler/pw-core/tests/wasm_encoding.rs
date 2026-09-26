//! **The store's commands and queries become Canonical-ABI core modules.**
//!
//! Architect ruling, 2026-08-19:
//!
//! > Encode the smallest real `add_to_cart` path first. […] **No semantic
//! > rediscovery in the encoder.** […] **Unsupported IR is a hard backend
//! > result.** Never emit `nop`, zero, empty block, dummy result, etc. for an
//! > instruction you haven't implemented.
//!
//! The E10-A encoder held every non-scalar as an `i32` handle, so its modules
//! validated and could not be components: `carts#add` had three core
//! parameters where the Canonical ABI gives six. E10-I replaced it — one
//! encoder, with every flattening and name read from `wit-parser` — and these
//! are its controls, each of which the handle encoder's version also asserted
//! in its own terms.
//!
//! # A module this repo declares valid is worth nothing
//!
//! Every "valid" below is `wasmparser::Validator`'s verdict.

use pw_core::backend::component;
use pw_core::backend::ir::Program;
use pw_core::backend::lower::{Checked, Context, program};
use pw_core::backend::wasm::{self, Encoding};
use pw_core::check::Unit;
use pw_core::contract::{ComponentContract, contracts};
use pw_core::hir::Hir;
use pw_core::lower::lower_file;
use pw_core::resolve::Workspace;
use pw_core::signatures::Signatures;
use pw_syntax::parse_tree;

fn store_files() -> Vec<(String, String)> {
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    let mut files = Vec::new();
    for d in [
        "packages/pw-std",
        "packages/pw-platform-web",
        "examples/lib",
        "examples/store",
    ] {
        let mut ps: Vec<std::path::PathBuf> = std::fs::read_dir(root.join(d))
            .unwrap_or_else(|e| panic!("{d}: {e}"))
            .map(|e| e.expect("entry").path())
            .filter(|p| p.extension().is_some_and(|x| x == "pw"))
            .collect();
        ps.sort();
        for p in ps {
            files.push((
                p.file_name().unwrap().to_string_lossy().to_string(),
                std::fs::read_to_string(&p).expect("read"),
            ));
        }
    }
    files.push((
        "domain.pw".to_string(),
        std::fs::read_to_string(root.join("examples/domain.pw")).expect("domain"),
    ));
    files
}

fn units(files: &[(String, String)]) -> Vec<Unit> {
    files
        .iter()
        .map(|(path, src)| Unit {
            path: path.clone(),
            src: src.clone(),
            hir: lower_file(src, &parse_tree(src).green),
        })
        .collect()
}

/// The store, lowered, with its contracts.
fn store() -> (Program, Vec<ComponentContract>) {
    let files = store_files();
    let units = units(&files);
    let hirs: Vec<Hir> = files
        .iter()
        .map(|(_, s)| lower_file(s, &parse_tree(s).green))
        .collect();
    let refs: Vec<&Hir> = hirs.iter().collect();
    let ws = Workspace::build(&refs);
    let sigs = Signatures::build(&ws, &refs);
    let cs = contracts(&refs, &sigs, &ws);
    let cx = Context {
        hirs: &refs,
        ws: &ws,
        sigs: &sigs,
        contracts: &cs,
    };
    let checked = Checked::of(&units, cx).unwrap_or_else(|ds| {
        panic!(
            "the store does not check: {:?}",
            ds.iter().map(|d| d.code).collect::<Vec<_>>()
        )
    });
    (program(&checked).0, cs.clone())
}

/// `wasmparser`'s verdict, not this repo's.
fn validate(bytes: &[u8]) -> Result<(), String> {
    wasmparser::Validator::new()
        .validate_all(bytes)
        .map(|_| ())
        .map_err(|e| e.to_string())
}

/// The components the store's commands and queries compile to.
const COMPONENTS: [&str; 5] = [
    "store.page.Cart",
    "store.page.Menu",
    "store.page.Store",
    "store.page.add_to_cart",
    "store.page.clear_cart",
];

// --- the gate ----------------------------------------------------------------

/// **Every command and query in the store compiles to a component whose core
/// module validates.** Five, not one: `add_to_cart` staying inside the
/// supported set would prove little if its neighbours were refused.
#[test]
fn every_store_command_and_query_encodes_to_a_valid_core_module() {
    let units = units(&store_files());
    for id in COMPONENTS {
        let compiled =
            component::compile(&units, id).unwrap_or_else(|e| panic!("`{id}` must compile: {e}"));
        validate(&compiled.component.core)
            .unwrap_or_else(|e| panic!("`{id}`'s core module is not valid Wasm: {e}"));
        component::audit(
            &compiled.component.bytes,
            &compiled.wit,
            &compiled.component.world,
        )
        .unwrap_or_else(|w| panic!("`{id}` disagrees with its world: {w:?}"));
    }
}

/// **The control the ruling asked to be frozen: same capability, two
/// callables, both valid.**
///
/// > `Carts.add` / `Carts.clear` — same capability, different signatures →
/// > **two callable imports**, both valid.
///
/// Their core arities are now the Canonical ABI's — `add` six (session, item
/// and quantity flattened, plus the result pointer), `clear` three — read back
/// from the bytes `wit-parser` shaped.
#[test]
fn one_capability_authorizes_two_callables_with_different_abis() {
    let (p, _) = store();
    let add = p
        .imports
        .iter()
        .find(|i| i.id.qualified() == "store:data/carts#add")
        .expect("Carts.add is an import");
    let clear = p
        .imports
        .iter()
        .find(|i| i.id.qualified() == "store:data/carts#clear")
        .expect("Carts.clear is an import");
    assert_ne!(add.signature, clear.signature, "different ABIs");
    let caps = |i: &pw_core::backend::ir::CallableImport| -> Vec<String> {
        i.required_capabilities.iter().map(|c| c.name()).collect()
    };
    assert_eq!(caps(add), ["database.write<Carts>"], "one authority");
    assert_eq!(caps(clear), ["database.write<Carts>"]);

    let units = units(&store_files());
    let add_core = component::compile(&units, "store.page.add_to_cart")
        .expect("add_to_cart")
        .component
        .core;
    let clear_core = component::compile(&units, "store.page.clear_cart")
        .expect("clear_cart")
        .component
        .core;
    assert_eq!(
        imported_arities(&add_core).get("store:data/carts#add"),
        Some(&6)
    );
    assert_eq!(
        imported_arities(&clear_core).get("store:data/carts#clear"),
        Some(&3)
    );
}

/// **Mutate only the declared ABI and the encoder refuses.**
///
/// The world is the ABI authority: remove a parameter from `carts#add` in the
/// WIT the contract fixed, leave the call site alone, and the encoder must
/// refuse — never emit a call that pushes three values to a two-value import.
#[test]
fn an_import_whose_declared_abi_disagrees_with_its_call_site_is_refused() {
    let units = units(&store_files());
    let compiled = component::compile(&units, "store.page.add_to_cart").expect("builds");
    let original = "add: func(arg0: capability-session-id, arg1: domain-menu-item-id, arg2: \
                    domain-positive-int) -> result<domain-cart, domain-cart-error>;";
    assert!(
        compiled.wit.contains(original),
        "the mutation's anchor moved"
    );
    let mutated = compiled.wit.replace(
        original,
        "add: func(arg0: capability-session-id, arg1: domain-menu-item-id) -> \
         result<domain-cart, domain-cart-error>;",
    );

    let (p, cs) = store();
    let f = p
        .functions
        .iter()
        .find(|f| f.export == "add_to_cart")
        .expect("lowered");
    let (resolve, world) =
        component::world_of(&mutated, &compiled.component.world).expect("mutated WIT parses");
    let _ = cs;
    match wasm::core_module(&resolve, world, f, "add-to-cart", &p.imports) {
        Encoding::Blocked { why } => {
            assert!(
                why.contains("passes 3 arguments"),
                "refused for the ABI: {why}"
            )
        }
        other => panic!("a call the world's ABI does not describe must be refused: {other}"),
    }
}

/// **A component imports only what its contract allows**, never a superset.
///
/// E8's rule, at the artifact: each compiled component's core imports are
/// among its OWN contract's imports.
#[test]
fn every_component_imports_only_what_its_contract_allows() {
    let units = units(&store_files());
    for id in COMPONENTS {
        let compiled = component::compile(&units, id).expect("compiles");
        let allowed: Vec<String> = compiled.contract.imports.iter().map(|i| i.key()).collect();
        let extra: Vec<&String> = compiled
            .component
            .imports
            .iter()
            .filter(|i| !allowed.contains(i))
            .collect();
        assert!(
            extra.is_empty(),
            "`{id}` imports what its contract does not allow: {extra:?}"
        );
        assert!(
            !compiled.component.imports.is_empty(),
            "`{id}` imports something, so the subset is not vacuous"
        );
    }
}

/// **Every capability a function calls has an import**, or the encoder refuses.
#[test]
fn every_capability_a_function_calls_has_an_import() {
    let (p, _) = store();
    let declared: Vec<String> = p
        .imports
        .iter()
        .flat_map(|i| i.required_capabilities.iter().map(|c| c.name()))
        .collect();
    let mut missing = Vec::new();
    for f in &p.functions {
        for c in &f.capabilities {
            if !declared.contains(&c.name()) {
                missing.push(format!("{}: {}", f.export, c.name()));
            }
        }
    }
    assert!(
        missing.is_empty(),
        "a function calls a capability the program declares no import for: {missing:?}"
    );
}

/// **An unimplemented instruction is refused by name, never encoded.**
///
/// A call to another declaration encoded to a zero would produce a module that
/// validates, links, instantiates and does the wrong thing. The lowering
/// inlines such calls (ADR-0039 §4), and compiles a recursive one beside the
/// export (ADR-0050); a `Call` to an instance nothing compiled comes only from
/// a program built by hand, and it is refused.
#[test]
fn an_unimplemented_instruction_is_refused_rather_than_faked() {
    use pw_core::backend::ir::{Block, BlockId, Function, Instr, Terminator, Type, ValueId};
    use pw_core::resolve::DefId;

    let wit = "package t:t;\ninterface api {\n    builds: func() -> s64;\n}\nworld w {\n    export api;\n}\n";
    let (resolve, world) = component::world_of(wit, "w").expect("the control's WIT");
    let def = DefId { unit: 0, decl: 0 };
    let f = Function {
        def,
        export: "builds".to_string(),
        params: vec![],
        ret: Type::Int,
        blocks: vec![Block {
            id: BlockId(0),
            instrs: vec![Instr::Call {
                result: ValueId(0),
                callee: DefId { unit: 0, decl: 1 },
                instance: vec![],
                args: vec![],
                ty: Type::Int,
            }],
            terminator: Terminator::Return(ValueId(0)),
        }],
        capabilities: vec![],
        instance: vec![],
        callees: vec![],
    };
    match wasm::core_module(&resolve, world, &f, "builds", &[]) {
        Encoding::Blocked { why } => {
            assert!(why.contains("nothing compiled"), "{why}")
        }
        other => panic!("a call to nothing compiled must be refused: {other}"),
    }
}

/// **A declared variant is not built** (ADR-0039 §6): a record is, and a
/// variant's construction is refused rather than laid out as a record.
#[test]
fn building_a_declared_variant_is_refused() {
    use pw_core::backend::ir::{Block, BlockId, Function, Instr, Terminator, Type, ValueId};
    use pw_core::resolve::DefId;

    let wit = "package t:t;\ninterface api {\n    variant v { a, b(s64) }\n    builds: func() -> v;\n}\nworld w {\n    export api;\n}\n";
    let (resolve, world) = component::world_of(wit, "w").expect("the control's WIT");
    let def = DefId { unit: 0, decl: 0 };
    let f = Function {
        def,
        export: "builds".to_string(),
        params: vec![],
        ret: Type::Nominal(def),
        blocks: vec![Block {
            id: BlockId(0),
            instrs: vec![Instr::Construct {
                result: ValueId(0),
                ctor: def,
                args: vec![],
                ty: Type::Nominal(def),
            }],
            terminator: Terminator::Return(ValueId(0)),
        }],
        capabilities: vec![],
        instance: vec![],
        callees: vec![],
    };
    match wasm::core_module(&resolve, world, &f, "builds", &[]) {
        Encoding::Unsupported { construct, .. } => {
            assert!(construct.contains("declared variant"), "{construct}")
        }
        other => panic!("a variant's construction must be refused by name: {other}"),
    }
}

/// **The encoder never decides anything from a name.**
#[test]
fn the_encoder_never_decides_from_a_name() {
    let dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("src/backend");
    let mut offenders = Vec::new();
    let mut scanned = 0;
    for file in ["wasm.rs", "component.rs"] {
        let src = std::fs::read_to_string(dir.join(file)).expect(file);
        scanned += src.lines().count();
        for (n, line) in src.lines().enumerate() {
            let l = line.trim();
            if l.starts_with("//") || l.starts_with("///") {
                continue;
            }
            // A capability, effect, interface or type spelling compared as a
            // literal. Which interface an import is comes from the world; which
            // types agree from `same_type`.
            if l.contains("== \"database")
                || l.contains("== \"session")
                || l.contains("== \"pw:host")
                || l.contains("== \"store:")
                || l.contains("starts_with(\"pw:host")
                || l.contains("contains(\"database")
                || l.contains("\"domain-")
            {
                offenders.push(format!("{file}:{}: {l}", n + 1));
            }
        }
    }
    assert!(scanned > 500, "the scan read too little to mean anything");
    assert!(
        offenders.is_empty(),
        "the encoder decides something from a spelling:\n  {}",
        offenders.join("\n  ")
    );
}

// --- reading the bytes back --------------------------------------------------

/// How many parameters each import's type declares, read back from the bytes.
fn imported_arities(bytes: &[u8]) -> std::collections::BTreeMap<String, usize> {
    let mut types: Vec<usize> = Vec::new();
    let mut out = std::collections::BTreeMap::new();
    for payload in wasmparser::Parser::new(0).parse_all(bytes) {
        match payload {
            Ok(wasmparser::Payload::TypeSection(s)) => {
                for g in s.into_iter().flatten() {
                    for t in g.into_types() {
                        types.push(match t.composite_type.inner {
                            wasmparser::CompositeInnerType::Func(f) => f.params().len(),
                            _ => 0,
                        });
                    }
                }
            }
            Ok(wasmparser::Payload::ImportSection(s)) => {
                for i in s.into_imports() {
                    let i = i.expect("import");
                    if let wasmparser::TypeRef::Func(t) = i.ty {
                        out.insert(
                            format!("{}#{}", i.module, i.name),
                            types.get(t as usize).copied().unwrap_or(0),
                        );
                    }
                }
            }
            _ => {}
        }
    }
    out
}
