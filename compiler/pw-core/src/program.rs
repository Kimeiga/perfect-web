//! Complete executable-program validation, shared by CLI check and build.
//!
//! Semantic-only checking remains available for partial/library HIR. This
//! module is the sole composition authority for executable programs.
use crate::check::Unit;
use crate::diagnostics::{Detector, Diagnostic, Repair, Severity};

pub type Diagnostics = Vec<(String, Vec<Diagnostic>)>;

/// The prerequisite gate used by each public backend and complete validation.
/// A borrowed proof never leaves this callback or applies to different HIR.
pub(crate) fn with_backend<R>(
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

/// Run semantic checks with a context derived from these exact units, then
/// lend the private prerequisite proof to a backend consumer. The callback
/// cannot mint or retain a proof for another program. This is semantic-only;
/// `validate` is the full executable-program authority used by CLI commands.
pub fn with_semantic_program<R>(
    units: &[Unit],
    f: impl FnOnce(&crate::backend::lower::Checked<'_>) -> R,
) -> Result<R, Diagnostics> {
    with_preparation(units, |result| result.map(|(checked, _)| f(&checked)))
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
    pub(crate) fn wit(failure: crate::wit::LocatedError, hirs: &[&crate::hir::Hir]) -> Self {
        let error = failure.error;
        let owner = failure
            .declaration
            .map(|def| crate::contract::component_id(hirs[def.unit], crate::hir::DeclId(def.decl)))
            .unwrap_or_else(|| match &error {
                crate::wit::WitError::Unmappable { at, .. } => at.clone(),
                crate::wit::WitError::Collision { second, .. } => second.clone(),
                crate::wit::WitError::Recursive { ty, .. } => ty.clone(),
                crate::wit::WitError::Claimed { .. } => String::new(),
            });
        Self::new(Stage::Wit, &owner, format!("WIT: {error}"))
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
    plan: Plan,
}
// Neither public diagnostics nor a caller-provided backend Context can mint
// this final proof. Its only constructor is Validation::finish below.
struct Executable {
    build: crate::build::Build,
}
enum Plan {
    Validated(Executable),
    Refused {
        partial: Option<crate::build::Build>,
        message: String,
    },
}
impl Validation {
    fn finish(diagnostics: Diagnostics, partial: Result<crate::build::Build, String>) -> Self {
        let errors: Vec<_> = diagnostics
            .iter()
            .flat_map(|(file, ds)| {
                ds.iter()
                    .filter(|d| d.is_error())
                    .map(move |d| format!("{file}: [{}] {}", d.code, d.message))
            })
            .collect();
        let plan = match (errors.is_empty(), partial) {
            (true, Ok(build)) => Plan::Validated(Executable { build }),
            (_, partial) => {
                let (partial, fallback) = match partial {
                    Ok(build) => (Some(build), String::new()),
                    Err(why) => (None, why),
                };
                let message = if errors.is_empty() {
                    fallback
                } else {
                    format!(
                        "the program does not check, so nothing is compiled: {}",
                        errors.join("; ")
                    )
                };
                Plan::Refused { partial, message }
            }
        };
        Self { diagnostics, plan }
    }
    pub fn into_build(self) -> Result<crate::build::Build, String> {
        match self.plan {
            Plan::Validated(ready) => Ok(ready.build),
            Plan::Refused { message, .. } => Err(message),
        }
    }
    /// Historical core inspection API. Build::write refuses a partial plan;
    /// the CLI uses into_build and cannot receive an unvalidated plan.
    pub(crate) fn inspect(self) -> Result<crate::build::Build, String> {
        match self.plan {
            Plan::Validated(ready) => Ok(ready.build),
            Plan::Refused {
                partial: Some(build),
                ..
            } => Ok(build),
            Plan::Refused { message, .. } => Err(message),
        }
    }
}

/// Parse one complete source program. Syntax refusals retain independent
/// semantic findings from clean siblings, but never enter executable backends.
pub fn validate_sources(files: &[(String, String)]) -> Validation {
    let parsed: Vec<_> = files
        .iter()
        .map(|(_, src)| pw_syntax::parse_tree(src))
        .collect();
    if parsed.iter().any(|file| !file.ok()) {
        return Validation::finish(
            crate::check::check_sources(files),
            Err("the program does not parse".into()),
        );
    }
    let units: Vec<_> = files
        .iter()
        .zip(parsed)
        .map(|((path, src), parsed)| Unit {
            path: path.clone(),
            src: src.clone(),
            hir: crate::lower::lower_file(src, &parsed.green),
        })
        .collect();
    validate(&units)
}

pub fn build(units: &[Unit]) -> Result<crate::build::Build, String> {
    validate(units).into_build()
}

pub fn validate(units: &[Unit]) -> Validation {
    with_preparation(units, |result| {
        let (checked, mut diagnostics) = match result {
            Ok(value) => value,
            Err(diagnostics) => {
                return Validation::finish(diagnostics, Err("the program does not check".into()));
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
        Validation::finish(diagnostics, plan)
    })
}
