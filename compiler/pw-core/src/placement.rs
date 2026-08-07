//! E5 — where code may run, derived from what it does.
//!
//! Charter §1.7: *"Browser code cannot access database or secret capabilities."*
//! Placement is not an annotation the author chooses freely — it is a
//! constraint solved from the effects a body performs and the capabilities each
//! world can grant.
//!
//! The direction matters. A checker that asked "is this effect allowed here?"
//! would need a rule per effect per world. This asks "which worlds can satisfy
//! everything this body needs?", so an effect with no world at all is a
//! contradiction the solver reports rather than a case someone forgot to write.

use std::collections::BTreeSet;
use std::fmt;

use crate::privacy::{Label, Restriction};

/// Where code runs.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum World {
    /// Evaluated once at build time. No request, no session, no user.
    Build,
    /// The user's browser.
    Browser,
    /// A shared edge node. Close to the user, trusted with less than origin.
    Edge,
    /// The origin. Holds the database and the secrets.
    Origin,
}

pub const ALL_WORLDS: &[World] = &[World::Build, World::Browser, World::Edge, World::Origin];

/// **Where an effect is meaningful, as the program declares it.**
///
/// Architect ruling, 2026-08-07, on deleting `World::worlds_for`:
///
/// > Do not turn `ontology lookup failed` into `no placement restriction`.
/// > That would recreate the `secret<Payments>` hole in a new form. […]
/// > `enum PlacementLookup { Known, Unrestricted, Blocked }` is safer than
/// > returning `Option<Set<World>>`, where `None` can ambiguously mean either
/// > "anywhere" or "I don't know."
///
/// The two readings of `None` have opposite safety, and the deleted table
/// collapsed them: `secret<Payments>` missed its entry and came back "not
/// restricted", which is the answer an unmodelled effect legitimately gets.
/// One value cannot mean both.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PlacementLookup {
    /// A declaration names the worlds this effect is meaningful in.
    Known(Vec<World>),
    /// Declared, and declared to constrain nothing. `effect log<L>` happens
    /// everywhere; so does `session.read`, whose *authority* is a capability
    /// and whose availability is a property of a topology.
    Unrestricted,
    /// **Nothing declares this effect, so placement does not continue.**
    ///
    /// `code` names the diagnostic that owns the failure — the block is
    /// already reported, in the vocabulary of the thing that is actually
    /// wrong, and a consumer must not invent a second message for it.
    Blocked { code: &'static str },
}

/// **What a program declares about where its effects are meaningful.**
///
/// A trait rather than a concrete `&Ontology` so that `placement.rs` keeps
/// knowing nothing about how an effect name is taken apart. The compiler's
/// implementation is `ontology::Ontology`; the tests below supply their own,
/// which is what makes the solver testable without a parser.
pub trait Placements {
    fn placement_of(&self, effect: &str) -> PlacementLookup;
}

/// Whether one world can host one effect.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum Grant {
    Yes,
    No,
    /// The effect does not resolve, so the question has no answer. **Not a
    /// no**: a caller that reports "cannot run here" for a `Blocked` effect
    /// sends the reader to the placement they wrote instead of to the effect
    /// they misspelled.
    Blocked,
}

impl World {
    pub fn name(self) -> &'static str {
        match self {
            World::Build => "build",
            World::Browser => "browser",
            World::Edge => "edge",
            World::Origin => "origin",
        }
    }

    /// Can this world host `effect`, given what the program declares?
    ///
    /// **Three-valued, and that is the whole point.** A world that cannot
    /// answer is not a world that permits: see [`PlacementLookup`].
    ///
    /// There was a `worlds_for` table here — `"database" => [Origin]` — and it
    /// was deleted on 2026-08-07 by ruling. Two things it taught are worth
    /// keeping in view, because both were live defects:
    ///
    /// - The first version listed the families a world *could* grant, so an
    ///   unlisted family was granted by nobody and `log` made a declaration
    ///   unplaceable. Two corpus files were "caught" that way: right file,
    ///   wrong reason, and a coverage number that moved while nothing had been
    ///   detected. Hence [`PlacementLookup::Unrestricted`] as a real answer.
    /// - The table was consulted with a second `split('.')` that stripped no
    ///   type argument, so `secret<Payments>` — no dot at all — had the whole
    ///   string for a family, matched nothing, and **was placeable in every
    ///   world including the browser.** Twelve uses, invisible for a
    ///   milestone, because `forbidden_in` and `secret_to_browser` caught it
    ///   through rules that stripped correctly. Defence in depth hid a hole in
    ///   one of the layers. The effect name is now taken apart in exactly one
    ///   module, `ontology.rs`, and this asks it rather than parsing.
    pub fn grants(self, effect: &str, declared: &dyn Placements) -> Grant {
        match declared.placement_of(effect) {
            PlacementLookup::Unrestricted => Grant::Yes,
            PlacementLookup::Known(worlds) if worlds.contains(&self) => Grant::Yes,
            PlacementLookup::Known(_) => Grant::No,
            PlacementLookup::Blocked { .. } => Grant::Blocked,
        }
    }

    /// May a value carrying `label` be present in this world?
    pub fn may_hold(self, label: &Label) -> bool {
        label.restrictions().all(|r| match r {
            // Secrets never leave the origin.
            Restriction::Secret(_) => self == World::Origin,
            // Device-bound values exist only where the device is.
            Restriction::Device => self == World::Browser,
            // Session, user and organization values may exist anywhere with a
            // request context — which excludes build time, where there is no
            // user to be private to.
            _ => self != World::Build,
        })
    }
}

impl fmt::Display for World {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.name())
    }
}

/// Why a world was ruled out. The material for a cause chain (charter §14 M5
/// task 5), which is why it keeps the effect rather than only the verdict.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Ruling {
    pub world: World,
    pub reason: RuledOut,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RuledOut {
    /// The world cannot grant a capability the body needs.
    MissingCapability { effect: String },
    /// A value's label may not exist in that world.
    LabelNotPermitted { restriction: Restriction },
    /// The author pinned a different world.
    NotDeclared,
}

impl fmt::Display for RuledOut {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            RuledOut::MissingCapability { effect } => {
                write!(f, "cannot grant `{effect}`")
            }
            RuledOut::LabelNotPermitted { restriction } => {
                write!(f, "may not hold a {restriction} value")
            }
            RuledOut::NotDeclared => write!(f, "not the declared placement"),
        }
    }
}

/// What a body needs in order to run somewhere.
#[derive(Debug, Clone, Default)]
pub struct Demand {
    /// Effects it performs, as written in its row.
    pub effects: Vec<String>,
    /// The join of every label its values carry.
    pub label: Label,
    /// A placement the author pinned, if any.
    pub declared: Option<World>,
}

/// The worlds that can satisfy a demand, and why the others cannot.
#[derive(Debug, Clone)]
pub struct Solution {
    pub feasible: BTreeSet<World>,
    pub ruled_out: Vec<Ruling>,
    /// Effects nothing declares.
    ///
    /// **A blocked solution is not an unsatisfiable one**, and reading
    /// `feasible.is_empty()` without asking this reports "nowhere to run"
    /// about a program whose real defect is an effect that names nothing.
    /// `feasible` is empty either way, so the safe direction is preserved: a
    /// caller that forgets refuses rather than permits.
    pub blocked: Vec<String>,
}

impl Solution {
    pub fn is_satisfiable(&self) -> bool {
        !self.feasible.is_empty()
    }

    /// Did an effect fail to resolve, leaving placement with no basis?
    pub fn is_blocked(&self) -> bool {
        !self.blocked.is_empty()
    }

    /// Every reason a specific world was rejected — the cause chain.
    pub fn why_not(&self, world: World) -> Vec<&Ruling> {
        self.ruled_out.iter().filter(|r| r.world == world).collect()
    }
}

/// Solve a demand against every world, given what the program declares.
pub fn solve(demand: &Demand, declared: &dyn Placements) -> Solution {
    let mut feasible = BTreeSet::new();
    let mut ruled_out = Vec::new();
    let mut blocked = BTreeSet::new();

    for &world in ALL_WORLDS {
        let mut ok = true;

        if demand.declared.is_some_and(|d| d != world) {
            ruled_out.push(Ruling {
                world,
                reason: RuledOut::NotDeclared,
            });
            continue;
        }

        for effect in &demand.effects {
            match world.grants(effect, declared) {
                Grant::Yes => {}
                Grant::No => {
                    ruled_out.push(Ruling {
                        world,
                        reason: RuledOut::MissingCapability {
                            effect: effect.clone(),
                        },
                    });
                    ok = false;
                }
                // Not a `Ruling`: nothing about this world ruled it out. The
                // effect has no meaning in this program, and saying "the
                // browser cannot grant `databse.read`" would confirm the typo
                // as a capability while blaming the placement.
                Grant::Blocked => {
                    blocked.insert(effect.clone());
                    ok = false;
                }
            }
        }
        for r in demand.label.restrictions() {
            if !world.may_hold(&Label::of(r.clone())) {
                ruled_out.push(Ruling {
                    world,
                    reason: RuledOut::LabelNotPermitted {
                        restriction: r.clone(),
                    },
                });
                ok = false;
            }
        }

        if ok {
            feasible.insert(world);
        }
    }

    Solution {
        feasible,
        ruled_out,
        blocked: blocked.into_iter().collect(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The vocabulary these tests solve against, standing in for a platform
    /// package's declarations.
    ///
    /// A slice of pairs rather than a map, so the test double cannot become a
    /// second effect table with a lookup order of its own. Everything absent
    /// is `Blocked`, which is what the compiler now does: the real
    /// `Unrestricted` cases are listed with an empty world set, exactly as a
    /// declaration with no `placement` clause produces.
    struct Vocabulary(&'static [(&'static str, &'static [World])]);

    const PLATFORM: Vocabulary = Vocabulary(&[
        ("database.read", &[World::Origin]),
        ("database.write", &[World::Origin]),
        ("secret", &[World::Origin]),
        ("secret.use", &[World::Origin]),
        ("dom.mutate", &[World::Browser]),
        ("device.geolocation", &[World::Browser]),
        (
            "network.fetch",
            &[World::Browser, World::Edge, World::Origin],
        ),
        ("cache.write", &[World::Edge, World::Origin]),
        // Declared, and declared to constrain nothing.
        ("log", &[]),
        ("resource.acquire", &[]),
    ]);

    impl Placements for Vocabulary {
        fn placement_of(&self, effect: &str) -> PlacementLookup {
            let name = effect.split('<').next().unwrap_or(effect);
            match self.0.iter().find(|(n, _)| *n == name) {
                Some((_, [])) => PlacementLookup::Unrestricted,
                Some((_, worlds)) => PlacementLookup::Known(worlds.to_vec()),
                None => PlacementLookup::Blocked { code: "PW5201" },
            }
        }
    }

    fn demand(effects: &[&str]) -> Demand {
        Demand {
            effects: effects.iter().map(|s| s.to_string()).collect(),
            ..Default::default()
        }
    }

    fn solve(demand: &Demand) -> Solution {
        super::solve(demand, &PLATFORM)
    }

    #[test]
    fn a_database_effect_cannot_run_in_the_browser() {
        // Charter §1.7, the headline placement rule.
        let s = solve(&demand(&["database.read"]));
        assert!(!s.feasible.contains(&World::Browser));
        assert!(s.feasible.contains(&World::Origin));
        assert_eq!(
            s.why_not(World::Browser)
                .iter()
                .map(|r| r.reason.to_string())
                .collect::<Vec<_>>(),
            ["cannot grant `database.read`"]
        );
    }

    #[test]
    fn a_dom_effect_cannot_run_on_the_origin() {
        // The converse, which a checker written one-effect-at-a-time tends to
        // forget: a browser-only device API on the server (§14 M5 task 4).
        let s = solve(&demand(&["dom.mutate"]));
        assert_eq!(s.feasible, BTreeSet::from([World::Browser]));
    }

    #[test]
    fn a_body_needing_both_the_dom_and_the_database_has_nowhere_to_run() {
        // The contradiction the solver exists to find. Asking "is this effect
        // allowed here?" one at a time never reports it.
        let s = solve(&demand(&["dom.mutate", "database.read"]));
        assert!(!s.is_satisfiable(), "{:?}", s.feasible);
    }

    #[test]
    fn a_secret_never_leaves_the_origin() {
        let d = Demand {
            label: Label::secret("payments"),
            ..Default::default()
        };
        let s = solve(&d);
        assert_eq!(s.feasible, BTreeSet::from([World::Origin]));
        assert!(
            s.why_not(World::Edge)
                .iter()
                .any(|r| r.reason.to_string().contains("Secret<payments>")),
            "the ruling must name the restriction, not just say no"
        );
    }

    #[test]
    fn a_device_value_exists_only_in_the_browser() {
        let d = Demand {
            label: Label::device(),
            ..Default::default()
        };
        assert_eq!(solve(&d).feasible, BTreeSet::from([World::Browser]));
    }

    #[test]
    fn build_time_has_no_user_to_be_private_to() {
        // A session value at build time is nondeterministic static rendering:
        // there is no request, so there is no session.
        let d = Demand {
            label: Label::session("abc"),
            ..Default::default()
        };
        let s = solve(&d);
        assert!(!s.feasible.contains(&World::Build));
        assert!(s.feasible.contains(&World::Origin));
    }

    #[test]
    fn a_pure_body_can_run_anywhere() {
        // The control. If everything were ruled out, every test above would
        // pass for the wrong reason.
        let s = solve(&Demand::default());
        assert_eq!(s.feasible.len(), ALL_WORLDS.len(), "{:?}", s.feasible);
    }

    #[test]
    fn a_declared_placement_narrows_but_cannot_widen() {
        // Pinning `browser` on a database read must not make it legal.
        let d = Demand {
            effects: vec!["database.read".to_string()],
            declared: Some(World::Browser),
            ..Default::default()
        };
        let s = solve(&d);
        assert!(!s.is_satisfiable());

        // ...and pinning a legal world narrows to exactly it.
        let d = Demand {
            effects: vec!["network.fetch".to_string()],
            declared: Some(World::Edge),
            ..Default::default()
        };
        assert_eq!(solve(&d).feasible, BTreeSet::from([World::Edge]));
    }

    #[test]
    fn every_world_grants_something_and_nothing_grants_everything() {
        // A vocabulary where one world granted everything would make the whole
        // check vacuous, and one that granted nothing would make it useless.
        let effects = [
            "database.read",
            "network.fetch",
            "dom.mutate",
            "secret.use",
            "cache.write",
            "device.geolocation",
        ];
        for &w in ALL_WORLDS {
            let n = effects
                .iter()
                .filter(|f| w.grants(f, &PLATFORM) == Grant::Yes)
                .count();
            assert!(n < effects.len(), "{w} grants everything");
        }
        assert!(
            effects
                .iter()
                .any(|f| World::Origin.grants(f, &PLATFORM) == Grant::Yes),
            "origin must grant something"
        );
    }

    #[test]
    fn a_declared_effect_with_no_placement_rules_out_nothing() {
        // The dangerous direction, and the reason `Unrestricted` is a value
        // rather than an absence. The first version of the deleted table
        // listed the families a world *could* grant, so an unlisted one was
        // granted by nobody and `log` made a declaration unplaceable. Two
        // corpus files were reported for the wrong reason, which raises a
        // coverage number while detecting nothing.
        for effect in ["log<Public>", "resource.acquire<Db>"] {
            let s = solve(&demand(&[effect]));
            assert_eq!(
                s.feasible.len(),
                ALL_WORLDS.len(),
                "`{effect}` declares no placement and must not rule out any world"
            );
            assert!(!s.is_blocked(), "`{effect}` IS declared");
        }
    }

    #[test]
    fn an_undeclared_effect_is_blocked_and_not_unrestricted() {
        // **The hole this migration exists to close.** `worlds_for` answered
        // `None` for an effect it did not model, and `None` meant "grants it".
        // So an effect the program never declared — a typo, a family from
        // another platform — was placeable everywhere, which is the answer a
        // legitimately-unconstrained effect gets.
        //
        // Architect ruling, 2026-08-07:
        //
        // > Do not turn `ontology lookup failed` into `no placement
        // > restriction`. That would recreate the `secret<Payments>` hole in a
        // > new form.
        let s = solve(&demand(&["databse.read"]));
        assert!(s.is_blocked(), "an undeclared effect has no placement");
        assert_eq!(s.blocked, ["databse.read"]);
        assert!(
            s.feasible.is_empty(),
            "and blocked must not read as feasible-everywhere"
        );
        // ...and it is not reported as a world's failing, because no world
        // failed. There is nothing to place.
        assert!(s.ruled_out.is_empty(), "{:?}", s.ruled_out);

        // The contrast that makes the assertion mean something: the correctly
        // spelled effect is Origin-only, not blocked.
        let ok = solve(&demand(&["database.read"]));
        assert!(!ok.is_blocked());
        assert_eq!(ok.feasible, BTreeSet::from([World::Origin]));
    }

    #[test]
    fn a_type_argument_is_not_part_of_the_lookup() {
        // `secret<Payments>` has no dot, so the deleted table's `split('.')`
        // gave it the whole string for a family, matched nothing, and made the
        // corpus's most-used secret effect placeable in the browser. Twelve
        // uses, invisible for a milestone.
        assert_eq!(
            solve(&demand(&["secret<Payments>"])).feasible,
            BTreeSet::from([World::Origin])
        );
        assert!(!solve(&demand(&["secret<Payments>"])).is_blocked());
    }
}
