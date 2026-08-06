//! `pw-core`'s algorithms running against real `.pw` source (`docs/NEXT.md` 7).
//!
//! The corpus is the specification. A file marked `@corpus: accepted` that
//! produces an error is a checker bug, and that is asserted first — a checker
//! that reports more by reporting wrongly is worse than one that reports less.

use pw_core::check::{Env, Unit, check_sources};
use pw_core::lower::lower_file;
use pw_syntax::parse_tree;

fn corpus_dir(which: &str) -> Vec<std::path::PathBuf> {
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../examples")
        .canonicalize()
        .expect("examples/ exists");
    let mut out: Vec<_> = std::fs::read_dir(root.join(which))
        .expect("corpus dir")
        .map(|e| e.expect("entry").path())
        .filter(|p| p.extension().is_some_and(|x| x == "pw"))
        .collect();
    out.sort();
    out
}

/// Every corpus file, as (path, source), checked as one program.
fn whole_corpus() -> Vec<(String, String)> {
    let mut out = Vec::new();
    for which in ["accepted", "rejected"] {
        for p in corpus_dir(which) {
            let name = p.file_name().unwrap().to_string_lossy().to_string();
            out.push((name, std::fs::read_to_string(&p).expect("read")));
        }
    }
    out
}

fn unit(name: &str, src: &str) -> Unit {
    let p = parse_tree(src);
    Unit {
        path: name.to_string(),
        src: src.to_string(),
        hir: lower_file(src, &p.green),
    }
}

#[test]
fn no_accepted_corpus_file_reports_an_exhaustiveness_error() {
    // Checked first and on its own. A false positive on an accepted file means
    // the checker is wrong about the language, which is a worse failure than
    // missing a rejected case.
    let results = check_sources(&whole_corpus());
    let mut bad = Vec::new();
    for (path, diags) in &results {
        if !path.starts_with("A-") {
            continue;
        }
        for d in diags {
            bad.push(format!("{path}: [{}] {}", d.code, d.message));
        }
    }
    assert!(
        bad.is_empty(),
        "accepted files must be clean:\n{}",
        bad.join("\n")
    );
}

#[test]
fn the_unhandled_variant_case_is_caught_at_its_real_span() {
    // R-007 declares `@rule: PW0305` and two `@expect-error` lines. It matches
    // on `OrderState`, which is declared in A-002 — the files are checked as
    // one program (assumption A-009).
    let results = check_sources(&whole_corpus());
    let (_, diags) = results
        .iter()
        .find(|(p, _)| p.starts_with("R-007"))
        .expect("R-007 is in the corpus");

    let d = diags
        .iter()
        .find(|d| d.code == "PW0305")
        .unwrap_or_else(|| panic!("R-007 must report PW0305, got {diags:?}"));

    assert_eq!(d.message, "match on `OrderState` is not exhaustive");

    let src = whole_corpus()
        .into_iter()
        .find(|(p, _)| p.starts_with("R-007"))
        .map(|(_, s)| s)
        .unwrap();

    // The span must cover the match, in the original `.pw` file — charter §14
    // M2 gate item 2 says "at original `.pw` spans", not at a generated file's.
    let text = &src[d.primary_span.clone()];
    assert!(
        text.starts_with("match state"),
        "primary span should cover the match, got {text:?}"
    );
    // And the related span must point at the scrutinee, per charter §16.3.
    assert_eq!(&src[d.related[0].span.clone()], "state");

    // The witnesses the file declares it expects.
    let repairs = d
        .repairs
        .iter()
        .map(|r| r.description.clone())
        .collect::<Vec<_>>()
        .join(" | ");
    for want in ["Cancelled", "Failed"] {
        assert!(
            repairs.contains(want),
            "the missing variant `{want}` must be named, got {repairs}"
        );
    }
}

#[test]
fn a_nullary_constructor_is_not_read_as_a_binding() {
    // The load-bearing conversion. `Draft` has no parentheses, so the grammar
    // emits a binding pattern. Read as a binding it matches everything, the
    // first arm covers the scrutinee, and a non-exhaustive match is reported as
    // exhaustive — silently, and in the favourable direction.
    let src = "module m\n\
               type State = | Draft | Sent | Done\n\
               fn f(s: State) -> Int !{} {\n    match s {\n        Draft => 1,\n    }\n}\n";
    let files = vec![("t.pw".to_string(), src.to_string())];
    let diags = &check_sources(&files)[0].1;
    assert_eq!(diags.len(), 1, "expected one diagnostic, got {diags:?}");
    let repairs: Vec<_> = diags[0].repairs.iter().map(|r| &r.description).collect();
    assert!(
        repairs.iter().any(|r| r.contains("Sent")) && repairs.iter().any(|r| r.contains("Done")),
        "both remaining variants must be named, got {repairs:?}"
    );

    // Positive control: the same match with every arm is clean.
    let full = src.replace(
        "        Draft => 1,\n",
        "        Draft => 1,\n        Sent => 2,\n        Done => 3,\n",
    );
    let clean = &check_sources(&[("t.pw".to_string(), full)])[0].1;
    assert!(
        clean.is_empty(),
        "the exhaustive version must be clean: {clean:?}"
    );

    // Negative control: a genuine binding still matches everything.
    let bound = src.replace("        Draft => 1,\n", "        anything => 1,\n");
    let none = &check_sources(&[("t.pw".to_string(), bound)])[0].1;
    assert!(
        none.is_empty(),
        "a binding that names no constructor must cover the match: {none:?}"
    );
}

#[test]
fn a_match_whose_scrutinee_type_is_unknown_is_not_reported() {
    // The rule that keeps this honest: no guessing. `x` has no declared type,
    // so nothing is claimed about it — silence, not a wildcard-shaped error.
    let src = "module m\n\
               type State = | Draft | Sent\n\
               fn f() -> Int !{} {\n    match x {\n        Draft => 1,\n    }\n}\n";
    let diags = &check_sources(&[("t.pw".to_string(), src.to_string())])[0].1;
    assert!(diags.is_empty(), "must not guess a type: {diags:?}");

    // Control: give it a type and the same match is reported.
    let typed = src.replace("fn f()", "fn f(x: State)");
    let now = &check_sources(&[("t.pw".to_string(), typed)])[0].1;
    assert_eq!(now.len(), 1, "with a declared type it must be caught");
}

#[test]
fn constructor_arity_is_respected() {
    // `Confirmed(_)` covers `Confirmed`, but `Confirmed` alone must not be
    // treated as covering a constructor that carries a field, and vice versa.
    let src = "module m\n\
               type S = | A | B(Int)\n\
               fn f(s: S) -> Int !{} {\n    match s {\n        A => 1,\n        B(n) => n,\n    }\n}\n";
    let diags = &check_sources(&[("t.pw".to_string(), src.to_string())])[0].1;
    assert!(diags.is_empty(), "this is exhaustive: {diags:?}");

    let missing_b = src.replace("        B(n) => n,\n", "");
    let d = &check_sources(&[("t.pw".to_string(), missing_b)])[0].1;
    assert_eq!(d.len(), 1);
    assert!(
        d[0].repairs.iter().any(|r| r.description.contains("B(")),
        "the witness must show the field, got {:?}",
        d[0].repairs
    );
}

#[test]
fn the_environment_spans_every_file_checked_together() {
    // Assumption A-009 made testable: a type declared in one file is visible to
    // a match in another, because one invocation is one program.
    let a = (
        "a.pw".to_string(),
        "module a\ntype T = | X | Y\n".to_string(),
    );
    let b = (
        "b.pw".to_string(),
        "module b\nfn f(t: T) -> Int !{} {\n    match t {\n        X => 1,\n    }\n}\n".to_string(),
    );

    let together = check_sources(&[a.clone(), b.clone()]);
    assert_eq!(
        together.iter().map(|(_, d)| d.len()).sum::<usize>(),
        1,
        "checked together, the match is checkable"
    );

    // Control: alone, `T` is unknown and nothing is claimed.
    let alone = check_sources(&[b]);
    assert!(
        alone[0].1.is_empty(),
        "alone, the type is unknown and must not be guessed: {:?}",
        alone[0].1
    );
}

#[test]
fn every_diagnostic_carries_what_charter_16_3_requires() {
    // rule + origin span + boundary span + inferred label + legal alternative.
    let results = check_sources(&whole_corpus());
    let sources: std::collections::HashMap<_, _> = whole_corpus().into_iter().collect();
    let mut n = 0;
    for (path, diags) in &results {
        let src = &sources[path];
        for d in diags {
            n += 1;
            assert!(!d.code.is_empty() && !d.invariant.is_empty(), "{path}");
            assert!(d.primary_span.end <= src.len(), "{path}: span out of range");
            assert!(!d.related.is_empty(), "{path}: no boundary span");
            assert!(
                d.related.iter().all(|r| r.span.end <= src.len()),
                "{path}: related span out of range"
            );
            assert!(
                !d.repairs.is_empty(),
                "{path}: no legal alternative offered"
            );
            assert!(d.explanation.is_some(), "{path}: no explanation");
        }
    }
    assert!(n > 0, "the corpus must produce at least one diagnostic");
}

#[test]
fn building_the_environment_reads_variants_and_opaques() {
    let u = unit(
        "t.pw",
        "module m\nopaque type StoreId = String\ntype S = | A | B(Int)\n",
    );
    let env = Env::build(std::slice::from_ref(&u));
    let p = env.program();
    assert_eq!(p.adts.len(), 1);
    assert_eq!(p.adts[0].name, "S");
    assert_eq!(p.adts[0].ctors.len(), 2);
    assert_eq!(p.adts[0].ctors[1].fields.len(), 1, "B carries one field");
    assert_eq!(p.opaques.len(), 1);
    assert_eq!(p.opaques[0].name, "StoreId");
}

/// Everything a developer would be shown for one diagnostic, as one string.
fn rendered(d: &pw_core::diagnostics::Diagnostic) -> String {
    let mut s = format!("[{}] {} — {}", d.code, d.invariant, d.message);
    if let Some(e) = &d.explanation {
        s.push_str(e);
    }
    for r in &d.repairs {
        s.push_str(&r.description);
    }
    for r in &d.related {
        s.push_str(&r.label);
    }
    s
}

/// The backticked payloads of a corpus file's `@expect-error` lines.
fn expected_payloads(src: &str) -> Vec<String> {
    src.lines()
        .filter_map(|l| l.trim().strip_prefix("// @expect-error:"))
        .flat_map(|l| {
            l.split('`')
                .skip(1)
                .step_by(2)
                .map(|s| s.to_string())
                .collect::<Vec<_>>()
        })
        .collect()
}

#[test]
fn a_caught_file_conveys_every_fact_its_expect_error_lines_declare() {
    // The corpus is the specification. `@expect-error` states what the
    // developer must be told; this asserts the diagnostic actually says it,
    // without pinning the exact prose.
    let sources: std::collections::HashMap<_, _> = whole_corpus().into_iter().collect();
    let results = check_sources(&whole_corpus());
    let mut checked = 0;
    let mut missing = Vec::new();

    for (path, diags) in &results {
        if diags.is_empty() {
            continue;
        }
        let src = &sources[path];
        let all: String = diags.iter().map(rendered).collect::<Vec<_>>().join(" ");
        for want in expected_payloads(src) {
            checked += 1;
            if !all.contains(&want) {
                missing.push(format!("{path}: never mentions `{want}`"));
            }
        }
    }

    assert!(checked > 0, "no caught file declared an @expect-error");
    assert!(missing.is_empty(), "{}", missing.join("\n"));
}

#[test]
fn semantic_coverage_of_the_rejected_corpus_does_not_regress() {
    // The ratchet from docs/NEXT.md item 8, counting declaration rules AND
    // body-level checks together. It is the honest number: what `pw check`
    // actually reports on the rejected corpus today.
    use pw_core::rules;
    use pw_syntax::parse;

    let results: std::collections::HashMap<_, _> =
        check_sources(&whole_corpus()).into_iter().collect();

    let mut caught = Vec::new();
    let mut total = 0;
    for p in corpus_dir("rejected") {
        total += 1;
        let name = p.file_name().unwrap().to_string_lossy().to_string();
        let src = std::fs::read_to_string(&p).expect("read");
        let decl_hits = rules::check(&parse(&src).file).iter().any(|f| f.is_error());
        let body_hits = results.get(&name).is_some_and(|d| !d.is_empty());
        if decl_hits || body_hits {
            caught.push(name);
        }
    }

    assert!(
        total >= 44,
        "expected the full rejected corpus, saw {total}"
    );
    assert!(
        caught.len() >= 5,
        "semantic coverage regressed: {}/{total} caught — {caught:?}",
        caught.len()
    );
}

#[test]
fn every_body_level_rejection_matches_its_declared_rule() {
    use pw_core::diagnostics::canonical_code;
    let sources: std::collections::HashMap<_, _> = whole_corpus().into_iter().collect();
    let mut mismatches = Vec::new();
    let mut checked = 0;

    for (path, diags) in check_sources(&whole_corpus()) {
        if !path.starts_with("R-") || diags.is_empty() {
            continue;
        }
        let src = &sources[&path];
        let Some(rule) = src
            .lines()
            .find_map(|l| l.trim().strip_prefix("// @rule:"))
            .map(str::trim)
        else {
            continue;
        };
        checked += 1;
        let want = canonical_code(rule);
        let got: Vec<&str> = diags.iter().map(|d| d.code).collect();
        if !got.contains(&want) {
            mismatches.push(format!("{path}: declares {want}, got {got:?}"));
        }
    }

    assert!(checked > 0, "no rejected file was caught by a body check");
    assert!(mismatches.is_empty(), "{}", mismatches.join("\n"));
}
