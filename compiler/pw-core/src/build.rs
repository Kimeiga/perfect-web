//! **One program, every artifact** — what `pw build` writes.
//!
//! E10's gate: "Store application builds from source to browser artifacts and
//! Wasm Components without Koka or Marko." Until 2026-09-25 the store's
//! artifacts came from four commands, and three of its five components were
//! compiled only inside tests. This composes the stages that already own each
//! artifact; it decides nothing itself:
//!
//! ```text
//! the template IR          template_ir::build_with, with the handler
//!                          identities resume_artifacts derived
//! the handler modules      backend::js::compile
//! the components           backend::component::compile_all, each audited
//! the contracts, the WIT   contract::contracts, wit::package
//! the resource graph       graph::Graph::build (since ADR-0123)
//! the speculations         backend::speculation::compile (ADR-0122)
//! ```
//!
//! Nothing here reaches Koka or Marko. `pw emit-koka` and `pw emit-marko` are
//! separate commands, and this module calls neither backend.

use crate::backend::component::Built;
use crate::backend::js;
use crate::check::Unit;

/// Every artifact of one program.
#[derive(Debug, Clone)]
pub struct Build {
    /// What the renderer renders, with each event part's handler identity.
    pub templates: Vec<crate::template_ir::Template>,
    /// Each resumable handler, compiled or refused.
    pub handlers: Vec<js::Compiled>,
    /// Each contract, in contract order: a component, a declaration with no
    /// component body, or a refusal.
    pub components: Vec<(String, Built)>,
    /// Each query that reaches no host, as an ES module (ADR-0044), by
    /// component id.
    pub modules: Vec<(String, String)>,
    pub contracts: Vec<crate::contract::ComponentContract>,
    pub wit: String,
    /// What each command emits and each entry listens for (ADR-0123): what
    /// a host's materializer consumes. Until 2026-10-02 the development
    /// server compiled a committed copy into itself.
    pub graph: crate::graph::Graph,
    /// Each page's optimistic speculation module (ADR-0122).
    pub speculations: Vec<crate::backend::speculation::Compiled>,
    /// What each page shows, as reads of what its queries return (ADR-0125).
    pub pages: Vec<crate::page_values::Planned>,
}

impl Build {
    /// Everything that was refused, one line each. A build with any is not
    /// finished: a page would render a button whose handler cannot load, or a
    /// host would be asked for a component that does not exist.
    pub fn refusals(&self) -> Vec<String> {
        let handlers = self.handlers.iter().filter_map(|h| match &h.module {
            crate::backend::wasm::Encoding::Encoded(_) => None,
            other => Some(format!("a handler in `{}`: {other}", h.declaration)),
        });
        let components = self.components.iter().filter_map(|(id, b)| match b {
            Built::Refused(why) => Some(format!("`{id}`: {why}")),
            _ => None,
        });
        // A placeholder is only a refusal once something depends on it: a
        // component whose contract imports one would call a body nobody wrote.
        let placeholders: Vec<&str> = self
            .components
            .iter()
            .filter(|(_, b)| matches!(b, Built::Placeholder))
            .map(|(id, _)| id.as_str())
            .collect();
        let depended = self.contracts.iter().flat_map(|c| {
            c.imports
                .iter()
                .filter(|i| i.kind == crate::contract::ImportKind::Component)
                .filter_map(|i| i.interface.strip_prefix("pw:app/"))
                .filter(|target| placeholders.contains(target))
                .map(|target| {
                    format!(
                        "`{}` depends on `{target}`, whose body is a placeholder (`todo`)",
                        c.component_id
                    )
                })
                .collect::<Vec<_>>()
        });
        // A declared optimistic clause the browser cannot execute is a
        // declaration that silently does nothing (ADR-0120).
        let speculations = self.speculations.iter().filter_map(|s| match &s.module {
            crate::backend::wasm::Encoding::Encoded(_) => None,
            other => Some(format!("`{}`'s speculations: {other}", s.page)),
        });
        // A graph edge naming nothing never fires (`pw emit-graph`'s rule).
        let dangling = self
            .graph
            .dangling
            .iter()
            .map(|d| format!("a graph edge from `{}` names nothing: `{}`", d.from, d.name));
        // An event part with no code is a button that does nothing when
        // pressed (ADR-0134). Every `on:` lambda is a handler with an
        // identity now; what remains is a handler that is not a lambda,
        // `on:submit={save}`, which waits for the event to be passed
        // (ADR-0131).
        let inert = self.templates.iter().flat_map(|t| {
            t.manifest()
                .into_iter()
                .filter(|p| p.kind == "event" && p.value.is_empty())
                .map(move |p| {
                    format!(
                        "`{}`: element {} handles an event with no code to run: write its \
                         handler as a lambda, `on:press={{() => ..}}` (ADR-0134)",
                        t.path,
                        p.owner.map(|o| o.0).unwrap_or_default()
                    )
                })
        });
        // A page whose values no plan states would be rendered by a host
        // guessing them (ADR-0125).
        let pages = self.pages.iter().filter_map(|p| {
            p.plan
                .as_ref()
                .err()
                .map(|why| format!("`{}`'s values: {why}", p.page))
        });
        handlers
            .chain(components)
            .chain(depended)
            .chain(speculations)
            .chain(dangling)
            .chain(inert)
            .chain(pages)
            .collect()
    }

    /// The contracts whose bodies are placeholders nothing depends on: listed
    /// by a build, never written as components.
    pub fn placeholders(&self) -> Vec<&str> {
        self.components
            .iter()
            .filter(|(_, b)| matches!(b, Built::Placeholder))
            .map(|(id, _)| id.as_str())
            .collect()
    }
}

/// **Build a checked program.** A program that does not check builds nothing.
pub fn build(units: &[Unit]) -> Result<Build, String> {
    // The gate every backend shares: `js::compile` and `compile_all` each
    // refuse an unchecked program, through `lower::Checked`.
    let handlers = js::compile(units)?;
    let components = crate::backend::component::compile_all(units)?;
    let modules = crate::backend::js_pure::modules(units)?;

    let hirs: Vec<&crate::hir::Hir> = units.iter().map(|u| &u.hir).collect();
    let ws = crate::resolve::Workspace::build(&hirs);
    let sigs = crate::signatures::Signatures::build(&ws, &hirs);
    let contracts = crate::contract::contracts(&hirs, &sigs, &ws);
    let (wit, _) = crate::wit::package(&hirs, &ws, &contracts).map_err(|e| format!("WIT: {e}"))?;

    let sources: Vec<&str> = units.iter().map(|u| u.src.as_str()).collect();
    let templates = templates(&hirs, &sources, &sigs);
    // A part the renderer refuses is a template that fails every render.
    // `pw emit-template` refuses one; until 2026-09-26 `pw build` wrote it
    // (ADR-0073).
    let blocked: Vec<String> = templates
        .iter()
        .flat_map(|t| {
            t.blocked().into_iter().filter_map(move |b| match b {
                crate::template_ir::Part::Blocked { reason, at } => {
                    Some(format!("{}: `{at}`: {reason}", t.path))
                }
                _ => None,
            })
        })
        .collect();
    if !blocked.is_empty() {
        return Err(format!(
            "a template part the renderer cannot render: {}",
            blocked.join("; ")
        ));
    }

    let graph = crate::graph::Graph::build(&hirs, &ws);
    let speculations = crate::backend::speculation::compile(units)?;
    let mut pages = crate::page_values::pages(&hirs, &ws, &sigs);
    // Each page's signals' first values (ADR-0130), which the backend
    // computes, into the plan the server renders from. A page whose signals
    // did not compile has no plan: the server would render it guessing.
    for compiled in crate::backend::signals::compile(units)? {
        let Some(planned) = pages.iter_mut().find(|p| p.page == compiled.page) else {
            continue;
        };
        match (compiled.signals, &mut planned.plan) {
            (Ok(signals), Ok(plan)) => plan.signals = signals,
            (Err(why), plan @ Ok(_)) => *plan = Err(format!("its signals: {why}")),
            (_, Err(_)) => {}
        }
    }
    Ok(Build {
        templates,
        handlers,
        components,
        modules,
        contracts,
        wit,
        graph,
        speculations,
        pages,
    })
}

/// **The template IR, with every event part's handler identity**: what
/// `pw emit-template` prints and `pw build` writes, from one derivation.
///
/// Per unit, because `resume_artifacts::located` walks one HIR against the
/// whole program's signatures.
pub fn templates(
    hirs: &[&crate::hir::Hir],
    sources: &[&str],
    sigs: &crate::signatures::Signatures,
) -> Vec<crate::template_ir::Template> {
    let mut identities = crate::template_ir::Handlers::new();
    for (hir, src) in hirs.iter().zip(sources) {
        for (decl, lambda, m, _) in
            crate::resume_artifacts::located(src, hir, sigs, crate::resume_artifacts::BUILD)
        {
            identities.insert((decl, lambda), (m.handler, m.capture_paths));
        }
    }
    crate::template_ir::build_with(hirs, &identities)
}
