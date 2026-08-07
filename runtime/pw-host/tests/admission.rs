//! E8 — no ambient authority, and the three checks that make it true.
//!
//! Architect ruling, 2026-08-07, on what E8 must demonstrate:
//!
//! > Wasmtime capability host, no ambient authority, typed capabilities
//! > (`database.read<Stores>`), placement ≠ capability distinct, declarative
//! > host topology replacing `worlds_for`, handles not raw secrets, final
//! > artifact audit, adversarial guests attempting undeclared interfaces.
//!
//! Everything here is decided on DATA, and that is deliberate: a capability
//! decision that needed a Wasm engine to be checked would be a decision nobody
//! could test cheaply, and the ones that matter are exactly the ones nobody
//! runs. `tests/artifact.rs` runs the same decision against real components.

use std::collections::{BTreeMap, BTreeSet};

use pw_host::*;

fn caps(names: &[&str]) -> Vec<Capability> {
    names
        .iter()
        .map(|n| {
            let (head, argument) = match n.split_once('<') {
                Some((h, rest)) => (h, Some(rest.trim_end_matches('>').to_string())),
                None => (*n, None),
            };
            let (family, operation) = match head.split_once('.') {
                Some((f, o)) => (f.to_string(), o.to_string()),
                None => (head.to_string(), String::new()),
            };
            Capability {
                family,
                operation,
                argument,
            }
        })
        .collect()
}

fn contract(id: &str, placements: &[&str], capabilities: &[&str]) -> ComponentContract {
    let required = caps(capabilities);
    ComponentContract {
        component_id: id.to_string(),
        capability_mapping: pw_host::CAPABILITY_MAPPING,
        abi_schema: "abi".to_string(),
        imports: required
            .iter()
            .map(|c| Import {
                interface: format!("pw:host/{}", c.family),
                name: if c.operation.is_empty() {
                    "use".to_string()
                } else {
                    c.operation.clone()
                },
                capability: c.name(),
                kind: ImportKind::HostCapability,
            })
            .collect(),
        required_capabilities: required,
        allowed_placements: placements.iter().map(|s| s.to_string()).collect(),
        exports: vec![Export {
            name: "Menu".into(),
            kind: "query".into(),
        }],
    }
}

fn node(name: &str, world: &str, grants: &[&str]) -> Node {
    Node {
        name: name.to_string(),
        world: world.to_string(),
        grants: grants
            .iter()
            .map(|s| s.to_string())
            .collect::<BTreeSet<_>>(),
    }
}

fn topology() -> Topology {
    Topology {
        nodes: vec![
            node(
                "primary",
                "origin",
                &["database.read<Stores>", "database.write<Stores>"],
            ),
            // A read-only replica. The reason the topology speaks in full
            // capability names rather than families: this node has a database
            // and cannot be written to, and a family-level grant could not say
            // so.
            node("replica", "origin", &["database.read<Stores>"]),
            node("cdn", "edge", &["cache.read"]),
            node("laptop", "browser", &["dom.mutate"]),
        ],
    }
}

fn allowed(c: &ComponentContract) -> Vec<String> {
    c.imports.iter().map(|i| i.key()).collect()
}

#[test]
fn a_component_is_admitted_where_its_world_and_its_capabilities_are_both_present() {
    let c = contract("shop.Menu", &["origin"], &["database.read<Stores>"]);
    let a = admit(&c, &topology(), "primary", &allowed(&c));
    assert_eq!(
        a,
        Admission::Admit {
            grants: vec!["database.read<Stores>".into()]
        }
    );
}

#[test]
fn placement_and_capability_are_two_checks_and_can_disagree_either_way() {
    // The architect's distinction, made falsifiable. Collapsing the two would
    // allow one of these two and which one depends on which way it collapsed.
    let t = topology();

    // Right world, missing capability: the replica is an origin node and has no
    // write.
    let writer = contract("shop.Restock", &["origin"], &["database.write<Stores>"]);
    let a = admit(&writer, &t, "replica", &allowed(&writer));
    assert_eq!(
        a.refusals(),
        [Refusal::Ungranted {
            node: "replica".into(),
            capability: "database.write<Stores>".into()
        }]
    );

    // Right capability, wrong world: `laptop` grants `dom.mutate`, and a
    // component that may run only at the origin still may not run there.
    let painter = contract("shop.Badge", &["origin"], &["dom.mutate"]);
    let a = admit(&painter, &t, "laptop", &allowed(&painter));
    assert!(a.refusals().contains(&Refusal::WrongWorld {
        node: "laptop".into(),
        world: "browser".into()
    }));
}

#[test]
fn a_capabilitys_type_argument_is_part_of_what_is_granted() {
    // `database.read<Stores>` and `database.read<Payments>` are different
    // authority. A host that matched on the family would let a component
    // authorised to read the catalogue read the payments table — and every
    // test that used one argument would pass.
    let payments = contract("shop.Ledger", &["origin"], &["database.read<Payments>"]);
    let a = admit(&payments, &topology(), "primary", &allowed(&payments));
    assert_eq!(
        a.refusals(),
        [Refusal::Ungranted {
            node: "primary".into(),
            capability: "database.read<Payments>".into()
        }],
        "the primary grants <Stores>, which is not <Payments>"
    );
}

#[test]
fn an_admitted_instance_gets_exactly_its_contract_and_not_the_nodes_whole_set() {
    // The primary grants read AND write. A component that requires only read
    // must receive only read, or the contract is advisory and the node's
    // capability set is the real policy.
    let reader = contract("shop.Menu", &["origin"], &["database.read<Stores>"]);
    let a = admit(&reader, &topology(), "primary", &allowed(&reader));
    assert_eq!(
        a,
        Admission::Admit {
            grants: vec!["database.read<Stores>".into()]
        }
    );

    let granted = Granted::from(&a, &BTreeMap::new()).expect("admitted");
    assert_eq!(granted.count(), 1);
    assert!(granted.handle("database.read<Stores>").is_some());
    assert!(
        granted.handle("database.write<Stores>").is_none(),
        "the node has it; this instance does not"
    );
}

#[test]
fn an_unknown_node_is_refused_rather_than_defaulted() {
    // Fail-closed. A deployment that names a node the topology does not have
    // is a misconfiguration, and "run it somewhere" is the worst possible
    // response.
    let c = contract("shop.Menu", &["origin"], &["database.read<Stores>"]);
    let a = admit(&c, &topology(), "typo", &allowed(&c));
    assert!(!a.is_admitted());
    assert!(a.refusals().contains(&Refusal::WrongWorld {
        node: "typo".into(),
        world: "unknown".into()
    }));
}

#[test]
fn a_component_that_can_run_nowhere_is_refused_everywhere() {
    // An empty `allowed_placements` is E5's solver saying no world satisfies
    // this component. A host that read it as "no constraint" would run the one
    // thing the compiler proved unrunnable.
    let c = contract("shop.Impossible", &[], &[]);
    for name in ["primary", "replica", "cdn", "laptop"] {
        let a = admit(&c, &topology(), name, &[]);
        assert!(a.refusals().contains(&Refusal::Unplaceable), "at {name}");
    }
}

#[test]
fn a_component_needing_nothing_is_admitted_and_granted_nothing() {
    let c = contract("shop.Label", &["build", "browser", "edge", "origin"], &[]);
    let a = admit(&c, &topology(), "laptop", &[]);
    assert_eq!(a, Admission::Admit { grants: vec![] });
    let granted = Granted::from(&a, &BTreeMap::new()).expect("admitted");
    assert_eq!(granted.count(), 0);
    assert!(granted.handle("dom.mutate").is_none());
}

// --- the artifact audit ------------------------------------------------------

#[test]
fn an_artifact_importing_more_than_its_contract_allows_is_refused() {
    // ADR-0020's rule, at the moment it matters. The contract is about the
    // program; this is about the BUILT THING, and they are not the same claim.
    let c = contract("shop.Menu", &["origin"], &["database.read<Stores>"]);
    let mut actual = allowed(&c);
    actual.push("wasi:cli/environment@0.2.9#get-environment".into());

    let a = admit(&c, &topology(), "primary", &actual);
    assert_eq!(
        a.refusals(),
        [Refusal::UndeclaredImport {
            imports: vec!["wasi:cli/environment@0.2.9#get-environment".into()]
        }]
    );
}

#[test]
fn the_std_runtime_import_set_is_refused_wholesale() {
    // `docs/evidence/E0/spike-wasmtime-component.txt`, as a decision rather
    // than an observation. A guest built with Rust `std` for `wasm32-wasip2`
    // declares ONE import in its world and the component demands fifteen. Not
    // one of the fourteen was asked for by a line of application code.
    let c = contract("shop.Menu", &["origin"], &["database.read<Stores>"]);
    let mut actual = allowed(&c);
    for wasi in [
        "wasi:cli/environment@0.2.9",
        "wasi:cli/exit@0.2.9",
        "wasi:cli/stderr@0.2.9",
        "wasi:cli/stdin@0.2.9",
        "wasi:cli/stdout@0.2.9",
        "wasi:cli/terminal-input@0.2.9",
        "wasi:cli/terminal-output@0.2.9",
        "wasi:cli/terminal-stderr@0.2.9",
        "wasi:cli/terminal-stdin@0.2.9",
        "wasi:cli/terminal-stdout@0.2.9",
        "wasi:clocks/monotonic-clock@0.2.9",
        "wasi:io/error@0.2.9",
        "wasi:io/poll@0.2.9",
        "wasi:io/streams@0.2.9",
    ] {
        actual.push(wasi.to_string());
    }

    let a = admit(&c, &topology(), "primary", &actual);
    match &a.refusals()[0] {
        Refusal::UndeclaredImport { imports } => {
            assert_eq!(imports.len(), 14, "every one is named, not just the first");
            assert!(imports.iter().all(|i| i.starts_with("wasi:")));
        }
        other => panic!("expected an import refusal, got {other:?}"),
    }
}

#[test]
fn importing_fewer_than_allowed_is_fine() {
    // Subset, not equality. Dead code and never-taken branches are not
    // security events.
    let c = contract(
        "shop.Both",
        &["origin"],
        &["database.read<Stores>", "database.write<Stores>"],
    );
    let a = admit(
        &c,
        &topology(),
        "primary",
        &["pw:host/database#read".into()],
    );
    assert!(a.is_admitted());
    assert!(admit(&c, &topology(), "primary", &[]).is_admitted());
}

#[test]
fn every_refusal_is_reported_not_only_the_first() {
    // A deployment that fixes one and rediscovers the next is a slow way to
    // learn the shape of a problem — and the slow way is where people stop.
    let c = contract("shop.Restock", &["browser"], &["database.write<Stores>"]);
    let a = admit(&c, &topology(), "replica", &["wasi:cli/exit@0.2.9".into()]);

    let kinds: Vec<&str> = a
        .refusals()
        .iter()
        .map(|r| match r {
            Refusal::WrongWorld { .. } => "world",
            Refusal::Ungranted { .. } => "capability",
            Refusal::UndeclaredImport { .. } => "import",
            Refusal::Unplaceable => "unplaceable",
            Refusal::UnknownMapping { .. } => "mapping",
        })
        .collect();
    assert_eq!(kinds, ["world", "capability", "import"]);
}

// --- handles ------------------------------------------------------------------

#[test]
fn a_guest_receives_a_handle_and_never_the_value() {
    let c = contract("shop.Menu", &["origin"], &["database.read<Stores>"]);
    let a = admit(&c, &topology(), "primary", &allowed(&c));

    let backing = BTreeMap::from([(
        "database.read<Stores>".to_string(),
        "postgres://user:hunter2@db.internal/stores".to_string(),
    )]);
    let granted = Granted::from(&a, &backing).expect("admitted");
    let handle = granted.handle("database.read<Stores>").expect("granted");

    // Everything a guest can see of the handle.
    let rendered = format!("{handle}|{handle:?}");
    assert!(
        !rendered.contains("hunter2") && !rendered.contains("postgres"),
        "a handle must not carry what it refers to: {rendered}"
    );
    assert_eq!(handle.capability(), "database.read<Stores>");

    // The host, holding the same handle, can reach the value. That is the
    // control: if `use_handle` returned nothing for everyone, the assertion
    // above would be about a capability that does not work.
    assert_eq!(
        granted.use_handle(handle),
        Some("postgres://user:hunter2@db.internal/stores")
    );
}

#[test]
fn a_handle_from_another_instance_does_not_work_here() {
    // Membership is checked rather than the handle being trusted to describe
    // itself. Otherwise a guest that learned another instance's handle shape
    // could name a capability it was never granted.
    let reader = contract("shop.Menu", &["origin"], &["database.read<Stores>"]);
    let writer = contract("shop.Restock", &["origin"], &["database.write<Stores>"]);
    let backing = BTreeMap::from([
        ("database.read<Stores>".to_string(), "R".to_string()),
        ("database.write<Stores>".to_string(), "W".to_string()),
    ]);

    let t = topology();
    let r = Granted::from(&admit(&reader, &t, "primary", &allowed(&reader)), &backing).unwrap();
    let w = Granted::from(&admit(&writer, &t, "primary", &allowed(&writer)), &backing).unwrap();

    let write_handle = w.handle("database.write<Stores>").expect("granted");
    assert_eq!(
        r.use_handle(write_handle),
        None,
        "the reader holds no write capability, whatever handle it is shown"
    );
    assert_eq!(w.use_handle(write_handle), Some("W"));
}

#[test]
fn a_refused_admission_yields_no_capabilities_at_all() {
    // There is deliberately no way to build a `Granted` from a refusal. An
    // "override" parameter is how a capability system becomes a logging system.
    let c = contract("shop.Restock", &["origin"], &["database.write<Stores>"]);
    let a = admit(&c, &topology(), "replica", &allowed(&c));
    assert!(Granted::from(&a, &BTreeMap::new()).is_none());
}

// --- the boundary --------------------------------------------------------------

#[test]
fn the_topology_is_data_the_host_owns() {
    // "A declarative host topology replacing `worlds_for`". The compiler's
    // table says which worlds COULD grant a family — a statement about the
    // shape of the web. This says what this deployment's machines actually
    // have, which no compiler can know.
    let json = r#"{
      "nodes": [
        { "name": "edge-lhr", "world": "edge", "grants": ["cache.read"] },
        { "name": "origin-1", "world": "origin", "grants": ["database.read<Stores>"] }
      ]
    }"#;
    let t = Topology::from_json(json).expect("parses");
    assert_eq!(t.nodes.len(), 2);
    assert_eq!(t.node("edge-lhr").unwrap().world, "edge");

    // And it decides. A cache-only edge node cannot host a database reader,
    // however the compiler feels about edges.
    let c = contract("shop.Menu", &["edge", "origin"], &["database.read<Stores>"]);
    assert!(!admit(&c, &t, "edge-lhr", &allowed(&c)).is_admitted());
    assert!(admit(&c, &t, "origin-1", &allowed(&c)).is_admitted());
}

// --- from a decision to an instantiation ---------------------------------

#[test]
fn the_linker_is_built_from_the_admission_and_not_from_the_node() {
    // `docs/evidence/E0/spike-wasmtime-component.txt` already proved the engine
    // half: a component whose import is withheld from the linker fails to
    // instantiate, and the diagnostic names the missing interface. What that
    // spike did not establish is where the linker's CONTENTS come from — it
    // added one capability by hand.
    //
    // Here they come from the decision. The primary node has read AND write; a
    // component requiring only read gets a linker holding only read, so the
    // engine's refusal is triggered by the CONTRACT rather than by whatever the
    // machine happened to be missing.
    let reader = contract("shop.Menu", &["origin"], &["database.read<Stores>"]);
    let a = admit(&reader, &topology(), "primary", &allowed(&reader));
    let granted = Granted::from(&a, &BTreeMap::new()).expect("admitted");

    assert_eq!(linkable(&reader, &granted), ["pw:host/database#read"]);
    assert!(
        !linkable(&reader, &granted).contains(&"pw:host/database#write".to_string()),
        "the node has write; this component's linker must not"
    );
}

#[test]
fn a_component_granted_nothing_gets_an_empty_linker() {
    // The case where "no ambient authority" is decided. An empty linker means
    // any import at all fails instantiation — which is what makes a pure
    // component genuinely pure rather than merely uninteresting.
    let c = contract("shop.Label", &["browser"], &[]);
    let granted =
        Granted::from(&admit(&c, &topology(), "laptop", &[]), &BTreeMap::new()).expect("admitted");
    assert!(linkable(&c, &granted).is_empty());
}

#[test]
fn a_contract_from_an_unknown_capability_mapping_is_refused() {
    // Two mappings can spell one capability the same way and mean different
    // authority. A host that interpreted an unknown mapping would be guessing
    // about exactly the thing it exists to decide.
    let mut c = contract("shop.Menu", &["origin"], &["database.read<Stores>"]);
    c.capability_mapping = pw_host::CAPABILITY_MAPPING + 1;

    let a = admit(&c, &topology(), "primary", &allowed(&c));
    assert!(a.refusals().contains(&Refusal::UnknownMapping {
        found: pw_host::CAPABILITY_MAPPING + 1,
        understood: pw_host::CAPABILITY_MAPPING,
    }));

    // The control: the same contract at the known mapping is admitted, so the
    // refusal is about the mapping rather than about the contract.
    c.capability_mapping = pw_host::CAPABILITY_MAPPING;
    assert!(admit(&c, &topology(), "primary", &allowed(&c)).is_admitted());
}

// --- three classes of import, three sources of truth -------------------

fn with_component_dep(mut c: ComponentContract) -> ComponentContract {
    c.imports.push(Import {
        interface: "pw:app/shop.Store".into(),
        name: "Store".into(),
        capability: String::new(),
        kind: ImportKind::Component,
    });
    c
}

#[test]
fn an_import_is_classified_before_it_is_compared() {
    // Architect ruling, 2026-08-07: the audit should classify actual Wasm
    // imports rather than flatten all imports into one set. Each class gets
    // compared against its own allowed source.
    let c = with_component_dep(contract(
        "shop.Page",
        &["origin"],
        &["database.read<Stores>"],
    ));
    assert_eq!(classify(&c, "pw:host/database#read"), ImportClass::Host);
    assert_eq!(
        classify(&c, "pw:app/shop.Store#Store"),
        ImportClass::Component
    );
    assert_eq!(
        classify(&c, "wasi:cli/exit@0.2.9#exit"),
        ImportClass::Runtime
    );
    // Anything the contract does not list is runtime — the fail-closed
    // direction, because an interface nobody described is authority nobody
    // decided about.
    assert_eq!(classify(&c, "mystery"), ImportClass::Runtime);

    // MEMBERSHIP, not spelling. A component built against a hand-written WIT
    // world uses that world's names, and refusing those on a prefix would make
    // the audit a check on naming rather than on authority.
    let mut spike = contract("spike.Store", &["origin"], &[]);
    spike.imports.push(Import {
        interface: "perfect-web:store/stores@0.1.0".into(),
        name: "read".into(),
        capability: "store.read".into(),
        kind: ImportKind::HostCapability,
    });
    assert_eq!(
        classify(&spike, "perfect-web:store/stores@0.1.0#read"),
        ImportClass::Host
    );
}

#[test]
fn a_component_dependency_is_not_authority() {
    // A page that calls a privileged query depends on it; it does not acquire
    // its `database.read`. The dependency is allowed and the capability is not.
    let c = with_component_dep(contract("shop.Page", &["origin"], &[]));
    assert!(c.required_capabilities.is_empty());

    let a = admit(
        &c,
        &topology(),
        "primary",
        &["pw:app/shop.Store#Store".into()],
    );
    assert!(
        a.is_admitted(),
        "the dependency is declared: {:?}",
        a.refusals()
    );

    // And the host grants it nothing, however privileged the component it calls.
    let granted = Granted::from(&a, &BTreeMap::new()).expect("admitted");
    assert_eq!(granted.count(), 0);
    assert!(linkable(&c, &granted).is_empty());
}

#[test]
fn a_component_slot_cannot_be_filled_by_a_host_import_or_the_reverse() {
    // The confusion the split exists to prevent. A flattened set would let
    // either satisfy the other, and then "this component may call the database"
    // and "this component may call the Store query" become one permission.
    let c = with_component_dep(contract(
        "shop.Page",
        &["origin"],
        &["database.read<Stores>"],
    ));

    // The declared pair, in their own slots: admitted.
    assert!(
        admit(
            &c,
            &topology(),
            "primary",
            &[
                "pw:host/database#read".into(),
                "pw:app/shop.Store#Store".into()
            ]
        )
        .is_admitted()
    );

    // A component import that is not declared: refused, even though a host
    // import IS declared.
    let a = admit(
        &c,
        &topology(),
        "primary",
        &["pw:app/shop.Ledger#Ledger".into()],
    );
    assert_eq!(
        a.refusals(),
        [Refusal::UndeclaredImport {
            imports: vec!["pw:app/shop.Ledger#Ledger".into()]
        }]
    );

    // A host import that is not declared: refused, even though a component
    // import IS declared.
    let a = admit(&c, &topology(), "primary", &["pw:host/secret#read".into()]);
    assert_eq!(
        a.refusals(),
        [Refusal::UndeclaredImport {
            imports: vec!["pw:host/secret#read".into()]
        }]
    );
}

#[test]
fn a_runtime_import_can_never_be_authorised() {
    // No effect row asks for `wasi:cli/exit` and no node grants it: it is not
    // authority the program requested, it is authority the toolchain added. So
    // there is deliberately no way to put one in a contract and have it pass.
    let mut c = contract("shop.Menu", &["origin"], &["database.read<Stores>"]);
    c.imports.push(Import {
        interface: "wasi:cli/exit@0.2.9".into(),
        name: "exit".into(),
        capability: "database.read<Stores>".into(),
        kind: ImportKind::HostCapability,
    });

    let a = admit(
        &c,
        &topology(),
        "primary",
        &["wasi:cli/exit@0.2.9#exit".into()],
    );
    assert_eq!(
        a.refusals(),
        [Refusal::UndeclaredImport {
            imports: vec!["wasi:cli/exit@0.2.9#exit".into()]
        }],
        "a contract cannot authorise a runtime import by listing it"
    );
}
