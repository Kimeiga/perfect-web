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
const DEPRECATED_ALIASES: &[(&str, &str)] = &[
    // (alias as written in the corpus, canonical invariant code)
    ("PW0326", "PW2004"), // a resource cannot outlive the scope that owns it
    ("PW0311", "PW2002"), // an ordinary task cannot be detached from its scope
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
        assert_eq!(canonical_code("PW0100"), "PW0100");
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
