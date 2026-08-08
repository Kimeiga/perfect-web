//! Deployment planning over the REAL committed contracts.
//!
//! `docs/evidence/E8/component-contracts.json` is compiler output, checked in
//! so the host is tested against what the compiler actually emits rather than a
//! fixture written to agree with it. The planner gets the same treatment.
//!
//! The property that shapes every test here, from the architect's ruling:
//!
//! > `admit(component, node)` stays local. The planner composes those local
//! > facts; it doesn't make `admit()` recursive.

use std::collections::BTreeSet;

use pw_host::plan::plan;
use pw_host::{ComponentContract, Node, Topology, admit};

fn contracts() -> Vec<ComponentContract> {
    let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../docs/evidence/E8/component-contracts.json");
    let text = std::fs::read_to_string(&path).expect("committed contracts");
    serde_json::from_str(&text).expect("contracts parse")
}

/// A deployment that can host the store: a laptop and an origin serving the
/// three read domains and the one write domain the demo needs.
fn full() -> Topology {
    Topology {
        nodes: vec![
            Node {
                name: "laptop".into(),
                world: "browser".into(),
                grants: BTreeSet::new(),
            },
            Node {
                name: "origin-1".into(),
                world: "origin".into(),
                grants: BTreeSet::from([
                    "database.read<Carts>".to_string(),
                    "database.read<Menus>".to_string(),
                    "database.read<Stores>".to_string(),
                    "database.write<Carts>".to_string(),
                ]),
            },
        ],
    }
}

#[test]
fn the_store_program_is_deployable_on_a_topology_that_serves_it() {
    let p = plan(&contracts(), &full());
    assert!(
        p.is_deployable(),
        "unplaceable: {:?}, dangling: {:?}",
        p.unplaceable,
        p.dangling
    );
    assert!(
        p.placements.len() >= 5,
        "every contract gets a placement: {}",
        p.placements.len()
    );
    assert!(!p.edges.is_empty(), "the page depends on its queries");
}

#[test]
fn a_topology_that_grants_nothing_cannot_host_the_privileged_components() {
    // The negative control. Without it, "deployable" above could be what a
    // planner that admits everything says.
    let barren = Topology {
        nodes: vec![Node {
            name: "laptop".into(),
            world: "browser".into(),
            grants: BTreeSet::new(),
        }],
    };
    let p = plan(&contracts(), &barren);
    assert!(!p.is_deployable());
    assert!(
        p.unplaceable.iter().any(|c| c.contains("add_to_cart")),
        "the command that writes has nowhere to run: {:?}",
        p.unplaceable
    );
    // And a component needing no authority still places, so "unplaceable" is
    // not the answer to everything.
    assert!(
        p.placements
            .iter()
            .any(|pl| pl.component.contains("StorePage") && pl.is_placeable()),
        "the page renders anywhere"
    );
}

#[test]
fn a_refused_component_carries_the_reason_from_every_node() {
    let barren = Topology {
        nodes: vec![Node {
            name: "laptop".into(),
            world: "browser".into(),
            grants: BTreeSet::new(),
        }],
    };
    let p = plan(&contracts(), &barren);
    let refused = p
        .placements
        .iter()
        .find(|pl| !pl.is_placeable())
        .expect("something is unplaceable");
    assert_eq!(refused.rejected.len(), 1, "one node, one rejection");
    assert!(
        !refused.rejected[0].1.is_empty(),
        "and it says why: {:?}",
        refused.rejected
    );
}

#[test]
fn the_planner_agrees_with_admit_on_every_pair() {
    // **The structural property.** The planner composes `admit`; it must not
    // disagree with it anywhere. If this ever fails, a second decision
    // procedure has appeared — which is the failure the module header forbids.
    let (cs, topo) = (contracts(), full());
    let p = plan(&cs, &topo);
    for c in &cs {
        let placement = p
            .placements
            .iter()
            .find(|pl| pl.component == c.component_id)
            .expect("a placement per contract");
        for node in &topo.nodes {
            let direct = admit(c, &topo, &node.name, &[]).is_admitted();
            let planned = placement.candidates.contains(&node.name);
            assert_eq!(
                direct, planned,
                "{} on {}: admit says {direct}, plan says {planned}",
                c.component_id, node.name
            );
        }
    }
}

#[test]
fn can_be_local_is_disjointness_of_candidate_sets() {
    // The fact transferability will consume: an edge is necessarily remote
    // exactly when its two ends share no node.
    //
    // On the real store contracts every edge CAN be local, and that is the
    // right answer rather than a gap: `StorePage` renders at the origin as
    // happily as in a browser, so it shares `origin-1` with the command it
    // triggers. My first version of this test assumed otherwise and asserted a
    // remote edge existed — a wrong expectation about the program, not about
    // the planner.
    let p = plan(&contracts(), &full());
    assert!(!p.edges.is_empty());
    for e in &p.edges {
        let here = &p
            .placements
            .iter()
            .find(|pl| pl.component == e.from)
            .expect("from")
            .candidates;
        let there = &p
            .placements
            .iter()
            .find(|pl| pl.component == e.to)
            .expect("to")
            .candidates;
        let shares = here.iter().any(|n| there.contains(n));
        assert_eq!(
            e.can_be_local, shares,
            "{} -> {}: candidates {here:?} and {there:?}",
            e.from, e.to
        );
    }
    assert!(
        p.edges.iter().all(|e| e.can_be_local),
        "on this topology every edge can be local: {:?}",
        p.edges
    );
}

#[test]
fn an_edge_whose_ends_share_no_node_is_necessarily_remote() {
    // The other side, and it needs SYNTHETIC contracts rather than the store's.
    //
    // Two attempts on the real ones failed, and the reason is a fact about the
    // demo rather than the planner: `StorePage` requires no capability and its
    // placements include the origin, so it shares a node with everything it
    // calls under every topology I could build. No edge in that program is
    // necessarily remote. Recorded here rather than engineered around — a test
    // that tortured the topology until the assertion passed would be measuring
    // the torture.
    let browser_only = ComponentContract {
        component_id: "app.Widget".into(),
        capability_mapping: pw_host::CAPABILITY_MAPPING,
        abi_schema: "x".into(),
        required_capabilities: vec![],
        allowed_placements: vec!["browser".into()],
        imports: vec![pw_host::Import {
            interface: "pw:app/app.Store".into(),
            name: "Store".into(),
            capability: String::new(),
            kind: pw_host::ImportKind::Component,
        }],
        exports: vec![pw_host::Export {
            name: "Widget".into(),
            kind: "component".into(),
            binding: pw_host::BindingSupport::default(),
        }],
    };
    let origin_only = ComponentContract {
        component_id: "app.Store".into(),
        capability_mapping: pw_host::CAPABILITY_MAPPING,
        abi_schema: "y".into(),
        required_capabilities: vec![],
        allowed_placements: vec!["origin".into()],
        imports: vec![],
        exports: vec![pw_host::Export {
            name: "Store".into(),
            kind: "query".into(),
            binding: pw_host::BindingSupport::default(),
        }],
    };

    let p = plan(&[browser_only, origin_only], &full());
    assert!(p.is_deployable(), "{:?}", p.unplaceable);
    assert_eq!(p.edges.len(), 1);
    assert!(
        !p.edges[0].can_be_local,
        "a browser-only component and an origin-only one share no node: {:?}",
        p.edges
    );
}

#[test]
fn an_import_naming_nothing_is_dangling_rather_than_silently_ignored() {
    let mut cs = contracts();
    let victim = cs
        .iter_mut()
        .find(|c| c.imports.iter().any(|i| i.interface.starts_with("pw:app/")))
        .expect("a component dependency");
    for i in victim.imports.iter_mut() {
        if i.interface.starts_with("pw:app/") {
            i.interface = "pw:app/nothing.at.all".to_string();
        }
    }
    let p = plan(&cs, &full());
    assert!(!p.dangling.is_empty(), "a missing export is reported");
    assert!(!p.is_deployable());
}

// --- transferability: the other half of an edge ------------------------------

/// Two independent facts, and neither implies the other.
///
/// ```text
/// may the two ends share a node?     placement — this module
/// may the values cross a boundary?   the types — the compiler
/// ```
///
/// The four combinations are all real, and only one of them is a failure.
#[test]
fn placement_and_transferability_are_independent_and_only_one_pair_fails() {
    // A browser-only importer and an origin-only exporter: the ends cannot
    // share a node, so the edge is necessarily remote. Whether that is fine
    // depends entirely on what the exported signature carries.
    let widget = |target: &str| ComponentContract {
        component_id: "app.Widget".into(),
        capability_mapping: pw_host::CAPABILITY_MAPPING,
        abi_schema: "x".into(),
        required_capabilities: vec![],
        allowed_placements: vec!["browser".into()],
        imports: vec![pw_host::Import {
            interface: format!("pw:app/{target}"),
            name: "Store".into(),
            capability: String::new(),
            kind: pw_host::ImportKind::Component,
        }],
        exports: vec![pw_host::Export {
            name: "Widget".into(),
            kind: "component".into(),
            binding: pw_host::BindingSupport::default(),
        }],
    };
    let store = |placements: &[&str], remote: pw_host::RemoteSupport| ComponentContract {
        component_id: "app.Store".into(),
        capability_mapping: pw_host::CAPABILITY_MAPPING,
        abi_schema: "y".into(),
        required_capabilities: vec![],
        allowed_placements: placements.iter().map(|s| s.to_string()).collect(),
        imports: vec![],
        exports: vec![pw_host::Export {
            name: "Store".into(),
            kind: "query".into(),
            binding: pw_host::BindingSupport {
                local: pw_host::LocalSupport::Direct,
                remote,
            },
        }],
    };
    let handle = || pw_host::RemoteSupport::Refused {
        positions: vec![pw_host::Untransferable {
            position: "argument 0".into(),
            ty: Some("OpenTransaction".into()),
            reason: "the value IS the thing held open".into(),
        }],
    };

    // 1. Co-locatable, transferable. Bind it either way.
    let p = plan(
        &[
            widget("app.Store"),
            store(&["browser", "origin"], pw_host::RemoteSupport::Transferable),
        ],
        &full(),
    );
    assert!(p.edges[0].can_be_local);
    assert_eq!(p.edges[0].can_be_remote, Some(true));
    assert!(p.is_deployable());

    // 2. Co-locatable, NOT transferable. Perfectly ordinary: a same-process
    //    call passing a handle. The whole reason `LocalSupport` is a separate
    //    field rather than the other arm of an enum.
    let p = plan(
        &[widget("app.Store"), store(&["browser", "origin"], handle())],
        &full(),
    );
    assert!(p.edges[0].can_be_local);
    assert_eq!(p.edges[0].can_be_remote, Some(false));
    assert!(p.is_deployable(), "co-location is a binding");

    // 3. Necessarily remote, transferable. Also ordinary: an RPC.
    let p = plan(
        &[
            widget("app.Store"),
            store(&["origin"], pw_host::RemoteSupport::Transferable),
        ],
        &full(),
    );
    assert!(!p.edges[0].can_be_local);
    assert_eq!(p.edges[0].can_be_remote, Some(true));
    assert!(p.is_deployable(), "a remote edge is a binding");

    // 4. Necessarily remote AND not transferable. The one failure, and it is
    //    invisible to either fact alone — each of the three cases above shares
    //    one half with it.
    let p = plan(
        &[widget("app.Store"), store(&["origin"], handle())],
        &full(),
    );
    assert!(!p.edges[0].can_be_local);
    assert_eq!(p.edges[0].can_be_remote, Some(false));
    assert!(!p.is_deployable(), "no binding exists for this edge");
    assert_eq!(p.unbindable().len(), 1);
    assert_eq!(
        p.unbindable()[0].untransferable[0].ty.as_deref(),
        Some("OpenTransaction"),
        "and it names what cannot cross, not just that something cannot"
    );
}

#[test]
fn an_undetermined_signature_does_not_refuse_a_deployment() {
    // The third answer, and the reason it is not `false`. A position whose type
    // the compiler could not determine is a build-time gap the author is owed a
    // diagnostic about. Refusing the deployment for it would report a typing
    // problem as a topology problem, at the point furthest from the cause.
    let store = ComponentContract {
        component_id: "app.Store".into(),
        capability_mapping: pw_host::CAPABILITY_MAPPING,
        abi_schema: "y".into(),
        required_capabilities: vec![],
        allowed_placements: vec!["origin".into()],
        imports: vec![],
        exports: vec![pw_host::Export {
            name: "Store".into(),
            kind: "query".into(),
            binding: pw_host::BindingSupport {
                local: pw_host::LocalSupport::Direct,
                remote: pw_host::RemoteSupport::Undetermined {
                    positions: vec![pw_host::Untransferable {
                        position: "argument 0".into(),
                        ty: None,
                        reason: "this build could not determine the type".into(),
                    }],
                },
            },
        }],
    };
    let widget = ComponentContract {
        component_id: "app.Widget".into(),
        capability_mapping: pw_host::CAPABILITY_MAPPING,
        abi_schema: "x".into(),
        required_capabilities: vec![],
        allowed_placements: vec!["browser".into()],
        imports: vec![pw_host::Import {
            interface: "pw:app/app.Store".into(),
            name: "Store".into(),
            capability: String::new(),
            kind: pw_host::ImportKind::Component,
        }],
        exports: vec![pw_host::Export {
            name: "Widget".into(),
            kind: "component".into(),
            binding: pw_host::BindingSupport::default(),
        }],
    };
    let p = plan(&[widget, store], &full());
    assert!(!p.edges[0].can_be_local, "browser-only and origin-only");
    assert_eq!(p.edges[0].can_be_remote, None, "not false");
    assert!(p.is_deployable());
    // ...and it still carries what the compiler could not decide, so a host
    // that wants to be strict has the material.
    assert!(!p.edges[0].untransferable.is_empty());
}

#[test]
fn every_edge_in_the_store_program_can_be_bound() {
    // The real contracts, and the control for the synthetic cases above. If the
    // classification refused something in the demo, `just ci` would be green
    // and the deployment would not exist.
    let p = plan(&contracts(), &full());
    assert!(p.unbindable().is_empty(), "{:?}", p.unbindable());
    // Nothing is REFUSED: no exported signature carries a resource.
    assert!(
        p.edges.iter().all(|e| e.can_be_remote != Some(false)),
        "{:?}",
        p.edges
            .iter()
            .filter(|e| e.can_be_remote == Some(false))
            .collect::<Vec<_>>()
    );

    // **Some carry an obligation, and that is the 2026-08-08 ruling working.**
    // `Cart` is session-scoped — only a `session query` produces one — and a
    // remote binding of an edge that carries it must reach the SAME session.
    // An origin node handles millions of them, so `World` cannot answer this
    // and a node-level privacy scope would be the wrong shape: the property
    // belongs to the binding, not to the machine.
    //
    // This asserted `Transferable` first, then `Undetermined`. The third answer
    // is the right one — the compiler knows exactly what is owed.
    let owing: Vec<&pw_host::plan::Edge> = p
        .edges
        .iter()
        .filter(|e| !e.obligations.is_empty())
        .collect();
    assert!(
        !owing.is_empty(),
        "the store has session-scoped exports and at least one edge reaches them"
    );
    for e in &owing {
        assert_eq!(
            e.can_be_remote,
            Some(true),
            "an obligation is not a refusal"
        );
        assert!(
            e.obligations.iter().any(|o| {
                let pw_host::Obligation::PreservePrincipal { principal, .. } = o;
                principal.contains("Session")
            }),
            "and it names the principal a binding must preserve: {e:?}"
        );
    }

    // The discriminating half: the public exports owe nothing, so "carries an
    // obligation" is not the answer to everything.
    assert!(
        p.edges.iter().any(|e| e.obligations.is_empty()),
        "`Menu` and `Store` carry keys and public records: {:?}",
        p.edges
    );

    // **And every obligation here is discharged by co-location.** A
    // same-process call does not leave the principal's context, so an edge
    // whose ends can share a node owes nothing in practice. That is why the
    // store still plans — and `undischarged` is what would catch it if a
    // session-scoped edge were ever forced apart.
    assert!(p.undischarged().is_empty(), "{:?}", p.undischarged());
    assert!(p.is_deployable());
}

/// **An obligation nothing discharges is not a complete plan.**
///
/// Architect ruling, 2026-08-08: *"Your planner may keep an `Undetermined` edge
/// as a candidate, but it must not call a deployment plan complete until the
/// binding discharges that obligation."*
///
/// Synthetic, because no edge in the store demo is necessarily remote — the
/// same reason `an_edge_whose_ends_share_no_node_is_necessarily_remote` is.
#[test]
fn a_necessarily_remote_edge_that_owes_a_principal_is_not_a_finished_plan() {
    let widget = ComponentContract {
        component_id: "app.Widget".into(),
        capability_mapping: pw_host::CAPABILITY_MAPPING,
        abi_schema: "x".into(),
        required_capabilities: vec![],
        allowed_placements: vec!["browser".into()],
        imports: vec![pw_host::Import {
            interface: "pw:app/app.Basket".into(),
            name: "Basket".into(),
            capability: String::new(),
            kind: pw_host::ImportKind::Component,
        }],
        exports: vec![pw_host::Export {
            name: "Widget".into(),
            kind: "component".into(),
            binding: pw_host::BindingSupport::default(),
        }],
    };
    let basket = ComponentContract {
        component_id: "app.Basket".into(),
        capability_mapping: pw_host::CAPABILITY_MAPPING,
        abi_schema: "y".into(),
        required_capabilities: vec![],
        // Origin-only, so the edge cannot be co-located and the obligation has
        // to be discharged by a binding rather than by proximity.
        allowed_placements: vec!["origin".into()],
        imports: vec![],
        exports: vec![pw_host::Export {
            name: "Basket".into(),
            kind: "query".into(),
            binding: pw_host::BindingSupport {
                local: pw_host::LocalSupport::Direct,
                remote: pw_host::RemoteSupport::Conditional {
                    obligations: vec![pw_host::Obligation::PreservePrincipal {
                        principal: "Session<SessionId>".into(),
                        position: "result".into(),
                        ty: Some("Cart".into()),
                    }],
                },
            },
        }],
    };

    let p = plan(&[widget, basket], &full());
    assert!(!p.edges[0].can_be_local, "browser-only and origin-only");
    assert_eq!(
        p.edges[0].can_be_remote,
        Some(true),
        "an obligation is a yes with a condition, not a refusal"
    );
    assert_eq!(p.undischarged().len(), 1);
    assert!(
        !p.is_deployable(),
        "nothing here proves the binding preserves the principal"
    );

    // **The discriminating half.** The same obligation on a CO-LOCATABLE edge
    // is discharged: a same-process call does not leave the principal's
    // context. Without this, the assertion above would hold for a planner that
    // refused every session-scoped edge, which would make private data
    // unusable across components.
    let mut colocatable = basket_colocatable();
    colocatable.allowed_placements = vec!["browser".into(), "origin".into()];
    let p = plan(&[widget_for(), colocatable], &full());
    assert!(p.edges[0].can_be_local);
    assert!(p.undischarged().is_empty());
    assert!(p.is_deployable());
}

fn widget_for() -> ComponentContract {
    ComponentContract {
        component_id: "app.Widget".into(),
        capability_mapping: pw_host::CAPABILITY_MAPPING,
        abi_schema: "x".into(),
        required_capabilities: vec![],
        allowed_placements: vec!["browser".into()],
        imports: vec![pw_host::Import {
            interface: "pw:app/app.Basket".into(),
            name: "Basket".into(),
            capability: String::new(),
            kind: pw_host::ImportKind::Component,
        }],
        exports: vec![pw_host::Export {
            name: "Widget".into(),
            kind: "component".into(),
            binding: pw_host::BindingSupport::default(),
        }],
    }
}

fn basket_colocatable() -> ComponentContract {
    ComponentContract {
        component_id: "app.Basket".into(),
        capability_mapping: pw_host::CAPABILITY_MAPPING,
        abi_schema: "y".into(),
        required_capabilities: vec![],
        allowed_placements: vec!["origin".into()],
        imports: vec![],
        exports: vec![pw_host::Export {
            name: "Basket".into(),
            kind: "query".into(),
            binding: pw_host::BindingSupport {
                local: pw_host::LocalSupport::Direct,
                remote: pw_host::RemoteSupport::Conditional {
                    obligations: vec![pw_host::Obligation::PreservePrincipal {
                        principal: "Session<SessionId>".into(),
                        position: "result".into(),
                        ty: Some("Cart".into()),
                    }],
                },
            },
        }],
    }
}
