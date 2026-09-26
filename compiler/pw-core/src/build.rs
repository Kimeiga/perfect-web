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
        handlers.chain(components).chain(depended).collect()
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

    Ok(Build {
        templates,
        handlers,
        components,
        modules,
        contracts,
        wit,
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
            identities.insert((decl, lambda), m.handler);
        }
    }
    crate::template_ir::build_with(hirs, &identities)
}
