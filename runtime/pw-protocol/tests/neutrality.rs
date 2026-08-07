//! The protocol is owned by neither endpoint.
//!
//! Architect ruling, 2026-08-07:
//!
//! > If `pw-protocol` depends on `pw-render` merely because `PartAddress`
//! > happens to live there, then eventually the browser patch runtime can
//! > accidentally drag renderer implementation code into its dependency
//! > closure. You'd recreate the exact coupling your no-replay structural gate
//! > is intended to prevent.
//!
//! Asserted structurally rather than observed, for the reason E7-R's no-replay
//! gate is: a passing round-trip test says nothing about what a build could
//! reach. This reads the manifests.

use std::fs::read_to_string;
use std::path::Path;

fn manifest(crate_name: &str) -> String {
    let path = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("..")
        .join(crate_name)
        .join("Cargo.toml");
    read_to_string(&path).unwrap_or_else(|e| panic!("{path:?}: {e}"))
}

/// The `[dependencies]` section only — a dev-dependency is a test's business
/// and cannot reach a built artifact.
fn dependencies(crate_name: &str) -> String {
    let text = manifest(crate_name);
    let start = text.find("[dependencies]").unwrap_or(0);
    let rest = &text[start..];
    let end = rest[1..].find("\n[").map(|i| i + 1).unwrap_or(rest.len());
    rest[..end].to_string()
}

#[test]
fn the_protocol_depends_on_identity_and_on_no_implementation() {
    let deps = dependencies("pw-protocol");
    for identity in ["pw-document", "pw-resource"] {
        assert!(
            deps.contains(identity),
            "the protocol speaks in terms of {identity}"
        );
    }
    for implementation in ["pw-render", "pw-materialize", "pw-tasks", "pw-resume"] {
        assert!(
            !deps.contains(implementation),
            "the protocol must not depend on {implementation}: neither endpoint \
             owns the contract, and a dependency here is how a browser runtime \
             acquires a server renderer"
        );
    }
}

#[test]
fn the_document_vocabulary_depends_on_no_renderer() {
    // The crate that exists so `PartAddress` is reachable without the renderer.
    // If this ever fails, extracting it bought nothing.
    let deps = dependencies("pw-document");
    assert!(!deps.contains("pw-render"), "{deps}");
    assert!(!deps.contains("pw-materialize"), "{deps}");
}

#[test]
fn the_renderer_and_the_materializer_do_not_depend_on_each_other() {
    // Storage can change without the renderer changing, and the reverse. They
    // are separate seams and the manifests should say so.
    assert!(!dependencies("pw-render").contains("pw-materialize"));
    assert!(!dependencies("pw-materialize").contains("pw-render"));
}

#[test]
fn this_check_can_fail() {
    // The negative control. Without it the assertions above hold for a
    // `dependencies()` that returns the empty string — which is exactly what a
    // typo in the section name would produce.
    let deps = dependencies("pw-render");
    assert!(
        deps.contains("pw-document"),
        "the reader must actually find dependencies: {deps:?}"
    );
    assert!(
        !dependencies("pw-protocol").is_empty(),
        "and must not be silently empty"
    );
}
