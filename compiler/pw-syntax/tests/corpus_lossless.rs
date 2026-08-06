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

#[test]
fn no_corpus_file_produces_unknown_tokens() {
    // An `Unknown` token means the lexer met a byte it has no rule for. In a
    // *specification* corpus that is a signal the grammar is missing something,
    // so it is worth failing on rather than tolerating.
    let mut offenders = Vec::new();
    for path in corpus_files() {
        let src = std::fs::read_to_string(&path).expect("read corpus file");
        for t in lex(&src) {
            if t.kind == Kind::Unknown {
                offenders.push(format!(
                    "{}: {:?} at {}..{}",
                    path.file_name().unwrap().to_string_lossy(),
                    &src[t.span.clone()],
                    t.span.start,
                    t.span.end
                ));
            }
        }
    }
    assert!(
        offenders.is_empty(),
        "unknown tokens:\n{}",
        offenders.join("\n")
    );
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
