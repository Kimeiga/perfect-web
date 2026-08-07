//! RQ-5 — capability compliance is checked against the **final artifact**.
//!
//! ADR-0011 §7, from the architect ruling:
//!
//! > Capability compliance is checked against the final component artifact, not
//! > against source declarations or the nominal WIT world alone.
//!
//! E0 measured why. A `std` Rust guest whose WIT world declares **one** import
//! compiles to a component that requests **fifteen**:
//! `wasi:cli/environment`, `exit`, stdio, five terminal interfaces,
//! `clocks/monotonic-clock`, and `wasi:io/{error,poll,streams}`. None of that
//! appears in the source or the world. It comes from the language runtime.
//!
//! One terminology correction, which the architect made and which matters: those
//! imports are a **requested authority surface**, not authority already held.
//! The host still decides what to link, and an unsatisfied import means the
//! component cannot instantiate at all — the E0 `denied` check proved that. The
//! real defect is narrower: *the declared world is not evidence of what the
//! artifact requests.*
//!
//! This module is deliberately free of any Wasm dependency so the rule can be
//! unit-tested without a runtime. The Wasmtime host supplies the actual list;
//! `spikes/wasmtime-component/host` already computes it via
//! `Component::component_type().imports()`.

/// What a component is permitted to import, and under which runtime profile.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CapabilityManifest {
    pub component: String,
    /// Interfaces the compiler derived from the program's effects.
    pub declared: Vec<String>,
    pub profile: RuntimeProfile,
}

/// Charter §14 M8 task 4 requires denying ambient authority by default. A richer
/// runtime is allowed but must be *explicit*, never inherited by accident.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RuntimeProfile {
    /// `no_std`-equivalent. Only declared interfaces may appear.
    Minimal,
    /// The component may additionally import the WASI CLI surface. Choosing this
    /// is a decision with a diff, not a default.
    WasiCli,
}

impl RuntimeProfile {
    /// Interfaces this profile tolerates beyond the declared set.
    fn allowance(self) -> &'static [&'static str] {
        match self {
            RuntimeProfile::Minimal => &[],
            RuntimeProfile::WasiCli => &[
                "wasi:cli/",
                "wasi:clocks/",
                "wasi:io/",
                "wasi:random/",
                "wasi:filesystem/",
            ],
        }
    }

    pub fn name(self) -> &'static str {
        match self {
            RuntimeProfile::Minimal => "minimal",
            RuntimeProfile::WasiCli => "wasi-cli",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CapabilityAudit {
    pub component: String,
    pub profile: RuntimeProfile,
    /// Declared and actually imported. The intended case.
    pub satisfied: Vec<String>,
    /// Imported but never declared. **Build failure** under `Minimal`.
    pub undeclared: Vec<String>,
    /// Declared but not imported. Not an error — the compiler may have been
    /// conservative, or dead code was eliminated — but worth surfacing, because
    /// it usually means an effect annotation is wrong.
    pub unused: Vec<String>,
    /// Undeclared, but tolerated by the chosen profile.
    pub allowed_by_profile: Vec<String>,
}

impl CapabilityAudit {
    pub fn is_compliant(&self) -> bool {
        self.undeclared.is_empty()
    }
}

/// Compare what a component *declares* against what the built artifact
/// *requests*. `actual` comes from the compiled component, never from source.
pub fn audit(manifest: &CapabilityManifest, actual: &[String]) -> CapabilityAudit {
    let matches_declared = |import: &str| {
        manifest
            .declared
            .iter()
            .any(|d| import == d || import.starts_with(d.as_str()))
    };
    let allowed = |import: &str| {
        manifest
            .profile
            .allowance()
            .iter()
            .any(|prefix| import.starts_with(prefix))
    };

    let mut satisfied = Vec::new();
    let mut undeclared = Vec::new();
    let mut allowed_by_profile = Vec::new();

    for import in actual {
        if matches_declared(import) {
            satisfied.push(import.clone());
        } else if allowed(import) {
            allowed_by_profile.push(import.clone());
        } else {
            undeclared.push(import.clone());
        }
    }

    let unused = manifest
        .declared
        .iter()
        .filter(|d| !actual.iter().any(|a| a == *d || a.starts_with(d.as_str())))
        .cloned()
        .collect();

    satisfied.sort();
    undeclared.sort();
    allowed_by_profile.sort();

    CapabilityAudit {
        component: manifest.component.clone(),
        profile: manifest.profile,
        satisfied,
        undeclared,
        unused,
        allowed_by_profile,
    }
}

/// `PW4007`, in the shape the architect specified.
pub fn render_undeclared_imports(audit: &CapabilityAudit) -> Option<String> {
    if audit.is_compliant() {
        return None;
    }
    let declared = if audit.satisfied.is_empty() && audit.unused.is_empty() {
        "  (none)".to_string()
    } else {
        audit
            .satisfied
            .iter()
            .chain(audit.unused.iter())
            .map(|s| format!("  - {s}"))
            .collect::<Vec<_>>()
            .join("\n")
    };
    let extra = audit
        .undeclared
        .iter()
        .map(|s| format!("  - {s}"))
        .collect::<Vec<_>>()
        .join("\n");

    Some(format!(
        "error: [PW4007] undeclared component import\n\n\
         Component `{}` declared:\n{declared}\n\n\
         The compiled artifact additionally imports:\n{extra}\n\n\
         note: runtime profile is `{}`\n\
         note: these imports are a requested authority surface, not authority already held —\n      \
         the host still decides what to link. But a declared world that does not describe\n      \
         what the artifact requests cannot be used as evidence of anything.\n\
         help: use the generated minimal runtime, declare the capabilities explicitly, or\n      \
         set `runtime_profile wasi-cli` for this component and accept the larger surface\n",
        audit.component,
        audit.profile.name(),
    ))
}

// --- E8 step 2: what a capability's type argument names ---------------------

use crate::codes;
use crate::diagnostics::{Detector, Diagnostic, Related, Repair, Severity};
use crate::hir::{Decl, DeclKind, Hir};
use std::collections::BTreeSet;

/// Everything a capability argument may legitimately name.
///
/// Types, opaque types **and modules**.
///
/// The corpus writes `database.read<Stores>`, and `Stores` is a MODULE — the
/// domain being read — not a type. A first version of this rule accepted only
/// types and reported both `R-002` and `R-008` as defective, which is the
/// false positive the architect's ruling specifically warns about: the check
/// exists so a name that resolves to nothing is caught early, not so a
/// convention the corpus already uses is outlawed.
///
/// Opaque types count for the reason the ruling gives — "unless the name refers
/// to a properly declared external/opaque contract". A module is exactly such a
/// contract, and so is an `opaque type`.
pub fn capability_argument_names(hirs: &[&Hir]) -> BTreeSet<String> {
    let mut out: BTreeSet<String> = hirs
        .iter()
        .flat_map(|h| h.all_decls().map(|(_, d)| d).collect::<Vec<_>>())
        .filter(|d| matches!(d.kind, DeclKind::Type | DeclKind::Opaque))
        .map(|d| d.name.clone())
        .collect();
    // Module names. `module Stores` is a header rather than a declaration, so
    // it is read from `module_of` — the same place resolution reads it.
    for h in hirs {
        for (id, _) in h.all_decls() {
            if let Some(m) = h.module_of(id) {
                out.insert(m.to_string());
            }
        }
    }
    out
}

/// `database.read<Stroes>` — a capability nobody can ever grant.
///
/// Architect ruling, 2026-08-07:
///
/// > Deployment is far too late to tell the developer "the node doesn't provide
/// > `database.read<Stroes>`". The actual problem is: `Stroes` does not name a
/// > type. That is a source-program error with a precise source span and an
/// > obvious repair. […] Not a warning. Authority requirements are not
/// > something we should "best effort" through.
///
/// The unresolved capability is deliberately KEPT in the contract — see
/// ADR-0020. Over-stating authority is refused work; under-stating it is
/// authority nobody approved. This rule is what makes the over-statement
/// visible at the moment it can be repaired.
pub fn capability_arguments(
    hir: &Hir,
    decl: &Decl,
    known: &BTreeSet<String>,
    out: &mut Vec<Diagnostic>,
) {
    let _ = hir;
    let Some(row) = &decl.declared_effects else {
        return;
    };
    for effect in row {
        let Some(argument) = argument_of(&effect.written) else {
            continue;
        };
        if known.contains(&argument) {
            continue;
        }
        let suggestion = nearest(&argument, known);
        let mut message = format!("unknown capability type argument `{argument}`");
        if let Some(near) = &suggestion {
            message.push_str(&format!("; did you mean `{near}`?"));
        }
        out.push(Diagnostic {
            code: codes::UNRESOLVED_CAPABILITY_ARGUMENT.id,
            invariant: codes::UNRESOLVED_CAPABILITY_ARGUMENT.invariant,
            reason: "capability_argument_names_no_type",
            detector: Detector::DeclarationRule,
            severity: Severity::Error,
            message,
            primary_span: effect.span.clone(),
            related: vec![Related {
                span: effect.span.clone(),
                label: match &suggestion {
                    Some(near) => {
                        format!("`{argument}` names nothing this program declares; `{near}` does")
                    }
                    None => format!("`{argument}` names nothing this program declares"),
                },
            }],
            explanation: Some(
                "A capability's type argument is part of its IDENTITY: \
                 `database.read<Stores>` and `database.read<Payments>` are different \
                 authority. An argument naming nothing produces a capability no \
                 deployment can ever grant, and without this rule the failure appears \
                 as `the node does not grant ..` at deployment — describing the \
                 deployment rather than the declaration that caused it, and far from \
                 anyone who could repair it."
                    .to_string(),
            ),
            repairs: match &suggestion {
                Some(near) => vec![Repair {
                    description: format!("did you mean `{near}`?"),
                    replacement: None,
                }],
                None => vec![Repair {
                    description: format!("declare `{argument}`, or import the module that does"),
                    replacement: None,
                }],
            },
        });
    }
}

/// The `<Argument>` of an effect, if it has one.
fn argument_of(effect: &str) -> Option<String> {
    let (_, rest) = effect.split_once('<')?;
    let arg = rest.trim_end_matches('>').trim();
    (!arg.is_empty()).then(|| arg.to_string())
}

/// The closest declared type, when one is close enough to be a typo.
///
/// A suggestion that is merely the alphabetically first type would be worse
/// than none — it sends the reader to an unrelated declaration. So the distance
/// is bounded relative to the length of what was written.
fn nearest(written: &str, known: &BTreeSet<String>) -> Option<String> {
    let budget = (written.len() / 3).max(1);
    known
        .iter()
        .map(|t| (distance(written, t), t))
        .filter(|(d, _)| *d <= budget)
        .min_by_key(|(d, t)| (*d, t.len()))
        .map(|(_, t)| t.clone())
}

/// Levenshtein distance, iteratively.
fn distance(a: &str, b: &str) -> usize {
    let (a, b): (Vec<char>, Vec<char>) = (a.chars().collect(), b.chars().collect());
    let mut prev: Vec<usize> = (0..=b.len()).collect();
    let mut row = vec![0usize; b.len() + 1];
    for (i, ca) in a.iter().enumerate() {
        row[0] = i + 1;
        for (j, cb) in b.iter().enumerate() {
            let cost = usize::from(ca != cb);
            row[j + 1] = (prev[j] + cost).min(prev[j + 1] + 1).min(row[j] + 1);
        }
        std::mem::swap(&mut prev, &mut row);
    }
    prev[b.len()]
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The exact import list E0 measured from the `no_std` guest.
    fn minimal_actual() -> Vec<String> {
        vec!["perfect-web:store/stores@0.1.0".to_string()]
    }

    /// The exact import list E0 measured from the `std` guest — 15 instances for
    /// a world declaring one. `docs/evidence/E0/spike-wasmtime-component.txt`.
    fn std_actual() -> Vec<String> {
        [
            "perfect-web:store/stores@0.1.0",
            "wasi:cli/environment@0.2.9",
            "wasi:cli/exit@0.2.9",
            "wasi:cli/stderr@0.2.9",
            "wasi:cli/stdin@0.2.9",
            "wasi:cli/stdout@0.2.9",
            "wasi:cli/terminal-input@0.2.9",
            "wasi:cli/terminal-output@0.2.9",
            "wasi:cli/terminal-stderr@0.2.9",
            "wasi:cli/terminal-stdin@0.2.9",
            "wasi:cli/terminal-stdout@0.2.9",
            "wasi:clocks/monotonic-clock@0.2.9",
            "wasi:io/error@0.2.9",
            "wasi:io/poll@0.2.9",
            "wasi:io/streams@0.2.9",
        ]
        .iter()
        .map(|s| s.to_string())
        .collect()
    }

    fn manifest(profile: RuntimeProfile) -> CapabilityManifest {
        CapabilityManifest {
            component: "public-store-query".to_string(),
            declared: vec!["perfect-web:store/stores".to_string()],
            profile,
        }
    }

    #[test]
    fn the_minimal_guest_is_compliant() {
        let a = audit(&manifest(RuntimeProfile::Minimal), &minimal_actual());
        assert!(a.is_compliant());
        assert_eq!(a.satisfied.len(), 1);
        assert!(a.undeclared.is_empty());
        assert!(render_undeclared_imports(&a).is_none());
    }

    #[test]
    fn the_std_guest_fails_the_minimal_profile_with_all_fourteen_extras() {
        let a = audit(&manifest(RuntimeProfile::Minimal), &std_actual());
        assert!(!a.is_compliant());
        assert_eq!(
            a.undeclared.len(),
            14,
            "E0 measured 15 imports for a one-capability world"
        );
        let text = render_undeclared_imports(&a).unwrap();
        assert!(text.contains("PW4007"), "{text}");
        assert!(text.contains("wasi:cli/environment"), "{text}");
        assert!(text.contains("wasi:io/streams"), "{text}");
        // The terminology correction must survive into the diagnostic.
        assert!(text.contains("requested authority surface"), "{text}");
        assert!(text.contains("not authority already held"), "{text}");
    }

    #[test]
    fn the_wasi_cli_profile_tolerates_them_but_only_when_chosen_explicitly() {
        let a = audit(&manifest(RuntimeProfile::WasiCli), &std_actual());
        assert!(a.is_compliant());
        assert_eq!(a.allowed_by_profile.len(), 14);
        // Still visible: choosing the profile does not make the surface invisible.
        assert!(!a.allowed_by_profile.is_empty());
    }

    #[test]
    fn an_import_outside_the_profile_allowance_still_fails() {
        let mut actual = std_actual();
        actual.push("wasi:sockets/tcp@0.2.9".to_string());
        let a = audit(&manifest(RuntimeProfile::WasiCli), &actual);
        assert!(
            !a.is_compliant(),
            "sockets are not in the wasi-cli allowance"
        );
        assert_eq!(a.undeclared, vec!["wasi:sockets/tcp@0.2.9".to_string()]);
    }

    #[test]
    fn a_declared_but_unimported_capability_is_reported_without_failing() {
        let m = CapabilityManifest {
            component: "public-store-query".to_string(),
            declared: vec![
                "perfect-web:store/stores".to_string(),
                "perfect-web:payments/secret".to_string(),
            ],
            profile: RuntimeProfile::Minimal,
        };
        let a = audit(&m, &minimal_actual());
        assert!(a.is_compliant(), "over-declaring is not a build failure");
        assert_eq!(a.unused, vec!["perfect-web:payments/secret".to_string()]);
    }

    #[test]
    fn the_audit_can_go_red_and_green() {
        // docs/RISK_QUEUE.md: a check that cannot fail is not a check.
        let m = manifest(RuntimeProfile::Minimal);
        assert!(audit(&m, &minimal_actual()).is_compliant());
        assert!(!audit(&m, &std_actual()).is_compliant());
    }

    #[test]
    fn a_secret_capability_leaking_into_an_edge_component_is_caught() {
        // Charter §14 M8 task 8: "edge component requests payment secret".
        let m = CapabilityManifest {
            component: "public-edge-renderer".to_string(),
            declared: vec!["perfect-web:store/stores".to_string()],
            profile: RuntimeProfile::Minimal,
        };
        let actual = vec![
            "perfect-web:store/stores@0.1.0".to_string(),
            "perfect-web:payments/secret@0.1.0".to_string(),
        ];
        let a = audit(&m, &actual);
        assert!(!a.is_compliant());
        assert_eq!(
            a.undeclared,
            vec!["perfect-web:payments/secret@0.1.0".to_string()]
        );
    }
}
