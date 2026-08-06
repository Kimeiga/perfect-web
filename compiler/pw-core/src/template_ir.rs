//! E7 task 2 — the checked template IR.
//!
//! ```text
//! .pw → canonical syntax → HIR → types/labels/effects/placement
//!     → checked Template IR → E7 server renderer → HTML bytes
//! ```
//!
//! # The renderer must not rediscover program meaning
//!
//! Architect ruling, 2026-08-06. No textual name resolution, no "if this
//! expression happens to be called `query`", no re-parsing fragments, no
//! adapter-specific guesses. The renderer consumes identities and decisions
//! that are already represented here.
//!
//! That is why this is not "the HIR, serialized". The HIR is a faithful record
//! of what was written; a renderer walking it would have to ask, at every node,
//! what the node probably means. Every such question is answered once, here,
//! where the answer can be wrong in a way a test can see.
//!
//! # Static and dynamic are different things
//!
//! ```text
//! Template {
//!     Static("<main><h1>")
//!     Text(part 0)
//!     Static("</h1>")
//!     ...
//! }
//! ```
//!
//! The split is explicit rather than derived, and it is the representation the
//! reactive renderer reuses: the server renders `Static` + `Part`s, and the
//! browser runtime later updates only `Part`s. Deriving it at render time would
//! mean deriving it again, differently, in the runtime.
//!
//! # Escaping context is part of the IR
//!
//! A `Part` carries the [`Context`] its value occupies, decided here from where
//! the value appears in the markup. A renderer with one `escape_html` and a
//! guess at the call site is the classic injection surface: the same string is
//! inert in text, an attribute breakout in an attribute, and a script in a URL.
//!
//! # Nothing is silently dropped
//!
//! A construct the IR cannot represent becomes [`Part::Blocked`], carrying why.
//! The `Outcome` lesson from E2D, applied to rendering: a renderer that emitted
//! `""` for a region it did not understand would turn "an earlier analysis
//! failed" into apparently valid HTML, and the page would look fine.

use serde::{Deserialize, Serialize};

use crate::hir::{AttrValue, Body, Expr, Hir, Node, NodeId};

/// Where a value sits in the document, which decides how it is escaped.
///
/// Not a hint. Two values with the same bytes in different contexts need
/// different treatment, and a renderer that escapes "the usual way" is correct
/// for text and wrong for the other four.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Context {
    /// Character data between tags. `<` and `&` are the dangerous ones.
    Text,
    /// Inside a double-quoted attribute value. `"` and `&` are the dangerous
    /// ones; `<` is not, and escaping it there is harmless but not the point.
    Attribute,
    /// An attribute the HTML spec treats as a URL — `href`, `src`, `action`,
    /// `formaction`, `poster`, `cite`. A `javascript:` scheme here is a script
    /// tag with extra steps, and no amount of character escaping prevents it.
    Url,
    /// Inside a `style` attribute or element.
    Style,
    /// Emitted verbatim. Reachable only from a value that carries the
    /// capability, never from an ordinary string — see [`Part::RawHtml`].
    RawHtml,
}

impl Context {
    /// The context an attribute's value occupies, from its name.
    ///
    /// A table, because it is a fact about HTML rather than about this program.
    /// `href` is a URL in every document ever written.
    pub fn of_attribute(name: &str) -> Context {
        const URL_ATTRS: &[&str] = &[
            "href",
            "src",
            "action",
            "formaction",
            "poster",
            "cite",
            "data",
            "manifest",
            "ping",
            "srcset",
        ];
        let bare = name.rsplit(':').next().unwrap_or(name);
        if URL_ATTRS.contains(&bare) {
            Context::Url
        } else if bare == "style" {
            Context::Style
        } else {
            Context::Attribute
        }
    }
}

/// One dynamic hole in a template.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case", tag = "part")]
pub enum Part {
    /// `{expr}` between tags.
    Text {
        /// The value's identity — a path the renderer looks up, never an
        /// expression it evaluates. Evaluation is the program's job and it has
        /// already happened by the time bytes are produced.
        value: String,
        context: Context,
    },
    /// `class={expr}` — the whole value.
    Attribute {
        name: String,
        value: String,
        context: Context,
    },
    /// `disabled={expr}` where the attribute's presence is the value.
    ///
    /// Separate from `Attribute` because the rendering rule is different in
    /// kind: a false boolean attribute is ABSENT, not empty. `disabled=""` is
    /// disabled.
    BooleanAttribute { name: String, value: String },
    /// A region rendered only when a condition holds.
    Conditional {
        value: String,
        then: Vec<Chunk>,
        /// Empty when there is no `else`.
        otherwise: Vec<Chunk>,
    },
    /// A region rendered once per element of a collection.
    Each {
        collection: String,
        /// The name each element is bound to inside `body`.
        binding: String,
        /// The field that identifies an element, from `(item.id)`. `None` means
        /// the list declared no key — which markup rules reject for a mutable
        /// collection and permit for a static one.
        key: Option<String>,
        body: Vec<Chunk>,
    },
    /// Another template, rendered in place.
    Component {
        /// The resolved path, not the name as written.
        path: String,
        args: Vec<(String, String)>,
    },
    /// Verbatim bytes.
    ///
    /// Reachable only from a value whose type carries the raw-HTML capability.
    /// The `capability` field records which, so the artifact says on its face
    /// what authorised the bypass — an "it was fine when I wrote it" is not
    /// checkable and this is.
    RawHtml { value: String, capability: String },
    /// A construct this IR does not represent, or one an earlier analysis
    /// rejected.
    ///
    /// Rendering it is an error, not an empty string. `Outcome` again: recovery
    /// is not proof, and a renderer that silently omits what it did not
    /// understand produces a page that looks correct and is missing something.
    Blocked { reason: String, at: String },
}

/// A piece of a template: literal bytes, or a hole.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
// Adjacently tagged, not internally: an internally-tagged representation
// cannot encode a newtype variant holding a string, and `Static` is one. The
// alternative was to make it a struct variant for serde's benefit, which would
// let a serialization detail choose the shape of the IR.
#[serde(rename_all = "snake_case", tag = "chunk", content = "value")]
pub enum Chunk {
    /// Bytes emitted exactly, already escaped where they came from source text.
    Static(String),
    Dynamic(Part),
}

/// One renderable declaration.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Template {
    /// `Module.Name`.
    pub path: String,
    pub name: String,
    /// Parameters, in declaration order — the renderer's inputs.
    pub params: Vec<String>,
    pub chunks: Vec<Chunk>,
}

impl Template {
    /// Every `Blocked` part, anywhere in the tree.
    ///
    /// The renderer refuses a template with any, so this is what a caller asks
    /// before rendering rather than a diagnostic it recovers from.
    pub fn blocked(&self) -> Vec<&Part> {
        fn walk<'a>(chunks: &'a [Chunk], out: &mut Vec<&'a Part>) {
            for c in chunks {
                let Chunk::Dynamic(p) = c else { continue };
                match p {
                    Part::Blocked { .. } => out.push(p),
                    Part::Conditional {
                        then, otherwise, ..
                    } => {
                        walk(then, out);
                        walk(otherwise, out);
                    }
                    Part::Each { body, .. } => walk(body, out),
                    _ => {}
                }
            }
        }
        let mut out = Vec::new();
        walk(&self.chunks, &mut out);
        out
    }
}

/// Elements HTML forbids a closing tag on.
///
/// A fact about HTML, listed rather than guessed. Emitting `</img>` produces a
/// parse error the browser repairs by ignoring it — plausible bytes, different
/// tree.
pub const VOID_ELEMENTS: &[&str] = &[
    "area", "base", "br", "col", "embed", "hr", "img", "input", "link", "meta", "param", "source",
    "track", "wbr",
];

/// Attributes whose PRESENCE is their value.
pub const BOOLEAN_ATTRIBUTES: &[&str] = &[
    "allowfullscreen",
    "async",
    "autofocus",
    "autoplay",
    "checked",
    "controls",
    "default",
    "defer",
    "disabled",
    "formnovalidate",
    "hidden",
    "inert",
    "ismap",
    "itemscope",
    "loop",
    "multiple",
    "muted",
    "nomodule",
    "novalidate",
    "open",
    "playsinline",
    "readonly",
    "required",
    "reversed",
    "selected",
];

/// Build the template IR for every renderable declaration in a program.
pub fn build(hirs: &[&Hir]) -> Vec<Template> {
    let mut out = Vec::new();
    for hir in hirs {
        for (id, decl) in hir.all_decls() {
            use crate::hir::DeclKind::*;
            if !matches!(decl.kind, View | Component | Page) {
                continue;
            }
            let Some(body_id) = decl.body else { continue };
            let body = hir.body(body_id);
            let module = hir.module_of(id).unwrap_or_default();
            let mut chunks = Vec::new();
            for root in roots_of(body) {
                lower_node(body, root, &mut chunks);
            }
            out.push(Template {
                path: if module.is_empty() {
                    decl.name.clone()
                } else {
                    format!("{module}.{}", decl.name)
                },
                name: decl.name.clone(),
                params: decl.params.iter().map(|p| p.name.clone()).collect(),
                chunks: coalesce(chunks),
            });
        }
    }
    out.sort_by(|a, b| a.path.cmp(&b.path));
    out
}

fn roots_of(body: &Body) -> Vec<NodeId> {
    let mut roots = Vec::new();
    for e in body.walk() {
        if let Expr::Template { roots: r, .. } = body.expr(e) {
            roots.extend(r.iter().copied());
        }
    }
    roots
}

/// Adjacent `Static` chunks become one.
///
/// Not an optimisation: it is what makes the output deterministic to compare.
/// `["<p>", ">"]` and `["<p>>"]` are the same document and would otherwise be
/// different IR depending on how the source happened to be split.
fn coalesce(chunks: Vec<Chunk>) -> Vec<Chunk> {
    let mut out: Vec<Chunk> = Vec::new();
    for c in chunks {
        match (out.last_mut(), c) {
            (Some(Chunk::Static(prev)), Chunk::Static(next)) => prev.push_str(&next),
            (_, c) => out.push(c),
        }
    }
    out
}

fn lower_node(body: &Body, id: NodeId, out: &mut Vec<Chunk>) {
    match body.node(id) {
        Node::Text(t) => out.push(Chunk::Static(escape_static_text(t))),
        Node::Interpolation(e) => out.push(Chunk::Dynamic(Part::Text {
            value: crate::infer::path_of(body, *e),
            context: Context::Text,
        })),
        Node::Element {
            tag,
            attrs,
            children,
            self_closing,
        } => lower_element(body, tag, attrs, children, *self_closing, out),
        Node::Block {
            directive,
            children,
        } => lower_block(body, directive, children, out),
    }
}

fn lower_element(
    body: &Body,
    tag: &str,
    attrs: &[crate::hir::Attr],
    children: &[NodeId],
    self_closing: bool,
    out: &mut Vec<Chunk>,
) {
    out.push(Chunk::Static(format!("<{tag}")));

    // Attribute order is the SOURCE order, and it is preserved rather than
    // sorted. HTML gives attribute order no meaning, so either would be
    // correct — but the choice has to be made once and written down, or two
    // runs of the same compiler produce different bytes and content addressing
    // stops working. Source order is chosen because it is the one a reader can
    // predict from the file.
    for a in attrs {
        // An event handler is not markup. It is a behaviour the browser runtime
        // attaches, and E7-R owns it; emitting anything for it here would be
        // inventing an encoding the runtime does not yet have.
        if a.name.starts_with("on:") {
            out.push(Chunk::Dynamic(Part::Blocked {
                reason: format!(
                    "`{}` is an event handler; attaching behaviour is E7-R's, and \
                     this renderer emits markup only",
                    a.name
                ),
                at: format!("<{tag} {}>", a.name),
            }));
            continue;
        }
        match &a.value {
            AttrValue::None => {
                out.push(Chunk::Static(format!(" {}", a.name)));
            }
            AttrValue::Static(v) => {
                // The HIR keeps the value AS WRITTEN, quotes included, because
                // it is a faithful record of the source. The IR is a
                // description of a document, so the quotes are the syntax that
                // delimited the value and not part of it — leaving them in
                // renders `aria-label="&quot;Menu&quot;"`, which a screen
                // reader announces with the quote marks.
                out.push(Chunk::Static(format!(
                    " {}=\"{}\"",
                    a.name,
                    escape_static_attribute(unquote(v))
                )));
            }
            AttrValue::Expr(e) => {
                let value = crate::infer::path_of(body, *e);
                out.push(Chunk::Static(" ".to_string()));
                if BOOLEAN_ATTRIBUTES.contains(&a.name.as_str()) {
                    out.push(Chunk::Dynamic(Part::BooleanAttribute {
                        name: a.name.clone(),
                        value,
                    }));
                } else {
                    out.push(Chunk::Dynamic(Part::Attribute {
                        name: a.name.clone(),
                        value,
                        context: Context::of_attribute(&a.name),
                    }));
                }
            }
        }
    }

    if VOID_ELEMENTS.contains(&tag) {
        // `<br>`, not `<br/>` and not `<br></br>`. The slash is ignored by the
        // HTML parser and the closing tag is a parse error it repairs — both
        // produce plausible bytes and, for the second, a different tree.
        out.push(Chunk::Static(">".to_string()));
        return;
    }

    out.push(Chunk::Static(">".to_string()));
    for c in children {
        lower_node(body, *c, out);
    }
    // A closing tag for everything that is not void, whether or not the source
    // wrote `/>`. A first version branched on `self_closing` and produced the
    // same bytes on both sides, which is the shape of a decision that was never
    // made: HTML has no self-closing non-void element, so `<div/>` and `<div>`
    // open the same element and both need `</div>`.
    let _ = self_closing;
    out.push(Chunk::Static(format!("</{tag}>")));
}

fn lower_block(body: &Body, directive: &str, children: &[NodeId], out: &mut Vec<Chunk>) {
    let d = directive.trim();
    let mut inner = Vec::new();
    for c in children {
        lower_node(body, *c, &mut inner);
    }
    let inner = coalesce(inner);

    if let Some((binding, collection, key)) = parse_each(d) {
        out.push(Chunk::Dynamic(Part::Each {
            collection,
            binding,
            key,
            body: inner,
        }));
        return;
    }
    if let Some(cond) = parse_if(d) {
        out.push(Chunk::Dynamic(Part::Conditional {
            value: cond,
            then: inner,
            otherwise: Vec::new(),
        }));
        return;
    }
    // Not represented. Blocked rather than dropped: a region the IR does not
    // understand must not become an empty string, or the page renders without
    // it and looks correct.
    out.push(Chunk::Dynamic(Part::Blocked {
        reason: format!("the block directive `{d}` has no template-IR representation"),
        at: d.to_string(),
    }));
}

/// `{#each items as item (item.id)}` -> `("item", "items", Some("id"))`.
fn parse_each(d: &str) -> Option<(String, String, Option<String>)> {
    let inner = d.strip_prefix("{#each")?.strip_suffix('}')?.trim();
    let (collection, rest) = inner.split_once(" as ")?;
    let rest = rest.trim();
    let (binding, key) = match rest.split_once('(') {
        Some((b, k)) => {
            let k = k.trim_end_matches(')').trim();
            // `(item.id)` names a FIELD of the binding. The field is what the
            // runtime keys on, so it is stored alone rather than as an
            // expression the renderer would have to interpret.
            (b.trim(), k.rsplit('.').next().map(str::to_string))
        }
        None => (rest, None),
    };
    Some((binding.to_string(), collection.trim().to_string(), key))
}

/// `{#if signed_in}` -> `Some("signed_in")`.
fn parse_if(d: &str) -> Option<String> {
    let inner = d.strip_prefix("{#if")?.strip_suffix('}')?.trim();
    (!inner.is_empty()).then(|| inner.to_string())
}

/// A source attribute value without its delimiters.
///
/// Only a matched pair is removed. `"a"b"` keeps everything, because the value
/// is then not what the outer quotes suggest and guessing would change it.
fn unquote(v: &str) -> &str {
    let bytes = v.as_bytes();
    if bytes.len() >= 2
        && (bytes[0] == b'"' || bytes[0] == b'\'')
        && bytes[0] == bytes[bytes.len() - 1]
    {
        &v[1..v.len() - 1]
    } else {
        v
    }
}

/// Source text between tags, escaped once at build time.
fn escape_static_text(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    for c in s.chars() {
        match c {
            '&' => out.push_str("&amp;"),
            '<' => out.push_str("&lt;"),
            '>' => out.push_str("&gt;"),
            _ => out.push(c),
        }
    }
    out
}

fn escape_static_attribute(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    for c in s.chars() {
        match c {
            '&' => out.push_str("&amp;"),
            '"' => out.push_str("&quot;"),
            '<' => out.push_str("&lt;"),
            _ => out.push(c),
        }
    }
    out
}
