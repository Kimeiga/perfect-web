//! **A list of records, from a browser** (ADR-0172).
//!
//! A handler sends a record or a list of what it can send, as the page
//! showed it, each field by its Pleris name. The host reads each by the
//! parameter types the artifact declares: a record field by field, a list
//! element by element. The store's `add_to_cart` takes a record and no list,
//! so the list is read here, through the compiled `domain.subtotal`, whose
//! `Cart` holds its `lines`.
//!
//! The component is `docs/evidence/E10/domain.subtotal.wasm`, held to the
//! compiler's output by `pw-core`'s `evidence_is_current`.

#![cfg(feature = "engine")]

use std::collections::{BTreeMap, BTreeSet};

use pw_host::*;
use wasmtime::component::Val;

fn root() -> std::path::PathBuf {
    std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../..")
}

const ID: &str = "domain.subtotal";

fn component() -> Vec<u8> {
    let path = root().join(format!("docs/evidence/E10/{ID}.wasm"));
    std::fs::read(&path)
        .unwrap_or_else(|e| panic!("{}: {e} — run `just e10-component`", path.display()))
}

fn contract() -> ComponentContract {
    let contracts = root().join("docs/evidence/E8/component-contracts.json");
    ComponentContract::from_json(&std::fs::read_to_string(contracts).expect("contracts"))
        .expect("the compiler's contracts parse")
        .into_iter()
        .find(|c| c.component_id == ID)
        .expect("subtotal's contract")
}

/// A line as a browser writes it.
fn line(item: &str, quantity: serde_json::Value, cents: i64) -> serde_json::Value {
    serde_json::json!({
        "item_id": item,
        "name": "Cortado",
        "quantity": quantity,
        "unit_price": { "minor_units": cents },
    })
}

/// The line, as the component's record names its fields.
fn val(item: &str, quantity: i64, cents: i64) -> Val {
    Val::Record(vec![
        ("item-id".into(), Val::String(item.into())),
        ("name".into(), Val::String("Cortado".into())),
        ("quantity".into(), Val::S64(quantity)),
        (
            "unit-price".into(),
            Val::Record(vec![("minor-units".into(), Val::S64(cents))]),
        ),
    ])
}

#[test]
fn a_list_of_records_is_read_element_by_element_and_runs() {
    let c = contract();
    let bytes = component();
    let prepared = engine::Prepared::compile(&bytes).expect("compiles");
    let e = c.exports[0]
        .component
        .clone()
        .expect("the contract says where its export is");
    let at = [e.interface.as_str(), e.function.as_str()];

    let sent = serde_json::json!({
        "lines": [line("cortado", 2.into(), 425), line("espresso", 1.into(), 350)],
    });
    let args = prepared.arguments(&at, &[sent]).expect("well-typed");
    assert_eq!(
        args,
        vec![Val::Record(vec![(
            "lines".into(),
            Val::List(vec![val("cortado", 2, 425), val("espresso", 1, 350)]),
        )])]
    );

    // And the component computes from what was read: 2 × $4.25 + $3.50.
    let actual = engine::imports_of(&bytes).expect("imports");
    let topology = Topology {
        nodes: vec![Node {
            name: "origin-1".into(),
            world: "origin".into(),
            grants: BTreeSet::new(),
        }],
    };
    let admission = admit(&c, &topology, "origin-1", &actual);
    let granted = Granted::from(&admission, &BTreeMap::new()).expect("admitted");
    let out = engine::call_within(
        &bytes,
        &c,
        &granted,
        &Limits {
            fuel: Some(10_000_000),
            memory_bytes: Some(16 * 1024 * 1024),
            table_elements: Some(1_000),
        },
        &BTreeMap::new(),
        &at,
        &args,
    )
    .expect("runs");
    assert_eq!(
        out,
        vec![Val::Record(vec![("minor-units".into(), Val::S64(1200))])]
    );

    // An empty list is a list.
    assert_eq!(
        prepared
            .arguments(&at, &[serde_json::json!({ "lines": [] })])
            .expect("well-typed"),
        vec![Val::Record(vec![("lines".into(), Val::List(Vec::new()))])]
    );

    // Each refusal says where it is.
    for (sent, why) in [
        (
            serde_json::json!({ "lines": {} }),
            ".lines: expected an array",
        ),
        (
            serde_json::json!({ "lines": [line("cortado", 2.into(), 425), { "item_id": "x" }] }),
            ".lines[1]: the record has no field `name`",
        ),
        (
            serde_json::json!({ "lines": [line("cortado", serde_json::json!(2.5), 425)] }),
            ".lines[0].quantity: expected an integer",
        ),
    ] {
        let err = prepared
            .arguments(&at, std::slice::from_ref(&sent))
            .expect_err("refused");
        println!("{sent}: {err}");
        assert!(err.contains(why), "{sent}: {err}");
    }
}
