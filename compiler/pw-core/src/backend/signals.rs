//! **A page's signals' first values** (ADR-0130).
//!
//! The server renders a page with each signal at its first value, and the
//! document hands the browser that value to hold from then on. Both read it
//! in the form the browser's compiled modules read a value (`js_pure`'s wire
//! form), computed here from the declaration: the expression is lowered as a
//! function of nothing, of the type the signal declares, and encoded if it
//! is data. A first value that computes is refused, by name: the build writes
//! data, and running a program at build time is not this slice.

use crate::hir::{DeclKind, Hir};
use crate::page_values::Signal;
use crate::resolve::Workspace;
use crate::signatures::Signatures;

use super::ir::Lowering;
use super::lower::{self, Context};

/// One page's signals, or why they did not compile.
#[derive(Debug, Clone)]
pub struct Compiled {
    /// The page's template path: `store.page.StorePage`.
    pub page: String,
    pub signals: Result<Vec<Signal>, String>,
}

/// **Every page's signals.** A page with none is not listed.
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
            // Its own signals, and each one the views composed into it hold
            // or are provided, at each use (ADR-0144): what its template
            // holds, as the template names them.
            let Some(lowered) = crate::template_ir::lowered(
                &hirs,
                &ws,
                &sigs,
                &crate::template_ir::Handlers::new(),
                unit,
                id,
            ) else {
                continue;
            };
            if lowered.instances.is_empty() {
                continue;
            }
            let page = crate::contract::component_id(hir, id);
            let mut signals = Vec::new();
            let mut refused = None;
            for instance in lowered.instances {
                match first_value(&cx, &sigs, &page, &instance) {
                    Ok(initial) => signals.push(Signal {
                        name: instance.name,
                        initial,
                    }),
                    Err(why) => {
                        refused = Some(why);
                        break;
                    }
                }
            }
            out.push(Compiled {
                page,
                signals: match refused {
                    Some(why) => Err(why),
                    None => Ok(signals),
                },
            });
        }
    }
    Ok(out)
}

/// **One instance's first value**, as the browser's compiled modules read a
/// value: its expression lowered as a function of nothing, of the type the
/// signal declares, and encoded.
fn first_value(
    cx: &Context<'_>,
    sigs: &Signatures,
    page: &str,
    instance: &crate::template_ir::Instance,
) -> Result<serde_json::Value, String> {
    use crate::template_ir::InstanceType;
    let (unit, decl_id) = instance.origin;
    let hir = cx.hirs[unit];
    let decl = hir.decl(decl_id);
    let name = &instance.name;
    let body = hir.body(
        decl.body
            .ok_or_else(|| format!("`{name}` is held by no body"))?,
    );
    let span = body.expr_span(instance.init);
    // The type as written: the `let`'s, or the module signal's declaration's.
    let resolved = match instance.ty {
        InstanceType::Written(t) => crate::resolved::written_in_body(body, t).and_then(|w| {
            sigs.resolve_type(hir.module_of(decl_id), decl, &w, span.clone())
                .resolved()
                .cloned()
        }),
        InstanceType::Declared(signal) => {
            sigs.signal_type(signal).and_then(|t| t.resolved()).cloned()
        }
    };
    let ty = resolved.ok_or_else(|| format!("`{name}`'s type resolves to nothing"))?;
    let ty = match lower::backend_type(cx, &ty, &span) {
        Lowering::Lowered(t) => t,
        other => return Err(format!("`{name}`'s type: {}", describe(&other))),
    };
    let export = format!("{page}.signal.{name}");
    let function = match lower::pure_expr_as(
        cx,
        unit,
        decl_id,
        instance.init,
        &[],
        Some(&ty),
        &export,
        span,
    ) {
        Lowering::Lowered(f) => f,
        other => return Err(format!("`{name}`'s first value: {}", describe(&other))),
    };
    let program = lower::program_of(cx, vec![function]);
    let first = super::js_pure::constant(&program, &program.functions[0])
        .map_err(|why| format!("`{name}`'s first value: {why}"))?;
    // As the browser's wire carries it: a value of a type that contains
    // itself as its nodes (ADR-0205).
    super::js_pure::wire_json(&program, &ty, first)
        .map_err(|why| format!("`{name}`'s first value: {why}"))
}

fn describe<T>(l: &Lowering<T>) -> String {
    match l {
        Lowering::Lowered(_) => "lowered".to_string(),
        Lowering::Unsupported {
            construct, reason, ..
        } => format!("{construct}: {reason}"),
        Lowering::Blocked { why, .. } => why.clone(),
    }
}
