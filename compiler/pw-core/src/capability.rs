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

#[cfg(test)]
mod tests {
    use super::*;

    /// The exact import list E0 measured from the `no_std` guest.
    fn minimal_actual() -> Vec<String> {
        vec!["perfect-web:store/stores@0.1.0".to_string()]
    }

    /// The exact import list E0 measured from the `std` guest — 15 instances for
    /// a world declaring one. `docs/evidence/M0/spike-wasmtime-component.txt`.
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
