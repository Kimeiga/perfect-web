//! **A page binds a signal to its address's fragment** (track `todomvc`,
//! ADR-0367; the integrator's ruling of 2026-10-14): `#/active` reaches no
//! server, so a page reads its fragment, and what it reads is a signal.
//!
//! ```text
//! session page Todos() {
//!     route    "/"
//!     fragment filter "/" => "all", "/active" => "active", "/completed" => "completed"
//!     signal filter: String = "all"
//!     ..
//! ```
//!
//! The clause names a signal of the page and each form the fragment takes,
//! with the value the signal takes then. The runtime keeps the signal in
//! step with `location.hash`, at start and on each `hashchange`; a fragment
//! that is no form, or none, is the signal's first value, the page's
//! declared default, which a server renders, since it never sees a fragment.
//! The address is the signal's only writer: no handler assigns it, and a
//! link to a fragment the page does not take names a form it does.
//!
//! - **PW6101**: the clause names a signal the page declares.
//! - **PW6102**: the signal is a `String`: a fragment is text, and a signal
//!   of another type needs a codec, which is not built (ADR-0131).
//! - **PW6103**: the forms are written as `"form" => "value"` pairs, each
//!   form once, of the characters a fragment holds.
//! - **PW6104**: the signal starts at a value one of the forms gives, so
//!   the default is a page the address can reach.
//! - **PW6105**: no handler assigns the signal.
//! - **PW6106**: a link to a fragment of such a page names a form it takes.

use std::collections::BTreeSet;

use crate::codes;
use crate::diagnostics::{Detector, Diagnostic, Related, Repair, Severity};
use crate::hir::{AttrValue, BinOp, Body, Decl, DeclKind, Expr, ExprId, Hir, Node, Pattern};
use crate::lexical::{Binder, Lexical};

/// One form the fragment takes, and the value the signal takes with it:
/// the text after `#`, as the browser reports it in `location.hash`.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct Form {
    pub form: String,
    pub value: String,
}

/// **A signal bound to the fragment**, as the page's plan carries it to the
/// browser.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct Binding {
    pub signal: String,
    pub forms: Vec<Form>,
}

/// Why a clause did not read, as a reason and a message.
type Unread = (&'static str, String);

/// **What a form may be made of**: a fragment's characters (RFC 3986 §3.5,
/// `pchar / "/" / "?"`) less `%`, so a form is exactly what `location.hash`
/// reports, which a browser never writes in another form: unreserved, the
/// sub-delimiters, `:`, `@`, `/` and `?`. Empty is the address with no
/// fragment.
fn form_char(c: char) -> bool {
    c.is_ascii_alphanumeric() || "-._~!$&'()*+,;=:@/?".contains(c)
}

/// A quoted text of the clause, at the start of `rest`, and what follows it.
fn quoted(rest: &str) -> Result<(&str, &str), Unread> {
    let Some(inner) = rest.strip_prefix('"') else {
        return Err((
            "fragment_form_not_text",
            format!("expected a quoted text, found `{}`", brief(rest)),
        ));
    };
    let Some(end) = inner.find('"') else {
        return Err((
            "fragment_form_not_text",
            "a quoted text is not closed".to_string(),
        ));
    };
    let text = &inner[..end];
    if text.contains('\\') {
        return Err((
            "fragment_form_not_text",
            format!("`{text}` holds an escape, and a form is written as it is"),
        ));
    }
    Ok((text, inner[end + 1..].trim_start()))
}

/// The start of what is left, for a message.
fn brief(rest: &str) -> String {
    rest.chars().take(16).collect()
}

/// **The clause's value, read**: the signal's name, then each form and the
/// value it gives, `"/active" => "active"`, by commas.
pub(crate) fn read(clause: &str) -> Result<Binding, Unread> {
    let clause = clause.trim();
    let end = clause
        .find(|c: char| !(c.is_ascii_alphanumeric() || c == '_'))
        .unwrap_or(clause.len());
    let (signal, mut rest) = clause.split_at(end);
    if signal.is_empty() {
        return Err((
            "fragment_names_no_signal",
            "`fragment` names no signal: write the signal's name first".to_string(),
        ));
    }
    rest = rest.trim_start();
    let mut forms: Vec<Form> = Vec::new();
    loop {
        let (form, after) = quoted(rest)?;
        if let Some(c) = form.chars().find(|c| !form_char(*c)) {
            return Err((
                "fragment_form_character",
                format!("the form `{form}` holds `{c}`, which a fragment does not hold as it is"),
            ));
        }
        let Some(after) = after.strip_prefix("=>") else {
            return Err((
                "fragment_form_not_pair",
                format!("the form `{form}` gives no value: write `\"{form}\" => \"value\"`"),
            ));
        };
        let (value, after) = quoted(after.trim_start())?;
        if forms.iter().any(|f| f.form == form) {
            return Err((
                "fragment_form_twice",
                format!("the form `{form}` is written twice, and a fragment is one form"),
            ));
        }
        forms.push(Form {
            form: form.to_string(),
            value: value.to_string(),
        });
        match after.strip_prefix(',') {
            Some(more) if !more.trim().is_empty() => rest = more.trim_start(),
            Some(_) => {
                return Err((
                    "fragment_form_not_pair",
                    "a comma ends the forms with none after it".to_string(),
                ));
            }
            None if after.is_empty() => break,
            None => {
                return Err((
                    "fragment_form_not_pair",
                    format!(
                        "expected `,` or the end of the clause, found `{}`",
                        brief(after)
                    ),
                ));
            }
        }
    }
    Ok(Binding {
        signal: signal.to_string(),
        forms,
    })
}

/// A page's signals: each one's name, its declaration, and its body's
/// expression for it.
fn signals_of(body: &Body) -> Vec<(String, ExprId)> {
    body.walk()
        .into_iter()
        .filter(|id| body.signals.contains(id))
        .filter_map(|id| match body.expr(id) {
            Expr::Let { pat: Some(p), .. } => match body.pat(*p) {
                Pattern::Bind { name, .. } => Some((name.clone(), id)),
                _ => None,
            },
            _ => None,
        })
        .collect()
}

/// **The binding a page states**, where its clause reads: what the plan
/// carries to the browser. A clause that does not read is PW6101's or
/// PW6103's, and binds nothing.
pub fn declared(decl: &Decl) -> Option<Binding> {
    if decl.kind != DeclKind::Page {
        return None;
    }
    read(&decl.policy("fragment")?.value).ok()
}

/// **A signal's first value**, where it is written as a text.
fn first_value(body: &Body, signal: ExprId) -> Option<String> {
    let Expr::Let {
        init: Some(init), ..
    } = body.expr(signal)
    else {
        return None;
    };
    body.string_text(*init)
        .map(|s| s.trim_matches('"').to_string())
}

/// **A signal's type, as written**: `String`, `Int`, `List<..>`.
fn written_type(body: &Body, signal: ExprId) -> String {
    match body.expr(signal) {
        Expr::Let { ty: Some(t), .. } => body
            .types
            .get(t.index())
            .map(|r| match r.args.is_empty() {
                true => r.path.clone(),
                false => format!("{}<..>", r.path),
            })
            .unwrap_or_default(),
        _ => String::new(),
    }
}

fn diagnostic(
    code: codes::Code,
    reason: &'static str,
    message: String,
    span: crate::hir::Span,
    related: Vec<Related>,
    explanation: &str,
    repair: String,
) -> Diagnostic {
    Diagnostic {
        code: code.id,
        invariant: code.invariant,
        reason,
        detector: Detector::DeclarationRule,
        severity: Severity::Error,
        message,
        primary_span: span,
        related,
        explanation: Some(explanation.to_string()),
        repairs: vec![Repair {
            description: repair,
            replacement: None,
        }],
    }
}

const BINDS_WHY: &str = "A page's address has a fragment, which no server sees, and which \
     changes as a reader follows `#/active`. A page that shows by it binds one signal to it, \
     and the runtime keeps the two in step: so which signal, and each form the fragment \
     takes, are the page's to say.";

/// **What `unit` writes wrong of a fragment**: each page's clause
/// (PW6101 to PW6104), the handlers that assign its signal (PW6105), and
/// the links to a fragment it does not take (PW6106).
pub fn check(hirs: &[&Hir], unit: usize) -> Vec<Diagnostic> {
    let hir = hirs[unit];
    let mut out = Vec::new();
    for (id, decl) in hir.all_decls() {
        if decl.kind != DeclKind::Page {
            continue;
        }
        // A clause written twice is the generic rule's (a policy is stated
        // once), and the first is the page's.
        let Some(first) = decl.policy("fragment") else {
            continue;
        };
        let at = hir.decl_span(id);
        let page = || Related::new(at.clone(), format!("the page `{}`", decl.name));
        let binding = match read(&first.value) {
            Ok(binding) => binding,
            Err((reason, message)) => {
                let code = if reason == "fragment_names_no_signal" {
                    codes::FRAGMENT_NAMES_NO_SIGNAL
                } else {
                    codes::FRAGMENT_FORMS
                };
                out.push(diagnostic(
                    code,
                    reason,
                    format!("`fragment {}`: {message}", first.value.trim()),
                    first.span.clone(),
                    vec![page()],
                    BINDS_WHY,
                    "write `fragment signal \"/\" => \"value\", \"/other\" => \"value\"`"
                        .to_string(),
                ));
                continue;
            }
        };
        let Some(body_id) = decl.body else { continue };
        let body = hir.body(body_id);
        let signals = signals_of(body);
        let Some((_, signal)) = signals.iter().find(|(n, _)| *n == binding.signal) else {
            let have: Vec<String> = signals.iter().map(|(n, _)| format!("`{n}`")).collect();
            out.push(diagnostic(
                codes::FRAGMENT_NAMES_NO_SIGNAL,
                "fragment_names_no_signal",
                format!(
                    "`fragment` binds `{}`, which `{}` does not declare as a signal{}",
                    binding.signal,
                    decl.name,
                    if have.is_empty() {
                        ", and it declares none".to_string()
                    } else {
                        format!(": its signals are {}", have.join(", "))
                    }
                ),
                first.span.clone(),
                vec![page()],
                BINDS_WHY,
                format!(
                    "declare `signal {}: String = \"..\"` in the page",
                    binding.signal
                ),
            ));
            continue;
        };
        let written = written_type(body, *signal);
        if written != "String" {
            out.push(diagnostic(
                codes::FRAGMENT_SIGNAL_NOT_TEXT,
                "fragment_signal_not_text",
                format!(
                    "`fragment` binds `{}`, a signal of `{written}`: a fragment is text, and a \
                     signal of another type needs a codec, which is not built (ADR-0131)",
                    binding.signal
                ),
                body.expr_span(*signal),
                vec![Related::new(
                    first.span.clone(),
                    "the fragment's clause".to_string(),
                )],
                "The runtime sets the signal to the value a form gives, as text; a signal \
                 of another type would be set to a value of the wrong type.",
                format!("declare `{}` a `String`", binding.signal),
            ));
            continue;
        }
        match first_value(body, *signal) {
            Some(start) if binding.forms.iter().any(|f| f.value == start) => {}
            start => out.push(diagnostic(
                codes::FRAGMENT_DEFAULT,
                "fragment_default_unreachable",
                match &start {
                    Some(start) => format!(
                        "`{}` starts at `{start}`, which no form of the fragment gives",
                        binding.signal
                    ),
                    None => format!(
                        "`{}` starts at a value that is not a text written there: the \
                         fragment's default is a literal",
                        binding.signal
                    ),
                },
                body.expr_span(*signal),
                vec![Related::new(
                    first.span.clone(),
                    "the fragment's clause".to_string(),
                )],
                "A server renders the signal at its first value, since it never sees a \
                 fragment, and the browser sets it to the one its address gives. A first \
                 value no form gives is a page the address could never be at.",
                format!(
                    "start `{}` at one of {}",
                    binding.signal,
                    binding
                        .forms
                        .iter()
                        .map(|f| format!("\"{}\"", f.value))
                        .collect::<BTreeSet<_>>()
                        .into_iter()
                        .collect::<Vec<_>>()
                        .join(", ")
                ),
            )),
        }
        assigned(decl, body, *signal, &binding, first.span.clone(), &mut out);
        links(body, &binding, &mut out);
    }
    out
}

/// **No handler assigns the bound signal** (PW6105): the address is its only
/// writer, so the signal and `location.hash` cannot differ.
fn assigned(
    decl: &Decl,
    body: &Body,
    signal: ExprId,
    binding: &Binding,
    clause: crate::hir::Span,
    out: &mut Vec<Diagnostic>,
) {
    let lexical = Lexical::build(decl, body);
    let Expr::Let { pat: Some(p), .. } = body.expr(signal) else {
        return;
    };
    let mine = Binder::Pattern(*p);
    for id in body.walk() {
        let Expr::Binary {
            op: BinOp::Assign,
            lhs,
            ..
        } = body.expr(id)
        else {
            continue;
        };
        if lexical.binder(*lhs) != Some(mine) {
            continue;
        }
        out.push(diagnostic(
            codes::FRAGMENT_SIGNAL_ASSIGNED,
            "fragment_signal_assigned",
            format!(
                "`{}` is bound to the address's fragment, and a handler assigns it",
                binding.signal
            ),
            body.expr_span(id),
            vec![Related::new(
                clause.clone(),
                format!("`{}` binds it", decl.name),
            )],
            "The address is the signal's only writer: the runtime sets it from \
             `location.hash`, so an assignment would leave the signal and the address \
             saying two things until the next change of either.",
            "follow a link to the form, `<a href=\"#/active\">`, and let the address set the \
             signal"
                .to_string(),
        ));
    }
}

/// **A link to a fragment names a form the page takes** (PW6106): a page that
/// binds its fragment to a signal owns it, and a link to any other would set
/// the signal back to its first value.
fn links(body: &Body, binding: &Binding, out: &mut Vec<Diagnostic>) {
    let mut roots = Vec::new();
    for e in body.walk() {
        if let Expr::Template { roots: r, .. } = body.expr(e) {
            roots.extend(r.iter().copied());
        }
    }
    for n in body.walk_markup(&roots) {
        let Node::Element { tag, attrs, .. } = body.node(n) else {
            continue;
        };
        if tag != "a" {
            continue;
        }
        let Some(href) = attrs.iter().find(|a| a.name == "href") else {
            continue;
        };
        let AttrValue::Static(raw) = &href.value else {
            continue;
        };
        let target = raw.trim_matches(|c| c == '"' || c == '\'');
        let Some(fragment) = target.strip_prefix('#') else {
            continue;
        };
        if binding.forms.iter().any(|f| f.form == fragment) {
            continue;
        }
        out.push(diagnostic(
            codes::LINK_NAMES_NO_FORM,
            "link_names_no_form",
            format!(
                "`href=\"#{fragment}\"` is a fragment the page does not take: it takes {}",
                binding
                    .forms
                    .iter()
                    .map(|f| format!("`#{}`", f.form))
                    .collect::<Vec<_>>()
                    .join(", ")
            ),
            href.span.clone(),
            Vec::new(),
            "A page that binds its fragment to a signal owns the fragment: a reader who \
             follows a link to one it does not take is at the signal's first value, as if \
             they had followed the first form.",
            "link to a form the clause lists, or to a page".to_string(),
        ));
    }
}
