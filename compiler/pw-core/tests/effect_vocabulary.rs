//! Every effect the corpus writes is declared somewhere.
//!
//! Effect ontology slice 2, step 3. Architect ruling, 2026-08-07:
//!
//! > Do not wait for full E9 to declare effect families and operations. […]
//! > A wildcard is convenient documentation but terrible semantic identity.
//!
//! `docs/RISK_QUEUE.md` 37 is what one missing declaration costs.
//! `style.mutate<LayoutAffect>` appeared in five corpus files and `style.pw`'s
//! own comment called the type argument "the point" — and `LayoutAffect` was
//! declared nowhere, so the distinction rested on a spelling no checker could
//! resolve. `style.mutate<LayoutAffect>` and `style.mutate<Anything>` were the
//! same effect to every analysis that read the row.
//!
//! This is the test that makes the next one of those loud. It reads the effect
//! rows out of the whole corpus and requires each name to name a declaration.
//!
//! # The vocabulary is MEASURED, not recalled
//!
//! `docs/NEXT.md` carried a remembered list. Compared against the corpus it
//! named four effects that appear in no row — `cache.read`, `cache.write`,
//! `observe.*`, `durable.*` — and omitted four that do: `session.read`,
//! `device.query`, `network.subscribe`, `unsafe.raw_html`. Which is exactly
//! why the list lives in a file the compiler reads rather than in prose.

use std::collections::{BTreeMap, BTreeSet};
use std::path::{Path, PathBuf};

use pw_core::hir::Hir;
use pw_core::lower::lower_file;
use pw_core::ontology::Ontology;
use pw_syntax::parse_tree;

fn root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../..")
}

/// Every `.pw` under the corpus directories, as (path, source).
fn corpus() -> Vec<(String, String)> {
    let root = root();
    let mut out = Vec::new();
    let mut stack = vec![
        root.join("packages"),
        root.join("examples"),
        root.join("spikes/own-renderer"),
    ];
    while let Some(dir) = stack.pop() {
        let Ok(entries) = std::fs::read_dir(&dir) else {
            continue;
        };
        for e in entries {
            let p = e.expect("entry").path();
            if p.is_dir() {
                stack.push(p);
            } else if p.extension().is_some_and(|x| x == "pw") {
                let rel = p
                    .strip_prefix(&root)
                    .unwrap_or(&p)
                    .to_string_lossy()
                    .to_string();
                out.push((rel, std::fs::read_to_string(&p).expect("read")));
            }
        }
    }
    out.sort();
    out
}

/// The platform packages, which is where the declarations live.
fn ontology_and_hirs() -> (Vec<Hir>, Ontology) {
    let hirs: Vec<Hir> = corpus()
        .iter()
        .filter(|(path, _)| path.starts_with("packages/"))
        .map(|(_, src)| lower_file(src, &parse_tree(src).green))
        .collect();
    let refs: Vec<&Hir> = hirs.iter().collect();
    let ontology = Ontology::build(&refs);
    (hirs, ontology)
}

/// Every effect written in a row, with the shapes it was written in.
///
/// From the LOWERED rows, not from a regex over the text. The point of
/// `EffectRef::args` is that the parser answered this; a test that re-scanned
/// the source would be measuring its own scanner.
fn written() -> BTreeMap<String, BTreeMap<usize, Vec<String>>> {
    let mut out: BTreeMap<String, BTreeMap<usize, Vec<String>>> = BTreeMap::new();
    for (path, src) in corpus() {
        // History files are frozen snapshots of what the language USED to
        // accept — `docs/CORPUS.md` — so they are not evidence about today's
        // vocabulary.
        if path.contains("examples/history/") {
            continue;
        }
        let hir = lower_file(&src, &parse_tree(&src).green);
        for (_, decl) in hir.all_decls() {
            for e in decl.declared_effects.iter().flatten() {
                out.entry(e.path.clone())
                    .or_default()
                    .entry(e.args.len())
                    .or_default()
                    .push(path.clone());
            }
        }
    }
    out
}

#[test]
fn a_declared_effect_no_program_uses_is_not_an_error() {
    // Architect correction, 2026-08-07:
    //
    // > The corpus-derived effect list should be a LOWER BOUND: every effect
    // > used by the program must resolve to a declaration — not necessarily
    // > `declared == used`. Otherwise adding a legitimate platform effect
    // > before a corpus example exists would perversely make the test fail.
    //
    // `paint.post` is already such an effect: `effects.rs`'s `intrinsic_effect`
    // produces it for a `post_paint { .. }` block and no row writes it. The
    // direction is asserted rather than assumed, because "every used effect is
    // declared" and "every declared effect is used" read almost the same and
    // one of them is wrong.
    let (_hirs, ontology) = ontology_and_hirs();
    let used: BTreeSet<String> = written().into_keys().collect();
    let declared_but_unused: Vec<String> = ontology
        .declarations()
        .map(|d| d.path.text())
        .filter(|n| !used.contains(n))
        .collect();
    assert!(
        !declared_but_unused.is_empty(),
        "the corpus happens to use every declared effect, so this test is not \
         currently proving the direction it exists to prove"
    );
    // And the suite is still green, which is the actual assertion: nothing
    // above fails because of these.
}

#[test]
fn the_scan_finds_the_rows_it_is_looking_for() {
    // The control. A scan that found nothing would report a fully declared
    // vocabulary, which is what a broken reader says too.
    let found = written();
    assert!(
        found.len() >= 20,
        "the scan found {} distinct effects, which is implausibly few: {:?}",
        found.len(),
        found.keys().collect::<Vec<_>>()
    );
    assert!(found.contains_key("database.read"), "{:?}", found.keys());
    assert!(found.contains_key("layout.measure"), "{:?}", found.keys());
}

#[test]
fn every_effect_the_corpus_writes_is_declared() {
    let (_hirs, ontology) = ontology_and_hirs();
    let mut undeclared: Vec<String> = Vec::new();

    for (name, shapes) in written() {
        // A bare family — `!{ database }` — is a broad claim covering the
        // family's members, and is satisfied by the family existing.
        if ontology.get(&name).is_some() || ontology.has_family(&name) {
            continue;
        }
        let example = shapes
            .values()
            .flatten()
            .next()
            .cloned()
            .unwrap_or_default();
        undeclared.push(format!("{name}  (e.g. {example})"));
    }

    assert!(
        undeclared.is_empty(),
        "these effects are written in the corpus and declared nowhere:\n  {}\n\n\
         Declare each in `packages/pw-std/effects.pw` or \
         `packages/pw-platform-web/effects.pw`. An effect that names nothing \
         means whatever the reader assumes — `docs/RISK_QUEUE.md` 37.",
        undeclared.join("\n  ")
    );
}

#[test]
fn no_declared_effect_is_a_wildcard() {
    // The ruling: "A wildcard is convenient documentation but terrible
    // semantic identity." `observe.*` and `durable.*` were in the remembered
    // vocabulary; neither may arrive as a declaration.
    let (_hirs, ontology) = ontology_and_hirs();
    let wild: Vec<String> = ontology
        .declarations()
        .map(|d| d.path.text())
        .filter(|n| n.contains('*') || n.ends_with('.'))
        .collect();
    assert!(wild.is_empty(), "wildcard effect declarations: {wild:?}");
}

#[test]
fn every_declaration_says_what_authority_it_needs() {
    // `capability none` is an answer; a MISSING capability clause is not.
    // Three facts used to be one string, and the whole value of separating
    // them is lost if an effect can decline to say which of the three it is.
    let (_hirs, ontology) = ontology_and_hirs();
    let (_hirs2, mut sources) = (0, Vec::new());
    for (path, src) in corpus() {
        if path.starts_with("packages/") {
            sources.push((path, src));
        }
    }

    let mut silent: Vec<String> = Vec::new();
    for decl in ontology.declarations() {
        let has_clause = sources.iter().any(|(_, src)| {
            let hir = lower_file(src, &parse_tree(src).green);
            hir.all_decls().any(|(_, d)| {
                d.kind == pw_core::hir::DeclKind::Effect
                    && d.name == decl.path.text()
                    && d.policy("capability").is_some()
            })
        });
        if !has_clause {
            silent.push(decl.path.text());
        }
    }
    assert!(
        silent.is_empty(),
        "these effects declare no `capability` clause: {silent:?}. \
         Write `capability none` if it needs no host authority — an effect \
         that does not say is not the same as one that says none."
    );
}

/// The effects written at more than one arity, and how many uses each shape has.
///
/// **A ratchet, not an allowance.** The architect's test list says a bare
/// `database.read` is a wrong-arity error, and the corpus writes one nineteen
/// times. Wiring the diagnostic today would report nineteen working rows, so
/// the conflict is MEASURED and pinned here instead of being discovered later
/// as a surprise — and a third ambiguous effect cannot be added quietly.
///
/// Format: effect, uses without an argument, uses with one.
const ARITY_UNSETTLED: &[(&str, usize, usize)] =
    &[("database.read", 19, 5), ("style.mutate", 2, 7)];

#[test]
fn the_two_effects_written_at_two_arities_are_exactly_the_ones_recorded() {
    let mut found: Vec<(String, usize, usize)> = Vec::new();
    for (name, shapes) in written() {
        if shapes.len() < 2 {
            continue;
        }
        let bare = shapes.get(&0).map(Vec::len).unwrap_or(0);
        let one = shapes.get(&1).map(Vec::len).unwrap_or(0);
        found.push((name, bare, one));
    }
    let expected: Vec<(String, usize, usize)> = ARITY_UNSETTLED
        .iter()
        .map(|(n, a, b)| (n.to_string(), *a, *b))
        .collect();

    assert_eq!(
        found, expected,
        "\nThe corpus writes an effect at two different arities, and the set \
         has changed.\n\n\
         If you REPAIRED one, remove it from `ARITY_UNSETTLED` — the list may \
         only shrink.\n\
         If you ADDED one: an effect's arity is part of its identity, and \
         `database.read` written both ways is the open question this list \
         exists to keep visible. See `docs/NEXT.md`.\n"
    );
}

#[test]
fn every_other_effect_is_written_at_exactly_the_arity_it_declares() {
    // The positive side, and the reason the ratchet above is two entries
    // rather than a shrug: twenty-three of twenty-five effects already agree
    // with their declaration, so the two that do not are a real inconsistency
    // and not the normal state of the corpus.
    let (_hirs, ontology) = ontology_and_hirs();
    let unsettled: BTreeSet<&str> = ARITY_UNSETTLED.iter().map(|(n, _, _)| *n).collect();

    let mut wrong: Vec<String> = Vec::new();
    let mut agreed = 0usize;
    for (name, shapes) in written() {
        if unsettled.contains(name.as_str()) {
            continue;
        }
        let Some(decl) = ontology.get(&name) else {
            continue; // a bare family; `every_effect..is_declared` covers it
        };
        for arity in shapes.keys() {
            if *arity == decl.arity {
                agreed += 1;
            } else {
                wrong.push(format!(
                    "{name}: written with {arity}, declared with {}",
                    decl.arity
                ));
            }
        }
    }
    assert!(wrong.is_empty(), "{wrong:#?}");
    assert!(
        agreed >= 20,
        "only {agreed} effects agreed with their declaration, which is too few \
         for this to be measuring anything"
    );
}

// --- placement: the fact `worlds_for` holds as a table ----------------------

/// Where the ontology and `World::worlds_for` disagree, and why.
///
/// Architect ruling, 2026-08-07: the declarations are closer to correct than
/// `worlds_for`, because that table conflates two questions.
///
/// ```text
/// Effect       what computation does
/// Capability   authority it must be GRANTED
/// Placement    where it is meaningful/possible
/// ```
///
/// `worlds_for` answers the third and `Capability::resolve` reads its answer as
/// the second, which is why `dom.mutate` emits `pw:host/dom#mutate` today —
/// asking a host to grant the document to code whose only crime is being a
/// user interface. The declarations say `placement browser, capability none`,
/// which is both facts stated separately.
///
/// This test is the INSTRUMENT for steps 6-8: it measures the divergence
/// before anything acts on it, so a later contract diff can be attributed to
/// the ontology rather than to an accident.
#[test]
fn the_ontology_and_worlds_for_agree_about_placement_where_both_speak() {
    use pw_core::placement::World;

    let (_hirs, ontology) = ontology_and_hirs();
    let mut disagreements: Vec<String> = Vec::new();
    let mut compared = 0usize;

    for decl in ontology.declarations() {
        if decl.placement.is_empty() {
            continue;
        }
        let Some(table) = World::worlds_for(decl.path.family()) else {
            // The table says nothing, so there is nothing to agree with. The
            // declaration is then the ONLY statement of where the effect is
            // meaningful, which is the point of adding the clause.
            continue;
        };
        compared += 1;
        let declared: BTreeSet<World> = decl.placement.iter().copied().collect();
        let tabled: BTreeSet<World> = table.iter().copied().collect();
        if declared != tabled {
            disagreements.push(format!(
                "{}: declared {:?}, `worlds_for` says {:?}",
                decl.path.text(),
                declared.iter().map(|w| w.name()).collect::<Vec<_>>(),
                tabled.iter().map(|w| w.name()).collect::<Vec<_>>(),
            ));
        }
    }

    assert!(
        compared >= 10,
        "only {compared} effects were compared, which is too few for this to \
         be measuring the table"
    );
    assert!(
        disagreements.is_empty(),
        "the ontology and `World::worlds_for` disagree about WHERE an effect \
         is meaningful:\n  {}\n\n\
         They must agree before `worlds_for` can be deleted — otherwise \
         deleting it changes placement as well as authority, and the contract \
         diff in step 8 cannot be attributed.",
        disagreements.join("\n  ")
    );
}

/// The families `worlds_for` restricts that NO declaration claims a capability
/// for.
///
/// This is the whole of step 7's blast radius, measured. Each is a family where
/// `Capability::resolve` produces a capability today — because the family is in
/// the table — and where the declaration says `capability none`. Making
/// `interface_for` declaration-driven removes exactly these host imports and
/// nothing else.
#[test]
fn the_families_that_lose_a_host_import_are_exactly_the_ones_recorded() {
    use pw_core::placement::World;

    const LOSES_ITS_HOST_IMPORT: &[&str] = &["animation", "dom", "layout", "paint", "style"];

    let (_hirs, ontology) = ontology_and_hirs();
    let mut found: BTreeSet<String> = BTreeSet::new();
    for decl in ontology.declarations() {
        let family = decl.path.family();
        if World::worlds_for(family).is_some() && decl.capability.is_none() {
            found.insert(family.to_string());
        }
    }

    let expected: BTreeSet<String> = LOSES_ITS_HOST_IMPORT
        .iter()
        .map(|s| s.to_string())
        .collect();
    assert_eq!(
        found, expected,
        "\nThe set of families that would lose a host import has changed.\n\n\
         Every family here is one `World::worlds_for` restricts and whose \
         declaration says `capability none` — placement-constrained rather \
         than authority-constrained. Step 7 removes their host imports, and \
         step 8 verifies that ONLY these changed. A family joining or leaving \
         this list changes what the compiler tells the host.\n"
    );

    // The control: `database` is restricted AND declares a capability, so it
    // is not in the list. Without this the assertion above would also pass for
    // a bug that put every restricted family in the set.
    assert!(!found.contains("database"), "{found:?}");
    assert!(!found.contains("secret"), "{found:?}");
    assert!(!found.contains("network"), "{found:?}");
}
