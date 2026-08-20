//! **A WIT world per component, generated from what the compiler already
//! decided.**
//!
//! Charter §14 M8 task 2, and the E8 gate item:
//!
//! > Generate a WIT world per `ComponentContract`: the world's imports are
//! > exactly `contract.imports`, and `wit-bindgen` accepts it.
//!
//! `imports_are_exactly_the_contracts_imports` is that criterion as a test, and
//! `wit_parser_resolves_the_generated_package` is the second half — validated
//! by the parser `wasm-tools` and `wit-bindgen` are built on, not by a reader
//! written here. A second WIT parser in this repo would be the shape the
//! project keeps deleting, and it would be a particularly bad instance: the
//! whole point of emitting WIT is that somebody else's toolchain reads it.
//!
//! # What comes from where
//!
//! ```text
//! world name, imports     the ComponentContract          authority
//! export signatures       the declaration's Interface    the same one binding.rs reads
//! type definitions        the program's type decls       records, variants, aliases
//! ```
//!
//! The contract is the authority boundary (ADR-0018/ADR-0020) and carries no
//! types, which is correct — a host deciding whether to instantiate does not
//! need them. WIT does, so this reads the program. What it must NOT do is
//! re-derive an import or a placement, and
//! `imports_are_exactly_the_contracts_imports` is what says it does not.
//!
//! # Refusal, not invention
//!
//! A type this generator cannot map produces [`WitError::Unmappable`] and no
//! output. The alternative is a plausible guess — `s64` for anything numeric,
//! `string` for anything opaque — which produces a world that parses, links,
//! and decodes a value into something it never was. Charter §7.10 is about
//! exactly that boundary.
//!
//! Name collisions are the same: WIT identifiers are kebab-case, so
//! `store.page.Cart` and `store.page.cart` mangle to one name. That is refused
//! rather than resolved, because silently merging two identities is
//! `docs/RISK_QUEUE.md` 34 in a new place.

use std::collections::{BTreeMap, BTreeSet};

use crate::binding::Interface;
use crate::contract::{ComponentContract, ImportKind};
use crate::hir::{Decl, DeclKind, Hir};
use crate::resolve::{Namespace, Resolution, Workspace};

/// The package version every generated world carries.
///
/// Fixed rather than derived: a world's version says which SHAPE of world this
/// is, and deriving it from a program would make every edit look like a
/// protocol change. `CAPABILITY_MAPPING` is the field that tracks meaning.
pub const PACKAGE: &str = "pw:app@0.1.0";

/// Why a world could not be generated.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum WitError {
    /// A type in an exported signature has no WIT form this generator knows.
    Unmappable { ty: String, at: String },
    /// Two distinct names mangle to one WIT identifier.
    Collision {
        wit: String,
        first: String,
        second: String,
    },
    /// **Two declarations claim one `interface#operation`.**
    ///
    /// An operation has one ABI, so two claimants are two ABIs for one name —
    /// exactly what `contract::abi` refuses when the second one comes from an
    /// artifact. Reported rather than resolved by order, because "whichever was
    /// met last" is an answer that looks like every other answer.
    Claimed { operation: String, count: usize },
}

impl std::fmt::Display for WitError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            WitError::Unmappable { ty, at } => {
                write!(f, "`{ty}` at {at} has no WIT form")
            }
            WitError::Collision { wit, first, second } => {
                write!(f, "`{first}` and `{second}` both mangle to `{wit}`")
            }
            WitError::Claimed { operation, count } => {
                write!(f, "{count} declarations claim `{operation}`")
            }
        }
    }
}

/// **A WIT identifier from a Pleris name.**
///
/// `store.page.StorePage` → `store-page-store-page`. Lowercase kebab, because
/// that is what WIT accepts; the dots and the case are what a Pleris name uses
/// to distinguish things, and both are lost here — which is why every caller
/// checks for a collision rather than trusting the mangling.
pub fn ident(name: &str) -> String {
    let mut out = String::new();
    let mut prev_lower = false;
    for ch in name.chars() {
        match ch {
            '.' | '_' | ' ' | '/' | ':' => {
                if !out.ends_with('-') && !out.is_empty() {
                    out.push('-');
                }
                prev_lower = false;
            }
            c if c.is_ascii_uppercase() => {
                if prev_lower && !out.ends_with('-') {
                    out.push('-');
                }
                out.push(c.to_ascii_lowercase());
                prev_lower = false;
            }
            c if c.is_ascii_alphanumeric() => {
                out.push(c);
                prev_lower = c.is_ascii_lowercase() || c.is_ascii_digit();
            }
            _ => {}
        }
    }
    let trimmed = out.trim_matches('-').to_string();
    // A WIT identifier may not start with a digit, and an empty one is not an
    // identifier at all.
    if trimmed.is_empty() {
        "x".to_string()
    } else if trimmed.starts_with(|c: char| c.is_ascii_digit()) {
        format!("x-{trimmed}")
    } else {
        trimmed
    }
}

/// Every name that must survive mangling, checked for collisions in one place.
fn no_collisions(names: impl IntoIterator<Item = String>) -> Result<(), WitError> {
    let mut seen: BTreeMap<String, String> = BTreeMap::new();
    for name in names {
        let wit = ident(&name);
        if let Some(first) = seen.get(&wit)
            && *first != name
        {
            return Err(WitError::Collision {
                wit,
                first: first.clone(),
                second: name,
            });
        }
        seen.insert(wit, name);
    }
    Ok(())
}

/// What the program declares about each type, in the form WIT needs.
///
/// **Keyed by qualified path, never by bare name.** `capability.SessionId` and
/// `domain.SessionId` are two declarations of two different types, and the
/// first version of this keyed by `d.name` — so both became `session-id` and
/// `wit-parser` refused the package for a duplicate definition. It refused; a
/// generator that had deduplicated instead would have exported one type where
/// the program has two, which is `docs/RISK_QUEUE.md` 34 on a wire.
///
/// Which of the two a signature means is a resolution question, so `Types` is
/// built with the workspace and every reference goes through it.
#[derive(Debug, Default)]
pub struct Types {
    /// Declaration order preserved, so the emitted package is stable.
    defs: Vec<TypeDef>,
    /// Qualified path → the WIT identifier it is emitted under.
    known: BTreeMap<String, String>,
    /// `DefId` → qualified path, so a resolved reference finds its definition.
    by_def: BTreeMap<crate::resolve::DefId, String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
enum TypeDef {
    /// `type Store = Store { id: Int }` → a WIT record.
    Record {
        name: String,
        /// Where it was declared, so its field types resolve from there.
        unit: usize,
        fields: Vec<(String, String)>,
    },
    /// A sum type → a WIT variant. Payload-free cases become `enum`-shaped
    /// variants, which WIT allows and which keeps one construct for both.
    Variant {
        name: String,
        unit: usize,
        cases: Vec<(String, Vec<String>)>,
    },
    /// `opaque type StoreId = String`.
    ///
    /// Emitted as an ALIAS of its representation, and that is a real decision:
    /// a WIT boundary has no opacity to offer, so the choice is between
    /// exposing the representation and refusing to export anything that uses
    /// an opaque type. `types::opaque_types_do_not_expose_their_representation`
    /// is about the Pleris type system, where the distinction is enforced;
    /// once a value is on a wire there is nothing left to enforce it with, and
    /// pretending otherwise would be the lie.
    Alias {
        name: String,
        unit: usize,
        of: String,
    },
}

impl Types {
    /// Collect every type the program declares, by resolved identity.
    pub fn build(hirs: &[&Hir]) -> Types {
        let mut out = Types::default();
        for (unit, hir) in hirs.iter().enumerate() {
            for (id, d) in hir.all_decls() {
                if !matches!(d.kind, DeclKind::Type | DeclKind::Opaque) {
                    continue;
                }
                let module = hir.module_of(id).unwrap_or_default();
                let path = if module.is_empty() {
                    d.name.clone()
                } else {
                    format!("{module}.{}", d.name)
                };
                out.by_def
                    .insert(crate::resolve::DefId { unit, decl: id.0 }, path.clone());
                match d.kind {
                    DeclKind::Opaque => {
                        let Some(of) = &d.opaque_of else { continue };
                        out.known.insert(path.clone(), ident(&path));
                        out.defs.push(TypeDef::Alias {
                            name: path,
                            of: of.clone(),
                            unit,
                        });
                    }
                    _ => out.push_type(&path, unit, d),
                }
            }
        }
        out
    }

    fn push_type(&mut self, path: &str, unit: usize, d: &Decl) {
        if let Some(variants) = &d.variants
            && variants.len() > 1
        {
            self.known.insert(path.to_string(), ident(path));
            self.defs.push(TypeDef::Variant {
                name: path.to_string(),
                unit,
                cases: variants
                    .iter()
                    .map(|v| (v.name.clone(), v.fields.clone()))
                    .collect(),
            });
            return;
        }
        // One variant, or none: a record. `type Store = Store { id: Int }` is
        // the corpus's shape and the fields live on the single variant or on
        // the declaration.
        // `f.ty` is the HEAD and `f.ty_args` its arguments, so a field written
        // `List<MenuItem>` is `("List", ["MenuItem"])` and reading only the
        // head produces `list` with nothing in it. `wit-parser` caught that on
        // the store demo the first time this ran, which is exactly why the
        // real parser decides here instead of a reader written in this repo.
        let fields = d
            .fields
            .as_ref()
            .map(|fs| {
                fs.iter()
                    .filter_map(|f| f.ty.as_ref().map(|t| (f.name.clone(), t.written())))
                    .collect::<Vec<_>>()
            })
            .unwrap_or_default();
        self.known.insert(path.to_string(), ident(path));
        self.defs.push(TypeDef::Record {
            name: path.to_string(),
            unit,
            fields,
        });
    }

    /// **The qualified path a written type name means, from where it is
    /// written.**
    ///
    /// Resolution, not spelling. `SessionId` in `store.page` and `SessionId` in
    /// `web.capability` are two types, and which one a signature means is the
    /// workspace's answer — the same one every other analysis asks for. A
    /// generator matching on the bare name would pick whichever it saw first.
    fn resolve(&self, ws: &Workspace, unit: usize, name: &str) -> Option<&str> {
        match ws.resolve_in(unit, Namespace::Type, name) {
            Resolution::Local(def) | Resolution::Imported { def, .. } => {
                self.by_def.get(&def).map(String::as_str)
            }
            _ => None,
        }
    }

    /// Every DECLARED type inside a written type, `Result<Store, E>` included,
    /// as qualified paths.
    ///
    /// Recursive over the carriers, because `use types.{result}` is not a thing
    /// and `use types.{store}` is — a signature naming `Result<Store, E>`
    /// touches `Store` and `E`, not `Result`.
    fn declared_within(
        &self,
        ws: &Workspace,
        unit: usize,
        written: &str,
        out: &mut BTreeSet<String>,
    ) {
        let (head, args) = split(written);
        if let Some(path) = self.resolve(ws, unit, head) {
            out.insert(path.to_string());
        }
        for a in args {
            self.declared_within(ws, unit, a, out);
        }
    }
}

impl TypeDef {
    fn name(&self) -> &str {
        match self {
            TypeDef::Record { name, .. }
            | TypeDef::Variant { name, .. }
            | TypeDef::Alias { name, .. } => name,
        }
    }

    fn unit(&self) -> usize {
        match self {
            TypeDef::Record { unit, .. }
            | TypeDef::Variant { unit, .. }
            | TypeDef::Alias { unit, .. } => *unit,
        }
    }

    /// The types this definition mentions, as written.
    fn referenced(&self) -> Vec<String> {
        match self {
            TypeDef::Record { fields, .. } => fields.iter().map(|(_, t)| t.clone()).collect(),
            TypeDef::Variant { cases, .. } => cases.iter().flat_map(|(_, f)| f.clone()).collect(),
            TypeDef::Alias { of, .. } => vec![of.clone()],
        }
    }
}

/// A written type into its head and arguments.
fn split(written: &str) -> (&str, Vec<&str>) {
    match written.split_once('<') {
        Some((h, rest)) => (
            h.trim(),
            rest.trim_end_matches('>')
                .split(',')
                .map(str::trim)
                .filter(|s| !s.is_empty())
                .collect(),
        ),
        None => (written.trim(), Vec::new()),
    }
}

/// **A Pleris type name as WIT.**
///
/// The table is explicit and short on purpose. Everything not in it and not
/// declared by the program is [`WitError::Unmappable`] — a generator that fell
/// back to `string` would produce a world that parses and decodes a value into
/// something it never was.
fn wit_type(
    ty: &str,
    types: &Types,
    ws: &Workspace,
    unit: usize,
    at: &str,
) -> Result<String, WitError> {
    let (head, args) = split(ty);
    let mapped = |a: &str| wit_type(a, types, ws, unit, at);
    Ok(match (head, args.as_slice()) {
        ("Int", []) => "s64".to_string(),
        ("Float", []) => "f64".to_string(),
        ("Bool", []) => "bool".to_string(),
        ("String" | "Str", []) => "string".to_string(),
        ("Unit", []) => "_".to_string(),
        ("List", [a]) => format!("list<{}>", mapped(a)?),
        ("Option", [a]) => format!("option<{}>", mapped(a)?),
        ("Result", [a, e]) => format!("result<{}, {}>", mapped(a)?, mapped(e)?),
        ("Result", [a]) => format!("result<{}>", mapped(a)?),
        // **A privacy qualifier is TRANSPARENT.** Architect ruling, 2026-08-20:
        //
        // > Privacy qualification is semantic metadata; it need not have an
        // > independent runtime representation.
        // >
        // >     AbiRepresentation(Session<T>) = Transparent(AbiRepresentation(T))
        //
        // So `Session<SessionId>` crosses as whatever `SessionId` crosses as,
        // and the SEMANTIC contract keeps the restriction that WIT never sees —
        // which is correct, because WIT could not prove `Session<A> → Session<B>`
        // anyway. That stays Pleris contract semantics, as capabilities stay
        // outside ordinary core Wasm types.
        //
        // The qualifier set comes from `labels::label_of_type`, the one place
        // that says what a qualifier is, so this is not a second list.
        //
        // **This is not "an opaque type is its representation".** Opacity and
        // ABI transparency are different facts, and the arm below still refuses
        // a generic opaque type that is not a qualifier — see
        // `a_generic_opaque_type_that_is_not_a_qualifier_still_has_no_wit_form`.
        (h, [inner]) if crate::labels::is_privacy_qualifier(h) => mapped(inner)?,
        (h, []) => match types.resolve(ws, unit, h) {
            Some(path) => ident(path),
            None => {
                return Err(WitError::Unmappable {
                    ty: ty.to_string(),
                    at: at.to_string(),
                });
            }
        },
        _ => {
            return Err(WitError::Unmappable {
                ty: ty.to_string(),
                at: at.to_string(),
            });
        }
    })
}

/// One exported declaration's signature, as WIT, plus the declared types it
/// names.
///
/// The second half is not decoration: a WIT interface referring to a type from
/// another interface must `use` it by name, so a signature that does not report
/// what it touched produces a package that does not resolve.
/// One signature as WIT.
///
/// `ident` is the operation's WIT identifier and is **already mangled** —
/// the caller decides, because it is the caller that knows where the name came
/// from. An export's name is a Pleris name and needs [`ident`]; a host
/// operation's name came out of `host "store:data/menus#for-store"` and is a
/// WIT identifier the author wrote, which mangling would turn into `forstore`.
fn wit_func(
    ident: &str,
    sig: &Interface,
    types: &Types,
    ws: &Workspace,
    unit: usize,
) -> Result<(String, BTreeSet<String>), WitError> {
    let name = ident;
    let at = format!("`{name}`");
    let mut used: BTreeSet<String> = BTreeSet::new();
    let mut params: Vec<String> = Vec::new();
    for (i, p) in sig.params.iter().enumerate() {
        let Some(ty) = p else {
            return Err(WitError::Unmappable {
                ty: format!("argument {i}"),
                at: at.clone(),
            });
        };
        params.push(format!("arg{i}: {}", wit_type(ty, types, ws, unit, &at)?));
        types.declared_within(ws, unit, ty, &mut used);
    }
    let ret = match sig.returns.as_deref() {
        None => String::new(),
        Some(head) => {
            let written = if sig.returns_args.is_empty() {
                head.to_string()
            } else {
                format!("{head}<{}>", sig.returns_args.join(", "))
            };
            types.declared_within(ws, unit, &written, &mut used);
            format!(" -> {}", wit_type(&written, types, ws, unit, &at)?)
        }
    };
    Ok((format!("{ident}: func({}){ret};", params.join(", ")), used))
}

/// One component's world.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct World {
    pub name: String,
    /// The interfaces this world imports, sorted and deduplicated.
    ///
    /// **Exactly `contract.imports`, projected onto interfaces.** A WIT world
    /// imports an interface, not a function, so two host imports naming
    /// `pw:host/database` are one line. A component dependency becomes this
    /// package's own interface for that component — `store-page-cart-api` —
    /// because a world cannot import a world. The projection is the only
    /// transformation, and
    /// `imports_are_exactly_the_contracts_imports` asserts it inverts.
    pub imports: Vec<String>,
    /// The name of the interface this world exports.
    pub exports: String,
}

/// The interface name for a component's exports.
///
/// Suffixed because a world and an interface share one namespace in a WIT
/// package, and every component needs both.
fn api(component_id: &str) -> String {
    format!("{}-api", ident(component_id))
}

/// **Generate the WIT package for a checked program.**
///
/// One `world` per contract, plus one `interface types` holding every type the
/// exported signatures name.
pub fn package(
    hirs: &[&Hir],
    ws: &Workspace,
    contracts: &[ComponentContract],
) -> Result<(String, Vec<World>), WitError> {
    let types = Types::build(hirs);
    no_collisions(types.known.keys().cloned())?;
    no_collisions(contracts.iter().map(|c| c.component_id.clone()))?;

    // The declaration behind each contract, by the path the contract names.
    // Contracts are keyed by `module.Name`, which is how `contracts()` builds a
    // component id — read rather than reconstructed, for the same reason the
    // planner reads the exporter map rather than reformatting an id.
    let mut decls: BTreeMap<String, (usize, &Decl)> = BTreeMap::new();
    for (unit, hir) in hirs.iter().enumerate() {
        for (id, d) in hir.all_decls() {
            let module = hir.module_of(id).unwrap_or_default();
            let path = if module.is_empty() {
                d.name.clone()
            } else {
                format!("{module}.{}", d.name)
            };
            decls.insert(path, (unit, d));
        }
    }

    let mut worlds = Vec::new();
    let mut apis: Vec<Api> = Vec::new();
    for c in contracts {
        let mut imports: Vec<String> = c
            .imports
            .iter()
            .map(|i| match i.kind {
                // A host capability's interface is named by the host package,
                // verbatim: `pw:host/database` is a reference into somebody
                // else's WIT and nothing here may rewrite it.
                ImportKind::HostCapability => i.interface.clone(),
                // A component dependency is this package's own interface for
                // that component. `pw:app/store.page.Cart` is not a WIT
                // identifier and a world cannot import a world; both are fixed
                // by naming the interface the exporter provides.
                ImportKind::Component => {
                    api(i.interface.strip_prefix("pw:app/").unwrap_or(&i.interface))
                }
            })
            .collect();
        imports.sort();
        imports.dedup();

        // One contract per DECLARATION, so the export's signature is the
        // component's own declaration — looked up by the path `contracts()`
        // built the id from, not reconstructed.
        let (unit, sig) = match decls.get(&c.component_id) {
            Some((u, d)) => (*u, Interface::of(d)),
            None => (0, Interface::default()),
        };
        let mut funcs = Vec::new();
        let mut used: BTreeSet<String> = BTreeSet::new();
        for e in &c.exports {
            let (f, u) = wit_func(&ident(&e.name), &sig, &types, ws, unit)?;
            funcs.push(f);
            used.extend(u);
        }
        apis.push(Api {
            name: api(&c.component_id),
            uses: used,
            funcs,
        });

        worlds.push(World {
            name: ident(&c.component_id),
            imports,
            exports: api(&c.component_id),
        });
    }

    // **The host packages, generated from the Pleris declarations.**
    //
    // Only the operations some contract actually imports: a program does not
    // publish an interface for a `host`-bound `fn` nothing reaches, any more
    // than it exports a component nothing instantiates.
    let wanted: BTreeSet<String> = contracts
        .iter()
        .flat_map(|c| &c.imports)
        .filter(|i| i.kind == ImportKind::HostCapability)
        .map(|i| i.key())
        .collect();
    let mut hosts: BTreeMap<String, HostPackage> = BTreeMap::new();
    let mut host_uses: BTreeSet<String> = BTreeSet::new();
    for (unit, hir) in hirs.iter().enumerate() {
        for (_, d) in hir.all_decls() {
            let Some(id) = crate::backend::host_binding(d) else {
                continue;
            };
            if !wanted.contains(&id.qualified()) {
                continue;
            }
            // `store:data/carts` → package `store:data`, interface `carts`.
            let Some((pkg, iface)) = id.interface.split_once('/') else {
                continue;
            };
            let sig = Interface::of(d);
            let (text, used) = wit_func(&id.name, &sig, &types, ws, unit)?;
            host_uses.extend(used.iter().cloned());
            let entry = hosts.entry(pkg.to_string()).or_insert_with(|| HostPackage {
                name: pkg.to_string(),
                interfaces: BTreeMap::new(),
            });
            let slot = entry
                .interfaces
                .entry(iface.to_string())
                .or_insert_with(|| (BTreeSet::new(), Vec::new()));
            slot.0.extend(used);
            slot.1.push(text);
        }
    }
    for h in hosts.values_mut() {
        for (_, funcs) in h.interfaces.values_mut() {
            funcs.sort();
            funcs.dedup();
        }
    }
    let hosts: Vec<HostPackage> = hosts.into_values().collect();

    // The type package must reach what the HOST signatures name too, not only
    // what the exports do — `capability.SessionId` is reachable from
    // `pw:host/session#read` and from nothing else.
    let seeds: Vec<String> = apis
        .iter()
        .flat_map(|a| a.uses.iter().cloned())
        .chain(host_uses)
        .collect();
    let type_text = render_types(&types, ws, &reachable(&types, ws, seeds))?;
    Ok((render(&type_text, &apis, &worlds, &hosts), worlds))
}

/// **Every host operation's WIT signature, as its Pleris declaration implies
/// it**, keyed by `interface#name`.
///
/// Rendered through exactly the machinery an export goes through —
/// `Interface::of` on the declaration, then [`wit_func`] — so the ABI a host
/// operation is given here and the ABI an exported query is given are one
/// derivation, not two that agree.
///
/// # Why this exists before it is used to emit anything
///
/// A host operation's ABI is currently derived **twice**: here, from the Pleris
/// `fn` carrying the `host` binding, and again by the deployment, in the WIT
/// package it publishes. Nothing compares them, and in this repo all six of the
/// store's operations disagree — see `tests/canonical_abi.rs`.
///
/// Which derivation is authoritative is a model question with the architect,
/// and it turns on [`crate::contract::Ownership`]: an operation the application
/// declared is arguably one the compiler should publish, while a platform
/// facility is one the compiler must conform to. This function is what **both**
/// answers need — to emit the WIT, or to compare against a published one — so
/// building it does not presume either.
///
/// The declaration is found by [`crate::backend::host_binding`], the one reader
/// of the `host` policy, so an operation this names and an operation the
/// backend imports cannot come apart.
///
/// # Per operation, not all-or-nothing
///
/// The result is an outcome **per operation**, because one that has no WIT form
/// must not hide the five that do — the same reason the artifact audit reports
/// every undeclared import rather than the first. It is not an `Option` either:
/// absent and refused would then be the same answer, and one of them is a
/// finding.
///
/// `pw:host/session#read` is currently the refused one, and what it refuses is
/// worth reading: it returns `Session<SessionId>`, a **privacy label**, and a
/// label has no ABI. See `tests/canonical_abi.rs`.
pub fn host_signatures(
    hirs: &[&Hir],
    ws: &Workspace,
) -> BTreeMap<String, Result<String, WitError>> {
    let types = Types::build(hirs);
    // Collected before rendering, because a second claimant is a defect about
    // the OPERATION and not about either declaration's signature — and
    // `out.insert` on a duplicate key answers with whichever came last, which
    // is how the effect declarations' empty signatures once overwrote the real
    // ones.
    let mut claims: BTreeMap<String, Vec<(usize, &Decl)>> = BTreeMap::new();
    for (unit, hir) in hirs.iter().enumerate() {
        for (_, d) in hir.all_decls() {
            if let Some(id) = crate::backend::host_binding(d) {
                claims.entry(id.qualified()).or_default().push((unit, d));
            }
        }
    }

    let mut out = BTreeMap::new();
    for (operation, claimants) in claims {
        let name = operation
            .rsplit('#')
            .next()
            .unwrap_or(&operation)
            .to_string();
        let rendered = match claimants.as_slice() {
            [(unit, d)] => {
                wit_func(&name, &Interface::of(d), &types, ws, *unit).map(|(text, _)| text)
            }
            many => Err(WitError::Claimed {
                operation: operation.clone(),
                count: many.len(),
            }),
        };
        out.insert(operation, rendered);
    }
    out
}

/// One component's exported interface.
struct Api {
    name: String,
    /// Declared type names this interface's signatures touch, so it can `use`
    /// them from `types`.
    uses: BTreeSet<String>,
    funcs: Vec<String>,
}

/// The package text.
/// **Every type a published signature can reach, transitively.**
///
/// Seeded rather than derived from `apis` since 2026-08-20: the host packages
/// this file now emits have signatures too, and `capability.SessionId` is
/// reachable from `pw:host/session#read` and from nothing an export names.
///
/// Only these are emitted. The platform packages declare far more — `Decoder`,
/// `Style`, `ElementRef` — and none of them appears in a signature the host
/// calls. Emitting the whole program's types would put shapes on the ABI that
/// nothing crosses it, and would make any one of them unmappable a refusal for
/// a package that never needed it.
fn reachable(
    types: &Types,
    ws: &Workspace,
    seeds: impl IntoIterator<Item = String>,
) -> BTreeSet<String> {
    let mut out: BTreeSet<String> = BTreeSet::new();
    let mut stack: Vec<String> = seeds.into_iter().collect();
    while let Some(path) = stack.pop() {
        if !out.insert(path.clone()) {
            continue;
        }
        let Some(def) = types.defs.iter().find(|d| d.name() == path) else {
            continue;
        };
        let mut found: BTreeSet<String> = BTreeSet::new();
        for written in def.referenced() {
            types.declared_within(ws, def.unit(), &written, &mut found);
        }
        stack.extend(found);
    }
    out
}

/// The type definitions, in declaration order, refusing anything unmappable.
///
/// **Refusing, not emitting a placeholder.** A record whose field has no WIT
/// form is one a host would decode into something it never was, and this is
/// reached only for types an export actually names.
fn render_types(
    types: &Types,
    ws: &Workspace,
    wanted: &BTreeSet<String>,
) -> Result<String, WitError> {
    let mut out = String::new();
    for def in &types.defs {
        if !wanted.contains(def.name()) {
            continue;
        }
        let (name, unit) = (def.name(), def.unit());
        match def {
            TypeDef::Record { fields, .. } => {
                if fields.is_empty() {
                    // A WIT record must have at least one field. A Pleris type
                    // with none carries no information, and a nameable type
                    // carrying none is the nearest honest thing.
                    out.push_str(&format!("    type {} = bool;\n", ident(name)));
                    continue;
                }
                out.push_str(&format!("    record {} {{\n", ident(name)));
                for (f, ty) in fields {
                    out.push_str(&format!(
                        "        {}: {},\n",
                        ident(f),
                        wit_type(ty, types, ws, unit, name)?
                    ));
                }
                out.push_str("    }\n");
            }
            TypeDef::Variant { cases, .. } => {
                out.push_str(&format!("    variant {} {{\n", ident(name)));
                for (case, fields) in cases {
                    match fields.first() {
                        Some(ty) => out.push_str(&format!(
                            "        {}({}),\n",
                            ident(case),
                            wit_type(ty, types, ws, unit, name)?
                        )),
                        None => out.push_str(&format!("        {},\n", ident(case))),
                    }
                }
                out.push_str("    }\n");
            }
            TypeDef::Alias { of, .. } => {
                out.push_str(&format!(
                    "    type {} = {};\n",
                    ident(name),
                    wit_type(of, types, ws, unit, name)?
                ));
            }
        }
    }
    Ok(out)
}

/// The package holding every type that crosses a boundary.
///
/// Separate from [`PACKAGE`] since 2026-08-20, and the separation is forced:
/// the host packages this file also emits need these types, and a `pw:app`
/// world imports `store:data/carts`, so putting the types in `pw:app` would
/// make the two packages depend on each other.
///
/// **Unversioned deliberately.** A nested package declaration and a `use` that
/// names it must agree, and `pw:types@0.1.0` declared inside a file is not
/// found by `use pw:types/types` — `wit_parser` reports *package not found*.
pub const TYPES_PACKAGE: &str = "pw:types";

/// One package of host operations, as the Pleris declarations define them.
struct HostPackage {
    /// `store:data`
    name: String,
    /// interface name -> its functions, and the types they name.
    interfaces: BTreeMap<String, (BTreeSet<String>, Vec<String>)>,
}

fn render(type_text: &str, apis: &[Api], worlds: &[World], hosts: &[HostPackage]) -> String {
    let mut out = String::new();
    out.push_str("// Generated by `pw emit-wit`. Do not edit.\n");
    out.push_str("//\n");
    out.push_str("// One world per ComponentContract. A world's imports are exactly the\n");
    out.push_str("// contract's imports, projected onto interfaces — see pw-core/src/wit.rs.\n");
    out.push_str("//\n");
    out.push_str("// The host packages are HERE, generated, because the Pleris declaration is\n");
    out.push_str("// the ABI authority for every operation this program declares. Architect\n");
    out.push_str("// ruling, 2026-08-20: a deployment IMPLEMENTS this interface; it does not\n");
    out.push_str("// independently specify what the interface means. One interface has one\n");
    out.push_str("// ABI authority, never a Pleris signature plus a hand-authored WIT one\n");
    out.push_str("// with a comparison keeping them synchronized.\n\n");
    out.push_str(&format!("package {PACKAGE};\n\n"));

    for a in apis {
        out.push_str(&format!("interface {} {{\n", a.name));
        if !a.uses.is_empty() {
            let names: Vec<String> = a.uses.iter().map(|u| ident(u)).collect();
            out.push_str(&format!(
                "    use {TYPES_PACKAGE}/types.{{{}}};\n",
                names.join(", ")
            ));
        }
        for f in &a.funcs {
            out.push_str(&format!("    {f}\n"));
        }
        out.push_str("}\n\n");
    }

    for w in worlds {
        out.push_str(&format!("world {} {{\n", w.name));
        for i in &w.imports {
            out.push_str(&format!("    import {i};\n"));
        }
        out.push_str(&format!("    export {};\n", w.exports));
        out.push_str("}\n\n");
    }

    // The dependency packages, nested. `push_dir` wants the directory's own
    // package unbraced and every other one braced, so this order is not a
    // stylistic choice.
    if !type_text.is_empty() {
        out.push_str(&format!("package {TYPES_PACKAGE} {{\n"));
        out.push_str("    interface types {\n");
        for line in type_text.lines() {
            out.push_str(&format!("    {line}\n"));
        }
        out.push_str("    }\n}\n\n");
    }

    for h in hosts {
        out.push_str(&format!("package {} {{\n", h.name));
        for (iface, (uses, funcs)) in &h.interfaces {
            out.push_str(&format!("    interface {iface} {{\n"));
            if !uses.is_empty() {
                let names: Vec<String> = uses.iter().map(|u| ident(u)).collect();
                out.push_str(&format!(
                    "        use {TYPES_PACKAGE}/types.{{{}}};\n",
                    names.join(", ")
                ));
            }
            for f in funcs {
                out.push_str(&format!("        {f}\n"));
            }
            out.push_str("    }\n");
        }
        out.push_str("}\n\n");
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn mangling_is_kebab_and_lossy_in_a_way_callers_must_check() {
        assert_eq!(ident("store.page.StorePage"), "store-page-store-page");
        assert_eq!(ident("add_to_cart"), "add-to-cart");
        assert_eq!(ident("StoreId"), "store-id");
        assert_eq!(ident("pw:host/database"), "pw-host-database");
        // A leading digit is not a WIT identifier.
        assert_eq!(ident("2fast"), "x-2fast");
        // And the lossiness, stated: two Pleris names, one WIT identifier.
        assert_eq!(ident("store.page.Cart"), ident("store.page.cart"));
    }

    #[test]
    fn a_collision_is_refused_rather_than_resolved() {
        // Silently merging two identities is `docs/RISK_QUEUE.md` 34 in a new
        // place, and the merged one would be exported to a host.
        let err = no_collisions(["shop.Cart".to_string(), "shop.cart".to_string()])
            .expect_err("a collision");
        assert!(matches!(err, WitError::Collision { .. }), "{err:?}");
        // ...and the same name twice is not a collision.
        assert!(no_collisions(["shop.Cart".to_string(), "shop.Cart".to_string()]).is_ok());
    }

    /// A checked program, since a type name only means something from the unit
    /// it was written in.
    fn analysed(src: &str) -> (Vec<crate::hir::Hir>, Types) {
        let hirs = vec![crate::lower::lower_file(
            src,
            &pw_syntax::parse_tree(src).green,
        )];
        let refs: Vec<&Hir> = hirs.iter().collect();
        let types = Types::build(&refs);
        (hirs, types)
    }

    const SHOP: &str = "\
module shop

type Store = Store { id: Int }
";

    #[test]
    fn an_unmappable_type_is_refused_rather_than_guessed() {
        // The direction that matters. `string` for anything unrecognised
        // produces a world that parses, links, and decodes a value into
        // something it never was.
        let (hirs, types) = analysed(SHOP);
        let refs: Vec<&Hir> = hirs.iter().collect();
        let ws = Workspace::build(&refs);

        let err = wit_type("Whatever", &types, &ws, 0, "f").expect_err("unmappable");
        assert_eq!(
            err,
            WitError::Unmappable {
                ty: "Whatever".into(),
                at: "f".into()
            }
        );
        // The control: the primitives and the carriers do map.
        assert_eq!(wit_type("Int", &types, &ws, 0, "f").unwrap(), "s64");
        assert_eq!(
            wit_type("Result<Int, String>", &types, &ws, 0, "f").unwrap(),
            "result<s64, string>"
        );
        assert_eq!(
            wit_type("List<Option<Bool>>", &types, &ws, 0, "f").unwrap(),
            "list<option<bool>>"
        );
    }

    #[test]
    fn a_declared_type_is_named_by_its_qualified_path() {
        // **Not by its bare name.** `capability.SessionId` and
        // `domain.SessionId` are two types, and keying WIT by `Store` rather
        // than `shop.Store` made both of them one `session-id` — which
        // `wit-parser` refused as a duplicate definition. It refused; a
        // generator that deduplicated instead would have put one type on the
        // wire where the program has two.
        let (hirs, types) = analysed(SHOP);
        let refs: Vec<&Hir> = hirs.iter().collect();
        let ws = Workspace::build(&refs);

        assert_eq!(
            wit_type("Store", &types, &ws, 0, "f").unwrap(),
            "shop-store"
        );
        assert_eq!(
            wit_type("Result<Store, String>", &types, &ws, 0, "f").unwrap(),
            "result<shop-store, string>"
        );
    }

    #[test]
    fn two_modules_declaring_one_name_stay_two_types() {
        // The case that produced the rule, as a program: `SessionId` in two
        // modules. Both are emitted, under different identifiers, and a
        // signature gets whichever one it can see.
        let src = "\
module a

opaque type SessionId = String
";
        let other = "\
module b

opaque type SessionId = String
";
        let hirs = [
            crate::lower::lower_file(src, &pw_syntax::parse_tree(src).green),
            crate::lower::lower_file(other, &pw_syntax::parse_tree(other).green),
        ];
        let refs: Vec<&Hir> = hirs.iter().collect();
        let types = Types::build(&refs);
        let ws = Workspace::build(&refs);

        assert_eq!(types.known.len(), 2, "{:?}", types.known);
        assert_eq!(
            wit_type("SessionId", &types, &ws, 0, "f").unwrap(),
            "a-session-id"
        );
        assert_eq!(
            wit_type("SessionId", &types, &ws, 1, "f").unwrap(),
            "b-session-id"
        );
    }
}
