//! **What a page shows, as reads of what its queries return** (ADR-0125).
//!
//! A page binds its queries and its template reads paths from the bindings:
//!
//! ```text
//! let cart = query Cart(current_session())     a binding: a query and its key
//! <p>{cart.line_count}</p>                     a part: the binding, then reads
//! ```
//!
//! A read is a record field, or a member function (ADR-0048): `line_count`
//! is `domain.line_count(cart)`, since `Cart` has no field of that name. Until
//! 2026-10-02 the development server computed every value a page showed in
//! Rust, and ran none of the store's queries. This module is the plan a host
//! follows instead: which component each binding runs with which arguments,
//! and, for each text part, the steps from the binding's value to the part's.
//! A member function a plan names is compiled as a component of its own, with
//! a contract of kind `function` ([`members`]).
//!
//! # What is refused, by name
//!
//! - a binding whose key is neither a page parameter nor an
//!   invocation-context call (`current_session()`);
//! - a member read inside a block (`{#each}`, `{#match}`, `{#if}`): a host
//!   would call it per instance, which this plan does not state;
//! - a path that reads through something that is neither a field nor a
//!   member of the type it reads.

use std::collections::BTreeSet;

use crate::hir::{Body, DeclId, DeclKind, Expr, ExprId, Hir, Pattern};
use crate::resolve::{DefId, Namespace, Resolution, Workspace};
use crate::resolved::ResolvedType;
use crate::signatures::{Receiver, Signatures};

/// One page binding: `let cart = query Cart(current_session())`.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct Binding {
    pub binding: String,
    /// The query, as a component id: `store.page.Cart`.
    pub resource: String,
    /// Each argument as a host computes it: a page parameter's name, or an
    /// invocation-context call, `current_session()`.
    pub args: Vec<String>,
    /// How a host runs the query: its declared policies (ADR-0127).
    pub policy: Policy,
}

/// A query's policies, as a host applies them (ADR-0127).
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct Policy {
    /// `shared`, `private`, or `none`: whether a value is kept, and for whom.
    pub cache: String,
    /// `public`, `session` or `private`: whose value it is.
    pub privacy: String,
    /// How long a kept value is served, in milliseconds. Absent is 0: a value
    /// nobody said may be reused is not reused.
    pub freshness_ms: u64,
    /// The whole request's budget, retries included, in milliseconds.
    pub timeout_ms: Option<u64>,
    /// Attempts in all, 1 meaning no retry.
    pub attempts: u32,
    /// `concurrency parallel`: requests for one key do not share a flight.
    pub parallel: bool,
    /// The arguments the entry is keyed by, by position. A query with no
    /// `key` clause is keyed by all of them, so two calls with different
    /// arguments never share an entry.
    pub key: Vec<usize>,
}

fn policy_of(decl: &crate::hir::Decl) -> Policy {
    use crate::manifest::{CachePartition, Concurrency, Privacy, Retry};
    let m = crate::manifest::of(decl);
    let key = match decl.policy("key") {
        Some(p) => p
            .value
            .split(',')
            .filter_map(|name| decl.params.iter().position(|q| q.name == name.trim()))
            .collect(),
        None => (0..decl.params.len()).collect(),
    };
    Policy {
        cache: match m.cache_partition {
            CachePartition::Shared => "shared",
            CachePartition::Private => "private",
            CachePartition::None => "none",
        }
        .to_string(),
        privacy: match m.privacy {
            Privacy::Public => "public",
            Privacy::Session => "session",
            Privacy::Private => "private",
        }
        .to_string(),
        freshness_ms: m.freshness.unwrap_or(0),
        timeout_ms: m.timeout,
        attempts: match m.retry {
            Retry::Bounded { max, .. } => max.max(1),
            Retry::None | Retry::Forever => 1,
        },
        parallel: m.concurrency == Some(Concurrency::Parallel),
        key,
    }
}

/// One step from a value to the next.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Step {
    /// A record field, by its Pleris name.
    Field(String),
    /// A member function, called with the value: its component id.
    Member(String),
}

/// One text part outside any block: what it shows.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct Part {
    pub part: u32,
    /// The template's path for it, as the renderer is given values.
    pub path: String,
    pub binding: String,
    pub steps: Vec<Step>,
}

/// **A page's signal** (ADR-0130): its name and its first value, as the
/// browser's compiled modules read a value (`js_pure`'s wire form). The
/// server renders the first value; the browser holds it from there.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct Signal {
    pub name: String,
    pub initial: serde_json::Value,
}

/// **A part a signal decides** (ADR-0130): the browser renders it again when
/// the signal changes. A text or attribute part reads `path`; a conditional
/// or match part is a block the signal chooses between.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct Live {
    pub part: u32,
    pub signal: String,
    pub path: String,
    /// The part's kind, as the parts manifest names it.
    pub kind: String,
    /// Every signal a block reads anywhere inside it, its own included: the
    /// block is rendered again when any of them changes. Empty for a text
    /// part, which reads `signal` alone.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub reads: Vec<String>,
}

/// Every path a part reads, itself and anything inside it.
fn paths_read(part: &crate::template_ir::Part, out: &mut Vec<String>) {
    use crate::template_ir::{Chunk, Part, Segment};
    fn chunks(cs: &[Chunk], out: &mut Vec<String>) {
        for c in cs {
            if let Chunk::Dynamic(p) = c {
                paths_read(p, out);
            }
        }
    }
    match part {
        Part::Text { value, .. }
        | Part::Attribute { value, .. }
        | Part::BooleanAttribute { value, .. }
        | Part::RawHtml { value, .. } => out.push(value.clone()),
        Part::InterpolatedAttribute { segments, .. } => {
            for s in segments {
                if let Segment::Value(v) = s {
                    out.push(v.clone());
                }
            }
        }
        Part::Conditional {
            value,
            then,
            otherwise,
            ..
        } => {
            out.push(value.clone());
            chunks(then, out);
            chunks(otherwise, out);
        }
        Part::Match { value, arms, .. } => {
            out.push(value.clone());
            for a in arms {
                chunks(&a.body, out);
            }
        }
        Part::Each {
            collection, body, ..
        } => {
            out.push(collection.clone());
            chunks(body, out);
        }
        Part::Component { args, .. } => out.extend(args.iter().map(|(_, v)| v.clone())),
        Part::Event { .. } | Part::Blocked { .. } => {}
    }
}

/// The part numbered `id`, wherever it is in `chunks`.
fn find_part(chunks: &[crate::template_ir::Chunk], id: u32) -> Option<&crate::template_ir::Part> {
    use crate::template_ir::{Chunk, Part};
    for c in chunks {
        let Chunk::Dynamic(p) = c else { continue };
        if p.id().is_some_and(|i| i.0 == id) {
            return Some(p);
        }
        let inner = match p {
            Part::Conditional {
                then, otherwise, ..
            } => find_part(then, id).or_else(|| find_part(otherwise, id)),
            Part::Match { arms, .. } => arms.iter().find_map(|a| find_part(&a.body, id)),
            Part::Each { body, .. } => find_part(body, id),
            _ => None,
        };
        if inner.is_some() {
            return inner;
        }
    }
    None
}

/// A page's plan.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct PageValues {
    /// The page's template path: `store.page.StorePage`.
    pub page: String,
    pub params: Vec<String>,
    pub bindings: Vec<Binding>,
    /// Each text part outside a block.
    pub parts: Vec<Part>,
    /// Each binding a block iterates or matches, which a host gives the
    /// renderer whole: `menu`.
    pub collections: Vec<String>,
    /// The page's signals (ADR-0130). Their first values are the backend's
    /// to compute, and `build` writes them here.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub signals: Vec<Signal>,
    /// The parts its signals decide.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub live: Vec<Live>,
}

/// **A body's signals, in order** (ADR-0130): each one's name and the
/// expression its first value is.
pub fn signals_of(body: &Body) -> Vec<(String, ExprId, ExprId)> {
    let mut out = Vec::new();
    for id in body.walk() {
        if !body.signals.contains(&id) {
            continue;
        }
        let Expr::Let {
            pat: Some(p),
            init: Some(init),
            ..
        } = body.expr(id)
        else {
            continue;
        };
        if let crate::hir::Pattern::Bind { name, .. } = body.pat(*p) {
            out.push((name.clone(), *init, id));
        }
    }
    out
}

/// The plan, or why there is none, for one page.
#[derive(Debug, Clone)]
pub struct Planned {
    pub page: String,
    pub plan: Result<PageValues, String>,
}

/// A path written in `unit`, resolved as a term.
fn resolve_term(ws: &Workspace, unit: usize, path: &str) -> Option<DefId> {
    let r = match path.contains('.') {
        true => ws.resolve_path(unit, path),
        false => ws.resolve_in(unit, Namespace::Term, path),
    };
    match r {
        Resolution::Local(d) | Resolution::Imported { def: d, .. } => Some(d),
        _ => None,
    }
}

/// `let b = query R(k..)` in a page's body: the name, the query, its keys.
pub(crate) fn query_bindings(
    ws: &Workspace,
    unit: usize,
    body: &Body,
) -> Vec<(String, DefId, Vec<ExprId>)> {
    let mut out = Vec::new();
    for e in body.walk() {
        let Expr::Let {
            pat: Some(pat),
            init: Some(init),
            ..
        } = body.expr(e)
        else {
            continue;
        };
        let Pattern::Bind { name, .. } = body.pat(*pat) else {
            continue;
        };
        let Expr::Keyword {
            keyword,
            modifiers,
            args,
            ..
        } = body.expr(*init)
        else {
            continue;
        };
        if keyword != "query" {
            continue;
        }
        let Some(resource) = modifiers.first().and_then(|p| resolve_term(ws, unit, p)) else {
            continue;
        };
        out.push((name.clone(), resource, args.clone()));
    }
    out
}

/// The value a resource's entry holds: its result, or `Ok`'s payload.
pub(crate) fn value_of(sigs: &Signatures, resource: DefId) -> Option<ResolvedType> {
    let result = sigs.by_def(resource)?.result()?;
    match result.as_builtin() {
        Some(crate::resolved::Builtin::Result) => result.args().first().cloned(),
        _ => Some(result.clone()),
    }
}

/// The component id of a declaration, found by its identity.
fn component_id_of(hirs: &[&Hir], def: DefId) -> Option<String> {
    let hir = hirs.get(def.unit)?;
    hir.all_decls()
        .find(|(id, _)| id.0 == def.decl)
        .map(|(id, _)| crate::contract::component_id(hir, id))
}

/// The steps a path's reads take from a value of type `ty`, and the member
/// functions it names.
fn steps(
    sigs: &Signatures,
    mut ty: ResolvedType,
    reads: &[&str],
) -> Result<Vec<(Step, Option<DefId>)>, String> {
    let mut out = Vec::new();
    for read in reads {
        let def = ty
            .def_id()
            .ok_or_else(|| format!("`.{read}` is read from a value with no declared type"))?;
        let field = sigs
            .type_decl(def)
            .and_then(|t| t.record.as_ref())
            .and_then(|fields| fields.iter().find(|(n, _)| n == read))
            .and_then(|(_, t)| t.resolved().cloned());
        if let Some(next) = field {
            out.push((Step::Field(read.to_string()), None));
            ty = next;
            continue;
        }
        let member = sigs
            .member_by(Receiver::Nominal(def), read)
            .ok_or_else(|| format!("`.{read}` is neither a field nor a member"))?;
        let next = member
            .result()
            .cloned()
            .ok_or_else(|| format!("`.{read}` has no declared result"))?;
        out.push((Step::Member(String::new()), Some(member.definition)));
        ty = next;
    }
    Ok(out)
}

/// The path a hole reads: its root name and the reads after it.
fn path_of(body: &Body, e: ExprId) -> Option<(String, Vec<String>)> {
    match body.expr(e) {
        Expr::Name(n) => Some((n.clone(), Vec::new())),
        Expr::Field { base, name } => {
            let (root, mut reads) = path_of(body, *base)?;
            reads.push(name.clone());
            Some((root, reads))
        }
        _ => None,
    }
}

fn plan(
    hirs: &[&Hir],
    ws: &Workspace,
    sigs: &Signatures,
    unit: usize,
    id: DeclId,
) -> Result<(PageValues, BTreeSet<DefId>), String> {
    let hir = hirs[unit];
    let decl = hir.decl(id);
    let body = hir.body(
        decl.body
            .ok_or_else(|| format!("`{}` has no body", decl.name))?,
    );
    let params: Vec<String> = decl.params.iter().map(|p| p.name.clone()).collect();
    let page = {
        let module = hir.module_of(id).unwrap_or_default();
        if module.is_empty() {
            decl.name.clone()
        } else {
            format!("{module}.{}", decl.name)
        }
    };

    let found = query_bindings(ws, unit, body);
    let mut bindings = Vec::new();
    for (name, resource, keys) in &found {
        let mut args = Vec::new();
        for k in keys {
            args.push(match body.expr(*k) {
                Expr::Name(n) if params.contains(n) => n.clone(),
                Expr::Call { callee, args } if args.is_empty() => {
                    format!("{}()", crate::infer::path_of(body, *callee))
                }
                _ => {
                    return Err(format!(
                        "`{name}`'s key is neither a page parameter nor an invocation-context call"
                    ));
                }
            });
        }
        let decl = crate::resolve::declaration(hirs, *resource)
            .ok_or_else(|| format!("`{name}`'s query is declared nowhere"))?;
        bindings.push(Binding {
            binding: name.clone(),
            resource: component_id_of(hirs, *resource)
                .ok_or_else(|| format!("`{name}`'s query has no component"))?,
            args,
            policy: policy_of(decl),
        });
    }

    let signals: Vec<String> = signals_of(body).into_iter().map(|(n, ..)| n).collect();
    let mut live = Vec::new();
    let mut parts = Vec::new();
    let mut members = BTreeSet::new();
    for hole in crate::template_ir::text_holes(hir, id) {
        let Some((root, reads)) = path_of(body, hole.expr) else {
            continue;
        };
        // A part a signal decides is the browser's to render again
        // (ADR-0130). Inside a block a query decides, its address carries a
        // frame the browser would have to compute: not this slice.
        if signals.contains(&root) {
            if hole.nested {
                continue;
            }
            live.push(Live {
                part: hole.part.0,
                signal: root.clone(),
                path: std::iter::once(root.clone())
                    .chain(reads.iter().cloned())
                    .collect::<Vec<_>>()
                    .join("."),
                kind: "text".to_string(),
                reads: Vec::new(),
            });
            continue;
        }
        let Some((_, resource, _)) = found.iter().find(|(n, ..)| *n == root) else {
            // A loop's or an arm's name: the renderer reads it from the
            // collection, by field. A member read there is refused below.
            if !hole.nested {
                return Err(format!(
                    "part {} reads `{root}`, which no query binds",
                    hole.part.0
                ));
            }
            continue;
        };
        let ty = value_of(sigs, *resource)
            .ok_or_else(|| format!("`{root}`'s query has no resolved result"))?;
        let reads: Vec<&str> = reads.iter().map(String::as_str).collect();
        let planned = steps(sigs, ty, &reads)?;
        if hole.nested && planned.iter().any(|(s, _)| matches!(s, Step::Member(_))) {
            return Err(format!(
                "part {} reads a member of `{root}` inside a block, which a host would call \
                 per instance",
                hole.part.0
            ));
        }
        if hole.nested {
            continue;
        }
        let mut out = Vec::new();
        for (step, def) in planned {
            out.push(match (step, def) {
                (Step::Member(_), Some(def)) => {
                    members.insert(def);
                    Step::Member(
                        component_id_of(hirs, def)
                            .ok_or_else(|| "a member function with no identity".to_string())?,
                    )
                }
                (step, _) => step,
            });
        }
        let path = std::iter::once(root.clone())
            .chain(reads.iter().map(|r| r.to_string()))
            .collect::<Vec<_>>()
            .join(".");
        parts.push(Part {
            part: hole.part.0,
            path,
            binding: root,
            steps: out,
        });
    }

    // A block's collection, given whole: each binding the template iterates.
    let mut collections = Vec::new();
    for t in crate::template_ir::build(&[hir]) {
        if t.path != page {
            continue;
        }
        for entry in t.manifest() {
            if entry.kind == "each"
                && let Some(root) = entry.value.split('.').next()
                && found.iter().any(|(n, ..)| n == root)
                && !collections.iter().any(|c: &String| c == root)
            {
                collections.push(root.to_string());
            }
            // A block or an attribute a signal decides (ADR-0130).
            if matches!(
                entry.kind,
                "conditional" | "match" | "attribute" | "boolean_attribute"
            ) && let Some(root) = entry.value.split('.').next()
                && signals.iter().any(|s| s == root)
            {
                if entry.kind == "attribute" || entry.kind == "boolean_attribute" {
                    return Err(format!(
                        "part {} is an attribute a signal decides, which this slice does not \
                         render again (ADR-0130)",
                        entry.id.0
                    ));
                }
                let mut read = Vec::new();
                if let Some(part) = find_part(&t.chunks, entry.id.0) {
                    paths_read(part, &mut read);
                }
                let mut reads: Vec<String> = read
                    .iter()
                    .filter_map(|p| p.split('.').next())
                    .filter(|r| signals.iter().any(|s| s == r))
                    .map(str::to_string)
                    .collect();
                reads.sort();
                reads.dedup();
                live.push(Live {
                    part: entry.id.0,
                    signal: root.to_string(),
                    path: entry.value.clone(),
                    kind: entry.kind.to_string(),
                    reads,
                });
            }
        }
    }

    Ok((
        PageValues {
            page,
            params,
            bindings,
            parts,
            collections,
            signals: Vec::new(),
            live,
        },
        members,
    ))
}

/// **Every page's plan.**
pub fn pages(hirs: &[&Hir], ws: &Workspace, sigs: &Signatures) -> Vec<Planned> {
    let mut out = Vec::new();
    for (unit, hir) in hirs.iter().enumerate() {
        for (id, decl) in hir.all_decls() {
            if decl.kind != DeclKind::Page {
                continue;
            }
            let page = {
                let module = hir.module_of(id).unwrap_or_default();
                if module.is_empty() {
                    decl.name.clone()
                } else {
                    format!("{module}.{}", decl.name)
                }
            };
            out.push(Planned {
                page,
                plan: plan(hirs, ws, sigs, unit, id).map(|(p, _)| p),
            });
        }
    }
    out
}

/// **The member functions some page's plan calls**: each gets a contract of
/// kind `function` and is compiled as a component of its own (ADR-0125).
pub fn members(hirs: &[&Hir], ws: &Workspace, sigs: &Signatures) -> BTreeSet<DefId> {
    let mut out = BTreeSet::new();
    for (unit, hir) in hirs.iter().enumerate() {
        for (id, decl) in hir.all_decls() {
            if decl.kind == DeclKind::Page
                && let Ok((_, m)) = plan(hirs, ws, sigs, unit, id)
            {
                out.extend(m);
            }
        }
    }
    out
}
