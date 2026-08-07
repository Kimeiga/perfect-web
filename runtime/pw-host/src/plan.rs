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
//! # What this does not decide
//!
//! Whether an edge between two placed components may be remote. That is
//! boundary transferability — `docs/NEXT.md` step 7 — and it needs the
//! `transfer_profile` work E8's binding design settled. A plan produced here
//! records the edges; it does not yet classify them.
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
    /// `false` means the edge is necessarily remote, which is a real fact about
    /// a deployment and the input transferability will need. It is NOT a
    /// verdict: a remote edge is legitimate, and whether THIS one may be remote
    /// is the question step 7 answers.
    pub can_be_local: bool,
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
    /// Every component placeable, and every component import satisfied. An
    /// unplaceable component and a dangling edge are different failures and
    /// both are reported — one is a deployment that cannot host the program,
    /// the other a program missing a piece.
    pub fn is_deployable(&self) -> bool {
        self.unplaceable.is_empty() && self.dangling.is_empty()
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
    for c in contracts {
        for e in &c.exports {
            exporters.insert(
                format!("pw:app/{}#{}", c.component_id, e.name),
                c.component_id.clone(),
            );
        }
        exporters.insert(format!("pw:app/{}", c.component_id), c.component_id.clone());
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
            edges.push(Edge {
                from: c.component_id.clone(),
                to: target.clone(),
                can_be_local: here.intersection(&there).next().is_some(),
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
