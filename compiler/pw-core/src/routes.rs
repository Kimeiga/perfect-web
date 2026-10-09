//! Charter §8.2 — internal links are checked against the route table.
//!
//! A broken internal link is statically detectable, so it should never reach
//! production. The route table is what the program declares: every `page` with
//! a `route "/…"` clause. Nothing is inferred from a file's name or a module's
//! path, because a link is dead relative to what the program *declares*, and a
//! convention would make the check agree with itself rather than with the app.
//!
//! Only internal links are checked. `href="https://…"`, `href="#section"` and
//! `href="mailto:…"` name something this program does not own.

use std::collections::BTreeSet;

use crate::codes;
use crate::diagnostics::{Detector, Diagnostic, Related, Repair, Severity};
use crate::hir::{AttrValue, Expr, Hir, Node};

/// **The relying party's routes** (ADR-0258, ADR-0265): Pleris's own, which
/// every deployment's host serves beside the program's pages, whatever its
/// provider. The development server's identity answers exactly these, and a
/// test there holds it to this list.
pub const RELYING_PARTY: &[(&str, &str)] = &[
    ("GET", "/sign-in"),
    ("GET", "/sign-up"),
    ("GET", "/sign-in/callback"),
    ("POST", "/sign-out"),
];

/// **What a request may reach** (ADR-0265): each page's route, which a link
/// is checked against, and every route a request may reach with the method
/// it answers: a page's, a GET; the relying party's; and each upload's, its
/// route a POST, and a lease shown under it and what it serves, GETs.
#[derive(Debug, Default, Clone)]
pub struct Table {
    pub pages: BTreeSet<String>,
    pub answered: BTreeSet<(&'static str, String)>,
}

impl Table {
    /// Does a route answering `method`, in capitals, match `target`?
    fn answers(&self, method: &str, target: &str) -> bool {
        self.answered
            .iter()
            .any(|(m, r)| *m == method && matches(r, target))
    }
}

/// Every route the program declares, and what each answers.
pub fn table(hirs: &[&Hir]) -> Table {
    let mut pages = BTreeSet::new();
    for hir in hirs {
        for (_, decl) in hir.all_decls() {
            // An upload's route is where a form posts, not a page a link
            // reaches (track `uploads`).
            if decl.kind == crate::hir::DeclKind::Upload {
                continue;
            }
            pages.extend(declared_route(hir, decl));
        }
    }
    let mut answered: BTreeSet<(&'static str, String)> =
        pages.iter().map(|r| ("GET", r.clone())).collect();
    answered.extend(RELYING_PARTY.iter().map(|(m, r)| (*m, r.to_string())));
    for u in crate::uploads::upload_clauses(hirs) {
        answered.insert(("POST", u.route.clone()));
        answered.insert(("GET", format!("{}/{{lease}}", u.route)));
        answered.insert(("GET", format!("{}/{{key}}", u.serves)));
    }
    Table { pages, answered }
}

/// **The route a declaration declares**, as written: `/stores/{id}`. What
/// the link check matches against, and where a host serves the page
/// (ADR-0160), so the two cannot read a route two ways.
///
/// **The policy first, the body scan second** — the same order
/// `declared_world` and `declared_cache` use, and for the same reason.
/// `route "/stores/{id}"` was a bare `Name` pair in a page's executable body
/// until 2026-08-11, when UI declarations began parsing their policies inside
/// their braces (architect ruling, policy values leave the executable body
/// tree). This reader was the one that still looked only in the body, and
/// `R-023` lost its catch: with no route table, every link is checked
/// against nothing.
pub(crate) fn declared_route(hir: &Hir, decl: &crate::hir::Decl) -> Option<String> {
    if let Some(p) = decl.policy("route") {
        let v = p.value.trim();
        if !v.is_empty() {
            return Some(v.trim_matches('"').to_string());
        }
    }
    let body = hir.body(decl.body?);
    let Expr::Block { stmts } = body.expr(body.root) else {
        return None;
    };
    let mut it = stmts.iter().peekable();
    while let Some(s) = it.next() {
        if !matches!(body.expr(*s), Expr::Name(n) if n == "route") {
            continue;
        }
        let Some(next) = it.peek() else { continue };
        if let Some(path) = body.string_text(**next) {
            return Some(path.trim_matches('"').to_string());
        }
    }
    None
}

pub fn check(hir: &Hir, table: &Table, out: &mut Vec<Diagnostic>) {
    // With no declared pages there is nothing to check against: a program
    // that declares none is a fragment of one, whose links reach pages it does
    // not declare, and reporting every link would be reporting that the
    // feature is unused.
    if table.pages.is_empty() {
        return;
    }

    for (id, decl) in hir.all_decls() {
        let Some(body_id) = decl.body else { continue };
        let body = hir.body(body_id);
        let at = hir.decl_span(id);
        let mut roots = Vec::new();
        for e in body.walk() {
            if let Expr::Template { roots: r, .. } = body.expr(e) {
                roots.extend(r.iter().copied());
            }
        }
        for n in body.walk_markup(&roots) {
            let Node::Element {
                tag,
                attrs,
                children,
                ..
            } = body.node(n)
            else {
                continue;
            };
            if tag == "form" {
                form(body, decl, at.clone(), attrs, children, table, out);
                continue;
            }
            if tag != "a" {
                continue;
            }
            for a in attrs {
                if a.name != "href" {
                    continue;
                }
                // `href="/stores/{id}"` is a string with holes since ADR-0042,
                // and its text as written is what matches a route pattern.
                let raw = match &a.value {
                    AttrValue::Static(raw) => raw,
                    AttrValue::Expr(e) => match body.expr(*e) {
                        Expr::Interpolated { text, .. } => text,
                        _ => continue,
                    },
                    AttrValue::None => continue,
                };
                let target = raw.trim_matches('"');
                // A link is a GET: a page, or a route the platform or an
                // upload answers so (ADR-0265).
                if !is_internal(target) || table.answers("GET", target) {
                    continue;
                }
                out.push(Diagnostic {
                    code: codes::DEAD_INTERNAL_LINK.id,
                    invariant: codes::DEAD_INTERNAL_LINK.invariant,
                    reason: "link_matches_no_declared_route",
                    detector: Detector::PatternMatrix,
                    severity: Severity::Error,
                    message: format!("internal link target `{target}` matches no declared route"),
                    primary_span: a.span.clone(),
                    related: vec![Related {
                        span: at.clone(),
                        label: format!("`{}` renders the link", decl.name),
                    }],
                    explanation: Some(format!(
                        "Internal links are checked against the route table, and this \
                         program declares {}. A link that matches none of them is \
                         broken at the moment it is written rather than the moment \
                         somebody clicks it.",
                        table
                            .pages
                            .iter()
                            .map(|r| format!("`{r}`"))
                            .collect::<Vec<_>>()
                            .join(", ")
                    )),
                    repairs: vec![Repair {
                        description: format!(
                            "declare a page with `route \"{target}\"`, or link to a \
                             route that exists"
                        ),
                        replacement: None,
                    }],
                });
            }
        }
    }
}

/// **A route and its page's parameters agree** (ADR-0160). A route is `/`
/// and segments, each a word or a `{parameter}`; it names each of the page's
/// parameters once and nothing else, so the address gives every one; and
/// each is text, what a segment carries (PW0340, PW0621).
/// **A form goes where something answers it** (ADR-0265): its `action`, an
/// internal route, answers its `method`, `get` where it states none. A
/// form that sends a file, states an `enctype` or posts to an upload's route
/// is PW5603's (track `uploads`), and one with no `action` submits to its
/// own page or its handler. Until ADR-0265 no form but a file's was checked,
/// and a link to `/sign-in` was PW5009's, its route the host's.
fn form(
    body: &crate::hir::Body,
    decl: &crate::hir::Decl,
    at: crate::hir::Span,
    attrs: &[crate::hir::Attr],
    children: &[crate::hir::NodeId],
    table: &Table,
    out: &mut Vec<Diagnostic>,
) {
    let value = |name: &str| {
        attrs
            .iter()
            .find(|a| a.name == name)
            .and_then(|a| match &a.value {
                AttrValue::Static(raw) => Some((raw.trim_matches('"').to_string(), a.span.clone())),
                AttrValue::Expr(e) => match body.expr(*e) {
                    Expr::Interpolated { text, .. } => {
                        Some((text.trim_matches('"').to_string(), a.span.clone()))
                    }
                    _ => None,
                },
                AttrValue::None => None,
            })
    };
    let Some((target, span)) = value("action") else {
        return;
    };
    let sends_file = body.walk_markup(children).into_iter().any(|c| {
        matches!(body.node(c), Node::Element { tag, attrs, .. }
            if tag == "input"
                && attrs.iter().any(|a| a.name == "type"
                    && matches!(&a.value, AttrValue::Static(t) if t.trim_matches('"') == "file")))
    });
    let to_upload = table
        .answered
        .iter()
        .any(|(m, r)| *m == "POST" && r == &target)
        && !RELYING_PARTY.iter().any(|(_, r)| *r == target);
    if sends_file || value("enctype").is_some() || to_upload || !is_internal(&target) {
        return;
    }
    let method = value("method").map_or_else(|| "GET".to_string(), |(m, _)| m.to_ascii_uppercase());
    if table.answers(&method, &target) {
        return;
    }
    let answering: Vec<String> = table
        .answered
        .iter()
        .filter(|(_, r)| matches(r, &target))
        .map(|(m, _)| format!("`{m}`"))
        .collect();
    out.push(Diagnostic {
        code: codes::FORM_ANSWERED_BY_NOTHING.id,
        invariant: codes::FORM_ANSWERED_BY_NOTHING.invariant,
        reason: "form_answered_by_nothing",
        detector: Detector::PatternMatrix,
        severity: Severity::Error,
        message: match answering.is_empty() {
            true => format!("a form sends `{method} {target}`, and nothing answers `{target}`"),
            false => format!(
                "a form sends `{method} {target}`, and `{target}` answers {} alone",
                answering.join(", ")
            ),
        },
        primary_span: span,
        related: vec![Related {
            span: at,
            label: format!("`{}` renders the form", decl.name),
        }],
        explanation: Some(
            "A form's `action` is a request the browser sends with the form's \
             `method`, `get` where it states none: a page answers a `get`, the \
             relying party its own routes (`/sign-in`, `/sign-up` and the callback \
             a `get`, `/sign-out` a `post`), and an upload a `post` of a file. A \
             form sent where nothing answers it fails as it is submitted, and so \
             does one whose handler would have prevented it, on a page whose \
             runtime has not loaded."
                .to_string(),
        ),
        repairs: vec![Repair {
            description: "send the form where something answers its method".to_string(),
            replacement: None,
        }],
    });
}

pub fn parameters_agree(
    hir: &Hir,
    unit: usize,
    sigs: &crate::signatures::Signatures,
) -> Vec<Diagnostic> {
    let mut out = Vec::new();
    for (id, decl) in hir.all_decls() {
        if decl.kind != crate::hir::DeclKind::Page {
            continue;
        }
        let Some(route) = declared_route(hir, decl) else {
            continue;
        };
        let at = hir.decl_span(id);
        let refuse = |out: &mut Vec<Diagnostic>, message: String, repair: String| {
            let code = codes::ROUTE_NAMES_ITS_PARAMETERS;
            out.push(Diagnostic {
                code: code.id,
                invariant: code.invariant,
                reason: "route_names_its_parameters",
                detector: Detector::DeclarationRule,
                severity: Severity::Error,
                message,
                primary_span: at.clone(),
                related: Vec::new(),
                explanation: Some(
                    "A page is served at its route, and the address gives its parameters, \
                     one segment each (ADR-0160). A parameter the route does not name \
                     would be given nothing, and a name that is no parameter would be \
                     given to nothing."
                        .to_string(),
                ),
                repairs: vec![Repair {
                    description: repair,
                    replacement: None,
                }],
            });
        };
        let segments: Vec<&str> = match route.as_str() {
            "/" => Vec::new(),
            r => r.strip_prefix('/').unwrap_or(r).split('/').collect(),
        };
        let word = |s: &str| {
            !s.is_empty()
                && s.chars()
                    .all(|c| c.is_ascii_alphanumeric() || matches!(c, '-' | '_' | '.' | '~'))
        };
        let mut named: Vec<&str> = Vec::new();
        let mut malformed = !route.starts_with('/');
        for s in &segments {
            match s.strip_prefix('{').and_then(|s| s.strip_suffix('}')) {
                Some(name) if word(name) => named.push(name),
                _ if word(s) => {}
                _ => malformed = true,
            }
        }
        if malformed {
            refuse(
                &mut out,
                format!(
                    "`{route}` is not a route: it is `/` and segments, a word or a \
                     `{{parameter}}` each"
                ),
                "write the route as `/` and segments, `/stores/{id}`".to_string(),
            );
            continue;
        }
        let params: Vec<&str> = decl.params.iter().map(|p| p.name.as_str()).collect();
        for (i, name) in named.iter().enumerate() {
            if !params.contains(name) {
                refuse(
                    &mut out,
                    format!(
                        "`{route}` names `{name}`, which is not a parameter of `{}`",
                        decl.name
                    ),
                    format!("name a parameter of `{}`, or declare `{name}`", decl.name),
                );
            } else if named[..i].contains(name) {
                refuse(
                    &mut out,
                    format!("`{route}` names `{name}` twice"),
                    format!("name `{name}` once"),
                );
            }
        }
        for p in &params {
            if !named.contains(p) {
                refuse(
                    &mut out,
                    format!(
                        "`{route}` does not name `{p}`, a parameter of `{}`: the address \
                         would not give it",
                        decl.name
                    ),
                    format!("add a `{{{p}}}` segment"),
                );
            }
        }
        // Each parameter the route gives is text (PW0621).
        let def = crate::resolve::DefId { unit, decl: id.0 };
        let Some(sig) = sigs.by_def(def) else {
            continue;
        };
        for (i, p) in decl.params.iter().enumerate() {
            let Some(ty) = sig
                .params
                .get(i)
                .and_then(Option::as_ref)
                .and_then(crate::resolved::TypeResolution::resolved)
            else {
                continue;
            };
            if !named.contains(&p.name.as_str()) || is_text(sigs, ty) {
                continue;
            }
            let code = codes::ROUTE_PARAMETER_IS_TEXT;
            out.push(Diagnostic {
                code: code.id,
                invariant: code.invariant,
                reason: "route_parameter_is_text",
                detector: Detector::DeclarationRule,
                severity: Severity::Error,
                message: format!(
                    "`{}`'s parameter `{}` is of type `{}`, and an address gives it text",
                    decl.name,
                    p.name,
                    ty.display_name()
                ),
                primary_span: at.clone(),
                related: Vec::new(),
                explanation: Some(
                    "A route's segment is text, and its parameter is given what the segment \
                     says, decoded (ADR-0160). A `String`, or an opaque type over one such as \
                     an id, takes it whole; a number or a flag would need decoding the \
                     route does not declare."
                        .to_string(),
                ),
                repairs: vec![Repair {
                    description: "declare the parameter a `String`, or an opaque type over one"
                        .to_string(),
                    replacement: None,
                }],
            });
        }
    }
    out
}

/// A `String`, or an opaque type whose representation is one.
pub(crate) fn is_text(sigs: &crate::signatures::Signatures, ty: &crate::resolved::ResolvedType) -> bool {
    use crate::resolved::Primitive;
    if ty.as_primitive() == Some(Primitive::Str) {
        return true;
    }
    ty.def_id()
        .and_then(|d| sigs.type_decl(d))
        .and_then(|t| t.representation.as_ref())
        .and_then(crate::resolved::TypeResolution::resolved)
        .is_some_and(|r| r.as_primitive() == Some(Primitive::Str))
}

/// **One route is one page's** (ADR-0160, PW0341): two pages at one route,
/// its parameters' names aside, leave an address naming two pages. Reported
/// on each page of `unit` that shares its route with a page before it.
pub fn declared_twice(hirs: &[&Hir], unit: usize) -> Vec<Diagnostic> {
    let shape = |route: &str| -> String {
        route
            .split('/')
            .map(|s| match s.starts_with('{') && s.ends_with('}') {
                true => "{}",
                false => s,
            })
            .collect::<Vec<_>>()
            .join("/")
    };
    let mut first: std::collections::BTreeMap<String, (usize, String)> =
        std::collections::BTreeMap::new();
    let mut out = Vec::new();
    for (u, hir) in hirs.iter().enumerate() {
        for (id, decl) in hir.all_decls() {
            if decl.kind != crate::hir::DeclKind::Page {
                continue;
            }
            let Some(route) = declared_route(hir, decl) else {
                continue;
            };
            match first.get(&shape(&route)) {
                None => {
                    first.insert(shape(&route), (u, decl.name.clone()));
                }
                Some((_, other)) if u == unit => {
                    let code = codes::ROUTE_DECLARED_TWICE;
                    out.push(Diagnostic {
                        code: code.id,
                        invariant: code.invariant,
                        reason: "route_declared_twice",
                        detector: Detector::DeclarationRule,
                        severity: Severity::Error,
                        message: format!(
                            "`{}` is served at `{route}`, and so is `{other}`",
                            decl.name
                        ),
                        primary_span: hir.decl_span(id),
                        related: Vec::new(),
                        explanation: Some(
                            "A page is served at its route (ADR-0160), and an address must \
                             say which page it is for: two pages at one route, whatever \
                             their parameters are called, leave it to whichever is found \
                             first."
                                .to_string(),
                        ),
                        repairs: vec![Repair {
                            description: "give each page a route of its own".to_string(),
                            replacement: None,
                        }],
                    });
                }
                Some(_) => {}
            }
        }
    }
    out
}

/// **The case a page's `not_found_on` names** (ADR-0163): the error type, by
/// identity, and the case's name, when the clause is `Type.Case` and the type
/// has the case. `Ok(None)` for a declaration without the clause.
pub(crate) fn not_found_case(
    ws: &crate::resolve::Workspace,
    sigs: &crate::signatures::Signatures,
    unit: usize,
    decl: &crate::hir::Decl,
) -> Result<Option<(crate::resolve::DefId, String)>, String> {
    use crate::resolve::{Namespace, Resolution};
    let Some(p) = decl.policy("not_found_on") else {
        return Ok(None);
    };
    let value = p.value.trim();
    let Some((ty, case)) = value.rsplit_once('.') else {
        return Err(format!("`{value}` is not a case: write `Type.Case`"));
    };
    // `StoreError`, or `domain.StoreError` by a module the file imports.
    let def = match ws.resolve_path_in(unit, Namespace::Type, ty) {
        Resolution::Local(d) | Resolution::Imported { def: d, .. } => d,
        _ => return Err(format!("`{ty}` names no type visible here")),
    };
    let has = sigs
        .type_decl(def)
        .and_then(|t| t.variants.as_ref())
        .is_some_and(|cases| cases.iter().any(|(c, _)| c == case));
    if !has {
        return Err(format!("`{ty}` has no case `{case}`"));
    }
    Ok(Some((def, case.to_string())))
}

/// The error type a query declares, `E` of its `Result<T, E>`, by identity.
pub(crate) fn error_of(
    sigs: &crate::signatures::Signatures,
    query: crate::resolve::DefId,
) -> Option<crate::resolve::DefId> {
    let result = sigs.by_def(query)?.result()?;
    if result.as_builtin() != Some(crate::resolved::Builtin::Result) {
        return None;
    }
    result.args().get(1)?.def_id()
}

/// Does this program own the target?
fn is_internal(target: &str) -> bool {
    target.starts_with('/') && !target.starts_with("//")
}

/// Does a declared route match a link target?
///
/// Segment by segment, where a `{…}` segment in the route matches any single
/// segment in the target. `/stores/{id}` matches `/stores/7`, and matches
/// `/stores/{id}` as written in source, because the interpolation occupies
/// exactly one segment. It does not match `/stores/7/reviews`: a parameter
/// stands for one segment, not for the rest of the path.
fn matches(route: &str, target: &str) -> bool {
    let r: Vec<&str> = route.trim_matches('/').split('/').collect();
    let t: Vec<&str> = target.trim_matches('/').split('/').collect();
    r.len() == t.len()
        && r.iter()
            .zip(&t)
            .all(|(r, t)| (r.starts_with('{') && r.ends_with('}')) || r == t)
}
