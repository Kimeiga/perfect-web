//! Every corpus file lowers to real declarations.
//!
//! E6F moved these from `pw-syntax/tests/corpus_parses.rs`, which asked the
//! **legacy declaration parser** the same questions. That parser is gone: one
//! parser decides what a `.pw` program means, so the properties are asserted
//! about the tree the compiler actually uses.
//!
//! These are the guards against a front end that "succeeds" by recognising
//! nothing. `corpus_grammar.rs` proves the files parse; this proves the parse
//! produced declarations with the contents later passes read. A grammar that
//! swallowed every file into one error node would satisfy the first and fail
//! this.
//!
//! The attribute checks that lived beside them are not ported. `corpus-check`
//! validates `@id`, `@category`, `@rule` and `@expect-error` on every fixture,
//! and the old test's own comment said it existed only until that duplication
//! could go away. Two readers of one fact is the shape this milestone exists
//! to remove.

use pw_core::hir::{DeclKind, Hir};
use pw_core::lower::lower_file;
use pw_syntax::parse_tree;
use std::path::{Path, PathBuf};

fn corpus(bucket: &str) -> Vec<PathBuf> {
    let dir = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../examples")
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

fn each(mut f: impl FnMut(&str, &str, &Hir)) {
    let mut total = 0;
    for bucket in ["accepted", "rejected"] {
        for path in corpus(bucket) {
            total += 1;
            let src = std::fs::read_to_string(&path).expect("read");
            let name = path.file_name().unwrap().to_string_lossy().to_string();
            let hir = lower_file(&src, &parse_tree(&src).green);
            f(&name, &src, &hir);
        }
    }
    assert!(total >= 60, "expected the full corpus, saw {total} files");
}

#[test]
fn every_corpus_file_lowers_a_module() {
    let mut missing = Vec::new();
    each(|name, _, hir| {
        if hir.modules.iter().next().is_none() {
            missing.push(name.to_string());
        }
    });
    assert!(missing.is_empty(), "no module lowered in: {missing:#?}");
}

#[test]
fn every_corpus_file_lowers_a_declaration_that_is_not_an_import() {
    // Guards against a front end that "succeeds" by recognising nothing.
    let mut thin = Vec::new();
    each(|name, _, hir| {
        let substantive = hir
            .all_decls()
            .filter(|(_, d)| d.kind != DeclKind::Import)
            .count();
        if substantive == 0 {
            thin.push(name.to_string());
        }
    });
    assert!(thin.is_empty(), "no real declarations in: {thin:#?}");
}

#[test]
fn effect_rows_survive_lowering() {
    // The corpus writes `!{}` and `!{ database.read<Stores>, trace }`. If rows
    // did not survive, every one would arrive as `None` and the whole effect
    // family — thirteen rules — would be checking nothing while reporting
    // nothing, which is indistinguishable from a clean corpus.
    let (mut with_row, mut empty_rows) = (0, 0);
    each(|_, _, hir| {
        for (_, d) in hir.all_decls() {
            if let Some(row) = &d.declared_effects {
                with_row += 1;
                if row.is_empty() {
                    empty_rows += 1;
                }
            }
        }
    });
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
fn policies_survive_lowering() {
    // The property E6 found broken: a `materialize` block's clauses were
    // parsed as expressions, so `decl.policies` was empty for every
    // materialization in the corpus and every graph edge pointed at nothing.
    //
    // Counted per declaration KIND, because the failure was confined to one:
    // a total policy count would have stayed comfortably large while
    // materializations had none.
    let (mut resource_policies, mut materialize_policies) = (0, 0);
    each(|_, _, hir| {
        for (_, d) in hir.all_decls() {
            match d.kind {
                DeclKind::Materialize => materialize_policies += d.policies.len(),
                DeclKind::Query | DeclKind::Command | DeclKind::Subscription => {
                    resource_policies += d.policies.len()
                }
                _ => {}
            }
        }
    });
    assert!(
        resource_policies >= 40,
        "expected many query/command policies, saw {resource_policies}"
    );
    assert!(
        materialize_policies >= 5,
        "a `materialize` block's clauses are policies, and they were expressions \
         until E6 — saw {materialize_policies}"
    );
}

#[test]
fn the_corpus_declaration_check_can_fail() {
    // docs/RISK_QUEUE.md: a check that cannot fail is not a check.
    let broken = "module m\n";
    let hir = lower_file(broken, &parse_tree(broken).green);
    assert_eq!(
        hir.all_decls()
            .filter(|(_, d)| d.kind != DeclKind::Import)
            .count(),
        0,
        "a file with only a module line has no substantive declaration"
    );
}
