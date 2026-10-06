//! **Optimistic transitions, compiled for the browser** (ADR-0122).
//!
//! ADR-0025 states the meaning:
//!
//! ```text
//! optimistic Cart(current_session()) as cart => Carts.with_line(cart, item, quantity)
//!
//! Cart(current_session())   which resource ENTRY is speculatively updated
//! cart                      a lexical name for its current value
//! Carts.with_line(..)       a PURE transition producing the speculative value
//! ```
//!
//! and until 2026-10-02 nothing executed it: `pw check` checked the clause and
//! no backend emitted anything for it. This module emits, per page, what the
//! browser needs to show a speculation and to abandon one:
//!
//! - `decode`: each speculated binding's value, read from the JSON the server
//!   sends (the page's embedded value, then each `entry_value` frame);
//! - `commands`: for each command a handler on the page calls, its
//!   transitions, as functions of the binding's value and the command's
//!   arguments as the handler sends them;
//! - `parts`: for each speculated binding, each text part that reads it, as a
//!   function of its value, so the page re-renders the speculative value.
//!
//! The runtime holds the authoritative value. A rejected command is undone by
//! restoring it, never by an inverse anyone wrote (ADR-0025).
//!
//! # What is refused, by name
//!
//! - a target whose key is not the page binding's: the page must show the
//!   entry the clause names, and the keys are compared by what they resolve
//!   to, an invocation-context call each (`current_session()`);
//! - a text part reading the binding inside a block (`{#each}`, `{#match}`,
//!   `{#if}`): its address carries a frame this slice does not compute;
//! - an attribute, what a block decides by, or a loop's list, reading the
//!   binding (ADR-0170): this module does not render it again, and the page
//!   would show two values at once;
//! - a part whose value has no text form here (a `Float`, ADR-0074).

use std::collections::{BTreeMap, BTreeSet};

use crate::hir::{DeclId, DeclKind, Expr, ExprId, Hir, Pattern};
use crate::resolve::{DefId, Namespace, Resolution, Workspace};
use crate::signatures::Signatures;

use super::ir::Lowering;
use super::ir::{Function, Type};
use super::lower::{self, Context};
use super::wasm::Encoding;

/// One page binding the module speculates on.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct Binding {
    /// The page's name for it: `cart`.
    pub binding: String,
    /// The resource it reads, as a component id: `store.page.Cart`.
    pub resource: String,
    /// Its key, as the page writes it: `["current_session()"]`, or
    /// `["current_session()", "shown"]` where the speculation's target leaves
    /// one unnamed (ADR-0222). The server finds the store's cart by it; any
    /// other binding's value is the one the document shows (ADR-0222).
    pub key: Vec<String>,
}

/// One page's speculation module.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Speculation {
    /// The page's template path: `store.page.StorePage`.
    pub page: String,
    /// The bindings the module speculates on.
    pub bindings: Vec<Binding>,
    /// The commands it speculates for, by component id.
    pub commands: Vec<String>,
    /// **Each part the browser renders again with a speculation**
    /// (ADR-0172): an attribute, a block or a loop at the top of the page
    /// that reads a speculated value. The document carries each one's
    /// template, which the browser's copy of the renderer renders.
    pub regions: Vec<u32>,
    /// The ES module.
    pub source: String,
}

/// **A part a speculation renders again in the browser** (ADR-0172).
struct Region {
    /// The speculated binding it reads: `cart`.
    binding: String,
    part: u32,
    kind: RegionKind,
}

/// What the browser renders again with a speculation (ADR-0172).
#[derive(Clone, Copy, PartialEq, Eq)]
enum RegionKind {
    Attribute,
    Block,
    List,
    /// What a handler at the top of the page captures (ADR-0217): its
    /// element's captures, written again.
    Captures,
}

/// **A speculated loop's rows** (ADR-0172): what the browser needs to render
/// one, beyond its template.
#[derive(Clone, Default)]
struct Rows {
    /// The loop's part.
    part: u32,
    /// The name a row binds: `line`.
    binding: String,
    /// The list, by its path: `cart.lines`.
    collection: String,
    /// The field a row is keyed by: `item_id`.
    key: String,
    /// Each read through a member, by its path in the row, and the function
    /// that computes it: `quantity.count`.
    computed: Vec<(String, usize)>,
}

/// Each part inside the part `id` of `chunks`, and each name a loop, an arm
/// or a stream's part binds inside it: what a region holds. `None` where no
/// part is `id`.
fn inside(
    chunks: &[crate::template_ir::Chunk],
    id: u32,
) -> Option<(BTreeSet<u32>, BTreeSet<String>)> {
    use crate::template_ir::Chunk;
    fn collect(
        p: &crate::template_ir::Part,
        parts: &mut BTreeSet<u32>,
        names: &mut BTreeSet<String>,
    ) {
        use crate::template_ir::{Chunk, Part};
        match p {
            Part::Each { binding, .. } => {
                names.insert(binding.clone());
            }
            Part::Match { arms, .. } => {
                for arm in arms {
                    names.extend(arm.binding.iter().cloned());
                    names.extend(arm.fields.iter().cloned());
                }
            }
            Part::Stream { ready, failed, .. } => {
                names.extend(ready.binding.iter().cloned());
                names.extend(failed.binding.iter().cloned());
            }
            _ => {}
        }
        for region in p.nested() {
            for c in region {
                if let Chunk::Dynamic(q) = c {
                    if let Some(id) = q.id() {
                        parts.insert(id.0);
                    }
                    collect(q, parts, names);
                }
            }
        }
    }
    for c in chunks {
        let Chunk::Dynamic(p) = c else { continue };
        if p.id().is_some_and(|i| i.0 == id) {
            let (mut parts, mut names) = (BTreeSet::new(), BTreeSet::new());
            collect(p, &mut parts, &mut names);
            return Some((parts, names));
        }
        for region in p.nested() {
            if let Some(found) = inside(region, id) {
                return Some(found);
            }
        }
    }
    None
}

/// The part `id` of `chunks`, wherever it is.
fn part_of(chunks: &[crate::template_ir::Chunk], id: u32) -> Option<&crate::template_ir::Part> {
    use crate::template_ir::Chunk;
    for c in chunks {
        let Chunk::Dynamic(p) = c else { continue };
        if p.id().is_some_and(|i| i.0 == id) {
            return Some(p);
        }
        for region in p.nested() {
            if let Some(found) = part_of(region, id) {
                return Some(found);
            }
        }
    }
    None
}

/// The element of the list at `fields` in a resource's value: `cart.lines`'s
/// `CartLine` (ADR-0172).
fn element_at(
    cx: &Context<'_>,
    resource: DefId,
    fields: &[&str],
) -> Option<crate::resolved::ResolvedType> {
    let mut ty = cx.sigs.by_def(resource)?.result()?.clone();
    if ty.as_builtin() == Some(crate::resolved::Builtin::Result) {
        ty = ty.args().first()?.clone();
    }
    for field in fields {
        ty = cx
            .sigs
            .type_decl(ty.def_id()?)?
            .record
            .as_ref()?
            .iter()
            .find(|(n, _)| n == field)?
            .1
            .resolved()?
            .clone();
    }
    (ty.as_builtin() == Some(crate::resolved::Builtin::List))
        .then(|| ty.args().first().cloned())
        .flatten()
}

/// What one page compiled to, or why it did not.
#[derive(Debug, Clone)]
pub struct Compiled {
    pub page: String,
    pub module: Encoding<Speculation>,
}

/// **Every page's speculation module.** A page none of whose handlers calls a
/// command with an optimistic clause has none, and is not listed.
pub fn compile(units: &[crate::check::Unit]) -> Result<Vec<Compiled>, String> {
    let hirs: Vec<&Hir> = units.iter().map(|u| &u.hir).collect();
    let ws = Workspace::build(&hirs);
    let sigs = Signatures::build(&ws, &hirs);
    let contracts = crate::contract::contracts(&hirs, &sigs, &ws);
    let cx = Context {
        hirs: &hirs,
        ws: &ws,
        sigs: &sigs,
        contracts: &contracts,
    };
    lower::Checked::of(units, cx).map_err(|ds| {
        format!(
            "the program does not check, so nothing is compiled: {}",
            ds.iter()
                .map(|d| format!("[{}] {}", d.code, d.message))
                .collect::<Vec<_>>()
                .join("; ")
        )
    })?;
    let sources: Vec<&str> = units.iter().map(|u| u.src.as_str()).collect();
    let handlers = super::js::handlers(&hirs, &sources, &ws, &sigs);
    let cx = Context {
        hirs: &hirs,
        ws: &ws,
        sigs: &sigs,
        contracts: &contracts,
    };
    let mut out = Vec::new();
    for (unit, hir) in hirs.iter().enumerate() {
        for (id, decl) in hir.all_decls() {
            if decl.kind != DeclKind::Page {
                continue;
            }
            // The handlers in the page's document: its own, and each composed
            // view's (ADR-0136). A view's button that calls an optimistic
            // command is the page's button.
            let mut written = vec![crate::contract::component_id(hir, id)];
            for v in crate::template_ir::lowered(
                &hirs,
                &ws,
                &sigs,
                &crate::template_ir::Handlers::new(),
                unit,
                id,
            )
            .map(|l| l.views)
            .unwrap_or_default()
            {
                written.push(crate::contract::component_id(hirs[v.unit], DeclId(v.decl)));
            }
            let mut commands: Vec<String> = Vec::new();
            for h in handlers.iter().filter(|h| written.contains(&h.declaration)) {
                if let Encoding::Encoded(m) = &h.module {
                    for c in &m.commands {
                        if !commands.contains(c) {
                            commands.push(c.clone());
                        }
                    }
                }
            }
            let speculating: Vec<String> = commands
                .into_iter()
                .filter(|c| {
                    command_decl(&hirs, c)
                        .is_some_and(|(u, d)| !hirs[u].decl(d).optimistic_clauses().is_empty())
                })
                .collect();
            if speculating.is_empty() {
                continue;
            }
            let page = template_path(hir, id);
            out.push(Compiled {
                page: page.clone(),
                module: page_module(&cx, unit, id, &page, &speculating),
            });
        }
    }
    Ok(out)
}

fn template_path(hir: &Hir, id: DeclId) -> String {
    let module = hir.module_of(id).unwrap_or_default();
    let name = &hir.decl(id).name;
    if module.is_empty() {
        name.clone()
    } else {
        format!("{module}.{name}")
    }
}

/// The command a component id names.
fn command_decl(hirs: &[&Hir], component: &str) -> Option<(usize, DeclId)> {
    hirs.iter().enumerate().find_map(|(u, hir)| {
        hir.all_decls()
            .find(|(id, d)| {
                d.kind == DeclKind::Command && crate::contract::component_id(hir, *id) == component
            })
            .map(|(id, _)| (u, id))
    })
}

/// A path written in `unit`, resolved as a term: a resource, a function.
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

/// A key argument, as what it resolves to: an invocation-context call with
/// no arguments, `current_session()`. Anything else is not compared.
fn context_call(ws: &Workspace, unit: usize, body: &crate::hir::Body, e: ExprId) -> Option<DefId> {
    let Expr::Call { callee, args } = body.expr(e) else {
        return None;
    };
    if !args.is_empty() {
        return None;
    }
    resolve_term(ws, unit, &crate::infer::path_of(body, *callee))
}

/// A page binding `let b = query R(k..)`.
struct PageBinding {
    name: String,
    resource: DefId,
    key: Vec<ExprId>,
}

fn page_bindings(ws: &Workspace, unit: usize, body: &crate::hir::Body) -> Vec<PageBinding> {
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
        out.push(PageBinding {
            name: name.clone(),
            resource,
            key: args.clone(),
        });
    }
    out
}

/// The value a resource's entry holds: its result, or `Ok`'s payload.
fn value_type(cx: &Context<'_>, resource: DefId, span: &crate::hir::Span) -> Lowering<Type> {
    let Some(sig) = cx.sigs.by_def(resource) else {
        return Lowering::Blocked {
            why: "a resource with no resolved signature".into(),
            span: span.clone(),
        };
    };
    let Some(result) = sig.result() else {
        return Lowering::Blocked {
            why: "a resource whose result is unresolved".into(),
            span: span.clone(),
        };
    };
    let value = match result.as_builtin() {
        Some(crate::resolved::Builtin::Result) => match result.args().first() {
            Some(t) => t.clone(),
            None => {
                return Lowering::Blocked {
                    why: "a `Result` with no arguments".into(),
                    span: span.clone(),
                };
            }
        },
        _ => result.clone(),
    };
    lower::backend_type(cx, &value, span)
}

/// Lowered, or the reason as an `Encoding`.
macro_rules! lowered {
    ($e:expr) => {
        match $e {
            Lowering::Lowered(x) => x,
            Lowering::Unsupported {
                construct, reason, ..
            } => return Encoding::Unsupported { construct, reason },
            Lowering::Blocked { why, .. } => return Encoding::Blocked { why },
        }
    };
}

/// One transition, ready to emit.
struct Transition {
    command: String,
    binding: String,
    function: usize,
    params: Vec<Type>,
}

fn page_module(
    cx: &Context<'_>,
    unit: usize,
    page_id: DeclId,
    page: &str,
    commands: &[String],
) -> Encoding<Speculation> {
    let hir = cx.hirs[unit];
    let Some(body_id) = hir.decl(page_id).body else {
        return Encoding::Blocked {
            why: format!("`{page}` has no body"),
        };
    };
    let body = hir.body(body_id);
    let bindings = page_bindings(cx.ws, unit, body);

    let mut functions: Vec<Function> = Vec::new();
    let mut transitions: Vec<Transition> = Vec::new();
    // binding name → (resource, its key as written, its value type)
    let mut speculated: Vec<(String, DefId, Vec<String>, Type)> = Vec::new();

    for command in commands {
        let Some((cu, cid)) = command_decl(cx.hirs, command) else {
            return Encoding::Blocked {
                why: format!("no command declares component `{command}`"),
            };
        };
        let chir = cx.hirs[cu];
        let cdecl = chir.decl(cid);
        let Some(cbody_id) = cdecl.body else {
            return Encoding::Blocked {
                why: format!("`{command}` has no body"),
            };
        };
        let cbody = chir.body(cbody_id);
        for (_, target, transition) in cdecl.optimistic_clauses() {
            let span = cbody.expr_span(target.root);
            let Expr::Call { callee, args } = cbody.expr(target.root) else {
                return Encoding::Blocked {
                    why: format!(
                        "`{command}`'s optimistic target is not a resource applied to a key"
                    ),
                };
            };
            let Some(resource) = resolve_term(cx.ws, cu, &crate::infer::path_of(cbody, *callee))
            else {
                return Encoding::Blocked {
                    why: format!("`{command}`'s optimistic target resolves to nothing"),
                };
            };
            // Each key: an invocation-context call, matched to the page's
            // own; or `_`, which matches the page's key whatever it is
            // (ADR-0222, ruling 0105-a): the timeline the page shows, at the
            // length it shows it.
            let target_key: Vec<Option<Option<DefId>>> = args
                .iter()
                .map(|a| match cbody.expr(a.value) {
                    Expr::Name(n) if n == "_" => Some(None),
                    _ => context_call(cx.ws, cu, cbody, a.value).map(Some),
                })
                .collect();
            let shown = bindings.iter().find(|b| {
                b.resource == resource
                    && b.key.len() == target_key.len()
                    && b.key.iter().zip(&target_key).all(|(k, t)| match t {
                        Some(None) => true,
                        Some(Some(call)) => context_call(cx.ws, unit, body, *k) == Some(*call),
                        None => false,
                    })
            });
            let Some(shown) = shown else {
                return Encoding::Unsupported {
                    construct: "a speculation on an entry the page does not show by the same key",
                    reason: format!(
                        "`{command}` speculates on an entry `{page}` reads by no binding whose \
                         key resolves to the same invocation context"
                    ),
                };
            };
            let value = lowered!(value_type(cx, resource, &span));
            // The command's parameters, typed as its signature declares.
            let Some(def) = cx.sigs.iter().map(|(_, s)| s).find(|s| {
                crate::resolve::declaration(cx.hirs, s.definition)
                    .is_some_and(|d| std::ptr::eq(d, cdecl))
            }) else {
                return Encoding::Blocked {
                    why: format!("`{command}` has no resolved signature"),
                };
            };
            let binder_name = transition
                .binders
                .first()
                .map(|(n, _)| n.clone())
                .unwrap_or_default();
            let mut inputs: Vec<(String, Type)> = vec![(binder_name, value.clone())];
            let mut params = Vec::new();
            for (i, p) in cdecl.params.iter().enumerate() {
                let Some(declared) = def
                    .params
                    .get(i)
                    .and_then(Option::as_ref)
                    .and_then(crate::resolved::TypeResolution::resolved)
                else {
                    return Encoding::Unsupported {
                        construct: "a command parameter with no declared type",
                        reason: format!("`{}` of `{command}`", p.name),
                    };
                };
                let ty = lowered!(lower::backend_type(cx, declared, &p.span));
                inputs.push((p.name.clone(), ty.clone()));
                params.push(ty);
            }
            let f = lowered!(lower::pure_expr(
                cx,
                cu,
                cid,
                transition.root,
                &inputs,
                &format!("{command}#optimistic"),
                cbody.expr_span(transition.root),
            ));
            functions.push(f);
            transitions.push(Transition {
                command: command.clone(),
                binding: shown.name.clone(),
                function: functions.len() - 1,
                params,
            });
            if !speculated.iter().any(|(n, ..)| *n == shown.name) {
                // Each key is an invocation-context call (`context_call`
                // matched it), written as the server computes it.
                let key = shown
                    .key
                    .iter()
                    .map(|k| match body.expr(*k) {
                        Expr::Call { callee, .. } => {
                            format!("{}()", crate::infer::path_of(body, *callee))
                        }
                        _ => crate::infer::path_of(body, *k),
                    })
                    .collect();
                speculated.push((shown.name.clone(), resource, key, value));
            }
        }
    }

    // With what each handler captures (ADR-0134): a region writes it, so a
    // capture of what the browser does not hold is refused below. Lowered
    // without it, an event part captured nothing, and such a handler built.
    let Some(lowered) = crate::template_ir::lowered(
        cx.hirs,
        cx.ws,
        cx.sigs,
        &crate::resume::capture_map(cx.hirs, cx.sigs),
        unit,
        page_id,
    ) else {
        return Encoding::Blocked {
            why: format!("`{page}` has no template"),
        };
    };
    let speculates = |root: &str| speculated.iter().any(|(n, ..)| n == root);

    // **A value computed from a speculated one** (ADR-0228) is computed
    // here, as the browser renders the speculation: a text part's from the
    // value whole, below, and a row's from its item whole, with its row. An
    // attribute's, or one from a field of the value, would show the server's
    // beside a list that shows the speculation: refused, by name.
    let computed = lowered
        .holes
        .iter()
        .filter_map(|h| Some((h.part.0, h.inputs.as_ref()?, "a value")))
        .chain(lowered.reads.iter().filter_map(|r| {
            let what = match r.kind {
                crate::template_ir::ReadKind::Subject => "a block's subject",
                _ => "an attribute's value",
            };
            Some((r.part.0, r.inputs.as_ref()?, what))
        }));
    for (part, inputs, what) in computed {
        for (name, read) in inputs {
            let root = read.split('.').next().unwrap_or_default();
            if speculates(root) && (what != "a value" || read != root) {
                return Encoding::Unsupported {
                    construct: "a value computed from a speculated one",
                    reason: format!(
                        "part {part} of `{page}` computes {what} from `{name}`, which the page \
                         speculates on, and the browser computes a text part from the value \
                         whole, between tags (ruling 0073-a)"
                    ),
                };
            }
        }
    }

    // **What the browser renders again with a speculation** (ADR-0172):
    // each attribute, block and loop at the top of the page that reads a
    // speculated value. Until 2026-10-03 one was refused (ADR-0170), and
    // until then it was left as it was, beside a count that moved.
    let mut regions: Vec<Region> = Vec::new();
    for read in &lowered.reads {
        let root = read.path.split('.').next().unwrap_or_default();
        if read.nested || !speculates(root) || regions.iter().any(|r| r.part == read.part.0) {
            continue;
        }
        let kind = match read.kind {
            crate::template_ir::ReadKind::Attribute => RegionKind::Attribute,
            crate::template_ir::ReadKind::Subject => RegionKind::Block,
            crate::template_ir::ReadKind::List => RegionKind::List,
            // One for each element: its handlers' captures are one attribute.
            crate::template_ir::ReadKind::Captures => {
                let owner =
                    |part: u32| {
                        lowered.template.chunks.iter().find_map(|c| match c {
                            crate::template_ir::Chunk::Dynamic(
                                crate::template_ir::Part::Event { id, owner, .. },
                            ) if id.0 == part => Some(*owner),
                            _ => None,
                        })
                    };
                let mine = owner(read.part.0);
                if mine.is_none()
                    || regions
                        .iter()
                        .any(|r| r.kind == RegionKind::Captures && owner(r.part) == mine)
                {
                    continue;
                }
                RegionKind::Captures
            }
            // Metadata is written into the head as the page is served, and
            // set again by nothing (ADR-0186): what a press speculates is no
            // metadata's.
            crate::template_ir::ReadKind::Meta => continue,
            // **A title that reads a speculated value** (ADR-0183): the
            // browser renders no title again from a speculation, so the
            // title would say what the value was while the page around it
            // says what it is about to be.
            crate::template_ir::ReadKind::Title => {
                return Encoding::Unsupported {
                    construct: "a title that reads a speculated value",
                    reason: format!(
                        "`{page}`'s title reads `{}`, which a speculation changes before \
                         the server answers, and the browser renders no title again from one",
                        read.path
                    ),
                };
            }
        };
        regions.push(Region {
            binding: root.to_string(),
            part: read.part.0,
            kind,
        });
    }
    // What a region holds, the browser renders from what it holds: the
    // speculated value, the page's signals, and the names bound inside it.
    // Anything else it does not hold, as a signal's block does not (ADR-0137).
    let signals: BTreeSet<String> = lowered.instances.iter().map(|i| i.name.clone()).collect();
    let mut within: Vec<(u32, BTreeSet<u32>)> = Vec::new();
    for region in &regions {
        let Some((parts, bound)) = inside(&lowered.template.chunks, region.part) else {
            continue;
        };
        let held =
            |root: &str| root == region.binding || bound.contains(root) || signals.contains(root);
        let paths = lowered
            .holes
            .iter()
            .filter(|h| parts.contains(&h.part.0))
            .map(|h| (h.part.0, h.path.clone()))
            .chain(
                lowered
                    .reads
                    .iter()
                    // A handler's captures are checked below, by name.
                    .filter(|r| {
                        parts.contains(&r.part.0)
                            && r.kind != crate::template_ir::ReadKind::Captures
                    })
                    .map(|r| (r.part.0, r.path.clone())),
            );
        for (part, path) in paths {
            let root = path.split('.').next().unwrap_or_default();
            if !held(root) {
                return Encoding::Unsupported {
                    construct: "a value a speculated region reads that the browser does not hold",
                    reason: format!(
                        "part {part} of `{page}` reads `{path}` inside part {}, which the \
                         browser renders again with a speculation of `{}`",
                        region.part, region.binding
                    ),
                };
            }
        }
        // And what each handler in it captures, which the region writes.
        for part in &parts {
            if let Some(crate::template_ir::Part::Event {
                captures, renames, ..
            }) = part_of(&lowered.template.chunks, *part)
            {
                for path in captures {
                    let root = path.split('.').next().unwrap_or_default();
                    let root = renames
                        .get(root)
                        .map(|to| to.split('.').next().unwrap_or_default().to_string())
                        .unwrap_or_else(|| root.to_string());
                    if !held(&root) {
                        return Encoding::Unsupported {
                            construct: "a value a speculated region reads that the browser does not hold",
                            reason: format!(
                                "part {part} of `{page}` captures `{path}` inside part {}, \
                                 which the browser renders again with a speculation of `{}`",
                                region.part, region.binding
                            ),
                        };
                    }
                }
            }
        }
        within.push((region.part, parts));
    }
    let in_a_region = |part: u32| within.iter().any(|(_, parts)| parts.contains(&part));
    // A speculated value read inside a block no region renders: nothing
    // would render it again.
    for read in &lowered.reads {
        let root = read.path.split('.').next().unwrap_or_default();
        if read.nested && speculates(root) && !in_a_region(read.part.0) {
            return Encoding::Unsupported {
                construct: "a speculated value read where this module does not render it again",
                reason: format!(
                    "part {} of `{page}` reads `{}` inside a block, which a speculation \
                     would not reach",
                    read.part.0, read.path
                ),
            };
        }
    }

    // **What a speculated row reads of its item through a member** (ADR-0172):
    // computed here, as a host computes it for a row (ADR-0169), and set in
    // the row whole, by its path (ADR-0170).
    let mut typed = BTreeMap::new();
    let mut rows: Vec<Rows> = Vec::new();
    for region in regions.iter().filter(|r| r.kind == RegionKind::List) {
        let Some(crate::template_ir::Part::Each {
            binding,
            collection,
            key,
            ..
        }) = part_of(&lowered.template.chunks, region.part)
        else {
            continue;
        };
        let Some((_, resource, _, _)) = speculated.iter().find(|(n, ..)| *n == region.binding)
        else {
            continue;
        };
        let fields: Vec<&str> = collection.split('.').skip(1).collect();
        let Some(element) = element_at(cx, *resource, &fields) else {
            return Encoding::Unsupported {
                construct: "a speculated loop over what is not a list of records",
                reason: format!("part {} of `{page}` iterates `{collection}`", region.part),
            };
        };
        let span = crate::hir::Span::default();
        let element = lowered!(lower::backend_type(cx, &element, &span));
        let parts = within
            .iter()
            .find(|(p, _)| *p == region.part)
            .map(|(_, parts)| parts.clone())
            .unwrap_or_default();
        let mut computed: Vec<(String, usize)> = Vec::new();
        let written = lowered
            .holes
            .iter()
            .filter(|h| parts.contains(&h.part.0))
            .map(|h| {
                (
                    h.path.clone(),
                    h.origin,
                    crate::template_ir::ReadAt::Expr(h.expr),
                    h.inputs.clone(),
                )
            })
            .chain(
                lowered
                    .reads
                    .iter()
                    .filter(|r| parts.contains(&r.part.0))
                    .map(|r| (r.path.clone(), r.origin, r.at, r.inputs.clone())),
            )
            .collect::<Vec<_>>();
        for (path, origin, at, inputs) in written {
            let Some((root, rest)) = path.split_once('.') else {
                continue;
            };
            let crate::template_ir::ReadAt::Expr(expr) = at else {
                continue;
            };
            if root != binding.as_str() || computed.iter().any(|(p, _)| p == rest) {
                continue;
            }
            // **A value computed from the row's item** (ADR-0228), whole: the
            // function the compiler lifted, compiled here, as a host runs it
            // for each row. From a field of the item, refused by name.
            if let Some([(name, read)]) = inputs.as_deref() {
                if read != binding {
                    return Encoding::Unsupported {
                        construct: "a speculated row's value computed from a field of its item",
                        reason: format!(
                            "part {} of `{page}` computes a value from `{read}`, a field of a \
                             row the page speculates on, and the browser computes one from \
                             the row's item whole (ruling 0073-a)",
                            region.part
                        ),
                    };
                }
                let (u, d) = origin;
                let Some(body) = cx.hirs[u].decl(d).body.map(|b| cx.hirs[u].body(b)) else {
                    continue;
                };
                let f = lowered!(lower::pure_expr(
                    cx,
                    u,
                    d,
                    expr,
                    &[(name.clone(), element.clone())],
                    &format!("{page}#row{}", region.part),
                    body.expr_span(expr),
                ));
                functions.push(f);
                computed.push((rest.to_string(), functions.len() - 1));
                continue;
            }
            if !crate::page_values::calls_a_member(cx.hirs, cx.ws, cx.sigs, &mut typed, origin, at)
            {
                continue;
            }
            // Compiled in the body it is written in, where the item may have
            // another name: a view's parameter (ADR-0136).
            let (u, d) = origin;
            let Some(body) = cx.hirs[u].decl(d).body.map(|b| cx.hirs[u].body(b)) else {
                continue;
            };
            let Some(own) =
                crate::template_ir::value_path_typed((cx.sigs, cx.ws), (cx.hirs, u, d), body, expr)
            else {
                continue;
            };
            let (name, own_rest) = own.split_once('.').unwrap_or((own.as_str(), ""));
            if own_rest != rest {
                return Encoding::Unsupported {
                    construct: "a speculated row's read through a view given a field",
                    reason: format!(
                        "`{path}` in part {} of `{page}` is read through a view given a field \
                         of its item",
                        region.part
                    ),
                };
            }
            let f = lowered!(lower::pure_expr(
                cx,
                u,
                d,
                expr,
                &[(name.to_string(), element.clone())],
                &format!("{page}#row{}", region.part),
                body.expr_span(expr),
            ));
            functions.push(f);
            computed.push((rest.to_string(), functions.len() - 1));
        }
        rows.push(Rows {
            part: region.part,
            binding: binding.clone(),
            collection: collection.clone(),
            key: key.clone().unwrap_or_default(),
            computed,
        });
    }

    // Each part reading a speculated binding: the page's own, and a view's
    // composed into it (ADR-0136), by the path the template reads.
    let mut reads: Vec<(String, u32, usize)> = Vec::new();
    for hole in lowered.holes.clone() {
        // **A text part computed from a speculated value** (ADR-0228), whole:
        // its function, compiled here, from the value the page holds now.
        if let Some([(name, read)]) = hole.inputs.as_deref() {
            let Some((_, _, _, value)) = speculated.iter().find(|(n, ..)| n == read) else {
                continue;
            };
            if hole.nested {
                continue;
            }
            let (at, decl) = hole.origin;
            let Some(written) = cx.hirs[at].decl(decl).body.map(|b| cx.hirs[at].body(b)) else {
                continue;
            };
            let f = lowered!(lower::pure_expr(
                cx,
                at,
                decl,
                hole.expr,
                &[(name.clone(), value.clone())],
                &format!("{page}#part{}", hole.part.0),
                written.expr_span(hole.expr),
            ));
            if !matches!(f.ret, Type::Int | Type::Str | Type::Bool) {
                return Encoding::Unsupported {
                    construct: "a speculated part with no text form here",
                    reason: format!("part {} of `{page}` is a {:?}", hole.part.0, f.ret),
                };
            }
            functions.push(f);
            reads.push((read.clone(), hole.part.0, functions.len() - 1));
            continue;
        }
        let root = hole.path.split('.').next().unwrap_or_default();
        let Some((_, _, _, value)) = speculated.iter().find(|(n, ..)| n == root) else {
            continue;
        };
        if hole.nested {
            // Rendered with its region (ADR-0172).
            if in_a_region(hole.part.0) {
                continue;
            }
            return Encoding::Unsupported {
                construct: "a speculated value read inside a block",
                reason: format!(
                    "part {} of `{page}` reads `{root}` inside a block, whose instances \
                     this module does not address",
                    hole.part.0
                ),
            };
        }
        // The expression is compiled in the body it is written in. In a view,
        // its first name is the view's parameter, given the binding whole:
        // the view's path and the template's differ in that name alone.
        let (at, decl) = hole.origin;
        let Some(written) = cx.hirs[at].decl(decl).body.map(|b| cx.hirs[at].body(b)) else {
            continue;
        };
        let Some(own) = crate::template_ir::value_path_typed(
            (cx.sigs, cx.ws),
            (cx.hirs, at, decl),
            written,
            hole.expr,
        ) else {
            continue;
        };
        let tail = |p: &str| p.split_once('.').map(|(_, t)| t.to_string());
        if tail(&own) != tail(&hole.path) {
            return Encoding::Unsupported {
                construct: "a speculated value given to a view in part",
                reason: format!(
                    "part {} of `{page}` reads `{}` through a view given a field of `{root}`, \
                     and a view's part is recomputed from a binding given whole",
                    hole.part.0, hole.path
                ),
            };
        }
        let name = own.split('.').next().unwrap_or_default().to_string();
        let f = lowered!(lower::pure_expr(
            cx,
            at,
            decl,
            hole.expr,
            &[(name, value.clone())],
            &format!("{page}#part{}", hole.part.0),
            written.expr_span(hole.expr),
        ));
        if !matches!(f.ret, Type::Int | Type::Str | Type::Bool) {
            return Encoding::Unsupported {
                construct: "a speculated part with no text form here",
                reason: format!("part {} of `{page}` is a {:?}", hole.part.0, f.ret),
            };
        }
        functions.push(f);
        reads.push((root.to_string(), hole.part.0, functions.len() - 1));
    }

    let program = lower::program_of(cx, functions);
    let mut source = format!(
        "// Generated by pw: optimistic speculation for {page} (ADR-0122). Do not edit.\n\n"
    );
    for (n, f) in program.functions.iter().enumerate() {
        match super::js_pure::isolated(f, &program) {
            Ok(js) => source.push_str(&format!("const f{n} = {js};\n\n")),
            Err(reason) => {
                return Encoding::Unsupported {
                    construct: "a speculation outside the JavaScript backend",
                    reason,
                };
            }
        }
    }
    source.push_str("export const decode = {\n");
    for (name, _, _, value) in &speculated {
        match super::js_pure::decoder(&program, value, "j") {
            Ok(d) => source.push_str(&format!("  {}: (j) => {d},\n", json(name))),
            Err(reason) => {
                return Encoding::Unsupported {
                    construct: "a speculated value the server cannot send",
                    reason,
                };
            }
        }
    }
    source.push_str("};\n\nexport const parts = {\n");
    for (name, ..) in &speculated {
        let mine: Vec<String> = reads
            .iter()
            .filter(|(b, ..)| b == name)
            .map(|(_, part, f)| format!("{}: f{f}", json(&part.to_string())))
            .collect();
        source.push_str(&format!("  {}: {{ {} }},\n", json(name), mine.join(", ")));
    }
    // Each region, by the binding it reads (ADR-0172): what the browser
    // renders again from the speculated value, and for a loop, what each row
    // reads through a member.
    source.push_str("};\n\nexport const regions = {\n");
    for (name, ..) in &speculated {
        let mine: Vec<String> = regions
            .iter()
            .filter(|r| r.binding == *name)
            .map(|r| match r.kind {
                RegionKind::Attribute => {
                    format!("{{ kind: \"attribute\", part: {} }}", r.part)
                }
                RegionKind::Block => {
                    format!("{{ kind: \"block\", part: {} }}", r.part)
                }
                RegionKind::Captures => {
                    format!("{{ kind: \"captures\", part: {} }}", r.part)
                }
                RegionKind::List => {
                    let found = rows
                        .iter()
                        .find(|rows| rows.part == r.part)
                        .cloned()
                        .unwrap_or_default();
                    let reads: Vec<String> = found
                        .computed
                        .iter()
                        .map(|(path, f)| format!("{}: f{f}", json(path)))
                        .collect();
                    format!(
                        "{{ kind: \"list\", part: {}, binding: {}, collection: {}, key: {}, rows: {{ {} }} }}",
                        r.part,
                        json(&found.binding),
                        json(&found.collection),
                        json(&found.key),
                        reads.join(", ")
                    )
                }
            })
            .collect();
        source.push_str(&format!("  {}: [{}],\n", json(name), mine.join(", ")));
    }
    source.push_str("};\n\nexport const commands = {\n");
    for command in commands {
        let mine: Vec<String> = transitions
            .iter()
            .filter(|t| t.command == *command)
            .map(|t| {
                let args: Vec<String> = t
                    .params
                    .iter()
                    .enumerate()
                    .map(|(i, ty)| super::js_pure::decoder(&program, ty, &format!("args[{i}]")))
                    .collect::<Result<_, _>>()?;
                Ok(format!(
                    "{{ binding: {}, transition: (value, args) => f{}(value, {}) }}",
                    json(&t.binding),
                    t.function,
                    args.join(", ")
                ))
            })
            .collect::<Result<_, String>>()
            .unwrap_or_default();
        if !mine.is_empty() {
            source.push_str(&format!("  {}: [{}],\n", json(command), mine.join(", ")));
        }
    }
    source.push_str("};\n");

    Encoding::Encoded(Speculation {
        page: page.to_string(),
        bindings: speculated
            .iter()
            .map(|(name, resource, key, _)| Binding {
                binding: name.clone(),
                resource: crate::resolve::declaration(cx.hirs, *resource)
                    .and_then(|_| {
                        cx.hirs.iter().find_map(|h| {
                            h.all_decls()
                                .find(|(_, d)| {
                                    crate::resolve::declaration(cx.hirs, *resource)
                                        .is_some_and(|r| std::ptr::eq(r, *d))
                                })
                                .map(|(id, _)| crate::contract::component_id(h, id))
                        })
                    })
                    .unwrap_or_default(),
                key: key.clone(),
            })
            .collect(),
        commands: transitions
            .iter()
            .map(|t| t.command.clone())
            .fold(Vec::new(), |mut v, c| {
                if !v.contains(&c) {
                    v.push(c);
                }
                v
            }),
        regions: regions.iter().map(|r| r.part).collect(),
        source,
    })
}

fn json(s: &str) -> String {
    serde_json::Value::String(s.to_string()).to_string()
}
