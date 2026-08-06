//! E4 — the resource manifest schema (charter §14 M4 task 2).
//!
//! The manifest is the **artifact that connects the compiler to the runtime**.
//! Before it, `hir::Policy` recorded what an author wrote and
//! `pw_resource::Manifest` was built by hand in tests, with nothing in between.
//!
//! It is data, deliberately. A generator that emitted Rust calling the runtime
//! would tie the compiler to one host; a serialized manifest is readable by the
//! Rust runtime today and by whatever E8 chooses later.
//!
//! # What a policy value means
//!
//! Policy values are parsed here, not in the parser: `30.seconds` is a duration
//! only because `freshness` says so, and `one_per_key` is a concurrency mode
//! only because `concurrency` says so. Parsing them in the grammar would bake
//! one policy's vocabulary into the syntax of every other.
//!
//! **A value that cannot be parsed is reported, never guessed.** A manifest with
//! a silently defaulted freshness is worse than no manifest: the runtime would
//! serve stale data and nothing would say why.

use serde::{Deserialize, Serialize};

use crate::hir::{Decl, DeclKind, Hir};

/// Milliseconds. The unit the runtime works in.
pub type Millis = u64;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Privacy {
    Public,
    Session,
    Private,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CachePartition {
    /// Readable by every user.
    Shared,
    /// One entry per session.
    Private,
    /// Not cached.
    None,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Consistency {
    Snapshot,
    Strong,
    Eventual,
    /// A session guarantee: a reader sees its own writes, and nothing weaker.
    /// Distinct from `Strong` — other sessions may still see an older value.
    ReadYourWrites,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Concurrency {
    /// At most one request per key at a time.
    OnePerKey,
    /// Requests may overlap.
    Parallel,
}

/// A retry policy, as written.
///
/// `forever` is representable so the *rule* that rejects it (PW0313) stays in
/// the checker. A schema that could not express an illegal policy would move
/// the rule into the parser, where it could not explain itself.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Retry {
    None,
    Forever,
    Bounded {
        strategy: String,
        max: u32,
        jitter: bool,
    },
}

/// Everything the runtime needs to behave as the declaration says.
///
/// Field for field, charter §14 M4 task 2.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Manifest {
    /// The declaration's name — the semantic name, not a generated symbol.
    pub name: String,
    /// `query`, `command`, `subscription`, `resource`, `task`.
    pub kind: String,
    /// Parameter types, in order: what identifies one row.
    pub key_type: Vec<String>,
    pub result_type: Option<String>,
    /// The `E` of a `Result<T, E>` return, when there is one.
    pub error_type: Option<String>,
    pub privacy: Privacy,
    pub freshness: Option<Millis>,
    pub consistency: Option<Consistency>,
    pub cache_partition: CachePartition,
    pub timeout: Option<Millis>,
    pub retry: Retry,
    pub concurrency: Option<Concurrency>,
    /// Events that invalidate a cached value, as written.
    pub invalidation: Vec<String>,
    /// Worlds this may run in, or empty when unconstrained.
    pub placement: Vec<String>,
    /// Effects the declaration claims, as written.
    pub effects: Vec<String>,
}

/// A policy value the schema could not interpret.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Unparsed {
    pub declaration: String,
    pub policy: String,
    pub value: String,
    pub reason: &'static str,
}

#[derive(Debug, Clone, Default)]
pub struct Built {
    pub manifests: Vec<Manifest>,
    /// Policies that were written but not understood. Reported, so a manifest
    /// is never quietly incomplete.
    pub unparsed: Vec<Unparsed>,
}

/// Build a manifest for every resource-shaped declaration in a module.
pub fn build(hir: &Hir) -> Built {
    let mut out = Built::default();
    for (_, decl) in hir.all_decls() {
        if !is_resource(decl.kind) {
            continue;
        }
        let (m, mut bad) = manifest_of(decl);
        out.manifests.push(m);
        out.unparsed.append(&mut bad);
    }
    out
}

fn is_resource(kind: DeclKind) -> bool {
    matches!(
        kind,
        DeclKind::Query
            | DeclKind::Command
            | DeclKind::Subscription
            | DeclKind::Resource
            | DeclKind::Task
    )
}

fn kind_name(kind: DeclKind) -> &'static str {
    match kind {
        DeclKind::Query => "query",
        DeclKind::Command => "command",
        DeclKind::Subscription => "subscription",
        DeclKind::Resource => "resource",
        DeclKind::Task => "task",
        _ => "other",
    }
}

fn manifest_of(decl: &Decl) -> (Manifest, Vec<Unparsed>) {
    let mut bad = Vec::new();
    let mut note = |policy: &str, value: &str, reason: &'static str| {
        bad.push(Unparsed {
            declaration: decl.name.clone(),
            policy: policy.to_string(),
            value: value.to_string(),
            reason,
        });
    };

    let value = |name: &str| decl.policy(name).map(|p| p.value.trim().to_string());

    let freshness = value("freshness").and_then(|v| match parse_duration(&v) {
        Some(ms) => Some(ms),
        None => {
            note("freshness", &v, "not a duration");
            None
        }
    });
    let timeout = value("timeout").and_then(|v| match parse_duration(&v) {
        Some(ms) => Some(ms),
        None => {
            note("timeout", &v, "not a duration");
            None
        }
    });
    let consistency = value("consistency").and_then(|v| match v.as_str() {
        "snapshot" => Some(Consistency::Snapshot),
        "strong" => Some(Consistency::Strong),
        "eventual" => Some(Consistency::Eventual),
        "read_your_writes" => Some(Consistency::ReadYourWrites),
        _ => {
            note("consistency", &v, "not a known consistency mode");
            None
        }
    });
    let concurrency = value("concurrency").and_then(|v| match v.as_str() {
        "one_per_key" => Some(Concurrency::OnePerKey),
        "parallel" => Some(Concurrency::Parallel),
        _ => {
            note("concurrency", &v, "not a known concurrency mode");
            None
        }
    });
    let cache_partition = match value("cache").as_deref() {
        None => CachePartition::None,
        Some("shared") => CachePartition::Shared,
        Some("private") => CachePartition::Private,
        Some(other) => {
            note("cache", other, "not a known cache partition");
            CachePartition::None
        }
    };
    let retry = match value("retry") {
        None => Retry::None,
        Some(v) => match parse_retry(&v) {
            Some(r) => r,
            None => {
                note("retry", &v, "not a known retry policy");
                Retry::None
            }
        },
    };

    let privacy = match decl.visibility.as_deref() {
        Some("session") => Privacy::Session,
        Some("private") => Privacy::Private,
        _ => Privacy::Public,
    };

    let invalidation = ["invalidates", "invalidates_on"]
        .iter()
        .filter_map(|k| value(k))
        .collect();

    let placement = value("placement").into_iter().collect();

    let (result_type, error_type) = split_result(decl);

    let m = Manifest {
        name: decl.name.clone(),
        kind: kind_name(decl.kind).to_string(),
        key_type: decl.params.iter().filter_map(|p| p.ty.clone()).collect(),
        result_type,
        error_type,
        privacy,
        freshness,
        consistency,
        cache_partition,
        timeout,
        retry,
        concurrency,
        invalidation,
        placement,
        effects: decl
            .declared_effects
            .as_deref()
            .unwrap_or_default()
            .iter()
            .map(|e| e.path.clone())
            .collect(),
    };
    (m, bad)
}

/// `Result<Store, StoreError>` splits into a result and an error type.
///
/// Charter §14 M4 task 2 lists them as separate manifest fields, and they are:
/// a caller handles the error side explicitly, so a manifest that reported
/// `Result` as the result type and nothing as the error type would describe a
/// declaration nobody wrote.
fn split_result(decl: &Decl) -> (Option<String>, Option<String>) {
    let Some(head) = decl.ret.as_deref() else {
        return (None, None);
    };
    match (head, decl.ret_args.as_slice()) {
        ("Result", [ok, err]) => (Some(ok.clone()), Some(err.clone())),
        _ => (Some(head.to_string()), None),
    }
}

/// `30.seconds`, `2.seconds`, `500.milliseconds`, `5.minutes`.
fn parse_duration(v: &str) -> Option<Millis> {
    let (n, unit) = v.split_once('.')?;
    let n: u64 = n.trim().parse().ok()?;
    Some(match unit.trim() {
        "milliseconds" | "millisecond" | "ms" => n,
        "seconds" | "second" => n * 1_000,
        "minutes" | "minute" => n * 60_000,
        "hours" | "hour" => n * 3_600_000,
        _ => return None,
    })
}

/// `forever`, `transport_only(max = 2, jitter = true)`,
/// `bounded_exponential(max = 3, jitter = true)`.
fn parse_retry(v: &str) -> Option<Retry> {
    let v = v.trim();
    if v == "forever" {
        return Some(Retry::Forever);
    }
    if v == "none" {
        return Some(Retry::None);
    }
    let (strategy, args) = v.split_once('(')?;
    let args = args.strip_suffix(')')?;

    let mut max = None;
    let mut jitter = false;
    for arg in args.split(',') {
        let (k, val) = arg.split_once('=')?;
        match k.trim() {
            "max" => max = val.trim().parse::<u32>().ok(),
            "jitter" => jitter = val.trim() == "true",
            _ => {}
        }
    }
    Some(Retry::Bounded {
        strategy: strategy.trim().to_string(),
        max: max?,
        jitter,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn durations_parse_in_every_unit_the_corpus_uses() {
        assert_eq!(parse_duration("30.seconds"), Some(30_000));
        assert_eq!(parse_duration("2.seconds"), Some(2_000));
        assert_eq!(parse_duration("180.milliseconds"), Some(180));
        assert_eq!(parse_duration("5.minutes"), Some(300_000));

        // Not guessed. A duration the schema cannot read is reported, because a
        // silently defaulted freshness makes the runtime serve stale data with
        // nothing to explain why.
        assert_eq!(parse_duration("soon"), None);
        assert_eq!(parse_duration("30.fortnights"), None);
        assert_eq!(parse_duration("30"), None);
    }

    #[test]
    fn retry_policies_parse_including_the_illegal_one() {
        assert_eq!(
            parse_retry("bounded_exponential(max = 3, jitter = true)"),
            Some(Retry::Bounded {
                strategy: "bounded_exponential".into(),
                max: 3,
                jitter: true
            })
        );
        assert_eq!(
            parse_retry("transport_only(max = 2, jitter = true)"),
            Some(Retry::Bounded {
                strategy: "transport_only".into(),
                max: 2,
                jitter: true
            })
        );
        // `forever` is representable ON PURPOSE. The rule that rejects it
        // (PW0313) belongs in the checker, where it can explain itself; a schema
        // that could not express it would move the rule into parsing.
        assert_eq!(parse_retry("forever"), Some(Retry::Forever));
        assert_eq!(parse_retry("bounded_exponential(jitter = true)"), None);
    }

    #[test]
    fn every_consistency_mode_the_corpus_uses_is_known() {
        // `read_your_writes` was missing, and the corpus coverage test found it
        // — not this list. The list is here so the vocabulary is visible in one
        // place, but the corpus is what decides it.
        for v in ["snapshot", "eventual", "read_your_writes", "strong"] {
            let d = crate::hir::Decl {
                name: "Q".into(),
                kind: DeclKind::Query,
                params: vec![],
                ret: None,
                ret_args: vec![],
                variants: None,
                fields: None,
                opaque_of: None,
                policies: vec![crate::hir::Policy {
                    name: "consistency".into(),
                    value: v.into(),
                    span: 0..0,
                }],
                imports: vec![],
                visibility: None,
                declared_effects: None,
                body: None,
                children: vec![],
            };
            let (m, bad) = manifest_of(&d);
            assert!(bad.is_empty(), "`{v}` must be understood: {bad:?}");
            assert!(m.consistency.is_some());
        }
    }

    #[test]
    fn a_manifest_round_trips_through_json() {
        // The manifest is an artifact, so it has to survive being written and
        // read back — that is the whole point of it being data.
        let m = Manifest {
            name: "Store".into(),
            kind: "query".into(),
            key_type: vec!["StoreId".into()],
            result_type: Some("Store".into()),
            error_type: None,
            privacy: Privacy::Public,
            freshness: Some(30_000),
            consistency: Some(Consistency::Snapshot),
            cache_partition: CachePartition::Shared,
            timeout: Some(2_000),
            retry: Retry::Bounded {
                strategy: "bounded_exponential".into(),
                max: 3,
                jitter: true,
            },
            concurrency: Some(Concurrency::OnePerKey),
            invalidation: vec!["StoreChanged(id)".into()],
            placement: vec![],
            effects: vec!["database.read".into()],
        };
        let json = serde_json::to_string_pretty(&m).expect("serialize");
        let back: Manifest = serde_json::from_str(&json).expect("deserialize");
        assert_eq!(m, back);
        assert!(json.contains("\"one_per_key\""), "{json}");
    }
}
