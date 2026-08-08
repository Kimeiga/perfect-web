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

pub mod plan;

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

/// Where an import's implementation comes from, and therefore what constrains
/// it.
///
/// Architect ruling, 2026-08-07:
///
/// > A component import isn't automatically "authority". It is a dependency on
/// > another component whose own authority is independently constrained.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ImportKind {
    #[default]
    HostCapability,
    Component,
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
pub struct Import {
    pub interface: String,
    pub name: String,
    #[serde(default)]
    pub capability: String,
    #[serde(default)]
    pub kind: ImportKind,
}

/// What class an ACTUAL Wasm import falls into.
///
/// The audit classifies rather than flattening, because each class is
/// constrained by a different thing: a host capability by the contract's
/// `required_capabilities` and the node's grants, a component interface by that
/// component's own contract, and a runtime import by nothing at all — which is
/// why the third class exists and is always refused.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ImportClass {
    Host,
    Component,
    /// `wasi:*` and anything else the toolchain injected. Never allowed: see
    /// `docs/evidence/E0/spike-wasmtime-component.txt` F-2.
    Runtime,
}

/// Which class an import falls into, given the contract that should describe it.
///
/// MEMBERSHIP decides, not spelling. The compiler emits `pw:host/…` and
/// `pw:app/…`, but a component built against a hand-written WIT world uses
/// whatever names that world declares — and refusing those on a prefix would
/// make the audit a check on naming rather than on authority.
///
/// The one exception is [`is_runtime`], which no contract can override.
pub fn classify(contract: &ComponentContract, import: &str) -> ImportClass {
    if is_runtime(import) {
        return ImportClass::Runtime;
    }
    for i in &contract.imports {
        if i.key() == import {
            return match i.kind {
                ImportKind::HostCapability => ImportClass::Host,
                ImportKind::Component => ImportClass::Component,
            };
        }
    }
    ImportClass::Runtime
}

/// **What one instance may consume.**
///
/// E8 gate item: *"fuel and memory limits per instance, driven by policy rather
/// than a constant."* E0's `check:fuel` proved wasmtime enforces both; what a
/// spike cannot say is where the number comes from. This is that, and it lives
/// beside `Topology` — part of what a DEPLOYMENT declares, because a budget
/// compiled into the host is a budget nobody can raise for a component that
/// legitimately needs more, and nobody can lower for one that does not.
///
/// `None` is unbounded, and it is a deliberate value rather than a default:
/// `Limits::unbounded()` has to be written, so a caller that wants no ceiling
/// says so.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct Limits {
    /// Execution budget. Exhausting it traps, which is what makes a runaway
    /// guest a bounded cost rather than an outage.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub fuel: Option<u64>,
    /// Ceiling on linear memory, in bytes.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub memory_bytes: Option<usize>,
    /// Ceiling on table elements — the other growable resource, and the one
    /// that gets forgotten. A guest denied memory can still exhaust a host
    /// through indirect-call tables.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub table_elements: Option<usize>,
}

impl Limits {
    /// No ceiling on anything. Written out, never defaulted into.
    pub fn unbounded() -> Limits {
        Limits::default()
    }

    pub fn is_bounded(&self) -> bool {
        self.fuel.is_some() || self.memory_bytes.is_some() || self.table_elements.is_some()
    }
}

/// The store data a limited instance runs with.
///
/// Public because `Store<T>`'s data type is part of the API surface once a
/// caller holds one; the fields are wasmtime's own limiter, unchanged.
#[derive(Debug)]
pub struct Meter {
    pub limits: MemoryLimits,
}

/// Wasmtime's `ResourceLimiter`, driven by [`Limits`].
///
/// The fields are public and readable without the `engine` feature, because a
/// host that has decided a ceiling should be able to report it — the alternative
/// is a limiter whose contents exist only inside the engine, which is the shape
/// that makes "what was this instance allowed?" unanswerable after the fact.
#[derive(Debug, Default)]
pub struct MemoryLimits {
    pub memory_bytes: Option<usize>,
    pub table_elements: Option<usize>,
}

impl From<&Limits> for Meter {
    fn from(l: &Limits) -> Meter {
        Meter {
            limits: MemoryLimits {
                memory_bytes: l.memory_bytes,
                table_elements: l.table_elements,
            },
        }
    }
}

#[cfg(feature = "engine")]
impl wasmtime::ResourceLimiter for MemoryLimits {
    fn memory_growing(
        &mut self,
        _current: usize,
        desired: usize,
        _maximum: Option<usize>,
    ) -> wasmtime::Result<bool> {
        // Refusing growth rather than erroring: the guest sees an allocation
        // failure it can handle, which is a different and better thing than
        // the host aborting. Charter §7.10's direction — a boundary reports
        // rather than crashes.
        Ok(self.memory_bytes.is_none_or(|max| desired <= max))
    }

    fn table_growing(
        &mut self,
        _current: usize,
        desired: usize,
        _maximum: Option<usize>,
    ) -> wasmtime::Result<bool> {
        Ok(self.table_elements.is_none_or(|max| desired <= max))
    }
}

/// Is this an interface the toolchain injected rather than the program asked
/// for?
///
/// `docs/evidence/E0/spike-wasmtime-component.txt` F-2: Rust `std` on
/// `wasm32-wasip2` adds fourteen `wasi:*` interfaces during runtime
/// initialization. No effect row asks for them and no node grants them, so a
/// contract cannot authorise one by listing it — this check runs BEFORE
/// membership for exactly that reason.
pub fn is_runtime(import: &str) -> bool {
    import.starts_with("wasi:")
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
    /// How this edge may be bound. Mirrored by field name, ADR-0018.
    ///
    /// `#[serde(default)]` so a contract written before the compiler emitted
    /// this parses as what the host assumed when there was no field: bindable
    /// either way.
    #[serde(default)]
    pub binding: BindingSupport,
}

/// **What binding modes an interface edge supports**, as the compiler derived
/// it from the signature's types.
///
/// Two independent answers rather than one enum, on the architect's ruling of
/// 2026-08-07: `Either` is just both, and an enum forecloses "remote ✓ only
/// through a host-mediated handle proxy".
///
/// A host may narrow this and must not widen it. Whether an edge SHOULD be
/// remote is the planner's, from placement; whether it CAN be is this, from the
/// types, and the compiler is the only thing that can see them.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
pub struct BindingSupport {
    pub local: LocalSupport,
    pub remote: RemoteSupport,
}

impl Default for BindingSupport {
    fn default() -> Self {
        BindingSupport {
            local: LocalSupport::Direct,
            remote: RemoteSupport::Transferable,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum LocalSupport {
    /// A direct call in one address space. Nothing about a type can forbid it;
    /// whether the two ends may SHARE a node is placement's question, which
    /// `plan` composes with this.
    Direct,
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case", tag = "kind")]
pub enum RemoteSupport {
    /// Every value in the signature can cross, with nothing owed.
    Transferable,
    /// Something in the signature cannot cross at all, and each one is named.
    Refused { positions: Vec<Untransferable> },
    /// **It can cross, IF the binding discharges these obligations.**
    ///
    /// The privacy property belongs to the binding, not to a node: an origin
    /// handles millions of sessions, so "this machine is Session A" is not a
    /// thing that can be true. A plan may keep such an edge as a candidate and
    /// must not call itself complete until something discharges what it owes.
    Conditional { obligations: Vec<Obligation> },
    /// The compiler had no basis to decide — a position whose type that build
    /// could not determine.
    ///
    /// Distinct from `Conditional`: there is nothing for a binding to prove,
    /// because nobody knows what crosses.
    Undetermined { positions: Vec<Untransferable> },
}

/// What a binding must prove before an edge may carry a signature.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case", tag = "kind")]
pub enum Obligation {
    /// The invocation must reach the same principal it left.
    PreservePrincipal {
        principal: String,
        position: String,
        ty: Option<String>,
    },
}

impl RemoteSupport {
    pub fn is_transferable(&self) -> bool {
        matches!(self, RemoteSupport::Transferable)
    }

    /// Could this edge be remote at all, given something to discharge what it
    /// owes? `Conditional` is a yes with a condition, not a no.
    pub fn is_possible(&self) -> bool {
        !matches!(self, RemoteSupport::Refused { .. })
    }

    pub fn obligations(&self) -> &[Obligation] {
        match self {
            RemoteSupport::Conditional { obligations } => obligations,
            _ => &[],
        }
    }
}

/// One position in a signature that cannot cross, and why.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
pub struct Untransferable {
    /// `argument 0`, `result`, `error`.
    pub position: String,
    pub ty: Option<String>,
    pub reason: String,
}

/// The capability→representation mapping this host understands.
///
/// A contract produced under a different mapping is not comparable: the same
/// `database.read<Stores>` may mean something else. Refused rather than
/// interpreted — see [`Refusal::UnknownMapping`].
pub const CAPABILITY_MAPPING: u32 = 1;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ComponentContract {
    pub component_id: String,
    #[serde(default)]
    pub capability_mapping: u32,
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
/// declarative host topology replacing `worlds_for`". That table said which
/// worlds *could* grant a family — a statement about the shape of the web —
/// and it was deleted from the compiler on 2026-08-07 in favour of a
/// `placement` clause on each effect declaration. This is the other half: which
/// capabilities THIS deployment's node has, which is a statement about a
/// machine, and no compiler can know it.
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
    /// The contract was produced under a capability mapping this host does not
    /// know, so its capabilities cannot be compared with this node's grants.
    ///
    /// Refused rather than interpreted. Two mappings can spell one capability
    /// the same way and mean different authority, and a host that guessed would
    /// be guessing about exactly the thing it exists to decide.
    UnknownMapping { found: u32, understood: u32 },
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
            Refusal::UnknownMapping { found, understood } => write!(
                f,
                "the contract uses capability mapping {found}; this host understands {understood}"
            ),
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

    if contract.capability_mapping != CAPABILITY_MAPPING {
        refusals.push(Refusal::UnknownMapping {
            found: contract.capability_mapping,
            understood: CAPABILITY_MAPPING,
        });
    }

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

    // 3. The artifact audit. ADR-0020's rule, applied to the built thing —
    //    and CLASSIFIED, so each class is compared against its own source.
    //
    //    A flattened set would let a component interface satisfy a host
    //    capability slot and vice versa, which is the one confusion the split
    //    exists to prevent.
    let host_allowed: BTreeSet<String> = contract
        .imports
        .iter()
        .filter(|i| i.kind == ImportKind::HostCapability)
        .map(Import::key)
        .collect();
    let component_allowed: BTreeSet<String> = contract
        .imports
        .iter()
        .filter(|i| i.kind == ImportKind::Component)
        .map(Import::key)
        .collect();

    let mut undeclared: Vec<String> = actual
        .iter()
        .filter(|i| {
            // A runtime import is never allowed, whatever a contract says.
            if is_runtime(i) {
                return true;
            }
            // Otherwise: allowed if the contract lists it, IN EITHER CLASS —
            // and the classes are separate sets, so a component interface
            // cannot fill a host capability slot or the reverse.
            !host_allowed.contains(*i) && !component_allowed.contains(*i)
        })
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

/// Exactly what a host may install in a component's linker.
///
/// The bridge between a decision and an instantiation. `docs/evidence/E0//// spike-wasmtime-component.txt` already proved the engine half — a component
/// whose import is withheld from the linker fails to instantiate, with a
/// diagnostic naming the missing interface — so what remained was that the
/// linker's CONTENTS come from the admission rather than from the node.
///
/// Derived from the contract's imports filtered by what was granted, so a host
/// cannot install an interface for a capability the decision refused, and
/// cannot install one the node happens to have.
pub fn linkable(contract: &ComponentContract, granted: &Granted) -> Vec<String> {
    let mut out: Vec<String> = contract
        .imports
        .iter()
        .filter(|i| granted.handle(&i.capability).is_some())
        .map(Import::key)
        .collect();
    out.sort();
    out.dedup();
    out
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

    use std::collections::{BTreeMap, BTreeSet};

    use wasmtime::component::types::ComponentItem;

    /// Every instance a component imports, as `interface#name` where the name
    /// is known and `interface` alone otherwise.
    ///
    /// Errors rather than returning an empty list. An empty list means "imports
    /// nothing", which passes every audit — so a failure to read must never be
    /// able to produce one.
    pub fn imports_of(bytes: &[u8]) -> Result<Vec<String>, String> {
        use wasmtime::component::Component;
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

    /// **Instantiate a component with exactly the capabilities it was granted.**
    ///
    /// E8 gate item: *"typed linking only from a `Granted`. A component whose
    /// contract omits an import fails to instantiate, with the engine's own
    /// diagnostic rather than ours."*
    ///
    /// The linker is populated from [`crate::linkable`] and from nothing else.
    /// That is the whole design:
    ///
    /// ```text
    /// admit()    decides whether this component may run here
    /// Granted    what it therefore holds
    /// linkable() which of its imports that satisfies
    /// this       the linker, and no other source of definitions
    /// ```
    ///
    /// **The engine refuses, not us.** A pre-flight check comparing lists would
    /// be a second implementation of instantiation's own rule, and the two
    /// would agree until a component imported something in a way the list did
    /// not model. An unlinked import is an unresolvable one, and wasmtime says
    /// so in its own words — which is also the better diagnostic, because it
    /// names what the artifact asked for rather than what we expected.
    ///
    /// Returns the linked import keys on success, so a caller can record what
    /// was actually supplied.
    ///
    /// Runs under [`crate::Limits::unbounded`]. Use [`instantiate_within`] to
    /// give an instance a budget.
    pub fn instantiate(
        bytes: &[u8],
        contract: &crate::ComponentContract,
        granted: &crate::Granted,
    ) -> Result<Vec<String>, String> {
        instantiate_within(bytes, contract, granted, &crate::Limits::unbounded()).map(|o| o.linked)
    }

    /// What an instantiation produced, and what it cost.
    #[derive(Debug, Clone, PartialEq, Eq)]
    pub struct Instantiated {
        /// The import keys that were linked, from the granted set and nothing
        /// else.
        pub linked: Vec<String>,
        /// Fuel consumed, where a budget was set.
        pub fuel_used: Option<u64>,
    }

    /// **Instantiate and CALL, with the host answering from behind a handle.**
    ///
    /// The positive control the architect asked for on 2026-08-08:
    ///
    /// > If you don't yet have a positive guest that actually exercises one
    /// > granted host interface, I would add that as the last E8 control.
    ///
    /// Everything else about E8 shows authority being *refused* or *linked*.
    /// This shows it being **used**: the guest calls the host function its
    /// contract permitted, and the value it returns is one only the host could
    /// have supplied. A capability system that never demonstrates a successful
    /// call has only ever been observed saying no.
    ///
    /// `answers` maps `interface#function` to the value the host returns, and
    /// is the host's OWN data — the guest receives the answer, never a handle
    /// to the store behind it.
    ///
    /// Returns what the exported function produced.
    pub fn call_within(
        bytes: &[u8],
        contract: &crate::ComponentContract,
        granted: &crate::Granted,
        limits: &crate::Limits,
        answers: &std::collections::BTreeMap<String, String>,
        export: &str,
        args: &[wasmtime::component::Val],
    ) -> Result<Vec<wasmtime::component::Val>, String> {
        use wasmtime::component::{Component, Linker, Val};
        use wasmtime::{Config, Engine, Store};

        let mut config = Config::new();
        config.wasm_component_model(true);
        config.consume_fuel(limits.fuel.is_some());
        let engine = Engine::new(&config).map_err(|e| e.to_string())?;
        let component = Component::new(&engine, bytes).map_err(|e| e.to_string())?;

        let mut linker: Linker<crate::Meter> = Linker::new(&engine);
        let supplied = crate::linkable(contract, granted);
        let functions = imported_functions(&engine, &component);
        let wanted: BTreeSet<&str> = supplied.iter().map(String::as_str).collect();

        for (interface, funcs) in &functions {
            let here: Vec<&String> = funcs
                .iter()
                .filter(|f| wanted.contains(format!("{interface}#{f}").as_str()))
                .collect();
            if here.is_empty() {
                continue;
            }
            let mut instance = linker
                .instance(interface)
                .map_err(|e| format!("{interface}: {e}"))?;
            for func in here {
                let key = format!("{interface}#{func}");
                let answer = answers.get(&key).cloned();
                instance
                    .func_new(func, move |_, _ty, args, results| {
                        // The host's own value, keyed by what the GUEST asked
                        // for. It never receives the map, only the answer —
                        // `Handle`'s whole point, at the call itself.
                        let asked = match args.first() {
                            Some(Val::String(s)) => s.clone(),
                            _ => String::new(),
                        };
                        if let Some(slot) = results.first_mut() {
                            *slot = match &answer {
                                Some(v) => Val::Option(Some(Box::new(Val::Record(vec![
                                    ("id".to_string(), Val::String(asked)),
                                    ("name".to_string(), Val::String(v.clone())),
                                ])))),
                                None => Val::Option(None),
                            };
                        }
                        Ok(())
                    })
                    .map_err(|e| format!("{key}: {e}"))?;
            }
        }

        let mut store = Store::new(&engine, crate::Meter::from(limits));
        store.limiter(|m| &mut m.limits);
        if let Some(fuel) = limits.fuel {
            store.set_fuel(fuel).map_err(|e| e.to_string())?;
        }
        let instance = linker
            .instantiate(&mut store, &component)
            .map_err(|e| e.to_string())?;

        let func = instance
            .get_func(&mut store, export)
            .ok_or_else(|| format!("no export `{export}`"))?;
        // One result slot. The spike's `lookup` returns a string, and a
        // component's result arity is part of its type — a caller guessing it
        // would be reimplementing the type check the engine already does, and
        // an arity mismatch is reported by `call` in the engine's own words.
        let mut results = vec![Val::Bool(false)];
        func.call(&mut store, args, &mut results)
            .map_err(|e| e.to_string())?;
        Ok(results)
    }

    /// Each imported interface's FUNCTION exports, read from the component.
    ///
    /// One reader, used by both `instantiate_within` and `call_within`: what an
    /// interface exports is the artifact's answer, and asking it twice in two
    /// ways is how the linker and the caller would come to disagree.
    fn imported_functions(
        engine: &wasmtime::Engine,
        component: &wasmtime::component::Component,
    ) -> BTreeMap<String, Vec<String>> {
        let ty = component.component_type();
        let mut out: BTreeMap<String, Vec<String>> = BTreeMap::new();
        for (name, item) in ty.imports(engine) {
            let ComponentItem::ComponentInstance(instance) = item.ty else {
                continue;
            };
            for (func, kind) in instance.exports(engine) {
                if matches!(kind.ty, ComponentItem::ComponentFunc(_)) {
                    out.entry(name.to_string())
                        .or_default()
                        .push(func.to_string());
                }
            }
        }
        out
    }

    /// **[`instantiate`], within a declared budget.**
    ///
    /// E8 gate item: *"fuel and memory limits per instance, driven by policy
    /// rather than a constant."* E0's `check:fuel` proved wasmtime enforces
    /// both; what it could not say is where the number comes from. [`Limits`]
    /// is that, and it travels with the deployment rather than being compiled
    /// in — a limit hard-coded in the host is a limit nobody can tune for a
    /// component that legitimately needs more.
    ///
    /// [`Limits`]: crate::Limits
    pub fn instantiate_within(
        bytes: &[u8],
        contract: &crate::ComponentContract,
        granted: &crate::Granted,
        limits: &crate::Limits,
    ) -> Result<Instantiated, String> {
        use wasmtime::component::{Component, Linker};
        use wasmtime::{Config, Engine, Store};

        let mut config = Config::new();
        config.wasm_component_model(true);
        // Metering is an ENGINE setting, so it is decided here from the policy
        // rather than being on always. An engine that consumed fuel for an
        // unbounded instance would charge for something nobody bounded.
        config.consume_fuel(limits.fuel.is_some());
        let engine = Engine::new(&config).map_err(|e| e.to_string())?;
        let component = Component::new(&engine, bytes).map_err(|e| e.to_string())?;

        let mut linker: Linker<crate::Meter> = Linker::new(&engine);
        let supplied = crate::linkable(contract, granted);

        // **Grouped by interface, and only the FUNCTIONS.**
        //
        // A linker instance is created once per interface — the spike's
        // `perfect-web:store/stores` exports both `read` and the `store` record,
        // and defining the instance twice is an error. So is defining a
        // function for `store`, which is a type: what an interface exports is
        // read from the COMPONENT's own type here, so the set of things to
        // define comes from the artifact rather than from the contract's idea
        // of it.
        let ty = component.component_type();
        let mut functions: BTreeMap<String, Vec<String>> = BTreeMap::new();
        for (name, item) in ty.imports(&engine) {
            let ComponentItem::ComponentInstance(instance) = item.ty else {
                continue;
            };
            for (func, kind) in instance.exports(&engine) {
                if matches!(kind.ty, ComponentItem::ComponentFunc(_)) {
                    functions
                        .entry(name.to_string())
                        .or_default()
                        .push(func.to_string());
                }
            }
        }

        // One definition per linkable import, and the value behind a capability
        // is never handed over — the guest gets a function that calls back into
        // the host, exactly as `Handle` describes. The bodies are stubs here
        // because what this proves is the LINKING rule; `add_to_cart` running
        // for real is the next gate item and needs the command path.
        let wanted: BTreeSet<&str> = supplied.iter().map(String::as_str).collect();
        for (interface, funcs) in &functions {
            // Decided before the instance is created, so an interface with
            // nothing granted gets no instance at all rather than an empty one
            // the engine might accept.
            let here: Vec<&String> = funcs
                .iter()
                .filter(|f| wanted.contains(format!("{interface}#{f}").as_str()))
                .collect();
            if here.is_empty() {
                continue;
            }
            let mut instance = linker
                .instance(interface)
                .map_err(|e| format!("{interface}: {e}"))?;
            for func in here {
                let name = func.clone();
                instance
                    .func_new(func, move |_, _ty, _args, results| {
                        // **A stub body, and what it stands in for matters.**
                        // A capability's VALUE lives on the host and never
                        // enters the guest's memory — `Handle` is the whole
                        // design. What this proves is the LINKING rule: an
                        // import the granted set covers gets a definition, and
                        // one it does not gets none.
                        //
                        // `option<T>` because that is what the spike's
                        // `stores.read` returns. A differently shaped result
                        // fails when CALLED, not when instantiated, which is
                        // the right boundary for a rule about instantiation.
                        let _ = &name;
                        for slot in results.iter_mut() {
                            *slot = wasmtime::component::Val::Option(None);
                        }
                        Ok(())
                    })
                    .map_err(|e| format!("{interface}#{func}: {e}"))?;
            }
        }

        let mut store = Store::new(&engine, crate::Meter::from(limits));
        store.limiter(|m| &mut m.limits);
        if let Some(fuel) = limits.fuel {
            store.set_fuel(fuel).map_err(|e| e.to_string())?;
        }

        // **The refusal, and it is the engine's.** An import with no definition
        // is unresolvable here, in wasmtime's own words.
        linker
            .instantiate(&mut store, &component)
            .map_err(|e| e.to_string())?;

        let fuel_used = limits.fuel.and_then(|budget| {
            store
                .get_fuel()
                .ok()
                .map(|left| budget.saturating_sub(left))
        });
        Ok(Instantiated {
            linked: supplied,
            fuel_used,
        })
    }
}
