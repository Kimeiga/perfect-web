//! HIR → Marko 6 templates (ADR-0017).
//!
//! The E3 adapter. Marko is **tape** (ADR-0002): it renders while `pw`'s own
//! renderer does not exist, and it is removable because this module reads HIR
//! and writes files into a build directory — nothing else depends on it.
//!
//! Two rules keep the adapter from quietly becoming the language:
//!
//! 1. **HIR is the only input.** A construct that is not modelled cannot be
//!    emitted, and the fix is to model it — not to special-case it here.
//! 2. **Unsupported constructs are reported, never approximated.** An
//!    approximation renders, which is worse than not rendering: the page looks
//!    right and is wrong.
//!
//! Every mapping was verified against Marko 6.3.32 templates that compile and
//! serve (`spikes/marko-stream-resume`), not read from documentation.

use crate::hir::{
    AttrValue, Body, Decl, DeclKind, Expr, ExprId, Hir, Literal, Node, NodeId, Pattern,
};

/// A declaration the adapter could not render, and why.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Skipped {
    pub name: String,
    pub reason: String,
}

#[derive(Debug, Clone, Default)]
pub struct Rendered {
    /// `(file name, contents)`, ready to write under a build directory.
    pub files: Vec<(String, String)>,
    pub skipped: Vec<Skipped>,
}

/// Lower every renderable declaration in a module.
pub fn render_module(hir: &Hir, source_name: &str) -> Rendered {
    let mut out = Rendered::default();
    for (_, decl) in hir.all_decls() {
        if !matches!(
            decl.kind,
            DeclKind::View | DeclKind::Component | DeclKind::Page
        ) {
            continue;
        }
        match render_decl(hir, decl, source_name) {
            Ok(text) => out.files.push((format!("{}.marko", decl.name), text)),
            Err(reason) => out.skipped.push(Skipped {
                name: decl.name.clone(),
                reason,
            }),
        }
    }
    out
}

fn render_decl(hir: &Hir, decl: &Decl, source_name: &str) -> Result<String, String> {
    let body_id = decl.body.ok_or_else(|| "no body".to_string())?;
    let body = hir.body(body_id);

    // The markup is whatever template regions the body contains. A view with
    // none renders nothing, which is a bug in the source, not in the adapter.
    let mut roots: Vec<NodeId> = Vec::new();
    for id in body.walk() {
        if let Expr::Template { roots: r, .. } = body.expr(id) {
            roots.extend(r.iter().copied());
        }
    }
    if roots.is_empty() {
        return Err("no markup to render".to_string());
    }

    let mut out = String::new();
    out.push_str(&banner(source_name, &decl.name));

    // A streamed region calls its query, so the template needs the
    // implementation. Generated templates import it from a host-provided
    // module; the host decides what a `query` actually does, which is the whole
    // point of the manifest being separate from the renderer (ADR-0018).
    let queries = imported_resources(hir, body, &roots);
    if !queries.is_empty() {
        out.push_str(&format!(
            "import {{ {} }} from \"{RESOURCE_MODULE}\";\n\n",
            queries.join(", ")
        ));
    }

    // A view's own bindings become Marko state. `let mut` is reactive state
    // (`<let/…>`); an immutable binding is a computed constant (`<const/…>`).
    // Without this the counter has nothing to increment.
    let params: Vec<String> = decl.params.iter().map(|p| p.name.clone()).collect();
    // Two passes: collect what each binding reads, then render. A handler in
    // the markup needs to know which binding its command invalidates, and the
    // markup is rendered after the bindings are known.
    let mut binding_reads: Vec<(String, String)> = Vec::new();
    if let Expr::Block { stmts } = body.expr(body.root) {
        for st in stmts {
            if let Expr::Let {
                pat: Some(pat),
                init: Some(init),
                ..
            } = body.expr(*st)
                && let Pattern::Bind { name, .. } = body.pat(*pat)
                && let Expr::Keyword {
                    keyword, modifiers, ..
                } = body.expr(*init)
                && matches!(keyword.as_str(), "query" | "subscription")
            {
                binding_reads.push((name.clone(), modifiers.first().cloned().unwrap_or_default()));
            }
        }
    }
    let ctx = Ctx {
        params: &params,
        hir,
        bindings: binding_reads,
    };
    let mut bindings = String::new();
    if let Expr::Block { stmts } = body.expr(body.root) {
        for st in stmts {
            let Expr::Let { pat, init, .. } = body.expr(*st) else {
                continue;
            };
            let (Some(pat), Some(init)) = (pat, init) else {
                continue;
            };
            let Pattern::Bind { name, mutable } = body.pat(*pat) else {
                continue;
            };
            // A binding a command DECLARES it invalidates must be reactive, or
            // the mutation runs and the DOM does not move. `const` is correct
            // for a value nothing invalidates and wrong for one something does
            // — and which is which is written down: `invalidates Cart(..)`
            // names the query.
            //
            // Found by the store demo's browser test failing in all three
            // engines. The E3 counter uses `let mut`, so `const` had never been
            // wrong before.
            // Compared against the QUERY this binding reads, not the binding's
            // own name: `let cart = query Cart(..)` binds `cart` and reads
            // `Cart`, and comparing the two spellings finds nothing.
            let reads = match body.expr(*init) {
                Expr::Keyword {
                    keyword, modifiers, ..
                } if matches!(keyword.as_str(), "query" | "subscription") => {
                    modifiers.first().cloned().unwrap_or_default()
                }
                _ => String::new(),
            };
            let invalidated = !reads.is_empty() && invalidated_queries(hir).contains(&reads);
            let kw = if *mutable || invalidated {
                "let"
            } else {
                "const"
            };
            bindings.push_str(&format!(
                "<{kw}/{name}={}>\n",
                expr_text(body, *init, &ctx)?
            ));
        }
    }
    if !bindings.is_empty() {
        out.push_str(&bindings);
        out.push('\n');
    }

    for r in &roots {
        emit_node(body, *r, 0, &ctx, &mut out)?;
    }
    Ok(out)
}

/// What a name means in this template. A view's parameters arrive as `input`
/// in Marko; its own bindings are plain locals.
struct Ctx<'a> {
    params: &'a [String],
    /// The module, for resolving a command's `invalidates` policy.
    hir: &'a Hir,
    /// `(binding name, the query it reads)`, so a handler can find which
    /// reactive binding its command invalidates.
    bindings: Vec<(String, String)>,
}

impl Ctx<'_> {
    fn name(&self, n: &str) -> String {
        if self.params.iter().any(|p| p == n) {
            format!("input.{n}")
        } else {
            n.to_string()
        }
    }
}

/// Where a generated template imports its resource implementations from,
/// relative to `routes/<route>/+page.marko`.
pub const RESOURCE_MODULE: &str = "../../resources.mjs";

/// The reactive binding a handler's command invalidates, if any.
///
/// `add_to_cart` declares `invalidates Cart(..)`, and the template binds
/// `let cart = query Cart(..)`. So the handler assigns its result to `cart`.
fn invalidated_binding(ctx: &Ctx<'_>, body: &Body, call: ExprId) -> Option<String> {
    let Expr::Call { callee, .. } = body.expr(call) else {
        return None;
    };
    let Expr::Name(command) = body.expr(*callee) else {
        return None;
    };
    let (_, decl) = ctx.hir.all_decls().find(|(_, d)| d.name == *command)?;
    let query = decl
        .policies
        .iter()
        .find(|p| p.name == "invalidates")?
        .value
        .trim()
        .split('(')
        .next()?
        .trim()
        .to_string();
    ctx.bindings
        .iter()
        .find(|(_, reads)| *reads == query)
        .map(|(binding, _)| binding.clone())
}

/// Query names some command in this module declares it invalidates.
///
/// The binding for such a query is reactive: a mutation that says it
/// invalidates `Cart` means the rendered cart must change when it runs.
fn invalidated_queries(hir: &Hir) -> Vec<String> {
    let mut out = Vec::new();
    for (_, d) in hir.all_decls() {
        for p in &d.policies {
            if p.name != "invalidates" {
                continue;
            }
            // `invalidates Cart(current_session())` — the query is the head.
            let head = p
                .value
                .trim()
                .split('(')
                .next()
                .unwrap_or_default()
                .trim()
                .to_string();
            if !head.is_empty() && !out.contains(&head) {
                out.push(head);
            }
        }
    }
    out
}

/// Every declaration whose implementation this template must import.
///
/// Two shapes, and only the first existed before the store demo:
///
/// ```text
/// <stream query={Recommendations(id)}>     a streamed region
/// let store = query Store(id)              a body-level dependency
/// ```
///
/// A template missing the second import compiles and then fails at runtime
/// with `Store is not defined`, which is the worst kind of generator bug: it
/// looks like the host forgot to implement something.
fn imported_resources(hir: &Hir, body: &Body, roots: &[NodeId]) -> Vec<String> {
    let mut names = streamed_queries(body, roots);

    // Host-provided free functions the template calls. `current_session` and
    // the constructor forms are not declarations in the module — the host
    // supplies them, exactly as it supplies a query's implementation.
    const HOST_PROVIDED: &[&str] = &["current_session", "PositiveInt"];

    // A call to a `query`, `command` or `subscription` DECLARED in this module
    // is a resource too. The store demo calls `add_to_cart` from a handler,
    // which is a command, and a template that does not import it renders and
    // then fails on click.
    let resources: Vec<&str> = hir
        .all_decls()
        .filter(|(_, d)| {
            matches!(
                d.kind,
                crate::hir::DeclKind::Query
                    | crate::hir::DeclKind::Command
                    | crate::hir::DeclKind::Subscription
            )
        })
        .map(|(_, d)| d.name.as_str())
        .collect();
    for id in body.walk() {
        let Expr::Call { callee, .. } = body.expr(id) else {
            continue;
        };
        let Expr::Name(n) = body.expr(*callee) else {
            continue;
        };
        if (resources.contains(&n.as_str()) || HOST_PROVIDED.contains(&n.as_str()))
            && !names.contains(n)
        {
            names.push(n.clone());
        }
    }

    for id in body.walk() {
        let Expr::Keyword {
            keyword, modifiers, ..
        } = body.expr(id)
        else {
            continue;
        };
        if !matches!(keyword.as_str(), "query" | "command" | "subscription") {
            continue;
        }
        if let Some(n) = modifiers.first()
            && !names.contains(n)
        {
            names.push(n.clone());
        }
    }
    names
}

/// Query names a `<stream>` in this markup calls.
fn streamed_queries(body: &Body, roots: &[NodeId]) -> Vec<String> {
    let mut names = Vec::new();
    for id in body.walk_markup(roots) {
        let Node::Element { tag, attrs, .. } = body.node(id) else {
            continue;
        };
        if tag != "stream" {
            continue;
        }
        for a in attrs.iter().filter(|a| a.name == "query") {
            let AttrValue::Expr(e) = a.value else {
                continue;
            };
            let Expr::Call { callee, .. } = body.expr(e) else {
                continue;
            };
            let Expr::Name(n) = body.expr(*callee) else {
                continue;
            };
            if !names.contains(n) {
                names.push(n.clone());
            }
        }
    }
    names
}

fn banner(source_name: &str, decl: &str) -> String {
    format!(
        "// GENERATED by pw from `{source_name}` ({decl}). Do not edit.\n\
         // ADR-0017: this file is build output. Editing it makes the `.pw`\n\
         // source no longer the truth, and the adapter no longer removable.\n\n"
    )
}

fn emit_node(
    body: &Body,
    id: NodeId,
    depth: usize,
    ctx: &Ctx<'_>,
    out: &mut String,
) -> Result<(), String> {
    let pad = "  ".repeat(depth);
    match body.node(id) {
        Node::Text(t) => {
            // Marko treats `${` as an interpolation, so literal text that
            // contains it would change meaning. Refuse rather than mangle.
            if t.contains("${") {
                return Err("text containing `${` cannot be rendered verbatim".to_string());
            }
            let trimmed = t.trim();
            if !trimmed.is_empty() {
                out.push_str(&format!("{pad}{trimmed}\n"));
            }
            Ok(())
        }

        Node::Interpolation(e) => {
            out.push_str(&format!("{pad}${{{}}}\n", expr_text(body, *e, ctx)?));
            Ok(())
        }

        Node::Block {
            directive,
            children,
        } => {
            let each = parse_each(directive).ok_or_else(|| {
                format!(
                    "the `{}` directive is not modelled yet",
                    directive.trim_matches(|c| c == '{' || c == '}').trim()
                )
            })?;
            // `by` keys the loop. Marko takes a property NAME for objects and a
            // FUNCTION for primitives — and the loop parameter is not in scope
            // inside `by=`, so `by=item.id` is an undefined variable. The
            // cheatsheet calls this out; getting it wrong compiles and silently
            // re-keys the list on every render.
            let by = match &each.key {
                Some(k) if k == &each.binding => format!(" by=({}) => {}", each.binding, k),
                Some(k) => match k.strip_prefix(&format!("{}.", each.binding)) {
                    Some(field) => format!(" by=\"{field}\""),
                    None => format!(" by=({}) => {k}", each.binding),
                },
                None => String::new(),
            };
            out.push_str(&format!(
                "{pad}<for|{}| of={}{by}>\n",
                each.binding,
                ctx.name(&each.source)
            ));
            for c in children {
                emit_node(body, *c, depth + 1, ctx, out)?;
            }
            out.push_str(&format!("{pad}</for>\n"));
            Ok(())
        }

        Node::Element {
            tag,
            attrs,
            children,
            self_closing,
        } if tag == "stream" => emit_stream(body, attrs, children, depth, ctx, out),

        Node::Element {
            tag,
            attrs,
            children,
            self_closing,
        } => {
            let mut head = format!("{pad}<{tag}");
            for a in attrs {
                head.push(' ');
                head.push_str(&attr_text(body, a, ctx)?);
            }
            if *self_closing {
                out.push_str(&format!("{head} />\n"));
                return Ok(());
            }

            // Whitespace with a newline in it is formatting; whitespace without
            // one is content. That is exactly the distinction the author made by
            // writing `<span>a</span> <span>b</span>` on one line, and it decides
            // the layout here.
            //
            // The first version assumed block layout was safe because "HTML
            // collapses the newline back to a space". It does not: **Marko
            // strips whitespace between elements in the template**, so the page
            // rendered "Espresso$3.50". A template-level test could not see
            // that — the template was fine. The browser test caught it.
            let inline = children.iter().any(|c| match body.node(*c) {
                Node::Text(t) => !t.contains('\n'),
                Node::Interpolation(_) => true,
                _ => false,
            });
            if inline {
                let mut buf = String::new();
                for c in children {
                    emit_inline(body, *c, ctx, &mut buf)?;
                }
                out.push_str(&format!("{head}>{}</{tag}>\n", buf.trim_end()));
                return Ok(());
            }

            out.push_str(&format!("{head}>\n"));
            for c in children {
                emit_node(body, *c, depth + 1, ctx, out)?;
            }
            out.push_str(&format!("{pad}</{tag}>\n"));
            Ok(())
        }
    }
}

/// `{#each items as item (item.id)}` — the only block directive modelled.
struct Each {
    source: String,
    binding: String,
    key: Option<String>,
}

fn parse_each(directive: &str) -> Option<Each> {
    let inner = directive.trim().strip_prefix("{#")?.strip_suffix('}')?;
    let rest = inner.trim().strip_prefix("each")?.trim();
    let (source, rest) = rest.split_once(" as ")?;
    let rest = rest.trim();
    let (binding, key) = match rest.split_once('(') {
        Some((b, k)) => (b.trim(), Some(k.trim_end_matches(')').trim().to_string())),
        None => (rest, None),
    };
    Some(Each {
        source: source.trim().to_string(),
        binding: binding.trim().to_string(),
        key,
    })
}

/// `<stream query={Q(x)}>` with `<placeholder>`, `<ready as={v}>` and
/// `<failed as={e}>` becomes Marko's `<try>` / `<await>`.
///
/// `@placeholder` and `@catch` go on the `<try>`, never on the `<await>` — E0
/// finding F-5, learned by reading the shipped cheatsheet after the build
/// rejected the other arrangement.
fn emit_stream(
    body: &Body,
    attrs: &[crate::hir::Attr],
    children: &[NodeId],
    depth: usize,
    ctx: &Ctx<'_>,
    out: &mut String,
) -> Result<(), String> {
    let pad = "  ".repeat(depth);
    let query = attrs
        .iter()
        .find(|a| a.name == "query")
        .ok_or("a `<stream>` needs a `query={..}` attribute")?;
    let AttrValue::Expr(q) = query.value else {
        return Err("a `<stream>` query must be an expression".to_string());
    };
    let promise = expr_text(body, q, ctx)?;

    let part = |name: &str| {
        children
            .iter()
            .find(|c| matches!(body.node(**c), Node::Element { tag, .. } if tag == name))
    };
    let ready = part("ready").ok_or("a `<stream>` needs a `<ready as={..}>` part")?;
    let (ready_binding, ready_children) = match body.node(*ready) {
        Node::Element {
            attrs, children, ..
        } => {
            let binding = attrs
                .iter()
                .find(|a| a.name == "as")
                .and_then(|a| match a.value {
                    AttrValue::Expr(e) => expr_text(body, e, ctx).ok(),
                    _ => None,
                })
                .ok_or("`<ready>` needs `as={name}`")?;
            (binding, children.clone())
        }
        _ => unreachable!("part() matched an element"),
    };

    out.push_str(&format!("{pad}<try>\n"));
    out.push_str(&format!("{pad}  <await|{ready_binding}|={promise}>\n"));
    for c in &ready_children {
        emit_node(body, *c, depth + 2, ctx, out)?;
    }
    out.push_str(&format!("{pad}  </await>\n"));

    if let Some(p) = part("placeholder") {
        let Node::Element { children, .. } = body.node(*p) else {
            unreachable!()
        };
        out.push_str(&format!("{pad}  <@placeholder>\n"));
        for c in children {
            emit_node(body, *c, depth + 2, ctx, out)?;
        }
        out.push_str(&format!("{pad}  </@placeholder>\n"));
    }
    if let Some(f) = part("failed") {
        let Node::Element {
            attrs, children, ..
        } = body.node(*f)
        else {
            unreachable!()
        };
        let binding = attrs
            .iter()
            .find(|a| a.name == "as")
            .and_then(|a| match a.value {
                AttrValue::Expr(e) => expr_text(body, e, ctx).ok(),
                _ => None,
            })
            .unwrap_or_else(|| "error".to_string());
        out.push_str(&format!("{pad}  <@catch|{binding}|>\n"));
        for c in children {
            emit_node(body, *c, depth + 2, ctx, out)?;
        }
        out.push_str(&format!("{pad}  </@catch>\n"));
    }
    out.push_str(&format!("{pad}</try>\n"));
    Ok(())
}

/// One node, with no added whitespace. Used inside mixed content, where every
/// space is part of what the page renders.
fn emit_inline(body: &Body, id: NodeId, ctx: &Ctx<'_>, out: &mut String) -> Result<(), String> {
    match body.node(id) {
        Node::Text(t) => {
            if t.contains("${") {
                return Err("text containing `${` cannot be rendered verbatim".to_string());
            }
            out.push_str(t);
            Ok(())
        }
        Node::Interpolation(e) => {
            out.push_str(&format!("${{{}}}", expr_text(body, *e, ctx)?));
            Ok(())
        }
        Node::Block { directive, .. } => Err(format!(
            "a `{}` block cannot appear inside inline content",
            directive.trim_matches(|c| c == '{' || c == '}').trim()
        )),
        Node::Element {
            tag,
            attrs,
            children,
            self_closing,
        } => {
            out.push('<');
            out.push_str(tag);
            for a in attrs {
                out.push(' ');
                out.push_str(&attr_text(body, a, ctx)?);
            }
            if *self_closing {
                out.push_str(" />");
                return Ok(());
            }
            out.push('>');
            for c in children {
                emit_inline(body, *c, ctx, out)?;
            }
            out.push_str(&format!("</{tag}>"));
            Ok(())
        }
    }
}

fn attr_text(body: &Body, a: &crate::hir::Attr, ctx: &Ctx<'_>) -> Result<String, String> {
    match (&a.value, a.namespace()) {
        // `on:press={h}` -> `onClick() { h }`. The event name is translated
        // rather than passed through: `press` is a *semantic* event in `pw` and
        // `onClick` is a DOM one, and conflating them is how a language ends up
        // shaped like its renderer.
        (AttrValue::Expr(e), Some(("on", event))) => {
            let dom = dom_event(event)?;
            // A handler may be written as a bare expression — `count = count + 1`
            // — or as a lambda, `() => add_to_cart(id)`. Both mean "do this when
            // the event fires", and Marko's `onClick() { .. }` is the same shape,
            // so the lambda's parameters are dropped and its body rendered.
            //
            // Only the E3 counter existed when this was written, and it uses the
            // first form. The store demo uses the second, and the adapter simply
            // refused it — a whole class of handler that could not be generated,
            // invisible because no example had one.
            let inner = match body.expr(*e) {
                Expr::Lambda { body: b, .. } => *b,
                _ => *e,
            };
            let call = expr_text(body, inner, ctx)?;
            // A handler that invokes a command which declares `invalidates Q`
            // must write the result back into `Q`'s reactive binding. Without
            // it the mutation runs and the DOM does not move — which is what
            // the store demo's browser test found in all three engines.
            match invalidated_binding(ctx, body, inner) {
                Some(binding) => Ok(format!("{dom}() {{ {binding} = {call} }}")),
                None => Ok(format!("{dom}() {{ {call} }}")),
            }
        }
        (_, Some(("on", _))) => Err("an `on:` attribute needs a handler expression".to_string()),

        (AttrValue::Expr(e), Some((ns, _))) => Err(format!(
            "the `{ns}:` attribute namespace is not modelled yet ({})",
            expr_text(body, *e, ctx).unwrap_or_default()
        )),
        (_, Some((ns, _))) => Err(format!(
            "the `{ns}:` attribute namespace is not modelled yet"
        )),

        (AttrValue::Static(v), None) => Ok(format!("{}={}", a.name, v)),
        (AttrValue::Expr(e), None) => Ok(format!("{}={}", a.name, expr_text(body, *e, ctx)?)),
        (AttrValue::None, None) => Ok(a.name.clone()),
    }
}

/// `pw`'s semantic events, mapped to DOM events.
///
/// Deliberately a closed list. Passing an unknown name straight through would
/// let a `.pw` file address the DOM directly, which is exactly the coupling
/// charter §11 says to avoid.
fn dom_event(event: &str) -> Result<&'static str, String> {
    Ok(match event {
        "press" => "onClick",
        "change" => "onChange",
        "input" => "onInput",
        "submit" => "onSubmit",
        "focus" => "onFocus",
        "blur" => "onBlur",
        other => return Err(format!("the `{other}` event is not modelled yet")),
    })
}

/// An expression, as Marko source.
///
/// Marko's expression syntax is JavaScript. The subset here is what a template
/// needs and no more; anything else is refused so it cannot silently render as
/// something that happens to be valid JavaScript with different meaning.
fn expr_text(body: &Body, id: ExprId, ctx: &Ctx<'_>) -> Result<String, String> {
    Ok(match body.expr(id) {
        Expr::Name(n) => ctx.name(n),
        Expr::Literal(Literal::Int(s) | Literal::Float(s) | Literal::Str(s)) => s.clone(),
        // Rendered as written; the adapter emits the template's own syntax.
        Expr::Interpolated { text, .. } => text.clone(),
        Expr::Literal(Literal::UnterminatedStr(_)) => {
            return Err("an unterminated string".to_string());
        }
        Expr::Field { base, name } => format!("{}.{}", expr_text(body, *base, ctx)?, name),
        Expr::Call { callee, args } => {
            let mut parts = Vec::new();
            for a in args {
                parts.push(expr_text(body, a.value, ctx)?);
            }
            format!("{}({})", expr_text(body, *callee, ctx)?, parts.join(", "))
        }
        // `query Store(id)` — a dependency on a declaration that runs at its
        // own placement. The template imports its implementation from the host
        // (ADR-0018), so it renders as the call the host provides.
        //
        // This arm exists because making `query` a statement keyword — which
        // fixed a false positive where every page declaring a dependency looked
        // like a page reaching the database — changed its HIR shape from `Call`
        // to `Keyword`, and the adapter had only ever seen the former. No E3
        // example uses `let x = query Q(..)`, so nothing noticed until the
        // store demo.
        Expr::Keyword {
            keyword,
            modifiers,
            args,
            ..
        } if matches!(keyword.as_str(), "query" | "command" | "subscription") => {
            let name = modifiers.first().cloned().unwrap_or_default();
            let rendered: Result<Vec<String>, String> =
                args.iter().map(|a| expr_text(body, *a, ctx)).collect();
            format!("{name}({})", rendered?.join(", "))
        }
        Expr::Binary { op, lhs, rhs } => {
            let o = js_binop(op)?;
            format!(
                "{} {o} {}",
                expr_text(body, *lhs, ctx)?,
                expr_text(body, *rhs, ctx)?
            )
        }
        other => return Err(format!("{} is not renderable", describe(other))),
    })
}

fn js_binop(op: &crate::hir::BinOp) -> Result<&'static str, String> {
    use crate::hir::BinOp::*;
    Ok(match op {
        Add => "+",
        Sub => "-",
        Mul => "*",
        Div => "/",
        Rem => "%",
        And => "&&",
        Or => "||",
        Cmp(c) => match c.as_str() {
            "==" => "===",
            "!=" => "!==",
            "<" => "<",
            ">" => ">",
            "<=" => "<=",
            ">=" => ">=",
            _ => return Err("an unsupported comparison".to_string()),
        },
        // Assignment is what an event handler does: `on:press={count = count + 1}`.
        Assign => "=",
        Pipe | Transition => return Err("a pipeline or transition".to_string()),
    })
}

fn describe(e: &Expr) -> &'static str {
    match e {
        Expr::Lambda { .. } => "a lambda",
        Expr::Match { .. } => "a match",
        Expr::If { .. } => "an if expression",
        Expr::Block { .. } => "a block",
        Expr::Record { .. } => "a record literal",
        Expr::List { .. } => "a collection literal",
        Expr::Let { .. } => "a binding",
        Expr::Keyword { .. } => "a body-level statement",
        Expr::Template { .. } => "nested markup",
        Expr::Unary { .. } => "a unary operator",
        Expr::Error => "an expression that did not parse",
        _ => "this expression",
    }
}
