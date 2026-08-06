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

impl World {
    pub fn name(self) -> &'static str {
        match self {
            World::Build => "build",
            World::Browser => "browser",
            World::Edge => "edge",
            World::Origin => "origin",
        }
    }

    /// Can this world grant `capability`?
    ///
    /// **Only restricted families are listed.** Anything not in the table is
    /// available everywhere, because a checker must not reject what it has not
    /// been taught.
    ///
    /// The first version had it the other way round: an unlisted family was
    /// granted by nobody, so a declaration using `log` or `resource` — neither
    /// of which the table mentioned — was reported as having nowhere to run.
    /// Two corpus files were "caught" that way. Right file, wrong reason, and
    /// the coverage number went up while nothing had actually been detected.
    pub fn grants(self, capability: &str) -> bool {
        let family = capability.split('.').next().unwrap_or(capability);
        match Self::worlds_for(family) {
            Some(worlds) => worlds.contains(&self),
            None => true,
        }
    }

    /// The worlds that can grant a restricted capability family, or `None` when
    /// the family is unrestricted.
    ///
    /// **This is a capability→world table, not a function-name table.** E2C's
    /// deletion gate is about the latter: no checker may know that
    /// `secrets.payments` yields a secret. Which *worlds* can grant the
    /// `secret` capability is a property of the deployment topology — charter
    /// §1.7 states it directly — and belongs in the compiler until E8's WIT
    /// worlds make it a declaration too.
    pub fn worlds_for(family: &str) -> Option<&'static [World]> {
        use World::*;
        Some(match family {
            // Privileged, server-side only.
            "database" | "secret" | "durable" => &[Origin],
            // The user's machine, and only there.
            "dom" | "style" | "layout" | "observe" | "animation" | "paint" | "device" => &[Browser],
            // Anywhere with a request context; build time has none.
            "network" => &[Browser, Edge, Origin],
            "cache" => &[Edge, Origin],
            _ => return None,
        })
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
}

impl Solution {
    pub fn is_satisfiable(&self) -> bool {
        !self.feasible.is_empty()
    }

    /// Every reason a specific world was rejected — the cause chain.
    pub fn why_not(&self, world: World) -> Vec<&Ruling> {
        self.ruled_out.iter().filter(|r| r.world == world).collect()
    }
}

/// Solve a demand against every world.
pub fn solve(demand: &Demand) -> Solution {
    let mut feasible = BTreeSet::new();
    let mut ruled_out = Vec::new();

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
            if !world.grants(effect) {
                ruled_out.push(Ruling {
                    world,
                    reason: RuledOut::MissingCapability {
                        effect: effect.clone(),
                    },
                });
                ok = false;
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
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn demand(effects: &[&str]) -> Demand {
        Demand {
            effects: effects.iter().map(|s| s.to_string()).collect(),
            ..Default::default()
        }
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
        // A table where one world granted everything would make the whole
        // check vacuous, and one that granted nothing would make it useless.
        let families = [
            "database.read",
            "network.fetch",
            "dom.mutate",
            "secret.use",
            "cache.write",
            "device.geolocation",
        ];
        for &w in ALL_WORLDS {
            let n = families.iter().filter(|f| w.grants(f)).count();
            assert!(n < families.len(), "{w} grants everything");
        }
        assert!(
            families.iter().any(|f| World::Origin.grants(f)),
            "origin must grant something"
        );
    }

    #[test]
    fn an_unknown_capability_family_rules_out_nothing() {
        // The dangerous direction. The first version granted an unlisted family
        // nowhere, so `log` and `resource` — neither in the table — made a
        // declaration unplaceable. Two corpus files were reported for the wrong
        // reason, which raises a coverage number while detecting nothing.
        for family in ["log", "resource", "something.nobody.modelled"] {
            let s = solve(&demand(&[family]));
            assert_eq!(
                s.feasible.len(),
                ALL_WORLDS.len(),
                "`{family}` is not modelled and must not rule out any world"
            );
        }
        assert!(World::worlds_for("log").is_none());
        assert!(
            World::worlds_for("database").is_some(),
            "but a known one is restricted"
        );
    }
}
