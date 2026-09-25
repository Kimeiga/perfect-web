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
//! # The supported set, and nothing else
//!
//! One call to a command, whose arguments are captured values and their
//! fields, literals, and opaque constructors over a primitive, and whose
//! parameters are all primitives or opaque types over one. Anything else is
//! refused by name, as the Wasm encoder refuses: a handler that compiled to
//! something plausible and wrong would run in a user's browser, with nothing
//! downstream to notice.
//!
//! # What the browser sends is not trusted
//!
//! The module runs in the user's browser, so the server cannot know that the
//! arguments it receives came from this code. The server types them by the
//! command component's own parameters and refuses anything else; it does not
//! re-check what an opaque type's name promises, because the language does not
//! state it (`opaque type PositiveInt = Int` has no invariant to check).

use crate::hir::{Body, Expr, ExprId, Hir, Literal};
use crate::resolve::{UnitId, Workspace};
use crate::resolved::{Primitive, ResolvedType, TypeResolution};
use crate::signatures::Signatures;
use crate::values::{Named, Target};

use super::wasm::Encoding;

macro_rules! refuse {
    ($construct:expr, $($reason:tt)*) => {
        return Encoding::Unsupported { construct: $construct, reason: format!($($reason)*) }
    };
}

/// A JavaScript number holds every integer up to 2^53 exactly. Past it, a
/// handler would send a different number than the one it was written with.
pub const EXACT_INTEGER: u64 = 1 << 53;

/// One compiled handler: the module, and the identity it is served under.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HandlerModule {
    /// The resume manifest's identity for this handler: what the document
    /// names, what `decide` authorises, and the file the runtime fetches.
    pub identity: String,
    /// The behaviour's name, as the template IR's event part carries it.
    pub name: String,
    /// The component id of the command it calls.
    pub command: String,
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
    let mut out = Vec::new();
    for (unit, (hir, src)) in hirs.iter().zip(sources).enumerate() {
        for (decl_id, lambda, manifest, _) in
            crate::resume_artifacts::located(src, hir, sigs, crate::resume_artifacts::BUILD)
        {
            let Some(body_id) = hir.decl(decl_id).body else {
                continue;
            };
            let body = hir.body(body_id);
            let cx = Cx {
                hirs,
                body,
                ws,
                sigs,
                at: unit,
                paths: crate::resume::capture_paths(body, lambda),
            };
            out.push(Compiled {
                declaration: crate::contract::component_id(hir, decl_id),
                module: module(&cx, lambda, &manifest.handler),
            });
        }
    }
    out
}

struct Cx<'a> {
    hirs: &'a [&'a Hir],
    body: &'a Body,
    ws: &'a Workspace,
    sigs: &'a Signatures,
    at: UnitId,
    /// What the handler reads of its captures: what the document carries.
    paths: Vec<String>,
}

fn module(cx: &Cx<'_>, lambda: ExprId, identity: &str) -> Encoding<HandlerModule> {
    let Expr::Lambda {
        params,
        body: inner,
        ..
    } = cx.body.expr(lambda)
    else {
        return Encoding::Blocked {
            why: "a handler identity that is not a lambda".into(),
        };
    };
    if !params.is_empty() {
        refuse!(
            "a handler with parameters",
            "the handler takes {} parameter(s); the event is not passed to a compiled \
             handler yet",
            params.len()
        );
    }
    // Exactly the shape the event part's name is read from, so the module and
    // the part cannot name different behaviours.
    let name = crate::template_ir::called_name(cx.body, lambda);
    let Expr::Call { callee, args } = cx.body.expr(*inner) else {
        refuse!(
            "a handler that is not one call",
            "the handler's body is {}; a handler compiles to one command call",
            describe(cx.body.expr(*inner))
        );
    };
    if name.is_empty() {
        refuse!(
            "a handler whose call is written through a path",
            "the event part names a handler by its bare callee"
        );
    }
    let (command, call) = match command_call(cx, *callee, args) {
        Encoding::Encoded(c) => c,
        Encoding::Unsupported { construct, reason } => {
            return Encoding::Unsupported { construct, reason };
        }
        Encoding::Blocked { why } => return Encoding::Blocked { why },
    };
    let source = format!(
        "// Generated by pw: resumable handler {identity}. Do not edit.\n\
         export const name = {name_json};\n\
         export const handler = {identity_json};\n\
         export async function run(context) {{\n\
         \x20 return {call};\n\
         }}\n",
        name_json = json(&name),
        identity_json = json(identity),
    );
    Encoding::Encoded(HandlerModule {
        identity: identity.to_string(),
        name,
        command,
        source,
    })
}

/// The handler's one call, which must be to a command.
fn command_call(
    cx: &Cx<'_>,
    callee: ExprId,
    args: &[crate::hir::Arg],
) -> Encoding<(String, String)> {
    let sig = match crate::values::named(cx.sigs, cx.ws, cx.at, cx.body, callee) {
        Named::Target(Target::Callable(sig)) => sig,
        _ => refuse!(
            "a handler that does not call a command",
            "`{}` is not a command",
            crate::infer::path_of(cx.body, callee)
        ),
    };
    let Some(decl) = crate::resolve::declaration(cx.hirs, sig.definition) else {
        return Encoding::Blocked {
            why: format!("`{}` resolved to a declaration no unit holds", sig.path),
        };
    };
    if decl.kind != crate::hir::DeclKind::Command {
        refuse!(
            "a handler that does not call a command",
            "`{}` is a {:?}; a handler reaches the server through a command",
            sig.path,
            decl.kind
        );
    }
    if args.len() != sig.params.len() || args.iter().any(|a| a.name.is_some()) {
        return Encoding::Blocked {
            why: format!(
                "`{}` takes {} positional argument(s); the checker reports a call that \
                 does not supply them",
                sig.path,
                sig.params.len()
            ),
        };
    }
    for (i, p) in sig.params.iter().enumerate() {
        if sendable(cx, p.as_ref()).is_none() {
            refuse!(
                "a command parameter a browser cannot send",
                "parameter {} of `{}` is `{}`; a compiled handler sends primitives and \
                 opaque types over them",
                i + 1,
                sig.path,
                p.as_ref()
                    .and_then(TypeResolution::resolved)
                    .map(ResolvedType::to_string)
                    .unwrap_or_else(|| "unannotated".into())
            );
        }
    }
    let mut compiled = Vec::new();
    for a in args {
        match value(cx, a.value) {
            Encoding::Encoded(s) => compiled.push(s),
            Encoding::Unsupported { construct, reason } => {
                return Encoding::Unsupported { construct, reason };
            }
            Encoding::Blocked { why } => return Encoding::Blocked { why },
        }
    }
    let hir = cx.hirs[sig.definition.unit];
    let component = crate::contract::component_id(hir, crate::hir::DeclId(sig.definition.decl));
    let call = format!(
        "await context.command({}, [{}])",
        json(&component),
        compiled.join(", ")
    );
    Encoding::Encoded((component, call))
}

/// The primitive a parameter of this type is sent as, if it is one.
fn sendable(cx: &Cx<'_>, p: Option<&TypeResolution>) -> Option<Primitive> {
    let t = p?.resolved()?;
    let primitive = t.as_primitive().or_else(|| {
        cx.sigs
            .type_decl(t.def_id()?)?
            .representation
            .as_ref()?
            .resolved()?
            .as_primitive()
    })?;
    (primitive != Primitive::Unit).then_some(primitive)
}

/// One argument, as JavaScript.
fn value(cx: &Cx<'_>, e: ExprId) -> Encoding<String> {
    match cx.body.expr(e) {
        Expr::Literal(Literal::Int(n)) => match n.parse::<i64>() {
            Ok(v) if v.unsigned_abs() <= EXACT_INTEGER => Encoding::Encoded(v.to_string()),
            _ => refuse!(
                "an integer a JavaScript number cannot hold exactly",
                "`{n}` is outside ±2^53"
            ),
        },
        Expr::Literal(Literal::Float(f)) => match f.parse::<f64>() {
            Ok(v) if v.is_finite() => Encoding::Encoded(format!("{v:?}")),
            _ => refuse!("a float literal", "`{f}` is not a finite number"),
        },
        // The literal's VALUE, JSON-encoded, never its token: the token keeps
        // its quotes and escapes as written, and what an escape means is not
        // yet decided by the language (`Literal::string_value`).
        Expr::Literal(l @ Literal::Str(s)) => match l.string_value() {
            Some(v) => Encoding::Encoded(json(v)),
            None => refuse!(
                "a string literal whose escapes the language does not define",
                "`{s}` means something only under an escape rule"
            ),
        },
        Expr::Name(n)
            if (n == "true" || n == "false")
                && !cx.paths.iter().any(|p| crate::resume::covers(n, p))
                && matches!(
                    cx.ws.resolve_in(cx.at, crate::resolve::Namespace::Term, n),
                    crate::resolve::Resolution::Unresolved
                ) =>
        {
            Encoding::Encoded(n.clone())
        }
        Expr::Name(_) | Expr::Field { .. } => {
            let path = crate::resume::field_chain(cx.body, e).unwrap_or_default();
            // The document carries exactly `cx.paths`; a read of anything else
            // would find nothing there.
            if !cx.paths.iter().any(|p| crate::resume::covers(p, &path)) {
                refuse!(
                    "a value the handler does not capture",
                    "`{path}` is not something the handler captured"
                );
            }
            let at: String = path.split('.').map(|s| format!("[{}]", json(s))).collect();
            Encoding::Encoded(format!("context.captures{at}"))
        }
        Expr::Call { callee, args } => {
            match crate::values::named(cx.sigs, cx.ws, cx.at, cx.body, *callee) {
                // `PositiveInt(1)` sends `1`: an opaque value crosses a boundary
                // as its representation, which is the ABI's rule for every
                // opaque type, not a guess about this one.
                Named::Target(Target::Opaque(def))
                    if cx
                        .sigs
                        .type_decl(def)
                        .and_then(|t| t.representation.as_ref())
                        .and_then(TypeResolution::resolved)
                        .and_then(ResolvedType::as_primitive)
                        .is_some_and(|p| p != Primitive::Unit) =>
                {
                    match args.as_slice() {
                        [one] if one.name.is_none() => value(cx, one.value),
                        _ => refuse!(
                            "an opaque constructor with other than one argument",
                            "`{}` is built from exactly its representation",
                            crate::infer::path_of(cx.body, *callee)
                        ),
                    }
                }
                _ => refuse!(
                    "a call inside a handler's arguments",
                    "`{}(..)` is computed in the browser; only an opaque constructor \
                     over a primitive is",
                    crate::infer::path_of(cx.body, *callee)
                ),
            }
        }
        other => refuse!(
            "an expression outside the handler backend",
            "{} does not compile to JavaScript yet",
            describe(other)
        ),
    }
}

fn json(s: &str) -> String {
    serde_json::Value::String(s.to_string()).to_string()
}

fn describe(e: &Expr) -> &'static str {
    match e {
        Expr::Binary { .. } => "a binary operator",
        Expr::Unary { .. } => "a unary operator",
        Expr::If { .. } => "a branch",
        Expr::Match { .. } => "a match",
        Expr::For { .. } => "a loop",
        Expr::Lambda { .. } => "a lambda",
        Expr::Block { .. } => "a block",
        Expr::Let { .. } => "a binding",
        Expr::Record { .. } => "a record",
        Expr::List { .. } => "a list",
        Expr::Interpolated { .. } => "an interpolated string",
        Expr::Try { .. } => "a `?` propagation",
        Expr::Cast { .. } => "a cast",
        Expr::Name(_) => "a name",
        Expr::Field { .. } => "a field",
        Expr::Call { .. } => "a call",
        Expr::Literal(_) => "a literal",
        Expr::Keyword { .. } => "a keyword form",
        Expr::Template { .. } => "markup",
        Expr::Error => "an expression that did not parse",
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
