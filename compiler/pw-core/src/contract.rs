//! E8-0 — what the compiler hands the host, frozen.
//!
//! Architect ruling, 2026-08-07:
//!
//! > The compiler decides what authority code needs. The host decides whether
//! > that authority physically exists. Neither should reconstruct the other's
//! > answer.
//!
//! and, on the shape of the boundary:
//!
//! > Freeze the compiler→host `ComponentContract` — component_id, abi_schema,
//! > required_capabilities, allowed_placements, imports, exports. Rule: actual
//! > Wasm imports ⊆ statically allowed set, never a superset. E8 consumes
//! > capabilities; it does not define effect semantics.
//!
//! # What this module is careful NOT to do
//!
//! It does not say what a capability *means*. `database.read<Stores>` is a
//! name derived from an effect row; whether a `database.read` exists, what it
//! connects to, and whether this deployment has one are the host's questions.
//! A compiler that answered them would be a second deployment topology, and
//! the two would disagree the first time one of them changed.
//!
//! It also does not invent placements. `allowed_placements` is E5's solver
//! output — the worlds that can satisfy what this component does — and nothing
//! here re-derives it from the capability list.
//!
//! # The rule, and why it is a subset
//!
//! ```text
//! actual Wasm imports  ⊆  the contract's allowed imports
//! ```
//!
//! **Subset, never superset.** A component that imports fewer host functions
//! than it is allowed to is fine: dead code, a branch never compiled in, a
//! capability declared for a sibling. A component that imports even one it was
//! not allowed is not a component with a small mistake — it is a component
//! whose authority the compiler never approved, and the host has no basis for
//! deciding whether it should have it.
//!
//! The audit is **fail-closed**. An import the audit cannot parse counts as
//! undeclared, because "I could not tell what this asks for" and "this asks
//! for nothing" must never produce the same verdict.
//!
//! # A data artifact
//!
//! ADR-0018: the contract is JSON. The compiler emits it and the host reads
//! it; neither links the other. The host mirrors these types by field name,
//! exactly as `pw-materialize` mirrors the graph and `pw-resource` mirrors
//! `EntryIdentitySpec`.

use std::collections::{BTreeMap, BTreeSet};

use serde::{Deserialize, Serialize};

use crate::effects::Inference;
use crate::hir::{DeclKind, Expr, Hir};
use crate::ontology::Ontology;
use crate::placement::{Demand, solve};
use crate::privacy::Label;
use crate::resolve::Workspace;
use crate::signatures::Signatures;

/// One capability a component needs, as the compiler derived it.
///
/// Structured rather than a string, because the host has to make decisions on
/// the parts. `database.read<Stores>` and `database.write<Stores>` differ in
/// the operation; `database.read<Stores>` and `database.read<Payments>` differ
/// in what they reach. A host that had to re-parse a string to see that would
/// be reimplementing this derivation, badly, at the point where it matters
/// most.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
pub struct Capability {
    /// `database`, `secret`, `dom`. The restricted family E5's world table
    /// keys on.
    pub family: String,
    /// `read`, `write`, `mutate`. Empty when the effect names only a family.
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub operation: String,
    /// The type argument, where the effect carries one: `Stores` in
    /// `database.read<Stores>`.
    ///
    /// Part of the capability's IDENTITY, not a decoration. E2D recorded what
    /// happens when it is dropped: `secret<Payments>` and `secret<Sessions>`
    /// collapse into one, and a component authorised for one reaches the
    /// other.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub argument: Option<String>,
}

/// The version of the capability→representation mapping this build used.
///
/// Architect ruling, 2026-08-07:
///
/// > E9 is free later to change the inference algorithm, source notation,
/// > internal effect-row representation and polymorphism machinery without
/// > changing `CapabilityId(DatabaseRead, Stores)` — unless E9 actually proves
/// > our semantic capability ontology itself was wrong.
///
/// Recorded so a host can tell a mapping change from an authority change. Two
/// contracts with different mapping versions are not comparable; two with the
/// same version and different capabilities are a real difference.
pub const CAPABILITY_MAPPING: u32 = 1;

/// Why an effect could not be turned into a capability.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum NotACapability {
    /// The family is not one this platform restricts. Not an error — an
    /// unrestricted effect simply needs no host authority.
    Unrestricted { family: String },
    /// The type argument names nothing the program declares.
    ///
    /// Refused rather than kept as a string: `database.read<Stores>` and
    /// `database.read<Stroes>` would otherwise be two different capabilities,
    /// one of which nothing will ever grant, and the failure would appear at
    /// deployment as "the node does not grant `database.read<Stroes>`".
    UnknownArgument { family: String, argument: String },
    /// **Nothing in the program declares this effect at all.**
    ///
    /// Distinct from `Unrestricted`, and the distinction is the point: one is
    /// "declared, and needs no authority", the other is "there is no such
    /// effect here". Collapsing them is what a fallback does — it turns *I do
    /// not know* into *no restriction*, which is the shape that made
    /// `secret<Payments>` placeable in the browser.
    Undeclared { effect: String },
}

impl Capability {
    /// **Build a capability from an effect, against what the program declares.**
    ///
    /// The architect's requirement that capability identity come "from
    /// resolved platform declarations, not from parsing strings out of
    /// effect-row syntax".
    ///
    /// There was a second entry point — `Capability::resolve(effect, types)`,
    /// which passed `Ontology::default()` — and deleting the `worlds_for`
    /// fallback made it vacuous: an empty ontology declares nothing, so every
    /// call could only answer [`NotACapability::Undeclared`]. It survived
    /// exactly as long as there was a table underneath to answer for it, which
    /// is the whole argument against having had one.
    ///
    /// **Whether an effect needs host authority is a DECLARED fact.** Architect
    /// ruling, 2026-08-07:
    ///
    /// > An ordinary Pleris operation like `layout.measure` doesn't necessarily
    /// > need to give arbitrary application code a raw DOM capability. […]
    /// > rather than granting every browser component unrestricted DOM
    /// > authority merely because it performs UI work.
    ///
    /// `World::worlds_for` answered a different question — where an effect is
    /// *meaningful* — and reading its restricted-family list as "needs a
    /// capability" is what made `layout.measure` ask a host to grant the layout
    /// engine. **The declaration is now the only thing that decides.**
    ///
    /// There was a fallback here, and deleting it is the substance of the
    /// architect's step 6:
    ///
    /// > ontology doesn't know effect → diagnostic already exists → contract
    /// > generation is Blocked. No legacy interpretation.
    ///
    /// The fallback read `worlds_for` when nothing declared the effect, so an
    /// undeclared spelling the old table happened to recognise still produced a
    /// capability. It was the conservative direction and it was still split
    /// semantic authority: an effect this program never declared got authority
    /// from a table in the compiler. An unresolved effect now yields
    /// [`NotACapability::Undeclared`], which a caller must handle as a refusal
    /// rather than as "unrestricted".
    pub fn resolve(
        effect: &str,
        declared_types: &BTreeSet<String>,
        ontology: &Ontology,
    ) -> Result<Capability, NotACapability> {
        let c = Capability::parse(effect);
        match ontology.declared_for(effect) {
            Some(decl) if decl.capability.is_none() => {
                return Err(NotACapability::Unrestricted { family: c.family });
            }
            Some(_) => {}
            None => {
                return Err(NotACapability::Undeclared {
                    effect: effect.to_string(),
                });
            }
        }
        if let Some(a) = &c.argument
            && !declared_types.contains(a)
        {
            return Err(NotACapability::UnknownArgument {
                family: c.family.clone(),
                argument: a.clone(),
            });
        }
        Ok(c)
    }

    /// Parse `family.operation<Argument>` as an effect row writes it.
    ///
    /// Not the construction path a contract uses — see [`Capability::resolve`].
    /// Kept public because a HOST reading a contract has only the canonical
    /// text and must be able to recover the parts.
    pub fn parse(effect: &str) -> Capability {
        let (head, argument) = match effect.split_once('<') {
            Some((h, rest)) => (h, Some(rest.trim_end_matches('>').to_string())),
            None => (effect, None),
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
    }

    /// The canonical text form, and the one an import names.
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

/// A host function this component is allowed to import, and what authorises it.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
pub struct Import {
    /// The interface, in the Wasm component naming the host will use.
    pub interface: String,
    /// The function within it.
    pub name: String,
    /// Which required capability makes this import legitimate — for a HOST
    /// import only.
    ///
    /// Present so the audit's refusal can say *why*: "this module imports
    /// `pw:host/database#read`, and nothing in its effect row asks for
    /// `database.read`" is actionable; "undeclared import" is not.
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub capability: String,
    /// What kind of dependency this is.
    ///
    /// Architect ruling, 2026-08-07:
    ///
    /// > A component import isn't automatically "authority". It is a dependency
    /// > on another component whose own authority is independently constrained.
    ///
    /// A page reading `query Store(id)` depends on the `Store` component; it
    /// does not thereby acquire that component's `database.read`. Flattening
    /// the two into one set would make every caller of a privileged component
    /// look privileged.
    #[serde(default)]
    pub kind: ImportKind,
}

/// Where an import's implementation comes from, and therefore what constrains
/// it.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ImportKind {
    /// Supplied by the host or environment. Constrained by
    /// `required_capabilities` and by what the node grants.
    #[default]
    HostCapability,
    /// An export of another application component. Constrained by that
    /// component's own contract, which the host checks independently.
    Component,
}

impl Import {
    /// The wire form the audit compares: `interface#name`.
    pub fn key(&self) -> String {
        format!("{}#{}", self.interface, self.name)
    }
}

/// Something the component provides to the host.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
pub struct Export {
    pub name: String,
    /// The declaration kind it came from: `query`, `command`, `page`.
    pub kind: String,
    /// **How this edge may be bound.** Architect ruling, 2026-08-07:
    ///
    /// > Remote capability is a property of an interface edge, not a component:
    /// > `get(StoreId) -> Result<Store, StoreError>` is probably remotable and
    /// > `with_transaction(fn(OpenTransaction) -> T)` is not, in the same
    /// > component.
    ///
    /// Which is why it is here and not a seventh field on the contract. A
    /// per-component answer would have to be the conjunction over its exports,
    /// making a whole component local because one function holds a handle.
    ///
    /// `#[serde(default)]` so a contract written before this field parses: it
    /// reads as `local: direct, remote: transferable`, which is what the host
    /// assumed when there was no field at all.
    #[serde(default)]
    pub binding: crate::binding::BindingSupport,
}

/// **What the compiler tells the host about one component.**
///
/// Frozen at E8-0: E8 may consume every field and must add none. A host that
/// needed a seventh field would be asking the compiler a question the compiler
/// has no business answering, or answering one itself that it should be asking.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ComponentContract {
    /// The component's semantic identity: its module path.
    pub component_id: String,
    /// Which capability→representation mapping produced this contract.
    ///
    /// Six semantic fields plus one about the artifact itself. A host reading a
    /// mapping version it does not know must refuse rather than interpret the
    /// capabilities under its own — see [`CAPABILITY_MAPPING`].
    #[serde(default = "default_mapping")]
    pub capability_mapping: u32,
    /// A hash over the component's INTERFACE — its exports, their kinds, its
    /// capabilities and its placements. Not over source text: a comment must
    /// not change it, and a new capability must.
    pub abi_schema: String,
    pub required_capabilities: Vec<Capability>,
    /// The worlds E5's solver found can satisfy this component.
    ///
    /// Empty means *nowhere* — a component that cannot run anywhere, which the
    /// host must refuse rather than place somewhere and hope.
    pub allowed_placements: Vec<String>,
    pub imports: Vec<Import>,
    pub exports: Vec<Export>,
}

fn default_mapping() -> u32 {
    CAPABILITY_MAPPING
}

/// What an audit of a built artifact concluded.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Audit {
    /// Every actual import is in the allowed set.
    Satisfied,
    /// At least one is not. Carries all of them, because a build that fixes
    /// the first and rediscovers the second is a slow way to learn the shape
    /// of a problem.
    Undeclared(Vec<String>),
}

impl Audit {
    pub fn is_satisfied(&self) -> bool {
        matches!(self, Audit::Satisfied)
    }
}

/// **The rule.** Actual imports ⊆ allowed imports.
///
/// Fail-closed by construction: the allowed set is what the contract lists, and
/// anything not in it is undeclared. There is no "unknown interface" branch to
/// forget, because there is no lookup — only membership.
pub fn audit(contract: &ComponentContract, actual: &[String]) -> Audit {
    let allowed: BTreeSet<String> = contract.imports.iter().map(Import::key).collect();
    let mut undeclared: Vec<String> = actual
        .iter()
        .filter(|i| !allowed.contains(*i))
        .cloned()
        .collect();
    undeclared.sort();
    undeclared.dedup();
    if undeclared.is_empty() {
        Audit::Satisfied
    } else {
        Audit::Undeclared(undeclared)
    }
}

/// Is this declaration something a host runs?
///
/// A type or an import declaration has no authority to describe, and a plain
/// `fn` is compiled into its callers rather than instantiated on its own.
fn component_kind(kind: DeclKind) -> Option<&'static str> {
    Some(match kind {
        DeclKind::Query => "query",
        DeclKind::Command => "command",
        DeclKind::Page => "page",
        DeclKind::Component => "component",
        _ => return None,
    })
}

/// The other components this declaration's body calls.
///
/// Resolved, so `Cart` in one module and `Cart` in another are two
/// dependencies — the same requirement `docs/RISK_QUEUE.md` 34 is about.
fn component_calls(
    inference: &Inference<'_>,
    unit: usize,
    hir: &Hir,
    id: crate::hir::DeclId,
    components: &BTreeMap<crate::resolve::DefId, (String, String)>,
) -> Vec<Import> {
    let Some(body_id) = hir.decl(id).body else {
        return Vec::new();
    };
    let body = hir.body(body_id);
    // Calls, and the resources a body READS.
    //
    // `let menu = query Menu(id)` is not a `Call` — it is the keyword form the
    // dependency graph already knows how to find, so `graph::queried` is reused
    // rather than reimplemented. A second finder would agree until one of them
    // learned about a new form.
    let mut paths: Vec<String> = body
        .walk()
        .into_iter()
        .filter_map(|expr| match body.expr(expr) {
            Expr::Call { callee, .. } => Some(crate::infer::path_of(body, *callee)),
            _ => None,
        })
        .collect();
    paths.extend(
        crate::graph::queried(body)
            .into_iter()
            .map(|(name, _)| name),
    );

    let mut out = Vec::new();
    for path in paths {
        if path.is_empty() {
            continue;
        }
        // The COMPONENT of that name, preferentially.
        //
        // `store.page` declares `query Store` and imports `domain.Store`, and
        // the general resolver tries the type namespace first — which is how
        // `docs/RISK_QUEUE.md` recorded every page dependency resolving to a
        // record definition. Here the question is specifically "which component
        // does this call", so the module-qualified form is tried first and the
        // general resolution is the fallback.
        let qualified = if path.contains('.') {
            None
        } else {
            hir.module_of(id)
                .map(|m| format!("{m}.{path}"))
                .and_then(|q| inference.resolved_from(unit, &q))
                .filter(|d| components.contains_key(d))
        };
        let Some(def) = qualified.or_else(|| inference.resolved_from(unit, &path)) else {
            continue;
        };
        // Itself is not a dependency: a recursive query does not import its
        // own export, and recording it would make every host wire a component
        // to itself.
        if def == (crate::resolve::DefId { unit, decl: id.0 }) {
            continue;
        }
        if let Some((component, name)) = components.get(&def) {
            out.push(Import {
                interface: format!("pw:app/{component}"),
                name: name.clone(),
                capability: String::new(),
                kind: ImportKind::Component,
            });
        }
    }
    out.sort();
    out.dedup();
    out
}

/// The host interface and function a capability is served by.
///
/// **From the effect's `host` clause**, which is where the mapping is declared:
///
/// ```pleris
/// effect database.read<T> {
///     capability database.read<T>
///     host       "pw:host/database#read"
/// }
/// ```
///
/// It was `format!("pw:host/{family}")` with the operation as the function
/// name, which happens to produce the identical string for every effect whose
/// declaration follows that shape — `database.read` and `database.write` among
/// them, which is why `docs/evidence/E8/component-contracts.json` does not
/// change. `secret<Payments>` is where they differ: formatted it is
/// `pw:host/secret#use`, and the platform's module is `secrets` with a `get`.
///
/// The convention remains the fallback for an effect nothing declares, so a
/// program checked without the platform packages keeps its imports rather than
/// silently losing them.
///
/// Still deliberately shallow: the compiler says which interface a capability
/// would be served by, and the host decides whether it has one. What changed is
/// only where the name comes from.
fn interface_for(capability: &Capability, ontology: &Ontology) -> (String, String) {
    if let Some(host) = ontology
        .declared_for(&capability.name())
        .and_then(|d| d.host.as_deref())
    {
        // `interface#function`, WIT's own separator. Split here because this is
        // the boundary that reads the clause — the same discipline `EffectPath`
        // applies to the dot.
        if let Some((interface, function)) = host.split_once('#') {
            return (interface.to_string(), function.to_string());
        }
        return (host.to_string(), "use".to_string());
    }
    let function = if capability.operation.is_empty() {
        "use".to_string()
    } else {
        capability.operation.clone()
    };
    (format!("pw:host/{}", capability.family), function)
}

/// Derive every component's contract from a checked program.
///
/// One pass, from the same signatures and the same placement solver every other
/// analysis uses. Nothing here re-derives an effect row or a world.
pub fn contracts(hirs: &[&Hir], sigs: &Signatures, ws: &Workspace) -> Vec<ComponentContract> {
    // INFERRED effects, not declared ones.
    //
    // A query's authority is in its body: `Menu` writes no effect row and
    // calls a helper that reads the database, and the host has to be told
    // about that read. A contract built from declared rows would hand the most
    // ordinary component in the language an empty capability set and let it
    // import whatever it liked — the audit would pass, because the allowed set
    // it compared against was the wrong one.
    let mut inference = Inference::new(sigs, ws);
    inference.run(hirs);

    // What each effect DECLARES about itself: whether it needs host authority,
    // and which interface serves it. Built once, beside the inference, because
    // both answer questions about the same rows.
    let ontology = Ontology::build_with(hirs, ws);

    // What the program says about each TYPE — is it a resource, is it produced
    // only by a scoped declaration. The same facts `resume.rs` asks about a
    // capture, asked here about a signature: one boundary-transfer analysis,
    // two policies over it.
    let type_facts = crate::boundary::TypeFacts::build(hirs, sigs);

    // Every type the program declares, for resolving a capability's argument.
    // A misspelled `database.read<Stroes>` must be refused here rather than
    // becoming a capability nothing will ever grant.
    let declared_types: BTreeSet<String> = hirs
        .iter()
        .flat_map(|h| h.all_decls().map(|(_, d)| d).collect::<Vec<_>>())
        .filter(|d| matches!(d.kind, DeclKind::Type | DeclKind::Opaque))
        .map(|d| d.name.clone())
        .collect();

    // Which declarations ARE components, by resolved identity. A call to one
    // is a component dependency; a call to a plain `fn` is not — a helper is
    // compiled into its caller and has no separate contract to depend on.
    let components: BTreeMap<crate::resolve::DefId, (String, String)> = hirs
        .iter()
        .enumerate()
        .flat_map(|(unit, hir)| {
            hir.all_decls()
                .filter(|(_, d)| component_kind(d.kind).is_some())
                .map(|(id, d)| {
                    let module = hir.module_of(id).unwrap_or_default().to_string();
                    let path = if module.is_empty() {
                        d.name.clone()
                    } else {
                        format!("{module}.{}", d.name)
                    };
                    (
                        crate::resolve::DefId { unit, decl: id.0 },
                        (path, d.name.clone()),
                    )
                })
                .collect::<Vec<_>>()
        })
        .collect();

    let mut out = Vec::new();
    for (unit, hir) in hirs.iter().enumerate() {
        for (id, decl) in hir.all_decls() {
            // Components are the things a host runs: the units with behaviour.
            // A type or an import declaration has no authority to describe.
            let Some(kind) = component_kind(decl.kind) else {
                continue;
            };

            // ONE CONTRACT PER DECLARATION, not per module.
            //
            // Least authority. A module holding a database query and a browser
            // component would otherwise give the component the query's
            // `database.read` and the query the component's `dom.mutate` — and
            // then E5's solver would find nowhere either could run, because the
            // union of their demands is unsatisfiable. Aggregating by module
            // over-grants and under-places at the same time.
            let module = hir.module_of(id).unwrap_or_default().to_string();
            let component_id = if module.is_empty() {
                decl.name.clone()
            } else {
                format!("{module}.{}", decl.name)
            };

            // What this component performs, EXCLUDING its resumable handlers.
            //
            // A handler is a separately loaded unit with its own identity —
            // E7-L made that concrete — so its authority is its own. A page
            // that renders `on:press={.. => add_to_cart(..)}` would otherwise
            // require `database.write` to RENDER, and a host granting it would
            // give the render path authority it never uses.
            //
            // The handler's own authority is not lost: the command it calls is
            // itself a component with its own contract, so `add_to_cart`'s
            // `database.write` is recorded once, against the thing that
            // performs it.
            let effects: Vec<String> = match decl.body {
                Some(body_id) => {
                    let body = hir.body(body_id);
                    // Only the DEFERRED body, not the whole lambda.
                    //
                    // Architect ruling, 2026-08-07:
                    //
                    //   effects of reading/building captures -> render context
                    //   effects inside handler invocation    -> handler
                    //
                    // A lambda's descriptor — `resumable(captures = { .. })` —
                    // is evaluated while the page renders: the captures are
                    // read, typed and serialized then. Excluding it would let a
                    // page perform an effect at render time and attribute it to
                    // a button nobody has pressed.
                    let lambdas: Vec<_> = body
                        .walk()
                        .into_iter()
                        .filter_map(|e| match body.expr(e) {
                            Expr::Lambda { body: inner, .. } => Some(*inner),
                            _ => None,
                        })
                        .collect();
                    inference.effective_effects_excluding(unit, hir, id, &lambdas)
                }
                // No body: an interface, or platform-external code. One
                // operation decides, for both branches.
                None => inference.effective_effects(unit, hir, id),
            };

            let capabilities: Vec<Capability> = effects
                .iter()
                .filter_map(
                    |e| match Capability::resolve(e, &declared_types, &ontology) {
                        Ok(c) => Some(c),
                        // An unrestricted family needs no host authority. This is
                        // the only reason an effect may leave no capability behind.
                        Err(NotACapability::Unrestricted { .. }) => None,
                        // An unresolvable argument KEEPS the capability.
                        //
                        // Dropping it would remove authority the program asked for,
                        // and the whole point of the direction argument is that
                        // over-stating is refused work while under-stating is
                        // authority nobody approved. The mistake surfaces at
                        // deployment as "the node does not grant
                        // `database.read<Stroes>`", which is a bad diagnostic —
                        // making it a build-time one is the next step and is
                        // recorded in `docs/NEXT.md`, not silently absorbed here.
                        Err(NotACapability::UnknownArgument { .. }) => Some(Capability::parse(e)),
                        // An effect nothing declares KEEPS its capability too,
                        // for the same reason and one more: the contract must
                        // not come out looking like a component that needs
                        // nothing. It cannot be deployed regardless —
                        // `allowed_placements` is empty below, because
                        // placement is `Blocked` for the same effect — and the
                        // build has already reported the row.
                        Err(NotACapability::Undeclared { .. }) => Some(Capability::parse(e)),
                    },
                )
                .collect::<BTreeSet<_>>()
                .into_iter()
                .collect();

            // Placements from E5's solver, given the same demand every other
            // caller builds. Not re-derived from the capability list: the
            // solver also weighs privacy labels, and a second derivation here
            // would agree until a label mattered.
            let demand = Demand {
                effects: effects.clone(),
                label: Label::public(),
                declared: None,
            };
            // A `Blocked` effect leaves this EMPTY, and empty is the contract's
            // existing word for "no node may run this". The host refuses it
            // everywhere, which is what "contract generation is Blocked" means
            // in the artifact's own vocabulary — no new field, and no way to
            // read an undeclared effect as an unconstrained one.
            let allowed_placements: Vec<String> = solve(&demand, &ontology)
                .feasible
                .into_iter()
                .map(|w| w.name().to_string())
                .collect();

            // Host authority, and component dependencies, in one list but
            // never one CLASS. A page that calls a privileged query depends on
            // it; it does not acquire its authority.
            let mut imports: BTreeSet<Import> = capabilities
                .iter()
                .map(|c| {
                    let (interface, name) = interface_for(c, &ontology);
                    Import {
                        interface,
                        name,
                        capability: c.name(),
                        kind: ImportKind::HostCapability,
                    }
                })
                .collect();
            for dep in component_calls(&inference, unit, hir, id, &components) {
                imports.insert(dep);
            }
            let imports: Vec<Import> = imports.into_iter().collect();

            // What this component provides. One entry today, because a
            // declaration is the unit; the field is plural because grouping
            // several declarations into one instantiable component is E8's
            // decision to make, not something to foreclose here.
            //
            // The binding support is per EXPORT for the same reason: remote
            // capability is a property of an interface edge, so a component
            // with a remotable query and a handle-passing helper says so about
            // each rather than about itself.
            // From the DECLARATION, not from `Signatures`. That lookup returned
            // nothing for a `page`, `view` or `component` — `Signatures` covers
            // `fn`, `query`, `command`, `subscription`, `resource` and `task` —
            // so every page export fell through to the permissive default and
            // came out `Transferable`. The right answer for this corpus,
            // produced by a mechanism unrelated to its types.
            let binding = crate::binding::binding_support(decl, &type_facts);
            let exports = vec![Export {
                name: decl.name.clone(),
                kind: kind.to_string(),
                binding,
            }];

            let abi_schema = schema_of(&component_id, &exports, &capabilities, &allowed_placements);
            out.push(ComponentContract {
                component_id,
                capability_mapping: CAPABILITY_MAPPING,
                abi_schema,
                required_capabilities: capabilities,
                allowed_placements,
                imports,
                exports,
            });
        }
    }
    out.sort_by(|a, b| a.component_id.cmp(&b.component_id));
    out
}

/// A hash over the interface, and over nothing else.
///
/// Exports, their kinds, the capabilities and the placements. Not the bodies:
/// a component whose implementation changed but whose authority and interface
/// did not is the same contract, and a host that reloaded on every edit would
/// be reacting to noise. Not the source text either — see
/// `resume_artifacts`'s scheme 1 for what that costs.
fn schema_of(
    module: &str,
    exports: &[Export],
    capabilities: &[Capability],
    placements: &[String],
) -> String {
    let mut text = String::from(module);
    for e in exports {
        text.push_str(&format!("|{}:{}", e.kind, e.name));
    }
    for c in capabilities {
        text.push_str(&format!("|cap:{}", c.name()));
    }
    for p in placements {
        text.push_str(&format!("|at:{p}"));
    }
    hash(&text)
}

fn hash(content: &str) -> String {
    let mut h: u64 = 0xcbf2_9ce4_8422_2325;
    for b in content.as_bytes() {
        h ^= *b as u64;
        h = h.wrapping_mul(0x0000_0100_0000_01b3);
    }
    format!("{h:016x}")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_capability_keeps_its_type_argument() {
        // E2D's lesson, at the boundary this time. Dropping the argument
        // collapses `secret<Payments>` and `secret<Sessions>` into one
        // capability, and a component authorised for one reaches the other.
        let a = Capability::parse("secret<Payments>");
        let b = Capability::parse("secret<Sessions>");
        assert_ne!(a, b);
        assert_eq!(a.family, "secret");
        assert_eq!(a.argument.as_deref(), Some("Payments"));
        assert_eq!(a.name(), "secret<Payments>");
    }

    #[test]
    fn an_operation_is_part_of_the_identity() {
        assert_ne!(
            Capability::parse("database.read<Stores>"),
            Capability::parse("database.write<Stores>")
        );
    }

    #[test]
    fn a_bare_family_parses() {
        let c = Capability::parse("secret");
        assert_eq!(c.family, "secret");
        assert!(c.operation.is_empty());
        assert_eq!(c.name(), "secret");
    }
}
