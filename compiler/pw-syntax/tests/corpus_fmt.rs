//! `pw fmt` against the corpus (ADR-0013's formatter-implementation gate).
//!
//! The three properties that make a formatter safe to run on someone's
//! repository — idempotent, semantics-preserving, comment-preserving — are
//! asserted on every program `just ci` holds to `pw fmt --check`, not on a
//! sample: the corpus, its library and rules, the platform's packages, and
//! since ADR-0276 the reference apps, the demo and the generality cases.

use pw_syntax::fmt::{format_source, meaning_kept};
use pw_syntax::lexer::{Kind, lex};
use pw_syntax::parse_tree;

fn corpus() -> Vec<std::path::PathBuf> {
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../examples")
        .canonicalize()
        .expect("examples/ exists");
    // The directories `just ci`'s `pw fmt --check` reads (`test-compile`),
    // and one level below those that hold one directory a case.
    let mut dirs: Vec<std::path::PathBuf> = [
        ".",
        "lib",
        "accepted",
        "rejected",
        "kiokun",
        "feed",
        "store",
        "demo",
        "../packages/pw-std",
        "../packages/pw-platform-web",
    ]
    .iter()
    .map(|d| root.join(d))
    .collect();
    for nested in ["rules", "generality"] {
        for e in std::fs::read_dir(root.join(nested)).expect("a directory of cases") {
            let p = e.expect("entry").path();
            if p.is_dir() {
                dirs.push(p);
            }
        }
    }
    let mut out = Vec::new();
    for dir in dirs {
        for e in std::fs::read_dir(&dir).unwrap_or_else(|e| panic!("{}: {e}", dir.display())) {
            let p = e.expect("entry").path();
            if p.extension().is_some_and(|x| x == "pw") {
                out.push(p);
            }
        }
    }
    out.sort();
    assert!(out.len() > 300, "{} programs", out.len());
    out
}

fn significant(src: &str) -> Vec<String> {
    lex(src)
        .into_iter()
        .filter(|t| !t.kind.is_trivia() && t.kind != Kind::Eof)
        .map(|t| src[t.span.clone()].to_string())
        .collect()
}

fn comments(src: &str) -> Vec<String> {
    lex(src)
        .into_iter()
        .filter(|t| matches!(t.kind, Kind::LineComment | Kind::DocAttr))
        .map(|t| src[t.span.clone()].trim_end().to_string())
        .collect()
}

#[test]
fn formatting_every_corpus_file_is_idempotent() {
    let mut bad = Vec::new();
    for p in corpus() {
        let src = std::fs::read_to_string(&p).expect("read");
        let once = format_source(&src);
        let twice = format_source(&once);
        if once != twice {
            let line = once
                .lines()
                .zip(twice.lines())
                .position(|(a, b)| a != b)
                .unwrap_or(0);
            bad.push(format!(
                "{}: differs first at line {}\n  once: {:?}\n  twice: {:?}",
                p.file_name().unwrap().to_string_lossy(),
                line + 1,
                once.lines().nth(line).unwrap_or(""),
                twice.lines().nth(line).unwrap_or(""),
            ));
        }
    }
    assert!(bad.is_empty(), "fmt(fmt(x)) != fmt(x):\n{}", bad.join("\n"));
}

#[test]
fn formatting_never_changes_the_program() {
    // The property that matters most: every significant token, in order.
    let mut bad = Vec::new();
    for p in corpus() {
        let src = std::fs::read_to_string(&p).expect("read");
        let before = significant(&src);
        let after = significant(&format_source(&src));
        if before != after {
            let at = before
                .iter()
                .zip(after.iter())
                .position(|(a, b)| a != b)
                .unwrap_or(before.len().min(after.len()));
            bad.push(format!(
                "{}: token {at} changed — {:?} -> {:?} ({} -> {} tokens)",
                p.file_name().unwrap().to_string_lossy(),
                before.get(at),
                after.get(at),
                before.len(),
                after.len(),
            ));
        }
    }
    assert!(
        bad.is_empty(),
        "formatting altered the program:\n{}",
        bad.join("\n")
    );
}

/// **Formatting keeps what each program says** (ADR-0276): its tokens, and
/// in each policy's value its gaps as written, which `pw fmt` checks before
/// it writes. The token check above passed the feed's `max_bytes 5_000_000`
/// written `5 _000_000`.
#[test]
fn formatting_keeps_what_every_program_says() {
    let mut bad = Vec::new();
    for p in corpus() {
        let src = std::fs::read_to_string(&p).expect("read");
        if let Err(why) = meaning_kept(&src, &format_source(&src)) {
            bad.push(format!("{}: {why}", p.display()));
        }
    }
    assert!(
        bad.is_empty(),
        "formatting changed what it says:\n{}",
        bad.join("\n")
    );
}

#[test]
fn formatting_never_drops_a_comment() {
    let mut bad = Vec::new();
    for p in corpus() {
        let src = std::fs::read_to_string(&p).expect("read");
        let before = comments(&src);
        let after = comments(&format_source(&src));
        if before != after {
            bad.push(format!(
                "{}: {} comments in, {} out",
                p.file_name().unwrap().to_string_lossy(),
                before.len(),
                after.len()
            ));
        }
    }
    assert!(bad.is_empty(), "comments were lost:\n{}", bad.join("\n"));
}

#[test]
fn formatted_output_still_parses() {
    // A formatter that produced something the parser rejects would be a much
    // worse failure than a layout disagreement, and the two tests above cannot
    // detect it — they read the token stream, not the grammar.
    let mut bad = Vec::new();
    for p in corpus() {
        let src = std::fs::read_to_string(&p).expect("read");
        let out = format_source(&src);
        let reparsed = parse_tree(&out);
        if !reparsed.ok() {
            bad.push(format!(
                "{}: {:?}",
                p.file_name().unwrap().to_string_lossy(),
                reparsed.errors.first()
            ));
        }
    }
    assert!(
        bad.is_empty(),
        "formatted output does not parse:\n{}",
        bad.join("\n")
    );
}

#[test]
fn the_corpus_is_already_canonically_formatted() {
    // ADR-0013's `pw fmt --check` gate. This is the ratchet: once the corpus is
    // canonical, a hand-edit that drifts from the format fails CI.
    let mut drifted = Vec::new();
    for p in corpus() {
        let src = std::fs::read_to_string(&p).expect("read");
        let out = format_source(&src);
        if src != out {
            let line = src
                .lines()
                .zip(out.lines())
                .position(|(a, b)| a != b)
                .unwrap_or(0);
            drifted.push(format!(
                "{}:{}\n  is:     {:?}\n  should: {:?}",
                p.file_name().unwrap().to_string_lossy(),
                line + 1,
                src.lines().nth(line).unwrap_or(""),
                out.lines().nth(line).unwrap_or(""),
            ));
        }
    }
    assert!(
        drifted.is_empty(),
        "{} file(s) are not canonically formatted:\n{}",
        drifted.len(),
        drifted
            .iter()
            .take(6)
            .cloned()
            .collect::<Vec<_>>()
            .join("\n")
    );
}
