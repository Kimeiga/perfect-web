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

use std::collections::{BTreeMap, BTreeSet};

use serde::{Deserialize, Serialize};

use crate::hir::{AttrValue, Body, DeclId, Expr, ExprId, Hir, Node, NodeId};
use crate::resolve::{DefId, Resolution, Workspace};

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
    /// `href` is a URL in every document ever written, however it is spelled:
    /// HTML lowercases an attribute's name, so `HREF` is `href`. Until
    /// 2026-09-26 the name was read as written, and `HREF={msg}` was escaped
    /// as an ordinary attribute, which refuses no scheme (ADR-0095).
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
        let bare = name.rsplit(':').next().unwrap_or(name).to_ascii_lowercase();
        let bare = bare.as_str();
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
        ///
        /// **What authorises it.** Derived from the implementation, its
        /// dependencies, its capture schema and the platform ABI, so a changed
        /// handler is a different identity and the runtime refuses to resume
        /// into it.
        handler: String,
        /// The handler's NAME: which behaviour to load.
        ///
        /// A different question from the identity, and E7-L needs both. The
        /// identity says *may this handler attach here*; the name says *which
        /// code is it*. One value cannot answer both, because the identity
        /// changes when the implementation changes and the code to load is the
        /// same code either way — so a loader keyed on identity alone would
        /// have nothing to ask for.
        #[serde(default)]
        name: String,
        /// **What the handler reads of its captures**, as field paths:
        /// `item.id` in `resumable(captures = { item }) => add_to_cart(item.id,
        /// ..)`. The renderer serializes each path's value onto the element,
        /// nested by segment, and the compiled handler reads it there (E10,
        /// 2026-09-25). One derivation, `resume::capture_paths`, for both.
        /// Empty for a handler that reads nothing it captured.
        #[serde(default, skip_serializing_if = "Vec::is_empty")]
        captures: Vec<String>,
        /// **Where a capture's value is, when the handler names it otherwise**
        /// (ADR-0136): a handler written in a view captures the view's
        /// parameter, `item`, and the view composed into a page reads it as
        /// the path it was given, `entry`. The renderer reads `item.id` at
        /// `entry.id` and writes it under `item.id`, which is what the
        /// handler's module reads. Empty where every name is the handler's.
        #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
        renames: BTreeMap<String, String>,
        /// **What the runtime does in the listener, before any code loads**
        /// (ADR-0131): `prevent` and `stop`, from `on:submit|prevent`.
        #[serde(default, skip_serializing_if = "Vec::is_empty")]
        modifiers: Vec<String>,
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
        /// What identifies an element, as the path from the element to it:
        /// `id` from `(item.id)`, `r.id` from `(item.r.id)`, and empty from
        /// `(item)` (ADR-0073). `None` means the list declared no key — which
        /// markup rules reject for a mutable collection and permit for a
        /// static one.
        key: Option<String>,
        body: Vec<Chunk>,
    },
    /// `{#match value}{:Some(x)} .. {:None} .. {/match}` (ADR-0042): the arm
    /// whose case the value is, with the case's payload bound to the arm's
    /// name. A range part, as a conditional is.
    Match {
        id: PartId,
        value: String,
        arms: Vec<Arm>,
    },
    /// `<stream query={Q(args)}>` (ADR-0148): a region that shows its
    /// query's state. While a streamed query is pending it shows its
    /// placeholder, and then the arm the query settled to. A range part, as a
    /// block is.
    Stream {
        id: PartId,
        /// The query, as a component id: `store.page.Recommendations`.
        query: String,
        /// Each argument as a host computes it: a value's path, `id`, or an
        /// invocation-context call, `current_session()`.
        args: Vec<String>,
        /// `delivery streamed`: the document is sent before the query answers.
        streamed: bool,
        /// What shows while a streamed query is pending. Empty for one the
        /// page waits for.
        placeholder: Vec<Chunk>,
        /// `<ready as={items}>`: what the query answered.
        ready: StreamArm,
        /// `<failed as={why}>`: `Some(e)`, its declared error, or `None`, the
        /// host's failure.
        failed: StreamArm,
    },
    /// `href="/stores/{id}"` (ADR-0042): static text and values, each value
    /// escaped for the attribute's context. In a URL, each value is a URI
    /// component, so it cannot add a segment, a query, a fragment or a scheme.
    InterpolatedAttribute {
        id: PartId,
        owner: ElementId,
        name: String,
        segments: Vec<Segment>,
        context: Context,
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

/// One arm of a [`Part::Match`]: its case, the names its payload is bound
/// to, and what it renders.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Arm {
    /// `Some`, `None`, `Ok` or `Err`; a declared case by its WIT name,
    /// `circle` (ADR-0061).
    pub case: String,
    /// The payload of a case of one field.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub binding: Option<String>,
    /// Each field of a case of several, whose payload is a list of them
    /// (ADR-0061).
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub fields: Vec<String>,
    pub body: Vec<Chunk>,
}

/// One settled arm of a [`Part::Stream`] (ADR-0148): the name its value is
/// bound to, and what it renders.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct StreamArm {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub binding: Option<String>,
    pub body: Vec<Chunk>,
}

/// A piece of an interpolated attribute: text as written, or a value's path.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case", tag = "segment", content = "value")]
pub enum Segment {
    /// Escaped at build time, as a static attribute is.
    Static(String),
    Value(String),
}

impl Part {
    /// The chunk lists nested in this part, in document order. Every walk
    /// over the IR descends through this, so a part with regions cannot be
    /// missed by one walk and seen by another.
    pub fn nested(&self) -> Vec<&[Chunk]> {
        match self {
            Part::Conditional {
                then, otherwise, ..
            } => vec![then, otherwise],
            Part::Each { body, .. } => vec![body],
            Part::Match { arms, .. } => arms.iter().map(|a| a.body.as_slice()).collect(),
            Part::Stream {
                placeholder,
                ready,
                failed,
                ..
            } => vec![placeholder, &ready.body, &failed.body],
            _ => Vec::new(),
        }
    }

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
            | Part::Match { id, .. }
            | Part::Stream { id, .. }
            | Part::InterpolatedAttribute { id, .. }
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
            | Part::Event { owner, .. }
            | Part::InterpolatedAttribute { owner, .. } => Some(*owner),
            _ => None,
        }
    }

    /// How this kind of part is anchored.
    pub fn anchor(&self) -> Option<Anchor> {
        Some(match self {
            Part::Attribute { .. }
            | Part::BooleanAttribute { .. }
            | Part::Event { .. }
            | Part::InterpolatedAttribute { .. } => Anchor::Element,
            Part::Text { .. }
            | Part::Conditional { .. }
            | Part::Each { .. }
            | Part::Match { .. }
            | Part::Stream { .. }
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
            Part::Match { .. } => "match",
            Part::Stream { .. } => "stream",
            Part::InterpolatedAttribute { .. } => "interpolated_attribute",
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
    /// How many blocks (`{#each}`, `{#match}`, `{#if}`) enclose the node
    /// being lowered.
    depth: u32,
    /// Each text part, with the expression its hole holds (ADR-0122).
    holes: Vec<Hole>,
    /// How many names composition has renamed (ADR-0136), so each new name
    /// is one no other has.
    renamed: u32,
    /// How many `{#each}` enclose the node being lowered: a part inside one
    /// is an instance's, addressed with a frame (ADR-0142).
    frames: u32,
    /// Each view composed into the template, in the order first written.
    views: Vec<DefId>,
    /// The blocks around the node being lowered, outermost first (ADR-0144).
    blocks: Vec<PartId>,
    /// Each signal instance the template holds, in the order written
    /// (ADR-0144).
    instances: Vec<Instance>,
}

/// **A signal the template holds** (ADR-0144): a page's own, a composed
/// view's own at each use, and each `provide`'s. The browser holds one value
/// for each, by its name.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Instance {
    /// As the template reads it: `open` in the page's own markup, a fresh
    /// name no source can write at each use of a view, `open~2`.
    pub name: String,
    /// The file and the declaration whose body writes its first value.
    pub origin: (usize, DeclId),
    /// Its first value: a `signal`'s initializer, or a `provide`'s value.
    pub init: ExprId,
    /// Its type, as written.
    pub ty: InstanceType,
    /// The blocks it is inside, outermost first. Its value ends when one of
    /// them shows another arm (ADR-0130, R6).
    pub within: Vec<PartId>,
}

/// Where an instance's type is written (ADR-0144).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum InstanceType {
    /// A `signal x: T = e`'s `T`, in the origin's body.
    Written(crate::hir::TypeRefId),
    /// A module's `signal drawer: T`, which a `provide` gives.
    Declared(DefId),
}

/// **A text part and the expression it shows** (ADR-0122): what an optimistic
/// speculation recomputes in the browser. Recorded by the same traversal that
/// numbers the parts, so the two cannot disagree about which part is which.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Hole {
    pub part: PartId,
    /// The path the part reads, as the template names it: through each view
    /// composed around it, a view's parameter read as what it was given
    /// (ADR-0136).
    pub path: String,
    pub expr: ExprId,
    /// The file and the declaration whose body `expr` is in: the template's
    /// own, or a view composed into it (ADR-0136).
    pub origin: (usize, DeclId),
    /// Inside a block.
    pub nested: bool,
    /// Inside an `{#each}`: an instance's, whose address carries a frame the
    /// browser does not compute (ADR-0142).
    pub framed: bool,
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

    /// A name for `name` that nothing written can be: `~` is in no
    /// identifier (ADR-0136).
    fn rename(&mut self, name: &str) -> String {
        self.renamed += 1;
        format!("{name}~{}", self.renamed)
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
    /// loop; the handler IDENTITY for an event.
    pub value: String,
    /// The handler's name, for an event part. Which code to load, as opposed
    /// to which identity to authorise — see [`Part::Event`].
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub name: String,
    /// The event, for an event part: what the runtime listens for (ADR-0138).
    /// Until 2026-10-02 the manifest did not say, and the runtime listened
    /// for a click whatever the event.
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub event: String,
    /// What the runtime does in the listener, for an event part (ADR-0131).
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub modifiers: Vec<String>,
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
                        | Part::Conditional { value, .. }
                        | Part::Match { value, .. } => value.clone(),
                        // Every value the attribute reads, in order.
                        Part::InterpolatedAttribute { segments, .. } => segments
                            .iter()
                            .filter_map(|s| match s {
                                Segment::Value(v) => Some(v.as_str()),
                                Segment::Static(_) => None,
                            })
                            .collect::<Vec<_>>()
                            .join(" "),
                        Part::Each { collection, .. } => collection.clone(),
                        Part::Event { handler, .. } => handler.clone(),
                        Part::Component { path, .. } => path.clone(),
                        // The query whose state the region shows.
                        Part::Stream { query, .. } => query.clone(),
                        Part::Blocked { .. } => String::new(),
                    },
                    name: match p {
                        Part::Event { name, .. } => name.clone(),
                        _ => String::new(),
                    },
                    event: match p {
                        Part::Event { event, .. } => event.clone(),
                        _ => String::new(),
                    },
                    modifiers: match p {
                        Part::Event { modifiers, .. } => modifiers.clone(),
                        _ => Vec::new(),
                    },
                });
                for inner in p.nested() {
                    walk(inner, out);
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
                if let Part::Blocked { .. } = p {
                    out.push(p);
                }
                for inner in p.nested() {
                    walk(inner, out);
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

/// Handler identities by where the lambda is, from `resume_artifacts::located`.
///
/// Keyed by the file too (ADR-0135): a declaration's and an expression's
/// index are each counted within one file, so two pages of one shape in two
/// files had one key, and one page's button ran the other's handler.
pub type Handlers = std::collections::BTreeMap<
    (usize, crate::hir::DeclId, crate::hir::ExprId),
    (String, Vec<String>),
>;

/// Build the template IR for every renderable declaration in a program.
///
/// Without handler identities: an `Event` part gets an empty handler, which is
/// honest for a caller that has no signatures to derive one from.
pub fn build(hirs: &[&Hir]) -> Vec<Template> {
    build_with(hirs, &Handlers::new())
}

/// Build with the handler identities the resume artifacts derived.
///
/// The identity is derived ONCE, in `resume_artifacts`, and read here. A second
/// derivation could disagree with the manifest the runtime compares against,
/// and the disagreement would be silent: `decide` would refuse a handler that
/// is in fact the right one, and the page would simply not respond.
pub fn build_with(hirs: &[&Hir], handlers: &Handlers) -> Vec<Template> {
    // The views a template uses are found by name, from the file it is
    // written in (ADR-0136).
    let ws = Workspace::build(hirs);
    let mut out = Vec::new();
    for (unit, hir) in hirs.iter().enumerate() {
        for (id, decl) in hir.all_decls() {
            use crate::hir::DeclKind::*;
            if !matches!(decl.kind, View | Component | Page) {
                continue;
            }
            if let Some(l) = lowered(hirs, &ws, handlers, unit, id) {
                out.push(l.template);
            }
        }
    }
    out.sort_by(|a, b| a.path.cmp(&b.path));
    out
}

/// One declaration's template, as one traversal lowered it.
#[derive(Debug, Clone)]
pub struct Lowered {
    pub template: Template,
    /// Each text part and the expression it shows (ADR-0122).
    pub holes: Vec<Hole>,
    /// **Each view composed into it**, through the views it uses (ADR-0136):
    /// whose handlers are in its document.
    pub views: Vec<DefId>,
    /// **Each signal it holds** (ADR-0144).
    pub instances: Vec<Instance>,
}

/// **One renderable declaration's template, and its text holes** (ADR-0122),
/// numbered by one traversal: the one [`build_with`] numbers them by, with
/// each view it uses composed in place (ADR-0136).
pub fn lowered(
    hirs: &[&Hir],
    ws: &Workspace,
    handlers: &Handlers,
    unit: usize,
    id: DeclId,
) -> Option<Lowered> {
    let hir = hirs.get(unit)?;
    let decl = hir.decl(id);
    let body = hir.body(decl.body?);
    let module = hir.module_of(id).unwrap_or_default();
    let mut chunks = Vec::new();
    let mut ix = Indexer::default();
    // Its own signals, and the signals it provides, by the names it writes:
    // the outermost body's names are its own (ADR-0144).
    let mut provided = BTreeMap::new();
    let mut signals = BTreeSet::new();
    for (name, init, declared) in crate::page_values::signals_of(body) {
        if let Expr::Let { ty: Some(t), .. } = body.expr(declared) {
            ix.instances.push(Instance {
                name: name.clone(),
                origin: (unit, id),
                init,
                ty: InstanceType::Written(*t),
                within: Vec::new(),
            });
        }
        signals.insert(name);
    }
    for (name, signal, value) in provides_of(hirs, ws, unit, body) {
        ix.instances.push(Instance {
            name: name.clone(),
            origin: (unit, id),
            init: value,
            ty: InstanceType::Declared(signal),
            within: Vec::new(),
        });
        provided.insert(signal, name.clone());
        signals.insert(name);
    }
    let ctx = Lowering {
        handlers,
        hirs,
        ws,
        unit,
        decl: id,
        names: BTreeMap::new(),
        within: Vec::new(),
        provided,
        signals,
    };
    for root in roots_of(body) {
        lower_node(body, root, &ctx, &mut ix, &mut chunks);
    }
    let chunks = coalesce(chunks);
    let params: Vec<String> = decl.params.iter().map(|p| p.name.clone()).collect();
    let template = Template {
        path: if module.is_empty() {
            decl.name.clone()
        } else {
            format!("{module}.{}", decl.name)
        },
        name: decl.name.clone(),
        schema: schema_of(&params, &chunks),
        params,
        chunks,
    };
    Some(Lowered {
        template,
        holes: ix.holes,
        views: ix.views,
        instances: ix.instances,
    })
}

/// **Each text part of a renderable declaration, with its hole's expression**
/// (ADR-0122), numbered by the traversal [`build_with`] numbers them by.
pub fn holes(hirs: &[&Hir], ws: &Workspace, unit: usize, decl: DeclId) -> Vec<Hole> {
    lowered(hirs, ws, &Handlers::new(), unit, decl)
        .map(|l| l.holes)
        .unwrap_or_default()
}

/// What lowering needs beyond the body: which declaration it is in, the
/// handler identities derived for it, and the program its views are in.
#[derive(Clone)]
struct Lowering<'a> {
    handlers: &'a Handlers,
    hirs: &'a [&'a Hir],
    ws: &'a Workspace,
    /// Which file, by its index in the program, and which declaration in it:
    /// the body the nodes being lowered are written in.
    unit: usize,
    decl: DeclId,
    /// **How a name this body writes is read where the template renders it**
    /// (ADR-0136): a composed view's parameter as the path it was given, and
    /// a name the view binds renamed where it would hide one of those paths.
    /// Empty in a declaration's own markup, whose names are its own.
    names: BTreeMap<String, String>,
    /// The views composed around this one. A view that contains itself is
    /// refused, not followed.
    within: Vec<DefId>,
    /// **The instance each provided signal is, here** (ADR-0144): the
    /// nearest `provide` around this body, by the signal's declaration.
    provided: BTreeMap<DefId, String>,
    /// The names this body writes that are signals: its own, the ones it
    /// provides, and the ones provided around it. A handler's module names
    /// each as the body writes it, and the element says which instance it is.
    signals: BTreeSet<String>,
}

impl<'a> Lowering<'a> {
    /// `path` as the template reads it: its root as this scope names it.
    fn read(&self, path: String) -> String {
        let (root, rest) = match path.split_once('.') {
            Some((root, rest)) => (root, Some(rest)),
            None => (path.as_str(), None),
        };
        match (self.names.get(root), rest) {
            (Some(to), Some(rest)) => format!("{to}.{rest}"),
            (Some(to), None) => to.clone(),
            (None, _) => path,
        }
    }

    /// **The scope inside a block that binds `bound`**, and each name as
    /// the template binds it (ADR-0136). A bound name hides a parameter of
    /// its own name. In a composed view, one that is the first name of a
    /// path the view was given would hide it too: `{#each item.tags as
    /// entry}` given `item={entry}` would read the tag where it meant the
    /// page's `entry`. Such a name is renamed, to one no source can write.
    fn binding(&self, bound: &[String], ix: &mut Indexer) -> (Lowering<'a>, Vec<String>) {
        let mut inner = self.clone();
        if self.names.is_empty() {
            return (inner, bound.to_vec());
        }
        for b in bound {
            inner.names.remove(b);
        }
        let mut written = Vec::new();
        for b in bound {
            let hides = inner
                .names
                .values()
                .any(|p| p.split('.').next() == Some(b.as_str()));
            if hides {
                let fresh = ix.rename(b);
                inner.names.insert(b.clone(), fresh.clone());
                written.push(fresh);
            } else {
                written.push(b.clone());
            }
        }
        (inner, written)
    }

    /// The view a capitalised tag names, from the file it is written in.
    fn view_named(&self, tag: &str) -> Option<DefId> {
        if !tag.starts_with(|c: char| c.is_ascii_uppercase()) {
            return None;
        }
        match self.ws.resolve(self.unit, tag) {
            Resolution::Local(def) | Resolution::Imported { def, .. } => {
                crate::resolve::declaration(self.hirs, def)
                    .is_some_and(|d| d.kind == crate::hir::DeclKind::View)
                    .then_some(def)
            }
            _ => None,
        }
    }
}

/// **Is a view's body what composition writes in place?** Its markup
/// (ADR-0136), and the signals it holds and provides, of which each use holds
/// its own (ADR-0144). A view that binds other values of its own would need
/// them where it is used, and they are not there.
pub(crate) fn composable(body: &Body) -> bool {
    match body.expr(body.root) {
        Expr::Block { stmts } => stmts.iter().all(|s| {
            matches!(body.expr(*s), Expr::Template { .. })
                || body.signals.contains(s)
                || body.provides.contains(s)
        }),
        Expr::Template { .. } => true,
        _ => false,
    }
}

/// **What a body provides** (ADR-0144): each `provide`'s name as written,
/// the module signal it names, and its value. A `provide` of anything else
/// is PW5306's to refuse, and is left out.
pub(crate) fn provides_of(
    hirs: &[&Hir],
    ws: &Workspace,
    unit: usize,
    body: &Body,
) -> Vec<(String, DefId, ExprId)> {
    let mut out = Vec::new();
    for id in &body.provides {
        let Expr::Binary { lhs, rhs, .. } = body.expr(*id) else {
            continue;
        };
        let Expr::Name(name) = body.expr(*lhs) else {
            continue;
        };
        if let Some(signal) = module_signal(hirs, ws, unit, name) {
            out.push((name.clone(), signal, *rhs));
        }
    }
    out
}

/// **The module signals a body names**, each once, by the name it writes
/// (ADR-0144).
pub(crate) fn signals_named(
    hirs: &[&Hir],
    ws: &Workspace,
    unit: usize,
    body: &Body,
) -> Vec<(String, DefId)> {
    let mut out: Vec<(String, DefId)> = Vec::new();
    for (_, e, _) in body.exprs() {
        let Expr::Name(name) = e else { continue };
        if out.iter().any(|(n, _)| n == name) {
            continue;
        }
        if let Some(signal) = module_signal(hirs, ws, unit, name) {
            out.push((name.clone(), signal));
        }
    }
    out
}

/// The signal a module declares that `name` resolves to, where it is written.
pub(crate) fn module_signal(
    hirs: &[&Hir],
    ws: &Workspace,
    unit: usize,
    name: &str,
) -> Option<DefId> {
    match ws.resolve(unit, name) {
        Resolution::Local(def) | Resolution::Imported { def, .. } => {
            crate::resolve::declaration(hirs, def)
                .is_some_and(|d| d.kind == crate::hir::DeclKind::Signal)
                .then_some(def)
        }
        _ => None,
    }
}

/// The name of the thing a handler lambda calls.
///
/// `on:press={resumable(..) => add_to_cart(..)}` is `add_to_cart`. Empty when
/// the handler is not a lambda that calls a named thing — in which case there
/// is no separately loadable behaviour to name, and the runtime says so rather
/// than guessing.
pub(crate) fn called_name(body: &Body, expr: crate::hir::ExprId) -> String {
    let Expr::Lambda { body: inner, .. } = body.expr(expr) else {
        return String::new();
    };
    let Expr::Call { callee, .. } = body.expr(*inner) else {
        return String::new();
    };
    match body.expr(*callee) {
        Expr::Name(n) => n.clone(),
        _ => String::new(),
    }
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

/// **The path a template reads a value by**: a name, or fields read from one.
///
/// `None` for anything computed. The renderer looks each value up by its path
/// among the values it is given, and a computation has none. Until 2026-09-26
/// a text hole or an attribute that was not a path lowered with an empty path,
/// and `f(x).name` with the path `.name`, so every render failed; a program
/// that wrote one now fails to build instead (ADR-0073).
fn value_path(body: &Body, e: ExprId) -> Option<String> {
    match body.expr(e) {
        Expr::Name(n) => Some(n.clone()),
        Expr::Field { base, name } => value_path(body, *base).map(|b| format!("{b}.{name}")),
        _ => None,
    }
}

/// [`value_path`], for a checker: the path a template reads a value by, or
/// `None` for anything computed (ADR-0073, ADR-0136).
pub fn value_path_of(body: &Body, e: ExprId) -> Option<String> {
    value_path(body, e)
}

/// What a hole holds when it is not a path, for the reason it is refused.
fn computed(body: &Body, e: ExprId) -> &'static str {
    match body.expr(e) {
        Expr::Call { .. } => "a call",
        Expr::Binary { .. } | Expr::Unary { .. } => "an operation",
        Expr::Literal(_) | Expr::Interpolated { .. } => "a literal",
        Expr::Field { .. } => "a field of a computed value",
        _ => "a computed value",
    }
}

fn lower_node(body: &Body, id: NodeId, ctx: &Lowering<'_>, ix: &mut Indexer, out: &mut Vec<Chunk>) {
    match body.node(id) {
        Node::Text(t) => out.push(Chunk::Static(escape_static_text(t))),
        Node::Interpolation(e) => out.push(Chunk::Dynamic(match value_path(body, *e) {
            Some(value) => {
                let value = ctx.read(value);
                let id = ix.part();
                ix.holes.push(Hole {
                    part: id,
                    path: value.clone(),
                    expr: *e,
                    origin: (ctx.unit, ctx.decl),
                    nested: ix.depth > 0,
                    framed: ix.frames > 0,
                });
                Part::Text {
                    id,
                    value,
                    context: Context::Text,
                }
            }
            None => Part::Blocked {
                reason: format!(
                    "a template hole is read by path, and this is {}",
                    computed(body, *e)
                ),
                at: "{..}".to_string(),
            },
        })),
        Node::Element {
            tag,
            attrs,
            children,
            self_closing,
        } => lower_element(
            body,
            Element {
                tag,
                attrs,
                children,
                self_closing: *self_closing,
            },
            ctx,
            ix,
            out,
        ),
        Node::Block {
            directive,
            children,
            subject,
            ..
        } => lower_block(body, directive, children, *subject, ctx, ix, out),
        // A marker outside any block. The markup rules refuse it (PW5019).
        Node::Branch { marker, .. } => out.push(Chunk::Dynamic(Part::Blocked {
            reason: format!("`{marker}` stands outside any block"),
            at: marker.clone(),
        })),
    }
}

/// **Does this attribute mount a resource?** `resource={StoreMap}` does
/// (A-007): its element hosts the resource, and the element's other
/// attributes are the resource's arguments. RDFa's `resource="/x"` is an
/// ordinary attribute (ADR-0075).
pub(crate) fn mounts(a: &crate::hir::Attr) -> bool {
    a.name == "resource" && matches!(a.value, AttrValue::Expr(_))
}

/// The element being lowered, as one value.
///
/// Four of these travelled as separate parameters and clippy was right to
/// object: they are one thing — the element — and passing them apart invites a
/// call site that pairs the wrong tag with the wrong attributes.
struct Element<'a> {
    tag: &'a str,
    attrs: &'a [crate::hir::Attr],
    children: &'a [NodeId],
    self_closing: bool,
}

fn lower_element(
    body: &Body,
    el: Element<'_>,
    ctx: &Lowering<'_>,
    ix: &mut Indexer,
    out: &mut Vec<Chunk>,
) {
    let Element {
        tag,
        attrs,
        children,
        self_closing,
    } = el;
    // A region showing its query's state (ADR-0148). Until 2026-10-03 it
    // was refused here, and only the Marko adapter rendered one (ADR-0075).
    if tag == "stream" {
        lower_stream(body, attrs, children, ctx, ix, out);
        return;
    }
    // Not compiled by this renderer (ADR-0075): an element that mounts a
    // resource (A-007). Until 2026-09-26 it lowered as a literal element, a
    // `<map-container>` whose resource was never mounted.
    if attrs.iter().any(mounts) {
        out.push(Chunk::Dynamic(Part::Blocked {
            reason: "an element that mounts a resource is not compiled".to_string(),
            at: format!("<{tag} resource={{..}}>"),
        }));
        return;
    }
    // A view used in another is written where it is used (ADR-0136). A
    // capitalised tag that names no view is not an element either: until
    // 2026-09-26 it was written out as one (ADR-0072).
    if tag.starts_with(|c: char| c.is_ascii_uppercase()) {
        match ctx.view_named(tag) {
            Some(def) => compose(body, def, tag, attrs, ctx, ix, out),
            None => out.push(Chunk::Dynamic(Part::Blocked {
                reason: format!("`<{tag}>` names no view this build can see"),
                at: format!("<{tag}>"),
            })),
        }
        return;
    }
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
    // **Which instance each signal its handlers name is** (ADR-0144). A
    // handler's module is compiled once and names a signal as its body
    // writes it; where this use of a view holds the signal under another
    // name, the element says so, and the browser reads and sets that one.
    // Written here, at compile time, so no handler reaches another instance.
    let mut instances: BTreeMap<&str, &str> = BTreeMap::new();
    for a in attrs.iter().filter(|a| a.event().is_some()) {
        let AttrValue::Expr(handler) = &a.value else {
            continue;
        };
        for e in body.walk_from(*handler) {
            if let Expr::Name(n) = body.expr(e)
                && ctx.signals.contains(n)
                && let Some(to) = ctx.names.get(n)
                && to != n
            {
                instances.insert(n, to);
            }
        }
    }
    if !instances.is_empty() {
        let json = serde_json::to_string(&instances).expect("names serialize");
        out.push(Chunk::Static(format!(
            " data-pw-signals=\"{}\"",
            escape_static_attribute(&json)
        )));
    }

    // Attribute order is the SOURCE order, and it is preserved rather than
    // sorted. HTML gives attribute order no meaning, so either would be
    // correct — but the choice has to be made once and written down, or two
    // runs of the same compiler produce different bytes and content addressing
    // stops working. Source order is chosen because it is the one a reader can
    // predict from the file.
    //
    // Handlers come last, in their source order (ADR-0138): they write no
    // markup but what they capture, and an element's handlers carry it in
    // one `data-pw-captures` attribute, which the renderer writes once for
    // the run of them. Two would be one attribute written twice, and a
    // browser keeps the first.
    let (handlers, markup): (Vec<&crate::hir::Attr>, Vec<&crate::hir::Attr>) =
        attrs.iter().partition(|a| a.event().is_some());
    for a in markup.into_iter().chain(handlers) {
        // An event handler is not markup. It is a behaviour the browser runtime
        // attaches, and E7-R owns it; emitting anything for it here would be
        // inventing an encoding the runtime does not yet have.
        // An event handler is behaviour, not markup: nothing is written into
        // the element for it. The part records which element, which event and
        // which handler, and the browser runtime attaches after `decide`.
        if let Some((event, modifiers)) = a.event() {
            // The identity `resume_artifacts` derived for this exact lambda.
            // Empty when the handler is not resumable — an ordinary handler has
            // no resume manifest and nothing to compare against.
            // And what the document carries for it (ADR-0134): derived with
            // the identity, once, where names are resolved.
            let (handler, captures) = match &a.value {
                AttrValue::Expr(e) => ctx
                    .handlers
                    .get(&(ctx.unit, ctx.decl, *e))
                    .cloned()
                    .unwrap_or_default(),
                _ => (String::new(), Vec::new()),
            };
            // A capture the scope names otherwise is read where it is
            // (ADR-0136): in a composed view, a parameter or a renamed name.
            let renames: BTreeMap<String, String> = captures
                .iter()
                .filter_map(|c| {
                    let root = c.split('.').next()?;
                    let to = ctx.names.get(root)?;
                    (to != root).then(|| (root.to_string(), to.clone()))
                })
                .collect();
            let name = match &a.value {
                AttrValue::Expr(e) => called_name(body, *e),
                _ => String::new(),
            };
            out.push(Chunk::Dynamic(Part::Event {
                id: ix.part(),
                owner: owner.expect("an element with a handler owns an identity"),
                event: event.to_string(),
                handler,
                name,
                captures,
                renames,
                modifiers: modifiers.iter().map(|m| m.to_string()).collect(),
            }));
            continue;
        }
        // `style:width={w}` is a directive, and `on:` is the only one compiled.
        // Written out, it is an attribute named `style:width`, which a browser
        // ignores: until 2026-09-26 the style was silently not applied
        // (ADR-0073). An XML namespace (`xlink:href`) is an attribute's name.
        if let Some((prefix, _)) = a.name.split_once(':')
            && !matches!(prefix, "xml" | "xlink" | "xmlns")
        {
            out.push(Chunk::Dynamic(Part::Blocked {
                reason: format!(
                    "`{}` is a directive, and only `on:` directives are compiled",
                    a.name
                ),
                at: a.name.clone(),
            }));
            continue;
        }
        match &a.value {
            AttrValue::None => {
                out.push(Chunk::Static(format!(" {}", a.name)));
            }
            // A `{` that opens no expression: `title="{}"`. A string with
            // holes lowers as one (`AttrValue::Expr`, below); what is left
            // here is a brace the author may have meant as a hole, and writing
            // it out would publish `/stores/{id}` as an address, as happened
            // before ADR-0042.
            AttrValue::Static(v) if unquote(v).contains('{') => {
                out.push(Chunk::Dynamic(Part::Blocked {
                    reason: format!("`{}` has a `{{` that opens no expression", a.name),
                    at: format!("{}={}", a.name, v),
                }));
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
            // `href="/stores/{id}"` (ADR-0042).
            AttrValue::Expr(e) if matches!(body.expr(*e), Expr::Interpolated { .. }) => {
                out.push(Chunk::Static(" ".to_string()));
                let owner = owner.expect("an element with a dynamic attribute owns an identity");
                if let Expr::Interpolated { text, parts } = body.expr(*e) {
                    out.push(Chunk::Dynamic(interpolated_attribute(
                        body, &a.name, text, parts, owner, ctx, ix,
                    )));
                }
            }
            AttrValue::Expr(e) => {
                let Some(value) = value_path(body, *e).map(|v| ctx.read(v)) else {
                    out.push(Chunk::Dynamic(Part::Blocked {
                        reason: format!(
                            "`{}` is read by path, and this is {}",
                            a.name,
                            computed(body, *e)
                        ),
                        at: format!("{}={{..}}", a.name),
                    }));
                    continue;
                };
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
        lower_node(body, *c, ctx, ix, out);
    }
    // A closing tag for everything that is not void, whether or not the source
    // wrote `/>`. A first version branched on `self_closing` and produced the
    // same bytes on both sides, which is the shape of a decision that was never
    // made: HTML has no self-closing non-void element, so `<div/>` and `<div>`
    // open the same element and both need `</div>`.
    let _ = self_closing;
    out.push(Chunk::Static(format!("</{tag}>")));
}

/// **A view used in another, written where it is used** (ADR-0136): its
/// markup lowered in place, in the template's one numbering, each parameter
/// read as the path the prop gives it. What cannot be composed is a
/// `Blocked` part, as everything the IR cannot represent is; `pw check`
/// refuses each case first (PW5020, PW0619).
fn compose(
    body: &Body,
    def: DefId,
    tag: &str,
    attrs: &[crate::hir::Attr],
    ctx: &Lowering<'_>,
    ix: &mut Indexer,
    out: &mut Vec<Chunk>,
) {
    let blocked = |reason: String| {
        Chunk::Dynamic(Part::Blocked {
            reason,
            at: format!("<{tag}>"),
        })
    };
    let Some(decl) = crate::resolve::declaration(ctx.hirs, def) else {
        out.push(blocked(format!("`<{tag}>` names no declaration")));
        return;
    };
    let hir = ctx.hirs[def.unit];
    let Some(view) = decl.body.map(|b| hir.body(b)) else {
        out.push(blocked(format!("`{tag}` has no body")));
        return;
    };
    if ctx.within.contains(&def) {
        out.push(blocked(format!("`<{tag}>` contains itself")));
        return;
    }
    if !composable(view) {
        out.push(blocked(format!(
            "`{tag}` declares values of its own, and a composed view holds its markup and its \
             signals alone"
        )));
        return;
    }
    if let Some(a) = attrs
        .iter()
        .find(|a| !decl.params.iter().any(|p| p.name == a.name))
    {
        out.push(blocked(format!("`{tag}` takes no prop `{}`", a.name)));
        return;
    }
    let mut names = BTreeMap::new();
    for p in &decl.params {
        let given = attrs
            .iter()
            .find(|a| a.name == p.name)
            .and_then(|a| match &a.value {
                AttrValue::Expr(e) => value_path(body, *e).map(|v| ctx.read(v)),
                _ => None,
            });
        let Some(path) = given else {
            out.push(blocked(format!(
                "`{tag}`'s prop `{}` is given no value path",
                p.name
            )));
            return;
        };
        names.insert(p.name.clone(), path);
    }
    // Its own signals, and the signals it provides: each use holds its own,
    // named so no source can write it (ADR-0144). Each row of a loop would
    // need one of its own, which the browser does not hold yet (PW5307).
    let own = crate::page_values::signals_of(view);
    let gives = provides_of(ctx.hirs, ctx.ws, def.unit, view);
    if ix.frames > 0 && !(own.is_empty() && gives.is_empty()) {
        out.push(blocked(format!(
            "`<{tag}>` holds a signal, and a view used in a loop's row holds none yet"
        )));
        return;
    }
    let origin = (def.unit, DeclId(def.decl));
    let mut provided = ctx.provided.clone();
    let mut signals = BTreeSet::new();
    for (name, init, declared) in own {
        let Expr::Let { ty: Some(t), .. } = view.expr(declared) else {
            continue;
        };
        let fresh = ix.rename(&name);
        ix.instances.push(Instance {
            name: fresh.clone(),
            origin,
            init,
            ty: InstanceType::Written(*t),
            within: ix.blocks.clone(),
        });
        names.insert(name.clone(), fresh);
        signals.insert(name);
    }
    for (name, signal, value) in gives {
        let fresh = ix.rename(&name);
        ix.instances.push(Instance {
            name: fresh.clone(),
            origin,
            init: value,
            ty: InstanceType::Declared(signal),
            within: ix.blocks.clone(),
        });
        names.insert(name.clone(), fresh.clone());
        provided.insert(signal, fresh);
        signals.insert(name);
    }
    // A signal provided around it is the nearest `provide`'s instance. With
    // none, as in a view's own template, it is read by its name: a page that
    // provides nothing a view it uses needs is refused (PW5305).
    for (name, signal) in signals_named(ctx.hirs, ctx.ws, def.unit, view) {
        if signals.contains(&name) {
            continue;
        }
        let instance = ctx.provided.get(&signal).cloned().unwrap_or(name.clone());
        names.insert(name.clone(), instance);
        signals.insert(name);
    }
    if !ix.views.contains(&def) {
        ix.views.push(def);
    }
    let mut within = ctx.within.clone();
    within.push(def);
    let inner = Lowering {
        handlers: ctx.handlers,
        hirs: ctx.hirs,
        ws: ctx.ws,
        unit: def.unit,
        decl: DeclId(def.decl),
        names,
        within,
        provided,
        signals,
    };
    for root in roots_of(view) {
        lower_node(view, root, &inner, ix, out);
    }
}

/// **A `<stream>`** (ADR-0148): its query and arguments, and each of its
/// parts, lowered as a block's arms are. At the top of the template, or in a
/// view composed there: one inside a block or a loop's row is not rendered
/// yet, since nothing would render it again when the block shows another arm.
fn lower_stream(
    body: &Body,
    attrs: &[crate::hir::Attr],
    children: &[NodeId],
    ctx: &Lowering<'_>,
    ix: &mut Indexer,
    out: &mut Vec<Chunk>,
) {
    let blocked = |reason: &str| {
        Chunk::Dynamic(Part::Blocked {
            reason: reason.to_string(),
            at: "<stream>".to_string(),
        })
    };
    if ix.depth > 0 {
        out.push(blocked(
            "a `<stream>` inside a block or a loop's row is not rendered yet (ADR-0148)",
        ));
        return;
    }
    let Some(call) = crate::streams::query_attr(attrs) else {
        out.push(blocked("a `<stream>` names its query: `query={Q(..)}`"));
        return;
    };
    let Some((def, decl)) = crate::streams::stream_query(ctx.ws, ctx.hirs, ctx.unit, body, call)
    else {
        out.push(blocked("a `<stream>`'s `query` is a call of a query"));
        return;
    };
    let Expr::Call { args: given, .. } = body.expr(call) else {
        return;
    };
    let mut args = Vec::new();
    for a in given.iter().map(|a| a.value) {
        match (value_path(body, a), body.expr(a)) {
            (Some(path), _) => args.push(ctx.read(path)),
            // An invocation-context call: `current_session()`.
            (None, Expr::Call { callee, args: none }) if none.is_empty() => {
                args.push(format!("{}()", crate::infer::path_of(body, *callee)));
            }
            _ => {
                out.push(blocked(
                    "a stream's query is given values by path, or an invocation-context call",
                ));
                return;
            }
        }
    }
    let Some(query) = ctx.hirs.get(def.unit).and_then(|hir| {
        hir.all_decls()
            .find(|(id, _)| id.0 == def.decl)
            .map(|(id, _)| crate::contract::component_id(hir, id))
    }) else {
        out.push(blocked("a stream's query has no component"));
        return;
    };
    let id = ix.part();
    // A signal instance inside it belongs to it (ADR-0144).
    ix.blocks.push(id);
    let mut placeholder = Vec::new();
    let mut ready = StreamArm::default();
    let mut failed = StreamArm::default();
    for c in children {
        let Node::Element {
            tag,
            attrs,
            children,
            ..
        } = body.node(*c)
        else {
            continue;
        };
        let bound: Vec<String> = attrs
            .iter()
            .filter(|a| a.name == "as")
            .filter_map(|a| match &a.value {
                AttrValue::Expr(e) => match body.expr(*e) {
                    Expr::Name(n) => Some(n.clone()),
                    _ => None,
                },
                _ => None,
            })
            .collect();
        let (scope, mut written) = ctx.binding(&bound, ix);
        let lowered = lower_run(body, children, &scope, ix);
        match tag.as_str() {
            "placeholder" => placeholder = lowered,
            "ready" => {
                ready = StreamArm {
                    binding: written.pop(),
                    body: lowered,
                }
            }
            "failed" => {
                failed = StreamArm {
                    binding: written.pop(),
                    body: lowered,
                }
            }
            _ => {}
        }
    }
    ix.blocks.pop();
    out.push(Chunk::Dynamic(Part::Stream {
        id,
        query,
        args,
        streamed: crate::streams::streamed(decl),
        placeholder,
        ready,
        failed,
    }));
}

fn lower_block(
    body: &Body,
    directive: &str,
    children: &[NodeId],
    subject: Option<ExprId>,
    ctx: &Lowering<'_>,
    ix: &mut Indexer,
    out: &mut Vec<Chunk>,
) {
    // The block's own id comes BEFORE its children's, so document order and id
    // order agree — a patch stream that arrives in id order arrives in a order
    // the runtime can apply without buffering.
    let id = ix.part();
    // What a signal instance inside it belongs to (ADR-0144).
    ix.blocks.push(id);
    lower_block_at(body, id, directive, children, subject, ctx, ix, out);
    ix.blocks.pop();
}

#[allow(clippy::too_many_arguments)]
fn lower_block_at(
    body: &Body,
    id: PartId,
    directive: &str,
    children: &[NodeId],
    subject: Option<ExprId>,
    ctx: &Lowering<'_>,
    ix: &mut Indexer,
    out: &mut Vec<Chunk>,
) {
    let d = directive.trim();
    let (lead, branches) = split_branches(body, children);
    let blocked = |reason: String| {
        Chunk::Dynamic(Part::Blocked {
            reason,
            at: d.to_string(),
        })
    };
    // `c` in `{#if c}`: a value path, as every template value is.
    let subject_path = subject.and_then(|e| value_path(body, e).map(|v| ctx.read(v)));

    if let Some((binding, collection, key)) = parse_each(d) {
        let written = collection.split('.').map(str::trim).all(|s| {
            s.starts_with(|c: char| c.is_alphabetic() || c == '_')
                && s.chars().all(|c| c.is_alphanumeric() || c == '_')
        });
        if !written {
            out.push(blocked(format!(
                "`{{#each}}` reads its list by path, and `{collection}` is computed"
            )));
            return;
        }
        // The key as the path from the element to it: `id` from `(item.id)`,
        // `r.id` from `(item.r.id)`, and empty from `(item)`. Until 2026-09-26
        // it was the key's last segment, so `(item.r.id)` keyed on `item.id`
        // (ADR-0073). A key whose head is another name is PW5021.
        let key = match key {
            None => None,
            Some(k) => {
                let mut segments = k.split('.').map(str::trim);
                if segments.next() != Some(binding.as_str()) {
                    out.push(blocked(format!(
                        "a loop's key is `{binding}` or a field read from it, and this is `{k}`"
                    )));
                    return;
                }
                Some(segments.collect::<Vec<_>>().join("."))
            }
        };
        if !branches.is_empty() {
            out.push(blocked(
                "`{#each}` takes no branch marker; `{#if xs}` around the list says \
                 `{:else}`"
                    .to_string(),
            ));
            return;
        }
        let collection = ctx.read(collection);
        let (scope, mut written) = ctx.binding(std::slice::from_ref(&binding), ix);
        ix.frames += 1;
        let inner = lower_run(body, &lead, &scope, ix);
        ix.frames -= 1;
        out.push(Chunk::Dynamic(Part::Each {
            id,
            collection,
            binding: written.pop().unwrap_or(binding),
            key,
            body: inner,
        }));
        return;
    }
    if opens(d, "{#if") {
        let Some(value) = subject_path else {
            out.push(blocked(
                "an `{#if}` condition must be a value path".to_string(),
            ));
            return;
        };
        out.push(conditional(body, id, value, &lead, &branches, ctx, ix));
        return;
    }
    if opens(d, "{#match") {
        let Some(value) = subject_path else {
            out.push(blocked(
                "a `{#match}` subject must be a value path".to_string(),
            ));
            return;
        };
        let stray = lead.iter().any(|n| match body.node(*n) {
            Node::Text(t) => !t.trim().is_empty(),
            _ => true,
        });
        if stray {
            out.push(blocked(
                "a `{#match}` holds only its arms: nothing may come before the first".to_string(),
            ));
            return;
        }
        let mut arms = Vec::new();
        for (marker, run) in &branches {
            let Some(arm) = marker.arm else {
                out.push(blocked(
                    "a `{#match}` arm is `{:Some(x)}` or `{:None}`".to_string(),
                ));
                return;
            };
            // A declared case as a component value names it: `circle` for
            // `Circle`, as its WIT case is (ADR-0061). The language's own
            // four keep their names.
            let case = match arm.short() {
                c @ ("Some" | "None" | "Ok" | "Err") => c.to_string(),
                c => crate::wit::ident(c),
            };
            let (scope, written) = ctx.binding(&arm.bindings, ix);
            let (binding, fields) = match written.as_slice() {
                [one] => (Some(one.clone()), Vec::new()),
                many => (None, many.to_vec()),
            };
            arms.push(Arm {
                case,
                binding,
                fields,
                body: lower_run(body, run, &scope, ix),
            });
        }
        out.push(Chunk::Dynamic(Part::Match { id, value, arms }));
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

/// `{#each items as item (item.id)}` -> `("item", "items", Some("item.id"))`.
fn parse_each(d: &str) -> Option<(String, String, Option<String>)> {
    each_parts(d)
}

/// `{#each items as item (item.id)}` -> `("item", "items", Some("item.id"))`:
/// the directive's parts as written.
pub(crate) fn each_parts(d: &str) -> Option<(String, String, Option<String>)> {
    let inner = d.trim().strip_prefix("{#each")?.strip_suffix('}')?.trim();
    let (collection, rest) = inner.split_once(" as ")?;
    let rest = rest.trim();
    let (binding, key) = match rest.split_once('(') {
        Some((b, k)) => (b.trim(), Some(k.trim_end_matches(')').trim().to_string())),
        None => (rest, None),
    };
    Some((binding.to_string(), collection.trim().to_string(), key))
}

/// `href="/stores/{id}"`: static text and value paths, each value escaped for
/// the attribute's context when rendered (ADR-0042).
fn interpolated_attribute(
    body: &Body,
    name: &str,
    text: &str,
    parts: &[ExprId],
    owner: ElementId,
    ctx: &Lowering<'_>,
    ix: &mut Indexer,
) -> Part {
    let blocked = |reason: &str| Part::Blocked {
        reason: reason.to_string(),
        at: format!("{name}={text}"),
    };
    let context = Context::of_attribute(name);
    if BOOLEAN_ATTRIBUTES.contains(&name) {
        return blocked("a boolean attribute's value is its presence, not text");
    }
    if context == Context::Style {
        return blocked("escaping inside a CSS declaration is not decided (ADR-0042)");
    }
    let mut segments = Vec::new();
    let mut holes = parts.iter();
    let mut rest = unquote(text);
    while let Some(open) = rest.find('{') {
        let after = &rest[open + 1..];
        let Some(close) = after.find('}') else { break };
        if open > 0 {
            segments.push(Segment::Static(escape_static_attribute(&rest[..open])));
        }
        if after[..close].trim().is_empty() {
            return blocked("`{}` holds no expression");
        }
        let Some(hole) = holes.next() else {
            return blocked("a hole in the attribute did not parse");
        };
        let Some(path) = value_path(body, *hole) else {
            return blocked(
                "a hole in an attribute must be a value path, as `{..}` between tags is",
            );
        };
        segments.push(Segment::Value(ctx.read(path)));
        rest = &after[close + 1..];
    }
    if !rest.is_empty() {
        segments.push(Segment::Static(escape_static_attribute(rest)));
    }
    // The author's text fixes a URL's scheme and where its path starts; a
    // value only fills in components.
    if context == Context::Url && !matches!(segments.first(), Some(Segment::Static(_))) {
        return blocked(
            "a URL with holes must begin with text, which fixes its scheme; a URL \
             that is wholly a value is written `name={value}`",
        );
    }
    Part::InterpolatedAttribute {
        id: ix.part(),
        owner,
        name: name.to_string(),
        segments,
        context,
    }
}

/// Does the directive open a block named `head`: `{#if ..}`, not `{#iffy}`?
fn opens(d: &str, head: &str) -> bool {
    d.strip_prefix(head)
        .is_some_and(|r| r.starts_with(char::is_whitespace) || r.starts_with('}'))
}

/// A branch marker's fields: as written, its condition, and the arm it names.
struct Marker<'b> {
    written: &'b str,
    condition: Option<ExprId>,
    arm: &'b Option<crate::hir::TemplateArm>,
}

/// A block's children split at its branch markers: the run before the first
/// marker, then each marker with the run after it (ADR-0042).
fn split_branches<'b>(
    body: &'b Body,
    children: &[NodeId],
) -> (Vec<NodeId>, Vec<(Marker<'b>, Vec<NodeId>)>) {
    let mut lead = Vec::new();
    let mut branches: Vec<(Marker<'b>, Vec<NodeId>)> = Vec::new();
    for c in children {
        match (body.node(*c), branches.last_mut()) {
            (
                Node::Branch {
                    marker,
                    condition,
                    arm,
                },
                _,
            ) => branches.push((
                Marker {
                    written: marker,
                    condition: *condition,
                    arm,
                },
                Vec::new(),
            )),
            (_, Some((_, run))) => run.push(*c),
            (_, None) => lead.push(*c),
        }
    }
    (lead, branches)
}

/// Some nodes, lowered in order and coalesced.
fn lower_run(body: &Body, nodes: &[NodeId], ctx: &Lowering<'_>, ix: &mut Indexer) -> Vec<Chunk> {
    let mut out = Vec::new();
    ix.depth += 1;
    for n in nodes {
        lower_node(body, *n, ctx, ix, &mut out);
    }
    ix.depth -= 1;
    coalesce(out)
}

/// `{#if a} A {:else if b} B {:else} C {/if}`: a conditional on `a` whose
/// `otherwise` is a conditional on `b`, whose `otherwise` is C. Ids follow the
/// document: the nested conditional's comes after A's parts.
fn conditional(
    body: &Body,
    id: PartId,
    value: String,
    then: &[NodeId],
    rest: &[(Marker<'_>, Vec<NodeId>)],
    ctx: &Lowering<'_>,
    ix: &mut Indexer,
) -> Chunk {
    let then = lower_run(body, then, ctx, ix);
    let blocked = |reason: &str, at: &str| {
        vec![Chunk::Dynamic(Part::Blocked {
            reason: reason.to_string(),
            at: at.to_string(),
        })]
    };
    let otherwise = match rest.split_first() {
        None => Vec::new(),
        Some(((marker, run), more)) => match marker.condition {
            Some(c) => {
                let nested = ix.part();
                match value_path(body, c).map(|v| ctx.read(v)) {
                    Some(v) => {
                        // A block of its own: what its arms hold ends when
                        // it shows another (ADR-0144).
                        ix.blocks.push(nested);
                        let chunk = conditional(body, nested, v, run, more, ctx, ix);
                        ix.blocks.pop();
                        vec![chunk]
                    }
                    None => blocked(
                        "an `{:else if}` condition must be a value path",
                        marker.written,
                    ),
                }
            }
            None if marker.written.trim() == "{:else}"
                && marker.arm.is_none()
                && more.is_empty() =>
            {
                lower_run(body, run, ctx, ix)
            }
            None => blocked(
                "an `{#if}` takes `{:else if c}` markers and one `{:else}`, last",
                marker.written,
            ),
        },
    };
    Chunk::Dynamic(Part::Conditional {
        id,
        value,
        then,
        otherwise,
    })
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
                        Part::Event {
                            event,
                            handler,
                            renames,
                            modifiers,
                            ..
                        } => {
                            feed(h, event.as_bytes());
                            feed(h, handler.as_bytes());
                            for (from, to) in renames {
                                feed(h, from.as_bytes());
                                feed(h, to.as_bytes());
                            }
                            for m in modifiers {
                                feed(h, b"|");
                                feed(h, m.as_bytes());
                            }
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
                        Part::Match { value, arms, .. } => {
                            feed(h, value.as_bytes());
                            for a in arms {
                                feed(h, b"|");
                                feed(h, a.case.as_bytes());
                                feed(h, a.binding.as_deref().unwrap_or("-").as_bytes());
                                walk(h, &a.body);
                            }
                        }
                        Part::Stream {
                            query,
                            args,
                            streamed,
                            placeholder,
                            ready,
                            failed,
                            ..
                        } => {
                            feed(h, query.as_bytes());
                            for a in args {
                                feed(h, a.as_bytes());
                            }
                            feed(h, if *streamed { b"streamed" } else { b"at-once" });
                            walk(h, placeholder);
                            for arm in [ready, failed] {
                                feed(h, b"|");
                                feed(h, arm.binding.as_deref().unwrap_or("-").as_bytes());
                                walk(h, &arm.body);
                            }
                        }
                        Part::InterpolatedAttribute {
                            name,
                            segments,
                            context,
                            ..
                        } => {
                            feed(h, name.as_bytes());
                            for s in segments {
                                match s {
                                    Segment::Static(t) => {
                                        feed(h, b"S");
                                        feed(h, t.as_bytes());
                                    }
                                    Segment::Value(v) => {
                                        feed(h, b"V");
                                        feed(h, v.as_bytes());
                                    }
                                }
                            }
                            feed(h, format!("{context:?}").as_bytes());
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
