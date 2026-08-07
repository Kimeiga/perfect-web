//! E8 — the host decides whether authority physically exists.
//!
//! Architect ruling, 2026-08-07:
//!
//! > The compiler decides what authority code needs. The host decides whether
//! > that authority physically exists. Neither should reconstruct the other's
//! > answer.
//!
//! So this crate answers exactly one question, in three parts:
//!
//! ```text
//! may this component be instantiated on this node?
//!
//!   1. does the node grant every capability the contract requires?   topology
//!   2. is the node's world one the contract allows?                  placement
//!   3. does the artifact import only what the contract allows?       audit
//! ```
//!
//! and it never asks a fourth. It does not infer effects, it does not solve
//! placements, and it does not decide what a component needs — those are
//! answered in `pw_core::contract` and arrive here as a JSON artifact.
//!
//! # Placement is not capability
//!
//! Two checks, not one, because they can disagree in both directions. A node in
//! the right world may lack a capability (an edge node with no cache attached);
//! a node with every capability may be in the wrong world (an origin process
//! asked to run browser-only DOM code). Collapsing them would make one of those
//! two mistakes silently allowed, and which one depends on which way the
//! collapse went.
//!
//! # No ambient authority
//!
//! `docs/evidence/E0/spike-wasmtime-component.txt` measured what this is
//! defending against. A guest built with Rust `std` for `wasm32-wasip2`
//! declares ONE import in its WIT world and the compiled component demands
//! **fifteen** — fourteen `wasi:*` interfaces the world never mentioned,
//! injected by std's runtime initialization. Every one is authority nobody
//! granted, and it arrives without a single line of application code asking
//! for it.
//!
//! That is why the audit is on the ARTIFACT rather than on the source. The
//! compiler can only tell you what the program says it needs.
//!
//! # Handles, not secrets
//!
//! A granted capability yields a [`Handle`] — an opaque reference the host can
//! revoke and account for. The value behind it never enters the guest's memory
//! through this crate.

use std::collections::{BTreeMap, BTreeSet};

use serde::{Deserialize, Serialize};

// --- the compiler's artifact, mirrored --------------------------------------
//
// ADR-0018/ADR-0020: field-name mirrors of `pw_core::contract`. This crate does
// not link the compiler, and the compiler does not link this crate. A rename on
// either side fails in `tests/contract_mirror.rs` rather than becoming an empty
// capability set at instantiation time.

/// One capability a component needs, as the compiler derived it.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
pub struct Capability {
    pub family: String,
    #[serde(default)]
    pub operation: String,
    #[serde(default)]
    pub argument: Option<String>,
}

impl Capability {
    /// The canonical text form. Must match `pw_core::contract::Capability::name`
    /// exactly — a second spelling is a second capability.
    pub fn name(&self) -> String {
        let mut out = self.family.clone();
        if !self.operation.is_empty() {
            out.push('.');
            out.push_str(&self.operation);
        }
        if let Some(a) = &self.argument {
            out.push('<');
            out.push_str(a);
            out.push('>');
        }
        out
    }
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
pub struct Import {
    pub interface: String,
    pub name: String,
    pub capability: String,
}

impl Import {
    pub fn key(&self) -> String {
        format!("{}#{}", self.interface, self.name)
    }
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
pub struct Export {
    pub name: String,
    pub kind: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ComponentContract {
    pub component_id: String,
    pub abi_schema: String,
    pub required_capabilities: Vec<Capability>,
    pub allowed_placements: Vec<String>,
    pub imports: Vec<Import>,
    pub exports: Vec<Export>,
}

impl ComponentContract {
    pub fn from_json(text: &str) -> Result<Vec<ComponentContract>, String> {
        serde_json::from_str(text).map_err(|e| e.to_string())
    }
}

// --- the deployment, declared -----------------------------------------------

/// One place code can run, and what it actually has.
///
/// **Declarative, and the host's own.** The architect's E8 list calls for "a
/// declarative host topology replacing `worlds_for`", and this is the half that
/// belongs here: `worlds_for` says which worlds *could* grant a family, which
/// is a statement about the shape of the web. This says which capabilities THIS
/// deployment's node has, which is a statement about a machine — and no
/// compiler can know it.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Node {
    /// The node's name, for diagnostics.
    pub name: String,
    /// Which world this node is: `build`, `browser`, `edge`, `origin`.
    pub world: String,
    /// The capabilities physically available here, in canonical form.
    ///
    /// `database.read<Stores>`, not `database`. A node with a read-only replica
    /// grants the first and not `database.write<Stores>`, and a topology that
    /// could only speak in families could not express that.
    pub grants: BTreeSet<String>,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct Topology {
    pub nodes: Vec<Node>,
}

impl Topology {
    pub fn from_json(text: &str) -> Result<Topology, String> {
        serde_json::from_str(text).map_err(|e| e.to_string())
    }

    pub fn node(&self, name: &str) -> Option<&Node> {
        self.nodes.iter().find(|n| n.name == name)
    }
}

// --- the decision -----------------------------------------------------------

/// Why a component may not be instantiated here.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Refusal {
    /// The node's world is not one the contract allows.
    WrongWorld { node: String, world: String },
    /// The node does not physically have a capability the contract requires.
    Ungranted { node: String, capability: String },
    /// The artifact imports something the contract does not allow.
    ///
    /// Distinct from `Ungranted` on purpose: one says the deployment cannot
    /// supply what the program asked for, the other says the ARTIFACT asked for
    /// something the program never did. The second is far more serious — it
    /// means the built thing and the checked thing are not the same thing.
    UndeclaredImport { imports: Vec<String> },
    /// The contract can run nowhere at all.
    Unplaceable,
}

impl std::fmt::Display for Refusal {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Refusal::WrongWorld { node, world } => {
                write!(
                    f,
                    "node `{node}` is a {world} node, which this component does not allow"
                )
            }
            Refusal::Ungranted { node, capability } => {
                write!(f, "node `{node}` does not grant `{capability}`")
            }
            Refusal::UndeclaredImport { imports } => write!(
                f,
                "the artifact imports {} which its contract does not allow: {}",
                imports.len(),
                imports.join(", ")
            ),
            Refusal::Unplaceable => {
                f.write_str("this component's demands can be satisfied by no world")
            }
        }
    }
}

/// What the host concluded.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Admission {
    /// Instantiate it, granting exactly these capabilities and no others.
    Admit { grants: Vec<String> },
    /// Do not. Every reason, not the first — a deployment that fixes one and
    /// rediscovers the next is a slow way to learn the shape of a problem.
    Refuse(Vec<Refusal>),
}

impl Admission {
    pub fn is_admitted(&self) -> bool {
        matches!(self, Admission::Admit { .. })
    }

    pub fn refusals(&self) -> &[Refusal] {
        match self {
            Admission::Admit { .. } => &[],
            Admission::Refuse(r) => r,
        }
    }
}

/// **The whole decision.**
///
/// `actual` is what the built artifact really imports — read from the component
/// itself, not from anything the compiler said. Pass an empty slice only when
/// there is no artifact yet; an empty import list from a component that has
/// imports would silently pass the audit, so the caller reading them must fail
/// loudly rather than default to none.
pub fn admit(
    contract: &ComponentContract,
    topology: &Topology,
    node_name: &str,
    actual: &[String],
) -> Admission {
    let mut refusals = Vec::new();

    if contract.allowed_placements.is_empty() {
        refusals.push(Refusal::Unplaceable);
    }

    let node = topology.node(node_name);
    match node {
        None => refusals.push(Refusal::WrongWorld {
            node: node_name.to_string(),
            world: "unknown".to_string(),
        }),
        Some(n) => {
            // 1. Placement.
            if !contract.allowed_placements.contains(&n.world) {
                refusals.push(Refusal::WrongWorld {
                    node: n.name.clone(),
                    world: n.world.clone(),
                });
            }
            // 2. Capability. A separate check on separate data — see the module
            //    header for why collapsing the two hides a real mistake.
            for cap in &contract.required_capabilities {
                if !n.grants.contains(&cap.name()) {
                    refusals.push(Refusal::Ungranted {
                        node: n.name.clone(),
                        capability: cap.name(),
                    });
                }
            }
        }
    }

    // 3. The artifact audit. ADR-0020's rule, applied to the built thing.
    let allowed: BTreeSet<String> = contract.imports.iter().map(Import::key).collect();
    let mut undeclared: Vec<String> = actual
        .iter()
        .filter(|i| !allowed.contains(*i))
        .cloned()
        .collect();
    undeclared.sort();
    undeclared.dedup();
    if !undeclared.is_empty() {
        refusals.push(Refusal::UndeclaredImport {
            imports: undeclared,
        });
    }

    if refusals.is_empty() {
        // EXACTLY what the contract requires, and nothing the node happens to
        // have. A host that granted its whole capability set to every component
        // it admitted would make the contract advisory.
        Admission::Admit {
            grants: contract
                .required_capabilities
                .iter()
                .map(Capability::name)
                .collect(),
        }
    } else {
        Admission::Refuse(refusals)
    }
}

// --- capabilities, as handles ------------------------------------------------

/// An opaque reference to something the host holds.
///
/// The guest receives this, never the value behind it. A connection string, a
/// signing key or a session token placed in guest memory is authority the host
/// can no longer withdraw, cannot account for, and cannot keep out of a crash
/// dump — and E7's resume manifests already established that a capability must
/// never be a value the client can carry.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub struct Handle {
    capability: String,
    slot: u32,
}

impl Handle {
    pub fn capability(&self) -> &str {
        &self.capability
    }

    pub fn slot(&self) -> u32 {
        self.slot
    }
}

/// `Debug` never prints what a handle refers to, because a handle does not know.
impl std::fmt::Display for Handle {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "handle({}, slot {})", self.capability, self.slot)
    }
}

/// What one admitted instance may reach.
///
/// Built from an [`Admission`], so an instance cannot hold a capability the
/// decision did not grant: there is no constructor that takes a capability list
/// directly.
#[derive(Debug, Default)]
pub struct Granted {
    handles: BTreeMap<String, Handle>,
    /// The values, held HERE. Never handed out.
    values: BTreeMap<String, String>,
}

impl Granted {
    /// Build the instance's capability set from an admission and the node's
    /// backing values.
    ///
    /// Returns `None` for a refusal. There is deliberately no way to build a
    /// `Granted` from a refused admission — an "override" parameter is how a
    /// capability system becomes a logging system.
    pub fn from(admission: &Admission, backing: &BTreeMap<String, String>) -> Option<Granted> {
        let Admission::Admit { grants } = admission else {
            return None;
        };
        let mut out = Granted::default();
        for (slot, capability) in grants.iter().enumerate() {
            out.handles.insert(
                capability.clone(),
                Handle {
                    capability: capability.clone(),
                    slot: slot as u32,
                },
            );
            if let Some(v) = backing.get(capability) {
                out.values.insert(capability.clone(), v.clone());
            }
        }
        Some(out)
    }

    /// The handle for a capability, if this instance was granted it.
    pub fn handle(&self, capability: &str) -> Option<&Handle> {
        self.handles.get(capability)
    }

    /// Use a capability, by handle.
    ///
    /// The host performs the work and returns the result; the value behind the
    /// handle stays here. `None` when the handle is not one this instance
    /// holds — including a handle forged from another instance's, because
    /// membership is checked rather than the handle being trusted to describe
    /// itself.
    pub fn use_handle(&self, handle: &Handle) -> Option<&str> {
        let held = self.handles.get(&handle.capability)?;
        if held != handle {
            return None;
        }
        self.values.get(&handle.capability).map(String::as_str)
    }

    pub fn count(&self) -> usize {
        self.handles.len()
    }
}

// --- reading a real artifact -------------------------------------------------

#[cfg(feature = "engine")]
pub mod engine {
    //! What a built component actually imports.
    //!
    //! Read from the artifact with `wasmtime`, because the point of the audit
    //! is to catch what the source does not say — `docs/evidence/E0/spike-\
    //! wasmtime-component.txt` measured a guest whose WIT world declares one
    //! import and whose component demands fifteen.

    /// Every instance a component imports, as `interface#name` where the name
    /// is known and `interface` alone otherwise.
    ///
    /// Errors rather than returning an empty list. An empty list means "imports
    /// nothing", which passes every audit — so a failure to read must never be
    /// able to produce one.
    pub fn imports_of(bytes: &[u8]) -> Result<Vec<String>, String> {
        use wasmtime::component::Component;
        use wasmtime::component::types::ComponentItem;
        use wasmtime::{Config, Engine};

        let mut config = Config::new();
        config.wasm_component_model(true);
        let engine = Engine::new(&config).map_err(|e| e.to_string())?;
        let component = Component::new(&engine, bytes).map_err(|e| e.to_string())?;
        let ty = component.component_type();

        let mut out = Vec::new();
        for (name, item) in ty.imports(&engine) {
            match item.ty {
                ComponentItem::ComponentInstance(instance) => {
                    let mut any = false;
                    for (func, _) in instance.exports(&engine) {
                        out.push(format!("{name}#{func}"));
                        any = true;
                    }
                    if !any {
                        out.push(name.to_string());
                    }
                }
                _ => out.push(name.to_string()),
            }
        }
        out.sort();
        out.dedup();
        Ok(out)
    }
}
