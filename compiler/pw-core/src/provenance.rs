//! Which resolved facts caused a conclusion.
//!
//! ADR-0022. `docs/RISK_QUEUE.md` names the failure this exists for:
//!
//! > **Coincidental correctness:** the compiler emits the right verdict, but
//! > the semantic information that is supposed to justify that verdict never
//! > actually flowed through the program.
//!
//! `R-037` is the case that produced it. The fixture claimed `layout.measure`
//! propagates through `List.map`'s callback; its callback parameter was never
//! bound, so nothing was propagating through anything, and the effect was found
//! by matching the spelling `getBoundingClientRect` against every declaration
//! in the program. **The diagnostic was correct.** The fixture would have
//! passed identically had it written `fn(anything_at_all)`.
//!
//! A test cannot see that by reading the verdict. It can see it by asking what
//! the verdict was built from:
//!
//! ```text
//! must contain                        must NOT contain
//!   a receiver-type member resolution   a spelling-only match
//!   propagation through a callback      an unresolved receiver
//! ```
//!
//! # RECORD, never RECOMPUTE
//!
//! ADR-0022's falsification condition, and the one rule this module must not
//! break:
//!
//! > If the DAG becomes a parallel implementation of the analysis — a second
//! > answer to the same question — it is the pattern this project exists to
//! > delete, and the ADR is wrong. The distinguishing question: does the
//! > provenance RECORD what the analysis did, or RECOMPUTE it?
//!
//! So there is no function here that derives a fact. Every constructor takes a
//! value the analysis has *already produced* and stores it. `Evidence` cannot
//! answer a question the analysis did not answer first — if a caller wanted it
//! to, that would be the second implementation arriving.
//!
//! # Why a DAG rather than a list
//!
//! `layout.measure` reached a view *because* a member resolved *because* a
//! parameter had a type. A flat list of facts records that all three happened;
//! it cannot distinguish that chain from three unrelated coincidences, which is
//! precisely the distinction R-037 turns on.

use std::collections::BTreeSet;

use crate::hir::Span;
use crate::resolve::DefId;

/// A fact's position in one [`Evidence`] graph.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct FactId(pub u32);

/// **How** a name was turned into a declaration.
///
/// The field R-037 turns on. Two resolutions can reach the same declaration and
/// mean entirely different things about whether the program says so.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum Route {
    /// A qualified path against a visible module: `Stores.get`.
    Qualified,
    /// The receiver's TYPE declared the member: `el: ElementRef`, and
    /// `ElementRef` declares `getBoundingClientRect`.
    ///
    /// What R-037 claims and did not have.
    ReceiverType,
    /// The last segment, matched within this unit's own module and its
    /// imports, with two matches meaning no answer.
    ScopedName,
    /// The last segment, matched against the whole program.
    ///
    /// **Deleted in `3a0f319`, and kept here as a nameable thing so a test can
    /// forbid it.** A route that cannot be named cannot be forbidden, and this
    /// is the one every instance of coincidental correctness so far has
    /// travelled by.
    ProgramWideName,
}

/// One thing an analysis concluded, and what it concluded it from.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum FactKind {
    /// A call resolved to a declaration.
    ResolvedCall { path: String, to: DefId, via: Route },
    /// A member resolved on a receiver whose type is known.
    ResolvedMember {
        receiver_type: String,
        member: String,
        via: Route,
    },
    /// A written effect resolved to a declaration.
    ///
    /// The ontology's answer, and the one that makes "no checker splits
    /// `database.read` at the dot" checkable: a fact recording that the WHOLE
    /// path named a declaration is different evidence from a fact recording
    /// that a final segment matched something.
    ResolvedEffect {
        path: String,
        to: crate::ontology::EffectDefId,
        via: Route,
    },
    /// An effect's type argument was looked up.
    ///
    /// Recorded whether or not it resolved. That an argument named nothing is
    /// a fact about the program — `docs/RISK_QUEUE.md` 37, where
    /// `LayoutAffect` named nothing in five files — and a graph holding only
    /// successes cannot tell "checked and found" from "never looked".
    ResolvedTypeArgument { written: String, resolved: bool },
    /// An effect entered a body.
    Effect { effect: String },
    /// An effect crossed into a lambda handed to another function.
    ThroughCallback { passed_to: String },
    /// An effect came from a declaration's own inferred row.
    ThroughHelper { helper: String },
}

/// A recorded conclusion and its causes.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Fact {
    pub id: FactId,
    pub kind: FactKind,
    /// Where in the source the analysis was looking.
    pub origin: Span,
    /// The facts this one was concluded from, in the order they were recorded.
    pub depends_on: Vec<FactId>,
}

/// The facts one analysis recorded, as a DAG.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Evidence {
    facts: Vec<Fact>,
}

impl Evidence {
    /// Store a fact the analysis has already concluded.
    ///
    /// Deliberately the only way to add one, and deliberately takes a finished
    /// `FactKind`: there is no `Evidence::resolve(..)` and there must never be.
    pub fn record(&mut self, kind: FactKind, origin: Span, depends_on: Vec<FactId>) -> FactId {
        let id = FactId(self.facts.len() as u32);
        self.facts.push(Fact {
            id,
            kind,
            origin,
            depends_on,
        });
        id
    }

    pub fn facts(&self) -> &[Fact] {
        &self.facts
    }

    pub fn is_empty(&self) -> bool {
        self.facts.is_empty()
    }

    /// Fold another graph in, renumbering it.
    ///
    /// Inference runs per body and a caller collects several, so the graphs
    /// have to compose. Renumbering rather than merging by identity, because
    /// two bodies can conclude the same thing for different reasons and
    /// collapsing them would lose exactly the distinction this records.
    pub fn absorb(&mut self, other: &Evidence) {
        let base = self.facts.len() as u32;
        for f in &other.facts {
            self.facts.push(Fact {
                id: FactId(f.id.0 + base),
                kind: f.kind.clone(),
                origin: f.origin.clone(),
                depends_on: f.depends_on.iter().map(|d| FactId(d.0 + base)).collect(),
            });
        }
    }

    /// Every route any resolution in this graph travelled by.
    ///
    /// The query a `must not contain` assertion is written against.
    pub fn routes(&self) -> BTreeSet<Route> {
        self.facts
            .iter()
            .filter_map(|f| match &f.kind {
                FactKind::ResolvedCall { via, .. }
                | FactKind::ResolvedMember { via, .. }
                | FactKind::ResolvedEffect { via, .. } => Some(*via),
                _ => None,
            })
            .collect()
    }

    /// Did this written effect resolve to a declaration, and to which?
    pub fn effect_resolved(&self, path: &str) -> Option<crate::ontology::EffectDefId> {
        self.facts.iter().find_map(|f| match &f.kind {
            FactKind::ResolvedEffect { path: p, to, .. } if p == path => Some(*to),
            _ => None,
        })
    }

    /// Did a member resolve on this receiver type?
    pub fn resolved_member_on(&self, receiver_type: &str, member: &str) -> bool {
        self.facts.iter().any(|f| {
            matches!(
                &f.kind,
                FactKind::ResolvedMember { receiver_type: r, member: m, .. }
                    if r == receiver_type && m == member
            )
        })
    }

    /// Did this effect reach the body, and through what?
    pub fn effect_reached(&self, effect: &str) -> Option<&Fact> {
        self.facts
            .iter()
            .find(|f| matches!(&f.kind, FactKind::Effect { effect: e } if e == effect))
    }

    /// The facts a fact was concluded from, transitively.
    ///
    /// A test asserts on the CHAIN rather than on the whole graph — ADR-0022:
    /// *"Assert required semantic edges and forbidden evidence sources"* —
    /// because asserting a whole proof is rewritten by every refactor.
    pub fn causes(&self, of: FactId) -> Vec<&Fact> {
        let mut seen: BTreeSet<u32> = BTreeSet::new();
        let mut stack = vec![of];
        let mut out = Vec::new();
        while let Some(id) = stack.pop() {
            let Some(f) = self.facts.get(id.0 as usize) else {
                continue;
            };
            if !seen.insert(id.0) {
                continue;
            }
            out.push(f);
            stack.extend(f.depends_on.iter().copied());
        }
        out
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn span() -> Span {
        0..1
    }

    #[test]
    fn a_chain_is_walkable_and_a_coincidence_is_not() {
        // The distinction the whole module exists for. Both graphs contain the
        // same three facts; only one says they caused each other.
        let mut chained = Evidence::default();
        let ty = chained.record(
            FactKind::ResolvedMember {
                receiver_type: "ElementRef".into(),
                member: "getBoundingClientRect".into(),
                via: Route::ReceiverType,
            },
            span(),
            vec![],
        );
        let cb = chained.record(
            FactKind::ThroughCallback {
                passed_to: "List.map".into(),
            },
            span(),
            vec![ty],
        );
        let eff = chained.record(
            FactKind::Effect {
                effect: "layout.measure".into(),
            },
            span(),
            vec![cb],
        );

        let mut coincidence = Evidence::default();
        coincidence.record(
            FactKind::ResolvedMember {
                receiver_type: "ElementRef".into(),
                member: "getBoundingClientRect".into(),
                via: Route::ReceiverType,
            },
            span(),
            vec![],
        );
        coincidence.record(
            FactKind::ThroughCallback {
                passed_to: "List.map".into(),
            },
            span(),
            vec![],
        );
        let loose = coincidence.record(
            FactKind::Effect {
                effect: "layout.measure".into(),
            },
            span(),
            vec![],
        );

        assert_eq!(chained.causes(eff).len(), 3, "the chain reaches all three");
        assert_eq!(
            coincidence.causes(loose).len(),
            1,
            "three unrelated facts are not a chain"
        );
    }

    #[test]
    fn a_forbidden_route_is_nameable() {
        // `ProgramWideName` is deleted from the compiler and kept here so a
        // test can say "not this". A route with no name cannot be forbidden.
        let mut e = Evidence::default();
        e.record(
            FactKind::ResolvedCall {
                path: "getBoundingClientRect".into(),
                to: DefId { unit: 0, decl: 1 },
                via: Route::ProgramWideName,
            },
            span(),
            vec![],
        );
        assert!(e.routes().contains(&Route::ProgramWideName));
        assert!(!e.routes().contains(&Route::ReceiverType));
    }

    #[test]
    fn absorbing_keeps_two_bodies_answers_apart() {
        // Two bodies concluding the same thing for different reasons must stay
        // two facts. Merging by identity would lose the difference, which is
        // the difference this module records.
        let mut a = Evidence::default();
        let one = a.record(
            FactKind::Effect {
                effect: "layout.measure".into(),
            },
            span(),
            vec![],
        );
        a.record(
            FactKind::ThroughCallback {
                passed_to: "List.map".into(),
            },
            span(),
            vec![one],
        );

        let b = a.clone();
        let mut merged = a.clone();
        merged.absorb(&b);

        assert_eq!(merged.facts().len(), 4);
        // The second graph's dependency still points inside the second graph.
        let last = merged.facts().last().expect("a fact");
        assert_eq!(last.depends_on, vec![FactId(2)]);
    }
}
