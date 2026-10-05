//! **A command computes its events, and hands them to the outbox**
//! (ADR-0208, ADR-0195's ruling 11).
//!
//! `emits Moved(to + 1)` carries what the command was given. The development
//! server computed an event's values from its key's text, `current_session()`
//! alone, and refused this one before the command ran (ADR-0104). The compiled
//! command evaluates each key itself, before its body, and calls the
//! platform's `pw:host/outbox#moved` with what it computed. The host commits
//! the event with the command's writes, or not at all.

use std::collections::BTreeMap;
use std::sync::{Arc, Mutex};

use pw_conformance::{Calls, Runnable, compile, operation, units};
use pw_host::engine::{HostFn, Val};

const PROGRAM: &str = r#"module m

type Spot = Spot { at: Int }

event Moved(to: Int)

event Named(who: String, to: Int)

fn place(to: Int) -> Int !{ database.write<Spot> }
    host "m:data/spots#place"

command move_to(to: Int) -> Int
    requires      SignedIn
    emits         Moved(to + 1), Named(to: to, who: "mover")
{
    place(to)
}
"#;

/// The host: the data layer's write, and the outbox, each call recorded in
/// the order the component made it.
fn host(calls: &Calls) -> BTreeMap<String, HostFn> {
    let mut ops: BTreeMap<String, HostFn> = BTreeMap::new();
    let (name, f) = operation(calls, "m:data/spots#place", |args| args[0].clone());
    ops.insert(name, f);
    for event in ["moved", "named"] {
        let calls = calls.clone();
        let key = format!("pw:host/outbox#{event}");
        let recorded = key.clone();
        let f: HostFn = Arc::new(move |args: &[Val]| {
            calls
                .lock()
                .expect("calls")
                .push((recorded.clone(), args.to_vec()));
            Ok(Vec::new())
        });
        ops.insert(key, f);
    }
    ops
}

#[test]
fn a_command_computes_its_events_from_what_it_was_given() {
    let compiled = compile(&units(&[("m.pw", PROGRAM)]), "m.move_to");
    let calls: Calls = Arc::new(Mutex::new(Vec::new()));
    let out = Runnable::new(compiled)
        .call(&host(&calls), &[Val::S64(41)])
        .expect("runs");
    assert_eq!(out, [Val::S64(41)]);
    // Each event's values in the order it declares them, `to` named after
    // `who` and given first; before its body, on what it was given.
    assert_eq!(
        *calls.lock().expect("calls"),
        [
            ("pw:host/outbox#moved".to_string(), vec![Val::S64(42)]),
            (
                "pw:host/outbox#named".to_string(),
                vec![Val::String("mover".into()), Val::S64(41)]
            ),
            ("m:data/spots#place".to_string(), vec![Val::S64(41)]),
        ]
    );
}

#[test]
fn its_contract_imports_the_outbox_for_each_event_and_requires_it() {
    let compiled = compile(&units(&[("m.pw", PROGRAM)]), "m.move_to");
    let outbox: Vec<(&str, Option<&str>, &str)> = compiled
        .contract
        .imports
        .iter()
        .filter(|i| i.interface == "pw:host/outbox")
        .map(|i| (i.name.as_str(), i.event.as_deref(), i.capability.as_str()))
        .collect();
    assert_eq!(
        outbox,
        [
            ("moved", Some("m.Moved"), "outbox.write"),
            ("named", Some("m.Named"), "outbox.write")
        ]
    );
    assert!(
        compiled
            .contract
            .required_capabilities
            .iter()
            .any(|c| c.name() == "outbox.write"),
        "{:?}",
        compiled.contract.required_capabilities
    );
    // And its world says what each takes.
    assert!(
        compiled.wit.contains("moved: func(arg0: s64);")
            && compiled
                .wit
                .contains("named: func(arg0: string, arg1: s64);"),
        "{}",
        compiled.wit
    );
}

#[test]
fn a_command_that_emits_nothing_imports_no_outbox() {
    let quiet = PROGRAM.replace(
        "    emits         Moved(to + 1), Named(to: to, who: \"mover\")\n",
        "",
    );
    let compiled = compile(&units(&[("m.pw", &quiet)]), "m.move_to");
    assert!(
        !compiled
            .contract
            .imports
            .iter()
            .any(|i| i.interface == "pw:host/outbox"),
        "{:?}",
        compiled.contract.imports
    );
    let calls: Calls = Arc::new(Mutex::new(Vec::new()));
    Runnable::new(compiled)
        .call(&host(&calls), &[Val::S64(1)])
        .expect("runs");
    assert_eq!(calls.lock().expect("calls").len(), 1, "the write alone");
}
