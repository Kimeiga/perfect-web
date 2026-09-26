//! **E10's browser backend, first step: resumable handlers → JavaScript modules.**
//!
//! Charter §14 M10 task 2, in its stated order:
//!
//! > first: generated modern JavaScript modules for handlers and pure
//! > computation; then: Wasm for pure compute-heavy modules; later: Wasm GC.
//!
//! A resumable handler is the page's behaviour:
//!
//! ```text
//! on:press={resumable(captures = { item }) => add_to_cart(item.id, PositiveInt(1))}
//! ```
//!
//! Until 2026-09-25 the development server answered `/handler/<id>.mjs` with a
//! module it wrote itself, and resolved which item a press meant from the
//! pressed element's address. Now the handler's BODY is compiled. It reads what
//! it captured from the document, where the renderer serialized exactly the
//! paths it reads (`resume::capture_paths`), and it calls the command with
//! arguments it computed itself.
//!
//! # Every decision here is read from the stage that owns it
//!
//! ```text
//! which handler this is             resume_artifacts::located, the identity the
//!                                   manifest and `decide` already use
//! its name                          template_ir::called_name, as the event
//!                                   part carries it
//! which declaration a call names    values::named, the rule the checker used
//! a command's component             contract::component_id, the contract's id
//! what it reads of its captures     resume::capture_paths, as the renderer
//!                                   serializes them
//! an opaque value on the wire       its representation (ABI transparency)
//! ```
//!
//! # What compiles
//!
//! Since 2026-09-25 (ADR-0058) a handler's body is lowered through the
//! backend IR, as a query's is, and written by the pure-computation emitter
//! (ADR-0044): bindings, arithmetic, strings, lists, branches, loops and calls
//! to the program's own functions, from what it captured and from literals.
//! A command it calls is awaited through its context, in order, however many
//! it calls. Until then a handler was one command call, whose arguments were
//! captures, fields, literals and opaque constructors, and anything else was
//! refused.
//!
//! Two representations meet at the handler's edges. The document carries a
//! captured value as JSON, where an `Int` is a JavaScript number exact within
//! ±2^53 (ADR-0033 §7); the module computes with a `BigInt` (ADR-0044). So a
//! captured `Int` is read into one, and an `Int` sent to a command is sent as a
//! number, or the handler traps before sending a different one.
//!
//! Refused by name: a handler with parameters (the event is not passed yet), a
//! handler that calls no command, a command called inside a function a list
//! operation runs, and a command parameter that is not a primitive or an
//! opaque type over one.
//!
//! # What the browser sends is not trusted
//!
//! The module runs in the user's browser, so the server cannot know that the
//! arguments it receives came from this code. The server types them by the
//! command component's own parameters and refuses anything else; it does not
//! re-check what an opaque type's name promises, because the language does not
//! state it (`opaque type PositiveInt = Int` has no invariant to check).

use crate::hir::{ExprId, Hir};
use crate::resolve::Workspace;
use crate::signatures::Signatures;

use super::ir::{Instr, Lowering, all_instrs};
use super::wasm::Encoding;

/// A JavaScript number holds every integer up to 2^53 exactly. Past it, a
/// handler would send a different number than the one it computed.
pub const EXACT_INTEGER: u64 = 1 << 53;

/// One compiled handler: the module, and the identity it is served under.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HandlerModule {
    /// The resume manifest's identity for this handler: what the document
    /// names, what `decide` authorises, and the file the runtime fetches.
    pub identity: String,
    /// The behaviour's name, as the template IR's event part carries it.
    pub name: String,
    /// The component id of each command it calls, in the order of its body.
    pub commands: Vec<String>,
    /// The ES module.
    pub source: String,
}

/// What one handler compiled to, or why it did not.
#[derive(Debug, Clone)]
pub struct Compiled {
    /// The declaration the handler is written in, as a component id.
    pub declaration: String,
    pub module: Encoding<HandlerModule>,
}

/// **Every resumable handler in a program, compiled.**
///
/// `sources` are the units' texts: the handler identity is derived from them
/// (resume scheme 2 hashes the body's tokens). Refusals are returned beside
/// the successes. A page with one handler outside the supported set is an
/// ordinary program, and reporting only what compiled would make the backend
/// look finished.
pub fn handlers(
    hirs: &[&Hir],
    sources: &[&str],
    ws: &Workspace,
    sigs: &Signatures,
) -> Vec<Compiled> {
    // A handler reaches the host through a command's component, never
    // itself, so it needs no contract of its own.
    let cx = super::lower::Context {
        hirs,
        ws,
        sigs,
        contracts: &[],
    };
    let mut out = Vec::new();
    for (unit, (hir, src)) in hirs.iter().zip(sources).enumerate() {
        for (decl_id, lambda, manifest, _) in
            crate::resume_artifacts::located(src, hir, sigs, crate::resume_artifacts::BUILD)
        {
            out.push(Compiled {
                declaration: crate::contract::component_id(hir, decl_id),
                module: module(&cx, unit, decl_id, lambda, &manifest.handler),
            });
        }
    }
    out
}

fn module(
    cx: &super::lower::Context<'_>,
    unit: usize,
    decl_id: crate::hir::DeclId,
    lambda: ExprId,
    identity: &str,
) -> Encoding<HandlerModule> {
    let hir = cx.hirs[unit];
    let Some(body_id) = hir.decl(decl_id).body else {
        return Encoding::Blocked {
            why: "a handler in a declaration with no body".into(),
        };
    };
    let body = hir.body(body_id);
    // Exactly the shape the event part's name is read from, so the module and
    // the part cannot name different behaviours.
    let name = crate::template_ir::called_name(body, lambda);
    let span = body.expr_span(lambda);
    let (function, paths) = match super::lower::handler(cx, unit, decl_id, lambda, span) {
        Lowering::Lowered(x) => x,
        Lowering::Unsupported {
            construct, reason, ..
        } => return Encoding::Unsupported { construct, reason },
        Lowering::Blocked { why, .. } => return Encoding::Blocked { why },
    };
    let mut commands: Vec<String> = Vec::new();
    for b in &function.blocks {
        for i in all_instrs(&b.instrs) {
            if let Instr::Command { command, .. } = i
                && !commands.contains(command)
            {
                commands.push(command.clone());
            }
        }
    }
    if commands.is_empty() {
        return Encoding::Unsupported {
            construct: "a handler that calls no command",
            reason: "a handler reaches the server through a command, and this one calls \
                     none, so pressing it would change nothing"
                .to_string(),
        };
    }
    let program = super::lower::handler_program(cx, function);
    match super::js_pure::handler_source(&program.functions[0], &program, identity, &name, &paths) {
        Ok(source) => Encoding::Encoded(HandlerModule {
            identity: identity.to_string(),
            name,
            commands,
            source,
        }),
        Err(reason) => Encoding::Unsupported {
            construct: "a handler outside the JavaScript backend",
            reason,
        },
    }
}

/// **Compile a program's handlers, end to end.**
///
/// Check first, through the same gate the component backend uses
/// (`lower::Checked`): a backend only ever sees a program that checked. The one
/// path the CLI and the tests share.
pub fn compile(units: &[crate::check::Unit]) -> Result<Vec<Compiled>, String> {
    let hirs: Vec<&Hir> = units.iter().map(|u| &u.hir).collect();
    let sources: Vec<&str> = units.iter().map(|u| u.src.as_str()).collect();
    let ws = Workspace::build(&hirs);
    let sigs = Signatures::build(&ws, &hirs);
    let contracts = crate::contract::contracts(&hirs, &sigs, &ws);
    let cx = super::lower::Context {
        hirs: &hirs,
        ws: &ws,
        sigs: &sigs,
        contracts: &contracts,
    };
    super::lower::Checked::of(units, cx).map_err(|ds| {
        format!(
            "the program does not check, so nothing is compiled: {}",
            ds.iter()
                .map(|d| format!("[{}] {}", d.code, d.message))
                .collect::<Vec<_>>()
                .join("; ")
        )
    })?;
    Ok(handlers(&hirs, &sources, &ws, &sigs))
}
