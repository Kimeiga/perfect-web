//! **Every position a view holds an expression at, read by every analysis
//! that walks a view's expressions** (ADR-0345, the each-head-effects
//! track).
//!
//! An `{#each}`'s list and key were text until 2026-10-15, and no walker of
//! a view's expressions read them: an effect, a secret or a signal's
//! assignment there checked clean. The project's recurring failure is a
//! consumer that names the kinds it knows and drops the rest, so this test
//! is the other half of the fix: [`Position::ALL`] is every position
//! `Node::exprs` reads, and for each, a view with a probe written there,
//! and for each analysis what it must report of the probe:
//!
//! - **effects** (`effects.rs`, the render's row): a database read is PW0401
//!   at every position the view evaluates while it renders, and PW5005 in a
//!   handler, which runs in the browser when a person acts;
//! - **privacy** (`labels.rs`, PW5003): a secret is refused at every
//!   position, a handler's included, whose captures the page holds;
//! - **signals** (`signals.rs`, where an expression runs): a signal's
//!   assignment is PW5300 wherever the view renders, and a body binding's
//!   assigned in a handler is PW5302;
//! - **names** (`names.rs`, `lexical.rs`): a name nothing binds is PW0021
//!   at every position;
//! - **printed** (`check.rs`, PW5040): a session's handle is refused where
//!   it is written as text, and nowhere else, by the rule's own match.
//!
//! Each probe's program and each verdict is an exhaustive `match` over
//! `Position`, so a position added to the language is a compile error here
//! until every analysis's verdict at it is written. The floor: the test
//! fails if `Position::ALL` is empty, if a probe is not at the position it
//! is written for (read back through `Node::exprs`), or if an analysis does
//! not report a probe it must refuse.

use pw_core::check::check_sources;
use pw_core::hir::Position;

fn library() -> Vec<(String, String)> {
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    let mut out = Vec::new();
    for d in ["packages/pw-std", "packages/pw-platform-web"] {
        let mut paths: Vec<std::path::PathBuf> = std::fs::read_dir(root.join(d))
            .unwrap_or_else(|e| panic!("{d}: {e}"))
            .map(|e| e.expect("entry").path())
            .filter(|p| p.extension().is_some_and(|x| x == "pw"))
            .collect();
        paths.sort();
        for p in paths {
            out.push((
                p.display().to_string(),
                std::fs::read_to_string(&p).expect("read"),
            ));
        }
    }
    out
}

/// The module every probe is written in: a database read three ways, a
/// secret, a record holding one, a session's handle, a signal and a body's
/// mutable binding.
const PRELUDE: &str = "module t\n\n\
     import capability.{ Secret, Payments, Session, SessionId }\n\n\
     type Item = Item { name: String }\n\
     type Card = Card { name: String, key: Secret<Payments> }\n\n\
     fn items() -> List<Item> !{ database.read<Item> }\n    host \"shop:data/items#all\"\n\n\
     fn count() -> Int !{ database.read<Item> }\n    host \"shop:data/items#count\"\n\n\
     fn label(i: Item) -> String !{ database.read<Item> }\n    host \"shop:data/items#label\"\n\n\
     view Probe(flag: Bool, rows: List<Item>, key: Secret<Payments>, cards: List<Card>, \
     s: Session<SessionId>, ss: List<Session<SessionId>>) !{} {\n    \
     signal n: Int = 0\n    let mut m = 0\n    ";

const CLOSE: &str = "\n}\n";

/// The markup around a probe at `p`, before and after it. `rows` is the
/// list a key's loop runs over, and `key` what a list's loop is keyed on.
fn around(p: Position, rows: &str, key: &str) -> (String, String) {
    let (before, after) = match p {
        Position::Hole => ("<p>{".to_string(), "}</p>".to_string()),
        Position::Attribute => ("<p title={".to_string(), "}>x</p>".to_string()),
        Position::Handler => (
            "<button type=\"button\" on:press={() => ".to_string(),
            "}>x</button>".to_string(),
        ),
        Position::Subject => ("{#if ".to_string(), "}<p>x</p>{/if}".to_string()),
        Position::Condition => (
            "{#if flag}<p>x</p>{:else if ".to_string(),
            "}<p>y</p>{/if}".to_string(),
        ),
        Position::List => (
            "<ul>{#each ".to_string(),
            format!(" as row ({key})}}<li>x</li>{{/each}}</ul>"),
        ),
        Position::Key => (
            format!("<ul>{{#each {rows} as row ("),
            ")}<li>x</li>{/each}</ul>".to_string(),
        ),
    };
    (before, after)
}

/// What an analysis must report of a probe.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Verdict {
    /// This code, at the probe or at the part that holds it.
    Refused(&'static str),
    /// Not this code at the probe: the position is one the rule decides
    /// against, in its own `match`.
    Accepted(&'static str),
}

struct Analysis {
    name: &'static str,
    probe: fn(Position) -> &'static str,
    verdict: fn(Position) -> Verdict,
    rows: &'static str,
    key: &'static str,
}

fn effects_probe(p: Position) -> &'static str {
    match p {
        Position::Hole | Position::Attribute | Position::Handler => "count()",
        Position::Subject | Position::Condition => "count() > 0",
        Position::List => "items()",
        Position::Key => "row.label",
    }
}

fn effects_verdict(p: Position) -> Verdict {
    match p {
        Position::Hole
        | Position::Attribute
        | Position::Subject
        | Position::Condition
        | Position::List
        | Position::Key => Verdict::Refused("PW0401"),
        Position::Handler => Verdict::Refused("PW5005"),
    }
}

fn privacy_probe(p: Position) -> &'static str {
    match p {
        Position::Hole | Position::Attribute | Position::Handler => "key",
        Position::Subject | Position::Condition => "key == key",
        Position::List => "[key]",
        Position::Key => "row.key",
    }
}

fn privacy_verdict(p: Position) -> Verdict {
    match p {
        Position::Hole
        | Position::Attribute
        | Position::Handler
        | Position::Subject
        | Position::Condition
        | Position::List
        | Position::Key => Verdict::Refused("PW5003"),
    }
}

fn signals_probe(p: Position) -> &'static str {
    match p {
        Position::Hole
        | Position::Attribute
        | Position::Subject
        | Position::Condition
        | Position::Key => "n = 1",
        Position::List => "[n = 1]",
        Position::Handler => "m = 1",
    }
}

fn signals_verdict(p: Position) -> Verdict {
    match p {
        Position::Hole
        | Position::Attribute
        | Position::Subject
        | Position::Condition
        | Position::List
        | Position::Key => Verdict::Refused("PW5300"),
        Position::Handler => Verdict::Refused("PW5302"),
    }
}

fn names_probe(p: Position) -> &'static str {
    match p {
        Position::Hole
        | Position::Attribute
        | Position::Handler
        | Position::Subject
        | Position::Condition
        | Position::List
        | Position::Key => "nowhere",
    }
}

fn names_verdict(p: Position) -> Verdict {
    match p {
        Position::Hole
        | Position::Attribute
        | Position::Handler
        | Position::Subject
        | Position::Condition
        | Position::List
        | Position::Key => Verdict::Refused("PW0021"),
    }
}

fn printed_probe(p: Position) -> &'static str {
    match p {
        Position::Hole | Position::Attribute | Position::Handler => "s",
        Position::Subject | Position::Condition => "s == s",
        Position::List => "ss",
        Position::Key => "row",
    }
}

fn printed_verdict(p: Position) -> Verdict {
    match p {
        Position::Hole | Position::Attribute => Verdict::Refused("PW5040"),
        // Run when a person acts; what a block decides by; the list the
        // rows come from; and the key, which the document holds as a keyed
        // digest, never as itself.
        Position::Handler
        | Position::Subject
        | Position::Condition
        | Position::List
        | Position::Key => Verdict::Accepted("PW5040"),
    }
}

const ANALYSES: &[Analysis] = &[
    Analysis {
        name: "effects",
        probe: effects_probe,
        verdict: effects_verdict,
        rows: "rows",
        key: "row.name",
    },
    Analysis {
        name: "privacy",
        probe: privacy_probe,
        verdict: privacy_verdict,
        rows: "cards",
        key: "row",
    },
    Analysis {
        name: "signals",
        probe: signals_probe,
        verdict: signals_verdict,
        rows: "rows",
        key: "row",
    },
    Analysis {
        name: "names",
        probe: names_probe,
        verdict: names_verdict,
        rows: "rows",
        key: "row",
    },
    Analysis {
        name: "printed",
        probe: printed_probe,
        verdict: printed_verdict,
        rows: "ss",
        key: "row",
    },
];

/// The program with `probe` at `p`, and where the probe is in it.
fn written(a: &Analysis, p: Position) -> (String, std::ops::Range<usize>) {
    let probe = (a.probe)(p);
    let (before, after) = around(p, a.rows, a.key);
    let start = PRELUDE.len() + before.len();
    let src = format!("{PRELUDE}{before}{probe}{after}{CLOSE}");
    (src, start..start + probe.len())
}

/// The positions an expression covering `at` is held at, as `Node::exprs`
/// reads the program's markup.
fn held_at(src: &str, at: &std::ops::Range<usize>) -> Vec<Position> {
    let hir = pw_core::lower::lower_file(src, &pw_syntax::parse_tree(src).green);
    let mut out = Vec::new();
    for (_, body, _) in hir.bodies.iter() {
        for (_, node, _) in body.nodes.iter() {
            for (position, e) in node.exprs() {
                let span = body.expr_span(e);
                if span.start <= at.start && at.end <= span.end {
                    out.push(position);
                }
            }
        }
    }
    out
}

fn overlaps(a: &std::ops::Range<usize>, b: &std::ops::Range<usize>) -> bool {
    a.start < b.end && b.start < a.end
}

#[test]
fn every_analysis_reads_every_position() {
    // The floor: no position, or no analysis, and nothing would be checked.
    assert!(
        Position::ALL.len() >= 7,
        "the positions a view holds an expression at: {:?}",
        Position::ALL
    );
    assert!(!ANALYSES.is_empty());
    let library = library();
    let mut checked = 0;
    let mut failures = Vec::new();
    for a in ANALYSES {
        let mut refused_somewhere = false;
        for &p in Position::ALL {
            let (src, at) = written(a, p);
            // The probe is where it is written for.
            let held = held_at(&src, &at);
            if !held.contains(&p) {
                failures.push(format!(
                    "{} at {p:?}: the probe `{}` is held at {held:?}",
                    a.name,
                    &src[at.clone()]
                ));
                continue;
            }
            let mut program = library.clone();
            program.push(("t.pw".to_string(), src.clone()));
            let found: Vec<(String, std::ops::Range<usize>)> = check_sources(&program)
                .into_iter()
                .filter(|(n, _)| n == "t.pw")
                .flat_map(|(_, ds)| ds.into_iter().map(|d| (d.code.to_string(), d.primary_span)))
                .collect();
            let at_probe = |code: &str| found.iter().any(|(c, s)| c == code && overlaps(s, &at));
            match (a.verdict)(p) {
                Verdict::Refused(code) => {
                    refused_somewhere = true;
                    if !at_probe(code) {
                        failures.push(format!(
                            "{} at {p:?}: expected {code} at `{}`, found {found:?}",
                            a.name,
                            &src[at.clone()]
                        ));
                    }
                }
                Verdict::Accepted(code) => {
                    if at_probe(code) {
                        failures.push(format!(
                            "{} at {p:?}: {code} at `{}`, which the rule does not read there",
                            a.name,
                            &src[at.clone()]
                        ));
                    }
                }
            }
            checked += 1;
        }
        assert!(refused_somewhere, "{}: refuses nothing anywhere", a.name);
    }
    assert_eq!(checked, ANALYSES.len() * Position::ALL.len());
    assert!(failures.is_empty(), "{}", failures.join("\n"));
}

/// **`Node::exprs` reads each position once**: a view writing an expression
/// at every position, each found at its own.
#[test]
fn node_exprs_finds_each_position_where_it_is_written() {
    let markup = "<p title={flag}>{flag}</p>\n    \
                  <button type=\"button\" on:press={() => flag}>x</button>\n    \
                  {#if flag}<p>a</p>{:else if flag}<p>b</p>{/if}\n    \
                  <ul>{#each rows as row (row.name)}<li>x</li>{/each}</ul>";
    let src = format!("{PRELUDE}{markup}{CLOSE}");
    let hir = pw_core::lower::lower_file(&src, &pw_syntax::parse_tree(&src).green);
    let mut seen: Vec<(Position, String)> = Vec::new();
    for (_, body, _) in hir.bodies.iter() {
        for (_, node, _) in body.nodes.iter() {
            for (position, e) in node.exprs() {
                seen.push((position, src[body.expr_span(e)].to_string()));
            }
        }
    }
    seen.sort();
    let expected: Vec<(Position, String)> = vec![
        (Position::Hole, "flag".to_string()),
        (Position::Attribute, "flag".to_string()),
        (Position::Handler, "() => flag".to_string()),
        (Position::Subject, "flag".to_string()),
        (Position::Condition, "flag".to_string()),
        (Position::List, "rows".to_string()),
        (Position::Key, "row.name".to_string()),
    ];
    assert_eq!(seen, expected);
    // And every position is one of them: none is read by nothing.
    for p in Position::ALL {
        assert!(seen.iter().any(|(q, _)| q == p), "{p:?} is written nowhere");
    }
}
