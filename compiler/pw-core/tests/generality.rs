//! `examples/generality/**` — the second score.
//!
//! Architect ruling, 2026-08-06, after 44/44:
//!
//! > Corpus conformance ≠ analysis generality ≠ language soundness ≠
//! > production readiness.
//! >
//! > Before 44/44, progress meant catching more fixtures. After 44/44,
//! > progress means making the same guarantees survive programs the fixtures
//! > did not anticipate.
//!
//! So each invariant gets a directory named for its **symbol**, holding
//! programs the corpus fixture did not anticipate:
//!
//! - `caught.pw` — structurally different from the fixture, same invariant,
//!   must be reported. This is what promotes an invariant to
//!   `GENERALLY_ENFORCED`.
//! - `slips-through.pw` — a program that violates the invariant and is **not**
//!   caught today. It must compile clean, and the moment it stops, the gap has
//!   closed and the file is promoted rather than deleted.
//!
//! The second file is the unusual one and it is the point: a known gap that is
//! only written down drifts out of date silently, and a known gap that is
//! executable cannot. `docs/evidence/P0/readiness.txt` lists these in prose;
//! this is the same list, run.

use pw_core::check::check_sources;
use std::collections::{BTreeMap, BTreeSet};

fn library() -> Vec<(String, String)> {
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    let mut out = Vec::new();
    for dir in [
        "examples/lib",
        "packages/pw-std",
        "packages/pw-platform-web",
    ] {
        for e in std::fs::read_dir(root.join(dir)).unwrap_or_else(|e| panic!("{dir}: {e}")) {
            let p = e.expect("entry").path();
            if p.extension().is_none_or(|x| x != "pw") {
                continue;
            }
            out.push((
                p.file_name().unwrap().to_string_lossy().to_string(),
                std::fs::read_to_string(&p).expect("read"),
            ));
        }
    }
    out.push((
        "domain.pw".to_string(),
        std::fs::read_to_string(root.join("examples/domain.pw")).expect("domain.pw"),
    ));
    out.sort();
    out
}

struct Witness {
    invariant: String,
    status: String,
    name: String,
    src: String,
}

fn witnesses() -> Vec<Witness> {
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../examples/generality");
    let mut out = Vec::new();
    let Ok(dirs) = std::fs::read_dir(&root) else {
        return out;
    };
    for d in dirs {
        let dir = d.expect("entry").path();
        if !dir.is_dir() {
            continue;
        }
        for e in std::fs::read_dir(&dir).expect("invariant dir") {
            let p = e.expect("entry").path();
            if p.extension().is_none_or(|x| x != "pw") {
                continue;
            }
            let src = std::fs::read_to_string(&p).expect("read");
            let field = |k: &str| {
                src.lines()
                    .find_map(|l| l.trim().strip_prefix(k))
                    .map(|v| v.trim().to_string())
                    .unwrap_or_else(|| panic!("{p:?} has no `{k}`"))
            };
            out.push(Witness {
                invariant: field("// @invariant:"),
                status: field("// @status:"),
                name: format!(
                    "{}/{}",
                    dir.file_name().unwrap().to_string_lossy(),
                    p.file_name().unwrap().to_string_lossy()
                ),
                src,
            });
        }
    }
    out.sort_by(|a, b| a.name.cmp(&b.name));
    out
}

/// The symbols a witness file reports, checked as its own program.
fn symbols_for(w: &Witness) -> BTreeSet<&'static str> {
    use pw_core::codes::lookup;
    let mut files = library();
    let leaf = w.name.rsplit('/').next().unwrap().to_string();
    files.push((leaf.clone(), w.src.clone()));
    let diags = check_sources(&files)
        .into_iter()
        .find(|(n, _)| *n == leaf)
        .map(|(_, d)| d)
        .unwrap_or_default();

    let mut out: BTreeSet<&'static str> = diags.iter().map(|d| d.symbol()).collect();
    // The declaration rules run on the syntax tree, not through `check_sources`.
    out.extend(
        pw_core::rules::check(&pw_syntax::parse(&w.src).file)
            .iter()
            .filter(|f| f.is_error())
            .filter_map(|f| lookup(f.code).map(|c| c.symbol)),
    );
    out
}

#[test]
fn every_witness_names_a_registered_invariant() {
    let symbols: BTreeSet<&str> = pw_core::codes::ALL.iter().map(|c| c.symbol).collect();
    for w in witnesses() {
        assert!(
            symbols.contains(w.invariant.as_str()),
            "{}: `{}` is not a registered invariant symbol",
            w.name,
            w.invariant
        );
        assert!(
            matches!(w.status.as_str(), "GENERAL" | "NARROW"),
            "{}: unknown @status {:?}",
            w.name,
            w.status
        );
    }
}

#[test]
fn a_general_witness_is_caught_and_a_narrow_one_is_not() {
    let all = witnesses();
    assert!(!all.is_empty(), "no generality witnesses");

    let mut wrong = Vec::new();
    for w in &all {
        let got = symbols_for(w);
        match w.status.as_str() {
            "GENERAL" => {
                if !got.contains(w.invariant.as_str()) {
                    wrong.push(format!(
                        "{}: declares GENERAL for `{}` but was caught by {got:?} — the \
                         analysis does not reach this shape",
                        w.name, w.invariant
                    ));
                }
            }
            // The gap must still be a gap. If this fires, that is GOOD NEWS
            // reported as a failure: the analysis got better and the witness
            // needs promoting to `caught.pw`.
            "NARROW" if got.contains(w.invariant.as_str()) => {
                wrong.push(format!(
                    "{}: declares NARROW for `{}` but is now caught — the gap has \
                     CLOSED. Rename it to `caught.pw`, set @status: GENERAL, and \
                     update docs/evidence/P0/readiness.txt",
                    w.name, w.invariant
                ));
            }
            _ => {}
        }
    }
    assert!(wrong.is_empty(), "{}", wrong.join("\n"));
}

/// The second score, recorded next to the first.
#[test]
fn generalization_is_reported_separately_from_conformance() {
    let all = witnesses();
    let mut by_invariant: BTreeMap<&str, (bool, bool)> = BTreeMap::new();
    for w in &all {
        let e = by_invariant.entry(w.invariant.as_str()).or_default();
        match w.status.as_str() {
            "GENERAL" => e.0 = true,
            "NARROW" => e.1 = true,
            _ => {}
        }
    }

    // Every invariant the rejected corpus exercises.
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../examples/rejected");
    let mut exercised: BTreeSet<String> = BTreeSet::new();
    for e in std::fs::read_dir(&root).expect("rejected") {
        let p = e.expect("entry").path();
        if p.extension().is_none_or(|x| x != "pw") {
            continue;
        }
        let src = std::fs::read_to_string(&p).expect("read");
        if let Some(sym) = src
            .lines()
            .find_map(|l| l.trim().strip_prefix("// @invariant:"))
        {
            exercised.insert(sym.trim().to_string());
        }
    }

    let general: Vec<&str> = exercised
        .iter()
        .filter(|s| by_invariant.get(s.as_str()).is_some_and(|(g, n)| *g && !*n))
        .map(String::as_str)
        .collect();
    let narrow: Vec<&str> = exercised
        .iter()
        .filter(|s| by_invariant.get(s.as_str()).is_some_and(|(_, n)| *n))
        .map(String::as_str)
        .collect();
    let untested: Vec<&str> = exercised
        .iter()
        .filter(|s| !by_invariant.contains_key(s.as_str()))
        .map(String::as_str)
        .collect();

    eprintln!("  corpus conformance:     44 / 44 (checking_source.rs)");
    eprintln!(
        "  generally enforced:     {} / {}",
        general.len(),
        exercised.len()
    );
    eprintln!("  narrowly enforced:      {} — {narrow:?}", narrow.len());
    eprintln!(
        "  generality untested:    {} — {untested:?}",
        untested.len()
    );

    // Recorded, and ratcheted so it cannot fall.
    assert!(
        general.len() >= 9,
        "generalization regressed: {} / {}",
        general.len(),
        exercised.len()
    );
}
