//! **A value the reader's device keeps** (track `device-store`, ADR-XXXX):
//! charter §9.1's fourth kind of state, as its first version builds it.
//!
//! ```text
//! signal todos: List<Todo> = []
//!     storage  device "todos-pleris"
//!     privacy  Device
//!     conflict last_write_wins
//! ```
//!
//! A module's signal (ADR-0144) that the device provides: its value is the
//! default the device starts with, every page and view that names it holds
//! the device's value, and a handler writes it as it writes any signal. The
//! runtime keeps it in the browser's storage, reads it before anything it
//! decides is shown, and tells every other tab of a write.
//!
//! A read of it carries `Device` (`Signatures`' label for it), which follows
//! the value as every label does (ADR-0129). What this module refuses:
//!
//! - **PW6801**: a device value in a command's argument, or a command call it
//!   decides: a command runs on the server.
//! - **PW6802**: a device value in a page query's key, which the browser
//!   sends to read the query again (ADR-0152).
//! - **PW6803**: a device value where the browser sends it of the page: a
//!   navigation's arguments, an address (`href`, `src`, ..) or a style that
//!   may hold one, and a form control's value inside a `<form>`.
//! - **PW6804**: a session's, user's, organization's or secret's value
//!   assigned to a device's signal: on a shared computer it would outlive
//!   the session it belongs to.
//! - **PW6805**: the clauses: `storage device "name"`, the name of letters,
//!   digits, `-`, `_` and `.`, one a program; `privacy Device`; `conflict
//!   last_write_wins`.
//! - **PW6806**: `version n` and a `migrate` of the functions that take each
//!   earlier version to the next, each pure, of one parameter, chained by
//!   type to the declared one.
//! - **PW6807**: a type the device can hold: data the browser's wire carries,
//!   with no function, no type that contains itself and no opaque type whose
//!   `where` a decoder would have to run.
//! - **PW6808**: no `provide` of it: the device provides it.

use std::collections::{BTreeMap, BTreeSet};

use crate::codes::{self, Code};
use crate::diagnostics::{Detector, Diagnostic, Related, Repair, Severity};
use crate::hir::{AttrValue, BinOp, Body, Decl, DeclId, DeclKind, Expr, ExprId, Hir, Node, Span};
use crate::privacy::{Label, Restriction};
use crate::resolve::{DefId, Resolution};
use crate::resolved::{Builtin, ResolvedType};
use crate::signatures::Signatures;

/// **Is `decl` a signal the device keeps?** A module's signal whose
/// `storage` is `device`. What else it must say is [`check`]'s.
pub fn is_kept(decl: &Decl) -> bool {
    decl.kind == DeclKind::Signal
        && decl
            .policy("storage")
            .and_then(|p| crate::policy::storage(&p.value))
            .is_some_and(|(word, _)| word == "device")
}

/// **What a signal the device keeps declares**, read as written: the name it
/// is kept under, its version, and the functions that take each earlier
/// version to the next, in order.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Kept {
    pub name: String,
    pub version: u32,
    pub migrations: Vec<String>,
}

impl Kept {
    pub fn of(decl: &Decl) -> Option<Kept> {
        if !is_kept(decl) {
            return None;
        }
        let name = decl
            .policy("storage")
            .and_then(|p| crate::policy::storage(&p.value))
            .and_then(|(_, name)| name)?
            .to_string();
        Some(Kept {
            name,
            version: version(decl),
            migrations: migrations(decl),
        })
    }

    /// **The key a version is kept under** (ADR-XXXX, decision 4): the name
    /// itself at version 1, `name@v` from version 2.
    pub fn key(&self, version: u32) -> String {
        match version {
            1 => self.name.clone(),
            v => format!("{}@{v}", self.name),
        }
    }
}

/// The declared `version`, 1 where none is written.
fn version(decl: &Decl) -> u32 {
    decl.policy("version")
        .and_then(|p| p.value.trim().parse::<u32>().ok())
        .unwrap_or(1)
}

/// Each `migrate` clause's functions, in the order written.
fn migrations(decl: &Decl) -> Vec<String> {
    decl.policies
        .iter()
        .filter(|p| p.name == "migrate")
        .flat_map(|p| {
            p.value
                .split(',')
                .map(|n| n.trim().to_string())
                .filter(|n| !n.is_empty())
                .collect::<Vec<_>>()
        })
        .collect()
}

/// **What a name may be**: letters, digits, `-`, `_` and `.`, from 1 to 128
/// of them, beginning with a letter. A key the program chooses, which other
/// scripts of its origin see, and which no `@` or `!` the runtime writes
/// after it can be confused with.
fn well_named(name: &str) -> bool {
    !name.is_empty()
        && name.len() <= 128
        && name.starts_with(|c: char| c.is_ascii_alphabetic())
        && name
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || matches!(c, '-' | '_' | '.'))
}

/// **Every signal the device keeps, in the program**, by its identity.
pub fn kept_signals(hirs: &[&Hir]) -> BTreeMap<DefId, Kept> {
    let mut out = BTreeMap::new();
    for (unit, hir) in hirs.iter().enumerate() {
        for (id, decl) in hir.all_decls() {
            if let Some(kept) = Kept::of(decl) {
                out.insert(DefId { unit, decl: id.0 }, kept);
            }
        }
    }
    out
}

/// **The signals the device keeps that a page holds** (track `device-store`):
/// each one its body, its layout's or a view it composes names, through every
/// view those compose, once, by the name it is declared with, in the order
/// first named. A page holds one value of each, the device's, which every
/// view in it reads.
pub fn held_by(
    hirs: &[&Hir],
    ws: &crate::resolve::Workspace,
    sigs: &Signatures,
    unit: usize,
    page: DeclId,
) -> Vec<(String, DefId)> {
    let kept = kept_signals(hirs);
    let mut out: Vec<(String, DefId)> = Vec::new();
    if kept.is_empty() {
        return out;
    }
    let mut seen: BTreeSet<DefId> = BTreeSet::new();
    let mut todo: Vec<DefId> = vec![DefId { unit, decl: page.0 }];
    if let Some(layout) = crate::layouts::of(hirs, ws, unit, hirs[unit].decl(page)) {
        todo.push(layout);
    }
    while let Some(def) = todo.pop() {
        if !seen.insert(def) {
            continue;
        }
        let Some(decl) = crate::resolve::declaration(hirs, def) else {
            continue;
        };
        let Some(body) = decl.body.map(|b| hirs[def.unit].body(b)) else {
            continue;
        };
        for (name, signal) in crate::template_ir::signals_named(hirs, ws, def.unit, body) {
            if let Some(k) = kept.get(&signal)
                && !out.iter().any(|(_, d)| *d == signal)
            {
                let _ = k;
                let declared = crate::resolve::declaration(hirs, signal)
                    .map(|d| d.name.clone())
                    .unwrap_or(name);
                out.push((declared, signal));
            }
        }
        for (_, view, _, _) in crate::signals::composed(sigs, def.unit, body) {
            todo.push(view);
        }
    }
    out
}

/// The signal the device keeps that `path` names where it is written in
/// `unit`, if it is one.
fn kept_named(
    sigs: &Signatures,
    kept: &BTreeMap<DefId, Kept>,
    unit: usize,
    path: &str,
) -> Option<DefId> {
    match sigs.workspace().resolve(unit, path) {
        Resolution::Local(d) | Resolution::Imported { def: d, .. } if kept.contains_key(&d) => {
            Some(d)
        }
        _ => None,
    }
}

/// The rules over one unit.
pub fn check(
    hirs: &[&Hir],
    unit: usize,
    sigs: &Signatures,
    summaries: &BTreeMap<DefId, Label>,
) -> Vec<Diagnostic> {
    let hir = hirs[unit];
    let kept = kept_signals(hirs);
    let mut out = Vec::new();
    for (id, decl) in hir.all_decls() {
        if decl.kind == DeclKind::Signal && !decl.policies.is_empty() {
            declaration(hirs, unit, id, decl, sigs, &kept, &mut out);
        }
    }
    if kept.is_empty() {
        return out;
    }
    let names: BTreeSet<&str> = kept
        .keys()
        .filter_map(|d| crate::resolve::declaration(hirs, *d))
        .map(|d| d.name.as_str())
        .collect();
    let imports = crate::labels::imported_modules(hir);
    for (id, decl) in hir.all_decls() {
        let Some(b) = decl.body else { continue };
        let body = hir.body(b);
        // Only a body that names one: no other value can carry its label,
        // since server code cannot read a signal (ADR-0130, R8).
        let names_one = body
            .walk()
            .into_iter()
            .any(|e| matches!(body.expr(e), Expr::Name(n) if names.contains(n.as_str())));
        if !names_one {
            continue;
        }
        let labels = crate::labels::Labels::of_decl(sigs, hir, id, body, &imports, Some(summaries));
        let cx = Flows {
            sigs,
            kept: &kept,
            unit,
            body,
            labels: &labels,
            lexical: crate::lexical::Lexical::build(decl, body),
            hirs,
        };
        cx.provides(&mut out);
        cx.assignments(&mut out);
        cx.sinks(&mut out);
        cx.markup(&mut out);
    }
    out
}

fn diagnostic(
    code: Code,
    reason: &'static str,
    (at, boundary): (Span, Related),
    message: String,
    explanation: &str,
    repair: &str,
) -> Diagnostic {
    Diagnostic {
        code: code.id,
        invariant: code.invariant,
        reason,
        detector: Detector::DeclarationRule,
        severity: Severity::Error,
        message,
        primary_span: at,
        related: vec![boundary],
        explanation: Some(explanation.to_string()),
        repairs: vec![Repair {
            description: repair.to_string(),
            replacement: None,
        }],
    }
}

const KEPT_EXPLAINED: &str = "A signal declared `privacy Device` belongs to this device and \
     never leaves it (charter §7.8, ADR-XXXX): the browser keeps it, reads it and renders \
     what it decides, and no server is told it. Its label follows every value made from it \
     (ADR-0129).";

/// **One signal's declaration**: its clauses (PW6805), its versions (PW6806)
/// and its type (PW6807).
fn declaration(
    hirs: &[&Hir],
    unit: usize,
    id: DeclId,
    decl: &Decl,
    sigs: &Signatures,
    kept: &BTreeMap<DefId, Kept>,
    out: &mut Vec<Diagnostic>,
) {
    let declared = || {
        Related::new(
            decl.name_span.clone(),
            format!(
                "`{}` is declared here, a signal the device keeps",
                decl.name
            ),
        )
    };
    let clauses = |at: Span, message: String, repair: &str| {
        diagnostic(
            codes::DEVICE_SIGNAL_CLAUSES,
            "device_signal_clauses",
            (at, declared()),
            message,
            "A module's signal with a value is one the device keeps (ADR-XXXX): its clauses \
             say where it rests and under what name, whose it is, and what two tabs' writes \
             do, so that none of the three is a default nobody chose. The first version keeps \
             a value on the device alone, the device's own, the whole value written last \
             being every tab's.",
            repair,
        )
    };
    let name = &decl.name;
    let at = decl.name_span.clone();
    // Where it rests, and its name.
    match decl.policy("storage") {
        None => out.push(clauses(
            at.clone(),
            format!("`{name}` has a value, and says nowhere to keep it"),
            "keep it on the device: `storage device \"name\"`, or give it no value and \
             `provide` it where a page or view gives it one",
        )),
        Some(p) => match crate::policy::storage(&p.value) {
            Some(("device", Some(written))) if !well_named(written) => out.push(clauses(
                p.span.clone(),
                format!(
                    "`{name}` is kept under `{written}`, and a name is of letters, digits, \
                     `-`, `_` and `.`, beginning with a letter"
                ),
                "name it as TodoMVC names its todos: `storage device \"todos-pleris\"`",
            )),
            Some(("device", Some(written))) => {
                // One name a program: the first declaration of a name keeps
                // it, and each other is told where it is.
                let me = DefId { unit, decl: id.0 };
                if let Some((first, _)) = kept
                    .iter()
                    .find(|(d, k)| k.name == written && **d < me && **d != me)
                {
                    let other = crate::resolve::declaration(hirs, *first)
                        .map(|d| d.name.clone())
                        .unwrap_or_default();
                    out.push(clauses(
                        p.span.clone(),
                        format!(
                            "`{name}` is kept under `{written}`, which `{other}` is kept under \
                             too: two values would overwrite each other"
                        ),
                        "give each value the device keeps a name of its own",
                    ));
                }
            }
            Some(("device", None)) => out.push(clauses(
                p.span.clone(),
                format!("`{name}` is kept on the device under no name"),
                "name what it is kept under: `storage device \"name\"`, which stays the same \
                 when the code is renamed",
            )),
            _ => out.push(clauses(
                p.span.clone(),
                format!("`{name}` is a module's signal kept at `{}`", p.value.trim()),
                "a signal is kept on the device, `storage device \"name\"`: the first version \
                 keeps no signal anywhere else",
            )),
        },
    }
    // Whose it is.
    match decl.policy("privacy").map(|p| (p, p.value.trim())) {
        None => out.push(clauses(
            at.clone(),
            format!("`{name}` is kept on the device, and does not say whose it is"),
            "say it is the device's own, and never leaves it: `privacy Device`",
        )),
        Some((_, "Device")) => {}
        Some((p, other)) => out.push(clauses(
            p.span.clone(),
            format!(
                "`{name}` is `privacy {other}`, and the first version keeps only the device's \
                 own: `privacy Device`"
            ),
            "write `privacy Device`; a value a user's, partitioned by user and cleared at \
             sign-out, is a later version's (ADR-0339's D1)",
        )),
    }
    // What two tabs' writes do.
    match decl.policy("conflict").map(|p| (p, p.value.trim())) {
        None => out.push(clauses(
            at.clone(),
            format!("`{name}` does not say what two tabs' writes of it do"),
            "state it: `conflict last_write_wins`, the whole value written last being every \
             tab's (no merge is silent, A-011)",
        )),
        Some((_, "last_write_wins")) => {}
        Some((p, other)) => out.push(clauses(
            p.span.clone(),
            format!(
                "`{name}`'s tabs are reconciled by `{other}`, and the first version offers \
                 `last_write_wins` alone"
            ),
            "write `conflict last_write_wins`",
        )),
    }
    // Its value: the default the device starts with.
    if decl.body.is_none() {
        out.push(clauses(
            at.clone(),
            format!("`{name}` is kept on the device with no default"),
            "write the value the device starts with: `signal name: Type = value`",
        ));
    }
    let Some(ty) = sigs
        .signal_type(DefId { unit, decl: id.0 })
        .and_then(|t| t.resolved())
        .cloned()
    else {
        return;
    };
    // Its type, and its versions'.
    if let Some(why) = no_device_form(sigs, &ty, &mut Vec::new()) {
        out.push(diagnostic(
            codes::DEVICE_SIGNAL_FORM,
            "device_signal_form",
            (at.clone(), declared()),
            format!(
                "`{name}` is a `{}`, which the device cannot hold: {why}",
                ty.display_name()
            ),
            "What the device keeps is written as data and read back through a decoder made \
             from its type (charter §7.10): every value read from the browser's storage is \
             checked before it is used. A function has no such form, a type that contains \
             itself crosses the browser's wire as its nodes, which the first version does not \
             store, and an opaque type's `where` would have to be run by the decoder.",
            "keep a record, a list, a case or a primitive of it",
        ));
    }
    versions(hirs, unit, decl, sigs, &ty, out);
}

/// **Why `ty` has no form the device can hold**, or `None` where it has one.
fn no_device_form(sigs: &Signatures, ty: &ResolvedType, within: &mut Vec<DefId>) -> Option<String> {
    if ty.as_primitive().is_some() || ty.parameter_binding().is_some() {
        return None;
    }
    if let Some(ctor) = ty.as_builtin() {
        return match ctor {
            Builtin::Function => Some("a function has no form as data".to_string()),
            Builtin::Map | Builtin::Set => Some(format!(
                "a `{}` is not kept by the first version",
                ty.display_name()
            )),
            Builtin::List | Builtin::Option | Builtin::Result => ty
                .args()
                .iter()
                .find_map(|a| no_device_form(sigs, a, within)),
        };
    }
    let def = ty.def_id()?;
    if within.contains(&def) {
        return Some(format!(
            "`{}` contains itself, and crosses the browser's wire as its nodes",
            ty.display_name()
        ));
    }
    let declared = sigs.type_decl(def)?;
    if declared.invariant.is_some() {
        return Some(format!(
            "`{}` holds a `where`, which a decoder of what the device kept would have to run",
            ty.display_name()
        ));
    }
    within.push(def);
    let mut found = ty
        .args()
        .iter()
        .find_map(|a| no_device_form(sigs, a, within));
    if found.is_none()
        && let Some(r) = declared.representation.as_ref().and_then(|r| r.resolved())
    {
        found = no_device_form(sigs, r, within);
    }
    if found.is_none() {
        for (_, field) in declared.record.iter().flatten() {
            if let Some(t) = field.resolved() {
                found = no_device_form(sigs, t, within);
            }
            if found.is_some() {
                break;
            }
        }
    }
    if found.is_none() {
        'cases: for (_, fields) in declared.variants.iter().flatten() {
            for field in fields {
                if let Some(t) = field.resolved() {
                    found = no_device_form(sigs, t, within);
                }
                if found.is_some() {
                    break 'cases;
                }
            }
        }
    }
    within.pop();
    found
}

/// **Its versions** (PW6806): a pure function for each one before its own,
/// of one parameter, chained by type to the declared one.
fn versions(
    hirs: &[&Hir],
    unit: usize,
    decl: &Decl,
    sigs: &Signatures,
    ty: &ResolvedType,
    out: &mut Vec<Diagnostic>,
) {
    let refuse = |at: Span, message: String, repair: &str| {
        diagnostic(
            codes::DEVICE_SIGNAL_VERSIONS,
            "device_signal_versions",
            (
                at,
                Related::new(
                    decl.name_span.clone(),
                    format!(
                        "`{}` is declared here, a signal the device keeps",
                        decl.name
                    ),
                ),
            ),
            message,
            "What a device keeps outlives the code that wrote it. Each version is kept under \
             a key of its own, and a page reading an earlier one takes it to its own version \
             through the functions its declaration names, in order, each from one version to \
             the next (ADR-XXXX, decision 4). A missing step, a step that performs something \
             or a step of the wrong type would leave a person's data unread.",
            repair,
        )
    };
    let name = &decl.name;
    let version = version(decl);
    let written = migrations(decl);
    let at = decl
        .policy("migrate")
        .or(decl.policy("version"))
        .map(|p| p.span.clone())
        .unwrap_or_else(|| decl.name_span.clone());
    if written.len() + 1 != version as usize {
        out.push(refuse(
            at.clone(),
            format!(
                "`{name}` is at version {version}, and names {} migration{} where it needs {}",
                written.len(),
                if written.len() == 1 { "" } else { "s" },
                version.saturating_sub(1)
            ),
            "name, after `migrate`, one function for each version before this one, the first \
             from version 1: `migrate from_1, from_2`",
        ));
        return;
    }
    // Each function, and what it takes and makes.
    let mut steps: Vec<(String, ResolvedType, ResolvedType)> = Vec::new();
    for f in &written {
        let def = match sigs.workspace().resolve(unit, f) {
            Resolution::Local(d) | Resolution::Imported { def: d, .. }
                if sigs.kind_of(d) == Some(DeclKind::Fn) =>
            {
                d
            }
            _ => {
                out.push(refuse(
                    at.clone(),
                    format!("`{name}`'s migration `{f}` is no function here"),
                    "name a function of the module, or one it imports",
                ));
                return;
            }
        };
        let Some(sig) = sigs.by_def(def) else { return };
        let pure = crate::resolve::declaration(hirs, def)
            .and_then(|d| d.declared_effects.as_ref())
            .is_some_and(|row| row.is_empty());
        if !pure {
            out.push(refuse(
                at.clone(),
                format!("`{name}`'s migration `{f}` does not say it performs nothing"),
                "write its row: `fn f(old: V1) -> V2 !{}`, a step computed from the value alone",
            ));
            return;
        }
        let (Some(param), Some(result)) = (
            match sig.params.as_slice() {
                [Some(p)] => p.resolved().cloned(),
                _ => None,
            },
            sig.result().cloned(),
        ) else {
            out.push(refuse(
                at.clone(),
                format!("`{name}`'s migration `{f}` does not take one value and give one"),
                "take the earlier version's value, alone, and give the next one's",
            ));
            return;
        };
        if let Some(why) = no_device_form(sigs, &param, &mut Vec::new()) {
            out.push(refuse(
                at.clone(),
                format!(
                    "`{name}`'s migration `{f}` takes a `{}`, which no device kept: {why}",
                    param.display_name()
                ),
                "take the type the earlier version was kept as",
            ));
            return;
        }
        steps.push((f.clone(), param, result));
    }
    for pair in steps.windows(2) {
        let (from, _, made) = &pair[0];
        let (to, takes, _) = &pair[1];
        if !made.same_as(takes) {
            out.push(refuse(
                at.clone(),
                format!(
                    "`{from}` makes a `{}`, and `{to}`, the next step, takes a `{}`",
                    made.display_name(),
                    takes.display_name()
                ),
                "make each step take what the one before it makes",
            ));
            return;
        }
    }
    if let Some((last, _, made)) = steps.last()
        && !made.same_as(ty)
    {
        out.push(refuse(
            at,
            format!(
                "`{last}`, `{name}`'s last migration, makes a `{}`, and `{name}` is a `{}`",
                made.display_name(),
                ty.display_name()
            ),
            "make the last step give the declared type",
        ));
    }
}

/// What one body's flows are read with.
struct Flows<'a> {
    sigs: &'a Signatures,
    kept: &'a BTreeMap<DefId, Kept>,
    unit: usize,
    body: &'a Body,
    labels: &'a crate::labels::Labels<'a>,
    lexical: crate::lexical::Lexical,
    hirs: &'a [&'a Hir],
}

impl Flows<'_> {
    fn device(&self, e: ExprId) -> bool {
        self.labels
            .label(self.body, e)
            .restrictions()
            .any(|r| *r == Restriction::Device)
    }

    /// **What brings the device's label into `e`**: the first signal the
    /// device keeps named in it, or else the first name holding a value of
    /// one (a page's signal it was assigned to, a binding made from it).
    fn blamed(&self, e: ExprId) -> (String, bool) {
        let names: Vec<(ExprId, &String)> = self
            .body
            .walk_from(e)
            .into_iter()
            .filter_map(|x| match self.body.expr(x) {
                Expr::Name(n) => Some((x, n)),
                _ => None,
            })
            .collect();
        if let Some((_, n)) = names
            .iter()
            .find(|(_, n)| kept_named(self.sigs, self.kept, self.unit, n).is_some())
        {
            return (format!("`{n}`"), true);
        }
        match names.iter().find(|(x, _)| self.device(*x)) {
            Some((_, n)) => (format!("`{n}`"), false),
            None => ("this value".to_string(), false),
        }
    }

    /// **Where `e` is the device's** (charter §16.3's boundary): the
    /// declaration of the signal the device keeps it names, or else where the
    /// binding holding one acquired it.
    fn origin(&self, e: ExprId) -> Related {
        for x in self.body.walk_from(e) {
            let Expr::Name(n) = self.body.expr(x) else {
                continue;
            };
            if let Some(def) = kept_named(self.sigs, self.kept, self.unit, n)
                && let Some(decl) = crate::resolve::declaration(self.hirs, def)
            {
                return Related::in_unit(
                    def.unit,
                    decl.name_span.clone(),
                    format!("`{n}` is kept on the device, `privacy Device`, here"),
                );
            }
        }
        for x in self.body.walk_from(e) {
            if let Expr::Name(n) = self.body.expr(x)
                && self.device(x)
                && let Some((_, at)) = self.labels.origin(x)
            {
                return Related::new(
                    at.clone(),
                    format!("`{n}` holds what the device keeps here"),
                );
            }
        }
        Related::new(
            self.body.expr_span(e),
            "this value holds what the device keeps",
        )
    }

    /// `e`'s blame as a sentence's subject.
    fn blame(&self, e: ExprId) -> String {
        match self.blamed(e) {
            (n, true) => format!("{n} is kept on this device"),
            (n, false) => format!("{n} holds what this device keeps"),
        }
    }

    /// `e`'s blame after a clause it ends.
    fn which(&self, e: ExprId) -> String {
        match self.blamed(e) {
            (n, true) => format!("{n}, which is kept on this device"),
            (n, false) => format!("{n}, which holds what this device keeps"),
        }
    }

    /// The signal the device keeps that `target`, an assignment's left
    /// side, names, where no binding of the body hides it.
    fn assigns_kept(&self, target: ExprId) -> Option<(DefId, String)> {
        let Expr::Name(n) = self.body.expr(target) else {
            return None;
        };
        if self.lexical.binder(target).is_some() {
            return None;
        }
        kept_named(self.sigs, self.kept, self.unit, n).map(|d| (d, n.clone()))
    }

    /// PW6808: no `provide` gives one a value.
    fn provides(&self, out: &mut Vec<Diagnostic>) {
        for &e in &self.body.provides {
            let Expr::Binary { lhs, .. } = self.body.expr(e) else {
                continue;
            };
            let Expr::Name(n) = self.body.expr(*lhs) else {
                continue;
            };
            if kept_named(self.sigs, self.kept, self.unit, n).is_none() {
                continue;
            }
            out.push(diagnostic(
                codes::DEVICE_SIGNAL_PROVIDED,
                "device_signal_provided",
                (self.body.expr_span(e), self.origin(*lhs)),
                format!("`{n}` is kept by the device, and this gives it a value of its own"),
                "A module's signal is provided where it lives (ADR-0144). One the device keeps \
                 lives on the device: every page and view that names it holds the device's \
                 value, and a `provide` would give a part of the page another one.",
                "read it as it is, and change it in a handler",
            ));
        }
    }

    /// PW6804: what a device keeps is its own, or public.
    fn assignments(&self, out: &mut Vec<Diagnostic>) {
        for e in self.body.walk() {
            let Expr::Binary {
                op: BinOp::Assign,
                lhs,
                rhs,
            } = self.body.expr(e)
            else {
                continue;
            };
            if self.body.provides.contains(&e) {
                continue;
            }
            let Some((_, name)) = self.assigns_kept(*lhs) else {
                continue;
            };
            let label = self.labels.label(self.body, *rhs);
            let foreign: Vec<String> = label
                .restrictions()
                .filter(|r| **r != Restriction::Device)
                .map(|r| r.to_string())
                .collect();
            if foreign.is_empty() {
                continue;
            }
            out.push(diagnostic(
                codes::DEVICE_KEEPS_ITS_OWN,
                "device_keeps_its_own",
                (self.body.expr_span(*rhs), self.origin(*lhs)),
                format!(
                    "`{name}` is kept on this device, and this value is {}",
                    foreign.join(" and ")
                ),
                "What the device keeps stays on it after the session it was read in has ended: \
                 on a shared computer, the next person's (ADR-XXXX, decision 5). A session's, a \
                 user's or an organization's value belongs to that reader, and a secret never \
                 leaves the origin.",
                "keep the device's own, or public data; a user's value kept on the device, \
                 partitioned and cleared at sign-out, is a later version's",
            ));
        }
    }

    /// PW6801, PW6802 and PW6803's navigation: what a handler or a page's
    /// read sends.
    fn sinks(&self, out: &mut Vec<Diagnostic>) {
        for e in self.body.walk() {
            match self.body.expr(e) {
                Expr::Call { callee, args } if self.calls_a_command(*callee) => {
                    let command = crate::infer::path_of(self.body, *callee);
                    let mut told = false;
                    for a in args {
                        if self.device(a.value) {
                            told = true;
                            out.push(diagnostic(
                                codes::DEVICE_VALUE_TO_A_COMMAND,
                                "device_value_to_a_command",
                                (self.body.expr_span(a.value), self.origin(a.value)),
                                format!(
                                    "{}, and `{command}`'s argument is \
                                     sent to the server",
                                    self.blame(a.value)
                                ),
                                KEPT_EXPLAINED,
                                "give the command what the server may know, or change the \
                                 device's value in the handler instead of a command",
                            ));
                        }
                    }
                    if told {
                        continue;
                    }
                    if let Some((label, cause)) = self.labels.condition(e)
                        && label.restrictions().any(|r| *r == Restriction::Device)
                    {
                        out.push(diagnostic(
                            codes::DEVICE_VALUE_TO_A_COMMAND,
                            "device_value_to_a_command",
                            (self.body.expr_span(*callee), self.origin(*cause)),
                            format!(
                                "whether `{command}` is sent is decided by {}",
                                self.which(*cause)
                            ),
                            KEPT_EXPLAINED,
                            "decide the call on what the server may know",
                        ));
                    }
                }
                Expr::Keyword { keyword, args, .. } if keyword == "query" => {
                    for &a in args {
                        if self.device(a) {
                            out.push(diagnostic(
                                codes::DEVICE_VALUE_KEYS_A_QUERY,
                                "device_value_keys_a_query",
                                (self.body.expr_span(a), self.origin(a)),
                                format!(
                                    "{}, and a query's key is sent to the \
                                     server to read it again",
                                    self.blame(a)
                                ),
                                KEPT_EXPLAINED,
                                "compute what the page shows of it in the browser, from the \
                                 signal itself",
                            ));
                        }
                    }
                }
                Expr::Keyword { keyword, args, .. } if keyword == "navigate" => {
                    for &a in args {
                        if self.device(a) {
                            out.push(diagnostic(
                                codes::DEVICE_VALUE_SENT,
                                "device_value_sent",
                                (self.body.expr_span(a), self.origin(a)),
                                format!(
                                    "{}, and a navigation's arguments \
                                     are the address the server is asked for",
                                    self.blame(a)
                                ),
                                KEPT_EXPLAINED,
                                "navigate by what the server may know",
                            ));
                        }
                    }
                }
                _ => {}
            }
        }
    }

    /// Whether `callee` names a command.
    fn calls_a_command(&self, callee: ExprId) -> bool {
        let path = crate::infer::path_of(self.body, callee);
        if path.is_empty() {
            return false;
        }
        match self.sigs.workspace().resolve(self.unit, &path) {
            Resolution::Local(d) | Resolution::Imported { def: d, .. } => {
                self.sigs.kind_of(d) == Some(DeclKind::Command)
            }
            _ => false,
        }
    }

    /// PW6803's markup: an address, a style, and a form's control.
    fn markup(&self, out: &mut Vec<Diagnostic>) {
        for e in self.body.walk() {
            let Expr::Template { roots, .. } = self.body.expr(e) else {
                continue;
            };
            for &root in roots {
                self.element(root, false, out);
            }
        }
    }

    fn element(&self, node: crate::hir::NodeId, in_form: bool, out: &mut Vec<Diagnostic>) {
        match self.body.node(node) {
            Node::Element {
                tag,
                attrs,
                children,
                ..
            } => {
                let control = matches!(
                    tag.as_str(),
                    "input" | "select" | "textarea" | "button" | "option"
                );
                for attr in attrs {
                    let AttrValue::Expr(value) = attr.value else {
                        continue;
                    };
                    if attr.event().is_some() {
                        continue;
                    }
                    let context = crate::template_ir::Context::of_attribute(&attr.name);
                    let bare = attr.name.rsplit(':').next().unwrap_or(&attr.name);
                    let why = match context {
                        crate::template_ir::Context::Url => Some(format!(
                            "`{bare}` is an address, which the browser sends to whoever serves it"
                        )),
                        crate::template_ir::Context::Style => Some(
                            "a style may hold an address, which the browser fetches".to_string(),
                        ),
                        _ if in_form
                            && control
                            && matches!(bare, "value" | "name" | "checked" | "selected") =>
                        {
                            Some(format!(
                                "`<{tag}>` is inside a `<form>`, which sends its controls' values"
                            ))
                        }
                        _ => None,
                    };
                    let Some(why) = why else { continue };
                    if !self.device(value) {
                        continue;
                    }
                    out.push(diagnostic(
                        codes::DEVICE_VALUE_SENT,
                        "device_value_sent",
                        (attr.span.clone(), self.origin(value)),
                        format!("{}, and {why}", self.blame(value)),
                        KEPT_EXPLAINED,
                        "show it as text, which the browser keeps, or outside the form",
                    ));
                }
                let in_form = in_form || tag == "form";
                for &c in children {
                    self.element(c, in_form, out);
                }
            }
            Node::Block { children, .. } => {
                for &c in children {
                    self.element(c, in_form, out);
                }
            }
            _ => {}
        }
    }
}

/// **What a value the device keeps is, as its decoder reads it** (ADR-XXXX,
/// decision 3): the browser's wire form of its type (`js_pure`), checked
/// field by field before the value is used. An opaque type is its
/// representation, and a case its wire name with its payload: none, one as
/// `value`, several as `value`'s array.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(tag = "t", rename_all = "snake_case")]
pub enum Schema {
    Int,
    Float,
    Bool,
    String,
    Unit,
    List { of: Box<Schema> },
    Option { of: Box<Schema> },
    Result { ok: Box<Schema>, err: Box<Schema> },
    Record { fields: Vec<(String, Schema)> },
    Cases { cases: Vec<(String, Vec<Schema>)> },
}

/// **A value the device keeps, as a page holds it** (ADR-XXXX): what the
/// page's plan and its document carry for the runtime.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct Held {
    /// The signal it is held as, by the name the page's template reads it.
    pub signal: String,
    /// The name the program keeps it under: its key at version 1.
    pub name: String,
    pub version: u32,
    pub conflict: String,
    /// Each version's form, from version 1 to its own.
    pub schemas: Vec<Schema>,
}

/// **The form of `ty`**, its type parameters given `args`, or why it has
/// none (PW6807's refusal, met here only in a program that did not check).
pub fn schema(sigs: &Signatures, ty: &ResolvedType, args: &[Schema]) -> Result<Schema, String> {
    if let Some(p) = ty.as_primitive() {
        return Ok(match p {
            crate::resolved::Primitive::Int => Schema::Int,
            crate::resolved::Primitive::Float => Schema::Float,
            crate::resolved::Primitive::Bool => Schema::Bool,
            crate::resolved::Primitive::Str => Schema::String,
            crate::resolved::Primitive::Unit => Schema::Unit,
        });
    }
    if let Some((_, index)) = ty.parameter_binding() {
        return args
            .get(index as usize)
            .cloned()
            .ok_or_else(|| format!("`{}` is given no type", ty.display_name()));
    }
    let of = |i: usize| -> Result<Box<Schema>, String> {
        let a = ty
            .args()
            .get(i)
            .ok_or_else(|| format!("`{}` has too few arguments", ty.display_name()))?;
        Ok(Box::new(schema(sigs, a, args)?))
    };
    if let Some(ctor) = ty.as_builtin() {
        return match ctor {
            Builtin::List => Ok(Schema::List { of: of(0)? }),
            Builtin::Option => Ok(Schema::Option { of: of(0)? }),
            Builtin::Result => Ok(Schema::Result {
                ok: of(0)?,
                err: of(1)?,
            }),
            _ => Err(format!("a `{}` has no device form", ty.display_name())),
        };
    }
    let def = ty
        .def_id()
        .ok_or_else(|| format!("`{}` names no type", ty.display_name()))?;
    let declared = sigs
        .type_decl(def)
        .ok_or_else(|| format!("`{}` declares no type", ty.display_name()))?;
    let given: Vec<Schema> = ty
        .args()
        .iter()
        .map(|a| schema(sigs, a, args))
        .collect::<Result<_, _>>()?;
    let field = |t: &crate::resolved::TypeResolution| -> Result<Schema, String> {
        let t = t
            .resolved()
            .ok_or_else(|| format!("a field of `{}` resolves to no type", ty.display_name()))?;
        schema(sigs, t, &given)
    };
    if let Some(r) = &declared.representation {
        return field(r);
    }
    if let Some(fields) = &declared.record {
        return Ok(Schema::Record {
            fields: fields
                .iter()
                .map(|(n, t)| Ok((n.clone(), field(t)?)))
                .collect::<Result<_, String>>()?,
        });
    }
    if let Some(cases) = &declared.variants {
        return Ok(Schema::Cases {
            cases: cases
                .iter()
                .map(|(n, fields)| {
                    Ok((
                        crate::wit::ident(n),
                        fields.iter().map(field).collect::<Result<_, String>>()?,
                    ))
                })
                .collect::<Result<_, String>>()?,
        });
    }
    Err(format!("`{}` has no device form", ty.display_name()))
}

/// **Each value the device keeps that a page holds**, as its plan carries
/// it, or why one cannot be: a type with no form, a version with no step.
pub fn held(
    hirs: &[&Hir],
    ws: &crate::resolve::Workspace,
    sigs: &Signatures,
    unit: usize,
    page: DeclId,
) -> Result<Vec<Held>, String> {
    let mut out = Vec::new();
    for (signal, def) in held_by(hirs, ws, sigs, unit, page) {
        let Some(decl) = crate::resolve::declaration(hirs, def) else {
            continue;
        };
        let Some(kept) = Kept::of(decl) else { continue };
        let ty = sigs
            .signal_type(def)
            .and_then(|t| t.resolved())
            .ok_or_else(|| format!("`{signal}`'s type resolves to nothing"))?;
        let mut schemas = Vec::new();
        for step in &kept.migrations {
            let from = match sigs.workspace().resolve(def.unit, step) {
                Resolution::Local(d) | Resolution::Imported { def: d, .. } => sigs
                    .by_def(d)
                    .and_then(|s| s.params.first().cloned().flatten())
                    .and_then(|p| p.resolved().cloned()),
                _ => None,
            }
            .ok_or_else(|| format!("`{signal}`'s migration `{step}` takes no value"))?;
            schemas.push(schema(sigs, &from, &[]).map_err(|why| format!("`{signal}`: {why}"))?);
        }
        schemas.push(schema(sigs, ty, &[]).map_err(|why| format!("`{signal}`: {why}"))?);
        out.push(Held {
            signal,
            name: kept.name,
            version: kept.version,
            conflict: decl
                .policy("conflict")
                .map(|p| p.value.trim().to_string())
                .unwrap_or_default(),
            schemas,
        });
    }
    Ok(out)
}
