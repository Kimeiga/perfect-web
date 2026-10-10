//! **What a page shows, as reads of what its queries return** (ADR-0125).
//!
//! A page binds its queries and its template reads paths from the bindings:
//!
//! ```text
//! let cart = query Cart(current_session())     a binding: a query and its key
//! <p>{cart.line_count}</p>                     a part: the binding, then reads
//! ```
//!
//! A read is a record field, or a member function (ADR-0048): `line_count`
//! is `domain.line_count(cart)`, since `Cart` has no field of that name. Until
//! 2026-10-02 the development server computed every value a page showed in
//! Rust, and ran none of the store's queries. This module is the plan a host
//! follows instead: which component each binding runs with which arguments,
//! and, for each text part, the steps from the binding's value to the part's.
//! A member function a plan names is compiled as a component of its own, with
//! a contract of kind `function` ([`members`]).
//!
//! # What is refused, by name
//!
//! - a binding whose key is neither a page parameter, an invocation-context
//!   call (`current_session()`), nor a page signal (ADR-0152);
//! - a member read inside a block (`{#each}`, `{#match}`, `{#if}`): a host
//!   would call it per instance, which this plan does not state;
//! - a path that reads through something that is neither a field nor a
//!   member of the type it reads.

use std::collections::{BTreeMap, BTreeSet};

use crate::hir::{Body, DeclId, DeclKind, Expr, ExprId, Hir, Pattern};
use crate::resolve::{DefId, Namespace, Resolution, Workspace};
use crate::resolved::ResolvedType;
use crate::signatures::{Receiver, Signatures};

/// One page binding: `let cart = query Cart(current_session())`.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct Binding {
    pub binding: String,
    /// The query, as a component id: `store.page.Cart`.
    pub resource: String,
    /// Each argument as a host computes it: a page parameter's name, an
    /// invocation-context call, `current_session()`, or a page signal's name.
    pub args: Vec<String>,
    /// How a host runs the query: its declared policies (ADR-0127).
    pub policy: Policy,
    /// **The page signals among its arguments** (ADR-0152): the browser
    /// reads the binding again, for the new key, when one changes. Empty
    /// for a binding whose key the page never changes.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub signals: Vec<String>,
    /// **The declared errors that mean the page is not found** (ADR-0163),
    /// by their WIT case names: `not-found`. The page's `not_found_on`, for
    /// a binding whose query can answer it. A host answers 404 for them,
    /// and 503 for any other failure.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub not_found: Vec<String>,
    /// **The declared error that means the page's address is another
    /// address of the page** (ADR-0295): the page's `redirect_on`, for a
    /// binding whose query can answer it. A host answers it 308, or 307,
    /// to the page's route filled with what the case carries.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub redirect: Option<Redirect>,
}

/// A page's `redirect_on`, as a host reads it (ADR-0295).
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct Redirect {
    /// The case, by its WIT name: `moved`.
    pub case: String,
    /// 308 where the move is permanent, 307 where it is not.
    pub permanent: bool,
}

/// A query's policies, as a host applies them (ADR-0127).
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct Policy {
    /// `shared`, `private`, or `none`: whether a value is kept, and for whom.
    pub cache: String,
    /// `public`, `session` or `private`: whose value it is.
    pub privacy: String,
    /// How long a kept value is served, in milliseconds. Absent is 0: a value
    /// nobody said may be reused is not reused.
    pub freshness_ms: u64,
    /// The whole request's budget, retries included, in milliseconds.
    pub timeout_ms: Option<u64>,
    /// Attempts in all, 1 meaning no retry.
    pub attempts: u32,
    /// **Whether the delays between attempts vary** (ADR-0215), as the
    /// query's `retry` says. Absent where they do not.
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub jitter: bool,
    /// `concurrency parallel`: requests for one key do not share a flight.
    pub parallel: bool,
    /// The arguments the entry is keyed by, by position. A query with no
    /// `key` clause is keyed by all of them, so two calls with different
    /// arguments never share an entry.
    pub key: Vec<usize>,
    /// **What a key's stale work does** (ADR-0152): `cancel`, `supersede` or
    /// `keep`, as declared. Absent where the query declares none.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub on_key_change: Option<String>,
    /// **What a read whose origin fails is answered with** (ADR-0177):
    /// `last_known_good` or `empty`, as declared. Absent where the query
    /// declares none.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub fallback: Option<String>,
}

fn policy_of(decl: &crate::hir::Decl) -> Policy {
    use crate::manifest::{CachePartition, Concurrency, Privacy, Retry};
    let m = crate::manifest::of(decl);
    let key = match decl.policy("key") {
        Some(p) => p
            .value
            .split(',')
            .filter_map(|name| decl.params.iter().position(|q| q.name == name.trim()))
            .collect(),
        None => (0..decl.params.len()).collect(),
    };
    Policy {
        cache: match m.cache_partition {
            CachePartition::Shared => "shared",
            CachePartition::Private => "private",
            CachePartition::None => "none",
        }
        .to_string(),
        privacy: match m.privacy {
            Privacy::Public => "public",
            Privacy::Session => "session",
            Privacy::Private => "private",
        }
        .to_string(),
        freshness_ms: m.freshness.unwrap_or(0),
        timeout_ms: m.timeout,
        attempts: match m.retry {
            Retry::Bounded { max, .. } => max.max(1),
            Retry::None | Retry::Forever => 1,
        },
        // Whether its delays vary, as its `retry` says (ADR-0215). Its
        // strategy needs nothing more: `bounded_exponential` and
        // `transport_only` retry a query alike, since a declared error is an
        // answer and only a failed read is tried again.
        jitter: match m.retry {
            Retry::Bounded { jitter, .. } => jitter,
            Retry::None | Retry::Forever => false,
        },
        parallel: m.concurrency == Some(Concurrency::Parallel),
        key,
        on_key_change: decl
            .policy("on_key_change")
            .map(|p| p.value.trim().to_string()),
        fallback: decl.policy("fallback").map(|p| p.value.trim().to_string()),
    }
}

/// One step from a value to the next.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Step {
    /// A record field, by its Pleris name.
    Field(String),
    /// A member function, called with the value: its component id.
    Member(String),
    /// **A computed hole's function** (ADR-0226), called with the value it
    /// reads: its component id.
    Derived(String),
}

/// **A computed part a host computes** (ADR-0226, ruling 0073-a): the pure
/// function the compiler lifts its expression into, compiled as a component
/// of its own, and run with the one value it reads.
#[derive(Debug, Clone)]
pub struct Derived {
    /// Its component's id: `feed.app.PostPage.derived_3`. No declaration
    /// can be named so: the page is no module.
    pub component_id: String,
    /// The page whose part it is.
    pub page: DefId,
    pub part: u32,
    /// Where its expression is written: the page's body, or a view's
    /// composed in it.
    pub origin: (usize, DeclId),
    pub expr: ExprId,
    /// Its input: the name its expression reads it by, and its type there.
    pub input: (String, ResolvedType),
    /// What it computes.
    pub result: ResolvedType,
}

impl Derived {
    /// Its function's name, as the program's lowering names it: with a `$`,
    /// which no declaration's name has, and a JavaScript name may.
    pub fn export(&self) -> String {
        format!("derived${}", self.part)
    }

    /// The declaration its expression is written in, which its lowered
    /// function is of.
    pub fn declaration(&self) -> DefId {
        DefId {
            unit: self.origin.0,
            decl: self.origin.1.0,
        }
    }

    /// What it takes and answers, as a contract's and a world's export is
    /// typed.
    pub fn interface(&self) -> crate::binding::Interface {
        crate::binding::Interface {
            params: vec![Some(crate::resolved::TypeResolution::Resolved(
                self.input.1.clone(),
            ))],
            returns: Some(crate::resolved::TypeResolution::Resolved(
                self.result.clone(),
            )),
        }
    }
}

/// One text part outside any block: what it shows.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct Part {
    pub part: u32,
    /// The template's path for it, as the renderer is given values.
    pub path: String,
    pub binding: String,
    pub steps: Vec<Step>,
}

/// **A page's signal** (ADR-0130): its name and its first value, as the
/// browser's compiled modules read a value (`js_pure`'s wire form). The
/// server renders the first value; the browser holds it from there.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct Signal {
    pub name: String,
    pub initial: serde_json::Value,
}

/// **A part a signal decides** (ADR-0130): the browser renders it again when
/// the signal changes. A text or attribute part reads `path`; a conditional
/// or match part is a block the signal chooses between.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct Live {
    pub part: u32,
    pub signal: String,
    pub path: String,
    /// The part's kind, as the parts manifest names it.
    pub kind: String,
    /// The signals a block is rendered again for: its own, and each read
    /// where the browser does not set a part in place (ADR-0142). Empty for
    /// a text part or an attribute, which reads `signal` alone.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub reads: Vec<String>,
    /// The attribute, for an attribute part: `value`, `disabled` (ADR-0142).
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub attribute: String,
    /// **The signals a block's arms hold** (ADR-0144): each use of a view
    /// inside it holds its own. When the block shows another arm, they end,
    /// and start again at their first values (ADR-0130, R6).
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub owns: Vec<String>,
    /// **A value computed from the signal** (ADR-0227): the component that
    /// computes it, which a host runs with the signal's first value, and
    /// whose function the page's module runs in the browser with the value
    /// at `path` each time the signal changes. Empty for a part that reads
    /// the signal as it is.
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub derived: String,
}

/// The part numbered `id`, wherever it is in `chunks`.
fn find_part(chunks: &[crate::template_ir::Chunk], id: u32) -> Option<&crate::template_ir::Part> {
    use crate::template_ir::Chunk;
    for c in chunks {
        let Chunk::Dynamic(p) = c else { continue };
        if p.id().is_some_and(|i| i.0 == id) {
            return Some(p);
        }
        // Through every region a part has, a stream's arms too (ADR-0148).
        if let Some(inner) = p.nested().into_iter().find_map(|r| find_part(r, id)) {
            return Some(inner);
        }
    }
    None
}

/// Where a part is, for what the browser renders again (ADR-0137).
#[derive(Debug, Clone)]
enum Reach {
    /// Outside any block: a text part here is rendered again by its range.
    Top,
    /// Inside a block a value other than a signal decides: an instance the
    /// browser does not address.
    Frame,
    /// Inside a block a signal decides, which the browser renders whole from
    /// the page's signals; with the names bound inside it.
    Live(Vec<String>),
}

/// **What the browser renders again, it can** (ADR-0137). The browser holds
/// a page's signals and nothing else, and renders again two kinds of part: a
/// text part outside any block, by its range, and a block a signal decides,
/// whole, from the signals' values (ADR-0133). A signal read anywhere else
/// would show its first value forever, and a block the browser renders that
/// reads anything but the signals and its own names would not render. Each
/// is refused, by its part, before a page is served with it.
fn rendered_again(
    chunks: &[crate::template_ir::Chunk],
    signals: &[String],
    // Each block subject the browser computes from a signal (ADR-0229): its
    // block is one a signal decides.
    browser: &[String],
    reach: &Reach,
) -> Result<(), String> {
    use crate::template_ir::{Chunk, Part, Segment};
    let root = |path: &str| path.split('.').next().unwrap_or_default().to_string();
    let signal = |path: &str| signals.contains(&root(path));
    for c in chunks {
        let Chunk::Dynamic(p) = c else { continue };
        let id = p.id().map(|i| i.0).unwrap_or_default();
        // What the part reads itself, not inside its regions.
        let own: Vec<String> = match p {
            Part::Text { value, .. }
            | Part::Attribute { value, .. }
            | Part::BooleanAttribute { value, .. }
            | Part::RawHtml { value, .. }
            | Part::Conditional { value, .. }
            | Part::Match { value, .. } => vec![value.clone()],
            Part::Each { collection, .. } => vec![collection.clone()],
            Part::InterpolatedAttribute { segments, .. } => segments
                .iter()
                .filter_map(|s| match s {
                    Segment::Value(v) => Some(v.clone()),
                    Segment::Static(_) => None,
                })
                .collect(),
            Part::Instance { args, .. } => args.iter().map(|(_, v)| v.clone()).collect(),
            Part::Title { pieces, .. } => title_reads(pieces),
            Part::Meta { content, .. } => title_reads(content),
            // A stream's query's arguments, each a value's path or an
            // invocation-context call (ADR-0148).
            Part::Stream { args, .. } => {
                args.iter().filter(|a| !a.ends_with(')')).cloned().collect()
            }
            // What the document carries for a handler, read where the
            // template holds it (ADR-0136).
            Part::Event {
                captures, renames, ..
            } => captures
                .iter()
                .map(|c| match c.split_once('.') {
                    Some((r, rest)) => renames
                        .get(r)
                        .map_or_else(|| c.clone(), |to| format!("{to}.{rest}")),
                    None => renames.get(c).cloned().unwrap_or_else(|| c.clone()),
                })
                .collect(),
            Part::Blocked { .. } => Vec::new(),
        };
        match reach {
            Reach::Live(bound) => {
                // A value the template computes is read by a path no source
                // writes (ADR-0226): said for what it is.
                if own.iter().any(|v| v.starts_with('#')) {
                    return Err(format!(
                        "part {id} computes a value inside a block a signal decides, and the \
                         browser, which renders that block again, computes none yet (ruling 0073-a)"
                    ));
                }
                if let Some(other) = own.iter().find(|v| !signal(v) && !bound.contains(&root(v))) {
                    return Err(format!(
                        "part {id} reads `{other}` inside a block a signal decides, and the \
                         browser renders that block again from the page's signals alone \
                         (ADR-0137)"
                    ));
                }
            }
            Reach::Top | Reach::Frame => {
                if let Some(s) = own.iter().find(|v| signal(v)) {
                    let s = root(s);
                    match (p, reach) {
                        (
                            Part::Text { .. } | Part::Conditional { .. } | Part::Match { .. },
                            Reach::Top,
                        ) => {}
                        // A view's instance, rendered again whole in the
                        // browser, from the page's signals alone (ADR-0203).
                        (Part::Instance { .. }, Reach::Top) => {
                            if let Some(other) = own.iter().find(|v| !signal(v)) {
                                return Err(format!(
                                    "part {id} is an instance given the signal `{s}` and \
                                     `{other}`, and the browser renders it again from the \
                                     page's signals alone (ADR-0203)"
                                ));
                            }
                        }
                        // Set in place (ADR-0142).
                        (Part::Attribute { .. } | Part::BooleanAttribute { .. }, Reach::Top)
                            if set_in_place(p) => {}
                        (
                            Part::Attribute { .. }
                            | Part::BooleanAttribute { .. }
                            | Part::InterpolatedAttribute { .. },
                            Reach::Top,
                        ) => {
                            return Err(format!(
                                "part {id} is an attribute a signal decides whose value is a URL \
                                 or a style, or written with holes, which the browser does not \
                                 set again: it would check what the server checks a second way \
                                 (ADR-0142)"
                            ));
                        }
                        (Part::Event { .. }, _) => {
                            return Err(format!(
                                "part {id}'s handler captures the signal `{s}` as the page was \
                                 rendered; a handler reads a signal through its context (ADR-0133)"
                            ));
                        }
                        (Part::Each { .. }, Reach::Top) => {
                            return Err(format!(
                                "part {id} is a list the signal `{s}` holds, which the browser \
                                 does not render again outside a block a signal decides (ADR-0137)"
                            ));
                        }
                        (_, Reach::Frame) => {
                            return Err(format!(
                                "part {id} reads the signal `{s}` inside a block a value other \
                                 than a signal decides, which the browser does not render again \
                                 (ADR-0137)"
                            ));
                        }
                        _ => {
                            return Err(format!(
                                "part {id} is a {} part the signal `{s}` decides, which the \
                                 browser does not render again (ADR-0137)",
                                p.kind()
                            ));
                        }
                    }
                }
            }
        }
        // Into the part's regions.
        match p {
            Part::Each { binding, body, .. } => {
                let inner = match reach {
                    Reach::Live(bound) => {
                        Reach::Live([bound.clone(), vec![binding.clone()]].concat())
                    }
                    _ => Reach::Frame,
                };
                rendered_again(body, signals, browser, &inner)?;
            }
            Part::Conditional {
                value,
                then,
                otherwise,
                ..
            } => {
                let inner = match reach {
                    Reach::Live(bound) => Reach::Live(bound.clone()),
                    Reach::Top if signal(value) || browser.contains(value) => {
                        Reach::Live(Vec::new())
                    }
                    _ => Reach::Frame,
                };
                rendered_again(then, signals, browser, &inner)?;
                rendered_again(otherwise, signals, browser, &inner)?;
            }
            Part::Match { value, arms, .. } => {
                for a in arms {
                    let names: Vec<String> = a.binding.iter().chain(&a.fields).cloned().collect();
                    let inner = match reach {
                        Reach::Live(bound) => Reach::Live([bound.clone(), names].concat()),
                        Reach::Top if signal(value) || browser.contains(value) => {
                            Reach::Live(names)
                        }
                        _ => Reach::Frame,
                    };
                    rendered_again(&a.body, signals, browser, &inner)?;
                }
            }
            _ => {}
        }
    }
    Ok(())
}

/// **What a part reads itself**, not inside its regions: each path, as the
/// template names it.
fn own_reads(p: &crate::template_ir::Part) -> Vec<String> {
    use crate::template_ir::{Part, Segment};
    match p {
        Part::Text { value, .. }
        | Part::Attribute { value, .. }
        | Part::BooleanAttribute { value, .. }
        | Part::RawHtml { value, .. }
        | Part::Conditional { value, .. }
        | Part::Match { value, .. } => vec![value.clone()],
        Part::Each { collection, .. } => vec![collection.clone()],
        Part::InterpolatedAttribute { segments, .. } => segments
            .iter()
            .filter_map(|s| match s {
                Segment::Value(v) => Some(v.clone()),
                Segment::Static(_) => None,
            })
            .collect(),
        Part::Instance { args, .. } => args.iter().map(|(_, v)| v.clone()).collect(),
        Part::Stream { args, .. } => args.iter().filter(|a| !a.ends_with(')')).cloned().collect(),
        Part::Title { pieces, .. } => title_reads(pieces),
        Part::Meta { content, .. } => title_reads(content),
        Part::Event { .. } | Part::Blocked { .. } => Vec::new(),
    }
}

/// **What a page's title reads** (ADR-0183), or its metadata's content
/// (ADR-0186): each value among its pieces.
fn title_reads(pieces: &[crate::template_ir::TitlePiece]) -> Vec<String> {
    use crate::template_ir::TitlePiece;
    pieces
        .iter()
        .filter_map(|p| match p {
            TitlePiece::Value(v) => Some(v.clone()),
            TitlePiece::Text(_) => None,
        })
        .collect()
}

/// **Does the browser set this part in place when its signal changes?**
/// (ADR-0142): a text part, and an attribute whose value is neither a URL nor
/// a style, outside any loop.
fn set_in_place(p: &crate::template_ir::Part) -> bool {
    use crate::template_ir::{Context, Part};
    match p {
        Part::Text { .. } | Part::BooleanAttribute { .. } => true,
        // And a `<textarea>`'s value, which the browser sets as the element's
        // value, written as its text (ADR-0221).
        Part::Attribute { context, .. } => {
            matches!(context, Context::Attribute | Context::Content)
        }
        _ => false,
    }
}

/// **The signals a block a signal decides is rendered again for**
/// (ADR-0142): its own, and each read where the browser does not set a part
/// in place, inside a loop, or by a part it does not set. A text part or an
/// attribute outside any loop is set in place, and a block a signal decides
/// renders itself. Until 2026-10-02 a block rendered again for every signal
/// read anywhere in it, so a field bound to a signal inside one was replaced
/// at each key pressed, and lost its focus.
fn block_reads(part: &crate::template_ir::Part, signals: &[String]) -> Vec<String> {
    use crate::template_ir::{Chunk, Part};
    let root = |p: &str| p.split('.').next().unwrap_or_default().to_string();
    fn walk(
        chunks: &[Chunk],
        signals: &[String],
        framed: bool,
        root: &dyn Fn(&str) -> String,
        out: &mut BTreeSet<String>,
    ) {
        for c in chunks {
            let Chunk::Dynamic(p) = c else { continue };
            let decided = matches!(p, Part::Conditional { .. } | Part::Match { .. })
                && own_reads(p).iter().any(|v| signals.contains(&root(v)));
            // A block a signal decides, outside a loop, renders itself.
            if decided && !framed {
                continue;
            }
            if framed || !set_in_place(p) {
                out.extend(
                    own_reads(p)
                        .iter()
                        .map(|v| root(v))
                        .filter(|r| signals.contains(r)),
                );
            }
            let inner = framed || matches!(p, Part::Each { .. });
            for region in p.nested() {
                walk(region, signals, inner, root, out);
            }
        }
    }
    let mut out: BTreeSet<String> = own_reads(part)
        .iter()
        .map(|v| root(v))
        .filter(|r| signals.contains(r))
        .collect();
    for region in part.nested() {
        walk(region, signals, false, &root, &mut out);
    }
    out.into_iter().collect()
}

/// **Each attribute a signal decides that the browser sets in place**
/// (ADR-0142): outside any loop, at top level or in a block a signal decides.
fn live_attributes(
    chunks: &[crate::template_ir::Chunk],
    signals: &[String],
    framed: bool,
    out: &mut Vec<Live>,
) {
    use crate::template_ir::{Chunk, Part};
    for c in chunks {
        let Chunk::Dynamic(p) = c else { continue };
        if !framed
            && set_in_place(p)
            && let Part::Attribute {
                id, name, value, ..
            }
            | Part::BooleanAttribute {
                id, name, value, ..
            } = p
            && let Some(root) = value.split('.').next()
            && signals.iter().any(|s| s == root)
        {
            out.push(Live {
                part: id.0,
                signal: root.to_string(),
                path: value.clone(),
                kind: p.kind().to_string(),
                reads: Vec::new(),
                attribute: name.clone(),
                owns: Vec::new(),
                derived: String::new(),
            });
        }
        let inner = framed || matches!(p, Part::Each { .. });
        for region in p.nested() {
            live_attributes(region, signals, inner, out);
        }
    }
}

/// **A region that shows a query's state** (ADR-0148): the query a host
/// runs for it, and whether the document waits for it.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct Stream {
    pub part: u32,
    /// The query, as a component id.
    pub resource: String,
    /// Each argument as a host computes it: a page parameter's name, or an
    /// invocation-context call, `current_session()`.
    pub args: Vec<String>,
    /// How a host runs the query: its declared policies (ADR-0127).
    pub policy: Policy,
    /// `delivery streamed`: the document is sent first, and the region is
    /// filled when the query settles, in the same response.
    pub streamed: bool,
}

/// A page's plan.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct PageValues {
    /// The page's template path: `store.page.StorePage`.
    pub page: String,
    /// **Where the page is served** (ADR-0160): its `route` clause, a
    /// `{name}` segment for each parameter, which the address carries.
    /// `None` for a page that declares no route.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub route: Option<String>,
    pub params: Vec<String>,
    pub bindings: Vec<Binding>,
    /// Each text part outside a block.
    pub parts: Vec<Part>,
    /// **Each list a loop iterates at a query's value**, by its path from
    /// the binding: `menu`, or `cart.lines`, a list inside the value
    /// (ADR-0170). A host gives the renderer each binding whole, and renders
    /// a list again when what it holds changes (ADR-0145).
    pub collections: Vec<String>,
    /// The page's signals (ADR-0130). Their first values are the backend's
    /// to compute, and `build` writes them here.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub signals: Vec<Signal>,
    /// The parts its signals decide.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub live: Vec<Live>,
    /// **The blocks a query's value decides, at the top of the page**
    /// (ADR-0146): a host renders each from the bindings' values, and again
    /// when what it renders changed. A block inside one is rendered with it.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub blocks: Vec<u32>,
    /// **Each region that shows a query's state** (ADR-0148): a host runs
    /// its query, and renders the arm it settled to.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub streams: Vec<Stream>,
    /// **What a loop's row reads of its item through a member** (ADR-0169),
    /// which a host computes for each row before the row is rendered. A
    /// field the renderer reads from the item itself, and is not here.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub rows: Vec<RowRead>,
    /// **Each attribute at the top of the page that reads a query's value**
    /// (ADR-0171), by its part: a host sets it again when what it reads
    /// changes, as it sets a text part. One in a block is rendered with its
    /// block, and one in a row with its row.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub attributes: Vec<u32>,
    /// **Each computed attribute's value** (ADR-0226), by the path the
    /// compiler names it: a host computes it into what the page is rendered
    /// with, and sets the attribute again as it sets any.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub derived: Vec<Part>,
    /// **Each element at the top of the page whose handlers capture a
    /// query's value** (ADR-0217), by its first handler's part: a host sets
    /// its captures again when the value changes.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub captures: Vec<u32>,
    /// **The page's title** (ADR-0183), by its part: a host writes it into
    /// the document's head from the bindings' values, and sets it again when
    /// what it reads changes, as it sets a text part.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub title: Option<u32>,
    /// **Its documents' scope** (ADR-0300), as their resume manifest says it
    /// and the browser's decision holds a capture to: `public`, `session:`,
    /// `user:`, `organization:` or `private:` (`resume::page_scope`).
    #[serde(default = "public_scope", skip_serializing_if = "is_public_scope")]
    pub scope: String,
    /// **The layout the page is shown in** (ADR-XXXX): its path, its own
    /// markup's schema, and how many parts and elements it numbers first. A
    /// navigation between two pages that record the same keeps it in place.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub layout: Option<crate::template_ir::PageLayout>,
}

fn public_scope() -> String {
    "public".to_string()
}

fn is_public_scope(scope: &String) -> bool {
    scope == "public"
}

/// **A row's read through a member function** (ADR-0169): `{item.price.display}`
/// inside `{#each menu as item (item.id)}`. One for each path, however many
/// parts read it.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct RowRead {
    /// The list the loop iterates, by its path from a query's binding:
    /// `menu`, or `cart.lines` (ADR-0170).
    pub collection: String,
    /// The name each item is bound to: `item`.
    pub binding: String,
    /// The template's path for it, from the item's name: `item.price.display`.
    pub path: String,
    /// The steps from the item: fields, and member functions by component.
    pub steps: Vec<Step>,
}

/// **Whether the value read at `at` calls a member function** anywhere along
/// its path (ADR-0169), as the value typer resolves each read in `origin`'s
/// body. Typed once for each body.
pub(crate) fn calls_a_member(
    hirs: &[&Hir],
    ws: &Workspace,
    sigs: &Signatures,
    typed: &mut BTreeMap<(usize, DeclId), crate::values::MemberReads>,
    origin: (usize, DeclId),
    at: crate::template_ir::ReadAt,
) -> bool {
    let (unit, decl) = origin;
    let hir = hirs[unit];
    let reads = typed
        .entry(origin)
        .or_insert_with(|| crate::values::member_reads(hir, sigs, ws, unit, decl));
    match at {
        crate::template_ir::ReadAt::List(node) => reads.lists.contains(&node),
        crate::template_ir::ReadAt::Expr(mut e) => {
            let Some(body) = hir.decl(decl).body.map(|b| hir.body(b)) else {
                return false;
            };
            loop {
                if reads.exprs.contains(&e) {
                    return true;
                }
                match body.expr(e) {
                    Expr::Field { base, .. } => e = *base,
                    _ => return false,
                }
            }
        }
    }
}

/// **A row's read of its item through a member** (ADR-0169), for the read
/// of `path` in `part`: its loop's, where the loop iterates a query's list.
/// `None` where it is not one.
#[allow(clippy::too_many_arguments)]
fn row_read(
    hirs: &[&Hir],
    sigs: &Signatures,
    chunks: &[crate::template_ir::Chunk],
    found: &[(String, DefId, Vec<ExprId>)],
    part: u32,
    path: &str,
    members: &mut BTreeSet<DefId>,
) -> Result<Option<RowRead>, String> {
    let mut segments = path.split('.');
    let root = segments.next().unwrap_or_default();
    let reads: Vec<&str> = segments.collect();
    let Some((_, collection)) = enclosing_loop(chunks, part, root, &mut Vec::new()).flatten()
    else {
        return Ok(None);
    };
    // The list: a query's value, or a list inside it, through its fields
    // (ADR-0170). One read through a member is a list's own read, which the
    // plan refuses but in a row (ADR-0169).
    let mut through = collection.split('.');
    let query = through.next().unwrap_or_default();
    let fields: Vec<&str> = through.collect();
    let Some((_, resource, _)) = found.iter().find(|(n, ..)| *n == query) else {
        return Ok(None);
    };
    let Some(element) = value_of(sigs, *resource)
        .and_then(|t| field_type(sigs, t, &fields))
        .filter(|t| t.as_builtin() == Some(crate::resolved::Builtin::List))
        .and_then(|t| t.args().first().cloned())
    else {
        return Ok(None);
    };
    let mut out = Vec::new();
    for (step, def) in steps(sigs, element, &reads)? {
        out.push(match (step, def) {
            (Step::Member(_), Some(def)) => {
                members.insert(def);
                Step::Member(
                    component_id_of(hirs, def)
                        .ok_or_else(|| "a member function with no identity".to_string())?,
                )
            }
            (step, _) => step,
        });
    }
    Ok(Some(RowRead {
        collection,
        binding: root.to_string(),
        path: path.to_string(),
        steps: out,
    }))
}

/// The type at `fields` in a value of type `ty`, through record fields
/// alone: `None` where one is not a field.
fn field_type(sigs: &Signatures, mut ty: ResolvedType, fields: &[&str]) -> Option<ResolvedType> {
    for field in fields {
        // Each item of a list (ADR-0181): `menu.*.items`.
        if *field == "*" {
            ty = ty
                .as_builtin()
                .filter(|b| *b == crate::resolved::Builtin::List)
                .and_then(|_| ty.args().first().cloned())?;
            continue;
        }
        ty = sigs
            .type_decl(ty.def_id()?)?
            .record
            .as_ref()?
            .iter()
            .find(|(n, _)| n == field)?
            .1
            .resolved()?
            .clone();
    }
    Some(ty)
}

/// A read through a member function that no host computes where it is
/// (ADR-0169): refused, so the page is not built to fail when rendered.
fn unplanned(part: u32, path: &str, what: &str) -> String {
    format!(
        "part {part} reads `{path}` through a member function in {what}, which no host \
         computes there: one computes it from a query's value in text at the top of the page \
         (ADR-0125), or from the item of a loop over a query's list, or over a list in such an \
         item (ADR-0169, ADR-0181)"
    )
}

/// **The loop whose items `root` names, around the part `part`**: the
/// innermost one enclosing it that binds that name, with the collection it
/// iterates. Two loops may bind one name, as the store's menu and its
/// recommendations both bind `item`.
fn enclosing_loop(
    chunks: &[crate::template_ir::Chunk],
    part: u32,
    root: &str,
    around: &mut Vec<(String, String)>,
) -> Option<Option<(String, String)>> {
    use crate::template_ir::{Chunk, Part};
    for c in chunks {
        let Chunk::Dynamic(p) = c else { continue };
        if p.id().is_some_and(|id| id.0 == part) {
            let Some(at) = around.iter().rposition(|(b, _)| b == root) else {
                return Some(None);
            };
            let (binding, collection) = &around[at];
            return Some(Some((
                binding.clone(),
                through_loops(collection, &around[..at]),
            )));
        }
        let bound = match p {
            Part::Each {
                binding,
                collection,
                ..
            } => Some((binding.clone(), collection.clone())),
            _ => None,
        };
        let pushed = bound.is_some();
        around.extend(bound);
        for region in p.nested() {
            if let Some(found) = enclosing_loop(region, part, root, around) {
                return Some(found);
            }
        }
        if pushed {
            around.pop();
        }
    }
    None
}

/// **A loop's list, as a path from a query's value** (ADR-0181): a loop
/// inside another iterates a list in the outer one's item, so its
/// `section.items` is `menu.*.items`, the list read through each item of
/// `menu`. A list no loop around binds is its path as written.
fn through_loops(collection: &str, outer: &[(String, String)]) -> String {
    let (head, rest) = collection
        .split_once('.')
        .map_or((collection, ""), |(h, r)| (h, r));
    match outer.iter().rposition(|(b, _)| b == head) {
        Some(at) => {
            let base = through_loops(&outer[at].1, &outer[..at]);
            match rest.is_empty() {
                true => format!("{base}.*"),
                false => format!("{base}.*.{rest}"),
            }
        }
        None => collection.to_string(),
    }
}

/// Every `<stream>` in `chunks`, in document order.
fn streams_in(chunks: &[crate::template_ir::Chunk]) -> Vec<&crate::template_ir::Part> {
    use crate::template_ir::{Chunk, Part};
    let mut out = Vec::new();
    for c in chunks {
        let Chunk::Dynamic(p) = c else { continue };
        if matches!(p, Part::Stream { .. }) {
            out.push(p);
        }
        for region in p.nested() {
            out.extend(streams_in(region));
        }
    }
    out
}

/// **What a host runs for each `<stream>`** (ADR-0148), or why it cannot: a
/// query given what a page has, whose `fallback` no host executes yet, and a
/// region that reads no signal. The server renders a region once, so a
/// signal read in it would show its first value forever.
fn planned_streams(
    hirs: &[&Hir],
    chunks: &[crate::template_ir::Chunk],
    params: &[String],
    signals: &[String],
) -> Result<Vec<Stream>, String> {
    use crate::template_ir::Part;
    let mut out = Vec::new();
    for p in streams_in(chunks) {
        let Part::Stream {
            id,
            query,
            args,
            streamed,
            ..
        } = p
        else {
            continue;
        };
        let decl = hirs
            .iter()
            .flat_map(|hir| {
                hir.all_decls()
                    .filter(|(_, d)| d.kind == DeclKind::Query)
                    .filter(move |(i, _)| crate::contract::component_id(hir, *i) == *query)
                    .map(|(_, d)| d)
            })
            .next()
            .ok_or_else(|| format!("stream {id}'s query `{query}` is declared nowhere"))?;
        for a in args {
            let given = params.contains(a) || (a.ends_with("()") && !a.contains('.'));
            if !given {
                return Err(format!(
                    "stream {id}'s query is given `{a}`, which is neither a page parameter nor \
                     an invocation-context call"
                ));
            }
        }
        if let Some(f) = decl.policy("fallback") {
            return Err(format!(
                "stream {id}'s query declares `fallback {}`, which no host executes for a stream \
                 yet: its region would show the failure the declaration replaces (ADR-0148)",
                f.value
            ));
        }
        let root = |path: &str| path.split('.').next().unwrap_or_default().to_string();
        for region in p.nested() {
            for read in streams_reads(region) {
                if signals.contains(&root(&read)) {
                    return Err(format!(
                        "stream {id} shows the signal `{}`, and the server renders a stream's \
                         region once (ADR-0148)",
                        root(&read)
                    ));
                }
            }
        }
        out.push(Stream {
            part: id.0,
            resource: query.clone(),
            args: args.clone(),
            policy: policy_of(decl),
            streamed: *streamed,
        });
    }
    Ok(out)
}

/// Every path the parts in `chunks` read, inside their regions too.
fn streams_reads(chunks: &[crate::template_ir::Chunk]) -> Vec<String> {
    use crate::template_ir::Chunk;
    let mut out = Vec::new();
    for c in chunks {
        let Chunk::Dynamic(p) = c else { continue };
        out.extend(own_reads(p));
        for region in p.nested() {
            out.extend(streams_reads(region));
        }
    }
    out
}

/// The names a page's queries are bound to.
fn found_names<T, U>(found: &[(String, T, U)]) -> Vec<&str> {
    found.iter().map(|(n, ..)| n.as_str()).collect()
}

/// **What a loop's row reads of a query besides its own item** (ADR-0146):
/// refused, naming the part. A row is rendered again when its item changes
/// (ADR-0145), and a value of another query read there would stay as it was
/// when that query changed.
fn in_rows(
    chunks: &[crate::template_ir::Chunk],
    queries: &[&str],
    in_row: bool,
) -> Result<(), String> {
    use crate::template_ir::{Chunk, Part, Segment};
    let root = |path: &str| path.split('.').next().unwrap_or_default().to_string();
    for c in chunks {
        let Chunk::Dynamic(p) = c else { continue };
        if in_row {
            let read: Vec<String> = match p {
                Part::Text { value, .. }
                | Part::Attribute { value, .. }
                | Part::BooleanAttribute { value, .. }
                | Part::Conditional { value, .. }
                | Part::Match { value, .. } => vec![root(value)],
                Part::Each { collection, .. } => vec![root(collection)],
                Part::InterpolatedAttribute { segments, .. } => segments
                    .iter()
                    .filter_map(|s| match s {
                        Segment::Value(path) => Some(root(path)),
                        _ => None,
                    })
                    .collect(),
                _ => Vec::new(),
            };
            if let Some(q) = read.iter().find(|r| queries.contains(&r.as_str())) {
                return Err(format!(
                    "part {} reads `{q}` inside a loop's row, and a row is rendered again for \
                     its own item alone (ADR-0146)",
                    p.id().map(|i| i.0).unwrap_or_default()
                ));
            }
        }
        let inner = in_row || matches!(p, Part::Each { .. });
        for region in p.nested() {
            in_rows(region, queries, inner)?;
        }
    }
    Ok(())
}

/// **A body's signals, in order** (ADR-0130): each one's name and the
/// expression its first value is.
pub fn signals_of(body: &Body) -> Vec<(String, ExprId, ExprId)> {
    let mut out = Vec::new();
    for id in body.walk() {
        if !body.signals.contains(&id) {
            continue;
        }
        let Expr::Let {
            pat: Some(p),
            init: Some(init),
            ..
        } = body.expr(id)
        else {
            continue;
        };
        if let crate::hir::Pattern::Bind { name, .. } = body.pat(*p) {
            out.push((name.clone(), *init, id));
        }
    }
    out
}

/// The plan, or why there is none, for one page.
#[derive(Debug, Clone)]
pub struct Planned {
    pub page: String,
    pub plan: Result<PageValues, String>,
}

/// A path written in `unit`, resolved as a term.
pub(crate) fn resolve_term(ws: &Workspace, unit: usize, path: &str) -> Option<DefId> {
    let r = match path.contains('.') {
        true => ws.resolve_path(unit, path),
        false => ws.resolve_in(unit, Namespace::Term, path),
    };
    match r {
        Resolution::Local(d) | Resolution::Imported { def: d, .. } => Some(d),
        _ => None,
    }
}

/// `let b = query R(k..)` in a page's body: the name, the query, its keys.
pub(crate) fn query_bindings(
    ws: &Workspace,
    unit: usize,
    body: &Body,
) -> Vec<(String, DefId, Vec<ExprId>)> {
    let mut out = Vec::new();
    for e in body.walk() {
        let Expr::Let {
            pat: Some(pat),
            init: Some(init),
            ..
        } = body.expr(e)
        else {
            continue;
        };
        let Pattern::Bind { name, .. } = body.pat(*pat) else {
            continue;
        };
        let Expr::Keyword {
            keyword,
            modifiers,
            args,
            ..
        } = body.expr(*init)
        else {
            continue;
        };
        if keyword != "query" {
            continue;
        }
        let Some(resource) = modifiers.first().and_then(|p| resolve_term(ws, unit, p)) else {
            continue;
        };
        out.push((name.clone(), resource, args.clone()));
    }
    out
}

/// The value a resource's entry holds: its result, or `Ok`'s payload.
pub(crate) fn value_of(sigs: &Signatures, resource: DefId) -> Option<ResolvedType> {
    let result = sigs.by_def(resource)?.result()?;
    match result.as_builtin() {
        Some(crate::resolved::Builtin::Result) => result.args().first().cloned(),
        _ => Some(result.clone()),
    }
}

/// The component id of a declaration, found by its identity.
fn component_id_of(hirs: &[&Hir], def: DefId) -> Option<String> {
    let hir = hirs.get(def.unit)?;
    hir.all_decls()
        .find(|(id, _)| id.0 == def.decl)
        .map(|(id, _)| crate::contract::component_id(hir, id))
}

/// The steps a path's reads take from a value of type `ty`, and the member
/// functions it names.
fn steps(
    sigs: &Signatures,
    mut ty: ResolvedType,
    reads: &[&str],
) -> Result<Vec<(Step, Option<DefId>)>, String> {
    let mut out = Vec::new();
    for read in reads {
        let def = ty
            .def_id()
            .ok_or_else(|| format!("`.{read}` is read from a value with no declared type"))?;
        let field = sigs
            .type_decl(def)
            .and_then(|t| t.record.as_ref())
            .and_then(|fields| fields.iter().find(|(n, _)| n == read))
            .and_then(|(_, t)| t.resolved().cloned());
        if let Some(next) = field {
            out.push((Step::Field(read.to_string()), None));
            ty = next;
            continue;
        }
        let member = sigs
            .member_by(Receiver::Nominal(def), read)
            .ok_or_else(|| format!("`.{read}` is neither a field nor a member"))?;
        let next = member
            .result()
            .cloned()
            .ok_or_else(|| format!("`.{read}` has no declared result"))?;
        out.push((Step::Member(String::new()), Some(member.definition)));
        ty = next;
    }
    Ok(out)
}

fn plan(
    hirs: &[&Hir],
    ws: &Workspace,
    sigs: &Signatures,
    // What each handler captures (ADR-0137): the document holds it.
    captures: &crate::template_ir::Handlers,
    // What each body performs, which a computed part may not (ADR-0226).
    inference: &crate::effects::Inference<'_>,
    unit: usize,
    id: DeclId,
) -> Result<(PageValues, BTreeSet<DefId>, Vec<Derived>), String> {
    let hir = hirs[unit];
    let decl = hir.decl(id);
    let body = hir.body(
        decl.body
            .ok_or_else(|| format!("`{}` has no body", decl.name))?,
    );
    let params: Vec<String> = decl.params.iter().map(|p| p.name.clone()).collect();
    let page = {
        let module = hir.module_of(id).unwrap_or_default();
        if module.is_empty() {
            decl.name.clone()
        } else {
            format!("{module}.{}", decl.name)
        }
    };

    // The page's own signals, by name: a query given one is read again, for
    // the new key, when it changes (ADR-0152).
    let page_signals: BTreeSet<String> = body
        .signals
        .iter()
        .filter_map(|s| match body.expr(*s) {
            Expr::Let { pat: Some(p), .. } => match body.pat(*p) {
                Pattern::Bind { name, .. } => Some(name.clone()),
                _ => None,
            },
            _ => None,
        })
        .collect();
    let mut found = query_bindings(ws, unit, body);
    // The error that means the page is not found (ADR-0163), PW0342's to
    // hold to the page's queries.
    let not_found = crate::routes::not_found_case(ws, sigs, unit, decl)
        .ok()
        .flatten();
    // And the one that means its address is another of the page's
    // (ADR-0295), PW0350's to hold.
    let redirect = crate::routes::redirect_case(ws, sigs, unit, decl)
        .ok()
        .flatten();
    let mut bindings = Vec::new();
    for (name, resource, keys) in &found {
        let mut args = Vec::new();
        let mut keyed_by = Vec::new();
        for k in keys {
            args.push(match body.expr(*k) {
                Expr::Name(n) if params.contains(n) => n.clone(),
                Expr::Name(n) if page_signals.contains(n) => {
                    keyed_by.push(n.clone());
                    n.clone()
                }
                Expr::Call { callee, args } if args.is_empty() => {
                    format!("{}()", crate::infer::path_of(body, *callee))
                }
                _ => {
                    return Err(format!(
                        "`{name}`'s key is neither a page parameter, an invocation-context \
                         call, nor a page signal"
                    ));
                }
            });
        }
        let decl = crate::resolve::declaration(hirs, *resource)
            .ok_or_else(|| format!("`{name}`'s query is declared nowhere"))?;
        bindings.push(Binding {
            binding: name.clone(),
            resource: component_id_of(hirs, *resource)
                .ok_or_else(|| format!("`{name}`'s query has no component"))?,
            args,
            policy: policy_of(decl),
            signals: keyed_by,
            not_found: match &not_found {
                Some((ty, case)) if crate::routes::error_of(sigs, *resource) == Some(*ty) => {
                    vec![crate::wit::ident(case)]
                }
                _ => Vec::new(),
            },
            redirect: match &redirect {
                Some((ty, case, permanent))
                    if crate::routes::error_of(sigs, *resource) == Some(*ty) =>
                {
                    Some(Redirect {
                        case: crate::wit::ident(case),
                        permanent: *permanent,
                    })
                }
                _ => None,
            },
        });
    }

    // **Its layout's bindings** (ADR-XXXX), read once for each document as
    // the page's are, each under the name the layout's markup is lowered
    // with (`cart~StoreLayout`). A key is the request's reader or the
    // layout's own signal: a layout is given nothing by its page. Neither
    // means the page is not found or is elsewhere: those are the page's
    // clauses, over its own queries.
    if let Some(def) = crate::layouts::of(hirs, ws, unit, decl) {
        let lhir = hirs[def.unit];
        let ldecl = lhir.decl(DeclId(def.decl));
        let lbody = lhir.body(
            ldecl
                .body
                .ok_or_else(|| format!("the layout `{}` has no body", ldecl.name))?,
        );
        let own: BTreeSet<String> = signals_of(lbody).into_iter().map(|(n, ..)| n).collect();
        let mut planned = Vec::new();
        for (name, resource, keys) in query_bindings(ws, def.unit, lbody) {
            let mut args = Vec::new();
            let mut keyed_by = Vec::new();
            for k in &keys {
                args.push(match lbody.expr(*k) {
                    Expr::Name(n) if own.contains(n) => {
                        let named = crate::layouts::bound(&ldecl.name, n);
                        keyed_by.push(named.clone());
                        named
                    }
                    Expr::Call { callee, args } if args.is_empty() => {
                        format!("{}()", crate::infer::path_of(lbody, *callee))
                    }
                    _ => {
                        return Err(format!(
                            "the layout `{}`'s `{name}`'s key is neither an invocation-context \
                             call nor the layout's signal",
                            ldecl.name
                        ));
                    }
                });
            }
            let query = crate::resolve::declaration(hirs, resource)
                .ok_or_else(|| format!("`{name}`'s query is declared nowhere"))?;
            let named = crate::layouts::bound(&ldecl.name, &name);
            planned.push(Binding {
                binding: named.clone(),
                resource: component_id_of(hirs, resource)
                    .ok_or_else(|| format!("`{name}`'s query has no component"))?,
                args,
                policy: policy_of(query),
                signals: keyed_by,
                not_found: Vec::new(),
                redirect: None,
            });
            found.push((named, resource, keys));
        }
        // Before the page's: the layout's markup is rendered first.
        bindings.splice(0..0, planned);
    }

    let mut live = Vec::new();
    let mut parts = Vec::new();
    let mut members = BTreeSet::new();
    // What a host computes for a computed part (ADR-0226).
    let mut computed: Vec<Derived> = Vec::new();
    let mut derived_values: Vec<Part> = Vec::new();
    // Each block's subject the browser computes from a signal (ADR-0229): its
    // signal, the path its value is read at, and its component.
    let mut browser_subjects: BTreeMap<u32, (String, String, String)> = BTreeMap::new();
    let page_def = DefId { unit, decl: id.0 };
    // The page's template, each view it uses composed in place (ADR-0136):
    // what the build renders, numbered as the build numbers it.
    let crate::template_ir::Lowered {
        template,
        holes,
        reads: others,
        instances,
        layout,
        ..
    } = crate::template_ir::lowered(hirs, ws, sigs, captures, unit, id)
        .ok_or_else(|| format!("`{}` has no template", decl.name))?;
    // The signals the browser holds: the page's own, and each one the views
    // composed in it hold and are provided (ADR-0144).
    let signals: Vec<String> = instances.iter().map(|i| i.name.clone()).collect();
    let owned = |part: u32| -> Vec<String> {
        instances
            .iter()
            .filter(|i| i.within.iter().any(|b| b.0 == part))
            .map(|i| i.name.clone())
            .collect()
    };
    // What a signal decides, the browser renders again (ADR-0137): a block
    // whose subject the browser computes from one as well (ADR-0229).
    let browser: Vec<String> = others
        .iter()
        .filter(|r| r.kind == crate::template_ir::ReadKind::Subject && !r.nested)
        .filter(|r| {
            r.inputs.as_deref().is_some_and(|inputs| {
                inputs.iter().any(|(_, read)| {
                    signals
                        .iter()
                        .any(|s| Some(s.as_str()) == read.split('.').next())
                })
            })
        })
        .map(|r| r.path.clone())
        .collect();
    rendered_again(&template.chunks, &signals, &browser, &Reach::Top)?;
    let mut rows: Vec<RowRead> = Vec::new();
    let mut typed = BTreeMap::new();
    for hole in holes {
        // **A computed hole** (ADR-0226): computed by a host, from the one
        // query's value it reads at the top of the page.
        if let Some(inputs) = &hole.inputs {
            let (derived, computes) = computed_part(
                hirs,
                ws,
                sigs,
                inference,
                &template.chunks,
                &found,
                (&signals, &params),
                (&page, page_def),
                hole.part.0,
                &hole.path,
                (hole.origin, hole.expr),
                inputs,
                hole.nested,
                &mut members,
            )?;
            match computes {
                // Inside a block a host renders (ADR-0229), set where the block
                // is rendered, with it; at the top, a part of its own.
                Computes::Host(part) if hole.nested => derived_values.push(part),
                Computes::Host(part) => parts.push(part),
                Computes::Row(row) => rows.push(row),
                Computes::Browser { signal, path } => live.push(Live {
                    part: hole.part.0,
                    signal,
                    path,
                    kind: "text".to_string(),
                    reads: Vec::new(),
                    attribute: String::new(),
                    owns: Vec::new(),
                    derived: derived.component_id.clone(),
                }),
            }
            computed.push(derived);
            continue;
        }
        // The path as the template reads it, through each view around it.
        let mut segments = hole.path.split('.').map(str::to_string);
        let Some(root) = segments.next() else {
            continue;
        };
        let reads: Vec<String> = segments.collect();
        // A part a signal decides is the browser's to render again
        // (ADR-0130): set in place, at top level or in a block a signal
        // decides (ADR-0142). Inside a loop, its address carries a frame the
        // browser does not compute, and its block renders it again
        // (ADR-0137).
        if signals.contains(&root) {
            // The browser reads a signal's value by field (ADR-0169).
            if calls_a_member(
                hirs,
                ws,
                sigs,
                &mut typed,
                hole.origin,
                crate::template_ir::ReadAt::Expr(hole.expr),
            ) {
                return Err(unplanned(hole.part.0, &hole.path, "a text part"));
            }
            if hole.framed {
                continue;
            }
            live.push(Live {
                part: hole.part.0,
                signal: root.clone(),
                path: std::iter::once(root.clone())
                    .chain(reads.iter().cloned())
                    .collect::<Vec<_>>()
                    .join("."),
                kind: "text".to_string(),
                reads: Vec::new(),
                attribute: String::new(),
                owns: Vec::new(),
                derived: String::new(),
            });
            continue;
        }
        let Some((_, resource, _)) = found.iter().find(|(n, ..)| *n == root) else {
            // **A page's parameter** (ADR-0231): the document is rendered
            // with it, as its address gave it, and it is the document's for
            // its life, so no host sets the part again. Until 2026-10-05 a
            // text part that read one was refused here.
            if !hole.nested && params.contains(&root) {
                continue;
            }
            // A loop's or an arm's name: the renderer reads it from the
            // collection, by field.
            if !hole.nested {
                return Err(format!(
                    "part {} reads `{root}`, which no query binds",
                    hole.part.0
                ));
            }
            // **A row reads a member of its item** (ADR-0169): for a loop
            // over a query's list, a host computes it for each row. Until
            // 2026-10-03 the plan said nothing of it, and the row failed to
            // render.
            if calls_a_member(
                hirs,
                ws,
                sigs,
                &mut typed,
                hole.origin,
                crate::template_ir::ReadAt::Expr(hole.expr),
            ) {
                let read = row_read(
                    hirs,
                    sigs,
                    &template.chunks,
                    &found,
                    hole.part.0,
                    &hole.path,
                    &mut members,
                )?
                .ok_or_else(|| unplanned(hole.part.0, &hole.path, "a text part"))?;
                if !rows
                    .iter()
                    .any(|r| r.collection == read.collection && r.path == read.path)
                {
                    rows.push(read);
                }
            }
            continue;
        };
        let ty = value_of(sigs, *resource)
            .ok_or_else(|| format!("`{root}`'s query has no resolved result"))?;
        let reads: Vec<&str> = reads.iter().map(String::as_str).collect();
        let planned = steps(sigs, ty, &reads)?;
        if hole.nested && planned.iter().any(|(s, _)| matches!(s, Step::Member(_))) {
            return Err(format!(
                "part {} reads a member of `{root}` inside a block, which a host would call \
                 per instance",
                hole.part.0
            ));
        }
        if hole.nested {
            continue;
        }
        let mut out = Vec::new();
        for (step, def) in planned {
            out.push(match (step, def) {
                (Step::Member(_), Some(def)) => {
                    members.insert(def);
                    Step::Member(
                        component_id_of(hirs, def)
                            .ok_or_else(|| "a member function with no identity".to_string())?,
                    )
                }
                (step, _) => step,
            });
        }
        let path = std::iter::once(root.clone())
            .chain(reads.iter().map(|r| r.to_string()))
            .collect::<Vec<_>>()
            .join(".");
        parts.push(Part {
            part: hole.part.0,
            path,
            binding: root,
            steps: out,
        });
    }

    // **Each value read outside a text part** (ADR-0169): an attribute's, a
    // block's subject, a loop's list. The renderer reads each by field;
    // through a member function, a host computes one for a row of a query's
    // list. Until 2026-10-03 none was planned or refused, and a page that
    // read one built, then failed to render.
    for read in &others {
        // A computed attribute (ADR-0226): its value is computed by a host,
        // as a computed hole's is, and the attribute set again as any is.
        if let Some(inputs) = &read.inputs {
            let crate::template_ir::ReadAt::Expr(expr) = read.at else {
                return Err(format!("part {} computes no expression", read.part.0));
            };
            let (derived, computes) = computed_part(
                hirs,
                ws,
                sigs,
                inference,
                &template.chunks,
                &found,
                (&signals, &params),
                (&page, page_def),
                read.part.0,
                &read.path,
                (read.origin, expr),
                inputs,
                read.nested,
                &mut members,
            )?;
            match computes {
                Computes::Host(part) => derived_values.push(part),
                Computes::Row(row) => rows.push(row),
                // A block's subject the browser computes (ADR-0229): its block
                // is rendered again in the browser, as one a signal decides,
                // below.
                Computes::Browser { signal, path }
                    if read.kind == crate::template_ir::ReadKind::Subject =>
                {
                    browser_subjects
                        .insert(read.part.0, (signal, path, derived.component_id.clone()));
                }
                // Set in place, as an attribute a signal decides is (ADR-0142).
                Computes::Browser { signal, path } => {
                    let (kind, attribute) = match find_part(&template.chunks, read.part.0) {
                        Some(
                            p @ (crate::template_ir::Part::Attribute { name, .. }
                            | crate::template_ir::Part::BooleanAttribute { name, .. }),
                        ) => (p.kind().to_string(), name.clone()),
                        _ => return Err(format!("part {} is no attribute", read.part.0)),
                    };
                    live.push(Live {
                        part: read.part.0,
                        signal,
                        path,
                        kind,
                        reads: Vec::new(),
                        attribute,
                        owns: Vec::new(),
                        derived: derived.component_id.clone(),
                    });
                }
            }
            computed.push(derived);
            continue;
        }
        // A handler's captures are paths of data, and where they are read
        // is the handler itself, whose calls are its own (ADR-0217).
        if read.kind == crate::template_ir::ReadKind::Captures
            || !calls_a_member(hirs, ws, sigs, &mut typed, read.origin, read.at)
        {
            continue;
        }
        let what = match read.kind {
            crate::template_ir::ReadKind::Attribute => "an attribute",
            crate::template_ir::ReadKind::Subject => "what a block decides by",
            crate::template_ir::ReadKind::List => "a loop's list",
            crate::template_ir::ReadKind::Title => "the page's title",
            crate::template_ir::ReadKind::Meta => "the page's metadata",
            crate::template_ir::ReadKind::Captures => "what a handler captures",
        };
        let row = row_read(
            hirs,
            sigs,
            &template.chunks,
            &found,
            read.part.0,
            &read.path,
            &mut members,
        )?
        .ok_or_else(|| unplanned(read.part.0, &read.path, what))?;
        if !rows
            .iter()
            .any(|r| r.collection == row.collection && r.path == row.path)
        {
            rows.push(row);
        }
    }

    // **An attribute at the top of the page reads a query's value**
    // (ADR-0171): a host sets it again when the value changes. Until
    // 2026-10-03 nothing did, and it kept its first value for the document's
    // life. A member read in one is refused above (ADR-0169).
    let mut attributes = Vec::new();
    for read in &others {
        let root = read.path.split('.').next().unwrap_or_default();
        // A computed one too (ADR-0226), whose value a host computes.
        let computed_here = derived_values.iter().any(|d| d.part == read.part.0);
        if read.kind == crate::template_ir::ReadKind::Attribute
            && !read.nested
            && (found.iter().any(|(n, ..)| n == root) || computed_here)
            && !attributes.contains(&read.part.0)
        {
            attributes.push(read.part.0);
        }
    }

    // **A handler at the top of the page captures a query's value**
    // (ADR-0217): a host sets its element's captures again when the value
    // changes, as it sets an attribute's. Until then nothing did, and a
    // press after a commit sent the value the page was first rendered with.
    // One part for each element: its handlers' captures are one attribute.
    let mut captures = Vec::new();
    let mut owners = BTreeSet::new();
    for read in &others {
        let root = read.path.split('.').next().unwrap_or_default();
        if read.kind != crate::template_ir::ReadKind::Captures
            || read.nested
            || !found.iter().any(|(n, ..)| n == root)
        {
            continue;
        }
        let owner = template.chunks.iter().find_map(|c| match c {
            crate::template_ir::Chunk::Dynamic(crate::template_ir::Part::Event {
                id,
                owner,
                ..
            }) if *id == read.part => Some(*owner),
            _ => None,
        });
        if let Some(owner) = owner
            && owners.insert(owner)
        {
            captures.push(read.part.0);
        }
    }

    // Each list the template iterates at a query's value, by its path: the
    // binding itself, or a list inside it (ADR-0170). Until 2026-10-03 the
    // binding was recorded, and a host given `cart` for `cart.lines` found
    // no list to render again.
    let mut collections = Vec::new();
    let mut blocks = Vec::new();
    // What a loop's row reads of a query besides its own item is not
    // rendered again when that query changes: the row is rendered again for
    // its item (ADR-0145). Refused, until rows are (ADR-0146).
    in_rows(&template.chunks, &found_names(&found), false)?;
    for entry in template.manifest() {
        if entry.kind == "each"
            && let Some(root) = entry.value.split('.').next()
            && found.iter().any(|(n, ..)| n == root)
            && !collections.contains(&entry.value)
        {
            collections.push(entry.value.clone());
        }
        // A block a query's value decides (ADR-0146): a host renders it,
        // and renders it again when what it renders changed. At the top of
        // the page, or inside such a block, which renders it again with it.
        // Its subject a host computes from a query's value too (ADR-0229).
        if matches!(entry.kind, "conditional" | "match")
            && let Some(root) = entry.value.split('.').next()
            && (found.iter().any(|(n, ..)| n == root)
                || derived_values.iter().any(|d| d.path == entry.value))
            && template.chunks.iter().any(
                |c| matches!(c, crate::template_ir::Chunk::Dynamic(p) if p.id() == Some(entry.id)),
            )
        {
            blocks.push(entry.id.0);
        }
        // A view's instance a query's value gives, at the top of the page
        // (ADR-0203): a host renders it, and again, whole, when what it
        // renders changed, as it does a block.
        if entry.kind == "instance"
            && let Some(crate::template_ir::Part::Instance { args, .. }) =
                find_part(&template.chunks, entry.id.0)
            && args.iter().any(|(_, v)| {
                let root = v.split('.').next().unwrap_or_default();
                found.iter().any(|(n, ..)| n == root)
            })
            && template.chunks.iter().any(
                |c| matches!(c, crate::template_ir::Chunk::Dynamic(p) if p.id() == Some(entry.id)),
            )
        {
            blocks.push(entry.id.0);
        }
        // And one the page's signals give: rendered again in the browser,
        // whole, when one of them changes. One given anything else as well
        // was refused above.
        if entry.kind == "instance"
            && let Some(part @ crate::template_ir::Part::Instance { args, .. }) =
                find_part(&template.chunks, entry.id.0)
            && let Some(root) = args
                .iter()
                .map(|(_, v)| v.split('.').next().unwrap_or_default())
                .find(|r| signals.iter().any(|s| s == r))
            && template.chunks.iter().any(
                |c| matches!(c, crate::template_ir::Chunk::Dynamic(p) if p.id() == Some(entry.id)),
            )
        {
            live.push(Live {
                part: entry.id.0,
                signal: root.to_string(),
                path: String::new(),
                kind: entry.kind.to_string(),
                reads: block_reads(part, &signals),
                attribute: String::new(),
                owns: Vec::new(),
                derived: String::new(),
            });
        }
        // A block whose subject the browser computes from a signal
        // (ADR-0229): rendered again in the browser when the signal changes,
        // its subject computed by its function.
        if matches!(entry.kind, "conditional" | "match")
            && let Some((signal, path, derived)) = browser_subjects.get(&entry.id.0)
            && let Some(part) = find_part(&template.chunks, entry.id.0)
        {
            // What its arms hold starts again when it shows another arm
            // (ADR-0144), which the browser knows from the subject's value
            // before it renders: not computed yet (ruling 0073-a).
            if !owned(entry.id.0).is_empty() {
                return Err(format!(
                    "part {} is a block whose subject the browser computes, and whose arms \
                     hold a view's signals, which the browser starts again by its arm \
                     (ruling 0073-a)",
                    entry.id.0
                ));
            }
            let mut reads = block_reads(part, &signals);
            if !reads.contains(signal) {
                reads.insert(0, signal.clone());
            }
            live.push(Live {
                part: entry.id.0,
                signal: signal.clone(),
                path: path.clone(),
                kind: entry.kind.to_string(),
                reads,
                attribute: String::new(),
                owns: owned(entry.id.0),
                derived: derived.clone(),
            });
        }
        // A block a signal decides (ADR-0130). What the browser cannot
        // render again was refused above (ADR-0137).
        if matches!(entry.kind, "conditional" | "match")
            && let Some(root) = entry.value.split('.').next()
            && signals.iter().any(|s| s == root)
            && let Some(part) = find_part(&template.chunks, entry.id.0)
        {
            live.push(Live {
                part: entry.id.0,
                signal: root.to_string(),
                path: entry.value.clone(),
                kind: entry.kind.to_string(),
                reads: block_reads(part, &signals),
                attribute: String::new(),
                owns: owned(entry.id.0),
                derived: String::new(),
            });
        }
    }
    // An attribute a signal decides, set in place (ADR-0142).
    live_attributes(&template.chunks, &signals, false, &mut live);
    live.sort_by_key(|l| l.part);
    // **The page's title** (ADR-0183), from its parameters and its queries'
    // values. One that reads a signal is refused when the signals compile:
    // the browser sets no title from one (ADR-0137).
    let title = template.chunks.iter().find_map(|c| match c {
        crate::template_ir::Chunk::Dynamic(crate::template_ir::Part::Title { id, .. }) => {
            Some(id.0)
        }
        _ => None,
    });
    // Each region that shows a query's state (ADR-0148), and a view's
    // signal held inside one, which nothing would render again.
    let streams = planned_streams(hirs, &template.chunks, &params, &signals)?;
    for s in &streams {
        if let Some(i) = instances
            .iter()
            .find(|i| i.within.iter().any(|b| b.0 == s.part))
        {
            return Err(format!(
                "stream {} holds a view's signal `{}`, and the server renders a stream's region \
                 once (ADR-0148)",
                s.part, i.name
            ));
        }
    }

    Ok((
        PageValues {
            page,
            route: crate::routes::declared_route(hir, decl),
            params,
            bindings,
            parts,
            collections,
            signals: Vec::new(),
            live,
            blocks,
            streams,
            rows,
            attributes,
            derived: derived_values,
            captures,
            title,
            scope: crate::resume::page_scope(hir, decl).to_string(),
            layout,
        },
        members,
        computed,
    ))
}

/// **Who computes a computed part** (ADR-0226, ADR-0227, ADR-0228).
enum Computes {
    /// A host, from a query's value, by the part's steps, the last its
    /// function.
    Host(Part),
    /// The browser, from the signal's value at `path`, each time it
    /// changes; and a host its first, from the signal's first value.
    Browser { signal: String, path: String },
    /// A host, for each row of a query's list, from the row's item, as it
    /// computes a member read of it (ADR-0169), and the browser for a row it
    /// renders from a speculated value (ADR-0172).
    Row(RowRead),
}

/// **A computed part** (ADR-0226, ADR-0227, ADR-0228): the function its
/// expression is lifted into, a component of its own, and who runs it with
/// the one value it reads. A query's value at the top of the page is a
/// host's: the steps reach it, then run the function. A signal's is the
/// browser's, which runs the function compiled for it, and a host's for the
/// first value. A row's item is a host's for each row of a query's list.
/// Anything else is refused by name: one in a block, an arm or an instance,
/// several values, or a value the page speculates on (ruling 0073-a); and none,
/// which is a value to write as it is.
#[allow(clippy::too_many_arguments)]
fn computed_part(
    hirs: &[&Hir],
    ws: &Workspace,
    sigs: &Signatures,
    inference: &crate::effects::Inference<'_>,
    chunks: &[crate::template_ir::Chunk],
    found: &[(String, DefId, Vec<ExprId>)],
    (signals, params): (&[String], &[String]),
    (page, page_def): (&str, DefId),
    part: u32,
    path: &str,
    (origin, expr): ((usize, DeclId), ExprId),
    inputs: &[(String, String)],
    nested: bool,
    members: &mut BTreeSet<DefId>,
) -> Result<(Derived, Computes), String> {
    let refuse = |why: String| Err(format!("part {part} computes a value {why}"));
    let hir = hirs[origin.0];
    let decl = hir.decl(origin.1);
    let body = hir.body(
        decl.body
            .ok_or_else(|| format!("`{}` has no body", decl.name))?,
    );
    let types = crate::infer::Types::of_decl(sigs, hir, origin.1, body);
    // **It performs nothing** (ADR-0226). The checker refuses an effect in
    // one a request renders (PW0334). What it leaves, a build's input read by
    // a page placed at build and an audited escape hatch, a host would run as
    // compiled code: `include_markdown` would answer its stub's "".
    let region = body.expr_span(expr);
    if let Some(source) = inference
        .infer_in_at(origin.0, body, &types)
        .sources
        .iter()
        .find(|s| s.span.start >= region.start && s.span.end <= region.end)
    {
        return refuse(format!(
            "that performs `{}`, and a host computes only a value that performs nothing",
            source.effect
        ));
    }
    let [(name, read)] = inputs else {
        return refuse(match inputs {
            [] => "from nothing it reads: write the value itself".to_string(),
            _ => format!(
                "from {}, and a host computes one from one value (ruling 0073-a)",
                inputs
                    .iter()
                    .map(|(n, _)| format!("`{n}`"))
                    .collect::<Vec<_>>()
                    .join(" and ")
            ),
        });
    };
    // Its input's type, where its expression reads it, and its result's.
    let input = body
        .walk_from(expr)
        .into_iter()
        .find(|x| matches!(body.expr(*x), Expr::Name(n) if n == name))
        .and_then(|x| types.of(body, x))
        .ok_or_else(|| format!("part {part}'s `{name}` has no type"))?;
    // What it computes, as the checker's value relations type it: any
    // expression they check, where `Types` knows a name, a field and a call.
    let result = crate::values::type_of(
        sigs,
        ws,
        origin.0,
        hir.module_of(origin.1),
        decl,
        body,
        expr,
    )
    .0
    .key()
    .and_then(|key| {
        crate::resolved::ResolvedType::inferred(
            &key,
            &|def| crate::resolve::declaration(hirs, def).map(|d| d.name.clone()),
            &region,
        )
    })
    .ok_or_else(|| format!("part {part}'s value has no type the build knows"))?;
    let component_id = format!("{page}.derived_{part}");
    let derived = Derived {
        component_id: component_id.clone(),
        page: page_def,
        part,
        origin,
        expr,
        input: (name.clone(), input),
        result,
    };
    // **In a loop's row** (ADR-0228): the compiler named its path from the
    // row's item, which alone it reads. Computed for each row by the steps
    // that reach its input from the item, then the function, and set in the
    // row by that path, as a member read of the item is (ADR-0169).
    if path.contains(".#") {
        let Some(mut row) = row_read(hirs, sigs, chunks, found, part, read, members)? else {
            return refuse(format!(
                "in a row of `{name}`, a list no query gives, which no host computes yet \
                 (ruling 0073-a)"
            ));
        };
        row.path = path.to_string();
        row.steps.push(Step::Derived(component_id));
        return Ok((derived, Computes::Row(row)));
    }
    let mut segments = read.split('.');
    let root = segments.next().unwrap_or_default();
    let reads: Vec<&str> = segments.collect();
    // A signal's value is the browser's to compute (ADR-0227), at the path
    // its expression reads it by: at the top of the page, where it is set in
    // place, or as a block's subject, where the block is rendered again.
    if signals.iter().any(|s| s == root) {
        if nested {
            return refuse(format!(
                "from the signal `{name}` inside a block, and the browser computes one at the \
                 top of the page (ruling 0073-a)"
            ));
        }
        return Ok((
            derived,
            Computes::Browser {
                signal: root.to_string(),
                path: read.clone(),
            },
        ));
    }
    let Some((_, resource, _)) = found.iter().find(|(n, ..)| n == root) else {
        // A page's parameter, which the page is rendered with (ADR-0231).
        if params.iter().any(|p| p == root) {
            return refuse(format!(
                "from the page's parameter `{name}`, and a host computes one from a query's \
                 value alone (ruling 0073-a)"
            ));
        }
        return refuse(format!("from `{name}`, which is no query's value"));
    };
    let ty = value_of(sigs, *resource)
        .ok_or_else(|| format!("`{root}`'s query has no resolved result"))?;
    let mut out = Vec::new();
    for (step, def) in steps(sigs, ty, &reads)? {
        out.push(match (step, def) {
            (Step::Member(_), Some(def)) => {
                members.insert(def);
                Step::Member(
                    component_id_of(hirs, def)
                        .ok_or_else(|| "a member function with no identity".to_string())?,
                )
            }
            (step, _) => step,
        });
    }
    out.push(Step::Derived(component_id));
    Ok((
        derived,
        Computes::Host(Part {
            part,
            path: path.to_string(),
            binding: root.to_string(),
            steps: out,
        }),
    ))
}

/// **Every page's plan.**
pub fn pages(hirs: &[&Hir], ws: &Workspace, sigs: &Signatures) -> Vec<Planned> {
    let captures = crate::resume::capture_map(hirs, sigs);
    let mut inference = crate::effects::Inference::new(sigs, ws);
    inference.run(hirs);
    let mut out = Vec::new();
    for (unit, hir) in hirs.iter().enumerate() {
        for (id, decl) in hir.all_decls() {
            if decl.kind != DeclKind::Page {
                continue;
            }
            let page = {
                let module = hir.module_of(id).unwrap_or_default();
                if module.is_empty() {
                    decl.name.clone()
                } else {
                    format!("{module}.{}", decl.name)
                }
            };
            out.push(Planned {
                page,
                plan: plan(hirs, ws, sigs, &captures, &inference, unit, id).map(|(p, ..)| p),
            });
        }
    }
    out
}

/// **The member functions some page's plan calls**: each gets a contract of
/// kind `function` and is compiled as a component of its own (ADR-0125).
pub fn members(hirs: &[&Hir], ws: &Workspace, sigs: &Signatures) -> BTreeSet<DefId> {
    let captures = crate::resume::capture_map(hirs, sigs);
    let mut inference = crate::effects::Inference::new(sigs, ws);
    inference.run(hirs);
    let mut out = BTreeSet::new();
    for (unit, hir) in hirs.iter().enumerate() {
        for (id, decl) in hir.all_decls() {
            if decl.kind == DeclKind::Page
                && let Ok((_, m, _)) = plan(hirs, ws, sigs, &captures, &inference, unit, id)
            {
                out.extend(m);
            }
        }
    }
    out
}

/// **The computed parts some page's plan has a host compute** (ADR-0226):
/// each a function the compiler lifts, with a contract of kind `derived`,
/// compiled as a component of its own.
pub fn derived(hirs: &[&Hir], ws: &Workspace, sigs: &Signatures) -> Vec<Derived> {
    let captures = crate::resume::capture_map(hirs, sigs);
    let mut inference = crate::effects::Inference::new(sigs, ws);
    inference.run(hirs);
    let mut out = Vec::new();
    for (unit, hir) in hirs.iter().enumerate() {
        for (id, decl) in hir.all_decls() {
            if decl.kind == DeclKind::Page
                && let Ok((_, _, d)) = plan(hirs, ws, sigs, &captures, &inference, unit, id)
            {
                out.extend(d);
            }
        }
    }
    out
}
