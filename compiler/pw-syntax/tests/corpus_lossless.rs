//! The losslessness property, checked against every real corpus file.
//!
//! Fixture-based round-trip tests only prove the lexer handles what I thought to
//! write down. The corpus is 68 files written before the lexer existed, full of
//! §, em-dashes, `30.seconds`, nested generics and attribute comments — i.e.
//! exactly the input a hand-written lexer gets wrong.
//!
//! If this test cannot fail, it is not a test (`docs/RISK_QUEUE.md`), so
//! `the_property_can_fail` proves a deliberately lossy lexer would be caught.

use std::path::{Path, PathBuf};

use pw_syntax::lexer::{Kind, lex, reconstruct};

fn corpus_files() -> Vec<PathBuf> {
    let root = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("..")
        .join("..")
        .join("examples");
    let mut out = Vec::new();
    for bucket in ["accepted", "rejected"] {
        let dir = root.join(bucket);
        let Ok(entries) = std::fs::read_dir(&dir) else {
            continue;
        };
        for e in entries.flatten() {
            let p = e.path();
            if p.extension().is_some_and(|x| x == "pw") {
                out.push(p);
            }
        }
    }
    out.sort();
    out
}

#[test]
fn every_corpus_file_round_trips_byte_for_byte() {
    let files = corpus_files();
    assert!(
        files.len() >= 60,
        "expected the full corpus, found {} files — is the path wrong?",
        files.len()
    );

    let mut failures = Vec::new();
    for path in &files {
        let src = std::fs::read_to_string(path).expect("read corpus file");
        let toks = lex(&src);
        if reconstruct(&src, &toks) != src {
            failures.push(path.display().to_string());
        }
    }
    assert!(
        failures.is_empty(),
        "these files did not round-trip: {failures:#?}"
    );
}

#[test]
fn every_corpus_file_has_gapless_contiguous_spans() {
    for path in corpus_files() {
        let src = std::fs::read_to_string(&path).expect("read corpus file");
        let mut cursor = 0usize;
        for t in lex(&src).iter().filter(|t| t.kind != Kind::Eof) {
            assert_eq!(
                t.span.start,
                cursor,
                "gap or overlap at byte {cursor} in {}",
                path.display()
            );
            // Slicing must never panic: spans have to land on char boundaries.
            let _ = &src[t.span.clone()];
            cursor = t.span.end;
        }
        assert_eq!(
            cursor,
            src.len(),
            "trailing bytes unlexed in {}",
            path.display()
        );
    }
}

/// The `Unknown` tokens in the tree the compiler reads: a character the
/// grammar has no rule for, where it is.
fn unknown_in(src: &str) -> Vec<(String, std::ops::Range<usize>)> {
    pw_syntax::parse_tree(src)
        .green
        .descendants_with_tokens()
        .filter_map(|e| e.into_token())
        .filter(|t| t.kind() == pw_syntax::SyntaxKind::Unknown)
        .map(|t| {
            let range = t.text_range();
            (
                t.text().to_string(),
                usize::from(range.start())..usize::from(range.end()),
            )
        })
        .collect()
}

#[test]
fn no_corpus_file_produces_unknown_tokens() {
    // An `Unknown` token means the grammar met a character it has no rule
    // for. In a *specification* corpus that is a signal the grammar is missing
    // something, so it is worth failing on rather than tolerating.
    //
    // Asked of the tree the compiler reads, not of the lexer's tokens. Since
    // ADR-0167 the parser reads markup's text again, where any character is
    // text: `·`, `—` or an emoji as much as a letter. The lexer does not know
    // markup and has no rule for them, and until corpus C16 no fixture wrote
    // one between tags (A-032's "1 reply · 2 likes", ADR-0226).
    let mut offenders = Vec::new();
    for path in corpus_files() {
        let src = std::fs::read_to_string(&path).expect("read corpus file");
        for (text, span) in unknown_in(&src) {
            offenders.push(format!(
                "{}: {text:?} at {}..{}",
                path.file_name().unwrap().to_string_lossy(),
                span.start,
                span.end
            ));
        }
    }
    assert!(
        offenders.is_empty(),
        "unknown tokens:\n{}",
        offenders.join("\n")
    );
}

/// The check above can fail: a character with no rule is `Unknown` in code,
/// and text between tags.
#[test]
fn a_character_with_no_rule_is_unknown_in_code_and_text_in_markup() {
    let code = "module t\n\nfn f() -> Int !{} {\n    1 · 2\n}\n";
    assert_eq!(unknown_in(code), [("·".to_string(), 36..38)]);
    let markup = "module t\n\nview V() !{} {\n    <p>1 reply · 2 likes, 🎉</p>\n}\n";
    assert_eq!(unknown_in(markup), []);
    // The lexer alone, which does not know markup, has no rule for either.
    assert!(lex(markup).iter().any(|t| t.kind == Kind::Unknown));
}

#[test]
fn attribute_comments_survive_as_their_own_token_kind() {
    // `pw fmt` must not reflow the `// @corpus:` header, and `corpus-check`
    // reads it. Both depend on it being lexed distinctly.
    let mut seen = 0;
    for path in corpus_files() {
        let src = std::fs::read_to_string(&path).expect("read corpus file");
        seen += lex(&src).iter().filter(|t| t.kind == Kind::DocAttr).count();
    }
    assert!(
        seen > 200,
        "expected many @-attributes across the corpus, saw {seen}"
    );
}

#[test]
fn the_property_can_fail() {
    // docs/RISK_QUEUE.md: a check that cannot fail is not a check. A lexer that
    // silently drops trivia would pass every other assertion in this file.
    let src = "fn f() { /* nothing */ }\n// a comment\n";
    let lossy: String = lex(src)
        .iter()
        .filter(|t| t.kind != Kind::Eof && !t.kind.is_trivia())
        .map(|t| t.text(src))
        .collect();
    assert_ne!(lossy, src, "dropping trivia must be detectable");
    assert_eq!(
        reconstruct(src, &lex(src)),
        src,
        "but the real lexer keeps it"
    );
}

// ---------------------------------------------------------------------------
// ADR-0012: the same properties, now asserted against the Rowan green tree.
//
// The migration is only real if the tree carries the invariants the hand-rolled
// representation established. Keeping both sets means a divergence between them
// is itself detectable.
// ---------------------------------------------------------------------------

use pw_syntax::kind::SyntaxKind;
use pw_syntax::tree::{flat_tree, tree_text};

#[test]
fn every_corpus_file_round_trips_through_the_green_tree() {
    let files = corpus_files();
    assert!(
        files.len() >= 60,
        "expected the full corpus, found {}",
        files.len()
    );

    let mut failures = Vec::new();
    for path in &files {
        let src = std::fs::read_to_string(path).expect("read corpus file");
        if tree_text(&flat_tree(&src)) != src {
            failures.push(path.display().to_string());
        }
    }
    assert!(
        failures.is_empty(),
        "green tree lost bytes in: {failures:#?}"
    );
}

#[test]
fn the_green_tree_and_the_token_stream_never_disagree() {
    // Two independent reconstructions. A divergence means the token->tree
    // mapping dropped or duplicated something, which no single-source check
    // could see.
    for path in corpus_files() {
        let src = std::fs::read_to_string(&path).expect("read");
        assert_eq!(
            reconstruct(&src, &lex(&src)),
            tree_text(&flat_tree(&src)),
            "token stream and green tree disagree in {}",
            path.display()
        );
    }
}

#[test]
fn green_tree_token_ranges_index_the_real_source() {
    // A range that does not match its own text means spans handed to
    // diagnostics would point at the wrong place.
    for path in corpus_files() {
        let src = std::fs::read_to_string(&path).expect("read");
        let tree = flat_tree(&src);
        for t in tree.children_with_tokens().filter_map(|e| e.into_token()) {
            let r = t.text_range();
            let (start, end) = (usize::from(r.start()), usize::from(r.end()));
            assert_eq!(
                &src[start..end],
                t.text(),
                "range/text mismatch at {start}..{end} in {}",
                path.display()
            );
        }
    }
}

#[test]
fn attribute_comments_survive_into_the_green_tree() {
    let mut seen = 0;
    for path in corpus_files() {
        let src = std::fs::read_to_string(&path).expect("read");
        seen += flat_tree(&src)
            .children_with_tokens()
            .filter_map(|e| e.into_token())
            .filter(|t| t.kind() == SyntaxKind::DocAttr)
            .count();
    }
    assert!(
        seen > 200,
        "expected many @-attributes in the tree, saw {seen}"
    );
}
