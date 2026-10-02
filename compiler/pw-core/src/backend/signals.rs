//! **A page's signals' first values** (ADR-0130).
//!
//! The server renders a page with each signal at its first value, and the
//! document hands the browser that value to hold from then on. Both read it
//! in the form the browser's compiled modules read a value (`js_pure`'s wire
//! form), computed here from the declaration: the expression is lowered as a
//! function of nothing, of the type the signal declares, and encoded if it
//! is data. A first value that computes is refused, by name: the build writes
//! data, and running a program at build time is not this slice.

use crate::hir::{DeclKind, Expr, Hir};
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
            let Some(b) = decl.body else { continue };
            let body = hir.body(b);
            let declared = crate::page_values::signals_of(body);
            if declared.is_empty() {
                continue;
            }
            let page = crate::contract::component_id(hir, id);
            if decl.kind != DeclKind::Page {
                out.push(Compiled {
                    page,
                    signals: Err(format!(
                        "a signal in a {:?}: this slice holds a page's signals, and a view's \
                         wait for views to compose (ADR-0130)",
                        decl.kind
                    )),
                });
                continue;
            }
            let module = hir.module_of(id);
            let mut signals = Vec::new();
            let mut refused = None;
            for (name, init, let_id) in declared {
                let span = body.expr_span(init);
                let written = match body.expr(let_id) {
                    Expr::Let { ty: Some(t), .. } => crate::resolved::written_in_body(body, *t),
                    _ => None,
                };
                let Some(ty) = written.and_then(|w| {
                    sigs.resolve_type(module, decl, &w, span.clone())
                        .resolved()
                        .cloned()
                }) else {
                    refused = Some(format!("`{name}`'s type resolves to nothing"));
                    break;
                };
                let ty = match lower::backend_type(&cx, &ty, &span) {
                    Lowering::Lowered(t) => t,
                    other => {
                        refused = Some(format!("`{name}`'s type: {}", describe(&other)));
                        break;
                    }
                };
                let export = format!("{page}.signal.{name}");
                let function =
                    match lower::pure_expr_as(&cx, unit, id, init, &[], Some(&ty), &export, span) {
                        Lowering::Lowered(f) => f,
                        other => {
                            refused = Some(format!("`{name}`'s first value: {}", describe(&other)));
                            break;
                        }
                    };
                let program = lower::program_of(&cx, vec![function]);
                match super::js_pure::constant(&program, &program.functions[0]) {
                    Ok(initial) => signals.push(Signal { name, initial }),
                    Err(why) => {
                        refused = Some(format!("`{name}`'s first value: {why}"));
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

fn describe<T>(l: &Lowering<T>) -> String {
    match l {
        Lowering::Lowered(_) => "lowered".to_string(),
        Lowering::Unsupported {
            construct, reason, ..
        } => format!("{construct}: {reason}"),
        Lowering::Blocked { why, .. } => why.clone(),
    }
}
