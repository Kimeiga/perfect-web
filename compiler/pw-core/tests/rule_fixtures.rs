//! `examples/rules/**` — programs that exercise one rule from several angles.
//!
//! Distinct from the charter §16 corpus in `examples/{accepted,rejected}`,
//! which is policed for category coverage. Architect ruling, 2026-08-06:
//!
//! > Add or split fixtures so the rule cannot accidentally pass through only
//! > one half. At least one valid case should prove the checker is not merely
//! > banning every unsafe escape.
//!
//! Each fixture declares `// @expect: clean` or `// @expect: PWxxxx`.

use pw_core::check::check_sources;

/// The shared library every rule fixture is checked against.
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

fn fixtures() -> Vec<(String, String, String, String)> {
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../examples/rules");
    let mut out = Vec::new();
    let mut dirs = vec![root];
    while let Some(dir) = dirs.pop() {
        for e in std::fs::read_dir(&dir).expect("examples/rules") {
            let p = e.expect("entry").path();
            if p.is_dir() {
                dirs.push(p);
                continue;
            }
            if p.extension().is_none_or(|x| x != "pw") {
                continue;
            }
            let src = std::fs::read_to_string(&p).expect("read");
            let expect = src
                .lines()
                .find_map(|l| l.trim().strip_prefix("// @expect:"))
                .unwrap_or_else(|| panic!("{p:?} has no `// @expect:`"))
                .trim()
                .to_string();
            out.push((
                p.file_name().unwrap().to_string_lossy().to_string(),
                src,
                expect,
                dir.file_name().unwrap().to_string_lossy().to_string(),
            ));
        }
    }
    out.sort();
    out
}

#[test]
fn every_rule_fixture_reports_exactly_what_it_declares() {
    let all = fixtures();
    assert!(
        all.len() >= 5,
        "expected the rule fixtures, saw {}",
        all.len()
    );

    // The platform packages and the shared domain, so a fixture can exercise a
    // rule that reads a SIGNATURE rather than only a syntax shape. Without
    // them `log.public` resolves to nothing and the privacy-sink rule cannot
    // fire — the fixture would pass by being unreadable.
    let mut files: Vec<(String, String)> = library();
    files.extend(all.iter().map(|(n, s, _, _)| (n.clone(), s.clone())));
    let results: std::collections::HashMap<_, _> = check_sources(&files).into_iter().collect();

    let mut wrong = Vec::new();
    for (name, _, expect, _) in &all {
        let got: Vec<&str> = results[name].iter().map(|d| d.code).collect();
        let ok = if expect == "clean" {
            got.is_empty()
        } else {
            got.contains(&expect.as_str())
        };
        if !ok {
            wrong.push(format!("{name}: expected {expect}, got {got:?}"));
        }
    }
    assert!(wrong.is_empty(), "{}", wrong.join("\n"));
}

#[test]
fn at_least_one_fixture_per_directory_is_clean() {
    // The control the architect asked for. A rule that banned its construct
    // outright would pass every rejected fixture and be wrong; only a valid
    // case can tell the difference.
    //
    // Checked per directory, which is what the name says and what the previous
    // version did not do: one clean fixture anywhere satisfied the whole
    // suite, so a new rule could arrive with no control at all and the test
    // would stay green on someone else's.
    use std::collections::{BTreeMap, BTreeSet};
    let all = fixtures();
    let mut by_dir: BTreeMap<&str, (usize, usize)> = BTreeMap::new();
    for (_, _, expect, dir) in &all {
        let e = by_dir.entry(dir.as_str()).or_default();
        e.0 += 1;
        if expect == "clean" {
            e.1 += 1;
        }
    }
    let uncontrolled: BTreeSet<&str> = by_dir
        .iter()
        .filter(|(_, (_, clean))| *clean == 0)
        .map(|(d, _)| *d)
        .collect();
    assert!(
        uncontrolled.is_empty(),
        "these rule directories have no valid case, so they cannot tell a rule \
         from a ban: {uncontrolled:?}"
    );
}

#[test]
fn the_unsafe_audit_rule_checks_both_halves_and_the_target() {
    // R-044 exposed that the rule checked the justification and stopped, so a
    // fixture missing its attribution target passed. These three cases pin all
    // three failure modes apart.
    let all = fixtures();
    let by_name = |n: &str| {
        all.iter()
            .find(|(f, _, _, _)| f == n)
            .unwrap_or_else(|| panic!("{n} missing"))
            .clone()
    };
    for (name, code) in [
        ("missing-justification.pw", "PW5010"),
        ("missing-attribution.pw", "PW5010"),
        ("unresolved-attribution.pw", "PW5015"),
    ] {
        let (n, src, _, _) = by_name(name);
        let d = &check_sources(&[(n.clone(), src)])[0].1;
        assert!(
            d.iter().any(|d| d.code == code),
            "{n} must report {code}, got {:?}",
            d.iter().map(|d| d.code).collect::<Vec<_>>()
        );
    }

    // An unresolved target and a missing one are different invariants: one is
    // an incomplete record, the other is a record that reads as reviewed and
    // is not.
    let (n, src, _, _) = by_name("unresolved-attribution.pw");
    let d = &check_sources(&[(n, src)])[0].1;
    assert!(
        !d.iter().any(|d| d.code == "PW5010"),
        "a complete-but-invalid record is not an incomplete one"
    );
}
