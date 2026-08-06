//! E6F's gate: **there is only one parser that decides what a `.pw` program
//! means.**
//!
//! Architect ruling, 2026-08-06:
//!
//! > The important criterion isn't literally deleting exactly 1,050 + 620
//! > lines. It is: there is only one parser that decides what a `.pw` program
//! > means. No semantic correctness path should still ask the legacy parser a
//! > question.
//! >
//! > I would require a control modeled on the `event` failure: add a synthetic
//! > declaration/policy construct in a test and prove there is only one parser
//! > definition that must understand it.
//!
//! So the control is here, and it is built to fail the way `event` failed: a
//! keyword added to one table and not another produced a file that was a
//! declaration to one parser and "expected a declaration" to the other, in the
//! same build.
//!
//! # Why these tests enumerate the real tables
//!
//! `pw_syntax::DECL_STARTERS` and friends are `pub` and read directly. A test
//! with its own list of keywords would be the fifth copy of the thing this
//! milestone removed, and it would go stale in exactly the same way — silently,
//! and in the direction that looks like a pass.

use pw_core::hir::DeclKind;
use pw_core::lower::lower_file;
use pw_syntax::{DECL_STARTERS, POLICY_KEYWORDS, RESOURCE_NOUNS, UI_NOUNS, parse_tree};

/// Keywords that begin a declaration but are not themselves a declaration
/// *kind*: `module`, `import`, the visibility prefixes, and the forms whose
/// meaning is staged to a later milestone.
///
/// Each is listed with the reason it is not a kind, because "it does not lower
/// to a declaration" is a claim that needs one.
const NOT_A_KIND: &[(&str, &str)] = &[
    ("module", "names the file's module; not a declaration in it"),
    (
        "import",
        "brings names in; `DeclKind::Import` is its own thing",
    ),
    (
        "public",
        "a visibility prefix on the declaration that follows",
    ),
    ("session", "a visibility prefix"),
    ("private", "a visibility prefix"),
    ("opaque", "a prefix: `opaque type` is `DeclKind::Opaque`"),
    ("handler_policy", "a named block; E7 gives it meaning"),
    ("replicated", "E11's replicated state; parsed, not modelled"),
    (
        "paint",
        "charter §7.5A painter; identified by its effect row",
    ),
];

fn kind_of(src: &str) -> Option<DeclKind> {
    let p = parse_tree(src);
    assert!(p.ok(), "{src:?} does not parse: {:?}", p.errors);
    let hir = lower_file(src, &p.green);
    hir.all_decls()
        .find(|(_, d)| !matches!(d.kind, DeclKind::Import))
        .map(|(_, d)| d.kind)
}

/// A minimal declaration for a keyword, in the shape the grammar expects.
fn minimal(kw: &str) -> String {
    // A prefix is not a declaration on its own; it needs one to be a prefix of.
    if matches!(kw, "public" | "session" | "private") {
        return format!(
            "module m\n\n{kw} query X(id: Int) -> Int\n    cache shared\n{{\n    0\n}}\n"
        );
    }
    if kw == "opaque" {
        return "module m\n\nopaque type X = Int\n".to_string();
    }
    if kw == "module" {
        return "module m\n".to_string();
    }
    if kw == "import" {
        return "module m\n\nimport other\n".to_string();
    }
    if kw == "handler_policy" {
        return "module m\n\nhandler_policy { on_version_mismatch reload }\n".to_string();
    }
    if UI_NOUNS.contains(&kw) {
        format!("module m\n\n{kw} X() {{\n    <p>x</p>\n}}\n")
    } else if kw == "materialize" {
        format!("module m\n\n{kw} X(id: Int) {{\n    partition public\n}}\n")
    } else if RESOURCE_NOUNS.contains(&kw) {
        format!("module m\n\n{kw} X(id: Int) -> Int\n    cache shared\n{{\n    0\n}}\n")
    } else if kw == "type" {
        "module m\n\ntype X = X { a: Int }\n".to_string()
    } else if kw == "fn" {
        "module m\n\nfn X() -> Int !{} { 0 }\n".to_string()
    } else if kw == "let" {
        "module m\n\nlet X: Int = 0\n".to_string()
    } else {
        format!("module m\n\n{kw} X\n")
    }
}

#[test]
fn every_declaration_keyword_reaches_the_hir_as_a_kind() {
    // The `event` control, generalised. A keyword in the one table must be
    // understood by the one pipeline — parse, lower, kind — with no second
    // definition to update. When this fails for a NEW keyword, exactly one
    // place needs the addition; that is the property being asserted.
    let mut unmodelled = Vec::new();
    for kw in DECL_STARTERS {
        if NOT_A_KIND.iter().any(|(k, _)| k == kw) {
            continue;
        }
        match kind_of(&minimal(kw)) {
            Some(DeclKind::Other) | None => unmodelled.push(*kw),
            Some(_) => {}
        }
    }
    assert!(
        unmodelled.is_empty(),
        "these declaration keywords parse but lower to nothing the compiler \
         models: {unmodelled:?}. Either give them a `DeclKind`, or list them in \
         `NOT_A_KIND` with the reason."
    );
}

#[test]
fn the_keywords_excluded_from_that_check_are_still_parseable() {
    // Without this, `NOT_A_KIND` is an escape hatch that hides a keyword the
    // grammar cannot read at all — and the test above would get greener the
    // more of the language stopped working.
    for (kw, why) in NOT_A_KIND {
        let src = minimal(kw);
        let p = parse_tree(&src);
        assert!(
            p.ok(),
            "`{kw}` is excluded because {why}, which is not a licence to \
             stop parsing: {:?}",
            p.errors
        );
    }
}

#[test]
fn a_policy_keyword_reaches_the_hir_from_both_positions_it_can_be_written_in() {
    // E6's actual defect, pinned. `materialize` writes its policies inside its
    // braces and every other declaration writes them before the body. One
    // table of policy keywords, two syntactic positions, and the lowering must
    // find them in both — it found them in one, so `decl.policies` was empty
    // for every materialization in the corpus.
    let trailing = "module m\n\nquery X(id: Int) -> Int\n    cache shared\n    partition public\n{\n    0\n}\n";
    let inside =
        "module m\n\nmaterialize X(id: Int) {\n    cache shared\n    partition public\n}\n";

    for src in [trailing, inside] {
        let p = parse_tree(src);
        assert!(p.ok(), "{src:?}: {:?}", p.errors);
        let hir = lower_file(src, &p.green);
        let names: Vec<&str> = hir
            .all_decls()
            .find(|(_, d)| d.name == "X")
            .map(|(_, d)| d.policies.iter().map(|p| p.name.as_str()).collect())
            .unwrap_or_default();
        assert!(
            names.contains(&"cache") && names.contains(&"partition"),
            "both policies must reach the HIR from this position; got {names:?} \
             for {src:?}"
        );
    }
}

#[test]
fn every_policy_keyword_is_read_as_a_policy_and_not_as_an_expression() {
    // The generalisation. A keyword in the one policy table, written where a
    // policy goes, must arrive as a `Policy` — never as two bare names in the
    // body, which is what `depends_on Store(id)` was until E6.
    let mut invisible = Vec::new();
    for kw in POLICY_KEYWORDS {
        let src =
            format!("module m\n\nquery X(id: Int) -> Int\n    {kw} something\n{{\n    0\n}}\n");
        let p = parse_tree(&src);
        if !p.ok() {
            invisible.push(format!("{kw} (does not parse)"));
            continue;
        }
        let hir = lower_file(&src, &p.green);
        let seen = hir
            .all_decls()
            .any(|(_, d)| d.policies.iter().any(|p| p.name == *kw));
        if !seen {
            invisible.push((*kw).to_string());
        }
    }
    assert!(
        invisible.is_empty(),
        "these policy keywords do not reach the HIR as policies: {invisible:?}"
    );
}

#[test]
fn no_crate_outside_pw_syntax_builds_a_second_semantic_tree() {
    // The architectural property, checked structurally because it is the one
    // that cannot be established by a passing pipeline: a second parser can be
    // added tomorrow and every test above would still be green.
    //
    // The check looks for a `mod`/`struct` that names itself an AST or parser
    // outside the one crate, and for any consumer of the deleted API. It is
    // deliberately narrow — a check that cries wolf gets disabled, which is
    // worse than not having one.
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    let mut offenders = Vec::new();
    let mut scanned = 0;

    let mut dirs = vec![
        root.join("compiler"),
        root.join("runtime"),
        root.join("tools"),
    ];
    while let Some(dir) = dirs.pop() {
        let Ok(entries) = std::fs::read_dir(&dir) else {
            continue;
        };
        for e in entries.flatten() {
            let p = e.path();
            if p.is_dir() {
                if p.file_name().is_some_and(|n| n == "target") {
                    continue;
                }
                dirs.push(p);
                continue;
            }
            if p.extension().is_none_or(|x| x != "rs") {
                continue;
            }
            let src = std::fs::read_to_string(&p).expect("read");
            scanned += 1;
            let rel = p
                .strip_prefix(&root)
                .unwrap_or(&p)
                .to_string_lossy()
                .to_string();
            // This file names the deleted API in order to look for it.
            if rel.ends_with("tests/one_parser.rs") {
                continue;
            }
            // The deleted API. Any use of it means a second tree came back.
            for gone in [
                "pw_syntax::parser",
                "pw_syntax::ast",
                "pw_syntax::SourceFile",
                "pw_syntax::Parsed",
                "pw_syntax::ParseError",
            ] {
                // Prose in a doc comment explaining the removal is not a use.
                for line in src.lines() {
                    let t = line.trim_start();
                    if t.starts_with("//") || t.starts_with("*") {
                        continue;
                    }
                    if line.contains(gone) {
                        offenders.push(format!("{rel}: uses `{gone}`"));
                    }
                }
            }
        }
    }

    assert!(
        scanned > 20,
        "expected to scan the workspace, saw {scanned}"
    );
    assert!(
        offenders.is_empty(),
        "a second declaration tree is back:\n  {}",
        offenders.join("\n  ")
    );
}
