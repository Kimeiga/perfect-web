//! The audit, against real components.
//!
//! Architect ruling, 2026-08-07: *"final artifact audit […] adversarial guests
//! attempting undeclared interfaces"*.
//!
//! Everything in `admission.rs` is decided on data, and it must be — a
//! capability decision that needed a Wasm engine to be checked is a decision
//! nobody runs. But the data has to come from somewhere, and the point of an
//! ARTIFACT audit is that the artifact can disagree with the source. So this
//! file reads the imports out of two real components built by
//! `just spike-wasmtime`, and puts them through the same `admit`.
//!
//! # The adversarial guest is not synthetic
//!
//! It is the ordinary one. `spikes/wasmtime-component/guest` declares one
//! import in its WIT world and demands fifteen, because Rust `std` on
//! `wasm32-wasip2` injects fourteen `wasi:*` interfaces during runtime
//! initialization — measured in `docs/evidence/E0/spike-wasmtime-component.txt`
//! and recorded as that spike's finding F-2.
//!
//! Nobody wrote that guest to attack anything. That is what makes it the right
//! adversary: undeclared authority arrives by *default*, from the toolchain,
//! without a line of application code asking for it, and a host that trusted
//! the source would have granted all fourteen.
//!
//! Requires the `engine` feature and the built components:
//!
//! ```text
//! just spike-wasmtime      # builds the guests
//! just e8-host             # runs this
//! ```

#![cfg(feature = "engine")]

use std::collections::BTreeSet;
use std::path::PathBuf;

use pw_host::*;

fn guest(dir: &str, file: &str) -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../spikes/wasmtime-component")
        .join(dir)
        .join("target/wasm32-wasip2/release")
        .join(file)
}

/// Read a component's imports, failing loudly.
///
/// Never a default of "no imports": that passes every audit, so a failure to
/// read must not be able to look like a clean component.
fn imports(path: PathBuf) -> Vec<String> {
    let bytes = std::fs::read(&path)
        .unwrap_or_else(|e| panic!("{}: {e}\nrun `just spike-wasmtime` first", path.display()));
    pw_host::engine::imports_of(&bytes).unwrap_or_else(|e| panic!("{}: {e}", path.display()))
}

/// The contract the minimal guest's WIT world corresponds to: one capability.
fn contract(imports: Vec<Import>) -> ComponentContract {
    ComponentContract {
        component_id: "spike.Store".into(),
        capability_mapping: pw_host::CAPABILITY_MAPPING,
        abi_schema: "abi".into(),
        required_capabilities: vec![Capability {
            family: "store".into(),
            operation: "read".into(),
            argument: None,
        }],
        allowed_placements: vec!["origin".into()],
        imports,
        exports: vec![Export {
            name: "lookup".into(),
            kind: "query".into(),
        }],
    }
}

fn topology() -> Topology {
    Topology {
        nodes: vec![Node {
            name: "origin-1".into(),
            world: "origin".into(),
            grants: BTreeSet::from(["store.read".to_string()]),
        }],
    }
}

#[test]
fn the_declared_component_imports_exactly_what_its_world_says() {
    let actual = imports(guest("guest-minimal", "spike_wasmtime_guest_minimal.wasm"));
    println!("minimal guest imports: {actual:?}");
    assert!(
        actual
            .iter()
            .all(|i| i.starts_with("perfect-web:store/stores")),
        "the no_std guest declares one interface and demands only that: {actual:?}"
    );

    // And the audit admits it, because the contract allows what it asks for.
    let allowed: Vec<Import> = actual
        .iter()
        .map(|i| {
            let (interface, name) = i.split_once('#').unwrap_or((i.as_str(), "use"));
            Import {
                interface: interface.to_string(),
                name: name.to_string(),
                capability: "store.read".into(),
                kind: ImportKind::HostCapability,
            }
        })
        .collect();
    let c = contract(allowed);
    let a = admit(&c, &topology(), "origin-1", &actual);
    assert!(a.is_admitted(), "refused: {:?}", a.refusals());
}

#[test]
fn the_std_component_is_refused_for_authority_nobody_asked_for() {
    // The adversarial case, and it is the ordinary build. Fourteen `wasi:*`
    // interfaces the WIT world never mentions, injected by std's runtime
    // initialization.
    let actual = imports(guest("guest", "spike_wasmtime_guest.wasm"));
    println!("std guest imports {} interfaces", actual.len());
    assert!(
        actual.len() > 1,
        "if std stopped injecting imports this test measures nothing: {actual:?}"
    );

    // The allowed set is the OTHER BUILD OF THE SAME WORLD.
    //
    // Both guests declare `perfect-web:store/stores` and nothing else, so what
    // the no_std build imports is exactly what this world legitimately needs —
    // including the resource types an interface brings with it, which a
    // hand-written allow-list would have got wrong and did.
    //
    // Same world, same contract, two builds. Everything the std build asks for
    // beyond the other one is authority the world never mentioned.
    let declared = imports(guest("guest-minimal", "spike_wasmtime_guest_minimal.wasm"));
    let c = contract(
        declared
            .iter()
            .map(|i| {
                let (interface, name) = i.split_once('#').unwrap_or((i.as_str(), "use"));
                Import {
                    interface: interface.to_string(),
                    name: name.to_string(),
                    capability: "store.read".into(),
                    kind: ImportKind::HostCapability,
                }
            })
            .collect(),
    );
    let a = admit(&c, &topology(), "origin-1", &actual);

    let Some(Refusal::UndeclaredImport { imports }) = a.refusals().first() else {
        panic!("expected an import refusal, got {:?}", a.refusals());
    };
    println!("undeclared: {imports:?}");
    assert!(
        imports.iter().any(|i| i.starts_with("wasi:")),
        "the refusal names the ambient WASI interfaces"
    );
    assert!(
        !imports.iter().any(|i| i.starts_with("perfect-web:store")),
        "and does not name the interface the world DID declare"
    );
    assert!(
        imports.len() >= 14,
        "every undeclared interface is named, not just the first: {imports:?}"
    );
}

#[test]
fn a_component_that_cannot_be_read_is_not_treated_as_importing_nothing() {
    // The failure mode that would make every test above vacuous. `imports_of`
    // must error rather than return an empty list, because an empty list
    // passes every audit.
    let err = pw_host::engine::imports_of(b"not a wasm component")
        .expect_err("garbage is not a component");
    assert!(!err.is_empty());
}
