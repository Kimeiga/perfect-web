//! **The placement baseline, across the deletion of `worlds_for`.**
//!
//! Architect ruling, 2026-08-07:
//!
//! > Use `the_ontology_and_worlds_for_agree_about_placement_where_both_speak`
//! > as the migration oracle: freeze current placement results, make
//! > planner/placement consume ontology + topology only, delete `worlds_for`,
//! > require all intended rows to remain unchanged, and classify any changed
//! > row explicitly.
//!
//! Same discipline as `contract_matrix.rs`, which caught a live security hole
//! on its first run precisely because it predated the change it was built for.
//!
//! # What was migrated away from
//!
//! `World::worlds_for(family)` was a hard-coded capability→world table in the
//! compiler, deleted on 2026-08-07. It is reproduced below as [`THE_TABLE`],
//! because a baseline that disappears with the thing it measures is not a
//! baseline. The ontology declares the same fact per effect:
//!
//! ```pleris
//! effect layout.measure   { placement browser  capability none }
//! effect database.read<T> { placement origin   capability database.read<T> }
//! ```
//!
//! Two sources for one answer, agreeing at the moment of deletion. These rows
//! are what says the deletion changed nothing it should not have.
//!
//! # And the hole it left
//!
//! The table returned `None` for an effect it did not model, and `None` was
//! read as *grants it*. So an effect nothing declares — a typo, a family from
//! another platform — was placeable in every world, which is the answer a
//! legitimately unconstrained effect gets. That is the shape the three-valued
//! [`pw_core::placement::PlacementLookup`] exists to make unrepresentable, and
//! `an_undeclared_effect_gets_no_placement_answer` is the negative control the
//! ruling asked for by name.

use std::collections::BTreeSet;

use pw_core::ontology::Ontology;
use pw_core::placement::{Demand, Grant, PlacementLookup, Placements, World, solve};
use pw_core::privacy::Label;

/// **`World::worlds_for`, as it read on the day it was deleted.**
///
/// Not a fallback and not reachable from the compiler — a frozen record, in a
/// test, of what the deleted table said. `no_source_file_maps_an_effect_family_
/// to_a_world` is the structural gate that keeps it the only copy.
const THE_TABLE: &[(&str, &[World])] = &[
    ("database", &[World::Origin]),
    ("secret", &[World::Origin]),
    ("durable", &[World::Origin]),
    ("dom", &[World::Browser]),
    ("style", &[World::Browser]),
    ("layout", &[World::Browser]),
    ("observe", &[World::Browser]),
    ("animation", &[World::Browser]),
    ("paint", &[World::Browser]),
    ("device", &[World::Browser]),
    ("network", &[World::Browser, World::Edge, World::Origin]),
    ("cache", &[World::Edge, World::Origin]),
];

/// The platform packages, which is where the effect vocabulary now lives.
fn platform() -> Ontology {
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    let mut sources: Vec<String> = Vec::new();
    for dir in ["packages/pw-std", "packages/pw-platform-web"] {
        let mut files: Vec<std::path::PathBuf> = std::fs::read_dir(root.join(dir))
            .unwrap_or_else(|e| panic!("{dir}: {e}"))
            .map(|e| e.expect("entry").path())
            .filter(|p| p.extension().is_some_and(|x| x == "pw"))
            .collect();
        files.sort();
        for p in files {
            sources.push(std::fs::read_to_string(&p).expect("read"));
        }
    }
    let hirs: Vec<pw_core::hir::Hir> = sources
        .iter()
        .map(|s| pw_core::lower::lower_file(s, &pw_syntax::parse_tree(s).green))
        .collect();
    let refs: Vec<&pw_core::hir::Hir> = hirs.iter().collect();
    let ws = pw_core::resolve::Workspace::build(&refs);
    Ontology::build_with(&refs, &ws)
}

/// The worlds a body performing these effects can run in.
fn feasible(effects: &[&str]) -> Vec<String> {
    let demand = Demand {
        effects: effects.iter().map(|s| s.to_string()).collect(),
        label: Label::public(),
        declared: None,
    };
    let mut out: Vec<String> = solve(&demand, &platform())
        .feasible
        .into_iter()
        .map(|w| w.name().to_string())
        .collect();
    out.sort();
    out
}

#[test]
fn the_solver_answers_at_all() {
    // The control. Every row below is a set comparison, and a solver returning
    // nothing for everything would make half of them trivially true.
    assert_eq!(feasible(&[]), ["browser", "build", "edge", "origin"]);
    assert_eq!(
        platform().placement_of("database.read"),
        PlacementLookup::Known(vec![World::Origin])
    );
}

#[test]
fn the_placement_baseline() {
    // Every effect the corpus writes, and where a body performing it may run.
    // Sorted names rather than `World`s, so a change shows as a readable diff.

    // Origin-only: real authority the browser must not have.
    for e in [
        "database.read<Stores>",
        "database.write<Carts>",
        "database.transaction",
        "database.connect",
        "secret<Payments>",
        "secret.read",
    ] {
        assert_eq!(feasible(&[e]), ["origin"], "{e}");
    }

    // Browser-only: meaningful where the document is.
    for e in [
        "dom.mutate",
        "dom.read",
        "layout.measure",
        "style.mutate<LayoutAffect>",
        "style.mutate<PaintOnly>",
        "animation.composite",
        "paint.custom",
        "device.location",
        "device.query",
    ] {
        assert_eq!(feasible(&[e]), ["browser"], "{e}");
    }

    // Anywhere with a request context; build time has none.
    for e in ["network.fetch", "network.subscribe"] {
        assert_eq!(feasible(&[e]), ["browser", "edge", "origin"], "{e}");
    }

    // Declared, and declared to constrain nothing. `session.read` is here
    // deliberately: its authority is a capability a topology provides, and
    // there is no world where reading session state is meaningless.
    for e in [
        "log<Public>",
        "trace",
        "clock.wall",
        "clock.read",
        "resource.acquire<MapHandle>",
        "resource.release<MapHandle>",
        "unsafe.raw_html",
        "session.read",
    ] {
        assert_eq!(
            feasible(&[e]),
            ["browser", "build", "edge", "origin"],
            "{e}"
        );
    }
}

#[test]
fn the_combinations_that_have_nowhere_to_run() {
    // The rows that prove placement is an INTERSECTION and not a lookup. These
    // are what a planner consuming ontology + topology must keep answering the
    // same way.
    assert!(
        feasible(&["layout.measure", "database.read<Stores>"]).is_empty(),
        "browser-only and origin-only in one body"
    );
    assert!(
        feasible(&["dom.mutate", "secret<Payments>"]).is_empty(),
        "the charter's opening example: a secret in a browser component"
    );
    // And one that DOES intersect, so "empty" is not the answer to everything.
    assert_eq!(
        feasible(&["network.fetch", "database.read<Stores>"]),
        ["origin"],
        "a fetch and a read agree on the origin"
    );
}

/// **The after half: every declaration still says what the table said.**
///
/// The table spoke per FAMILY and the declarations speak per OPERATION, so this
/// compares each declared effect against its family's old row. A family the
/// table never mentioned has nothing to compare — the declaration is then the
/// only statement of where that effect is meaningful, which is the point of
/// adding the clause.
#[test]
fn every_declared_placement_matches_what_the_deleted_table_said() {
    let ontology = platform();
    let mut disagreements: Vec<String> = Vec::new();
    let mut compared = 0usize;

    for decl in ontology.declarations() {
        if decl.placement.is_empty() {
            continue;
        }
        let Some((_, table)) = THE_TABLE.iter().find(|(f, _)| *f == decl.path.family()) else {
            continue;
        };
        compared += 1;
        let declared: BTreeSet<World> = decl.placement.iter().copied().collect();
        let tabled: BTreeSet<World> = table.iter().copied().collect();
        if declared != tabled {
            disagreements.push(format!(
                "{}: declares {:?}, the table said {:?}",
                decl.path.text(),
                declared.iter().map(|w| w.name()).collect::<Vec<_>>(),
                tabled.iter().map(|w| w.name()).collect::<Vec<_>>(),
            ));
        }
    }

    assert!(
        compared >= 10,
        "only {compared} effects were compared, which is too few for this to \
         be measuring anything"
    );
    assert!(
        disagreements.is_empty(),
        "a declared placement no longer matches what `World::worlds_for` said \
         when it was deleted:\n  {}\n\n\
         That is allowed — the declarations are the authority now — but it is \
         a change to where code may run, and the ruling asks for it to be \
         classified rather than absorbed. Update THE_TABLE with the reason.",
        disagreements.join("\n  ")
    );
}

/// **The exact old hole, and it must stay closed.**
///
/// Architect ruling, 2026-08-07:
///
/// > I would explicitly negative-test the exact old hole: `effect typo.thing`
/// > with some matching old `worlds_for` spelling injected as a control, and
/// > require that it still cannot acquire a placement without an
/// > `EffectDefId`. That proves the deletion is semantic, not merely that the
/// > table stopped being called in today's fixtures.
///
/// So the control is a spelling the deleted table DID recognise — `dom.thing`,
/// whose family is one of the twelve rows above — written by a program that
/// declares no such effect. Under the table it would have been browser-only.
/// Under the ontology it is `Blocked`: nothing declares it, so placement has no
/// answer, and the answer it must not give is "anywhere".
#[test]
fn an_undeclared_effect_gets_no_placement_answer() {
    let ontology = platform();

    for effect in [
        // A family the deleted table restricted, with an operation nobody
        // declares. The strongest form of the control: every ingredient the
        // old table needed is present.
        "dom.thing",
        "database.purge",
        // A family nothing declares at all.
        "typo.thing",
        // And the type-argument shape that beat the table's `split('.')`.
        "secrets<Payments>",
    ] {
        let lookup = ontology.placement_of(effect);
        assert!(
            matches!(lookup, PlacementLookup::Blocked { .. }),
            "`{effect}` is declared nowhere and must not receive a placement, \
             got {lookup:?}"
        );
        assert_eq!(
            World::Browser.grants(effect, &ontology),
            Grant::Blocked,
            "`{effect}`"
        );

        let solution = solve(
            &Demand {
                effects: vec![effect.to_string()],
                label: Label::public(),
                declared: None,
            },
            &ontology,
        );
        assert!(solution.is_blocked(), "`{effect}`");
        assert!(
            solution.feasible.is_empty(),
            "`{effect}` must not be placeable anywhere, got {:?}",
            solution.feasible
        );
        // Not reported as a world's failing either. No world refused it; the
        // effect has no meaning, and a `MissingCapability` ruling would blame
        // the placement for a misspelled row.
        assert!(solution.ruled_out.is_empty(), "`{effect}`");
    }

    // The discriminating control. Every assertion above would hold for a
    // solver that blocked everything, which is the failure mode the first
    // version of the deleted table actually had.
    let real = ontology.placement_of("dom.mutate");
    assert_eq!(real, PlacementLookup::Known(vec![World::Browser]));
    assert_eq!(
        World::Browser.grants("dom.mutate", &ontology),
        Grant::Yes,
        "the correctly spelled neighbour still resolves"
    );
}

/// A blocked effect and an unplaceable one are different failures.
///
/// Both leave `feasible` empty — deliberately, so a caller that forgets to ask
/// refuses rather than permits — and a caller that reports "nowhere to run" for
/// the first sends the reader to the placement they wrote instead of to the
/// effect they misspelled.
#[test]
fn blocked_is_distinguishable_from_unsatisfiable() {
    let ontology = platform();
    let of = |effects: &[&str]| {
        solve(
            &Demand {
                effects: effects.iter().map(|s| s.to_string()).collect(),
                label: Label::public(),
                declared: None,
            },
            &ontology,
        )
    };

    let nowhere = of(&["dom.mutate", "database.read<Stores>"]);
    assert!(!nowhere.is_satisfiable());
    assert!(
        !nowhere.is_blocked(),
        "both effects are perfectly well known"
    );
    assert!(!nowhere.ruled_out.is_empty(), "and every world said why");

    let unknown = of(&["dom.thing"]);
    assert!(!unknown.is_satisfiable());
    assert!(unknown.is_blocked());
    assert!(unknown.ruled_out.is_empty());
}

// --- the structural gate -----------------------------------------------------

/// **No source file maps an effect family to a world.**
///
/// Architect ruling, 2026-08-07, step 6 item 6: *"Structural test: no legacy
/// effect-spelling→world mapping remains."*
///
/// A behavioural test cannot prove that absence — it can only show that the
/// programs it happens to write do not consult one. So this is structural, and
/// it is the same shape as `no_source_file_maps_a_phase_keyword_to_an_effect_
/// spelling`: scan the crate for a family name sitting next to a `World`, which
/// is exactly what `worlds_for`'s arms were.
///
/// ```text
/// "database" | "secret" | "durable" => &[Origin],
/// ```
///
/// [`THE_TABLE`] above is the one permitted copy, and it lives in a test where
/// nothing can reach it.
///
/// # Where the scan stops
///
/// At each file's `#[cfg(test)]`, because that is the boundary of what the
/// compiler is built from. Two test doubles live past it — `placement.rs`'s
/// `PLATFORM` and `effects.rs`'s `Platform` — and both exist precisely because
/// the code under test no longer carries a table and has to be handed one.
/// Forbidding them would forbid testing the thing the deletion created.
///
/// A real table hidden behind a `#[cfg(test)]` would evade this, and would also
/// not be reachable from the compiler, which is the property being defended.
#[test]
fn no_source_file_maps_an_effect_family_to_a_world() {
    let dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("src");
    let families = [
        "database",
        "secret",
        "durable",
        "dom",
        "style",
        "layout",
        "observe",
        "animation",
        "paint",
        "device",
        "network",
        "cache",
        "session",
    ];
    let worlds = ["Origin", "Browser", "Edge", "Build"];
    let mut offenders = Vec::new();
    let mut scanned = 0usize;
    let mut lines_read = 0usize;

    for e in std::fs::read_dir(&dir).expect("src") {
        let p = e.expect("entry").path();
        if p.extension().and_then(|x| x.to_str()) != Some("rs") {
            continue;
        }
        scanned += 1;
        let file = p.file_name().unwrap().to_string_lossy().to_string();
        let text = std::fs::read_to_string(&p).expect("read");
        let compiled = text
            .lines()
            .take_while(|l| l.trim() != "#[cfg(test)]")
            .collect::<Vec<_>>();
        lines_read += compiled.len();
        for (n, line) in compiled.into_iter().enumerate() {
            let l = line.trim();
            if l.starts_with("//") || l.starts_with("///") || l.starts_with("//!") {
                continue;
            }
            // A quoted family name and a `World` variant on one line. Match-arm
            // shaped or not: an `if effect == "database" { World::Origin }` is
            // the same table written longhand, and a scan that only looked for
            // `=>` would miss it.
            if families.iter().any(|f| l.contains(&format!("\"{f}\"")))
                && worlds.iter().any(|w| l.contains(w))
            {
                offenders.push(format!("{file}:{}: {l}", n + 1));
            }
        }
    }

    assert!(
        scanned >= 25,
        "the scan read {scanned} files, which is implausibly few"
    );
    assert!(
        lines_read >= 10_000,
        "the scan read {lines_read} lines, which means the `#[cfg(test)]` cut \
         is truncating far more than the test modules"
    );
    assert!(
        offenders.is_empty(),
        "a source file maps an effect family to a world:\n  {}\n\n\
         Where an effect is meaningful is a DECLARED fact — the `placement` \
         clause on its `effect` declaration — and a table in the compiler is a \
         second answer to a question the program already answers. That is what \
         `World::worlds_for` was, and it is what let `secret<Payments>` be \
         placeable in the browser for a milestone.",
        offenders.join("\n  ")
    );
}

/// The control for the scan above, since "no offenders" is what a broken
/// detector says too. This is `worlds_for`'s exact shape.
#[test]
fn the_gate_can_detect_the_thing_it_forbids() {
    let sample = r#"            "database" | "secret" | "durable" => &[Origin],"#;
    let families = ["database", "secret"];
    let worlds = ["Origin", "Browser"];
    assert!(
        families
            .iter()
            .any(|f| sample.contains(&format!("\"{f}\"")))
    );
    assert!(worlds.iter().any(|w| sample.contains(w)));

    // And the discriminating half: an ordinary line mentioning one but not the
    // other is not an offender, or the gate would forbid `World::Origin`
    // appearing anywhere at all.
    let innocent = "    let w = World::Origin;";
    assert!(
        !families
            .iter()
            .any(|f| innocent.contains(&format!("\"{f}\"")))
    );
}
