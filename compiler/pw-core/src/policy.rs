//! **What a policy head's value MEANS, as data.**
//!
//! Architect ruling, 2026-08-10:
//!
//! > Give policy/CSS call-shaped values contextual semantic resolution; don't
//! > merely exclude them. […] Have each policy operator define the semantic
//! > kind of each argument. […] Being inside a policy never exempts an
//! > executable term from resolution, but not every call-shaped policy argument
//! > is an executable term.
//!
//! # The hole this closes
//!
//! `pw-syntax`'s `POLICY_KEYWORDS` says which heads exist, and an unrecognised
//! head is already `PW0322`. Nothing said what a head's VALUE is. So
//! `hir::Policy` carries `value: String` — the characters after the keyword —
//! and every consumer that wanted meaning did its own `trim()` and `==`.
//!
//! Two consequences, both measured before this file existed:
//!
//! ```text
//! optimistic cart.add(item, quantity)   executable code, never parsed,
//! rollback   cart.remove(item)          never resolved, never checked
//! ```
//!
//! `cart` names nothing in `add_to_cart`'s scope. Two real calls into the void,
//! invisible to every analysis, because the text was never anything but text.
//!
//! And the inverse, from `tests/policy_consumers.rs`: where a policy value IS
//! lowered into the body — a page's statements, a `handler_policy` block — the
//! analyses cannot tell it from executable code, and a page's declared
//! authority moves because of what a policy value spells.
//!
//! One table answers both. A position this table calls `Term` gets ordinary
//! resolution, including the unresolved-name error. A position it calls
//! anything else does not, and the reason is recorded rather than implied.
//!
//! # What this file is NOT
//!
//! It is not the `PolicyExpr`/`TermExpr` split. That changes the HIR and moves
//! every consumer; this is the table that split has to be driven by, landed
//! first so the split is a mechanical change against a reviewed classification
//! rather than a judgement call per policy.

use std::collections::BTreeSet;

/// What kind of value a policy head takes.
///
/// The variants are semantic categories, not syntactic ones: `Word` and
/// `Strategy` are both bare identifiers in the source and are different facts
/// about the program.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Domain {
    /// A word from a closed set: `cache shared`, `consistency snapshot`. The
    /// set is the domain's whole content — an unlisted word is an error, not
    /// an extension point.
    Word(&'static [&'static str]),
    /// `30.seconds`, `0.seconds`.
    Duration,
    /// A duration greater than zero: `2.seconds` (ADR-0109).
    Budget,
    /// A type the program declares: `idempotent_by InteractionId`.
    TypeRef,
    /// A parameter of the declaration being written: `key id`. A cache key
    /// may also name a partition it separates, `key store, user` (PW5004):
    /// one of `privacy::PARTITIONS`.
    ///
    /// **Not** a term. `id` here selects a cache-key dimension; it is not
    /// evaluated, and resolving it as a term would make a cache key a function
    /// call.
    ParamRef,
    /// A declared resource, with its key arguments: `depends_on Menu(id)`.
    ResourceRef,
    /// A declared event and the values it carries: `emits
    /// CartChanged(current_session())`. Each value is a term, evaluated where
    /// the declaration runs (ADR-0088).
    EventRef,
    /// A declared event a declaration listens for, and what each of its
    /// values must equal: `invalidates_on InventoryChanged(id, _)`. Each
    /// argument is one of the declaration's parameters, binding that part of
    /// its key, or `_`, any value (ADR-0091). Not an `EventRef`: nothing in it
    /// is evaluated.
    Listener,
    /// A predicate over the caller: `requires SignedIn, OwnsOrder(order)`.
    PredicateRef,
    /// **An optimistic transition.** `optimistic Cart(current_session()) as
    /// cart => cart.add(item, quantity)` — a resource ENTRY, a binder for its
    /// current value, and a pure expression producing the speculative one.
    ///
    /// Architect ruling, 2026-08-11 (ADR-0025): it identifies an entry rather
    /// than a value of a type, because two entries can share a type; and it has
    /// no written inverse, because the platform restores the value it held and
    /// a hand-written inverse is generally false.
    Transition,
    /// **Written by nobody.** `rollback` was a policy until 2026-08-11 and is
    /// now derived: the runtime restores the resource value it held before the
    /// speculative one. A source that writes it is reported rather than
    /// ignored — an author who describes an inverse is describing something the
    /// platform will not use.
    Derived,
    /// A named operator with a signature: `retry bounded_exponential(max = 3)`.
    /// The words beside it are the values it may be instead: `retry forever`
    /// (refused, PW0313) and `retry none`.
    Operator(&'static [Op], &'static [&'static str]),
    /// A word from a closed set that may carry a condition: `impact
    /// layout_write when LayoutAffect`. The condition names a type, and is
    /// the effect ontology's (PW5204).
    ConditionedWord(&'static [&'static str]),
    /// An effect as written, type argument included: `capability
    /// database.read<T>`.
    EffectRef,
    /// A quoted interface name: `host "pw:host/database#read"`.
    Str,
    /// One or more worlds: `placement browser, edge, origin`.
    Worlds,
    /// A privacy label constructor applied to a term: `privacy User(consumer)`.
    LabelCtor,
    /// A list of the declaration's own parameters: `inputs values, width,
    /// height`.
    ParamList,
    /// A bare word with no value: `affine`. Present or absent is the whole
    /// fact.
    Flag,
    /// **A block of executable code**, optionally binding a parameter:
    /// `release(handle) { Maps.destroy(handle) }`, `draw(ctx) { .. }`.
    ///
    /// The second `Term`-carrying domain, and the one most easily missed: it
    /// does not look like a policy value at all, and its contents are already
    /// walked by every body analysis. A resource's `release` block is where
    /// cleanup lives — R-044's whole subject — so a rule that skipped policy
    /// values wholesale would stop seeing it.
    Body,
    /// A CSS length: `intrinsic_height 24.px`.
    Length,
    /// A route pattern: `route "/stores/{id}"`. Not a `Str`: its holes name the
    /// declaration's parameters, and charter §8.2 checks every internal link
    /// against the set of these.
    RoutePattern,
    /// A declaration this program defines, named for attribution:
    /// `attributes_forced_layout_to VendorMap`.
    DeclRef,
}

/// One policy operator, with the semantic kind of each argument.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Op {
    /// Stable identity. Never derived from the spelling at a use site — the
    /// spelling is how the operator is FOUND, and this is what it IS.
    pub id: &'static str,
    pub name: &'static str,
    pub args: &'static [Arg],
}

/// What one argument of a policy operator is.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Arg {
    /// `max = 3` — a named literal of one kind. The name is part of the
    /// signature, and an argument that is not `required` may be left out.
    Named {
        name: &'static str,
        kind: Literal,
        required: bool,
    },
    /// A field of the declaration's result, paired with a strategy:
    /// `merge_by_field(notes = last_write_wins)`. Repeatable.
    FieldStrategy(&'static [&'static str]),
    /// A dimension of the thing being identified: `content_address(code,
    /// captures)`. A word from a closed set, not a value.
    Dimension(&'static [&'static str]),
    /// **A real executable term.** Receives ordinary resolution.
    Term,
}

/// **What a named literal argument is** (ADR-0089).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Literal {
    /// A count from 1: `max = 3`. The manifest holds it as a `u32`.
    Count,
    /// `true` or `false`: `jitter = true`.
    Truth,
}

impl Literal {
    /// Is `written` a literal of this kind?
    pub fn fits(self, written: &str) -> bool {
        match self {
            Literal::Count => written.parse::<u32>().is_ok_and(|n| n >= 1),
            Literal::Truth => matches!(written, "true" | "false"),
        }
    }

    /// What a literal of this kind is, for a message.
    pub fn describe(self) -> &'static str {
        match self {
            Literal::Count => "a count from 1",
            Literal::Truth => "`true` or `false`",
        }
    }
}

/// A retry's bound and whether its delays vary. The bound is what makes a
/// retry bounded (PW0313), so it is required.
const RETRY_ARGS: &[Arg] = &[
    Arg::Named {
        name: "max",
        kind: Literal::Count,
        required: true,
    },
    Arg::Named {
        name: "jitter",
        kind: Literal::Truth,
        required: false,
    },
];

/// The strategies `merge_by_field` accepts per field.
const MERGE: &[&str] = &["last_write_wins", "maximum", "minimum", "union"];

/// What `content_address` may hash over.
const IDENTITY_DIMENSIONS: &[&str] = &["code", "captures", "document"];

const RETRY_OPS: &[Op] = &[
    Op {
        id: "policy.retry.bounded_exponential",
        name: "bounded_exponential",
        args: RETRY_ARGS,
    },
    Op {
        id: "policy.retry.transport_only",
        name: "transport_only",
        args: RETRY_ARGS,
    },
    // A fixed delay between attempts. The corpus writes it as a retry a
    // valid program declares (`retry_not_idempotent`'s neighbour), and the
    // manifest carries a strategy by name (ADR-0089, ruling needed).
    Op {
        id: "policy.retry.fixed",
        name: "fixed",
        args: RETRY_ARGS,
    },
];

const CONFLICT_OPS: &[Op] = &[Op {
    id: "policy.conflict.merge_by_field",
    name: "merge_by_field",
    args: &[Arg::FieldStrategy(MERGE)],
}];

const IDENTITY_OPS: &[Op] = &[Op {
    id: "policy.identity.content_address",
    name: "content_address",
    args: &[Arg::Dimension(IDENTITY_DIMENSIONS)],
}];

/// The domain of every policy head the parser recognises.
///
/// Exhaustive by test: `every_policy_keyword_has_a_domain` fails closed when
/// `pw_syntax::POLICY_KEYWORDS` grows. That is the countermeasure this project
/// has had to adopt three times — a central table that silently ignores what it
/// does not recognise is how `impact` went a whole commit without meaning.
pub fn domain_of(head: &str) -> Option<Domain> {
    Some(match head {
        // --- caching and freshness
        "cache" => Domain::Word(&["shared", "private"]),
        "partition" => Domain::Word(&["public", "private"]),
        "consistency" => Domain::Word(&["strong", "snapshot", "read_your_writes", "eventual"]),
        "freshness" => Domain::Duration,
        // A budget: a request is given it, and ends when it is spent. Zero
        // ends every request before it starts (ADR-0109).
        "timeout" => Domain::Budget,
        "fallback" => Domain::Word(&["last_known_good", "empty"]),
        "stampede" => Domain::Word(&["single_flight"]),
        "regenerate" => Domain::Word(&["on_invalidation"]),
        "locale" | "tenant" | "policy_version" | "code_version" => {
            Domain::Word(&["included_in_key"])
        }

        // --- keys and concurrency
        "key" | "dedupe_by" => Domain::ParamRef,
        // `parallel` is the manifest's second mode (ADR-0089).
        "concurrency" => Domain::Word(&["one_per_key", "parallel"]),
        // The three PW0325's repair names and R-027 expects; the charter's
        // "cancels or supersedes stale work" (ADR-0089).
        "on_key_change" => Domain::Word(&["cancel", "supersede", "keep"]),
        "idempotent_by" => Domain::TypeRef,

        // --- the dependency graph
        "depends_on" => Domain::ResourceRef,
        "invalidates" => Domain::ResourceRef,
        "invalidates_on" => Domain::Listener,
        "emits" => Domain::EventRef,

        // --- authority and placement
        "requires" => Domain::PredicateRef,
        "placement" => Domain::Worlds,
        "privacy" => Domain::LabelCtor,
        "capability" => Domain::EffectRef,
        "host" => Domain::Str,
        // ADR-0040: the operation the compiler supplies, by its name.
        "intrinsic" => Domain::Str,
        "route" => Domain::RoutePattern,
        "impact" => Domain::ConditionedWord(&[
            "layout_read",
            "layout_write",
            "dom_write",
            "paint_write",
            "compositor",
        ]),

        // --- executable
        "optimistic" => Domain::Transition,
        "rollback" => Domain::Derived,

        // --- operators
        "retry" | "reconnect" => Domain::Operator(RETRY_OPS, &["none", "forever"]),
        "conflict" => Domain::Operator(CONFLICT_OPS, &[]),
        "identity" => Domain::Operator(IDENTITY_OPS, &[]),

        // --- transport, storage, lifecycle
        "transport" => Domain::Word(&["websocket", "sse", "poll"]),
        "delivery" => Domain::Word(&["streamed", "batched"]),
        "storage" => Domain::Word(&["device", "origin"]),
        "offline" => Domain::Word(&["writable", "readable"]),
        "sync" => Domain::Word(&["on_reconnect", "immediate"]),
        "on_conflict_unresolved" => Domain::Word(&["surface_to_user", "refuse"]),
        // `application` is the scope a subscription may not claim inside a
        // component (R-028, PW2004): a word of the domain, refused where it
        // outlives its owner (ADR-0089).
        "scope" => Domain::Word(&["component", "page", "session", "application"]),
        "on_scope_exit" => Domain::Word(&["close", "detach"]),
        "transaction" => Domain::Word(&["serializable", "read_committed"]),
        "captures" => Domain::Word(&["serializable_only"]),
        "on_version_mismatch" => Domain::Word(&["safe_refetch", "refuse"]),
        "load" => Domain::Word(&["on_first_interaction", "eager"]),
        "inputs" => Domain::ParamList,
        "isolated" => Domain::Word(&["true", "false"]),

        // --- resource lifecycle and painting: executable blocks
        "acquire" | "release" | "draw" => Domain::Body,
        // The hooks of an `unsafe.lifecycle { .. }` escape hatch (charter §7.5):
        // executable blocks, written inside the statement's block.
        "on_mount" | "on_unmount" => Domain::Body,
        "affine" => Domain::Flag,
        "intrinsic_height" => Domain::Length,
        "revision" => Domain::Word(&["content_hash"]),
        // Inside an `animate` block: the user preference the animation yields
        // to (charter §7.5A). A statement clause the grammar reads after the
        // keyframes, given its vocabulary here so its value is a word of this
        // domain and not a name to resolve (ADR-0047).
        "respects" => Domain::Word(&["prefers_reduced_motion"]),

        // --- the audit record on an escape hatch
        "because" => Domain::Str,
        "attributes_forced_layout_to" => Domain::DeclRef,

        _ => return None,
    })
}

/// **The execution context a block policy's contents run in.**
///
/// Architect ruling, 2026-08-11: the policy operator's identity selects the
/// form, and downstream resolution receives already-lowered lexical bindings.
/// Nothing recognises `"draw"` or `"release"` by spelling to decide what a
/// block introduces — this table does, once.
///
/// `Draw`, `Acquire` and `Release` get no new legality rules; they carry only
/// the semantics they already had. What changes is that their contents are
/// attributed to their own root rather than to whatever declaration happens to
/// enclose them.
pub fn execution_context(head: &str) -> Option<crate::hir::ExecutionContext> {
    use crate::hir::ExecutionContext as X;
    match domain_of(head)? {
        Domain::Body => Some(match head {
            "draw" => X::Draw,
            "acquire" => X::Acquire,
            "release" => X::Release,
            _ => return None,
        }),
        _ => None,
    }
}

/// Every policy head whose value is, or contains, **executable code**.
///
/// This is the list `PW0024` has to consult. A name written in one of these
/// positions is a term and must resolve; a name written anywhere else is not,
/// and reporting it would be the over-broad rule that was reverted twice.
///
/// The `Term` domains are direct. The rest carry terms in ARGUMENT positions:
/// `emits CartChanged(current_session())` names an event — which is not a term
/// — and passes it one.
pub fn carries_terms(head: &str) -> bool {
    matches!(
        domain_of(head),
        Some(Domain::Transition)
            | Some(Domain::Body)
            | Some(Domain::ResourceRef)
            | Some(Domain::EventRef)
            | Some(Domain::Listener)
            | Some(Domain::PredicateRef)
            | Some(Domain::LabelCtor)
    )
}

/// **A clause that names a declaration and passes it its key** (ADR-0088):
/// the namespace its names are looked up in, and the kinds of declaration
/// it may name. `invalidates Cart(current_session())` names a resource, and
/// `emits CartChanged(current_session())` an event. Each key is a term.
pub fn keyed(head: &str) -> Option<(crate::resolve::Namespace, &'static [crate::hir::DeclKind])> {
    use crate::hir::DeclKind as K;
    use crate::resolve::Namespace;
    match domain_of(head)? {
        Domain::ResourceRef => Some((Namespace::Term, &[K::Query, K::Subscription, K::Resource])),
        Domain::EventRef | Domain::Listener => Some((Namespace::Event, &[K::Event])),
        _ => None,
    }
}

/// **Which declarations a dependency-graph clause belongs to** (ADR-0092),
/// and what they are, for a message.
///
/// ADR-0007 made invalidation explicit: a command emits typed events and
/// invalidates the entries it changes, and a resource or a materialization
/// listens for events and depends on resources. A clause elsewhere means
/// nothing: nothing reads a query's `emits`, and a `fn` is not in the graph.
/// Other heads are not in this table yet (ruling needed).
pub fn declared_by(head: &str) -> Option<(&'static [crate::hir::DeclKind], &'static str)> {
    use crate::hir::DeclKind as K;
    Some(match head {
        "emits" | "invalidates" => (&[K::Command], "a command"),
        "invalidates_on" => (
            &[K::Query, K::Subscription, K::Resource, K::Materialize],
            "a resource or a materialization",
        ),
        "depends_on" => (&[K::Materialize], "a materialization"),
        _ => return None,
    })
}

/// **May `head` be written more than once in one declaration?** Only an
/// effect's `impact`: an effect has as many impacts as it has facets
/// (`impact dom_write`, `impact layout_write when LayoutAffect`). Any other
/// head written twice says two things, and every reader took the first
/// (ADR-0098).
pub fn repeats(head: &str) -> bool {
    matches!(domain_of(head), Some(Domain::ConditionedWord(_)))
}

/// The operator a spelling names, within a head's domain.
///
/// Contextual: `merge_by_field` is an operator under `conflict` and nothing
/// under `retry`. There is no global operator namespace to fall back to, which
/// is what makes an unknown operator an error rather than a silent pass.
pub fn operator(head: &str, spelling: &str) -> Option<&'static Op> {
    match domain_of(head)? {
        Domain::Operator(ops, _) => ops.iter().find(|o| o.name == spelling),
        _ => None,
    }
}

/// **A duration as written: `30.seconds`, `500.milliseconds`, `1.hours`.**
///
/// The one reading, in milliseconds: the manifest reads it, and a value it
/// cannot read is refused where it is written (ADR-0089).
pub fn duration(v: &str) -> Option<u64> {
    let (n, unit) = v.split_once('.')?;
    let n: u64 = n.trim().parse().ok()?;
    let per = match unit.trim() {
        "milliseconds" | "millisecond" | "ms" => 1,
        "seconds" | "second" => 1_000,
        "minutes" | "minute" => 60_000,
        "hours" | "hour" => 3_600_000,
        _ => return None,
    };
    n.checked_mul(per)
}

/// **What is wrong with a policy's value, by its domain** (ADR-0089).
///
/// Architect ruling, 2026-08-07, on an unknown policy head: "Pleris may
/// reject authored semantics; it must never silently erase them." A value its
/// domain does not have is the same failure one level down. `cache Shared`
/// was read as no cache by the manifest, and as no shared cache by the rule
/// that keeps a session's data out of one.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ValueFault {
    /// Not one of the words the domain lists.
    Word(&'static [&'static str]),
    /// Not a duration.
    Duration,
    /// A duration of zero, where the domain is a budget (ADR-0109).
    ZeroBudget,
    /// Not a world.
    World(String),
    /// A flag written with a value.
    Flag,
    /// No operator of that name, where these are the domain's.
    Operator(String, Vec<&'static str>),
    /// An operator's arguments opened and never closed.
    Unclosed(String),
    /// An argument the operator does not take.
    UnknownArgument(&'static str, String),
    /// An argument given twice.
    ArgumentTwice(&'static str, String),
    /// A required argument left out.
    MissingArgument(&'static str, &'static str),
    /// An argument that is not of its kind: `max = "two"`.
    ArgumentKind(&'static str, &'static str, Literal, String),
    /// A value where every argument is written by name.
    Positional(&'static str, String),
    /// A word the operator's argument does not list: a strategy, a dimension.
    ArgumentWord(&'static str, String, &'static [&'static str]),
}

/// **The fault in `value`, as `head`'s domain reads it**, or `None` where the
/// domain has it or is not one this reads. A name the value mentions, a
/// parameter or a type, is resolved where scope is known (`check.rs`).
pub fn value_fault(head: &str, value: &str) -> Option<ValueFault> {
    let value = value.trim();
    match domain_of(head)? {
        Domain::Word(words) => (!words.contains(&value)).then_some(ValueFault::Word(words)),
        Domain::ConditionedWord(words) => {
            let word = value.split_once(" when ").map_or(value, |(w, _)| w).trim();
            (!words.contains(&word)).then_some(ValueFault::Word(words))
        }
        Domain::Duration => duration(value).is_none().then_some(ValueFault::Duration),
        Domain::Budget => match duration(value) {
            None => Some(ValueFault::Duration),
            Some(0) => Some(ValueFault::ZeroBudget),
            Some(_) => None,
        },
        Domain::Worlds => value
            .split(',')
            .map(str::trim)
            .find(|w| !crate::placement::ALL_WORLDS.iter().any(|x| x.name() == *w))
            .map(|w| ValueFault::World(w.to_string())),
        Domain::Flag => (!value.is_empty()).then_some(ValueFault::Flag),
        Domain::Operator(ops, words) => {
            if words.contains(&value) {
                return None;
            }
            let (name, args) = match value.split_once('(') {
                Some((name, rest)) => match rest.trim_end().strip_suffix(')') {
                    Some(args) => (name.trim(), args),
                    None => return Some(ValueFault::Unclosed(name.trim().to_string())),
                },
                None => (value, ""),
            };
            let Some(op) = ops.iter().find(|o| o.name == name) else {
                return Some(ValueFault::Operator(
                    name.to_string(),
                    ops.iter().map(|o| o.name).collect(),
                ));
            };
            operator_fault(op, args)
        }
        _ => None,
    }
}

/// What is wrong with an operator's arguments: each written once, by the
/// name its signature gives it, and of its kind.
fn operator_fault(op: &'static Op, args: &str) -> Option<ValueFault> {
    let written: Vec<&str> = args
        .split(',')
        .map(str::trim)
        .filter(|a| !a.is_empty())
        .collect();
    let mut given: Vec<&str> = Vec::new();
    for a in &written {
        match op.args {
            [Arg::Dimension(words)] => {
                if !words.contains(a) {
                    return Some(ValueFault::ArgumentWord(op.name, a.to_string(), words));
                }
            }
            [Arg::FieldStrategy(strategies)] => {
                let Some((_, strategy)) = a.split_once('=') else {
                    return Some(ValueFault::Positional(op.name, a.to_string()));
                };
                let strategy = strategy.trim();
                if !strategies.contains(&strategy) {
                    return Some(ValueFault::ArgumentWord(
                        op.name,
                        strategy.to_string(),
                        strategies,
                    ));
                }
            }
            _ => {
                let Some((name, v)) = a.split_once('=') else {
                    return Some(ValueFault::Positional(op.name, a.to_string()));
                };
                let (name, v) = (name.trim(), v.trim());
                let Some((kind, _)) = named(op, name) else {
                    return Some(ValueFault::UnknownArgument(op.name, name.to_string()));
                };
                if given.contains(&name) {
                    return Some(ValueFault::ArgumentTwice(op.name, name.to_string()));
                }
                given.push(name);
                if !kind.fits(v) {
                    let arg = named(op, name).map_or("", |(_, n)| n);
                    return Some(ValueFault::ArgumentKind(op.name, arg, kind, v.to_string()));
                }
            }
        }
    }
    op.args.iter().find_map(|a| match a {
        Arg::Named {
            name,
            required: true,
            ..
        } if !given.contains(name) => Some(ValueFault::MissingArgument(op.name, name)),
        _ => None,
    })
}

/// The named argument `name` of `op`: its kind, and its name as the
/// signature spells it.
fn named(op: &Op, name: &str) -> Option<(Literal, &'static str)> {
    op.args.iter().find_map(|a| match a {
        Arg::Named { name: n, kind, .. } if *n == name => Some((*kind, *n)),
        _ => None,
    })
}

/// **The operator a value applies**: `transport_only` in `retry
/// transport_only(max = 2)`. How a rule asks which operator it is, rather
/// than reading the value's spelling.
pub fn applied(head: &str, value: &str) -> Option<&'static Op> {
    let name = value.split_once('(').map_or(value, |(n, _)| n).trim();
    operator(head, name)
}

/// Every operator id the registry defines, for the stability test.
pub fn all_operator_ids() -> BTreeSet<&'static str> {
    let mut out = BTreeSet::new();
    for ops in [RETRY_OPS, CONFLICT_OPS, IDENTITY_OPS] {
        for o in ops {
            out.insert(o.id);
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    /// **Fails closed when the parser learns a new policy head.**
    ///
    /// The countermeasure, stated once in `tests/unknown_policy.rs` and owed
    /// again here: a head the parser accepts and this table does not classify
    /// is a value with no meaning, and it would look exactly like a value
    /// nobody wrote.
    #[test]
    fn every_policy_keyword_has_a_domain() {
        let missing: Vec<&str> = pw_syntax::POLICY_KEYWORDS
            .iter()
            .copied()
            .filter(|k| domain_of(k).is_none())
            .collect();
        assert!(
            missing.is_empty(),
            "the parser recognises these policy heads and this table does not \
             say what their values mean: {missing:?}"
        );
    }

    /// And the other direction, so the table cannot drift into describing
    /// policies the language does not have.
    ///
    /// The exceptions are the heads written inside a `handler_policy`,
    /// `replicated` or `paint` BLOCK. Those lower as body statements rather
    /// than as `Policy` entries — which is precisely what the `PolicyExpr`
    /// split has to fix — so the parser's list does not contain them yet.
    #[test]
    fn the_table_describes_only_policies_the_language_has() {
        const IN_BLOCK_ONLY: &[&str] = &[
            "identity",
            "captures",
            "on_version_mismatch",
            "load",
            "on_conflict_unresolved",
            "inputs",
            "isolated",
            "tenant",
            "respects",
            "on_mount",
            "on_unmount",
        ];
        let heads = [
            "cache",
            "partition",
            "consistency",
            "freshness",
            "timeout",
            "fallback",
            "stampede",
            "regenerate",
            "locale",
            "tenant",
            "policy_version",
            "code_version",
            "key",
            "dedupe_by",
            "concurrency",
            "on_key_change",
            "idempotent_by",
            "depends_on",
            "invalidates",
            "invalidates_on",
            "emits",
            "requires",
            "placement",
            "route",
            "privacy",
            "capability",
            "host",
            "intrinsic",
            "impact",
            "optimistic",
            "rollback",
            "retry",
            "reconnect",
            "conflict",
            "identity",
            "transport",
            "delivery",
            "storage",
            "offline",
            "sync",
            "on_conflict_unresolved",
            "scope",
            "on_scope_exit",
            "transaction",
            "captures",
            "on_version_mismatch",
            "load",
            "inputs",
            "isolated",
            "acquire",
            "release",
            "draw",
            "on_mount",
            "on_unmount",
            "affine",
            "intrinsic_height",
            "revision",
            "respects",
            "because",
            "attributes_forced_layout_to",
        ];
        let invented: Vec<&str> = heads
            .iter()
            .copied()
            .filter(|h| !pw_syntax::POLICY_KEYWORDS.contains(h) && !IN_BLOCK_ONLY.contains(h))
            .collect();
        assert!(
            invented.is_empty(),
            "this table classifies heads the parser does not recognise as \
             policies at all: {invented:?}"
        );
    }

    /// **Operator ids are stable and unique.**
    ///
    /// They are what a `PolicyApply` node will carry, and what an artifact will
    /// record. Two operators sharing one id would make a contract hash agree
    /// across a semantic difference.
    #[test]
    fn operator_ids_are_unique_and_namespaced() {
        let ids = all_operator_ids();
        let mut count = 0;
        for ops in [RETRY_OPS, CONFLICT_OPS, IDENTITY_OPS] {
            count += ops.len();
        }
        assert_eq!(ids.len(), count, "two operators share one id");
        for id in &ids {
            assert!(id.starts_with("policy."), "{id} is not namespaced");
        }
    }

    /// **An operator is resolved within its head's domain, never globally.**
    #[test]
    fn an_operator_belongs_to_one_domain() {
        assert!(operator("retry", "bounded_exponential").is_some());
        assert!(operator("reconnect", "bounded_exponential").is_some());
        assert!(
            operator("conflict", "bounded_exponential").is_none(),
            "a retry strategy is not a conflict strategy, and the only reason \
             to accept it here would be a global operator table"
        );
        assert!(operator("retry", "merge_by_field").is_none());
        assert!(
            operator("cache", "shared").is_none(),
            "`cache` takes a word, not an operator — asking for one must not \
             invent an operator from the word"
        );
    }

    /// **The term positions are exactly the ones that hold executable code.**
    ///
    /// The discriminator matters more than the list: `key id` and `cache
    /// shared` are the shapes the over-broad `PW0024` reported, twice, before
    /// being reverted.
    #[test]
    fn only_executable_positions_carry_terms() {
        for head in [
            "optimistic",
            "emits",
            "invalidates",
            "requires",
            "acquire",
            "release",
            "draw",
        ] {
            assert!(carries_terms(head), "{head} holds executable code");
        }
        assert!(
            !carries_terms("rollback"),
            "`rollback` is written by nobody since ADR-0025 — the platform \
             restores the value it held"
        );
        for head in [
            "cache",
            "key",
            "consistency",
            "freshness",
            "placement",
            "retry",
            "impact",
            "capability",
            "host",
            "concurrency",
            "transport",
            "storage",
        ] {
            assert!(
                !carries_terms(head),
                "{head} does not hold executable code, and reporting its value \
                 as an unresolved name is the rule that was reverted twice"
            );
        }
    }
}
