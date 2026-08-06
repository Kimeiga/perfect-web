//! E2-c: parse every corpus file.
//!
//! This is the first test in the project that runs the compiler over the
//! *specification*. Until now the 68 corpus files were documents that
//! `corpus-check` validated the shape of; nothing read their contents.
//!
//! Scope note, stated up front so the result is not over-read: this asserts the
//! files **parse**, not that they type-check, and certainly not that the rejected
//! ones are rejected *for the right reason*. Rejected files are rejected by
//! later passes (`pw-core`), which E2 has not yet wired to the parser. What this
//! does prove is that the grammar covers the language the corpus is written in —
//! and it has already found real defects in both.

use std::path::{Path, PathBuf};

use pw_syntax::ast::DeclKind;
use pw_syntax::parser::parse;

fn corpus(bucket: &str) -> Vec<PathBuf> {
    let dir = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("..")
        .join("..")
        .join("examples")
        .join(bucket);
    let mut out: Vec<PathBuf> = std::fs::read_dir(&dir)
        .unwrap_or_else(|e| panic!("read {}: {e}", dir.display()))
        .flatten()
        .map(|e| e.path())
        .filter(|p| p.extension().is_some_and(|x| x == "pw"))
        .collect();
    out.sort();
    out
}

#[test]
fn every_corpus_file_parses_without_error() {
    let mut failures = Vec::new();
    let mut total = 0;

    for bucket in ["accepted", "rejected"] {
        for path in corpus(bucket) {
            total += 1;
            let src = std::fs::read_to_string(&path).expect("read");
            let p = parse(&src);
            if !p.ok() {
                let name = path.file_name().unwrap().to_string_lossy().to_string();
                for e in p.errors.iter().take(3) {
                    let snippet = src
                        .get(e.span.clone())
                        .unwrap_or("<bad span>")
                        .chars()
                        .take(40)
                        .collect::<String>();
                    failures.push(format!(
                        "{name}: [{}] {} at {:?} — {snippet:?}",
                        e.code, e.message, e.span
                    ));
                }
            }
        }
    }

    assert!(total >= 60, "expected the full corpus, saw {total} files");
    assert!(
        failures.is_empty(),
        "{} parse failures across {total} files:\n{}",
        failures.len(),
        failures.join("\n")
    );
}

#[test]
fn every_corpus_file_declares_a_module() {
    // A file with no `module` line means the parser silently skipped the
    // declaration, which would make the test above pass for the wrong reason.
    let mut missing = Vec::new();
    for bucket in ["accepted", "rejected"] {
        for path in corpus(bucket) {
            let src = std::fs::read_to_string(&path).expect("read");
            if parse(&src).file.module.is_none() {
                missing.push(path.file_name().unwrap().to_string_lossy().to_string());
            }
        }
    }
    assert!(
        missing.is_empty(),
        "no module declaration parsed in: {missing:#?}"
    );
}

#[test]
fn the_parser_actually_builds_declarations_not_just_empty_files() {
    // Guards against a parser that "succeeds" by recognising nothing.
    let mut thin = Vec::new();
    for bucket in ["accepted", "rejected"] {
        for path in corpus(bucket) {
            let src = std::fs::read_to_string(&path).expect("read");
            let p = parse(&src);
            let substantive = p
                .file
                .decls
                .iter()
                .filter(|d| !matches!(d.kind, DeclKind::Module | DeclKind::Import))
                .count();
            if substantive == 0 {
                thin.push(path.file_name().unwrap().to_string_lossy().to_string());
            }
        }
    }
    assert!(thin.is_empty(), "parsed no real declarations in: {thin:#?}");
}

#[test]
fn corpus_attributes_are_readable_from_the_parse() {
    // `corpus-check` reads these with its own ad-hoc scanner. Once the parser
    // exposes them, that duplication can go away.
    for bucket in ["accepted", "rejected"] {
        for path in corpus(bucket) {
            let src = std::fs::read_to_string(&path).expect("read");
            let f = parse(&src).file;
            let name = path.file_name().unwrap().to_string_lossy();
            assert_eq!(f.attr("corpus"), Some(bucket), "@corpus mismatch in {name}");
            assert!(f.attr("id").is_some(), "missing @id in {name}");
            assert!(f.attr("category").is_some(), "missing @category in {name}");
            if bucket == "rejected" {
                assert!(f.attr("rule").is_some(), "missing @rule in {name}");
                assert!(
                    f.attrs_named("expect-error").next().is_some(),
                    "missing @expect-error in {name}"
                );
            }
        }
    }
}

#[test]
fn effect_rows_are_recovered_from_the_corpus() {
    // The corpus writes `!{}` and `!{ database.read<Stores>, trace }`. If the
    // parser did not understand effect rows, every one would come back as None
    // and E1's lowering would have nothing to work with.
    let mut with_row = 0;
    let mut empty_rows = 0;
    for bucket in ["accepted", "rejected"] {
        for path in corpus(bucket) {
            let src = std::fs::read_to_string(&path).expect("read");
            for d in parse(&src).file.decls {
                let row = match &d.kind {
                    DeclKind::Function { effects, .. } => effects.clone(),
                    DeclKind::Ui { effects, .. } => effects.clone(),
                    _ => None,
                };
                if let Some(r) = row {
                    with_row += 1;
                    if r.effects.is_empty() {
                        empty_rows += 1;
                    }
                }
            }
        }
    }
    assert!(
        with_row >= 10,
        "expected many effect rows in the corpus, saw {with_row}"
    );
    assert!(
        empty_rows >= 3,
        "expected several explicit `!{{}}` purity claims, saw {empty_rows}"
    );
}

#[test]
fn the_corpus_parse_check_can_fail() {
    // docs/RISK_QUEUE.md: a check that cannot fail is not a check.
    let broken = "module m\n\n$$$ not a declaration $$$\n";
    assert!(!parse(broken).ok(), "malformed input must be rejected");
    assert!(
        parse("module m\n").ok(),
        "well-formed input must be accepted"
    );
}
