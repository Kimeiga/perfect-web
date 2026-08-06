//! Charter §8.5, §7.8 — what a resumable handler may capture.
//!
//! A `resumable(captures = { .. })` handler does not close over the browser's
//! memory; its captures are **serialized into the resume manifest**, which
//! ships with the document. Two things follow, and the corpus writes one
//! fixture for each:
//!
//! - **R-010** captures a live `DatabaseConnection`. Serializing it is not
//!   merely lossy, it is impossible — the value *is* the open socket.
//! - **R-030** captures a `Cart`, which only a `session query` produces.
//!   Anything in the manifest inherits the document's cacheability, so a
//!   session-scoped value in it is served to whoever the shell is served to.
//!
//! Both are read from declarations. A type is unserializable because some
//! function declares `resource.acquire<T>` for it, and a type is
//! session-scoped because the query producing it is declared `session`. There
//! is no list of unserializable types in the compiler, and adding a resource
//! to a library brings both consequences with it.

use std::collections::BTreeMap;

use crate::codes;
use crate::diagnostics::{Detector, Diagnostic, Related, Repair, Severity};
use crate::hir::{Decl, Expr, ExprId, Hir, Span};
use crate::privacy::Restriction;
use crate::signatures::Signatures;

/// What the whole program says about a type, gathered once.
#[derive(Default)]
pub struct Manifest {
    /// Types some function acquires as a resource.
    resources: BTreeMap<String, String>,
    /// Types produced only by a scoped declaration, with the scope.
    scoped: BTreeMap<String, Restriction>,
}

impl Manifest {
    pub fn build(hirs: &[&Hir], sigs: &Signatures) -> Manifest {
        let mut m = Manifest::default();

        for (path, sig) in sigs.iter() {
            for e in &sig.effects {
                let Some(ty) = e
                    .strip_prefix("resource.acquire<")
                    .and_then(|r| r.strip_suffix('>'))
                else {
                    continue;
                };
                m.resources.insert(ty.to_string(), path.clone());
            }
        }

        for hir in hirs {
            for (_, d) in hir.all_decls() {
                let Some(vis) = d.visibility.as_deref() else {
                    continue;
                };
                let restriction = match vis {
                    "session" => Restriction::Session("SessionId".into()),
                    "user" => Restriction::User("UserId".into()),
                    "organization" => Restriction::Organization("OrganizationId".into()),
                    _ => continue,
                };
                // `session query Cart(..) -> Result<Cart, CartError>` produces a
                // `Cart`. The head is the carrier, so the produced type is its
                // first argument.
                let produced = match d.ret.as_deref() {
                    Some("Result") | Some("Option") | Some("List") => d.ret_args.first().cloned(),
                    other => other.map(str::to_string),
                };
                if let Some(ty) = produced {
                    m.scoped.insert(ty, restriction);
                }
            }
        }
        m
    }
}

pub fn check(hir: &Hir, manifest: &Manifest, out: &mut Vec<Diagnostic>) {
    for (id, decl) in hir.all_decls() {
        let Some(body_id) = decl.body else { continue };
        let body = hir.body(body_id);
        let at = hir.decl_span(id);

        // Each parameter's written type. A capture names a binding, and the
        // only thing that says what a binding IS here is its annotation.
        let types: BTreeMap<&str, &str> = decl
            .params
            .iter()
            .filter_map(|p| Some((p.name.as_str(), p.ty.as_deref()?)))
            .collect();

        for lambda in body.walk() {
            let Expr::Lambda {
                descriptor: Some(d),
                ..
            } = body.expr(lambda)
            else {
                continue;
            };
            for (name, span) in captures(body, *d) {
                let Some(ty) = types.get(name.as_str()) else {
                    continue;
                };
                if let Some(producer) = manifest.resources.get(*ty) {
                    out.push(unserializable(decl, &name, ty, producer, span, &at));
                } else if let Some(r) = manifest.scoped.get(*ty) {
                    out.push(private_value(decl, &name, r, span, &at));
                }
            }
        }
    }
}

/// The names inside `resumable(captures = { a, b })`.
fn captures(body: &crate::hir::Body, descriptor: ExprId) -> Vec<(String, Span)> {
    let Expr::Call { callee, args } = body.expr(descriptor) else {
        return Vec::new();
    };
    if !matches!(body.expr(*callee), Expr::Name(n) if n == "resumable") {
        return Vec::new();
    }
    args.iter()
        .filter(|a| a.name.as_deref() == Some("captures"))
        .flat_map(|a| body.walk_from(a.value))
        .filter_map(|e| match body.expr(e) {
            Expr::Name(n) => Some((n.clone(), body.expr_span(e))),
            _ => None,
        })
        .collect()
}

fn unserializable(
    decl: &Decl,
    name: &str,
    ty: &str,
    producer: &str,
    span: Span,
    at: &Span,
) -> Diagnostic {
    Diagnostic {
        code: codes::UNSERIALIZABLE_CAPTURE.id,
        invariant: codes::UNSERIALIZABLE_CAPTURE.invariant,
        reason: "resumable_handler_captures_a_resource",
        detector: Detector::PatternMatrix,
        severity: Severity::Error,
        message: format!("handler captures `{name}: {ty}`, which is not serializable"),
        primary_span: span,
        related: vec![Related {
            span: at.clone(),
            label: format!("`{}` declares the handler", decl.name),
        }],
        explanation: Some(format!(
            "A resumable handler may capture only serializable immutable values and \
             resource keys. `{ty}` is a resource — `{producer}` declares \
             `resource.acquire<{ty}>` — so the value IS the thing held open, not a \
             description of it. Serializing it cannot fail informatively at runtime; \
             it produces a manifest that is simply wrong."
        )),
        repairs: vec![Repair {
            description: format!(
                "capture the key that identifies what `{name}` points at, and acquire \
                 the resource again on the other side"
            ),
            replacement: None,
        }],
    }
}

fn private_value(decl: &Decl, name: &str, r: &Restriction, span: Span, at: &Span) -> Diagnostic {
    let written = match r {
        Restriction::Session(s) => format!("Session<{s}>"),
        Restriction::User(u) => format!("User<{u}>"),
        Restriction::Organization(o) => format!("Organization<{o}>"),
        Restriction::Secret(c) => format!("Secret<{c}>"),
        Restriction::Device => "Device".to_string(),
    };
    Diagnostic {
        code: codes::PRIVATE_IN_RESUME_MANIFEST.id,
        invariant: codes::PRIVATE_IN_RESUME_MANIFEST.invariant,
        reason: "resumable_handler_captures_a_private_value",
        detector: Detector::PatternMatrix,
        severity: Severity::Error,
        message: format!(
            "resume manifest for `{}` would contain a `{written}` value",
            decl.name
        ),
        primary_span: span,
        related: vec![Related {
            span: at.clone(),
            label: format!("`{}` declares the handler", decl.name),
        }],
        explanation: Some(format!(
            "The resume manifest is served with the public shell and may contain only \
             `Public` values. `{name}` is `{written}`, and anything in the manifest \
             inherits the document's cacheability — so this is not a question of who \
             reads the page, it is a question of what a cache is allowed to keep."
        )),
        repairs: vec![Repair {
            description: format!(
                "capture an identifier and re-read `{name}` on the server when the \
                 handler runs"
            ),
            replacement: None,
        }],
    }
}
