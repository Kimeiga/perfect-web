//! E7 task 2 — the server renderer.
//!
//! > Given already-checked template IR and values, perfect-web can
//! > deterministically produce correct semantic HTML without Marko.
//!
//! Deliberately narrower than "build the renderer". No resumption, no patches,
//! no document parts, no lazy handlers — those layer on top of a boring,
//! trustworthy HTML serializer, and a serializer that is not boring is not
//! something to layer on.
//!
//! # It does not rediscover meaning
//!
//! The IR arrives with every decision already made: which context a value
//! occupies, which attributes are boolean, which regions are conditional, what
//! a list is keyed by. This crate looks nothing up by name, parses nothing, and
//! guesses nothing. Where the IR does not say, it refuses (see [`Blocked`]).
//!
//! ADR-0018's boundary again: the compiler emits data, the runtime reads it.
//! `pw-core` does not know this crate exists.
//!
//! # Determinism is a property, not an accident
//!
//! Identical IR plus identical values produces identical bytes. Later work
//! depends on it — content hashing, caching, materialization keys, document
//! schemas, resumption identity — so the ordering rules are chosen and written
//! down rather than inherited from a hash map's iteration order.
//!
//! # Nothing invalid disappears
//!
//! A `Blocked` part is an error return, never an empty string. The `Outcome`
//! lesson: a renderer that omits what it did not understand produces a page
//! that looks correct and is missing something, and no test of the page finds
//! it.

pub mod escape;
pub mod ir;

pub use ir::{Anchor, Chunk, Context, ElementId, Part, PartEntry, PartId, Template};

use std::collections::BTreeMap;

/// Why a render produced no bytes.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Blocked {
    /// The IR carries a part an earlier stage could not represent.
    UnrepresentedConstruct { reason: String, at: String },
    /// A part names a value the environment does not have.
    ///
    /// An error rather than an empty string, for the same reason: a missing
    /// value renders as a page with a hole in it, and the hole looks like
    /// content that happened to be empty.
    MissingValue { path: String },
    /// A `RawHtml` part whose capability the caller did not present.
    UnauthorisedRawHtml { capability: String },
    /// A template naming another that is not in the set given.
    UnknownComponent { path: String },
}

impl std::fmt::Display for Blocked {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Blocked::UnrepresentedConstruct { reason, at } => {
                write!(f, "unrepresented construct at `{at}`: {reason}")
            }
            Blocked::MissingValue { path } => write!(f, "no value for `{path}`"),
            Blocked::UnauthorisedRawHtml { capability } => {
                write!(f, "raw HTML requires the `{capability}` capability")
            }
            Blocked::UnknownComponent { path } => write!(f, "no template named `{path}`"),
        }
    }
}

/// A value a template can render.
#[derive(Debug, Clone, PartialEq)]
pub enum Value {
    Text(String),
    Bool(bool),
    Int(i64),
    List(Vec<Value>),
    Record(BTreeMap<String, Value>),
    /// Bytes to emit verbatim, and the capability that authorises it.
    ///
    /// A separate variant, not a flag on `Text`. Architect ruling: raw HTML
    /// "should require a distinct trusted type/capability rather than being an
    /// option on normal strings". A boolean argument is something a caller can
    /// pass by accident; a different type is not.
    Raw {
        html: String,
        capability: String,
    },
}

impl Value {
    /// How this value reads in text or attribute position.
    fn as_str(&self) -> Option<String> {
        Some(match self {
            Value::Text(s) => s.clone(),
            Value::Int(i) => i.to_string(),
            Value::Bool(b) => b.to_string(),
            // A list or a record has no text form. `None` becomes
            // `MissingValue` rather than `"[object Object]"`.
            _ => return None,
        })
    }

    fn truthy(&self) -> bool {
        match self {
            Value::Bool(b) => *b,
            Value::Int(i) => *i != 0,
            Value::Text(s) => !s.is_empty(),
            Value::List(v) => !v.is_empty(),
            Value::Record(_) | Value::Raw { .. } => true,
        }
    }
}

/// The values a render has, by path.
#[derive(Debug, Clone, Default)]
pub struct Env {
    values: BTreeMap<String, Value>,
    /// Capabilities the caller presented, for `RawHtml` parts.
    granted: Vec<String>,
    /// What makes this document's instance tokens its own.
    ///
    /// Architect ruling, 2026-08-06: an instance token "must not reveal the
    /// source key and should not correlate instances across unrelated
    /// documents". The second half needs something document-specific in the
    /// derivation, and the server is what knows it — store 47, session s-1.
    ///
    /// It is an INPUT rather than a random value, because determinism is a gate
    /// (E7-2 gate 5): identical IR plus identical values must produce identical
    /// bytes. A salt drawn from a generator would break content addressing,
    /// caching and materialization keys all at once.
    document: String,
    /// The loop instances enclosing what is being rendered, outermost first.
    ///
    /// Generic on purpose. A frame comes from a repeatable scope, and today
    /// that is a keyed `Each`; later it can be a component instance or a
    /// streamed one without changing the address model.
    path: Vec<(u32, String)>,
}

impl Env {
    pub fn new() -> Env {
        Env::default()
    }

    pub fn set(mut self, path: &str, value: Value) -> Env {
        self.values.insert(path.to_string(), value);
        self
    }

    /// Present a capability. Without it, a `RawHtml` part is refused.
    pub fn grant(mut self, capability: &str) -> Env {
        self.granted.push(capability.to_string());
        self
    }

    /// Identify this document instance, for instance-token derivation.
    pub fn document(mut self, id: &str) -> Env {
        self.document = id.to_string();
        self
    }

    /// `item.name` where `item` is a record, or a whole path set directly.
    fn get(&self, path: &str) -> Option<&Value> {
        if let Some(v) = self.values.get(path) {
            return Some(v);
        }
        let (base, field) = path.split_once('.')?;
        match self.values.get(base)? {
            Value::Record(fields) => fields.get(field),
            _ => None,
        }
    }

    fn with(&self, name: &str, value: Value) -> Env {
        let mut next = self.clone();
        next.values.insert(name.to_string(), value);
        next
    }

    /// Enter a loop instance.
    fn within(&self, each: PartId, token: &str) -> Env {
        let mut next = self.clone();
        next.path.push((each.0, token.to_string()));
        next
    }
}

/// An opaque, document-scoped identifier for one instance of a repeatable
/// scope.
///
/// Architect ruling, 2026-08-06:
///
/// > Same declared loop key under the same parent instance within the same
/// > document generation → same `InstanceToken`. Different key or parent →
/// > different token. Token must not reveal the source key and should not
/// > correlate instances across unrelated documents.
///
/// So the derivation takes the document, the enclosing path, the loop, and the
/// key — and returns none of them. Putting the raw key in the markup would be
/// the same exposure whether it went in an attribute or a comment: both are
/// read by anything that can read the document, and a cart's item id is a
/// domain identifier.
///
/// FNV-1a truncated to 32 bits. Not a secret and not claimed to be one: it
/// hides a key from a reader of the markup, and a party who already knows the
/// document and the candidate keys can confirm a guess. What it prevents is the
/// key being *published*, which is the thing that happens by accident.
fn instance_token(document: &str, path: &[(u32, String)], each: PartId, key: &str) -> String {
    let mut h: u64 = 0xcbf2_9ce4_8422_2325;
    let mut feed = |bytes: &[u8]| {
        for b in bytes {
            h ^= *b as u64;
            h = h.wrapping_mul(0x0000_0100_0000_01b3);
        }
    };
    feed(document.as_bytes());
    for (p, t) in path {
        feed(&p.to_le_bytes());
        feed(t.as_bytes());
    }
    feed(&each.0.to_le_bytes());
    feed(b"\x1f");
    feed(key.as_bytes());
    format!("{:08x}", (h ^ (h >> 32)) as u32)
}

/// Render one template to HTML bytes.
///
/// `others` is every template that may be reached by a `Component` part. A
/// component naming a template not in the set is `Blocked`, not skipped.
pub fn render(t: &Template, env: &Env, others: &[Template]) -> Result<String, Blocked> {
    let mut out = String::new();
    emit(&t.chunks, env, others, &mut out)?;
    Ok(out)
}

fn emit(chunks: &[Chunk], env: &Env, others: &[Template], out: &mut String) -> Result<(), Blocked> {
    for c in chunks {
        match c {
            Chunk::Static(s) => out.push_str(s),
            Chunk::Dynamic(p) => emit_part(p, env, others, out)?,
        }
    }
    Ok(())
}

fn emit_part(p: &Part, env: &Env, others: &[Template], out: &mut String) -> Result<(), Blocked> {
    match p {
        Part::Blocked { reason, at } => Err(Blocked::UnrepresentedConstruct {
            reason: reason.clone(),
            at: at.clone(),
        }),

        Part::Text { id, value, context } => {
            let v = env.get(value).ok_or(Blocked::MissingValue {
                path: value.clone(),
            })?;
            // A raw value in text position is still raw — its capability is
            // what authorises it, and reaching it through a `Text` part does
            // not change that.
            if let Value::Raw { html, capability } = v {
                if !env.granted.iter().any(|g| g == capability) {
                    return Err(Blocked::UnauthorisedRawHtml {
                        capability: capability.clone(),
                    });
                }
                out.push_str(html);
                return Ok(());
            }
            let s = v.as_str().ok_or(Blocked::MissingValue {
                path: value.clone(),
            })?;
            // A range anchor, because a text part can be empty and an empty
            // range still needs a place to reappear. An element attribute
            // cannot express "nothing, here".
            out.push_str(&format!("<!--pw:s{id}-->"));
            out.push_str(&escaped(&s, *context));
            out.push_str(&format!("<!--pw:e{id}-->"));
            Ok(())
        }

        Part::Attribute {
            name,
            value,
            context,
            ..
        } => {
            let v = env.get(value).ok_or(Blocked::MissingValue {
                path: value.clone(),
            })?;
            let s = v.as_str().ok_or(Blocked::MissingValue {
                path: value.clone(),
            })?;
            out.push_str(&format!("{name}=\"{}\"", escaped(&s, *context)));
            Ok(())
        }

        // False means ABSENT, not empty: `disabled=""` is disabled.
        Part::BooleanAttribute { name, value, .. } => {
            let v = env.get(value).ok_or(Blocked::MissingValue {
                path: value.clone(),
            })?;
            if v.truthy() {
                out.push_str(name);
            } else if out.ends_with(' ') {
                // Remove the separator the IR emitted for an attribute that
                // turned out not to exist, so the bytes do not carry a trailing
                // space whose presence depends on a value.
                out.pop();
            }
            Ok(())
        }

        // Behaviour, not markup. The browser runtime attaches it after
        // `decide`; the server emits nothing, because the element already
        // carries the `data-pw` that says which element it is.
        Part::Event { .. } => Ok(()),

        Part::Conditional {
            id,
            value,
            then,
            otherwise,
        } => {
            let v = env.get(value).ok_or(Blocked::MissingValue {
                path: value.clone(),
            })?;
            let branch = if v.truthy() { then } else { otherwise };
            out.push_str(&format!("<!--pw:s{id}-->"));
            emit(branch, env, others, out)?;
            out.push_str(&format!("<!--pw:e{id}-->"));
            Ok(())
        }

        Part::Each {
            id,
            collection,
            binding,
            key,
            body,
        } => {
            let v = env.get(collection).ok_or(Blocked::MissingValue {
                path: collection.clone(),
            })?;
            let Value::List(items) = v else {
                return Err(Blocked::MissingValue {
                    path: collection.clone(),
                });
            };
            // The loop's own range, so the whole list is addressable — an
            // unkeyed list is replaced wholesale, and that is where.
            out.push_str(&format!("<!--pw:s{id}-->"));
            for (i, item) in items.iter().enumerate() {
                let scoped = env.with(binding, item.clone());
                match key {
                    // A KEYED list: each instance gets its own boundaries and
                    // an opaque token, so one instance can be addressed,
                    // reordered or removed without touching its neighbours.
                    Some(field) => {
                        let raw = match item {
                            Value::Record(fields) => fields
                                .get(field)
                                .and_then(|v| v.as_str())
                                .unwrap_or_default(),
                            other => other.as_str().unwrap_or_default(),
                        };
                        let token = instance_token(&env.document, &env.path, *id, &raw);
                        out.push_str(&format!("<!--pw:s{id}@{token}-->"));
                        emit(body, &scoped.within(*id, &token), others, out)?;
                        out.push_str(&format!("<!--pw:e{id}@{token}-->"));
                    }
                    // An UNKEYED list renders and promises nothing about
                    // per-item identity. No instance boundaries, because an
                    // identity that is really a position is worse than none:
                    // it looks addressable and moves when the list does.
                    None => {
                        let _ = i;
                        emit(body, &scoped, others, out)?;
                    }
                }
            }
            out.push_str(&format!("<!--pw:e{id}-->"));
            Ok(())
        }

        Part::Component { id, path, args } => {
            let t = others
                .iter()
                .find(|t| t.path == *path)
                .ok_or(Blocked::UnknownComponent { path: path.clone() })?;
            let mut child = Env::new();
            child.granted = env.granted.clone();
            for (param, source) in args {
                let v = env.get(source).ok_or(Blocked::MissingValue {
                    path: source.clone(),
                })?;
                child = child.set(param, v.clone());
            }
            let rendered = render(t, &child, others)?;
            out.push_str(&format!("<!--pw:s{id}-->"));
            out.push_str(&rendered);
            out.push_str(&format!("<!--pw:e{id}-->"));
            Ok(())
        }

        Part::RawHtml {
            id,
            value,
            capability,
        } => {
            if !env.granted.iter().any(|g| g == capability) {
                return Err(Blocked::UnauthorisedRawHtml {
                    capability: capability.clone(),
                });
            }
            let v = env.get(value).ok_or(Blocked::MissingValue {
                path: value.clone(),
            })?;
            out.push_str(&format!("<!--pw:s{id}-->"));
            match v {
                Value::Raw { html, .. } => out.push_str(html),
                other => {
                    let s = other.as_str().ok_or(Blocked::MissingValue {
                        path: value.clone(),
                    })?;
                    out.push_str(&s);
                }
            }
            out.push_str(&format!("<!--pw:e{id}-->"));
            Ok(())
        }
    }
}

/// The one place a context becomes an escaping function.
///
/// A single `match`, so "which escaping did this value get" has one answer that
/// a reader can check, and adding a context without deciding is a compile
/// error rather than a default.
fn escaped(value: &str, context: Context) -> String {
    match context {
        Context::Text => escape::text(value),
        Context::Attribute => escape::attribute(value),
        Context::Url => escape::url(value),
        Context::Style => escape::style(value),
        Context::RawHtml => value.to_string(),
    }
}
