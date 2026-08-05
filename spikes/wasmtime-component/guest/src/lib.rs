//! spike: wasmtime-component — the guest component.
//!
//! Charter §14 Milestone 0 task 10. This is the `public-store-query` world from
//! `../wit/store.wit`: it imports exactly one capability (`stores`) and exports
//! one function.
//!
//! Written in Rust rather than Koka. Charter §14 M8 task 7 explicitly permits
//! this: "Initially write components in Rust if the Koka-to-Wasm Component path
//! is not mature." Milestone 0 does not need the Koka path; it needs to know
//! whether the capability boundary is real.

wit_bindgen::generate!({
    path: "../wit",
    world: "public-store-query",
});

// The imported host capability. Note there is no `use std::fs`, no network, no
// environment access anywhere in this component — and, more importantly, none is
// *reachable*: the component's only imports are what the WIT world declares.
use perfect_web::store::stores;

struct Component;

impl Guest for Component {
    fn lookup(id: String) -> String {
        match stores::read(&id) {
            Some(store) => format!("found:{}:{}", store.id, store.name),
            // Absence is a value, not an exception. Charter §7.1.
            None => format!("missing:{id}"),
        }
    }
}

export!(Component);
