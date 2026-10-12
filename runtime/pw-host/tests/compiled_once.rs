//! **A component is compiled once a process for its bytes** (ADR-XXXX).
//!
//! Until 2026-10-12 a server loading a build compiled each of its components
//! twice, once to read its imports and once to run it, and every server a
//! process loaded did so again: seven of a kiokun test's eight seconds were
//! Wasmtime compiling (W6's finding, 2026-10-09), and the development
//! server's suite, four tests at once, held more than CI's memory bound
//! (ADR-0292). One test, so this process's compiles are its alone.

#![cfg(feature = "engine")]

use pw_host::engine::{Prepared, imports_of};

#[test]
fn a_components_bytes_are_compiled_once_a_process() {
    let a = "(component)";
    let b = "(component (core module))";
    let before = Prepared::compiles();
    let first = Prepared::compile(a.as_bytes()).expect("compiles");
    let again = Prepared::compile(a.as_bytes()).expect("compiles");
    let imports = imports_of(a.as_bytes()).expect("its imports read");
    assert_eq!(
        Prepared::compiles() - before,
        1,
        "one compile for one component's bytes, its imports read from it"
    );
    assert!(imports.is_empty(), "{imports:?}");
    assert_eq!(first.imports(), again.imports());

    // Other bytes, another compile.
    Prepared::compile(b.as_bytes()).expect("compiles");
    assert_eq!(Prepared::compiles() - before, 2);

    // Bytes that do not compile fail each time, and are not compiled again.
    assert!(Prepared::compile(b"not a component").is_err());
    assert!(Prepared::compile(b"not a component").is_err());
    assert!(imports_of(b"not a component").is_err());
    assert_eq!(Prepared::compiles() - before, 3);

    // Eight callers at once with new bytes: one compiles, the rest wait for
    // it and take its result.
    let c = "(component (core module (func)))";
    std::thread::scope(|scope| {
        for _ in 0..8 {
            scope.spawn(|| Prepared::compile(c.as_bytes()).expect("compiles"));
        }
    });
    assert_eq!(Prepared::compiles() - before, 4);
}
