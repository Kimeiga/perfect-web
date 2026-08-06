//! The tree-emitting grammar against every corpus file.
//!
//! The declaration parser already passes on all 68. This asserts the *new*
//! grammar — which parses bodies into real expression nodes — does too, and
//! stays lossless while doing it.
//!
//! Losslessness is asserted on every file rather than a sample, because a
//! grammar that drops a token would still produce a plausible tree.

use std::path::{Path, PathBuf};

use pw_syntax::grammar::parse_tree;
use pw_syntax::kind::SyntaxKind as K;
use pw_syntax::tree::tree_text;

fn corpus() -> Vec<PathBuf> {
    let root = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("..")
        .join("..")
        .join("examples");
    let mut out = Vec::new();
    for bucket in ["accepted", "rejected"] {
        let dir = root.join(bucket);
        if let Ok(entries) = std::fs::read_dir(&dir) {
            for e in entries.flatten() {
                let p = e.path();
                if p.extension().is_some_and(|x| x == "pw") {
                    out.push(p);
                }
            }
        }
    }
    out.sort();
    out
}

/// Corpus files the tree-emitting grammar cannot yet parse.
///
/// Empty. It is kept — with the two-way ratchet below — because the list is
/// how a gap gets recorded honestly instead of being hidden in a threshold.
const KNOWN_UNPARSED: &[&str] = &[];

#[test]
fn the_grammar_parses_every_corpus_file() {
    let files = corpus();
    assert!(
        files.len() >= 60,
        "expected the full corpus, saw {}",
        files.len()
    );

    let mut unexpected = Vec::new();
    let mut fixed = Vec::new();

    for path in &files {
        let name = path.file_name().unwrap().to_string_lossy().to_string();
        let src = std::fs::read_to_string(path).expect("read");
        let p = parse_tree(&src);
        let known = KNOWN_UNPARSED.contains(&name.as_str());

        if !p.ok() && !known {
            let e = &p.errors[0];
            let snippet: String = src
                .get(e.span.clone())
                .unwrap_or("")
                .chars()
                .take(40)
                .collect();
            unexpected.push(format!("{name}: [{}] {} — {snippet:?}", e.code, e.message));
        }
        // A ratchet only works in both directions: when a known file starts
        // parsing, the list must shrink or it stops meaning anything.
        if p.ok() && known {
            fixed.push(name);
        }
    }

    assert!(
        unexpected.is_empty(),
        "grammar regressed on {} file(s):\n{}",
        unexpected.len(),
        unexpected.join("\n")
    );
    assert!(
        fixed.is_empty(),
        "these now parse — remove them from KNOWN_UNPARSED:\n{fixed:#?}"
    );
}

#[test]
fn the_grammar_covers_most_of_the_corpus() {
    // The headline number, asserted rather than claimed in prose.
    let files = corpus();
    let ok = files
        .iter()
        .filter(|p| parse_tree(&std::fs::read_to_string(p).unwrap()).ok())
        .count();
    assert_eq!(
        ok,
        files.len() - KNOWN_UNPARSED.len(),
        "KNOWN_UNPARSED and the real failure set disagree"
    );
    assert!(
        ok == files.len(),
        "core grammar coverage regressed: {ok}/{} files parse",
        files.len()
    );
}

#[test]
fn the_grammar_is_lossless_on_every_corpus_file() {
    let mut bad = Vec::new();
    for path in corpus() {
        let src = std::fs::read_to_string(&path).expect("read");
        if tree_text(&parse_tree(&src).green) != src {
            bad.push(path.file_name().unwrap().to_string_lossy().to_string());
        }
    }
    assert!(bad.is_empty(), "grammar lost bytes in: {bad:#?}");
}

#[test]
fn bodies_actually_become_expression_nodes() {
    // Guards against a grammar that "succeeds" by swallowing bodies whole. If
    // this number collapses, body parsing has silently regressed to the old
    // token-range capture.
    let mut calls = 0;
    let mut lambdas = 0;
    let mut matches = 0;
    let mut fields = 0;
    for path in corpus() {
        let src = std::fs::read_to_string(&path).expect("read");
        for n in parse_tree(&src).green.descendants() {
            match n.kind() {
                K::CallExpr => calls += 1,
                K::LambdaExpr => lambdas += 1,
                K::MatchExpr => matches += 1,
                K::FieldExpr => fields += 1,
                _ => {}
            }
        }
    }
    // Thresholds are the MEASURED counts minus headroom, not guesses. The first
    // version asserted >50 field accesses and failed at 15 — because a dotted
    // path like `store.name` is consumed as a single `Name` node, not a
    // `FieldExpr` chain. Distinguishing a module path from a field access is a
    // *resolution* question, not a parsing one, so it belongs in HIR.
    assert!(
        calls > 50,
        "expected many calls across the corpus, saw {calls}"
    );
    assert!(
        fields >= 10,
        "expected field accesses after calls, saw {fields}"
    );
    assert!(matches >= 2, "expected match expressions, saw {matches}");
    assert!(lambdas >= 3, "expected lambdas, saw {lambdas}");
}

#[test]
fn the_generic_callback_shape_parses_as_a_lambda_inside_the_call() {
    // R-037's construct is the one the architect called load-bearing: an effect
    // smuggled through a generic callback. If the lambda body were
    // misassociated, an effect checker would inspect the wrong expression and
    // still report something plausible.
    //
    // R-037 the FILE is still in KNOWN_UNPARSED for an unrelated reason (it
    // sits in a view body with template markup), so the construct is asserted
    // directly. When R-037 parses, this should read from the file instead.
    let src = "fn f(items: List<ElementRef>) -> List<Float> !{ layout.measure } {\n    items |> List.map(el => el.getBoundingClientRect().width)\n}\n";
    let p = parse_tree(src);
    assert!(p.ok(), "{:?}", p.errors);

    let lambda = p
        .green
        .descendants()
        .find(|n| n.kind() == K::LambdaExpr)
        .expect("a lambda");
    assert!(
        lambda.text().to_string().contains("getBoundingClientRect"),
        "lambda body: {}",
        lambda.text()
    );
    assert!(
        lambda.ancestors().any(|a| a.kind() == K::ArgList),
        "the lambda must be inside the call's argument list, not a sibling"
    );
    // And the pipeline must not have swallowed it.
    assert!(
        p.green.descendants().any(|n| n.kind() == K::BinaryExpr),
        "the |> pipeline should form a binary expression"
    );
}

#[test]
fn the_corpus_grammar_check_can_fail() {
    // docs/RISK_QUEUE.md negative control.
    assert!(parse_tree("module m\n").ok());
    assert!(!parse_tree("module m\n$$$\n").ok());
}
