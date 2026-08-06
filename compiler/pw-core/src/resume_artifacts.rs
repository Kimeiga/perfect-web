//! E7 generator — the resume manifest and the handler artifact, generated
//! **independently**, and then compared.
//!
//! Architect ruling, 2026-08-06:
//!
//! > Do not scaffold it against a fake generator. A stub would risk producing a
//! > tautological test: stub generates A, stub reports that it generated A,
//! > comparison says A == A.
//!
//! So the two records are derived by different walks over different parts of
//! the declaration, and the comparison is only meaningful because of that:
//!
//! ```text
//! the MANIFEST      is what the document carries, derived from the resumable
//!                   ATTRIBUTE — the `captures = { .. }` list as written at the
//!                   binding site, and the markup node it is attached to
//!
//! the ARTIFACT      is what the compiled handler exports, derived from the
//!                   LAMBDA BODY — the parameters it will actually read, and
//!                   the declarations it resolves to
//! ```
//!
//! Two sources for one fact is normally a smell. Here it is the point: a
//! generator bug that changes one walk and not the other is exactly the class
//! of defect this comparison exists to catch, and a single source of truth
//! would make it undetectable by construction.
//!
//! # What this is not
//!
//! The runtime's job. `runtime/pw-resume` decides whether artifacts from
//! DIFFERENT builds are compatible. This decides whether artifacts from THE
//! SAME build agree with each other, which is a build error and is reported as
//! one.

use crate::codes;
use crate::diagnostics::{Detector, Diagnostic, Related, Repair, Severity};
use crate::hir::{Decl, Expr, ExprId, Hir, Node, Span};
use crate::infer::Types;
use crate::signatures::Signatures;

/// A stable content hash. The same FNV-1a the runtime uses, so a value
/// generated here and a value compared there are the same kind of thing.
fn hash(content: &str) -> String {
    let mut h: u64 = 0xcbf2_9ce4_8422_2325;
    for b in content.as_bytes() {
        h ^= *b as u64;
        h = h.wrapping_mul(0x0000_0100_0000_01b3);
    }
    format!("{h:016x}")
}

/// What the document carries.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ResumeManifest {
    pub handler: String,
    pub capture_schema: String,
    pub document_schema: String,
    pub platform_abi: u32,
    pub build: String,
    pub span: Span,
}

/// What the compiled handler exports.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HandlerArtifact {
    pub handler: String,
    pub accepted_capture_schema: String,
    pub expected_document_schema: String,
    pub required_platform_abi: u32,
    pub build: String,
}

/// The ABI this compiler generates for.
pub const PLATFORM_ABI: u32 = 1;

/// The build identity. One value per compilation, and both records take it
/// from here — a build cannot disagree with itself about which build it is,
/// so the mutation controls inject a different one rather than expecting the
/// generator to produce one.
pub const BUILD: &str = "dev";

/// Derive the manifest from the **attribute**: the capture list as written, and
/// the markup node it is attached to.
fn manifest_of(
    body: &crate::hir::Body,
    types: &Types<'_>,
    lambda: ExprId,
    descriptor: ExprId,
    document_schema: &str,
    build: &str,
) -> Option<ResumeManifest> {
    let captures = crate::resume::capture_names_and_types(body, types, descriptor);
    if captures.is_empty() {
        return None;
    }
    let capture_schema = schema_of(&captures);
    Some(ResumeManifest {
        handler: handler_id(body, lambda, &capture_schema),
        capture_schema,
        document_schema: document_schema.to_string(),
        platform_abi: PLATFORM_ABI,
        build: build.to_string(),
        span: body.expr_span(descriptor),
    })
}

/// Derive the artifact from the **lambda body**: what the handler will read.
///
/// Deliberately not the same walk. It recomputes the capture schema from the
/// names the body actually mentions, so a capture the attribute declares and
/// the body never reads — or the reverse — makes the two disagree.
fn artifact_of(
    body: &crate::hir::Body,
    types: &Types<'_>,
    lambda: ExprId,
    descriptor: ExprId,
    document_schema: &str,
    build: &str,
) -> Option<HandlerArtifact> {
    let declared = crate::resume::capture_names_and_types(body, types, descriptor);
    if declared.is_empty() {
        return None;
    }
    let Expr::Lambda { body: inner, .. } = body.expr(lambda) else {
        return None;
    };
    // The captures this body reads, in the order the attribute declared them —
    // order comes from the declaration because a schema is a shape, not a
    // usage trace.
    let mentioned: Vec<String> = body
        .walk_from(*inner)
        .into_iter()
        .filter_map(|e| match body.expr(e) {
            Expr::Name(n) => Some(n.clone()),
            Expr::Field { .. } => Some(crate::infer::path_of(body, e)),
            _ => None,
        })
        .collect();
    let accepted: Vec<(String, Option<String>)> = declared
        .iter()
        .filter(|(name, _)| {
            mentioned
                .iter()
                .any(|m| m == name || m.starts_with(&format!("{name}.")))
        })
        .cloned()
        .collect();

    let accepted_capture_schema = schema_of(&accepted);
    Some(HandlerArtifact {
        handler: handler_id(body, lambda, &schema_of(&declared)),
        accepted_capture_schema,
        expected_document_schema: document_schema.to_string(),
        required_platform_abi: PLATFORM_ABI,
        build: build.to_string(),
    })
}

fn schema_of(captures: &[(String, Option<String>)]) -> String {
    let text: Vec<String> = captures
        .iter()
        .map(|(n, t)| format!("{n}:{}", t.as_deref().unwrap_or("?")))
        .collect();
    hash(&text.join(","))
}

/// The handler's content identity, matching the runtime's derivation shape:
/// implementation, dependency set (sorted, deduplicated), capture schema, ABI.
fn handler_id(body: &crate::hir::Body, lambda: ExprId, capture_schema: &str) -> String {
    let Expr::Lambda { body: inner, .. } = body.expr(lambda) else {
        return hash("<not a lambda>");
    };
    let span = body.expr_span(*inner);
    let implementation = hash(&format!("{}..{}", span.start, span.end));

    let mut deps: Vec<String> = body
        .walk_from(*inner)
        .into_iter()
        .filter_map(|e| match body.expr(e) {
            Expr::Call { callee, .. } => Some(crate::infer::path_of(body, *callee)),
            _ => None,
        })
        .filter(|p| !p.is_empty())
        .collect();
    deps.sort_unstable();
    deps.dedup();

    hash(&format!(
        "{implementation}\u{1}{}\u{1}{capture_schema}\u{1}{PLATFORM_ABI}",
        deps.join("\u{2}")
    ))
}

/// Generate both records for one declaration, for the mutation controls.
///
/// Exposed so a test can produce a VALID pair, mutate exactly one field of one
/// artifact, and assert the comparison rejects it. An ordinary successful
/// generation proves nothing — both records would agree even if the comparison
/// were `|_, _| None`.
pub fn generate(
    hir: &Hir,
    sigs: &Signatures,
    build: &str,
) -> Vec<(ResumeManifest, HandlerArtifact)> {
    let mut out = Vec::new();
    for (_, decl) in hir.all_decls() {
        let Some(body_id) = decl.body else { continue };
        let body = hir.body(body_id);
        let types = Types::of_body(sigs, decl, body);
        let document_schema = document_schema_of(body);
        for lambda in body.walk() {
            let Expr::Lambda {
                descriptor: Some(d),
                ..
            } = body.expr(lambda)
            else {
                continue;
            };
            if let (Some(m), Some(a)) = (
                manifest_of(body, &types, lambda, *d, &document_schema, build),
                artifact_of(body, &types, lambda, *d, &document_schema, build),
            ) {
                out.push((m, a));
            }
        }
    }
    out
}

/// The comparison, exposed for the mutation controls.
pub fn disagreement(m: &ResumeManifest, a: &HandlerArtifact) -> Option<&'static str> {
    compare(m, a)
}

/// Generate both records for every resumable handler in a declaration, and
/// report any disagreement.
pub fn check(hir: &Hir, sigs: &Signatures, build: &str, out: &mut Vec<Diagnostic>) {
    for (id, decl) in hir.all_decls() {
        let Some(body_id) = decl.body else { continue };
        let body = hir.body(body_id);
        let types = Types::of_body(sigs, decl, body);
        let at = hir.decl_span(id);
        let document_schema = document_schema_of(body);

        for lambda in body.walk() {
            let Expr::Lambda {
                descriptor: Some(d),
                ..
            } = body.expr(lambda)
            else {
                continue;
            };
            let (Some(manifest), Some(artifact)) = (
                manifest_of(body, &types, lambda, *d, &document_schema, build),
                artifact_of(body, &types, lambda, *d, &document_schema, build),
            ) else {
                continue;
            };
            if let Some(disagreement) = compare(&manifest, &artifact) {
                out.push(diagnostic(decl, &manifest, disagreement, &at));
            }
        }
    }
}

/// The shape of the markup this body renders.
fn document_schema_of(body: &crate::hir::Body) -> String {
    let mut tags: Vec<String> = Vec::new();
    for id in body.walk() {
        let Expr::Template { roots, .. } = body.expr(id) else {
            continue;
        };
        for n in body.walk_markup(roots) {
            if let Node::Element { tag, .. } = body.node(n) {
                tags.push(tag.clone());
            }
        }
    }
    hash(&tags.join(">"))
}

/// What the two records disagree about, if anything.
///
/// Named so the diagnostic can say which field, because "the artifacts
/// disagree" sends a reader to read both by hand.
fn compare(m: &ResumeManifest, a: &HandlerArtifact) -> Option<&'static str> {
    if m.handler != a.handler {
        return Some("handler identity");
    }
    if m.capture_schema != a.accepted_capture_schema {
        return Some("capture schema");
    }
    if m.document_schema != a.expected_document_schema {
        return Some("document-part schema");
    }
    if m.platform_abi != a.required_platform_abi {
        return Some("platform ABI");
    }
    if m.build != a.build {
        return Some("build identity");
    }
    None
}

fn diagnostic(decl: &Decl, m: &ResumeManifest, field: &'static str, at: &Span) -> Diagnostic {
    Diagnostic {
        code: codes::RESUME_ARTIFACT_CONTRACT_MISMATCH.id,
        invariant: codes::RESUME_ARTIFACT_CONTRACT_MISMATCH.invariant,
        reason: "manifest_and_handler_artifact_disagree",
        detector: Detector::PatternMatrix,
        severity: Severity::Error,
        message: format!("the resume manifest and the handler artifact disagree about {field}"),
        primary_span: m.span.clone(),
        related: vec![Related {
            span: at.clone(),
            label: format!("`{}` generates both", decl.name),
        }],
        explanation: Some(format!(
            "Within one build these are derived from the same source by two \
             different walks — the manifest from the `captures` list as written, \
             the artifact from what the handler body reads — so a disagreement \
             about {field} means one of them describes something the other does \
             not. Shipped, it would make every resume of this handler fail after \
             deployment, for a reason unreadable from the deployment. The most \
             common cause is a capture the attribute declares and the body never \
             reads."
        )),
        repairs: vec![Repair {
            description: "capture what the handler reads, and read what it captures".to_string(),
            replacement: None,
        }],
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::lower::lower_file;
    use crate::resolve::Workspace;
    use pw_syntax::parse_tree;

    /// A view with a resumable handler that reads exactly what it captures.
    const VALID: &str = "module m\n\n\
        type Store = Store { id: Int }\n\n\
        view Panel(store: Store) !{} {\n    \
            <button on:press={resumable(captures = { store }) => refresh(store)}>x</button>\n\
        }\n";

    fn pair(src: &str) -> (ResumeManifest, HandlerArtifact) {
        let hir = lower_file(src, &parse_tree(src).green);
        let refs = vec![&hir];
        let sigs = Signatures::build(&Workspace::build(&refs), &refs);
        generate(&hir, &sigs, BUILD)
            .into_iter()
            .next()
            .expect("one resumable handler")
    }

    #[test]
    fn a_valid_pair_agrees() {
        let (m, a) = pair(VALID);
        assert_eq!(disagreement(&m, &a), None, "manifest {m:?} artifact {a:?}");
    }

    /// The test with teeth. Six mutations, each of exactly one field of one
    /// artifact, each of which must be caught and named.
    ///
    /// Without these, `a_valid_pair_agrees` passes even if `compare` returns
    /// `None` unconditionally — which is the tautology the architect warned a
    /// stub generator would produce.
    #[test]
    fn mutating_one_artifact_is_always_caught() {
        for (what, mutate) in [
            (
                "capture schema",
                Box::new(|m: &mut ResumeManifest, _: &mut HandlerArtifact| {
                    m.capture_schema = hash("something else");
                }) as Box<dyn Fn(&mut ResumeManifest, &mut HandlerArtifact)>,
            ),
            (
                "capture schema",
                Box::new(|_: &mut ResumeManifest, a: &mut HandlerArtifact| {
                    a.accepted_capture_schema = hash("something else");
                }),
            ),
            (
                "document-part schema",
                Box::new(|_: &mut ResumeManifest, a: &mut HandlerArtifact| {
                    a.expected_document_schema = hash("a different tree");
                }),
            ),
            (
                "handler identity",
                Box::new(|m: &mut ResumeManifest, _: &mut HandlerArtifact| {
                    m.handler = hash("a different handler");
                }),
            ),
            (
                "platform ABI",
                Box::new(|_: &mut ResumeManifest, a: &mut HandlerArtifact| {
                    a.required_platform_abi = PLATFORM_ABI + 1;
                }),
            ),
            (
                "build identity",
                Box::new(|_: &mut ResumeManifest, a: &mut HandlerArtifact| {
                    a.build = "other-build".into();
                }),
            ),
        ] {
            let (mut m, mut a) = pair(VALID);
            mutate(&mut m, &mut a);
            assert_eq!(
                disagreement(&m, &a),
                Some(what),
                "mutating {what} was not caught, or was reported as something else"
            );
        }
    }

    /// The generator's two walks really are different.
    ///
    /// A capture the attribute declares and the body never reads makes them
    /// disagree — which is the whole reason the artifact is derived from the
    /// body rather than copied from the manifest.
    #[test]
    fn a_declared_but_unread_capture_makes_the_walks_disagree() {
        let src = "module m\n\n\
            type Store = Store { id: Int }\n\n\
            view Panel(store: Store, other: Store) !{} {\n    \
                <button on:press={resumable(captures = { store, other }) => refresh(store)}>x</button>\n\
            }\n";
        let (m, a) = pair(src);
        assert_eq!(
            disagreement(&m, &a),
            Some("capture schema"),
            "`other` is captured and never read; the walks must notice"
        );
    }
}
