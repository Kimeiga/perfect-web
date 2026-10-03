//! **What a program means, for review, and what a change does to it**
//! (ADR-0149; charter §14 M14 task 3, §19.2).
//!
//! A line diff of a Pleris change shows text. A reviewer needs what the text
//! did: which effects a declaration gained, which capabilities a node must now
//! grant, what is now cached and for whom, what a page waits for, how many
//! bytes reach the browser. [`Model::of`] reduces a checked, built program to
//! that, as plain sorted data with no spans, so a comment or a reflow changes
//! nothing. [`diff`] compares two, section by section, in §19.2's order.

use std::collections::{BTreeMap, BTreeSet};

use serde::Serialize;

use crate::build::Build;
use crate::check::Unit;
use crate::hir::{DeclKind, Expr};

/// **A program, reduced to what review reads.** Every map is keyed by a
/// declaration's path, `store.page.Menu`.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize)]
pub struct Model {
    /// Records' fields, `name: Type`, and sums' cases, `Case(Payload)`.
    pub types: BTreeMap<String, Shape>,
    /// What each declaration says of itself.
    pub declarations: BTreeMap<String, Declared>,
    /// What each component needs of the node that runs it.
    pub components: BTreeMap<String, Component>,
    /// Each handler the browser loads, `name in page`, with its identity and
    /// bytes. A handler with no name, a lambda that sets signals, is named by
    /// its identity.
    pub handlers: BTreeMap<String, (String, usize)>,
    /// Each page's speculation module's bytes.
    pub speculations: BTreeMap<String, usize>,
    /// What each page shows.
    pub pages: BTreeMap<String, Page>,
    /// The resource graph's edges, `from kind to(key)`.
    pub edges: BTreeSet<String>,
    /// Each unsafe escape hatch, `declaration: unsafe.imperative`.
    pub unsafe_uses: BTreeSet<String>,
    /// Each warning, `code message`.
    pub warnings: BTreeSet<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case", tag = "shape", content = "of")]
pub enum Shape {
    Record(BTreeMap<String, String>),
    Sum(BTreeMap<String, String>),
    Opaque(String),
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize)]
pub struct Declared {
    pub kind: String,
    /// `public`, `session`, `private`, or empty.
    pub visibility: String,
    /// The declared effect row, as written; `None` where none was written.
    pub effects: Option<BTreeSet<String>>,
    /// Each policy, `name -> value`.
    pub policies: BTreeMap<String, String>,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize)]
pub struct Component {
    pub capabilities: BTreeSet<String>,
    pub placements: BTreeSet<String>,
    pub imports: BTreeSet<String>,
    /// The audited component's bytes; 0 for a declaration with no body.
    pub bytes: usize,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize)]
pub struct Page {
    /// `name = Resource(args)`.
    pub bindings: BTreeSet<String>,
    /// `Resource(args)`, `streamed` or `waited for`.
    pub streams: BTreeSet<String>,
    pub signals: BTreeSet<String>,
}

impl Model {
    /// **The model of a program**, checked and built as `pw build` builds it,
    /// or why there is none: its errors, or what the build refused. A diff
    /// of a program the compiler refused would describe meaning it does not
    /// have.
    pub fn of(units: &[Unit]) -> Result<Model, String> {
        let mut warnings = BTreeSet::new();
        let mut errors = Vec::new();
        for (path, diagnostics) in crate::check::check_units(units) {
            for d in diagnostics {
                let line = format!("{} {}", d.code, d.message);
                if d.is_error() {
                    errors.push(format!("{path}: {line}"));
                } else {
                    warnings.insert(line);
                }
            }
        }
        if !errors.is_empty() {
            errors.truncate(3);
            return Err(format!("it does not check: {}", errors.join("; ")));
        }
        let build = crate::build::build(units)?;
        let mut model = Model {
            warnings,
            ..Model::default()
        };
        model.read_declarations(units);
        model.read_build(&build);
        Ok(model)
    }

    fn read_declarations(&mut self, units: &[Unit]) {
        for unit in units {
            let hir = &unit.hir;
            for (id, d) in hir.all_decls() {
                if matches!(d.kind, DeclKind::Import) || d.name.is_empty() {
                    continue;
                }
                let module = hir.module_of(id).unwrap_or_default();
                let path = if module.is_empty() {
                    d.name.clone()
                } else {
                    format!("{module}.{}", d.name)
                };
                match d.kind {
                    DeclKind::Type if d.variants.is_some() => {
                        let cases = d
                            .variants
                            .iter()
                            .flatten()
                            .map(|v| {
                                let payload: Vec<String> =
                                    v.fields.iter().map(|f| f.written()).collect();
                                (v.name.clone(), payload.join(", "))
                            })
                            .collect();
                        self.types.insert(path.clone(), Shape::Sum(cases));
                    }
                    DeclKind::Type => {
                        let fields = d
                            .fields
                            .iter()
                            .flatten()
                            .map(|f| {
                                (
                                    f.name.clone(),
                                    f.ty.as_ref().map(|t| t.written()).unwrap_or_default(),
                                )
                            })
                            .collect();
                        self.types.insert(path.clone(), Shape::Record(fields));
                    }
                    DeclKind::Opaque => {
                        let of = d
                            .opaque_of
                            .as_ref()
                            .map(|t| t.written())
                            .unwrap_or_default();
                        self.types.insert(path.clone(), Shape::Opaque(of));
                    }
                    _ => {}
                }
                self.declarations.insert(
                    path.clone(),
                    Declared {
                        kind: format!("{:?}", d.kind).to_lowercase(),
                        visibility: d.visibility.clone().unwrap_or_default(),
                        effects: d
                            .declared_effects
                            .as_ref()
                            .map(|row| row.iter().map(|e| e.written.clone()).collect()),
                        policies: d
                            .policies
                            .iter()
                            .map(|p| (p.name.clone(), p.value.clone()))
                            .collect(),
                    },
                );
                // Each unsafe escape hatch its body writes.
                if let Some(b) = d.body {
                    let body = hir.body(b);
                    for e in body.walk() {
                        if let Expr::Keyword { keyword, .. } = body.expr(e)
                            && keyword.starts_with("unsafe")
                        {
                            self.unsafe_uses.insert(format!("{path}: {keyword}"));
                        }
                    }
                }
            }
        }
    }

    fn read_build(&mut self, build: &Build) {
        for c in &build.contracts {
            let capabilities = c
                .required_capabilities
                .iter()
                .map(|cap| {
                    let mut s = cap.family.clone();
                    if !cap.operation.is_empty() {
                        s.push('.');
                        s.push_str(&cap.operation);
                    }
                    if let Some(a) = &cap.argument {
                        s.push_str(&format!("<{a}>"));
                    }
                    s
                })
                .collect();
            self.components.insert(
                c.component_id.clone(),
                Component {
                    capabilities,
                    placements: c.allowed_placements.iter().cloned().collect(),
                    imports: c
                        .imports
                        .iter()
                        .map(|i| format!("{}#{}", i.interface, i.name))
                        .collect(),
                    bytes: 0,
                },
            );
        }
        for (id, built) in &build.components {
            if let crate::backend::component::Built::Component { compiled, .. } = built
                && let Some(c) = self.components.get_mut(id)
            {
                c.bytes = compiled.component.bytes.len();
            }
        }
        for h in &build.handlers {
            if let crate::backend::wasm::Encoding::Encoded(m) = &h.module {
                let name = if m.name.is_empty() {
                    format!("the handler {}", &m.identity[..m.identity.len().min(8)])
                } else {
                    format!("the handler `{}`", m.name)
                };
                self.handlers.insert(
                    format!("{name} in `{}`", h.declaration),
                    (m.identity.clone(), m.source.len()),
                );
            }
        }
        for s in &build.speculations {
            if let crate::backend::wasm::Encoding::Encoded(m) = &s.module {
                self.speculations.insert(m.page.clone(), m.source.len());
            }
        }
        for planned in &build.pages {
            let Ok(plan) = &planned.plan else { continue };
            self.pages.insert(
                plan.page.clone(),
                Page {
                    bindings: plan
                        .bindings
                        .iter()
                        .map(|b| format!("{} = {}({})", b.binding, b.resource, b.args.join(", ")))
                        .collect(),
                    streams: plan
                        .streams
                        .iter()
                        .map(|s| {
                            format!(
                                "{}({}), {}",
                                s.resource,
                                s.args.join(", "),
                                if s.streamed { "streamed" } else { "waited for" }
                            )
                        })
                        .collect(),
                    signals: plan.signals.iter().map(|s| s.name.clone()).collect(),
                },
            );
        }
        for e in &build.graph.edges {
            self.edges.insert(format!(
                "{} {:?} {}({})",
                e.from,
                e.kind,
                e.to,
                e.key.join(", ")
            ));
        }
    }
}

/// One section of a report: its title, and each change, one line.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct Section {
    pub title: &'static str,
    pub changes: Vec<String>,
}

/// **What a change does**, section by section, in charter §19.2's order.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct Report {
    pub sections: Vec<Section>,
}

impl Report {
    /// As §19.2 writes it: each section's title, then its changes, or `none`,
    /// so a section with nothing in it reads as checked, not skipped.
    pub fn text(&self) -> String {
        let mut out = String::new();
        for s in &self.sections {
            out.push_str(s.title);
            out.push('\n');
            if s.changes.is_empty() {
                out.push_str("- none\n");
            }
            for c in &s.changes {
                out.push_str("- ");
                out.push_str(c);
                out.push('\n');
            }
            out.push('\n');
        }
        out
    }

    /// Whether any section has a change.
    pub fn is_empty(&self) -> bool {
        self.sections.iter().all(|s| s.changes.is_empty())
    }
}

/// Which section a declared policy's change belongs to. A policy no section
/// names is reported under "Other policies", never dropped: a table that
/// ignores what it does not know is how a policy goes unreviewed.
fn section_of(policy: &str) -> &'static str {
    match policy {
        "cache" | "freshness" | "consistency" | "key" | "timeout" | "retry" | "delivery"
        | "fallback" | "concurrency" | "on_key_change" | "stampede" | "regenerate"
        | "dedupe_by" | "transport" | "reconnect" | "scope" | "on_scope_exit" => "cache",
        "requires" | "idempotent_by" | "optimistic" | "rollback" | "transaction" => "obligations",
        "placement" => "placement",
        "partition" | "privacy" => "privacy",
        // The graph's clauses: reported as its edges, once.
        "invalidates" | "invalidates_on" | "emits" | "depends_on" => "graph",
        _ => "other",
    }
}

/// `added`, `removed`, each with how a member reads, between two sets.
fn set_changes<T: Ord + Clone>(old: &BTreeSet<T>, new: &BTreeSet<T>) -> (Vec<T>, Vec<T>) {
    (
        new.difference(old).cloned().collect(),
        old.difference(new).cloned().collect(),
    )
}

/// **What changed between two programs.**
pub fn diff(old: &Model, new: &Model) -> Report {
    let mut domain = Vec::new();
    let mut effects = Vec::new();
    let mut capabilities = Vec::new();
    let mut privacy = Vec::new();
    let mut placement = Vec::new();
    let mut cache = Vec::new();
    let mut invalidation = Vec::new();
    let mut pages = Vec::new();
    let mut client = Vec::new();
    let mut server = Vec::new();
    let mut obligations = Vec::new();
    let mut unsafe_changes = Vec::new();
    let mut diagnostics = Vec::new();
    let mut other = Vec::new();

    // --- domain: types, their fields and their cases
    let types: BTreeSet<&String> = old.types.keys().chain(new.types.keys()).collect();
    for t in types {
        match (old.types.get(t), new.types.get(t)) {
            (None, Some(_)) => domain.push(format!("added type `{t}`")),
            (Some(_), None) => domain.push(format!("removed type `{t}`")),
            (Some(Shape::Sum(a)), Some(Shape::Sum(b))) => {
                for (case, payload) in b {
                    match a.get(case) {
                        None => domain.push(format!("added `{case}` to `{t}`")),
                        Some(was) if was != payload => domain.push(format!(
                            "`{t}.{case}` now carries `({payload})`, was `({was})`"
                        )),
                        _ => {}
                    }
                }
                for case in a.keys().filter(|c| !b.contains_key(*c)) {
                    domain.push(format!("removed `{case}` from `{t}`"));
                }
            }
            (Some(Shape::Record(a)), Some(Shape::Record(b))) => {
                for (field, ty) in b {
                    match a.get(field) {
                        None => domain.push(format!("added field `{t}.{field}: {ty}`")),
                        Some(was) if was != ty => {
                            domain.push(format!("`{t}.{field}` is now `{ty}`, was `{was}`"))
                        }
                        _ => {}
                    }
                }
                for field in a.keys().filter(|f| !b.contains_key(*f)) {
                    domain.push(format!("removed field `{t}.{field}`"));
                }
            }
            (Some(a), Some(b)) if a != b => {
                domain.push(format!("`{t}` changed its shape"));
            }
            _ => {}
        }
    }

    // --- each declaration: its effects, its visibility, its policies
    let declarations: BTreeSet<&String> = old
        .declarations
        .keys()
        .chain(new.declarations.keys())
        .collect();
    for d in declarations {
        let (a, b) = (old.declarations.get(d), new.declarations.get(d));
        let none = Declared::default();
        let (a_, b_) = (a.unwrap_or(&none), b.unwrap_or(&none));
        let row = |r: &Option<BTreeSet<String>>| r.clone().unwrap_or_default();
        let (added, removed) = set_changes(&row(&a_.effects), &row(&b_.effects));
        let new_decl = a.is_none();
        for e in added {
            effects.push(if new_decl {
                format!("`{d}` (new) performs `{e}`")
            } else {
                format!("`{d}` now performs `{e}`")
            });
        }
        for e in removed {
            if b.is_some() {
                effects.push(format!("`{d}` no longer performs `{e}`"));
            }
        }
        if b.is_some() && a_.visibility != b_.visibility {
            privacy.push(match (a.is_some(), a_.visibility.is_empty()) {
                (false, _) => format!("`{d}` (new) is `{}`", b_.visibility),
                (true, true) => format!("`{d}` is now `{}`", b_.visibility),
                (true, false) => format!(
                    "`{d}` is now `{}`, was `{}`",
                    if b_.visibility.is_empty() {
                        "unmarked"
                    } else {
                        &b_.visibility
                    },
                    a_.visibility
                ),
            });
        }
        let names: BTreeSet<&String> = a_.policies.keys().chain(b_.policies.keys()).collect();
        for p in names {
            let line = match (a_.policies.get(p), b_.policies.get(p)) {
                (None, Some(v)) if new_decl => format!("`{d}` (new): `{p} {v}`"),
                (None, Some(v)) => format!("`{d}` now declares `{p} {v}`"),
                (Some(v), None) if b.is_some() => format!("`{d}` no longer declares `{p} {v}`"),
                (Some(was), Some(v)) if was != v => format!("`{d}`: `{p}` is `{v}`, was `{was}`"),
                _ => continue,
            };
            match section_of(p) {
                "cache" => cache.push(line),
                "obligations" => obligations.push(line),
                "placement" => placement.push(line),
                "privacy" => privacy.push(line),
                "graph" => {}
                _ => other.push(line),
            }
        }
    }

    // --- capabilities and placement: what each component needs of its node
    let components: BTreeSet<&String> =
        old.components.keys().chain(new.components.keys()).collect();
    let none = Component::default();
    for c in components {
        let (a, b) = (old.components.get(c), new.components.get(c));
        let (a_, b_) = (a.unwrap_or(&none), b.unwrap_or(&none));
        let (added, removed) = set_changes(&a_.capabilities, &b_.capabilities);
        for cap in added {
            capabilities.push(format!("`{c}` needs `{cap}`"));
        }
        for cap in removed {
            capabilities.push(format!("`{c}` no longer needs `{cap}`"));
        }
        if a.is_some() && b.is_some() && a_.placements != b_.placements {
            let list = |s: &BTreeSet<String>| s.iter().cloned().collect::<Vec<_>>().join(", ");
            placement.push(format!(
                "`{c}` may run at {}, was {}",
                list(&b_.placements),
                list(&a_.placements)
            ));
        }
        match (a, b) {
            (None, Some(n)) if n.bytes > 0 => {
                server.push(format!("added component `{c}`, {} bytes", n.bytes))
            }
            (Some(o), None) if o.bytes > 0 => server.push(format!("removed component `{c}`")),
            (Some(o), Some(n)) if o.bytes != n.bytes && o.bytes > 0 && n.bytes > 0 => {
                server.push(format!("`{c}`: {} bytes, was {}", n.bytes, o.bytes))
            }
            _ => {}
        }
    }
    let granted = |m: &Model| -> BTreeSet<String> {
        m.components
            .values()
            .flat_map(|c| c.capabilities.clone())
            .collect()
    };
    let (gained, lost) = set_changes(&granted(old), &granted(new));
    for cap in gained {
        capabilities.push(format!("the node must now grant `{cap}`"));
    }
    for cap in lost {
        capabilities.push(format!("the node no longer needs to grant `{cap}`"));
    }

    // --- the resource graph
    let (added, removed) = set_changes(&old.edges, &new.edges);
    invalidation.extend(added.into_iter().map(|e| format!("added `{e}`")));
    invalidation.extend(removed.into_iter().map(|e| format!("removed `{e}`")));

    // --- pages
    let all: BTreeSet<&String> = old.pages.keys().chain(new.pages.keys()).collect();
    let empty = Page::default();
    for p in all {
        let (a, b) = (old.pages.get(p), new.pages.get(p));
        match (a, b) {
            (None, Some(_)) => pages.push(format!("added page `{p}`")),
            (Some(_), None) => pages.push(format!("removed page `{p}`")),
            _ => {}
        }
        let (a_, b_) = (a.unwrap_or(&empty), b.unwrap_or(&empty));
        for (what, x, y) in [
            ("reads", &a_.bindings, &b_.bindings),
            ("streams", &a_.streams, &b_.streams),
            ("holds the signal", &a_.signals, &b_.signals),
        ] {
            let (added, removed) = set_changes(x, y);
            for v in added {
                pages.push(format!("`{p}` {what} `{v}`"));
            }
            for v in removed {
                if b.is_some() {
                    pages.push(format!("`{p}` no longer {what} `{v}`"));
                }
            }
        }
    }

    // --- what reaches the browser
    let handlers: BTreeSet<&String> = old.handlers.keys().chain(new.handlers.keys()).collect();
    for h in handlers {
        match (old.handlers.get(h), new.handlers.get(h)) {
            (None, Some((_, n))) => client.push(format!("added {h}, {n} bytes")),
            (Some(_), None) => client.push(format!("removed {h}")),
            // Its identity is what a served document's resume decision
            // checks: a changed one is a press that loads new code.
            (Some((was, o)), Some((now, n))) if was != now => {
                client.push(format!("{h} changed: {n} bytes, was {o}"))
            }
            _ => {}
        }
    }
    let speculations: BTreeSet<&String> = old
        .speculations
        .keys()
        .chain(new.speculations.keys())
        .collect();
    for s in speculations {
        match (old.speculations.get(s), new.speculations.get(s)) {
            (None, Some(n)) => client.push(format!("added the speculation of `{s}`, {n} bytes")),
            (Some(_), None) => client.push(format!("removed the speculation of `{s}`")),
            (Some(o), Some(n)) if o != n => {
                client.push(format!("the speculation of `{s}`: {n} bytes, was {o}"))
            }
            _ => {}
        }
    }
    let bytes = |m: &Model| -> usize {
        m.handlers.values().map(|(_, b)| b).sum::<usize>() + m.speculations.values().sum::<usize>()
    };
    if bytes(old) != bytes(new) {
        client.push(format!(
            "the browser's compiled code: {} bytes, was {}",
            bytes(new),
            bytes(old)
        ));
    }

    // --- unsafe, and the diagnostics
    let (added, removed) = set_changes(&old.unsafe_uses, &new.unsafe_uses);
    unsafe_changes.extend(added.into_iter().map(|u| format!("added `{u}`")));
    unsafe_changes.extend(removed.into_iter().map(|u| format!("removed `{u}`")));
    let (added, removed) = set_changes(&old.warnings, &new.warnings);
    diagnostics.extend(added.into_iter().map(|w| format!("new warning: {w}")));
    diagnostics.extend(removed.into_iter().map(|w| format!("gone: {w}")));

    Report {
        sections: vec![
            Section {
                title: "Domain changes",
                changes: domain,
            },
            Section {
                title: "Effects",
                changes: effects,
            },
            Section {
                title: "Capabilities",
                changes: capabilities,
            },
            Section {
                title: "Privacy changes",
                changes: privacy,
            },
            Section {
                title: "Placement",
                changes: placement,
            },
            Section {
                title: "Cache and freshness",
                changes: cache,
            },
            Section {
                title: "Invalidation",
                changes: invalidation,
            },
            Section {
                title: "Pages",
                changes: pages,
            },
            Section {
                title: "Client impact",
                changes: client,
            },
            Section {
                title: "Server components",
                changes: server,
            },
            Section {
                title: "Obligations",
                changes: obligations,
            },
            Section {
                title: "Unsafe changes",
                changes: unsafe_changes,
            },
            Section {
                title: "Diagnostics",
                changes: diagnostics,
            },
            Section {
                title: "Other policies",
                changes: other,
            },
        ],
    }
}
