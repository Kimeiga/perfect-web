//! E5 — the privacy-label algebra.
//!
//! Charter §7.8 lists the labels and then says the thing that decides the whole
//! design:
//!
//! > Do not force all labels into a simplistic total order. Some are
//! > incomparable.
//!
//! `Session<a>` and `Session<b>` are not ordered — neither may flow into the
//! other, and that is the cross-tenant leak. A total order cannot express it,
//! and a checker built on one would rank one tenant "more private" than another
//! and let the other through.
//!
//! # The model
//!
//! A label is a **set of restrictions**. `Public` is the empty set. Joining two
//! labels is set union, and one label flows into another only when the target
//! carries at least the source's restrictions.
//!
//! ```text
//! Public                     = {}
//! Session<abc>               = { Session(abc) }
//! Session<abc> ⊔ User<u1>    = { Session(abc), User(u1) }
//! Session<abc> ⊔ Session<x>  = { Session(abc), Session(x) }   -- not either one
//! ```
//!
//! That last line is the case a total order gets wrong. The join of two
//! incomparable labels is *more restrictive than both*, which is exactly the
//! conservative answer, and it falls out of the model rather than being a rule
//! bolted on.

use std::collections::BTreeSet;
use std::fmt;

/// One restriction on where a value may go.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum Restriction {
    /// Belongs to one session.
    Session(String),
    /// Belongs to one user.
    User(String),
    /// Belongs to one organization — the cross-tenant boundary.
    Organization(String),
    /// Belongs to one device and cannot be moved off it.
    Device,
    /// A capability's secret material. Never leaves the world that holds it.
    Secret(String),
}

impl fmt::Display for Restriction {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Restriction::Session(s) => write!(f, "Session<{s}>"),
            Restriction::User(u) => write!(f, "User<{u}>"),
            Restriction::Organization(o) => write!(f, "Organization<{o}>"),
            Restriction::Device => write!(f, "Device"),
            Restriction::Secret(c) => write!(f, "Secret<{c}>"),
        }
    }
}

/// A privacy label: the set of restrictions a value carries.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct Label(BTreeSet<Restriction>);

impl Label {
    /// No restrictions. The identity of [`Label::join`].
    pub fn public() -> Self {
        Self::default()
    }

    pub fn of(r: Restriction) -> Self {
        Label(BTreeSet::from([r]))
    }

    pub fn session(id: &str) -> Self {
        Self::of(Restriction::Session(id.to_string()))
    }
    pub fn user(id: &str) -> Self {
        Self::of(Restriction::User(id.to_string()))
    }
    pub fn organization(id: &str) -> Self {
        Self::of(Restriction::Organization(id.to_string()))
    }
    pub fn device() -> Self {
        Self::of(Restriction::Device)
    }
    pub fn secret(capability: &str) -> Self {
        Self::of(Restriction::Secret(capability.to_string()))
    }

    pub fn is_public(&self) -> bool {
        self.0.is_empty()
    }

    pub fn restrictions(&self) -> impl Iterator<Item = &Restriction> {
        self.0.iter()
    }

    /// The least upper bound: everything both labels restrict.
    ///
    /// A value derived from two sources carries both sets of restrictions.
    /// Nothing is dropped, which is what makes the algebra conservative.
    pub fn join(&self, other: &Label) -> Label {
        Label(self.0.union(&other.0).cloned().collect())
    }

    /// May a value labelled `self` flow into a place labelled `into`?
    ///
    /// Only if the destination already carries every restriction the value has.
    /// `Public` flows everywhere; nothing non-public flows into `Public`.
    pub fn flows_into(&self, into: &Label) -> bool {
        self.0.is_subset(&into.0)
    }

    /// Restrictions the destination does not carry — the reason a flow is
    /// rejected, and the material for a diagnostic that names the value rather
    /// than reporting a type mismatch (charter §14 M5 gate).
    pub fn violations(&self, into: &Label) -> Vec<Restriction> {
        self.0.difference(&into.0).cloned().collect()
    }

    /// Two labels neither of which flows into the other. The cross-tenant case.
    pub fn incomparable_with(&self, other: &Label) -> bool {
        !self.flows_into(other) && !other.flows_into(self)
    }

    /// May this value be stored in a cache shared between users?
    ///
    /// Charter §7.8: a session or user value in a shared public cache is the
    /// canonical failure. `Device` is included because a shared cache is by
    /// definition not on one device.
    pub fn safe_in_shared_cache(&self) -> bool {
        self.is_public()
    }

    /// The partitions a cache key must include for this value to be cacheable
    /// at all. A shared cache keyed without them serves one tenant's data to
    /// another (charter §14 M5 task 4, "cross-tenant cache key omission").
    pub fn required_cache_partitions(&self) -> Vec<String> {
        self.0
            .iter()
            .filter_map(|r| match r {
                Restriction::Session(_) => Some("session".to_string()),
                Restriction::User(_) => Some("user".to_string()),
                Restriction::Organization(_) => Some("organization".to_string()),
                Restriction::Device => Some("device".to_string()),
                // A secret is never cached, so no partition makes it safe.
                Restriction::Secret(_) => None,
            })
            .collect()
    }

    /// Secrets must never be logged, serialized to a client, or cached.
    pub fn holds_a_secret(&self) -> bool {
        self.0.iter().any(|r| matches!(r, Restriction::Secret(_)))
    }
}

impl fmt::Display for Label {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        if self.0.is_empty() {
            return write!(f, "Public");
        }
        let parts: Vec<String> = self.0.iter().map(|r| r.to_string()).collect();
        write!(f, "{}", parts.join(" ⊔ "))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A small universe that still contains every shape that matters: public,
    /// two of the same kind with different ids (incomparable), two of different
    /// kinds, a device, and a secret.
    fn universe() -> Vec<Label> {
        let atoms = [
            Label::public(),
            Label::session("abc"),
            Label::session("xyz"),
            Label::user("u1"),
            Label::organization("acme"),
            Label::device(),
            Label::secret("payments"),
        ];
        // Plus every pairwise join, so the tests see composite labels too.
        let mut all: Vec<Label> = atoms.to_vec();
        for a in &atoms {
            for b in &atoms {
                all.push(a.join(b));
            }
        }
        all.sort_by_key(|l| l.to_string());
        all.dedup();
        all
    }

    // --- lattice laws, checked exhaustively over the universe ---------------
    //
    // Charter §14 M5 task 8 asks for property-based tests. Exhaustive
    // verification over a bounded universe is stronger than sampling: it cannot
    // miss the one pair that matters, and it needs no dependency.

    #[test]
    fn join_is_commutative_associative_and_idempotent() {
        let u = universe();
        for a in &u {
            assert_eq!(a.join(a), *a, "idempotent: {a}");
            for b in &u {
                assert_eq!(a.join(b), b.join(a), "commutative: {a}, {b}");
                for c in &u {
                    assert_eq!(
                        a.join(b).join(c),
                        a.join(&b.join(c)),
                        "associative: {a}, {b}, {c}"
                    );
                }
            }
        }
    }

    #[test]
    fn public_is_the_identity_and_the_bottom() {
        let u = universe();
        let public = Label::public();
        for a in &u {
            assert_eq!(a.join(&public), *a, "identity: {a}");
            assert!(public.flows_into(a), "public flows everywhere: {a}");
            if !a.is_public() {
                assert!(
                    !a.flows_into(&public),
                    "nothing restricted flows into public: {a}"
                );
            }
        }
    }

    #[test]
    fn a_join_is_an_upper_bound_and_the_least_one() {
        let u = universe();
        for a in &u {
            for b in &u {
                let j = a.join(b);
                assert!(
                    a.flows_into(&j) && b.flows_into(&j),
                    "upper bound: {a}, {b}"
                );
                // Least: anything both flow into must also accept the join.
                for c in &u {
                    if a.flows_into(c) && b.flows_into(c) {
                        assert!(j.flows_into(c), "least upper bound: {a}, {b} vs {c}");
                    }
                }
            }
        }
    }

    #[test]
    fn flows_into_is_a_partial_order_and_not_a_total_one() {
        let u = universe();
        for a in &u {
            assert!(a.flows_into(a), "reflexive: {a}");
            for b in &u {
                if a.flows_into(b) && b.flows_into(a) {
                    assert_eq!(a, b, "antisymmetric: {a}, {b}");
                }
                for c in &u {
                    if a.flows_into(b) && b.flows_into(c) {
                        assert!(a.flows_into(c), "transitive: {a}, {b}, {c}");
                    }
                }
            }
        }

        // The charter's actual requirement, asserted rather than assumed.
        assert!(
            Label::session("abc").incomparable_with(&Label::session("xyz")),
            "two sessions must be incomparable — a total order would rank one \
             above the other and let a cross-tenant flow through"
        );
        assert!(Label::user("u1").incomparable_with(&Label::organization("acme")));
    }

    #[test]
    fn joining_two_tenants_is_stricter_than_either() {
        // The case a total order gets wrong. The join must not collapse to one
        // of the two, or a value derived from both would be treated as
        // belonging to one of them.
        let a = Label::session("abc");
        let b = Label::session("xyz");
        let j = a.join(&b);
        assert_ne!(j, a);
        assert_ne!(j, b);
        assert!(!j.flows_into(&a) && !j.flows_into(&b));
        assert_eq!(j.to_string(), "Session<abc> ⊔ Session<xyz>");
    }

    // --- the rules the algebra exists to serve ------------------------------

    #[test]
    fn only_public_values_are_safe_in_a_shared_cache() {
        assert!(Label::public().safe_in_shared_cache());
        for l in [
            Label::session("abc"),
            Label::user("u1"),
            Label::organization("acme"),
            Label::device(),
            Label::secret("payments"),
        ] {
            assert!(
                !l.safe_in_shared_cache(),
                "{l} must not be shared-cacheable"
            );
        }
    }

    #[test]
    fn violations_name_what_is_missing_not_merely_that_something_is() {
        // Charter §14 M5 gate: "Diagnostics name the source value and invalid
        // boundary, not merely a type mismatch."
        let value = Label::session("abc").join(&Label::user("u1"));
        let destination = Label::user("u1");
        let missing = value.violations(&destination);
        assert_eq!(missing, [Restriction::Session("abc".into())]);
        assert_eq!(missing[0].to_string(), "Session<abc>");

        // Control: a legal flow reports nothing.
        assert!(value.violations(&value).is_empty());
    }

    #[test]
    fn a_cache_key_must_carry_every_partition_the_value_needs() {
        let l = Label::session("abc").join(&Label::organization("acme"));
        let mut needed = l.required_cache_partitions();
        needed.sort();
        assert_eq!(needed, ["organization", "session"]);

        // A secret contributes no partition, because no key makes it cacheable.
        assert!(
            Label::secret("payments")
                .required_cache_partitions()
                .is_empty()
        );
        assert!(Label::secret("payments").holds_a_secret());
        assert!(Label::public().required_cache_partitions().is_empty());
    }
}
