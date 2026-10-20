//! **What a page computes from its signals, as the browser computes it**
//! (ADR-0227).
//!
//! A value a template computes from a signal (ruling 0073-a) is the
//! browser's: it changes as a person types, and no host hears of it. The
//! compiler lifts it into a function (ADR-0226), a component a host runs for
//! the page's first render, and compiles the same function here, with the
//! browser's pure backend (`js_pure`, ADR-0044), into one module per page.
//! The runtime loads it when one of those signals first changes, and runs
//! each function with the signal's value as the document holds it, in
//! `js_pure`'s wire form.

use crate::page_values::{Derived, Live};

use super::ir::Lowering;
use super::lower::{self, Context};

/// One page's module, or why it did not compile.
#[derive(Debug, Clone)]
pub struct Compiled {
    /// The page's template path: `feed.app.Home`.
    pub page: String,
    /// Its source: each function, and `parts`, each part's by its number.
    pub module: Result<String, String>,
}

/// **Every page's module.** A page that computes nothing from a signal has
/// none, and is not listed.
pub fn compile(units: &[crate::check::Unit]) -> Result<Vec<Compiled>, String> {
    crate::program::with_backend(units, compile_checked)
}

pub(crate) fn compile_checked(
    checked: &super::lower::Checked<'_>,
) -> Result<Vec<Compiled>, String> {
    let cx = checked.context();

    let derived = &checked.pages().derived;
    let mut out = Vec::new();
    for planned in &checked.pages().pages {
        let Ok(plan) = &planned.plan else { continue };
        let computed: Vec<&Live> = plan.live.iter().filter(|l| !l.derived.is_empty()).collect();
        // And the steps each value the device keeps takes from an earlier
        // version to its own (track `device-store`), which the runtime loads
        // only where the device holds an earlier one.
        let migrating = plan.device.iter().any(|h| h.version > 1);
        if computed.is_empty() && !migrating {
            continue;
        }
        let module = module(cx, &planned.page, &computed, derived);
        out.push(Compiled {
            module: migrated(module, cx, &planned.page, migrating),
            page: planned.page.clone(),
        });
    }
    Ok(out)
}

/// Reuse the exact functions already lowered for the host's first render.
pub(crate) fn compile_prepared(
    checked: &super::lower::Checked<'_>,
    compiled: &super::component::Program<'_>,
) -> Result<Vec<Compiled>, String> {
    let analysis = checked.pages();
    let program = compiled.lowered();
    let mut out = Vec::new();
    for planned in &analysis.pages {
        let Ok(plan) = &planned.plan else {
            continue;
        };
        let computed: Vec<&Live> = plan.live.iter().filter(|l| !l.derived.is_empty()).collect();
        // And the steps each value the device keeps takes from an earlier
        // version to its own (track `device-store`), as `compile_checked`.
        let migrating = plan.device.iter().any(|h| h.version > 1);
        if computed.is_empty() && !migrating {
            continue;
        }
        let functions: Option<Vec<_>> = computed
            .iter()
            .map(|live| {
                let derived = analysis
                    .derived
                    .iter()
                    .find(|d| d.component_id == live.derived)?;
                program
                    .functions
                    .iter()
                    .find(|f| f.def == derived.declaration() && f.export == derived.export())
            })
            .collect();
        let module = match functions {
            Some(functions) => emit(&planned.page, &computed, &functions, program),
            // Preserve the original stage's precise refusal if lowering failed.
            None => module(
                checked.context(),
                &planned.page,
                &computed,
                &analysis.derived,
            ),
        };
        out.push(Compiled {
            page: planned.page.clone(),
            module: migrated(module, checked.context(), &planned.page, migrating),
        });
    }
    Ok(out)
}

/// **A page's module, with the migrations of the values the device keeps**
/// (track `device-store`, ADR-0366) where it holds one at a later version:
/// the runtime loads them only where the device holds an earlier one.
fn migrated(
    module: Result<String, String>,
    cx: &Context<'_>,
    page: &str,
    migrating: bool,
) -> Result<String, String> {
    module.and_then(|mut source| {
        if migrating {
            source.push_str(&migrations(cx, page)?);
        }
        Ok(source)
    })
}

/// **Each value the device keeps that `page` holds, its steps from each
/// earlier version to the next** (track `device-store`, ADR-0366): `export
/// const migrations`, by the signal, each step a function from the stored
/// value's wire form to the next version's.
fn migrations(cx: &Context<'_>, page: &str) -> Result<String, String> {
    let mut found = None;
    for (unit, hir) in cx.hirs.iter().enumerate() {
        for (id, decl) in hir.all_decls() {
            if decl.kind == crate::hir::DeclKind::Page
                && crate::contract::component_id(hir, id) == page
            {
                found = Some((unit, id));
            }
        }
    }
    let (unit, id) = found.ok_or_else(|| format!("no page `{page}`"))?;
    let mut source = String::from("\nexport const migrations = {\n");
    for (signal, def) in crate::device_store::held_by(cx.hirs, cx.ws, cx.sigs, unit, id) {
        let Some(kept) =
            crate::resolve::declaration(cx.hirs, def).and_then(crate::device_store::Kept::of)
        else {
            continue;
        };
        if kept.version < 2 {
            continue;
        }
        let mut steps = Vec::new();
        for step in &kept.migrations {
            let target = match cx.sigs.workspace().resolve(def.unit, step) {
                crate::resolve::Resolution::Local(d)
                | crate::resolve::Resolution::Imported { def: d, .. } => d,
                _ => return Err(format!("`{signal}`'s migration `{step}` names nothing")),
            };
            let decl = crate::resolve::declaration(cx.hirs, target)
                .ok_or_else(|| format!("`{step}` has no declaration"))?;
            let span = decl.name_span.clone();
            let function = match lower::function(cx, target.unit, decl, span) {
                Lowering::Lowered(f) => f,
                other => return Err(format!("`{signal}`'s migration `{step}`: {other}")),
            };
            let program = lower::program_of(cx, vec![function]);
            steps.push(super::js_pure::wire_to_wire(
                &program.functions[0],
                &program,
            )?);
        }
        // The signal's name as a JSON string, a key no name can break.
        source.push_str(&format!(
            "  {}: [\n{}\n  ],\n",
            serde_json::Value::String(signal),
            steps
                .iter()
                .map(|s| format!("    {s}"))
                .collect::<Vec<_>>()
                .join(",\n")
        ));
    }
    source.push_str("};\n");
    Ok(source)
}

/// **One page's module**: each function the compiler lifted, as `js_pure`
/// writes one, and each part's, taking the signal's value in its wire form.
fn module(
    cx: &Context<'_>,
    page: &str,
    computed: &[&Live],
    derived: &[Derived],
) -> Result<String, String> {
    let mut functions = Vec::new();
    for live in computed {
        let d = derived
            .iter()
            .find(|d| d.component_id == live.derived)
            .ok_or_else(|| format!("part {} of `{page}` lifted no function", live.part))?;
        match lower::hole(cx, d) {
            Lowering::Lowered(f) => functions.push(f),
            other => return Err(format!("part {} of `{page}`: {other}", live.part)),
        }
    }
    let program = lower::program_of(cx, functions);
    emit(
        page,
        computed,
        &program.functions.iter().collect::<Vec<_>>(),
        &program,
    )
}

fn emit(
    page: &str,
    computed: &[&Live],
    functions: &[&super::ir::Function],
    program: &super::ir::Program,
) -> Result<String, String> {
    let mut source = format!(
        "// Generated by pw: what `{page}` computes from its signals (ADR-0227). Do not edit.\n\n"
    );
    for (n, f) in functions.iter().enumerate() {
        let js = super::js_pure::isolated(f, program)?;
        source.push_str(&format!("const f{n} = {js};\n\n"));
    }
    source.push_str("export const parts = {\n");
    for (n, live) in computed.iter().enumerate() {
        let f = functions[n];
        if f.params.is_empty() {
            return Err(format!("part {} of `{page}` takes nothing", live.part));
        }
        // Each value it reads, decoded from its wire form, in order: one, or
        // several the browser holds (track `device-store`, decision 7).
        let mut takes = Vec::new();
        let mut decoded = Vec::new();
        for (i, (_, input)) in f.params.iter().enumerate() {
            let j = match f.params.len() {
                1 => "j".to_string(),
                _ => format!("j{i}"),
            };
            decoded.push(super::js_pure::decoder(program, input, &j)?);
            takes.push(j);
        }
        source.push_str(&format!(
            "  \"{}\": ({}) => f{n}({}),\n",
            live.part,
            takes.join(", "),
            decoded.join(", ")
        ));
    }
    source.push_str("};\n");
    Ok(source)
}
