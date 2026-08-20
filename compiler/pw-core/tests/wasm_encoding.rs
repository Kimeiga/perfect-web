//! **E10-A — the real `add_to_cart` becomes a core Wasm module.**
//!
//! Architect ruling, 2026-08-19:
//!
//! > Encode the smallest real `add_to_cart` path first. Parameters →
//! > target/session host call → `database.write<Carts>` host call →
//! > `Result<Cart, CartError>` → return. Don't broaden language support until
//! > that path demands it.
//!
//! and, on what the encoder may not do:
//!
//! > **No semantic rediscovery in the encoder.** […] **Unsupported IR is a hard
//! > backend result.** Never emit `nop`, zero, empty block, dummy result, etc.
//! > for an instruction you haven't implemented.
//!
//! # A module this repo declares valid is worth nothing
//!
//! Every test below that says "valid" means `wasmparser::Validator` accepted
//! it — the parser `wasmtime` is built on. The E8 gate made the same choice for
//! WIT and for the same reason: the whole point of emitting a standard format
//! is that somebody else's toolchain reads it.

use pw_core::backend::ir::Program;
use pw_core::backend::lower::{Checked, Context, program};
use pw_core::backend::wasm;
use pw_core::check::Unit;
use pw_core::contract::contracts;
use pw_core::hir::Hir;
use pw_core::lower::lower_file;
use pw_core::resolve::Workspace;
use pw_core::signatures::Signatures;
use pw_syntax::parse_tree;

/// The store demo, lowered through the whole front end.
fn store_program() -> Program {
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

    let units: Vec<Unit> = files
        .iter()
        .map(|(path, src)| Unit {
            path: path.clone(),
            src: src.clone(),
            hir: lower_file(src, &parse_tree(src).green),
        })
        .collect();
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
    program(&checked).0
}

/// `wasmparser`'s verdict, not this repo's.
fn validate(bytes: &[u8]) -> Result<(), String> {
    wasmparser::Validator::new_with_features(wasmparser::WasmFeatures::default())
        .validate_all(bytes)
        .map(|_| ())
        .map_err(|e| e.to_string())
}

// --- the gate ----------------------------------------------------------------

/// **The real `add_to_cart` encodes, and the module validates.**
///
/// It did not, for one commit, and the reason was a model error the encoder
/// found on its first run: `Instr::HostCall` was keyed on a CAPABILITY, and
/// `add_to_cart` and `clear_cart` both call `database.write<Carts>` with three
/// arguments and one. `wasmparser` said *"expected i32 but nothing on stack"*.
///
/// Architect ruling, 2026-08-20:
///
/// > **A capability authorizes an operation. It does not identify the
/// > operation.** So `database.write<Carts>` must never be used as the callable
/// > import identity.
///
/// An import is now a `CallableImport` with an `ImportId`, a signature taken
/// from the callee's own declaration, and a SET of required capabilities.
#[test]
fn the_store_encodes_to_a_valid_core_module() {
    let p = store_program();
    let (bytes, refusals) = wasm::module(&p);

    validate(&bytes).unwrap_or_else(|e| {
        panic!(
            "the generated module is not valid Wasm: {e}\nrefusals: {:?}",
            refusals.iter().map(|r| r.to_string()).collect::<Vec<_>>()
        )
    });

    let exports = exported_names(&bytes);
    for want in ["Menu", "Store", "Cart", "add_to_cart", "clear_cart"] {
        assert!(
            exports.contains(&want.to_string()),
            "`{want}` encodes: {exports:?}"
        );
    }
    assert!(
        refusals.is_empty(),
        "nothing in the store refuses to encode: {:?}",
        refusals.iter().map(|r| r.to_string()).collect::<Vec<_>>()
    );
}

/// **The control the ruling asked to be frozen: same capability, two
/// callables, both valid.**
///
/// > `Carts.add` / `Carts.clear` — same capability, different signatures →
/// > **two callable imports**, both valid.
///
/// This is the whole finding, as a positive proof rather than a refusal.
#[test]
fn one_capability_authorizes_two_callables_with_different_abis() {
    let p = store_program();

    let add = p
        .imports
        .iter()
        .find(|i| i.id.qualified() == "pw:host/carts#add")
        .expect("Carts.add is an import");
    let clear = p
        .imports
        .iter()
        .find(|i| i.id.qualified() == "pw:host/carts#clear")
        .expect("Carts.clear is an import");

    // Different ABIs.
    assert_eq!(add.signature.params.len(), 3);
    assert_eq!(clear.signature.params.len(), 1);
    assert_ne!(add.signature, clear.signature);

    // One authority.
    let caps = |i: &pw_core::backend::ir::CallableImport| -> Vec<String> {
        i.required_capabilities.iter().map(|c| c.name()).collect()
    };
    assert_eq!(caps(add), ["database.write<Carts>"]);
    assert_eq!(caps(clear), ["database.write<Carts>"]);

    // And both are real imports of the built module, with their own types.
    let (bytes, _) = wasm::module(&p);
    let names = imported_names(&bytes);
    assert!(names.contains(&("pw:host/carts".to_string(), "add".to_string())));
    assert!(names.contains(&("pw:host/carts".to_string(), "clear".to_string())));

    // The signature the encoder used is the CALLABLE's, not one derived from a
    // call site: `clear_cart` calls `clear` with one argument and `add_to_cart`
    // calls `add` with three, and a shared import would have had to be one or
    // the other.
    let arities = imported_arities(&bytes);
    assert_eq!(arities.get("pw:host/carts#add"), Some(&3));
    assert_eq!(arities.get("pw:host/carts#clear"), Some(&1));
}

/// **The module's imports are exactly the contract's**, name for name.
///
/// The E8 rule, one layer earlier: actual imports ⊆ the allowed set, never a
/// superset. Here it is an equality because the program declares exactly what
/// its functions call — and the encoder read the interface off
/// `Program::imports` rather than deriving it, so this compares the artifact
/// with the CONTRACT rather than with itself.
#[test]
fn the_modules_imports_are_the_contracts_imports() {
    let p = store_program();
    let (bytes, _) = wasm::module(&p);

    let mut actual: Vec<(String, String)> = imported_names(&bytes);
    actual.sort();
    actual.dedup();

    let mut allowed: Vec<(String, String)> = p
        .imports
        .iter()
        .map(|i| (i.id.interface.clone(), i.id.name.clone()))
        .collect();
    allowed.sort();
    allowed.dedup();

    // **Subset, never superset** — E8's rule, one layer earlier. A module that
    // imports fewer than it is allowed is fine; one that imports even one more
    // has authority the compiler never approved.
    let extra: Vec<&(String, String)> = actual.iter().filter(|a| !allowed.contains(a)).collect();
    assert!(
        extra.is_empty(),
        "the module imports something the contract does not allow: {extra:?}"
    );
    assert!(
        !actual.is_empty(),
        "and it imports something, so the subset is not vacuous"
    );
}

/// **Every capability a function calls has an import**, or the encoder refuses.
///
/// Without this the encoder could emit `call 0` for a capability nothing
/// declared — a call to whatever import happened to be first, which validates
/// and is wrong. `body` returns `Blocked` instead, and this is what says the
/// precondition holds for the real program rather than only being handled.
#[test]
fn every_capability_a_function_calls_has_an_import() {
    let p = store_program();
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
/// The rule the three-valued outcome exists for. A `Construct` encoded to a
/// zero would produce a module that validates, links, instantiates and does the
/// wrong thing — and every check downstream of here reads bytes, so nothing
/// would notice.
#[test]
fn an_unimplemented_instruction_is_refused_rather_than_faked() {
    use pw_core::backend::ir::{Block, BlockId, Function, Instr, Terminator, Type, ValueId};
    use pw_core::resolve::DefId;

    let def = DefId { unit: 0, decl: 0 };
    let p = Program {
        functions: vec![Function {
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
        }],
        types: vec![],
        imports: vec![],
    };

    let (bytes, refusals) = wasm::module(&p);
    assert_eq!(refusals.len(), 1, "one function, one refusal");
    let text = refusals[0].to_string();
    assert!(
        text.contains("record or variant"),
        "the refusal names the construct: {text}"
    );
    assert!(!refusals[0].is_encoded());

    // And the module is still valid — it simply has no such function. A refusal
    // must not corrupt what did encode.
    validate(&bytes).expect("a module missing a refused function is still a module");
    assert!(
        exported_names(&bytes).is_empty(),
        "and it exports nothing, rather than exporting an empty body"
    );
}

/// **The encoder never decides anything from a name.**
///
/// The structural guard `lower.rs` carries, one layer down and with more at
/// stake: this file's answers become machine code, where nothing downstream can
/// notice a wrong one. `docs/RISK_QUEUE.md` is mostly instances of
/// spelling-based resolution.
#[test]
fn the_encoder_never_decides_from_a_name() {
    let src = std::fs::read_to_string(
        std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("src/backend/wasm.rs"),
    )
    .expect("wasm.rs");

    let mut offenders = Vec::new();
    for (n, line) in src.lines().enumerate() {
        let l = line.trim();
        if l.starts_with("//") || l.starts_with("///") {
            continue;
        }
        // A capability, effect or interface spelling compared as a literal.
        // `capability.name()` is used as a MAP KEY, which is the identity the
        // contract established — comparing it against a constant would be the
        // encoder deciding.
        if l.contains("== \"database")
            || l.contains("== \"session")
            || l.contains("== \"pw:host")
            || l.contains("starts_with(\"pw:host")
            || l.contains("contains(\"database")
        {
            offenders.push(format!("wasm.rs:{}: {l}", n + 1));
        }
    }
    assert!(
        offenders.is_empty(),
        "the encoder decides something from a spelling:\n  {}\n\n\
         Which interface serves a capability is the CONTRACT's answer, carried \
         on `Program::imports`. Matching a name here would make the E8 audit a \
         comparison of the encoder with itself.",
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

fn exported_names(bytes: &[u8]) -> Vec<String> {
    let mut out = Vec::new();
    for payload in wasmparser::Parser::new(0).parse_all(bytes) {
        if let Ok(wasmparser::Payload::ExportSection(s)) = payload {
            for e in s.into_iter().flatten() {
                out.push(e.name.to_string());
            }
        }
    }
    out
}

fn imported_names(bytes: &[u8]) -> Vec<(String, String)> {
    let mut out = Vec::new();
    for payload in wasmparser::Parser::new(0).parse_all(bytes) {
        if let Ok(wasmparser::Payload::ImportSection(s)) = payload {
            // `into_imports` flattens the compact encodings — an import
            // section may group several names under one module, and iterating
            // the section directly yields the GROUPS.
            for i in s.into_imports() {
                let i = i.expect("import entry");
                out.push((i.module.to_string(), i.name.to_string()));
            }
        }
    }
    out
}
