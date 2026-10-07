//! **The host's engine enables only what Pleris's components use**
//! (ADR-0244). Wasmtime 48 enables GC, exceptions and the component model's
//! async by default, and the host's engines had each on for nothing; three
//! advisories against 48.0.3 were in them (RUSTSEC-2026-0325 to 0327). A
//! component that uses one is refused when it loads, and when its imports
//! are audited. The controls: a component using none loads, and Wasmtime's
//! default engine loads what the host's refuses.

#![cfg(feature = "engine")]

use pw_host::engine::{engine_config, imports_of};

/// Whether an engine made from `config` loads `wat`, and why not.
fn loads(config: &wasmtime::Config, wat: &str) -> Result<(), String> {
    let engine = wasmtime::Engine::new(config).map_err(|e| e.to_string())?;
    wasmtime::component::Component::new(&engine, wat)
        .map(|_| ())
        .map_err(|e| format!("{e:?}"))
}

/// The engine the host made before ADR-0244.
fn default_config() -> wasmtime::Config {
    let mut config = wasmtime::Config::new();
    config.wasm_component_model(true);
    config
}

const NOTHING: &str = "(component (core module))";
/// A core module declaring a GC struct type.
const GC: &str = "(component (core module (type (struct))))";
/// A core module declaring an exception tag.
const EXCEPTIONS: &str = "(component (core module (tag)))";
/// A component type of the async ABI: a stream.
const ASYNC: &str = "(component (type (stream u8)))";

#[test]
fn a_component_using_none_of_them_loads() {
    assert_eq!(loads(&engine_config(), NOTHING), Ok(()));
}

#[test]
fn a_component_using_gc_is_refused() {
    let refused = loads(&engine_config(), GC).expect_err("refused");
    assert!(refused.contains("without the gc feature"), "{refused}");
    // The default engine had it on.
    assert_eq!(loads(&default_config(), GC), Ok(()));
}

#[test]
fn a_component_using_exceptions_is_refused() {
    let refused = loads(&engine_config(), EXCEPTIONS).expect_err("refused");
    assert!(
        refused.contains("exceptions proposal not enabled"),
        "{refused}"
    );
    assert_eq!(loads(&default_config(), EXCEPTIONS), Ok(()));
}

#[test]
fn a_component_using_the_async_abi_is_refused() {
    let refused = loads(&engine_config(), ASYNC).expect_err("refused");
    assert!(
        refused.contains("requires the component model async feature"),
        "{refused}"
    );
}

#[test]
fn the_import_audit_refuses_them_too() {
    // `imports_of` reads a component with the same engine: a component it
    // cannot load has no import list, and the audit says so.
    assert_eq!(imports_of(NOTHING.as_bytes()), Ok(Vec::new()));
    for wat in [GC, EXCEPTIONS, ASYNC] {
        assert!(imports_of(wat.as_bytes()).is_err(), "{wat}");
    }
}
