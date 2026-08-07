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
pub mod identity;
pub mod ir;

pub use identity::{
    Anchor, ElementId, IdentityDomain, InstanceFrame, InstancePath, InstanceToken, LocalPartId,
    PartAddress, Partition, TemplateSchemaId,
};
pub use ir::{Chunk, Context, Part, PartEntry, PartId, Template};

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
    /// Two different loop keys derived the same instance token.
    ///
    /// Vanishingly unlikely with a 96-bit keyed derivation, and refused rather
    /// than trusted anyway. Architect ruling, 2026-08-06: *"Do not make
    /// correctness depend purely on probability."* Emitting ambiguous markup
    /// would give two document instances one `PartAddress`, and a patch aimed
    /// at either would reach whichever the runtime indexed last.
    InstanceTokenCollision {
        token: String,
        first: String,
        second: String,
    },
    /// An item has no value for the key its loop declares.
    ///
    /// Its own case, because the repair differs: a duplicate key is two items
    /// claiming one identity, and a missing key is an item with none. Reported
    /// as an empty duplicate would have been the same message for two
    /// different defects.
    MissingLoopKey { each: PartId, field: String },
    /// One `{#each}` rendered two items with the same declared key.
    ///
    /// Diagnosed separately from a token collision, because it is a defect in
    /// the DATA rather than in the derivation and the repair is different: a
    /// key that does not identify is not a key.
    DuplicateLoopKey { each: PartId, key: String },
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
            Blocked::InstanceTokenCollision {
                token,
                first,
                second,
            } => write!(
                f,
                "two loop keys derived the instance token `{token}`: `{first}` and `{second}`"
            ),
            Blocked::MissingLoopKey { each, field } => write!(
                f,
                "loop part {each} declares the key `{field}` and an item has no value \
                 for it; an item with no identity cannot be addressed"
            ),
            Blocked::DuplicateLoopKey { each, key } => write!(
                f,
                "loop part {each} rendered two items with the key `{key}`; a key that \
                 does not identify is not a key"
            ),
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
    /// Who shares an identity with whom.
    ///
    /// A TYPE, not a free-form salt. It used to be `document: String`, and a
    /// caller could pass a session identifier as the domain of a public
    /// fragment — which would give two readers of one shared cache entry
    /// different bytes. `IdentityDomain` makes that composition unrepresentable
    /// rather than documented; see [`identity`].
    domain: IdentityDomain,
    /// The loop instances enclosing what is being rendered, outermost first.
    ///
    /// `pw_document::InstancePath`, not a local tuple: the renderer builds the
    /// same path a patch will later address, and two representations of it
    /// would be two answers to "which instance".
    path: InstancePath,
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

    /// Set the identity domain: who shares an identity with whom.
    pub fn in_domain(mut self, domain: IdentityDomain) -> Env {
        self.domain = domain;
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

    /// Enter a repeatable scope.
    fn within(&self, scope: PartId, instance: InstanceToken) -> Env {
        let mut next = self.clone();
        next.path.push(InstanceFrame { scope, instance });
        next
    }
}

/// Render one template to HTML bytes.
///
/// `others` is every template that may be reached by a `Component` part. A
/// component naming a template not in the set is `Blocked`, not skipped.
/// Render ONE instance of a keyed loop, anchors included.
///
/// What a patch carries when it inserts an item: the markup the server would
/// have produced for that instance in a full render, byte for byte, including
/// its instance boundaries. Producing it here rather than in a patch generator
/// is what keeps one renderer — a second one would drift, and the drift would
/// show up as an inserted item that looks right and cannot be addressed.
///
/// The env must carry the same [`IdentityDomain`] the document was rendered
/// with, or the instance's token will not match the addresses already in the
/// browser's index.
pub fn render_instance(
    t: &Template,
    each: PartId,
    item: &Value,
    env: &Env,
    others: &[Template],
) -> Result<String, Blocked> {
    fn find(chunks: &[Chunk], each: PartId) -> Option<&Part> {
        for c in chunks {
            let Chunk::Dynamic(p) = c else { continue };
            if p.id() == Some(each) {
                return Some(p);
            }
            let nested = match p {
                Part::Conditional {
                    then, otherwise, ..
                } => find(then, each).or_else(|| find(otherwise, each)),
                Part::Each { body, .. } => find(body, each),
                _ => None,
            };
            if nested.is_some() {
                return nested;
            }
        }
        None
    }

    let Some(Part::Each {
        id,
        binding,
        key,
        body,
        ..
    }) = find(&t.chunks, each)
    else {
        return Err(Blocked::UnrepresentedConstruct {
            reason: format!("part {each} is not a keyed loop in `{}`", t.path),
            at: t.path.clone(),
        });
    };
    let Some(field) = key else {
        return Err(Blocked::UnrepresentedConstruct {
            reason: "an unkeyed loop has no addressable instance to render".into(),
            at: t.path.clone(),
        });
    };

    let raw = match item {
        Value::Record(fields) => fields
            .get(field)
            .and_then(|v| v.as_str())
            .unwrap_or_default(),
        other => other.as_str().unwrap_or_default(),
    };
    if raw.is_empty() {
        return Err(Blocked::MissingLoopKey {
            each: *id,
            field: field.clone(),
        });
    }

    let token = env.domain.instance_token(&env.path, *id, &raw);
    let scoped = env.with(binding, item.clone()).within(*id, token.clone());
    let mut out = format!("<!--pw:s{id}@{token}-->");
    emit(body, &scoped, others, &mut out)?;
    out.push_str(&format!("<!--pw:e{id}@{token}-->"));
    Ok(out)
}

/// The instance token an item would get in this template's loop.
///
/// The server needs it to say WHICH instance a patch removes or moves, and
/// deriving it here rather than in the patch generator keeps one derivation.
pub fn instance_token_of(each: PartId, item: &Value, key_field: &str, env: &Env) -> InstanceToken {
    let raw = match item {
        Value::Record(fields) => fields
            .get(key_field)
            .and_then(|v| v.as_str())
            .unwrap_or_default(),
        other => other.as_str().unwrap_or_default(),
    };
    env.domain.instance_token(&env.path, each, &raw)
}

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
            // Per-loop, per-render: two different keys deriving one token, and
            // two items declaring one key, are both refused here.
            let mut tokens: BTreeMap<String, String> = BTreeMap::new();
            let mut seen: BTreeMap<String, String> = BTreeMap::new();

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
                        // A declared key that does not identify is not a key.
                        // Both defects are in the DATA rather than in the
                        // derivation, and they are separate because their
                        // repairs are.
                        if raw.is_empty() {
                            return Err(Blocked::MissingLoopKey {
                                each: *id,
                                field: field.clone(),
                            });
                        }
                        if seen.insert(raw.clone(), raw.clone()).is_some() {
                            return Err(Blocked::DuplicateLoopKey {
                                each: *id,
                                key: raw,
                            });
                        }
                        let token = env.domain.instance_token(&env.path, *id, &raw);
                        // Correctness does not rest on probability. A 96-bit
                        // keyed derivation makes this unreachable; refusing
                        // rather than trusting is what makes that a fact about
                        // the renderer instead of about the odds.
                        if let Some(first) = tokens.insert(token.to_string(), raw.clone())
                            && first != raw
                        {
                            return Err(Blocked::InstanceTokenCollision {
                                token: token.to_string(),
                                first,
                                second: raw,
                            });
                        }
                        out.push_str(&format!("<!--pw:s{id}@{token}-->"));
                        emit(body, &scoped.within(*id, token.clone()), others, out)?;
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
