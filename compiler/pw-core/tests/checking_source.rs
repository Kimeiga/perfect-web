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

/// The library every corpus program is checked against: the shared `domain`
/// module plus E2C's platform interfaces.
///
/// Before E2B these tests fed the checker all 68 corpus files as if they were
/// one program. They are not — five rejected fixtures reuse module names with
/// each other, and 17 are reused across the buckets — so the compiler now
/// reports 20 duplicate declarations for a question nobody meant to ask.
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
    let domain = root.join("examples/domain.pw");
    out.push((
        "domain.pw".to_string(),
        std::fs::read_to_string(&domain).expect("domain.pw"),
    ));
    out.sort();
    out
}

/// The accepted corpus as the one program it is: library + every accepted file.
fn whole_corpus() -> Vec<(String, String)> {
    let mut out = library();
    for p in corpus_dir("accepted") {
        let name = p.file_name().unwrap().to_string_lossy().to_string();
        out.push((name, std::fs::read_to_string(&p).expect("read")));
    }
    out
}

/// Every rejected fixture, each checked as its own program.
///
/// Returns `(file name, source, diagnostics)`. Five of them reuse module names
/// with each other, so checking them together asks a question nobody meant.
fn rejected_results() -> Vec<(String, String, Vec<pw_core::diagnostics::Diagnostic>)> {
    corpus_dir("rejected")
        .into_iter()
        .map(|p| {
            let name = p.file_name().unwrap().to_string_lossy().to_string();
            let src = std::fs::read_to_string(&p).expect("read");
            let results = check_sources(&rejected_program(&p));
            let diags = results
                .into_iter()
                .find(|(n, _)| *n == name)
                .expect("the fixture")
                .1;
            (name, src, diags)
        })
        .collect()
}

/// One rejected fixture, as its own program: the library, whatever accepted
/// modules the fixture imports, and the fixture.
///
/// A counterexample may legitimately depend on a correct module — `R-004` is a
/// page that MISUSES a correctly-declared session query, and the `Session`
/// label that makes it a violation comes from that query's own declaration.
/// Pulling the dependency in by its import is what keeps the fixture honest:
/// it sees what it asked for and nothing else.
fn rejected_program(path: &std::path::Path) -> Vec<(String, String)> {
    let src = std::fs::read_to_string(path).expect("read");
    let imported: Vec<String> = src
        .lines()
        .filter_map(|l| l.trim().strip_prefix("import "))
        .map(|m| m.split(['.', ' ']).next().unwrap_or("").to_string())
        .collect();

    let mut out = library();
    for p in corpus_dir("accepted") {
        let s = std::fs::read_to_string(&p).expect("read");
        let Some(module) = s
            .lines()
            .find_map(|l| l.trim().strip_prefix("module "))
            .map(str::trim)
        else {
            continue;
        };
        let head = module.split('.').next().unwrap_or(module);
        if imported.iter().any(|i| i == head) {
            out.push((p.file_name().unwrap().to_string_lossy().to_string(), s));
        }
    }
    out.push((path.file_name().unwrap().to_string_lossy().to_string(), src));
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
fn no_accepted_corpus_file_reports_a_body_level_error() {
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
    // R-007 declares `@rule: PW0305`. Since E2B it imports `OrderState` from
    // `domain` rather than seeing it through an ambient union, so its program
    // is library + itself.
    let path = corpus_dir("rejected")
        .into_iter()
        .find(|p| p.to_string_lossy().contains("R-007"))
        .expect("R-007");
    let results = check_sources(&rejected_program(&path));
    let (_, diags) = results
        .iter()
        .find(|(p, _)| p.starts_with("R-007"))
        .expect("R-007 is in the program");

    let d = diags
        .iter()
        .find(|d| d.code == "PW0305")
        .unwrap_or_else(|| panic!("R-007 must report PW0305, got {diags:?}"));

    assert_eq!(d.message, "match on `OrderState` is not exhaustive");

    let src = std::fs::read_to_string(&path).expect("read");

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
    let mut all: Vec<(String, String, Vec<pw_core::diagnostics::Diagnostic>)> = rejected_results();
    // Accepted files must produce none, which is asserted elsewhere; include
    // them so a diagnostic that only appears there is still shape-checked.
    for (n, s) in whole_corpus() {
        let d = check_sources(&whole_corpus())
            .into_iter()
            .find(|(p, _)| *p == n)
            .map(|(_, d)| d)
            .unwrap_or_default();
        all.push((n, s, d));
    }
    let mut n = 0;
    for (path, src, diags) in &all {
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
    let mut checked = 0;
    let mut missing = Vec::new();

    for p in corpus_dir("rejected") {
        let results = check_sources(&rejected_program(&p));
        let path = p.file_name().unwrap().to_string_lossy().to_string();
        let diags = &results
            .iter()
            .find(|(n, _)| *n == path)
            .expect("the fixture")
            .1;
        if diags.is_empty() {
            continue;
        }
        let src = &std::fs::read_to_string(&p).expect("read");
        let path = &path;
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

/// Fixtures whose declared invariant is only *partly* enforced.
///
/// Architect ruling, 2026-08-06:
///
/// > A corpus fixture counts as enforced only when the entire invariant it
/// > specifies is checked, not merely when its canonical code appears.
///
/// R-044 was the case that produced the rule: the checker required a `because`
/// justification and stopped, so a fixture missing its *attribution target*
/// went green. Both halves are now checked and the list is empty — it stays,
/// because the next partial rule will need somewhere to be recorded rather
/// than rounded up.
const PARTIALLY_ENFORCED: &[(&str, &str)] = &[];

#[test]
fn corpus_enforcement_is_reported_as_three_numbers_not_one() {
    // "19/44 enforced" hides the difference between a red diagnostic, the
    // *right* red diagnostic, and the whole declared invariant being checked.
    use pw_core::diagnostics::canonical_code;
    use pw_core::rules;
    use pw_syntax::parse;

    let (mut errored, mut declared_code, mut fully) = (0, 0, 0);
    let mut total = 0;

    for (name, src, diags) in rejected_results() {
        total += 1;
        let mut codes: Vec<&str> = rules::check(&parse(&src).file)
            .iter()
            .filter(|f| f.is_error())
            .map(|f| f.code)
            .collect();
        codes.extend(diags.iter().map(|d| d.code));
        if codes.is_empty() {
            continue;
        }
        errored += 1;

        let want = src
            .lines()
            .find_map(|l| l.trim().strip_prefix("// @rule:"))
            .map(|r| canonical_code(r.trim()));
        if !want.is_some_and(|w| codes.contains(&w)) {
            continue;
        }
        declared_code += 1;

        let id = &name[..5];
        if !PARTIALLY_ENFORCED.iter().any(|(f, _)| *f == id) {
            fully += 1;
        }
    }

    // Recorded rather than asserted at an exact value: these move as rules land.
    eprintln!("  {errored}/{total} rejected fixtures produce a compile error");
    eprintln!("  {declared_code}/{total} emit their declared canonical code");
    eprintln!("  {fully}/{total} fully enforce the complete declared invariant");

    // The floor moves only upward, and only when a real rule lands. It was
    // briefly 27 while `secret<Payments>` failed to cover `secret.read`: two
    // fixtures reported an effect they had in fact declared. Both were wrong-
    // reason catches, which is why `declared_code == errored` is asserted for
    // equality and not as another floor — a catch that is merely red is a
    // regression even when the count goes up.
    assert!(
        errored >= 25,
        "regressed: {errored}/{total} produce an error"
    );
    assert_eq!(
        declared_code, errored,
        "every catch must be for the declared invariant, not merely red"
    );
    assert!(
        fully >= 25,
        "regressed: {fully}/{total} fully enforced, partial list = {PARTIALLY_ENFORCED:?}"
    );
}

#[test]
fn semantic_coverage_of_the_rejected_corpus_does_not_regress() {
    // The ratchet from docs/NEXT.md item 8, counting declaration rules AND
    // body-level checks together, each fixture as its own program.
    use pw_core::rules;
    use pw_syntax::parse;

    let mut caught = Vec::new();
    let mut total = 0;
    for (name, src, diags) in rejected_results() {
        total += 1;
        let decl_hits = rules::check(&parse(&src).file).iter().any(|f| f.is_error());
        if decl_hits || !diags.is_empty() {
            caught.push(name);
        }
    }

    assert!(
        total >= 44,
        "expected the full rejected corpus, saw {total}"
    );
    assert!(
        caught.len() >= 25,
        "semantic coverage regressed: {}/{total} caught — {caught:?}",
        caught.len()
    );
}

#[test]
fn every_body_level_rejection_matches_its_declared_rule() {
    use pw_core::diagnostics::canonical_code;
    let mut mismatches = Vec::new();
    let mut checked = 0;

    for (path, src, diags) in rejected_results() {
        if diags.is_empty() {
            continue;
        }
        let src = &src;
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

#[test]
fn the_structured_concurrency_rules_run_on_real_bodies() {
    // docs/NEXT.md item 7 for `scope.rs`: the E2A-S graph and its rules were
    // written and tested against hand-built inputs. Nothing is re-implemented
    // here — the bridge builds the graph from HIR and converts what comes back.
    let all = rejected_results();
    let find = |prefix: &str, code: &'static str| {
        let (_, src, diags) = all
            .iter()
            .find(|(p, _, _)| p.starts_with(prefix))
            .unwrap_or_else(|| panic!("{prefix} is in the corpus"));
        let d = diags
            .iter()
            .find(|d| d.code == code)
            .unwrap_or_else(|| panic!("{prefix} must report {code}, got {diags:?}"));
        (src.clone(), d.clone())
    };

    // R-013: `task.spawn(detached)` with no durable capability. The file
    // declares PW0311, which aliases to the invariant's canonical code.
    let (src, d) = find("R-013", "PW2002");
    let src = &src;
    // The primary span is the offending argument, the related span is where the
    // handle came from — charter §16.3 wants both, and they must differ.
    assert_eq!(&src[d.primary_span.clone()], "detached");
    assert!(src[d.related[0].span.clone()].starts_with("task.spawn"));
    assert_ne!(d.primary_span, d.related[0].span);
    assert_ne!(
        d.explanation.as_deref(),
        Some(d.repairs[0].description.as_str()),
        "the note and the help must not be the same sentence"
    );

    // R-039: a subscription declaring a scope that outlives its component.
    let (src, d) = find("R-039", "PW2004");
    let src = &src;
    assert_eq!(&src[d.primary_span.clone()], "scope application");

    // Control: the same subscription scoped to its component is legal, and a
    // `durable.spawn` is the sanctioned way to outlive a scope.
    let ok = "module m\ncomponent C() {\n    let v = observe intersection(self) -> Bool { scope component }\n    view { <p /> }\n}\n";
    assert!(
        check_sources(&[("ok.pw".into(), ok.into())])[0]
            .1
            .is_empty(),
        "a component-scoped observation must be accepted: {:?}",
        check_sources(&[("ok.pw".into(), ok.into())])[0].1
    );
    let plain = "module m\nview V() !{} {\n    task.spawn(work) { g() }\n}\n";
    assert!(
        check_sources(&[("p.pw".into(), plain.into())])[0]
            .1
            .is_empty(),
        "an ordinary scoped spawn must be accepted"
    );
}

#[test]
fn the_privacy_and_placement_rules_catch_their_corpus_cases() {
    // E5's first slice. Each case is a *flow* answered by the algebra in
    // `pw_core::privacy` and `pw_core::placement`, not a syntax pattern.
    let all = rejected_results();
    let want = [
        ("R-002", "PW5002"), // database read inside a browser-placed component
        ("R-003", "PW5003"), // a secret rendered into markup
        ("R-005", "PW5004"), // shared cache keyed without the tenant
        ("R-026", "PW5002"), // a secret capability at the edge
    ];
    for (file, code) in want {
        let (_, src, diags) = all
            .iter()
            .find(|(p, _, _)| p.starts_with(file))
            .unwrap_or_else(|| panic!("{file} is in the corpus"));
        let d = diags
            .iter()
            .find(|d| d.code == code)
            .unwrap_or_else(|| panic!("{file} must report {code}, got {diags:?}"));

        // Charter §14 M5 gate: "Diagnostics name the source value and invalid
        // boundary, not merely a type mismatch."
        assert!(d.primary_span.end <= src.len());
        assert!(!d.related.is_empty(), "{file}: no boundary span");
        let note = d.explanation.as_deref().unwrap_or("");
        assert!(
            note.len() > 40,
            "{file}: the note must explain the cause chain, got {note:?}"
        );
    }

    // Control: the placement solver must not fire on the accepted corpus, where
    // every declaration has somewhere legal to run.
    let bad: Vec<String> = check_sources(&whole_corpus())
        .into_iter()
        .filter(|(p, d)| p.starts_with("A-") && d.iter().any(|d| d.code.starts_with("PW50")))
        .map(|(p, _)| p)
        .collect();
    assert!(bad.is_empty(), "false positives on accepted files: {bad:?}");
}

#[test]
fn an_unmodelled_capability_family_never_manufactures_a_placement_error() {
    // The failure this rule already had once: an effect family absent from the
    // table ruled out every world, so two corpus files were reported for the
    // wrong reason. A coverage number that goes up without a detection is worse
    // than one that stays put.
    let src = "module m\nfn f() -> Int !{ telemetry.emit, log<Public> } {\n    1\n}\n";
    let diags = &check_sources(&[("t.pw".into(), src.into())])[0].1;
    assert!(
        diags.iter().all(|d| d.code != "PW5002"),
        "an unmodelled family must not be unplaceable: {diags:?}"
    );
}
