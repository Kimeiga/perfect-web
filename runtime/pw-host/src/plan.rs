//! **Deployment planning: which component runs where, and whether it can.**
//!
//! Architect ruling, 2026-08-07:
//!
//! ```text
//! candidate nodes = contract.allowed_placements
//!                   ∩ nodes satisfying required_capabilities
//! ```
//!
//! and the constraint that shapes the whole module:
//!
//! > `admit(component, node)` stays local. The planner composes those local
//! > facts; it doesn't make `admit()` recursive.
//!
//! So there is exactly one decision procedure. [`crate::admit`] answers "may
//! THIS component run on THIS node" and knows nothing about the graph; the
//! planner asks it once per candidate pair and reasons about the answers. A
//! planner that re-derived admission would be a second implementation of the
//! rule the host exists to enforce, and the two would agree until one changed.
//!
//! # Two independent questions about an edge
//!
//! ```text
//! may the two ends share a node?     placement — this module, from admit()
//! may the values cross a boundary?   transferability — the compiler, from types
//! ```
//!
//! Neither answers the other, and an edge needs both. A page and the command it
//! calls can be co-locatable and pass a database handle, or separated by
//! placement and pass nothing but keys. The compiler cannot see a deployment
//! and this module cannot see a signature's types, so each says what it knows
//! and `Edge` carries both.
//!
//! # What this still does not decide
//!
//! Whether an edge SHOULD be remote where it could be either. A remote binding
//! chosen for latency or blast radius is a policy layer above this; the
//! language's job is to state constraints, and browser/server separation
//! existing conceptually is not one of them.
//!
//! # Why refusals are collected rather than short-circuited
//!
//! The same reason `admit` does it: a deployment that fixes one problem and
//! rediscovers the next is a slow way to learn the shape of a problem. A plan
//! that fails names every component with nowhere to run, not the first.

use std::collections::{BTreeMap, BTreeSet};

// The host's OWN mirrored contract types. ADR-0018: the contract is a data
// artifact and neither side links the other, so a planner reaching across that
// boundary would undo the thing the boundary is for.
use crate::{Admission, ComponentContract, ImportKind, Topology, admit};

/// Where one component may run.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Placement {
    pub component: String,
    /// Every node that admits it, in topology order. Empty means nowhere.
    pub candidates: Vec<String>,
    /// Why each rejecting node rejected it, so a failure is actionable.
    pub rejected: Vec<(String, Vec<crate::Refusal>)>,
}

impl Placement {
    pub fn is_placeable(&self) -> bool {
        !self.candidates.is_empty()
    }
}

/// One dependency between placed components.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Edge {
    pub from: String,
    /// The interface key, as the importing contract names it.
    pub to: String,
    /// Whether the two ends can share a node in at least one assignment.
    ///
    /// From PLACEMENT. `false` means the edge is necessarily remote, which is
    /// a real fact about a deployment. It is not a verdict on its own — a
    /// remote edge is perfectly legitimate — and it becomes one only next to
    /// `can_be_remote`.
    pub can_be_local: bool,
    /// Whether the values in the callee's signature can cross a process
    /// boundary.
    ///
    /// From the TYPES, which only the compiler can see, carried in the callee's
    /// `Export`. `None` where the compiler had no basis to decide: a position
    /// whose type that build could not determine. Not a no — see
    /// [`crate::RemoteSupport::Undetermined`].
    pub can_be_remote: Option<bool>,
    /// Why the edge cannot be remote, where it cannot, or why the compiler
    /// could not tell. Empty when it can.
    pub untransferable: Vec<crate::Untransferable>,
    /// **What a remote binding of this edge must prove.**
    ///
    /// Non-empty when the callee's signature carries a restricted value:
    /// `Session<A>` must reach `Session<A>`, and an origin node handling
    /// millions of sessions cannot answer that. Empty is *nothing owed*, not
    /// *nothing known* — an undetermined signature has no obligations because
    /// nobody knows what crosses it.
    pub obligations: Vec<crate::Obligation>,
}

impl Edge {
    /// **Is there any way to bind this edge at all?**
    ///
    /// The conjunction the two independent facts finally make: an edge whose
    /// ends cannot share a node and whose values cannot cross a boundary has no
    /// binding, and the program cannot be deployed on this topology however the
    /// components are assigned.
    ///
    /// `Undetermined` transferability counts as bindable here, deliberately.
    /// The alternative is refusing a deployment because of a typing gap
    /// somewhere in the callee's signature, which is a diagnostic the compiler
    /// owes the author at build time — not a deployment the host refuses with
    /// no idea what to say about it.
    pub fn is_bindable(&self) -> bool {
        self.can_be_local || self.can_be_remote.unwrap_or(true)
    }

    /// **Is this edge bound, as opposed to merely bindable?**
    ///
    /// An edge with an undischarged obligation that CANNOT be co-located is a
    /// candidate, not a plan: something has to prove the binding preserves the
    /// principal, and nothing here can. Co-locatable edges owe nothing, because
    /// a same-process call does not leave the principal's context.
    pub fn is_discharged(&self) -> bool {
        self.can_be_local || self.obligations.is_empty()
    }
}

/// **A deployment plan, or the reasons there is none.**
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Plan {
    pub placements: Vec<Placement>,
    pub edges: Vec<Edge>,
    /// Components no node admits.
    pub unplaceable: Vec<String>,
    /// Imports naming a component the program does not contain.
    pub dangling: Vec<Edge>,
}

impl Plan {
    /// Is there at least one consistent assignment?
    ///
    /// Every component placeable, every component import satisfied, and every
    /// edge bindable some way. Three different failures, all reported — one is
    /// a deployment that cannot host the program, one a program missing a
    /// piece, and one a program whose parts cannot be connected however they
    /// are placed.
    pub fn is_deployable(&self) -> bool {
        self.unplaceable.is_empty()
            && self.dangling.is_empty()
            && self.unbindable().is_empty()
            && self.undischarged().is_empty()
    }

    /// **Edges that owe something no binding here has proved.**
    ///
    /// Architect ruling, 2026-08-08: *"Your planner may keep an
    /// `Undetermined` edge as a candidate, but it must not call a deployment
    /// plan complete until the binding discharges that obligation."*
    ///
    /// A co-locatable edge owes nothing: a same-process call does not leave the
    /// principal's context. What is left is an edge that must be remote and
    /// carries a restricted value, and nothing in this plan proves the binding
    /// preserves the principal — because no binding mechanism has been chosen
    /// yet. When one is, it discharges these.
    pub fn undischarged(&self) -> Vec<&Edge> {
        self.edges.iter().filter(|e| !e.is_discharged()).collect()
    }

    /// **Edges with no binding at all.**
    ///
    /// The conjunction of the two independent facts: the ends cannot share a
    /// node AND the values cannot cross a boundary. Either alone is ordinary —
    /// a necessarily-remote edge over transferable types is a normal RPC, and
    /// a handle-passing edge between co-located components is a normal call.
    pub fn unbindable(&self) -> Vec<&Edge> {
        self.edges.iter().filter(|e| !e.is_bindable()).collect()
    }
}

/// Plan a set of contracts against a topology.
///
/// One `admit` call per (component, node) pair, and no other source of truth.
pub fn plan(contracts: &[ComponentContract], topology: &Topology) -> Plan {
    // What each component exports, by the interface key an importer would use.
    // `contract::contracts` names a component import `pw:app/<component_id>`,
    // so the reverse map is keyed the same way — built from the contracts
    // themselves rather than by reformatting a component id here, because two
    // constructions of one key is how they come to disagree.
    let mut exporters: BTreeMap<String, String> = BTreeMap::new();
    // And the binding support of the export each key names, so an edge can be
    // classified without re-deriving anything: transferability is a compiler
    // fact and arrives in the contract.
    let mut supports: BTreeMap<String, &crate::BindingSupport> = BTreeMap::new();
    for c in contracts {
        for e in &c.exports {
            let key = format!("pw:app/{}#{}", c.component_id, e.name);
            supports.insert(key.clone(), &e.binding);
            exporters.insert(key, c.component_id.clone());
        }
        exporters.insert(format!("pw:app/{}", c.component_id), c.component_id.clone());
        // A whole-component key names the component's exports collectively, so
        // the conservative reading is the one that refuses if ANY of them
        // refuses — an importer naming the component may call any of them.
        if let Some(worst) = c
            .exports
            .iter()
            .map(|e| &e.binding)
            // Strongest constraint wins: an importer naming the whole
            // component may call any of its exports, so the component's answer
            // is the least permissive one among them.
            .min_by_key(|b| match &b.remote {
                crate::RemoteSupport::Refused { .. } => 0,
                crate::RemoteSupport::Undetermined { .. } => 1,
                crate::RemoteSupport::Conditional { .. } => 2,
                crate::RemoteSupport::Transferable => 3,
            })
        {
            supports.insert(format!("pw:app/{}", c.component_id), worst);
        }
    }

    let mut placements = Vec::new();
    let mut unplaceable = Vec::new();

    for c in contracts {
        // The artifact's real imports are not known here — a plan is made
        // before anything is built. `admit` audits them when there is an
        // artifact; passing none asks only the questions a plan can answer,
        // which are placement and capability.
        let mut candidates = Vec::new();
        let mut rejected = Vec::new();
        for node in &topology.nodes {
            match admit(c, topology, &node.name, &[]) {
                Admission::Admit { .. } => candidates.push(node.name.clone()),
                Admission::Refuse(r) => rejected.push((node.name.clone(), r)),
            }
        }
        if candidates.is_empty() {
            unplaceable.push(c.component_id.clone());
        }
        placements.push(Placement {
            component: c.component_id.clone(),
            candidates,
            rejected,
        });
    }

    let by_component: BTreeMap<&str, &Placement> = placements
        .iter()
        .map(|p| (p.component.as_str(), p))
        .collect();

    let mut edges = Vec::new();
    let mut dangling = Vec::new();
    for c in contracts {
        for i in c.imports.iter().filter(|i| i.kind == ImportKind::Component) {
            let target = exporters
                .get(&i.key())
                .or_else(|| exporters.get(&i.interface));
            let Some(target) = target else {
                dangling.push(Edge {
                    from: c.component_id.clone(),
                    to: i.key(),
                    can_be_local: false,
                    // Nothing exports it, so there is no signature to ask.
                    // `None` rather than `false`: the edge is not untransferable,
                    // it is absent, and the plan reports it as `dangling`.
                    can_be_remote: None,
                    untransferable: Vec::new(),
                    obligations: Vec::new(),
                });
                continue;
            };
            // Local iff the two ends share a candidate node. Set intersection
            // over the LOCAL admission answers — the planner composing local
            // facts, which is the whole shape of this module.
            let here: BTreeSet<&str> = by_component
                .get(c.component_id.as_str())
                .map(|p| p.candidates.iter().map(String::as_str).collect())
                .unwrap_or_default();
            let there: BTreeSet<&str> = by_component
                .get(target.as_str())
                .map(|p| p.candidates.iter().map(String::as_str).collect())
                .unwrap_or_default();
            // The callee's own answer about its types. Read from the export the
            // import names, not re-derived: the host cannot see a signature.
            let support = supports
                .get(&i.key())
                .or_else(|| supports.get(&i.interface));
            let mut obligations: Vec<crate::Obligation> = Vec::new();
            let (can_be_remote, untransferable) = match support.map(|s| &s.remote) {
                Some(crate::RemoteSupport::Transferable) => (Some(true), Vec::new()),
                Some(crate::RemoteSupport::Refused { positions }) => {
                    (Some(false), positions.clone())
                }
                // Possible, and owing something. `Some(true)` because the edge
                // CAN be remote — a host that read an obligation as a refusal
                // would force co-location for every session-scoped query.
                Some(crate::RemoteSupport::Conditional { obligations: owed }) => {
                    obligations = owed.clone();
                    (Some(true), Vec::new())
                }
                Some(crate::RemoteSupport::Undetermined { positions }) => (None, positions.clone()),
                // No export entry for this key. A contract from before the
                // field existed reads as the default, which is `Transferable`,
                // so reaching here means the key names nothing — and that is
                // the dangling case, handled above.
                None => (None, Vec::new()),
            };
            edges.push(Edge {
                from: c.component_id.clone(),
                to: target.clone(),
                can_be_local: here.intersection(&there).next().is_some(),
                can_be_remote,
                untransferable,
                obligations,
            });
        }
    }

    edges.sort_by(|a, b| (&a.from, &a.to).cmp(&(&b.from, &b.to)));
    edges.dedup();
    unplaceable.sort();
    unplaceable.dedup();

    Plan {
        placements,
        edges,
        unplaceable,
        dangling,
    }
}
