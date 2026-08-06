//! The one diagnostic type every semantic detector produces.
//!
//! Architect ruling: **no semantic rule may live only in the CLI**, and all
//! detectors must produce the same structured value so the same checker can be
//! driven by the CLI, a language server, a test harness, a build system, a
//! browser playground, an AI coding loop, and the semantic PR analyser.
//! Rendering belongs to callers.
//!
//! The other ruling this encodes: **one public code per violated invariant, not
//! per detector.** A developer should learn *"a resource cannot outlive the
//! scope that owns it"*, not which implementation pass happened to notice. The
//! detector and the sub-reason are metadata.

use std::ops::Range;

pub type Span = Range<usize>;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Severity {
    Error,
    Warning,
}

/// Which analysis found the defect. Metadata; never part of the public code.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Detector {
    /// Read directly off a declaration header.
    DeclarationRule,
    /// `pw-core`'s scope graph, from handle flow.
    ScopeGraph,
    /// The exhaustiveness checker's pattern matrix.
    PatternMatrix,
    /// Declared-versus-actual component imports.
    CapabilityAudit,
    /// The type-directed boundary decoder.
    AbiDecoder,
}

impl Detector {
    pub fn name(self) -> &'static str {
        match self {
            Detector::DeclarationRule => "declaration_rule",
            Detector::ScopeGraph => "scope_graph",
            Detector::PatternMatrix => "pattern_matrix",
            Detector::CapabilityAudit => "capability_audit",
            Detector::AbiDecoder => "abi_decoder",
        }
    }
}

/// A span other than the primary one that the reader needs in order to
/// understand the defect — charter §16.3's "where the value originated".
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Related {
    pub span: Span,
    pub label: String,
}

/// A concrete legal alternative. Charter §16.3 requires at least one.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Repair {
    pub description: String,
    /// A machine-applicable edit, when one exists. `None` means the repair is
    /// advisory — which is honest, and better than a fix-it that guesses.
    pub replacement: Option<(Span, String)>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Diagnostic {
    /// Stable, public, identifies the **invariant**.
    pub code: &'static str,
    /// One sentence naming the invariant, in the developer's vocabulary.
    pub invariant: &'static str,
    /// Machine-readable sub-reason within the invariant. Distinguishes repairs
    /// without splitting the code.
    pub reason: &'static str,
    pub detector: Detector,
    pub severity: Severity,
    pub message: String,
    pub primary_span: Span,
    pub related: Vec<Related>,
    /// Why the rule exists, when that is not obvious from the message.
    pub explanation: Option<String>,
    pub repairs: Vec<Repair>,
}

impl Diagnostic {
    pub fn error(
        code: &'static str,
        invariant: &'static str,
        detector: Detector,
        message: impl Into<String>,
        primary_span: Span,
    ) -> Self {
        Self {
            code,
            invariant,
            reason: "",
            detector,
            severity: Severity::Error,
            message: message.into(),
            primary_span,
            related: Vec::new(),
            explanation: None,
            repairs: Vec::new(),
        }
    }

    pub fn warning(
        code: &'static str,
        invariant: &'static str,
        detector: Detector,
        message: impl Into<String>,
        primary_span: Span,
    ) -> Self {
        Self {
            severity: Severity::Warning,
            ..Self::error(code, invariant, detector, message, primary_span)
        }
    }

    pub fn reason(mut self, r: &'static str) -> Self {
        self.reason = r;
        self
    }

    pub fn related(mut self, span: Span, label: impl Into<String>) -> Self {
        self.related.push(Related {
            span,
            label: label.into(),
        });
        self
    }

    pub fn explain(mut self, e: impl Into<String>) -> Self {
        self.explanation = Some(e.into());
        self
    }

    pub fn repair(mut self, description: impl Into<String>) -> Self {
        self.repairs.push(Repair {
            description: description.into(),
            replacement: None,
        });
        self
    }

    pub fn is_error(&self) -> bool {
        self.severity == Severity::Error
    }
}

/// Deprecated code aliases, kept only while the corpus migrates.
///
/// Architect migration rule: if the corpus already declares one code as
/// canonical, preserve it; mark the other a deprecated alias; have every
/// detector emit the canonical code; remove the alias before diagnostic-code
/// stability is publicly promised.
///
/// Two permanent codes for one invariant would be wrong. Aliasing during a
/// migration is fine.
pub const DEPRECATED_ALIASES: &[(&str, &str)] = &[
    // (alias as written in the corpus, canonical invariant code)
    ("PW0326", "PW2004"), // a resource cannot outlive the scope that owns it
    ("PW0311", "PW2002"), // an ordinary task cannot be detached from its scope
    // Both are "a body requires a capability its world cannot grant" — one for
    // a database in the browser, one for a secret at the edge. The corpus gave
    // them separate codes before the placement algebra existed; one invariant
    // gets one code, not one per instance.
    ("PW0301", "PW5002"),
    ("PW0324", "PW5002"),
    // The corpus's number for "the placement you named cannot grant this".
    ("PW0323", "PW5005"),
    ("PW0302", "PW5003"),
    // R-006: a secret reaching a sink declared `Public`.
    ("PW0304", "PW5006"), // a secret cannot be rendered to the browser
    ("PW0303", "PW5004"), // a shared cache key must carry every partition
    // R-004 declares PW0100, which `rules.rs` also uses for this invariant —
    // the charter §16.3 worked example. The scope graph and the label algebra
    // reach the same conclusion by different routes, so one code, not two.
    ("PW0100", "PW5001"),
    // The layout family. `PW3001` is the corpus's code for an effect reaching a
    // context that may not have it — which is one invariant, not one per
    // effect family.
    ("PW3001", "PW0401"),
    ("PW3002", "PW0401"),
    // The frame-phase family. Each names "work happening in a phase that does
    // not permit it" — a write during measure, a measure after paint. One
    // invariant, stated once; which phase and which effect are metadata.
    ("PW3004", "PW0402"),
    ("PW3008", "PW0402"),
    // R-034 measures after a layout-affecting write in one transaction. The
    // measure block is measuring, which is what it is for — but a measure that
    // an earlier write has already invalidated is not the measure phase at all,
    // so it is the same invariant seen from the ordering side.
    ("PW3003", "PW0402"),
    // A compositor animation and a painter both forbid work regardless of what
    // the declaration admits to, which is what PW0401 says.
    ("PW3006", "PW0401"),
    ("PW3007", "PW0401"),
    // These two are NOT effect-row violations, so they are not aliased onto
    // one. A cycle is a relation between an observation and a write; a false
    // independence is a declared assertion the code contradicts.
    ("PW3005", "PW0403"),
    ("PW3009", "PW0404"),
    // A view that fetches, a build-time page that reads the wall clock, a
    // shared fragment that does. Each is "an effect is not permitted where this
    // runs, whatever the row says" — PW0401 — differing only in which context
    // and which effect, which the diagnostic carries as metadata.
    ("PW0300", "PW0401"),
    ("PW0314", "PW0401"),
    ("PW0315", "PW0401"),
    ("PW0316", "PW5011"), // a list over a mutable collection needs a stable key
    ("PW0317", "PW5012"), // an element may only contain the children HTML permits
    ("PW0318", "PW5013"), // interactive behaviour belongs on an interactive element
    ("PW0319", "PW5014"), // a form control must have something that names it
    // Both are "an unsafe escape hatch must record why it is necessary": one
    // written with no justification at all, one with an unaudited use.
    ("PW0329", "PW5010"),
    // R-024 has no audit record at all, which is the most incomplete one.
    ("PW0322", "PW5010"),
    ("PW3010", "PW5010"),
];

/// Resolve a corpus-declared code to the canonical invariant code.
pub fn canonical_code(declared: &str) -> &str {
    DEPRECATED_ALIASES
        .iter()
        .find(|(alias, _)| *alias == declared)
        .map(|(_, canonical)| *canonical)
        .unwrap_or(declared)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn an_alias_resolves_to_its_canonical_invariant() {
        assert_eq!(canonical_code("PW0326"), "PW2004");
        assert_eq!(canonical_code("PW0100"), "PW5001");
        // A code with no alias is its own canonical form.
        assert_eq!(canonical_code("PW0312"), "PW0312");
    }

    #[test]
    fn syntax_codes_and_semantic_codes_do_not_share_a_range() {
        // They did: `PW0100` was the parser's "expected X, found Y" AND the
        // shared-cache invariant, so a corpus file declaring it could match
        // either. Syntax now lives in PW00xx and semantics at PW01xx and above.
        for (alias, canonical) in DEPRECATED_ALIASES {
            assert!(
                !alias.starts_with("PW000") && !alias.starts_with("PW009"),
                "{alias} is in the syntax range"
            );
            assert!(
                !canonical.starts_with("PW000"),
                "{canonical} is in the syntax range"
            );
        }
    }

    #[test]
    fn one_invariant_can_be_reported_by_two_detectors_under_one_code() {
        // The architect's worked example: the developer learns one invariant;
        // the detector and reason are metadata.
        let from_declaration = Diagnostic::error(
            "PW2004",
            "a resource cannot outlive the scope that owns it",
            Detector::DeclarationRule,
            "subscription `Tracking` declares `application` scope",
            0..10,
        )
        .reason("declared_scope_exceeds_owner");

        let from_graph = Diagnostic::error(
            "PW2004",
            "a resource cannot outlive the scope that owns it",
            Detector::ScopeGraph,
            "task handle `load_menu` escapes the component that owns it",
            20..30,
        )
        .reason("handle_escapes_to_ancestor");

        assert_eq!(from_declaration.code, from_graph.code);
        assert_eq!(from_declaration.invariant, from_graph.invariant);
        assert_ne!(from_declaration.detector, from_graph.detector);
        assert_ne!(from_declaration.reason, from_graph.reason);
    }

    #[test]
    fn a_diagnostic_carries_the_charter_16_3_shape() {
        let d = Diagnostic::error(
            "PW0100",
            "a shared cache may contain only Public values",
            Detector::DeclarationRule,
            "msg",
            0..5,
        )
        .related(10..20, "declared `session` here")
        .explain("why")
        .repair("use `cache private`");
        assert!(!d.related.is_empty(), "origin span");
        assert!(d.explanation.is_some());
        assert!(!d.repairs.is_empty(), "at least one legal alternative");
    }
}
