//! The compiler's template IR, read as data.
//!
//! ADR-0018 again: `pw-core` emits it, this crate deserializes it, and neither
//! links the other. The types mirror `pw_core::template_ir` **by field name**,
//! which is what makes it a boundary rather than a coupling.
//!
//! The cost is that a field could be renamed on one side. `tests/contract.rs`
//! reads IR the real compiler produced, so a rename fails a test rather than
//! becoming an empty template at run time.

use serde::{Deserialize, Serialize};

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

/// One row of the parts manifest.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PartEntry {
    pub id: PartId,
    pub kind: String,
    pub anchor: Anchor,
    /// Set for the element-anchored kinds.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub owner: Option<ElementId>,
    /// What the part reads, for a text or attribute part; the collection for a
    /// loop; the handler for an event.
    pub value: String,
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
                    kind: p.kind().to_string(),
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
pub const VOID_ELEMENTS: &[&str] = &[
    "area", "base", "br", "col", "embed", "hr", "img", "input", "link", "meta", "param", "source",
    "track", "wbr",
];
