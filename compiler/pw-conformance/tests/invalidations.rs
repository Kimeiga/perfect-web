//! **A command computes the entries it invalidates** (ADR-0209, ADR-0195's
//! ruling 11).
//!
//! `invalidates Spot(to + 1)` names an entry by what the command was given.
//! The development server read a key's text, `current_session()`, and dropped
//! every entry of the query for anything else (ADR-0127). The compiled
//! command evaluates each key itself, before its body, in the order its
//! clauses are written, and calls the platform's
//! `pw:host/invalidations#m-spot` with what it computed. The host drops that
//! entry once the command's writes commit.

use std::collections::BTreeMap;
use std::sync::{Arc, Mutex};

use pw_conformance::{Calls, Runnable, compile, operation, units};
use pw_host::engine::{HostFn, Val};

const PROGRAM: &str = r#"module m

type Spot = Spot { at: Int }

event Moved(to: Int)

fn read(at: Int) -> Int !{ database.read<Spot> }
    host "m:data/spots#read"

fn place(to: Int) -> Int !{ database.write<Spot> }
    host "m:data/spots#place"

public query Spot(at: Int) -> Int
    freshness     30.seconds
    consistency   snapshot
    cache         shared
    key           at
    concurrency   one_per_key
    on_key_change cancel
    timeout       2.seconds
{
    read(at)
}

command move_to(to: Int) -> Int
    requires      SignedIn
    invalidates   Spot(to + 1)
    emits         Moved(to)
{
    place(to)
}
"#;

/// The host: the data layer's write, the invalidations and the outbox, each
/// call recorded in the order the component made it.
fn host(calls: &Calls) -> BTreeMap<String, HostFn> {
    let mut ops: BTreeMap<String, HostFn> = BTreeMap::new();
    let (name, f) = operation(calls, "m:data/spots#place", |args| args[0].clone());
    ops.insert(name, f);
    for key in ["pw:host/invalidations#m-spot", "pw:host/outbox#moved"] {
        let calls = calls.clone();
        let recorded = key.to_string();
        let f: HostFn = Arc::new(move |args: &[Val]| {
            calls
                .lock()
                .expect("calls")
                .push((recorded.clone(), args.to_vec()));
            Ok(Vec::new())
        });
        ops.insert(key.to_string(), f);
    }
    ops
}

#[test]
fn a_command_computes_the_entries_it_invalidates() {
    let compiled = compile(&units(&[("m.pw", PROGRAM)]), "m.move_to");
    let calls: Calls = Arc::new(Mutex::new(Vec::new()));
    let out = Runnable::new(compiled)
        .call(&host(&calls), &[Val::S64(41)])
        .expect("runs");
    assert_eq!(out, [Val::S64(41)]);
    // The entry, then the event, as written; both before the body.
    assert_eq!(
        *calls.lock().expect("calls"),
        [
            (
                "pw:host/invalidations#m-spot".to_string(),
                vec![Val::S64(42)]
            ),
            ("pw:host/outbox#moved".to_string(), vec![Val::S64(41)]),
            ("m:data/spots#place".to_string(), vec![Val::S64(41)]),
        ]
    );
}

#[test]
fn its_contract_imports_the_invalidations_under_the_outbox() {
    let compiled = compile(&units(&[("m.pw", PROGRAM)]), "m.move_to");
    let entries: Vec<(&str, Option<&str>, &str)> = compiled
        .contract
        .imports
        .iter()
        .filter(|i| i.interface == "pw:host/invalidations")
        .map(|i| {
            (
                i.name.as_str(),
                i.invalidates.as_deref(),
                i.capability.as_str(),
            )
        })
        .collect();
    assert_eq!(entries, [("m-spot", Some("m.Spot"), "outbox.write")]);
    assert!(
        compiled.wit.contains("m-spot: func(arg0: s64);"),
        "{}",
        compiled.wit
    );
}
