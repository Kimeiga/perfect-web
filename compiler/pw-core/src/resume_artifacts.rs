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
    src: &str,
    body: &crate::hir::Body,
    types: &Types<'_>,
    lambda: ExprId,
    descriptor: ExprId,
    document_schema: &str,
    build: &str,
) -> Option<ResumeManifest> {
    // Resumable, not "captures something". A handler that captures nothing is
    // still a handler with an identity, an ABI, a build and a document schema;
    // its capture schema is simply the schema of nothing.
    if !crate::resume::is_resumable(body, descriptor) {
        return None;
    }
    let captures = crate::resume::capture_names_and_types(body, types, descriptor);
    let capture_schema = schema_of(&captures);
    Some(ResumeManifest {
        handler: handler_id(src, body, types, lambda, &capture_schema),
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
    src: &str,
    body: &crate::hir::Body,
    types: &Types<'_>,
    lambda: ExprId,
    descriptor: ExprId,
    document_schema: &str,
    build: &str,
) -> Option<HandlerArtifact> {
    if !crate::resume::is_resumable(body, descriptor) {
        return None;
    }
    let declared = crate::resume::capture_names_and_types(body, types, descriptor);
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
        handler: handler_id(src, body, types, lambda, &schema_of(&declared)),
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

/// Which algorithm produced an implementation hash.
///
/// ```text
/// 1  normalized source text        SUPERSEDED
/// 2  semantic tokens + resolved reference identities
/// 3  canonical typed IR            not implemented
/// ```
///
/// Carried in the manifest, because a digest cannot say what made it and
/// manifests outlive deployments.
pub const HASH_SCHEME: u16 = 2;

/// The handler's implementation identity — scheme 2.
///
/// # What participates
///
/// Token kinds, identifier spellings, operators, literal values, control-flow
/// syntax, and statement order. Plus the **resolved identity** of every
/// reference, because `foo()` can mean a different declaration after an import
/// change with identical tokens — E2B's module graph is what makes that
/// answerable.
///
/// # What does not
///
/// Whitespace, indentation, comments, and source offsets. Scheme 1 hashed
/// source text, so a formatter run invalidated every open tab on every deploy.
/// Spans were the other candidate and are worse: one comment near the top of a
/// file shifts every span below it while changing no behaviour.
///
/// # What still invalidates, deliberately
///
/// A local rename, an `if` rewritten as an equivalent `match`, two reordered
/// independent pure expressions. Those are false rejections and they are
/// tolerable. Proving them equivalent is an optimizer, and an optimizer inside
/// an identity system is a worse hazard than an occasional reload — scheme 3 is
/// where that becomes safe.
fn implementation_hash(src: &str, span: &Span, resolved: &[String]) -> String {
    let tokens = pw_syntax::lexer::lex(src);
    let mut stream = String::new();
    for t in tokens {
        // Offsets never participate; only whether a token is inside the body.
        if t.span.start < span.start || t.span.end > span.end {
            continue;
        }
        if t.kind.is_trivia() {
            continue;
        }
        // Kind AND spelling: `1_000` and `1000` are different literals until a
        // typed IR says otherwise, and two identifiers that differ are two
        // different names.
        stream.push_str(&format!("{:?}:{}\u{2}", t.kind, t.text(src)));
    }
    // Resolved identities, in the order they appear — a reference's MEANING is
    // part of what the body does, and identical tokens can resolve elsewhere.
    hash(&format!(
        "v{HASH_SCHEME}\u{1}{stream}\u{1}{}",
        resolved.join("\u{2}")
    ))
}

/// The handler's content identity, matching the runtime's derivation shape:
/// implementation, dependency set (sorted, deduplicated), capture schema, ABI.
fn handler_id(
    src: &str,
    body: &crate::hir::Body,
    types: &Types<'_>,
    lambda: ExprId,
    capture_schema: &str,
) -> String {
    let Expr::Lambda { body: inner, .. } = body.expr(lambda) else {
        return hash("<not a lambda>");
    };
    let span = body.expr_span(*inner);

    // Every reference, with what it resolved to. `foo()` with identical tokens
    // may name a different declaration after an import change, and the
    // implementation hash must see that.
    let resolved: Vec<String> = body
        .walk_from(*inner)
        .into_iter()
        .filter_map(|e| match body.expr(e) {
            Expr::Call { callee, .. } => {
                let path = crate::infer::path_of(body, *callee);
                let target = types
                    .callee(body, *callee)
                    .map(|s| s.path.clone())
                    .unwrap_or_else(|| "<unresolved>".to_string());
                Some(format!("{path}->{target}"))
            }
            _ => None,
        })
        .collect();

    let implementation = implementation_hash(src, &span, &resolved);

    // The dependency SET: order-insensitive and deduplicated, because the order
    // two references were discovered in is not behavioural. Separate from the
    // implementation hash, which is order-sensitive — one answers what this
    // depends on, the other whether it still means the same executable thing.
    let mut deps: Vec<String> = resolved
        .iter()
        .map(|r| r.split("->").last().unwrap_or(r).to_string())
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
    src: &str,
    hir: &Hir,
    sigs: &Signatures,
    build: &str,
) -> Vec<(ResumeManifest, HandlerArtifact)> {
    located(src, hir, sigs, build)
        .into_iter()
        .map(|(_, _, m, a)| (m, a))
        .collect()
}

/// Every resumable handler, with **where** it is.
///
/// E7-R needs the handler identity on the template IR's `Event` part, and the
/// identity is derived here. Exposing the location rather than re-deriving the
/// id elsewhere keeps one derivation with two readers: an `Event` part whose
/// handler id was computed by a second walk could disagree with the manifest
/// the runtime compares it against, and the disagreement would be silent —
/// `decide` would refuse a handler that is in fact the right one.
pub fn located(
    src: &str,
    hir: &Hir,
    sigs: &Signatures,
    build: &str,
) -> Vec<(crate::hir::DeclId, ExprId, ResumeManifest, HandlerArtifact)> {
    let mut out = Vec::new();
    for (id, decl) in hir.all_decls() {
        let Some(body_id) = decl.body else { continue };
        let body = hir.body(body_id);
        let types = Types::of_body(sigs, decl, body, hir.module_of(id));
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
                manifest_of(src, body, &types, lambda, *d, &document_schema, build),
                artifact_of(src, body, &types, lambda, *d, &document_schema, build),
            ) {
                out.push((id, lambda, m, a));
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
pub fn check(src: &str, hir: &Hir, sigs: &Signatures, build: &str, out: &mut Vec<Diagnostic>) {
    for (id, decl) in hir.all_decls() {
        let Some(body_id) = decl.body else { continue };
        let body = hir.body(body_id);
        let types = Types::of_body(sigs, decl, body, hir.module_of(id));
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
                manifest_of(src, body, &types, lambda, *d, &document_schema, build),
                artifact_of(src, body, &types, lambda, *d, &document_schema, build),
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
        generate(src, &hir, &sigs, BUILD)
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

    /// The identity matrix — the falsifiable contract for scheme 2.
    ///
    /// Without this, "normalized" is a word rather than a specification. Each
    /// row states whether a change to the handler body must alter its
    /// implementation identity.
    #[test]
    fn the_scheme_2_identity_matrix_holds() {
        let base = "module m\n\n\
            type Store = Store { id: Int }\n\n\
            view Panel(store: Store) !{} {\n    \
                <button on:press={resumable(captures = { store }) => refresh(store)}>x</button>\n\
            }\n";
        let id_of = |src: &str| pair(src).0.handler;
        let base_id = id_of(base);

        // MUST NOT change identity.
        for (what, src) in [
            (
                "trailing whitespace",
                base.replace("=> refresh(store)}", "=> refresh(store)   }"),
            ),
            (
                "indentation",
                base.replace("    <button", "        <button"),
            ),
            (
                "a comment elsewhere in the file",
                base.replace(
                    "module m\n",
                    "module m\n// a comment that shifts every span below it\n",
                ),
            ),
            (
                "a comment before the declaration",
                base.replace("view Panel", "// explains the panel\nview Panel"),
            ),
        ] {
            assert_eq!(
                id_of(&src),
                base_id,
                "{what} must not change implementation identity"
            );
        }

        // MUST change identity.
        for (what, src) in [
            (
                "a literal",
                base.replace("=> refresh(store)", "=> refresh(store, 2)"),
            ),
            (
                "operation order",
                base.replace("=> refresh(store)", "=> { notify(store); refresh(store) }"),
            ),
            (
                "the called declaration",
                base.replace("=> refresh(store)", "=> reload(store)"),
            ),
            (
                "a local rename",
                base.replace("store: Store", "s: Store")
                    .replace("captures = { store }", "captures = { s }")
                    .replace("refresh(store)", "refresh(s)"),
            ),
        ] {
            assert_ne!(
                id_of(&src),
                base_id,
                "{what} MUST change implementation identity"
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
