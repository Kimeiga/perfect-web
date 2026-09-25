//! **E10-I steps 4-6: the store's `add_to_cart`, compiled to a component.**
//!
//! ```text
//! 4  Canonical ABI adapters from checked callable signatures
//! 5  component wrapping via UPSTREAM wit-component
//! 6  validate with an independent component parser
//! ```
//!
//! Every assertion here reads the artifact back through a parser this
//! repository did not write: `wasmparser` for validity, `wit-component`'s
//! decoder for the component's types. A component this repo wrote and this
//! repo read would prove only that the two halves agree with each other.

use pw_core::backend::component::{self, Component};
use pw_core::backend::ir::Program;
use pw_core::backend::lower::{Checked, Context, program};
use pw_core::backend::wasm::Encoding;
use pw_core::contract::contracts;
use pw_core::hir::Hir;
use pw_core::lower::lower_file;
use pw_core::resolve::Workspace;
use pw_core::signatures::Signatures;
use pw_syntax::parse_tree;

fn root() -> std::path::PathBuf {
    std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../..")
}

fn store() -> Vec<(String, String)> {
    let mut out = Vec::new();
    for dir in [
        "packages/pw-std",
        "packages/pw-platform-web",
        "examples/lib",
        "examples/store",
    ] {
        let mut files: Vec<std::path::PathBuf> = std::fs::read_dir(root().join(dir))
            .unwrap_or_else(|e| panic!("{dir}: {e}"))
            .map(|e| e.expect("entry").path())
            .filter(|p| p.extension().is_some_and(|x| x == "pw"))
            .collect();
        files.sort();
        for p in files {
            out.push((
                p.file_name().unwrap().to_string_lossy().to_string(),
                std::fs::read_to_string(&p).expect("read"),
            ));
        }
    }
    out.push((
        "domain.pw".to_string(),
        std::fs::read_to_string(root().join("examples/domain.pw")).expect("domain.pw"),
    ));
    out
}

/// The program, its WIT package, its worlds and its lowered IR.
struct Compiled {
    wit: String,
    worlds: Vec<pw_core::wit::World>,
    program: Program,
}

fn compile(files: &[(String, String)]) -> Compiled {
    let hirs: Vec<Hir> = files
        .iter()
        .map(|(_, s)| lower_file(s, &parse_tree(s).green))
        .collect();
    let units: Vec<pw_core::check::Unit> = files
        .iter()
        .map(|(path, src)| pw_core::check::Unit {
            path: path.clone(),
            src: src.clone(),
            hir: lower_file(src, &parse_tree(src).green),
        })
        .collect();
    let refs: Vec<&Hir> = hirs.iter().collect();
    let ws = Workspace::build(&refs);
    let sigs = Signatures::build(&ws, &refs);
    let cs = contracts(&refs, &sigs, &ws);
    let (wit, worlds) = pw_core::wit::package(&refs, &ws, &cs).expect("the store generates WIT");
    let cx = Context {
        hirs: &refs,
        ws: &ws,
        sigs: &sigs,
        contracts: &cs,
    };
    let checked = Checked::of(&units, cx).unwrap_or_else(|ds| {
        panic!(
            "the store must check before the backend sees it: {:?}",
            ds.iter().map(|d| (d.code, &d.message)).collect::<Vec<_>>()
        )
    });
    let (program, _) = program(&checked);
    Compiled {
        wit,
        worlds,
        program,
    }
}

fn build(c: &Compiled, export: &str) -> Encoding<Component> {
    let f = c
        .program
        .functions
        .iter()
        .find(|f| f.export == export)
        .unwrap_or_else(|| panic!("`{export}` lowers"));
    let world = c
        .worlds
        .iter()
        .find(|w| w.declaration == f.def)
        .unwrap_or_else(|| panic!("`{export}` has a world"));
    component::build(&c.wit, world, f, &c.program, &Default::default())
}

fn add_to_cart() -> (Compiled, Component) {
    let c = compile(&store());
    let built = build(&c, "add_to_cart");
    let component = match built {
        Encoding::Encoded(component) => component,
        other => panic!("`add_to_cart` must compile to a component: {other}"),
    };
    (c, component)
}

#[test]
fn add_to_cart_compiles_to_a_component_an_independent_validator_accepts() {
    let (_, component) = add_to_cart();
    let mut validator = wasmparser::Validator::new_with_features(wasmparser::WasmFeatures::all());
    validator
        .validate_all(&component.bytes)
        .expect("wasmparser validates the component");
    // And the core module on its own, with the default feature set a plain
    // engine has.
    wasmparser::Validator::new()
        .validate_all(&component.core)
        .expect("wasmparser validates the core module");
}

#[test]
fn the_component_imports_exactly_the_operations_the_function_calls() {
    let (_, component) = add_to_cart();
    // `store:data/carts` declares `add`, `clear` and `current`. The command
    // calls `add`, and reads the session: exactly those two.
    assert_eq!(
        component.imports,
        ["pw:host/session#read", "store:data/carts#add"],
        "the core module's own import list"
    );
}

#[test]
fn the_component_has_exactly_the_types_its_world_fixed() {
    let (c, component) = add_to_cart();
    let compared = component::audit(&component.bytes, &c.wit, &component.world)
        .unwrap_or_else(|wrong| panic!("the component disagrees with its world: {wrong:#?}"));
    // `session#read`, `carts#add` and `add-to-cart`: an audit that compared
    // nothing would agree with anything.
    assert_eq!(compared, 3, "the audit compared {compared} functions");
}

/// The WIT with `carts#add` returning a store instead of a cart. At the core
/// level the two are identical — both results are returned through a pointer
/// — which is exactly why the checks below must be component-level.
fn with_add_returning_a_store(wit: &str) -> String {
    let original = "add: func(arg0: capability-session-id, arg1: domain-menu-item-id, arg2: \
                    domain-positive-int) -> result<domain-cart, domain-cart-error>;";
    assert!(wit.contains(original), "the control's anchor moved");
    wit.replace(
        original,
        "add: func(arg0: capability-session-id, arg1: domain-menu-item-id, arg2: \
         domain-positive-int) -> result<domain-store, domain-store-error>;",
    )
    .replace(
        "use pw:types/types.{capability-session-id, domain-cart, domain-cart-error, \
         domain-menu-item-id, domain-positive-int};",
        "use pw:types/types.{capability-session-id, domain-cart, domain-cart-error, \
         domain-menu-item-id, domain-positive-int, domain-store, domain-store-error};",
    )
}

#[test]
fn an_import_whose_component_type_differs_is_refused_though_its_core_type_does_not() {
    let (c, component) = add_to_cart();
    let mutated = with_add_returning_a_store(&c.wit);

    // The core module the mutated world would need is the same core module:
    // same import signature, same pointer result.
    let (resolve, world) = component::world_of(&mutated, &component.world).expect("mutated WIT");
    let add = resolve.worlds[world]
        .imports
        .iter()
        .find_map(|(k, i)| match i {
            wit_parser::WorldItem::Interface { id, .. }
                if resolve.name_world_key(k) == "store:data/carts" =>
            {
                resolve.interfaces[*id].functions.get("add").cloned()
            }
            _ => None,
        })
        .expect("carts#add");
    let (orig_resolve, orig_world) = component::world_of(&c.wit, &component.world).expect("WIT");
    let orig_add = orig_resolve.worlds[orig_world]
        .imports
        .iter()
        .find_map(|(k, i)| match i {
            wit_parser::WorldItem::Interface { id, .. }
                if orig_resolve.name_world_key(k) == "store:data/carts" =>
            {
                orig_resolve.interfaces[*id].functions.get("add").cloned()
            }
            _ => None,
        })
        .expect("carts#add");
    assert_eq!(
        resolve.wasm_signature(wit_parser::abi::AbiVariant::GuestImport, &add),
        orig_resolve.wasm_signature(wit_parser::abi::AbiVariant::GuestImport, &orig_add),
        "the premise: the two imports are indistinguishable at the core level"
    );

    // 1. The adapter refuses to connect them: the command returns what
    //    `carts#add` returns, and a store is not a cart.
    let f = c
        .program
        .functions
        .iter()
        .find(|f| f.export == "add_to_cart")
        .expect("lowered");
    let world_decl = c
        .worlds
        .iter()
        .find(|w| w.declaration == f.def)
        .expect("world");
    match component::build(&mutated, world_decl, f, &c.program, &Default::default()) {
        Encoding::Blocked { why } => assert!(why.contains("another component type"), "{why}"),
        other => panic!("a cart result built from a store import must be refused: {other}"),
    }

    // 2. The audit refuses the REAL artifact against the mutated world, and
    //    names the operation.
    let wrong = component::audit(&component.bytes, &mutated, &component.world)
        .expect_err("the artifact's carts#add is not the mutated world's");
    assert!(
        wrong.iter().any(|w| w.contains("store:data/carts#add")),
        "{wrong:#?}"
    );
}
