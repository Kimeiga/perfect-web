//! Diagnostic model + rendering via `annotate-snippets`.
//!
//! Charter §16.3 sets the bar. A rejection should say:
//!
//! ```text
//! what rule was violated
//! where the value/effect originated
//! which boundary made it invalid
//! one or more legal alternatives
//! relevant inferred type/effect/label
//! ```
//!
//! That shape needs **at least two spans** — origin and boundary — plus footers.
//! This module exists to prove that shape renders well before Milestone 2
//! commits to it.

use annotate_snippets::{AnnotationKind, Level, Renderer, Snippet};

use crate::lexer::Span;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Severity {
    Error,
    Warning,
}

#[derive(Debug, Clone)]
pub struct Label {
    pub span: Span,
    pub text: String,
    /// Primary = the place the rule fired. Context = supporting evidence, such
    /// as where an offending privacy label was introduced.
    pub primary: bool,
}

#[derive(Debug, Clone)]
pub struct Diagnostic {
    pub severity: Severity,
    /// Stable machine-readable code, e.g. `PW0100`. Stable codes matter for the
    /// compile-fail corpus (charter §16) and for the AI benchmark (§19).
    pub code: &'static str,
    pub title: String,
    pub labels: Vec<Label>,
    pub notes: Vec<String>,
    pub helps: Vec<String>,
}

impl Diagnostic {
    pub fn new(severity: Severity, code: &'static str, title: impl Into<String>) -> Self {
        Self {
            severity,
            code,
            title: title.into(),
            labels: Vec::new(),
            notes: Vec::new(),
            helps: Vec::new(),
        }
    }

    pub fn error(code: &'static str, title: impl Into<String>) -> Self {
        Self::new(Severity::Error, code, title)
    }

    pub fn is_error(&self) -> bool {
        self.severity == Severity::Error
    }

    pub fn primary(mut self, span: Span, text: impl Into<String>) -> Self {
        self.labels.push(Label {
            span,
            text: text.into(),
            primary: true,
        });
        self
    }

    /// A secondary span. This is the charter §16.3 "where the value originated"
    /// slot — the reason a two-span diagnostic is required rather than nice.
    pub fn context(mut self, span: Span, text: impl Into<String>) -> Self {
        self.labels.push(Label {
            span,
            text: text.into(),
            primary: false,
        });
        self
    }

    pub fn note(mut self, text: impl Into<String>) -> Self {
        self.notes.push(text.into());
        self
    }

    pub fn help(mut self, text: impl Into<String>) -> Self {
        self.helps.push(text.into());
        self
    }
}

/// Render diagnostics to a string.
///
/// `styled = false` produces byte-stable output with no ANSI escapes, which is
/// what the compile-fail snapshot tests will consume from Milestone 2 onward.
pub fn render(source: &str, path: &str, diagnostics: &[Diagnostic], styled: bool) -> String {
    let renderer = if styled {
        Renderer::styled()
    } else {
        Renderer::plain()
    };

    let mut out = String::new();
    for d in diagnostics {
        let level = match d.severity {
            Severity::Error => Level::ERROR,
            Severity::Warning => Level::WARNING,
        };

        let mut snippet = Snippet::source(source).path(path).line_start(1);
        for label in &d.labels {
            let kind = if label.primary {
                AnnotationKind::Primary
            } else {
                AnnotationKind::Context
            };
            snippet = snippet.annotation(kind.span(label.span.clone()).label(&label.text));
        }

        let mut group = level
            .primary_title(format!("[{}] {}", d.code, d.title))
            .element(snippet);
        for n in &d.notes {
            group = group.element(Level::NOTE.message(n));
        }
        for h in &d.helps {
            group = group.element(Level::HELP.message(h));
        }

        out.push_str(&renderer.render(&[group]));
        out.push('\n');
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn plain_rendering_has_no_ansi_escapes() {
        let src = "session query cart() cache shared";
        let d = Diagnostic::error("PW0100", "test")
            .primary(21..33, "here")
            .context(0..7, "origin");
        let out = render(src, "test.pw", &[d], false);
        assert!(!out.contains('\u{1b}'), "plain output must be ANSI-free");
        assert!(out.contains("PW0100"));
    }

    #[test]
    fn both_spans_appear_in_the_rendered_output() {
        let src = "session query cart() cache shared";
        let d = Diagnostic::error("PW0100", "t")
            .primary(21..33, "BOUNDARY_MARKER")
            .context(0..7, "ORIGIN_MARKER");
        let out = render(src, "t.pw", &[d], false);
        assert!(out.contains("BOUNDARY_MARKER"), "{out}");
        assert!(out.contains("ORIGIN_MARKER"), "{out}");
    }
}
