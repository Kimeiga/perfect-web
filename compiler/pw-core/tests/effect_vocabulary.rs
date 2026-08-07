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

/// The effects written at more than one arity.
///
/// **Empty, and it stays empty.** Architect ruling, 2026-08-07:
///
/// > I would keep the original rule: `effect database.read<T>` means a use must
/// > supply exactly one argument. […] Don't make omission secretly mean
/// > wildcard.
///
/// It was two entries — `database.read` written bare 19 times and
/// `style.mutate` twice — and all 21 rows were classified and specified rather
/// than mechanically rewritten. What each turned out to be:
///
/// ```text
/// 7   examples/lib/*.pw          real interface contracts. `Stores.get` reads
///                                Stores, `Carts.current` reads Carts,
///                                `Menus.for_store` reads Menus. The row was
///                                saying "the database" where the function
///                                names the domain in its own module header
/// 9   generality/ and rules/     fixtures whose bodies all call `Stores.*`,
///                                so `<Stores>` is what they already meant
/// 2   style.pw                   `mutate` and `set_custom`, which said "does
///                                not invalidate layout" by OMISSION — the
///                                wildcard the ruling rejects, and there was no
///                                way to say it positively until `PaintOnly`
/// 1   Database.connect           see below: the one that is not a domain read
/// 2   Menus.for_user etc.        partition witnesses, same domain
/// ```
///
/// So the ruling's prediction held: the rows were underspecified rather than
/// deliberately general, and specifying them named a distinction the platform
/// had been making in comments. `PaintOnly` is the clearest case — five corpus
/// files distinguished layout-affecting writes from ordinary ones, and the
/// ordinary side of that distinction had no name.
const ARITY_UNSETTLED: &[(&str, usize, usize)] = &[];

#[test]
fn no_effect_is_written_at_two_different_arities() {
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
         An effect's arity is part of its identity, and a use must supply \
         exactly the number its declaration binds. Omission does NOT mean \
         \"any T\" — architect ruling, 2026-08-07. If a row genuinely means \
         \"any database read\", that is the motivating case for explicit \
         wildcard syntax (`database.read<_>`) and wants a ruling, not a bare \
         spelling.\n"
    );
}

#[test]
fn every_effect_is_written_at_exactly_the_arity_it_declares() {
    // The rule, now that `ARITY_UNSETTLED` is empty: a use supplies exactly
    // the number of arguments its declaration binds. This and
    // `no_effect_is_written_at_two_different_arities` are complementary —
    // that one catches an effect written inconsistently even when nothing
    // declares it, this one catches one written consistently and wrongly.
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

// --- placement: the fact the deleted table used to hold ---------------------

/// The families that are placement-constrained but not authority-constrained.
///
/// The three separated facts, and the pair this test is about:
///
/// ```text
/// Effect       what computation does
/// Capability   authority it must be GRANTED
/// Placement    where it is meaningful/possible
/// ```
///
/// `World::worlds_for` answered the third and `Capability::resolve` read its
/// answer as the second, which is why `dom.mutate` used to emit
/// `pw:host/dom#mutate` — asking a host to grant the document to code whose
/// only crime is being a user interface. These five families are where the two
/// questions came apart: `placement browser, capability none` is both facts
/// stated separately, and a host is asked for neither.
///
/// The table is gone as of 2026-08-07; `tests/placement_migration.rs` holds
/// what it said, frozen. This states the fact from the declarations alone, so
/// a family gaining or losing a capability clause is still a visible change to
/// what the compiler tells a host.
#[test]
fn the_families_that_need_no_host_import_are_exactly_the_ones_recorded() {
    // `post_paint` is here and was not in the table-based version of this test,
    // which is the one row the migration moved. Not a change in behaviour: the
    // deleted table had no `post_paint` entry, so it said nothing about the
    // family and the old test skipped it. Reading the declarations directly
    // sees what the declarations say. It is a placement-constrained,
    // authority-free effect exactly like `paint`, and the reason it exists at
    // all — two spellings for one thing, `paint.post` and `post_paint` — is
    // recorded in `packages/pw-platform-web/effects.pw` rather than reconciled
    // here, because reconciling it changes what corpus file A-022 says.
    const NEEDS_NO_HOST_IMPORT: &[&str] =
        &["animation", "dom", "layout", "paint", "post_paint", "style"];

    let (_hirs, ontology) = ontology_and_hirs();
    let mut found: BTreeSet<String> = BTreeSet::new();
    for decl in ontology.declarations() {
        let family = decl.path.family();
        if !decl.placement.is_empty() && decl.capability.is_none() {
            found.insert(family.to_string());
        }
    }

    let expected: BTreeSet<String> = NEEDS_NO_HOST_IMPORT.iter().map(|s| s.to_string()).collect();
    assert_eq!(
        found, expected,
        "\nThe set of families that need no host import has changed.\n\n\
         Every family here declares a placement and `capability none` — \
         constrained by where it is meaningful rather than by authority. A \
         family joining this list stops asking a host for something; a family \
         leaving it starts. Both change what the compiler tells the host, and \
         neither should happen by accident.\n"
    );

    // The control: `database` declares both a placement and a capability, so
    // it is not in the list. Without this the assertion above would also pass
    // for a bug that put every placement-constrained family in the set.
    assert!(!found.contains("database"), "{found:?}");
    assert!(!found.contains("secret"), "{found:?}");
    assert!(!found.contains("network"), "{found:?}");
}
