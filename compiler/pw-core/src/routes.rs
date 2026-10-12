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
                if !is_internal(target) {
                    continue;
                }
                if table.answers("GET", target) {
                    // And its query names only what its page's route gives
                    // (ADR-0309).
                    let given = target
                        .split_once('?')
                        .map(|(_, q)| q.split('#').next().unwrap_or(q))
                        .into_iter()
                        .flat_map(|q| q.split('&'))
                        .filter_map(|kv| kv.split('=').next())
                        // A name the program computes is not matched.
                        .filter(|name| !name.is_empty() && !name.contains('{'));
                    for name in given {
                        unknown_query_name(name, target, a.span.clone(), decl, &at, table, out);
                    }
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
                    related: vec![Related::new(
                        at.clone(),
                        format!("`{}` renders the link", decl.name),
                    )],
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

/// **A name an address gives that its page's route does not** (ADR-0309,
/// PW5050): a link's query name, or a GET form's field, reaching a page
/// whose route's query part does not name it. The host ignores a name its
/// route does not declare, as an address may carry one from elsewhere
/// (`utm_source`), so a program's own would carry its value to nothing.
fn unknown_query_name(
    name: &str,
    target: &str,
    span: crate::hir::Span,
    decl: &crate::hir::Decl,
    at: &crate::hir::Span,
    table: &Table,
    out: &mut Vec<Diagnostic>,
) {
    let Some(route) = table.pages.iter().find(|r| matches(r, target)) else {
        return;
    };
    let names = route_parts(route).1;
    if names.contains(&name) {
        return;
    }
    let path = target.split(['?', '#']).next().unwrap_or(target);
    let code = codes::ADDRESS_NAMES_NO_QUERY_PARAMETER;
    out.push(Diagnostic {
        code: code.id,
        invariant: code.invariant,
        reason: "address_names_no_query_parameter",
        detector: Detector::PatternMatrix,
        severity: Severity::Error,
        message: match names.is_empty() {
            true => {
                format!("`{path}` is given `{name}`, and its route `{route}` has no query part")
            }
            false => format!(
                "`{path}` is given `{name}`, and its route `{route}` gives {}",
                names
                    .iter()
                    .map(|n| format!("`{n}`"))
                    .collect::<Vec<_>>()
                    .join(", ")
            ),
        },
        primary_span: span,
        related: vec![Related::new(
            at.clone(),
            format!("`{}` gives it", decl.name),
        )],
        explanation: Some(
            "A route's query part, `{?q}`, names what an address gives its page \
             (ADR-0309). A link's query and a GET form's fields are that address, \
             and the host reads the names its route declares and no other: a \
             name it does not declare would carry its value to nothing."
                .to_string(),
        ),
        repairs: vec![Repair {
            description: format!(
                "give `{path}` only what its route's query names, or name `{name}` in it"
            ),
            replacement: None,
        }],
    });
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
        // A GET form's fields are its address's query (HTML's "mutate action
        // URL"): each a name its page's route gives (ADR-0309).
        if method == "GET" {
            for c in body.walk_markup(children) {
                let Node::Element { tag, attrs, .. } = body.node(c) else {
                    continue;
                };
                if !matches!(tag.as_str(), "input" | "select" | "textarea" | "button") {
                    continue;
                }
                let Some(a) = attrs.iter().find(|a| a.name == "name") else {
                    continue;
                };
                if let AttrValue::Static(raw) = &a.value {
                    let name = raw.trim_matches('"');
                    unknown_query_name(name, &target, a.span.clone(), decl, &at, table, out);
                }
            }
        }
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
        related: vec![Related::new(
            at,
            format!("`{}` renders the form", decl.name),
        )],
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
        // Its path, and the names its query part gives (ADR-0309).
        let (path, queried) = route_parts(&route);
        let segments: Vec<&str> = match path {
            "/" => Vec::new(),
            r => r.strip_prefix('/').unwrap_or(r).split('/').collect(),
        };
        let word = |s: &str| {
            !s.is_empty()
                && s.chars()
                    .all(|c| c.is_ascii_alphanumeric() || matches!(c, '-' | '_' | '.' | '~'))
        };
        // A query written as text, `/search?q=x`, or an expansion anywhere but
        // the route's end, is no route: its query part names what an address
        // may give, `{?q}` (ADR-0309).
        if path.contains('?') || path.contains("{?") {
            refuse(
                &mut out,
                format!(
                    "`{route}` writes its query: a route's query part is `{{?name}}` at its \
                     end, RFC 6570's form-style expansion, naming what an address may give"
                ),
                "write the query part as `{?q}`, after the path".to_string(),
            );
            continue;
        }
        let mut named: Vec<&str> = Vec::new();
        let mut malformed = !route.starts_with('/') || queried.iter().any(|q| !word(q));
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
                     `{{parameter}}` each, and may end in a query part, `{{?name,..}}`"
                ),
                "write the route as `/` and segments, `/stores/{id}`".to_string(),
            );
            continue;
        }
        let params: Vec<&str> = decl.params.iter().map(|p| p.name.as_str()).collect();
        // The path's names, then the query's: each a parameter, once.
        let path_names = named.len();
        named.extend(queried.iter().copied());
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
                    format!("add a `{{{p}}}` segment, or name it in the query, `{{?{p}}}`"),
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
            // A query's parameter is an `Option` of text: an address may leave
            // it out (ADR-0309, PW0630).
            let name = p.name.as_str();
            if named[path_names..].contains(&name) && !named[..path_names].contains(&name) {
                if !optional_text(sigs, ty) {
                    let code = codes::QUERY_PARAMETER_IS_OPTIONAL_TEXT;
                    out.push(Diagnostic {
                        code: code.id,
                        invariant: code.invariant,
                        reason: "query_parameter_is_optional_text",
                        detector: Detector::DeclarationRule,
                        severity: Severity::Error,
                        message: format!(
                            "`{}`'s parameter `{}` is of type `{}`, and an address may leave \
                             its query out: it is given as an `Option` of text",
                            decl.name,
                            p.name,
                            ty.display_name()
                        ),
                        primary_span: at.clone(),
                        related: Vec::new(),
                        explanation: Some(
                            "A route's query part, `{?q}`, names what an address may give: a \
                             link, a form or a reader may leave it out, and a field left empty \
                             gives it empty. The page is given `None` where it is left out, and \
                             what the address says, decoded, where it is not."
                                .to_string(),
                        ),
                        repairs: vec![Repair {
                            description: "declare the parameter an `Option<String>`, or an \
                                          `Option` of an opaque type over one"
                                .to_string(),
                            replacement: None,
                        }],
                    });
                }
                continue;
            }
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

/// **An `Option` of text** (ADR-0309): what a query parameter is given.
fn optional_text(sigs: &crate::signatures::Signatures, ty: &crate::resolved::ResolvedType) -> bool {
    ty.as_builtin() == Some(crate::resolved::Builtin::Option)
        && ty.args().first().is_some_and(|a| is_text(sigs, a))
}

/// A `String`, or an opaque type whose representation is one.
pub(crate) fn is_text(
    sigs: &crate::signatures::Signatures,
    ty: &crate::resolved::ResolvedType,
) -> bool {
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
    // By the path alone: two routes that differ in their query part name one
    // page's address (ADR-0309).
    let shape = |route: &str| -> String {
        route_parts(route)
            .0
            .split('/')
            .map(|s| match s.starts_with('{') && s.ends_with('}') {
                true => "{}",
                false => s,
            })
            .collect::<Vec<_>>()
            .join("/")
    };
    // Each route's first page: its unit, its name, its module's, and where
    // it is declared.
    #[allow(clippy::type_complexity)]
    let mut first: std::collections::BTreeMap<
        String,
        (usize, String, Option<String>, crate::hir::Span),
    > = std::collections::BTreeMap::new();
    let mut out = Vec::new();
    for (u, hir) in hirs.iter().enumerate() {
        for (id, decl) in hir.all_decls() {
            if decl.kind != crate::hir::DeclKind::Page {
                continue;
            }
            let Some(route) = declared_route(hir, decl) else {
                continue;
            };
            let module = hir.module_of(id).map(str::to_string);
            match first.get(&shape(&route)) {
                None => {
                    first.insert(
                        shape(&route),
                        (u, decl.name.clone(), module, hir.decl_span(id)),
                    );
                }
                Some((at, other, theirs, span)) if u == unit => {
                    // Two pages of one name each by its module, or the
                    // message names one page twice. Until 2026-10-11 the
                    // store's `CartPage` and a corpus file's were "`CartPage`
                    // is served at `/cart`, and so is `CartPage`".
                    let named = |name: &str, module: &Option<String>| match module {
                        Some(m) if *other == decl.name => format!("{m}.{name}"),
                        _ => name.to_string(),
                    };
                    let code = codes::ROUTE_DECLARED_TWICE;
                    out.push(Diagnostic {
                        code: code.id,
                        invariant: code.invariant,
                        reason: "route_declared_twice",
                        detector: Detector::DeclarationRule,
                        severity: Severity::Error,
                        message: format!(
                            "`{}` is served at `{route}`, and so is `{}`",
                            named(&decl.name, &module),
                            named(other, theirs)
                        ),
                        primary_span: hir.decl_span(id),
                        // The page first served there, in its own file
                        // (ADR-0321).
                        related: vec![Related::in_unit(
                            *at,
                            span.clone(),
                            format!("`{}` is served at it first", named(other, theirs)),
                        )],
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
    let Some(p) = decl.policy("not_found_on") else {
        return Ok(None);
    };
    case_named(ws, sigs, unit, p.value.trim()).map(Some)
}

/// **The case a page's `redirect_on` names** (ADR-0295), as `not_found_on`'s
/// is, and whether the move is `permanent` (308) or `temporary` (307), which
/// the clause must say. `Ok(None)` for a declaration without the clause.
pub(crate) fn redirect_case(
    ws: &crate::resolve::Workspace,
    sigs: &crate::signatures::Signatures,
    unit: usize,
    decl: &crate::hir::Decl,
) -> Result<Option<(crate::resolve::DefId, String, bool)>, String> {
    let Some(p) = decl.policy("redirect_on") else {
        return Ok(None);
    };
    let words: Vec<&str> = p.value.split_whitespace().collect();
    let [case, how] = words[..] else {
        return Err("write `Type.Case permanent`, or `Type.Case temporary`".to_string());
    };
    let permanent = match how {
        "permanent" => true,
        "temporary" => false,
        other => return Err(format!("`{other}` is neither `permanent` nor `temporary`")),
    };
    let (ty, case) = case_named(ws, sigs, unit, case)?;
    Ok(Some((ty, case, permanent)))
}

/// `Type.Case`, resolved: the type by identity and the case's name, where the
/// type is visible as written and has the case.
fn case_named(
    ws: &crate::resolve::Workspace,
    sigs: &crate::signatures::Signatures,
    unit: usize,
    value: &str,
) -> Result<(crate::resolve::DefId, String), String> {
    use crate::resolve::{Namespace, Resolution};
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
    Ok((def, case.to_string()))
}

/// **A route's path, and the names its query part gives** (ADR-0309):
/// `"/search{?q}"` is `("/search", ["q"])`, RFC 6570's form-style query
/// expansion, at the route's end; a route without one gives none.
pub(crate) fn route_parts(route: &str) -> (&str, Vec<&str>) {
    let route = route.trim().trim_matches('"');
    match route.find("{?") {
        Some(at)
            if route.ends_with('}') && !route[at + 2..route.len() - 1].contains(['{', '}']) =>
        {
            (
                &route[..at],
                route[at + 2..route.len() - 1]
                    .split(',')
                    .map(str::trim)
                    .collect(),
            )
        }
        _ => (route, Vec::new()),
    }
}

/// The `{name}` segments of a route's path as written, `"/word/{word}"`, in
/// order: its query part's names are not segments (ADR-0309).
pub(crate) fn route_holes(route: &str) -> Vec<String> {
    route_parts(route)
        .0
        .split('/')
        .filter_map(|s| s.strip_prefix('{').and_then(|s| s.strip_suffix('}')))
        .map(str::to_string)
        .collect()
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
    // By the path: a route's query part, and a target's query and fragment,
    // say what is given to a page, not which page (ADR-0309).
    let route = route_parts(route).0;
    let target = target.split(['?', '#']).next().unwrap_or(target);
    let r: Vec<&str> = route.trim_matches('/').split('/').collect();
    let t: Vec<&str> = target.trim_matches('/').split('/').collect();
    r.len() == t.len()
        && r.iter()
            .zip(&t)
            .all(|(r, t)| (r.starts_with('{') && r.ends_with('}')) || r == t)
}
