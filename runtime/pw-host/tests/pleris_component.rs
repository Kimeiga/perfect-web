//! **E10-I step 8: a Pleris-compiled command, running in the E8 host.**
//!
//! The component is `docs/evidence/E10/store.page.add_to_cart.wasm`, produced
//! by `just e10-component` from `examples/store/app.pw` and held to the
//! compiler's current output by `pw-core`'s `evidence_is_current`. The contract
//! is the compiler's, from `docs/evidence/E8/component-contracts.json`. The
//! host reads both as data — it links no compiler, which is ADR-0020's
//! boundary: the compiler decides what authority the code needs, the host
//! decides whether it exists.
//!
//! What runs is the compiled body of
//!
//! ```text
//! command add_to_cart(item: MenuItemId, quantity: PositiveInt) -> Result<Cart, CartError>
//! {
//!     Carts.add(current_session(), item, quantity)
//! }
//! ```
//!
//! and nothing else: no Rust closure computes its result.

#![cfg(feature = "engine")]

use std::collections::{BTreeMap, BTreeSet};
use std::sync::{Arc, Mutex};

use pw_host::engine::HostFn;
use pw_host::*;
use wasmtime::component::Val;

fn root() -> std::path::PathBuf {
    std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../..")
}

fn component() -> Vec<u8> {
    let path = root().join("docs/evidence/E10/store.page.add_to_cart.wasm");
    std::fs::read(&path)
        .unwrap_or_else(|e| panic!("{}: {e} — run `just e10-component`", path.display()))
}

fn contract() -> ComponentContract {
    let path = root().join("docs/evidence/E8/component-contracts.json");
    let text = std::fs::read_to_string(&path)
        .unwrap_or_else(|e| panic!("{}: {e} — run `just e8-contracts`", path.display()));
    ComponentContract::from_json(&text)
        .expect("the compiler's contracts parse")
        .into_iter()
        .find(|c| c.component_id == "store.page.add_to_cart")
        .expect("the contract for add_to_cart")
}

fn origin(grants: &[&str]) -> Topology {
    Topology {
        nodes: vec![Node {
            name: "origin-1".into(),
            world: "origin".into(),
            grants: grants
                .iter()
                .map(|g| g.to_string())
                .collect::<BTreeSet<_>>(),
        }],
    }
}

/// The contract's export, where the compiler says it is in the component.
fn export(c: &ComponentContract) -> [String; 2] {
    let e = c.exports[0]
        .component
        .clone()
        .expect("the contract says where its export is in the component");
    [e.interface, e.function]
}

/// A cart the data layer returns: one line, as the WIT's record fields name
/// them.
fn cart(item: &str, quantity: i64) -> Val {
    Val::Record(vec![(
        "lines".into(),
        Val::List(vec![Val::Record(vec![
            ("item-id".into(), Val::String(item.into())),
            ("quantity".into(), Val::S64(quantity)),
            (
                "unit-price".into(),
                Val::Record(vec![("minor-units".into(), Val::S64(450))]),
            ),
        ])]),
    )])
}

/// What the host saw of each call, so a test can assert the COMPONENT made it.
type Calls = Arc<Mutex<Vec<(String, Vec<Val>)>>>;

/// The deployment's operations: the platform's session, and a data layer.
fn host(calls: &Calls, add_result: Val) -> BTreeMap<String, HostFn> {
    let session_calls = calls.clone();
    let read: HostFn = Arc::new(move |args: &[Val]| {
        session_calls
            .lock()
            .unwrap()
            .push(("pw:host/session#read".into(), args.to_vec()));
        Ok(vec![Val::String("session-7".into())])
    });
    let add_calls = calls.clone();
    let add: HostFn = Arc::new(move |args: &[Val]| {
        add_calls
            .lock()
            .unwrap()
            .push(("store:data/carts#add".into(), args.to_vec()));
        Ok(vec![add_result.clone()])
    });
    BTreeMap::from([
        ("pw:host/session#read".to_string(), read),
        ("store:data/carts#add".to_string(), add),
    ])
}

fn admitted(c: &ComponentContract, bytes: &[u8], grants: &[&str]) -> Result<Granted, String> {
    let actual = engine::imports_of(bytes)?;
    let admission = admit(c, &origin(grants), "origin-1", &actual);
    Granted::from(&admission, &BTreeMap::new()).ok_or_else(|| format!("{admission:?}"))
}

const BOTH: [&str; 2] = ["database.write<Carts>", "session.read"];

fn limits() -> Limits {
    Limits {
        fuel: Some(10_000_000),
        memory_bytes: Some(16 * 1024 * 1024),
        table_elements: Some(1_000),
    }
}

#[test]
fn the_artifact_imports_exactly_what_its_contract_allows() {
    let actual = engine::imports_of(&component()).expect("the component reads");
    println!("compiled add_to_cart imports: {actual:?}");
    assert_eq!(actual, ["pw:host/session#read", "store:data/carts#add"]);
    let c = contract();
    assert!(
        matches!(
            admit(&c, &origin(&BOTH), "origin-1", &actual),
            Admission::Admit { .. }
        ),
        "the host admits the compiled artifact against the compiler's contract"
    );
}

#[test]
fn the_compiled_command_runs_through_the_host() {
    let c = contract();
    let bytes = component();
    let granted = admitted(&c, &bytes, &BOTH).expect("admitted");
    let calls: Calls = Arc::default();
    let returned = Val::Result(Ok(Some(Box::new(cart("cortado", 2)))));
    let [interface, function] = export(&c);

    let out = engine::call_within(
        &bytes,
        &c,
        &granted,
        &limits(),
        &host(&calls, returned.clone()),
        &[&interface, &function],
        &[Val::String("cortado".into()), Val::S64(2)],
    )
    .unwrap_or_else(|e| panic!("the compiled command must run: {e}"));

    let seen = calls.lock().unwrap().clone();
    println!("the host saw: {seen:?}");
    println!("the command returned: {out:?}");

    // The COMPONENT read the session, then called the data layer with it and
    // with its own arguments, in that order.
    assert_eq!(seen.len(), 2, "{seen:?}");
    assert_eq!(seen[0], ("pw:host/session#read".to_string(), vec![]));
    assert_eq!(
        seen[1],
        (
            "store:data/carts#add".to_string(),
            vec![
                Val::String("session-7".into()),
                Val::String("cortado".into()),
                Val::S64(2),
            ]
        ),
        "the session came from the host; the item and quantity from the caller"
    );
    // And the data layer's answer is the command's answer, lifted back out of
    // the component's memory by the engine.
    assert_eq!(out, vec![returned]);
}

#[test]
fn a_data_layer_failure_is_the_commands_failure() {
    let c = contract();
    let bytes = component();
    let granted = admitted(&c, &bytes, &BOTH).expect("admitted");
    let calls: Calls = Arc::default();
    let failed = Val::Result(Err(Some(Box::new(Val::Variant(
        "cart-expired".into(),
        None,
    )))));
    let [interface, function] = export(&c);
    let out = engine::call_within(
        &bytes,
        &c,
        &granted,
        &limits(),
        &host(&calls, failed.clone()),
        &[&interface, &function],
        &[Val::String("cortado".into()), Val::S64(1)],
    )
    .expect("the call completes; the command's result is a failure");
    println!("a failing data layer: {out:?}");
    assert_eq!(out, vec![failed]);
}

#[test]
fn a_node_without_the_write_capability_does_not_admit_it() {
    let c = contract();
    let bytes = component();
    let err = admitted(&c, &bytes, &["session.read"]).expect_err("refused");
    println!("refused without database.write<Carts>: {err}");
    assert!(err.contains("database.write<Carts>"), "{err}");
}

#[test]
fn an_ungranted_operation_is_refused_by_the_engine() {
    // The contract without the write: the host links only what is granted, so
    // `carts#add` has no definition and the ENGINE refuses to instantiate —
    // in its own words, naming the interface.
    let mut c = contract();
    c.required_capabilities
        .retain(|cap| cap.name() != "database.write<Carts>");
    let bytes = component();
    let admission = admit(&c, &origin(&BOTH), "origin-1", &[]);
    let granted = Granted::from(&admission, &BTreeMap::new()).expect("admitted as edited");
    let calls: Calls = Arc::default();
    let [interface, function] = export(&c);
    let err = engine::call_within(
        &bytes,
        &c,
        &granted,
        &limits(),
        &host(&calls, Val::Bool(false)),
        &[&interface, &function],
        &[Val::String("cortado".into()), Val::S64(1)],
    )
    .expect_err("the write is not linked");
    println!("ungranted write refused by the engine: {err}");
    assert!(err.contains("store:data/carts"), "{err}");
    assert!(calls.lock().unwrap().is_empty(), "nothing ran");
}

#[test]
fn a_granted_operation_the_host_does_not_implement_is_refused() {
    let c = contract();
    let bytes = component();
    let granted = admitted(&c, &bytes, &BOTH).expect("admitted");
    let calls: Calls = Arc::default();
    let mut ops = host(&calls, Val::Bool(false));
    ops.remove("store:data/carts#add");
    let [interface, function] = export(&c);
    let err = engine::call_within(
        &bytes,
        &c,
        &granted,
        &limits(),
        &ops,
        &[&interface, &function],
        &[Val::String("cortado".into()), Val::S64(1)],
    )
    .expect_err("granted is not implemented");
    assert!(err.contains("implements nothing"), "{err}");
}

#[test]
fn the_instance_runs_within_its_fuel() {
    let c = contract();
    let bytes = component();
    let granted = admitted(&c, &bytes, &BOTH).expect("admitted");
    let calls: Calls = Arc::default();
    let [interface, function] = export(&c);
    let starved = Limits {
        fuel: Some(1),
        memory_bytes: None,
        table_elements: None,
    };
    let err = engine::call_within(
        &bytes,
        &c,
        &granted,
        &starved,
        &host(&calls, Val::Result(Ok(Some(Box::new(cart("x", 1)))))),
        &[&interface, &function],
        &[Val::String("cortado".into()), Val::S64(1)],
    )
    .expect_err("one unit of fuel runs out");
    println!("starved of fuel: {err}");
    assert!(err.to_lowercase().contains("fuel"), "{err}");
}

// --- E10: arguments that arrive as JSON, from a compiled handler -----------

/// **A browser's arguments are typed by the component's own parameters.**
///
/// The compiled handler for `add_to_cart(item.id, PositiveInt(1))` runs in the
/// user's browser and sends `["cortado", 1]`. The host converts it by the
/// parameter types the ARTIFACT declares, `(string, s64)`, and refuses anything
/// else before the component runs.
#[test]
fn json_arguments_are_typed_by_the_export_they_are_for() {
    let c = contract();
    let prepared = engine::Prepared::compile(&component()).expect("compiles");
    let [interface, function] = export(&c);
    let at = [interface.as_str(), function.as_str()];

    let args = prepared
        .arguments(&at, &[serde_json::json!("cortado"), serde_json::json!(2)])
        .expect("well-typed");
    assert_eq!(args, vec![Val::String("cortado".into()), Val::S64(2)]);

    for (json, why) in [
        (
            vec![serde_json::json!("cortado")],
            "takes 2 argument(s); 1 were sent",
        ),
        (
            vec![
                serde_json::json!("a"),
                serde_json::json!(1),
                serde_json::json!(1),
            ],
            "takes 2 argument(s); 3 were sent",
        ),
        (
            vec![serde_json::json!(7), serde_json::json!(1)],
            "expected a string",
        ),
        (
            vec![serde_json::json!("a"), serde_json::json!(1.5)],
            "expected an integer",
        ),
        (
            vec![serde_json::json!("a"), serde_json::json!(true)],
            "expected an integer",
        ),
        (
            vec![serde_json::json!("a"), serde_json::json!(null)],
            "expected an integer",
        ),
        (
            vec![serde_json::json!("a"), serde_json::json!(u64::MAX)],
            "is outside",
        ),
    ] {
        let err = prepared.arguments(&at, &json).expect_err("refused");
        println!("{json:?}: {err}");
        assert!(err.contains(why), "{json:?}: {err}");
    }

    // The extremes of `s64` are the type's, not JavaScript's: the host takes
    // the whole range the parameter declares.
    let edge = prepared
        .arguments(&at, &[serde_json::json!("a"), serde_json::json!(i64::MIN)])
        .expect("in range");
    assert_eq!(edge[1], Val::S64(i64::MIN));
}

#[test]
fn arguments_for_an_export_the_component_does_not_have_are_refused() {
    let c = contract();
    let prepared = engine::Prepared::compile(&component()).expect("compiles");
    let [interface, _] = export(&c);
    let err = prepared
        .arguments(&[&interface, "remove-from-cart"], &[])
        .expect_err("no such function");
    assert!(err.contains("exports no function"), "{err}");
    let err = prepared
        .arguments(&["pw:app/nothing", "add-to-cart"], &[])
        .expect_err("no such instance");
    assert!(err.contains("exports no instance"), "{err}");
}
