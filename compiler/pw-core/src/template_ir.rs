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

/// A part's identity within its template.
///
/// **Template-scoped and ordinal**, namespaced by [`Template::schema`].
/// Architect ruling, 2026-08-06:
///
/// > semantic identity of entire template + cheap structural identity inside
/// > that template
///
/// not a content hash per part. Two `<span>{price}</span>` parts have identical
/// IR and are different places in the document, so content identity cannot say
/// which to patch; adding enough parent context to disambiguate reinvents
/// structural position at a higher price.
///
/// Positional identity is safe here **because E7V already refuses across
/// schemas**. Local part 3 is never interpreted as local part 3 of an
/// incompatible template, so a source edit may renumber freely: it also changes
/// the schema, and a cross-version patch is rejected or migrated.
///
/// Assigned by deterministic traversal of the IR, never from source offsets. A
/// comment or a reflow must not perturb an id unless it changed the structure.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub struct PartId(pub u32);

impl std::fmt::Display for PartId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.0)
    }
}

/// An element that owns at least one element-local part.
///
/// Separate from `PartId` because one element can own several parts — two
/// dynamic attributes and a handler — and giving each its own comment pair
/// would cost six nodes to say one thing.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub struct ElementId(pub u32);

impl std::fmt::Display for ElementId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.0)
    }
}

/// How a part is anchored in the document.
///
/// Architect ruling: a `PartId` is a renderer concept and these are two wire
/// encodings of it, chosen per part KIND rather than one forced onto all.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Anchor {
    /// Comment boundaries: `<!--pw:s3-->` … `<!--pw:e3-->`.
    ///
    /// A range can be zero nodes, one text node, twenty `<li>`s or a component
    /// subtree. An attribute on an element cannot represent any of those, which
    /// is why conditionals, loops, components and text ranges use comments even
    /// though they cost two nodes.
    Range,
    /// The owning element carries `data-pw`.
    ///
    /// For attributes, boolean attributes and handlers, where the thing being
    /// updated belongs to an element that already exists.
    Element,
}

/// One dynamic hole in a template.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case", tag = "part")]
pub enum Part {
    /// `{expr}` between tags.
    Text {
        id: PartId,
        /// The value's identity — a path the renderer looks up, never an
        /// expression it evaluates. Evaluation is the program's job and it has
        /// already happened by the time bytes are produced.
        value: String,
        context: Context,
    },
    /// `class={expr}` — the whole value.
    Attribute {
        id: PartId,
        owner: ElementId,
        name: String,
        value: String,
        context: Context,
    },
    /// `disabled={expr}` where the attribute's presence is the value.
    ///
    /// Separate from `Attribute` because the rendering rule is different in
    /// kind: a false boolean attribute is ABSENT, not empty. `disabled=""` is
    /// disabled.
    BooleanAttribute {
        id: PartId,
        owner: ElementId,
        name: String,
        value: String,
    },
    /// `on:press={handler}` — behaviour the browser runtime attaches.
    ///
    /// Represented rather than `Blocked`, which is what E7-2 did: a renderer
    /// with no runtime had nothing to emit and refusing was the honest answer.
    /// E7-R has one, so the part carries what it needs — which element, which
    /// event, and which handler identity to authorise and attach.
    Event {
        id: PartId,
        owner: ElementId,
        /// `press`, from `on:press`. The semantic event, not a DOM event name:
        /// translating it is the runtime's job and it differs per element.
        event: String,
        /// The handler's identity, as the resume manifest names it.
        handler: String,
    },
    /// A region rendered only when a condition holds.
    Conditional {
        id: PartId,
        value: String,
        then: Vec<Chunk>,
        /// Empty when there is no `else`.
        otherwise: Vec<Chunk>,
    },
    /// A region rendered once per element of a collection.
    Each {
        id: PartId,
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
        id: PartId,
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
    RawHtml {
        id: PartId,
        value: String,
        capability: String,
    },
    /// A construct this IR does not represent, or one an earlier analysis
    /// rejected.
    ///
    /// Rendering it is an error, not an empty string. `Outcome` again: recovery
    /// is not proof, and a renderer that silently omits what it did not
    /// understand produces a page that looks correct and is missing something.
    Blocked { reason: String, at: String },
}

impl Part {
    /// This part's identity, or `None` for a `Blocked` one — which has no
    /// identity because it is never rendered.
    pub fn id(&self) -> Option<PartId> {
        Some(match self {
            Part::Text { id, .. }
            | Part::Attribute { id, .. }
            | Part::BooleanAttribute { id, .. }
            | Part::Event { id, .. }
            | Part::Conditional { id, .. }
            | Part::Each { id, .. }
            | Part::Component { id, .. }
            | Part::RawHtml { id, .. } => *id,
            Part::Blocked { .. } => return None,
        })
    }

    /// The element this part belongs to, for the element-anchored kinds.
    pub fn owner(&self) -> Option<ElementId> {
        match self {
            Part::Attribute { owner, .. }
            | Part::BooleanAttribute { owner, .. }
            | Part::Event { owner, .. } => Some(*owner),
            _ => None,
        }
    }

    /// How this kind of part is anchored.
    pub fn anchor(&self) -> Option<Anchor> {
        Some(match self {
            Part::Attribute { .. } | Part::BooleanAttribute { .. } | Part::Event { .. } => {
                Anchor::Element
            }
            Part::Text { .. }
            | Part::Conditional { .. }
            | Part::Each { .. }
            | Part::Component { .. }
            | Part::RawHtml { .. } => Anchor::Range,
            Part::Blocked { .. } => return None,
        })
    }

    pub fn kind(&self) -> &'static str {
        match self {
            Part::Text { .. } => "text",
            Part::Attribute { .. } => "attribute",
            Part::BooleanAttribute { .. } => "boolean_attribute",
            Part::Event { .. } => "event",
            Part::Conditional { .. } => "conditional",
            Part::Each { .. } => "each",
            Part::Component { .. } => "component",
            Part::RawHtml { .. } => "raw_html",
            Part::Blocked { .. } => "blocked",
        }
    }
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

/// Assigns every identity, once.
///
/// Architect ruling, 2026-08-06:
///
/// > One traversal assigns identity. Everybody else reads it.
///
/// The alternative — the server serializer numbering parts one way, the
/// manifest generator walking them another, the patch generator a third — is
/// the duplicated-traversal shape this project has found dangerous four times.
/// So identity is assigned during construction, in document order, and the
/// server renderer, the parts manifest, the patch generator and any devtools
/// read the same numbers.
#[derive(Debug, Default)]
struct Indexer {
    next_part: u32,
    next_element: u32,
}

impl Indexer {
    fn part(&mut self) -> PartId {
        let id = PartId(self.next_part);
        self.next_part += 1;
        id
    }

    fn element(&mut self) -> ElementId {
        let id = ElementId(self.next_element);
        self.next_element += 1;
        id
    }
}

/// One renderable declaration.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Template {
    /// `Module.Name`.
    pub path: String,
    pub name: String,
    /// Parameters, in declaration order — the renderer's inputs.
    pub params: Vec<String>,
    /// The template's **semantic** identity: what makes local part 3 mean
    /// something.
    ///
    /// Over the IR's structure — kinds, names, contexts, nesting, order — and
    /// not over source bytes, so a comment or a reflow does not change it and
    /// a reordered attribute does. E7V's scheme-2 reasoning, one layer up: a
    /// digest cannot say what made it, so the parts it namespaces are only
    /// comparable within one schema, and E7V's decision already refuses across
    /// schemas.
    pub schema: String,
    pub chunks: Vec<Chunk>,
}

/// One row of the parts manifest.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PartEntry {
    pub id: PartId,
    pub kind: &'static str,
    pub anchor: Anchor,
    /// Set for the element-anchored kinds.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub owner: Option<ElementId>,
    /// What the part reads, for a text or attribute part; the collection for a
    /// loop; the handler for an event.
    pub value: String,
}

impl Template {
    /// The **parts manifest**: dynamic regions only (charter §14 M7 task 4).
    ///
    /// What the browser runtime needs in order to find and update a part, and
    /// nothing else. Static regions do not appear, because there is nothing for
    /// the runtime to do with them — which is the same invariant that keeps
    /// them free of identity markup.
    ///
    /// Read from the same numbering the server renderer emitted, because there
    /// is one indexing pass and everybody reads it. A manifest generator that
    /// walked the IR again would be the second traversal.
    pub fn manifest(&self) -> Vec<PartEntry> {
        fn walk(chunks: &[Chunk], out: &mut Vec<PartEntry>) {
            for c in chunks {
                let Chunk::Dynamic(p) = c else { continue };
                let (Some(id), Some(anchor)) = (p.id(), p.anchor()) else {
                    continue;
                };
                out.push(PartEntry {
                    id,
                    kind: p.kind(),
                    anchor,
                    owner: p.owner(),
                    value: match p {
                        Part::Text { value, .. }
                        | Part::Attribute { value, .. }
                        | Part::BooleanAttribute { value, .. }
                        | Part::RawHtml { value, .. }
                        | Part::Conditional { value, .. } => value.clone(),
                        Part::Each { collection, .. } => collection.clone(),
                        Part::Event { handler, .. } => handler.clone(),
                        Part::Component { path, .. } => path.clone(),
                        Part::Blocked { .. } => String::new(),
                    },
                });
                match p {
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
        out.sort_by_key(|e| e.id);
        out
    }

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
            let mut ix = Indexer::default();
            for root in roots_of(body) {
                lower_node(body, root, &mut ix, &mut chunks);
            }
            let chunks = coalesce(chunks);
            let params: Vec<String> = decl.params.iter().map(|p| p.name.clone()).collect();
            out.push(Template {
                path: if module.is_empty() {
                    decl.name.clone()
                } else {
                    format!("{module}.{}", decl.name)
                },
                name: decl.name.clone(),
                schema: schema_of(&params, &chunks),
                params,
                chunks,
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

fn lower_node(body: &Body, id: NodeId, ix: &mut Indexer, out: &mut Vec<Chunk>) {
    match body.node(id) {
        Node::Text(t) => out.push(Chunk::Static(escape_static_text(t))),
        Node::Interpolation(e) => out.push(Chunk::Dynamic(Part::Text {
            id: ix.part(),
            value: crate::infer::path_of(body, *e),
            context: Context::Text,
        })),
        Node::Element {
            tag,
            attrs,
            children,
            self_closing,
        } => lower_element(body, tag, attrs, children, *self_closing, ix, out),
        Node::Block {
            directive,
            children,
        } => lower_block(body, directive, children, ix, out),
    }
}

fn lower_element(
    body: &Body,
    tag: &str,
    attrs: &[crate::hir::Attr],
    children: &[NodeId],
    self_closing: bool,
    ix: &mut Indexer,
    out: &mut Vec<Chunk>,
) {
    // An element gets an identity only if it OWNS something dynamic. Charter
    // §14 M7 task 3: stable IDs for dynamic parts "without making static HTML
    // noisy", and the invariant that gives it meaning is that a static region
    // receives no identity markup at all.
    let dynamic_owner = attrs
        .iter()
        .any(|a| a.name.starts_with("on:") || matches!(a.value, AttrValue::Expr(_)));
    let owner = dynamic_owner.then(|| ix.element());

    out.push(Chunk::Static(format!("<{tag}")));
    if let Some(o) = owner {
        out.push(Chunk::Static(format!(" data-pw=\"{o}\"")));
    }

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
        // An event handler is behaviour, not markup: nothing is written into
        // the element for it. The part records which element, which event and
        // which handler, and the browser runtime attaches after `decide`.
        if let Some((_, event)) = a.name.split_once(':')
            && a.name.starts_with("on:")
        {
            let handler = match &a.value {
                AttrValue::Expr(e) => crate::infer::path_of(body, *e),
                _ => String::new(),
            };
            out.push(Chunk::Dynamic(Part::Event {
                id: ix.part(),
                owner: owner.expect("an element with a handler owns an identity"),
                event: event.to_string(),
                handler,
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
                let owner = owner.expect("an element with a dynamic attribute owns an identity");
                if BOOLEAN_ATTRIBUTES.contains(&a.name.as_str()) {
                    out.push(Chunk::Dynamic(Part::BooleanAttribute {
                        id: ix.part(),
                        owner,
                        name: a.name.clone(),
                        value,
                    }));
                } else {
                    out.push(Chunk::Dynamic(Part::Attribute {
                        id: ix.part(),
                        owner,
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
        lower_node(body, *c, ix, out);
    }
    // A closing tag for everything that is not void, whether or not the source
    // wrote `/>`. A first version branched on `self_closing` and produced the
    // same bytes on both sides, which is the shape of a decision that was never
    // made: HTML has no self-closing non-void element, so `<div/>` and `<div>`
    // open the same element and both need `</div>`.
    let _ = self_closing;
    out.push(Chunk::Static(format!("</{tag}>")));
}

fn lower_block(
    body: &Body,
    directive: &str,
    children: &[NodeId],
    ix: &mut Indexer,
    out: &mut Vec<Chunk>,
) {
    let d = directive.trim();
    // The block's own id comes BEFORE its children's, so document order and id
    // order agree — a patch stream that arrives in id order arrives in a order
    // the runtime can apply without buffering.
    let id = ix.part();
    let mut inner = Vec::new();
    for c in children {
        lower_node(body, *c, ix, &mut inner);
    }
    let inner = coalesce(inner);

    if let Some((binding, collection, key)) = parse_each(d) {
        out.push(Chunk::Dynamic(Part::Each {
            id,
            collection,
            binding,
            key,
            body: inner,
        }));
        return;
    }
    if let Some(cond) = parse_if(d) {
        out.push(Chunk::Dynamic(Part::Conditional {
            id,
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

/// The template's semantic identity.
///
/// Over the IR's SHAPE — part kinds, names, contexts, static bytes, nesting and
/// order — because that is what makes a local part id mean something. Not over
/// source bytes: a comment or a reflow must not change it, and E7V's scheme-2
/// reasoning is the same argument one layer up.
///
/// FNV-1a. Not cryptographic: this distinguishes templates within a build, and
/// what refuses across builds is E7V's decision, which has its own hash.
fn schema_of(params: &[String], chunks: &[Chunk]) -> String {
    fn feed(h: &mut u64, bytes: &[u8]) {
        for b in bytes {
            *h ^= *b as u64;
            *h = h.wrapping_mul(0x0000_0100_0000_01b3);
        }
    }
    fn walk(h: &mut u64, chunks: &[Chunk]) {
        for c in chunks {
            match c {
                Chunk::Static(s) => {
                    feed(h, b"S");
                    feed(h, s.as_bytes());
                }
                Chunk::Dynamic(p) => {
                    feed(h, p.kind().as_bytes());
                    match p {
                        Part::Text { value, context, .. } => {
                            feed(h, value.as_bytes());
                            feed(h, format!("{context:?}").as_bytes());
                        }
                        Part::Attribute {
                            name,
                            value,
                            context,
                            ..
                        } => {
                            feed(h, name.as_bytes());
                            feed(h, value.as_bytes());
                            feed(h, format!("{context:?}").as_bytes());
                        }
                        Part::BooleanAttribute { name, value, .. } => {
                            feed(h, name.as_bytes());
                            feed(h, value.as_bytes());
                        }
                        Part::Event { event, handler, .. } => {
                            feed(h, event.as_bytes());
                            feed(h, handler.as_bytes());
                        }
                        Part::Conditional {
                            value,
                            then,
                            otherwise,
                            ..
                        } => {
                            feed(h, value.as_bytes());
                            walk(h, then);
                            feed(h, b"|");
                            walk(h, otherwise);
                        }
                        Part::Each {
                            collection,
                            binding,
                            key,
                            body,
                            ..
                        } => {
                            feed(h, collection.as_bytes());
                            feed(h, binding.as_bytes());
                            feed(h, key.as_deref().unwrap_or("-").as_bytes());
                            walk(h, body);
                        }
                        Part::Component { path, args, .. } => {
                            feed(h, path.as_bytes());
                            for (a, b) in args {
                                feed(h, a.as_bytes());
                                feed(h, b.as_bytes());
                            }
                        }
                        Part::RawHtml {
                            value, capability, ..
                        } => {
                            feed(h, value.as_bytes());
                            feed(h, capability.as_bytes());
                        }
                        Part::Blocked { reason, at } => {
                            feed(h, reason.as_bytes());
                            feed(h, at.as_bytes());
                        }
                    }
                }
            }
        }
    }
    let mut h: u64 = 0xcbf2_9ce4_8422_2325;
    for p in params {
        feed(&mut h, p.as_bytes());
    }
    walk(&mut h, chunks);
    format!("{h:016x}")
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
