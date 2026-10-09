//! **A host's answer is read through the type the component declares**
//! (ADR-0166).
//!
//! A data layer's row can hold more than a program asks for. The canonical
//! store's `Store` and `MenuItem` declare a `description` since ADR-0166, and
//! the benchmark's frozen copy of the store does not; one development server
//! answers both. Until 2026-10-03 a host had to answer each program's records
//! field for field, in their order, or the engine refused the call.
//!
//! The components are the compiled `Store` and `Menu` queries,
//! `docs/evidence/E10/store.page.{Store,Menu}.wasm`, held to the compiler's
//! output by `pw-core`'s `evidence_is_current`.

#![cfg(feature = "engine")]

use std::collections::{BTreeMap, BTreeSet};
use std::sync::Arc;

use pw_host::engine::HostFn;
use pw_host::*;
use wasmtime::component::Val;

fn root() -> std::path::PathBuf {
    std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../..")
}

fn component(id: &str) -> Vec<u8> {
    let path = root().join(format!("docs/evidence/E10/{id}.wasm"));
    std::fs::read(&path)
        .unwrap_or_else(|e| panic!("{}: {e} — run `just e10-component`", path.display()))
}

fn contract(id: &str) -> ComponentContract {
    let path = root().join("docs/evidence/E8/component-contracts.json");
    let text = std::fs::read_to_string(&path)
        .unwrap_or_else(|e| panic!("{}: {e} — run `just e8-contracts`", path.display()));
    ComponentContract::from_json(&text)
        .expect("the compiler's contracts parse")
        .into_iter()
        .find(|c| c.component_id == id)
        .unwrap_or_else(|| panic!("the contract for {id}"))
}

/// The query `id`, granted `grant`, run for store 47 with its one host
/// operation `op` answering `answer`.
fn run(id: &str, grant: &str, op: &str, answer: Val) -> Result<Vec<Val>, String> {
    let c = contract(id);
    let bytes = component(id);
    let actual = engine::imports_of(&bytes)?;
    let topology = Topology {
        nodes: vec![Node {
            name: "origin-1".into(),
            world: "origin".into(),
            grants: BTreeSet::from([grant.to_string()]),
        }],
    };
    let admission = admit(&c, &topology, "origin-1", &actual);
    let granted = Granted::from(&admission, &BTreeMap::new()).expect("admitted");
    let host: HostFn = Arc::new(move |_: &[Val]| Ok(vec![answer.clone()]));
    let e = c.exports[0]
        .component
        .clone()
        .expect("the contract says where its export is");
    engine::call_within(
        &bytes,
        &c,
        &granted,
        &Limits {
            fuel: Some(10_000_000),
            memory_bytes: Some(16 * 1024 * 1024),
            table_elements: Some(1_000),
        },
        &BTreeMap::from([(op.to_string(), host)]),
        &[&e.interface, &e.function],
        &[Val::String("47".into())],
    )
}

fn hours(extra: &[(&str, Val)]) -> Val {
    let mut fields = vec![
        ("opens-minute".to_string(), Val::S64(420)),
        ("closes-minute".to_string(), Val::S64(1140)),
    ];
    fields.extend(extra.iter().map(|(n, v)| (n.to_string(), v.clone())));
    Val::Record(fields)
}

/// A store's row, as the program's `Store` declares it.
fn declared() -> Vec<(String, Val)> {
    vec![
        ("id".into(), Val::String("47".into())),
        ("name".into(), Val::String("Blue Bottle".into())),
        (
            "description".into(),
            Val::String("Small-batch coffee.".into()),
        ),
        ("hours".into(), hours(&[])),
    ]
}

fn ok(v: Val) -> Val {
    Val::Result(Ok(Some(Box::new(v))))
}

fn the_store(answer: Val) -> Result<Vec<Val>, String> {
    run(
        "store.page.Store",
        "database.read<Stores>",
        "store:data/stores#get",
        answer,
    )
}

#[test]
fn an_answer_as_the_type_declares_it_is_passed_as_it_is() {
    let answer = ok(Val::Record(declared()));
    assert_eq!(the_store(answer.clone()), Ok(vec![answer]));
}

#[test]
fn a_field_the_type_does_not_declare_is_not_passed_in() {
    // A rating at the top, and a time zone in the hours: the program reads
    // neither.
    let mut row = declared();
    row.push(("rating".into(), Val::S64(5)));
    row[3].1 = hours(&[("time-zone", Val::String("America/Los_Angeles".into()))]);
    assert_eq!(
        the_store(ok(Val::Record(row))),
        Ok(vec![ok(Val::Record(declared()))])
    );
}

#[test]
fn a_field_in_another_order_is_read_by_its_name() {
    let mut row = declared();
    row.reverse();
    assert_eq!(
        the_store(ok(Val::Record(row))),
        Ok(vec![ok(Val::Record(declared()))])
    );
}

#[test]
fn a_field_the_type_declares_and_the_answer_lacks_is_refused() {
    let row: Vec<(String, Val)> = declared()
        .into_iter()
        .filter(|(n, _)| n != "description")
        .collect();
    let refused = the_store(ok(Val::Record(row))).expect_err("refused");
    assert!(
        refused.contains("`store:data/stores#get`: the host's record has no field `description`"),
        "{refused}"
    );
}

#[test]
fn a_field_written_as_the_program_names_it_is_read() {
    // ADR-XXXX: a data layer writes `opens_minute`, as the program's record
    // names it, and the world's `opens-minute` is read from it. kiokun's
    // layer wrote `chinese_char`, and its query trapped.
    let mut row = declared();
    row[3].1 = Val::Record(vec![
        ("opens_minute".into(), Val::S64(420)),
        ("closes_minute".into(), Val::S64(1140)),
    ]);
    assert_eq!(
        the_store(ok(Val::Record(row))),
        Ok(vec![ok(Val::Record(declared()))])
    );
}

#[test]
fn a_field_under_neither_name_is_refused_by_both() {
    let mut row = declared();
    row[3].1 = Val::Record(vec![("closes_minute".into(), Val::S64(1140))]);
    let refused = the_store(ok(Val::Record(row))).expect_err("refused");
    assert!(
        refused.contains("the host's record has no field `opens-minute`, nor `opens_minute`"),
        "{refused}"
    );
}

#[test]
fn a_list_s_rows_and_a_declared_error_are_read_through_their_types() {
    let coffee = || {
        Val::Record(vec![
            ("id".into(), Val::String("coffee".into())),
            ("name".into(), Val::String("Coffee".into())),
        ])
    };
    let item = |id: &str, extra: bool| {
        let mut fields = vec![
            ("id".to_string(), Val::String(id.into())),
            // Its store and its category (ADR-0181).
            ("store-id".to_string(), Val::String("47".into())),
            ("name".to_string(), Val::String(id.to_uppercase())),
            (
                "description".to_string(),
                Val::String(format!("{id}, described")),
            ),
            // A record in the row: read through its own type (ADR-0169).
            (
                "price".to_string(),
                Val::Record(vec![("minor-units".into(), Val::S64(450))]),
            ),
            // Whether it can be ordered (ADR-0178).
            ("available".to_string(), Val::Bool(true)),
            ("category".to_string(), coffee()),
        ];
        if extra {
            fields.push(("calories".into(), Val::S64(5)));
        }
        Val::Record(fields)
    };
    // The menu, grouped by category (ADR-0181): a list of sections, each a
    // record holding a list, each row read through its type in turn.
    let menu = |extra: bool| {
        let mut section = vec![
            ("category".to_string(), coffee()),
            (
                "items".to_string(),
                Val::List(vec![item("espresso", extra), item("cortado", extra)]),
            ),
        ];
        if extra {
            section.push(("position".into(), Val::S64(1)));
        }
        ok(Val::List(vec![Val::Record(section)]))
    };
    let read = |answer: Val| {
        run(
            "store.page.Menu",
            "database.read<Menus>",
            "store:data/menus#sections",
            answer,
        )
    };
    assert_eq!(read(menu(true)), Ok(vec![menu(false)]));
    // And the error the query declares, whose case carries nothing.
    let missing = Val::Result(Err(Some(Box::new(Val::Variant("not-found".into(), None)))));
    assert_eq!(read(missing.clone()), Ok(vec![missing]));
}
