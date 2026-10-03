//! **A price as a person reads it** (ADR-0169): the compiled
//! `domain.display`, which a menu row reads as `item.price.display`.
//!
//! en-US writes US dollars `$3.50`, `$1,234.50`, and `-$3.50` for an amount
//! owed back, as `Intl.NumberFormat("en-US", { style: "currency", currency:
//! "USD" })` does. The amount is an `Int` of cents, so each one is written
//! exactly, the least included. The platform's formatter takes a float, and
//! wrote that one `-$92,233,720,368,547,760.00` (Node, 2026-10-03).
//!
//! The component is `docs/evidence/E10/domain.display.wasm`, held to the
//! compiler's output by `pw-core`'s `evidence_is_current`.

#![cfg(feature = "engine")]

use std::collections::{BTreeMap, BTreeSet};

use pw_host::*;
use wasmtime::component::Val;

fn root() -> std::path::PathBuf {
    std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../..")
}

/// `display(Money { minor_units: cents })`, run by the engine as a host runs
/// it for a page.
fn display(cents: i64) -> Result<Vec<Val>, String> {
    let id = "domain.display";
    let path = root().join(format!("docs/evidence/E10/{id}.wasm"));
    let bytes = std::fs::read(&path)
        .unwrap_or_else(|e| panic!("{}: {e} — run `just e10-component`", path.display()));
    let contracts = root().join("docs/evidence/E8/component-contracts.json");
    let c = ComponentContract::from_json(&std::fs::read_to_string(contracts).expect("contracts"))
        .expect("the compiler's contracts parse")
        .into_iter()
        .find(|c| c.component_id == id)
        .expect("display's contract");
    let actual = engine::imports_of(&bytes)?;
    let topology = Topology {
        nodes: vec![Node {
            name: "origin-1".into(),
            world: "origin".into(),
            grants: BTreeSet::new(),
        }],
    };
    let admission = admit(&c, &topology, "origin-1", &actual);
    let granted = Granted::from(&admission, &BTreeMap::new()).expect("admitted");
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
        &BTreeMap::new(),
        &[&e.interface, &e.function],
        &[Val::Record(vec![("minor-units".into(), Val::S64(cents))])],
    )
}

#[test]
fn a_price_is_written_as_en_us_writes_dollars() {
    for (cents, shown) in [
        (0, "$0.00"),
        (5, "$0.05"),
        (50, "$0.50"),
        (100, "$1.00"),
        (350, "$3.50"),
        (99_999, "$999.99"),
        (100_000, "$1,000.00"),
        (123_450, "$1,234.50"),
        (100_005_000, "$1,000,050.00"),
        (100_000_000, "$1,000,000.00"),
    ] {
        assert_eq!(
            display(cents),
            Ok(vec![Val::String(shown.into())]),
            "{cents} cents"
        );
    }
}

#[test]
fn an_amount_owed_back_is_written_with_its_sign_first() {
    for (cents, shown) in [
        (-5, "-$0.05"),
        (-50, "-$0.50"),
        (-350, "-$3.50"),
        (-123_450, "-$1,234.50"),
    ] {
        assert_eq!(
            display(cents),
            Ok(vec![Val::String(shown.into())]),
            "{cents} cents"
        );
    }
}

#[test]
fn every_amount_is_exact_the_least_and_the_greatest_included() {
    for (cents, shown) in [
        (i64::MAX, "$92,233,720,368,547,758.07"),
        (i64::MIN, "-$92,233,720,368,547,758.08"),
    ] {
        assert_eq!(
            display(cents),
            Ok(vec![Val::String(shown.into())]),
            "{cents} cents"
        );
    }
}
