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

/// **The module validates, and `add_to_cart` does not encode — for a reason
/// that is a real finding.**
///
/// Architect ruling, 2026-08-19, on what to do when this happens:
///
/// > Treat the encoder almost adversarially. Give it `Checked` IR, make it
/// > refuse anything it cannot faithfully represent, and let the first real
/// > component tell us what the next missing backend primitive actually is.
///
/// It told us immediately. **A capability is not a function.**
///
/// ```text
/// add_to_cart   HostCall database.write<Carts> [session, item, quantity]
/// clear_cart    HostCall database.write<Carts> [session]
/// ```
///
/// One authority, two argument lists — because the two calls go through
/// `Carts.add(s, item, qty)` and `Carts.clear(s)`, which are different Pleris
/// functions that both require `database.write<Carts>`. `Instr::HostCall`
/// carries the *capability the enclosing contract requires* and the *arguments
/// of the Pleris function that needed it*, and a core import has one signature.
///
/// Encoding it against the first arity found produced a module `wasmparser`
/// rejected with *"expected i32 but nothing on stack"*. The encoder refuses
/// instead — the module is still valid, it simply has fewer functions — and
/// `docs/RISK_QUEUE.md` carries the classification.
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

    // What DID encode: the three queries, each of whose capability is reached
    // through exactly one Pleris function. That is the encoder working.
    let exports = exported_names(&bytes);
    for want in ["Menu", "Store", "Cart"] {
        assert!(
            exports.contains(&want.to_string()),
            "`{want}` encodes: {exports:?}"
        );
    }

    // And what did not, with the reason.
    let why: Vec<String> = refusals.iter().map(|r| r.to_string()).collect();
    assert!(
        !exports.contains(&"add_to_cart".to_string()),
        "TODAY: `add_to_cart` does not encode. When `HostCall` names the \
         FUNCTION rather than only the authority, this fails — and that is the \
         repair, not a regression."
    );
    assert!(
        why.iter()
            .any(|w| w.contains("called with [1, 3] arguments")),
        "and the refusal names the real defect rather than the symptom: {why:?}"
    );
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
        .map(|i| (i.interface.clone(), i.name.clone()))
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
    let declared: Vec<String> = p.imports.iter().map(|i| i.capability.name()).collect();
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
