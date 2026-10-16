//! Complete executable-program validation, shared by CLI check and build.
//!
//! Semantic-only checking remains available for partial/library HIR. This
//! module is the sole composition authority for executable programs.
use crate::check::Unit;
use crate::diagnostics::{Detector, Diagnostic, Repair, Severity};

pub type Diagnostics = Vec<(String, Vec<Diagnostic>)>;

/// The prerequisite gate used by each public backend and complete validation.
/// A borrowed proof never leaves this callback or applies to different HIR.
pub(crate) fn with_checked<R>(
    units: &[Unit],
    f: impl FnOnce(&crate::backend::lower::Checked<'_>) -> Result<R, String>,
) -> Result<R, String> {
    with_preparation(units, |result| {
        let (checked, _) = result.map_err(|ds| {
            format!(
                "the program does not check, so nothing is compiled: {}",
                ds.iter()
                    .flat_map(|(_, ds)| ds.iter())
                    .filter(|d| d.is_error())
                    .map(|d| format!("[{}] {}", d.code, d.message))
                    .collect::<Vec<_>>()
                    .join("; ")
            )
        })?;
        f(&checked)
    })
}

/// One implementation prepares and checks the shared semantic environment.
fn with_preparation<R>(
    units: &[Unit],
    f: impl FnOnce(Result<(crate::backend::lower::Checked<'_>, Diagnostics), Diagnostics>) -> R,
) -> R {
    let hirs: Vec<_> = units.iter().map(|u| &u.hir).collect();
    let ws = crate::resolve::Workspace::build(&hirs);
    let sigs = crate::signatures::Signatures::build(&ws, &hirs);
    let mut inference = crate::effects::Inference::new(&sigs, &ws);
    inference.run(&hirs);
    let cx = crate::backend::lower::Context {
        hirs: &hirs,
        ws: &ws,
        sigs: &sigs,
        contracts: &[],
    };
    let (checked, diagnostics) =
        match crate::backend::lower::Checked::diagnose_inferred(units, cx, &inference) {
            Ok(result) => result,
            Err(diagnostics) => return f(Err(diagnostics)),
        };
    // No page planner, contract generator or encoder runs after semantic errors.
    checked.set_pages(crate::page_values::analyze_inferred(
        &hirs, &ws, &sigs, &inference,
    ));
    let contracts =
        crate::contract::contracts_inferred(&hirs, &sigs, &ws, checked.pages(), &inference);
    let prepared = checked.with_contracts(&contracts);
    f(Ok((prepared, diagnostics)))
}

/// Typed stage/subreason metadata, independent of the underlying refusal text.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Stage {
    Handler,
    Component,
    Placeholder,
    PureJs,
    Wit,
    Template,
    Speculation,
    Computed,
    Graph,
    Event,
    Page,
    Signal,
}
impl Stage {
    pub fn reason(self) -> &'static str {
        match self {
            Self::Handler => "backend_handler",
            Self::Component => "backend_component",
            Self::Placeholder => "backend_placeholder",
            Self::PureJs => "backend_pure_js",
            Self::Wit => "backend_wit",
            Self::Template => "backend_template",
            Self::Speculation => "backend_speculation",
            Self::Computed => "backend_computed",
            Self::Graph => "backend_graph",
            Self::Event => "backend_event",
            Self::Page => "backend_page",
            Self::Signal => "backend_signal",
        }
    }
}
#[derive(Debug)]
pub(crate) struct Refusal {
    pub stage: Stage,
    pub owner: String,
    pub message: String,
    at: Option<String>,
}
impl Refusal {
    pub fn new(stage: Stage, owner: &str, message: String) -> Self {
        Self {
            stage,
            owner: owner.into(),
            message,
            at: None,
        }
    }
    pub fn at(mut self, text: &str) -> Self {
        self.at = Some(text.into());
        self
    }
    fn diagnostic(&self, units: &[Unit]) -> Option<(usize, Diagnostic)> {
        let mut located = None;
        for (i, unit) in units.iter().enumerate() {
            for (id, _) in unit.hir.all_decls() {
                let path = crate::contract::component_id(&unit.hir, id);
                if (self.owner == path || self.owner.starts_with(&format!("{path}.")))
                    && located.as_ref().is_none_or(|(_, _, n)| path.len() > *n)
                {
                    located = Some((i, unit.hir.decl_span(id), path.len()));
                }
            }
        }
        let (unit, mut span, _) =
            located.or_else(|| units.first().map(|u| (0, 0..u.src.len().min(1), 0)))?;
        if let Some(at) = &self.at
            && let Some(offset) = units[unit].src[span.clone()].find(at)
        {
            span = span.start + offset..span.start + offset + at.len();
        }
        let code = crate::codes::EXECUTABLE_PROGRAM;
        Some((unit, Diagnostic {
            code: code.id, invariant: code.invariant, reason: self.stage.reason(), detector: Detector::Backend,
            severity: Severity::Error, message: self.message.clone(), primary_span: span,
            related: Vec::new(),
            explanation: Some("The executable backend refused this program. Accepting it during check would promise an artifact the compiler cannot execute. Check and build use the same lowering, encoding, audit and page plans.".into()),
            repairs: vec![Repair { description: "use a supported construct described by the refusal, or supply the body the program depends on".into(), replacement: None }],
        }))
    }
}

/// A complete validation report; its artifact plan is private. Only a report
/// with no errors can release the successful executable plan through build().
pub struct Validation {
    pub diagnostics: Diagnostics,
    plan: Result<crate::build::Build, String>,
}
impl Validation {
    pub fn into_build(self) -> Result<crate::build::Build, String> {
        let errors: Vec<_> = self
            .diagnostics
            .iter()
            .flat_map(|(file, ds)| {
                ds.iter()
                    .filter(|d| d.is_error())
                    .map(move |d| format!("{file}: [{}] {}", d.code, d.message))
            })
            .collect();
        if errors.is_empty() {
            self.plan
        } else {
            Err(format!(
                "the program does not check, so nothing is compiled: {}",
                errors.join("; ")
            ))
        }
    }
    /// Historical core inspection API. Build::write still refuses a partial
    /// plan; the CLI uses into_build, never this inspection projection.
    pub(crate) fn inspect(self) -> Result<crate::build::Build, String> {
        if self.plan.is_err() {
            self.into_build()
        } else {
            self.plan
        }
    }
}

pub fn build(units: &[Unit]) -> Result<crate::build::Build, String> {
    validate(units).into_build()
}

pub fn validate(units: &[Unit]) -> Validation {
    with_preparation(units, |result| {
        let (checked, mut diagnostics) = match result {
            Ok(value) => value,
            Err(diagnostics) => {
                return Validation {
                    diagnostics,
                    plan: Err("the program does not check".into()),
                };
            }
        };
        let (plan, refusals) = match crate::build::prepare(&checked) {
            Ok(plan) => {
                let refusals = plan.support_refusals();
                (Ok(plan), refusals)
            }
            Err(refusals) => {
                let text = refusals
                    .iter()
                    .map(|r| r.message.as_str())
                    .collect::<Vec<_>>()
                    .join("; ");
                (Err(text), refusals)
            }
        };
        for refusal in refusals {
            if let Some((unit, diagnostic)) = refusal.diagnostic(units) {
                diagnostics[unit].1.push(diagnostic);
            }
        }
        Validation { diagnostics, plan }
    })
}
