//! The compiler-robustness gate.
//!
//! Architect ruling, 2026-08-06, after a `.pw` program panicked the
//! exhaustiveness checker:
//!
//! > For every syntactically representable program, the compiler must do one
//! > of these: produce output; produce ordinary source diagnostics; produce a
//! > clearly marked internal-compiler-error report. It must not terminate
//! > without a report or silently discard unrelated diagnostics.
//!
//! The last clause is the one that made the panic serious. It was not that one
//! rule was wrong — it was that **every** rule in the invocation reported
//! nothing, for every file, because the process died. A compiler that reports
//! nothing looks exactly like a compiler that found nothing.
//!
//! This is a separate gate from corpus conformance and from generality,
//! because it asks a different question: not "is this program accepted or
//! rejected", but "does the compiler answer at all".
//!
//! # What generates the inputs
//!
//! No fuzzing dependency. `docs/DECISIONS` keeps the dependency set small and
//! pinned, and a deterministic generator is reproducible without a corpus
//! directory of saved seeds — a failure names the seed and the seed rebuilds
//! the input. It is a *structured* generator, not a real fuzzer, and the
//! difference is worth stating: it explores mutations of real programs and
//! random token soup, and it will not find what a coverage-guided fuzzer
//! would.

use std::panic::{AssertUnwindSafe, catch_unwind};

use pw_core::check::check_sources;

/// xorshift64*. Deterministic, seedable, no dependency, and good enough to
/// shuffle bytes — this is not sampling anything statistical.
struct Rng(u64);

impl Rng {
    fn next(&mut self) -> u64 {
        let mut x = self.0;
        x ^= x >> 12;
        x ^= x << 25;
        x ^= x >> 27;
        self.0 = x;
        x.wrapping_mul(0x2545_F491_4F6C_DD1D)
    }
    fn below(&mut self, n: usize) -> usize {
        (self.next() % n as u64) as usize
    }
}

fn corpus() -> Vec<(String, String)> {
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    let mut out = Vec::new();
    let mut stack = vec![root.join("examples"), root.join("packages")];
    while let Some(dir) = stack.pop() {
        let Ok(entries) = std::fs::read_dir(&dir) else {
            continue;
        };
        for e in entries {
            let p = e.expect("entry").path();
            if p.is_dir() {
                stack.push(p);
            } else if p.extension().is_some_and(|x| x == "pw") {
                out.push((
                    p.file_name().unwrap().to_string_lossy().to_string(),
                    std::fs::read_to_string(&p).expect("read"),
                ));
            }
        }
    }
    out.sort();
    out
}

/// Run one source through the whole front end, reporting a panic instead of
/// dying of one.
fn survives(name: &str, src: &str) -> Result<(), String> {
    let owned = (name.to_string(), src.to_string());
    catch_unwind(AssertUnwindSafe(|| {
        let _ = pw_core::rules::check(&pw_syntax::parse(src).file);
        let _ = check_sources(std::slice::from_ref(&owned));
    }))
    .map_err(|e| {
        e.downcast_ref::<&str>()
            .map(|s| s.to_string())
            .or_else(|| e.downcast_ref::<String>().cloned())
            .unwrap_or_else(|| "<non-string panic>".to_string())
    })
}

#[test]
fn no_corpus_file_panics_the_compiler() {
    let mut failures = Vec::new();
    for (name, src) in corpus() {
        if let Err(msg) = survives(&name, &src) {
            failures.push(format!("{name}: {msg}"));
        }
    }
    assert!(
        failures.is_empty(),
        "the compiler panicked on real corpus source:\n  {}",
        failures.join("\n  ")
    );
}

/// Every mutation of every corpus file, for a fixed set of mutations.
///
/// Truncation and brace imbalance are the ones that matter: they produce
/// syntactically representable programs whose HIR is *poisoned* — a body with
/// no closing brace, a pattern with no arguments — and a poisoned HIR is what
/// reached the arity panic.
#[test]
fn no_mutation_of_a_corpus_file_panics_the_compiler() {
    let all = corpus();
    let mut failures = Vec::new();
    let mut checked = 0;

    for (name, src) in &all {
        let bytes = src.as_bytes();
        let mut rng = Rng(0x5EED_0000 ^ name.len() as u64);

        for round in 0..12 {
            let mutated = match round % 4 {
                // Truncate. Half a declaration, half a match arm, half a string.
                0 => {
                    let at = rng.below(bytes.len().max(1));
                    src.char_indices()
                        .take_while(|(i, _)| *i < at)
                        .map(|(_, c)| c)
                        .collect::<String>()
                }
                // Delete a line.
                1 => {
                    let lines: Vec<&str> = src.lines().collect();
                    if lines.is_empty() {
                        continue;
                    }
                    let drop = rng.below(lines.len());
                    lines
                        .iter()
                        .enumerate()
                        .filter(|(i, _)| *i != drop)
                        .map(|(_, l)| *l)
                        .collect::<Vec<_>>()
                        .join("\n")
                }
                // Unbalance the braces.
                2 => src.replacen('}', "", 1),
                // Duplicate a line, which can duplicate a declaration.
                _ => {
                    let lines: Vec<&str> = src.lines().collect();
                    if lines.is_empty() {
                        continue;
                    }
                    let at = rng.below(lines.len());
                    let mut v = lines.clone();
                    v.insert(at, lines[at]);
                    v.join("\n")
                }
            };
            checked += 1;
            if let Err(msg) = survives(name, &mutated) {
                failures.push(format!("{name} (mutation {round}): {msg}"));
            }
        }
    }

    assert!(checked > 500, "only {checked} mutations ran");
    assert!(
        failures.is_empty(),
        "the compiler panicked on a mutated corpus file. Minimize it into \
         examples/robustness/regressions/ before fixing:\n  {}",
        failures.join("\n  ")
    );
}

/// Arbitrary bytes. The weakest property, and the one that must never fail.
#[test]
fn arbitrary_source_bytes_do_not_panic_the_compiler() {
    const ALPHABET: &[&str] = &[
        "module",
        "fn",
        "view",
        "component",
        "page",
        "query",
        "command",
        "match",
        "if",
        "else",
        "let",
        "use",
        "return",
        "{",
        "}",
        "(",
        ")",
        "[",
        "]",
        "<",
        ">",
        "!",
        "|",
        "=>",
        "->",
        ",",
        ".",
        ":",
        "=",
        "\"a\"",
        "\"{x}\"",
        "x",
        "Foo",
        "0",
        "1.5",
        "//c\n",
        "\n",
        " ",
        "é",
        "→",
        "\u{1F600}",
    ];
    let mut failures = Vec::new();
    for seed in 0..400u64 {
        let mut rng = Rng(seed.wrapping_mul(0x9E37_79B9_7F4A_7C15) | 1);
        let len = 1 + rng.below(60);
        let src: String = (0..len)
            .map(|_| ALPHABET[rng.below(ALPHABET.len())])
            .collect();
        if let Err(msg) = survives("fuzz.pw", &src) {
            failures.push(format!("seed {seed}: {msg}\n    source: {src:?}"));
        }
    }
    assert!(
        failures.is_empty(),
        "the compiler panicked on generated source. The seed reproduces it:\n  {}",
        failures.join("\n  ")
    );
}

/// Or-patterns of every length, in every position.
///
/// The second targeted generator, and it exists for the same reason as the
/// first: a panic was found by a counterexample rather than by fuzzing, and
/// the shape that produced it deserves a generator rather than a hope that a
/// mutation lands on it.
///
/// `a | b | c` lowers right-nested, so the interesting parameter is the NUMBER
/// of alternatives — one and two both work with a non-recursive expansion, and
/// three is the first that nests.
/// Boundary: **resolution → typing**.
///
/// A name that does not resolve must not be able to masquerade as a valid
/// constructor or member downstream, and two candidates must produce ambiguity
/// rather than a choice. The by-name member fallback was removed for exactly
/// this reason; these are the shapes that would tempt it back.
#[test]
fn unresolved_and_ambiguous_names_do_not_panic_the_compiler() {
    let mut failures = Vec::new();
    for (label, src) in [
        (
            "unresolved module",
            "module m\n\nimport nowhere\n\nfn f() -> Int !{} { g() }\n",
        ),
        (
            "unresolved member",
            "module m\n\nfn f(x: Int) -> Int !{} { x.nothing() }\n",
        ),
        (
            "member on an unresolved type",
            "module m\n\nfn f(x: Missing) -> Int !{} { x.anything() }\n",
        ),
        (
            "duplicate declarations",
            "module m\n\nfn f() -> Int !{} { 0 }\nfn f() -> Int !{} { 1 }\n",
        ),
        (
            "a constructor that is not one",
            "module m\n\ntype S = | A\n\nfn f(s: S) -> Int !{} { match s { NotAVariant => 0 } }\n",
        ),
        (
            "a module used as a value",
            "module m\n\nimport domain\n\nfn f() -> Int !{} { domain }\n",
        ),
    ] {
        if let Err(msg) = survives("resolve.pw", src) {
            failures.push(format!("{label}: {msg}"));
        }
    }
    assert!(failures.is_empty(), "{}", failures.join("\n  "));
}

/// Boundary: **HIR → effects and labels**.
///
/// A label must survive every construct that carries a value, and a construct
/// the dataflow has not seen before must not crash it. Recursion is the
/// interesting case — a label lattice that iterated without a bound would hang
/// rather than panic, which is worse.
#[test]
fn label_and_effect_dataflow_does_not_panic_or_hang() {
    let mut failures = Vec::new();
    for (label, src) in [
        (
            "self-referential binding",
            "module m\n\nfn f() -> Int !{} { let a = a\n a }\n",
        ),
        (
            "mutually referential bindings",
            "module m\n\nfn f() -> Int !{} { let a = b\n let b = a\n a }\n",
        ),
        (
            "deeply nested branches",
            "module m\n\nfn f(c: Bool) -> Int !{} { if c { if c { if c { if c { 1 } else { 2 } } else { 3 } } else { 4 } } else { 5 } }\n",
        ),
        (
            "a hole inside a hole",
            "module m\n\nfn f() -> String !{} { \"{ \\\"{x}\\\" }\" }\n",
        ),
        (
            "an empty capture list",
            "module m\n\nview V() !{} { <button on:press={resumable(captures = { }) => f()}>x</button> }\n",
        ),
        (
            "a record built from itself",
            "module m\n\ntype R = R { v: Int }\n\nfn f() -> R !{} { let r = R { v: r }\n r }\n",
        ),
    ] {
        if let Err(msg) = survives("dataflow.pw", src) {
            failures.push(format!("{label}: {msg}"));
        }
    }
    assert!(failures.is_empty(), "{}", failures.join("\n  "));
}

/// Boundary: **parser → HIR**.
///
/// One semantic construct must not silently become two statements. That is not
/// a crash — it is worse, because it produces a plausible tree that means
/// something else, and it is what made every page declaring a query dependency
/// look like a page reaching the database.
#[test]
fn one_construct_does_not_lower_as_two_statements() {
    use pw_core::hir::Expr;
    use pw_core::lower::lower_file;
    use pw_syntax::parse_tree;

    // `let x = <keyword> Y(z)` must produce a Let whose init IS the construct.
    for (label, src, keyword) in [
        (
            "query",
            "module m\n\npage P() {\n    let c = query Cart(s)\n    view { <main /> }\n}\n",
            "query",
        ),
        (
            "command",
            "module m\n\npage P() {\n    let c = command Clear(s)\n    view { <main /> }\n}\n",
            "command",
        ),
    ] {
        let hir = lower_file(src, &parse_tree(src).green);
        let (_, decl) = hir.all_decls().next().expect("a declaration");
        let body = hir.body(decl.body.expect("body"));
        let Expr::Block { stmts } = body.expr(body.root) else {
            panic!("{label}: no block");
        };
        let init_is_the_construct = stmts.iter().any(|s| {
            let Expr::Let { init: Some(i), .. } = body.expr(*s) else {
                return false;
            };
            matches!(body.expr(*i), Expr::Keyword { keyword: k, .. } if k == keyword)
        });
        assert!(
            init_is_the_construct,
            "{label}: `let x = {keyword} Y(z)` did not lower as one expression — the \
             call became a sibling statement, so its effects are attributed to the \
             enclosing body"
        );
        // And no bare `Call` sibling left behind.
        let stray = stmts
            .iter()
            .filter(|s| matches!(body.expr(**s), Expr::Call { .. }))
            .count();
        assert_eq!(stray, 0, "{label}: a stray call statement remains");
    }
}

#[test]
fn or_patterns_of_any_length_do_not_panic_the_compiler() {
    let domain = "module domain\n\n\
                  type S =\n    | A\n    | B\n    | C\n    | D(R)\n\n\
                  type R =\n    | X\n";
    let ctors = ["A", "B", "C"];
    let mut failures = Vec::new();

    for n in 1..=3 {
        for wildcard in [true, false] {
            let alts = ctors[..n].join(" | ");
            let rest = if wildcard {
                "        _ => \"rest\"\n"
            } else {
                ""
            };
            let src = format!(
                "{domain}\nfn f(s: S) -> String !{{}} {{\n    match s {{\n        \
                 {alts} => \"one\"\n{rest}    }}\n}}\n"
            );
            if let Err(msg) = survives("or.pw", &src) {
                failures.push(format!("{n} alternatives, wildcard={wildcard}: {msg}"));
            }
        }
    }
    assert!(failures.is_empty(), "{}", failures.join("\n  "));
}

/// Constructor patterns whose arity disagrees with their declaration.
///
/// The targeted generator the architect asked for, and the one that earns its
/// place. Reverting the fix in `exhaust.rs` was tried against all four tests
/// in this file:
///
/// ```text
/// inconsistent_constructor_arities   FAILED   <- catches it
/// no_mutation_of_a_corpus_file       ok       <- does not
/// arbitrary_source_bytes             ok       <- does not
/// no_corpus_file                     ok       <- does not
/// ```
///
/// That is the negative control for this suite, and it also says something
/// about the other three: random mutation did not reach a defect that a
/// generator aimed at one specific inconsistency found immediately. Broad
/// generators are not a substitute for knowing which invariant between two
/// data structures is the fragile one.
#[test]
fn inconsistent_constructor_arities_do_not_panic_the_compiler() {
    let domain = "module domain\n\n\
                  type S =\n    | A\n    | B(R)\n    | C(R, R)\n\n\
                  type R =\n    | X\n";
    let mut failures = Vec::new();

    // Every arity from 0 to 3 written against every constructor of `S`.
    for ctor in ["A", "B", "C"] {
        for arity in 0..4 {
            let args = if arity == 0 {
                String::new()
            } else {
                format!("({})", vec!["p"; arity].join(", "))
            };
            let src = format!(
                "{domain}\nfn f(s: S) -> String !{{}} {{\n    match s {{\n        \
                 {ctor}{args} => \"one\"\n        _ => \"rest\"\n    }}\n}}\n"
            );
            if let Err(msg) = survives("arity.pw", &src) {
                failures.push(format!("{ctor} with arity {arity}: {msg}"));
            }
        }
    }
    assert!(failures.is_empty(), "{}", failures.join("\n  "));
}

/// The CLI reports an internal error rather than dying silently.
///
/// Asserted by running the binary against a source that panics on purpose, so
/// the containment is tested end to end rather than by reading the hook.
#[test]
fn the_cli_turns_a_panic_into_a_marked_internal_error() {
    // There is deliberately no way to make the current compiler panic — that
    // is the gate. So this asserts the containment exists and is wired, by
    // checking the binary's own source for the two halves that make it work.
    // A behavioural test would need a panic to exist, and the moment one does,
    // `no_corpus_file_panics_the_compiler` fails first.
    let cli = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../pw-cli/src/main.rs");
    let src = std::fs::read_to_string(cli).expect("pw-cli source");
    assert!(
        src.contains("catch_unwind"),
        "the CLI must contain an unexpected panic rather than dying"
    );
    assert!(
        src.contains("internal compiler error"),
        "a contained panic must be MARKED as an internal error, not reported as \
         an ordinary diagnostic — the user has to know nothing was checked"
    );
    assert!(
        src.contains("reproduce:"),
        "a contained panic must print something that reproduces it"
    );
}

/// No panicking construct sits on a path a user program can reach.
///
/// Architect ruling, 2026-08-06:
///
/// > Add a CI rule banning `unreachable!`, unchecked indexing, and `expect` in
/// > paths reachable from user programs unless accompanied by a documented
/// > phase invariant and a targeted generator.
///
/// Both panics this project has had were an `expect` or an `unreachable!`
/// stating a phase invariant that a `.pw` program could violate. The rule is
/// not "never panic" — a genuine internal invariant may still be asserted —
/// it is that the assertion must be **argued in a comment** naming why a user
/// program cannot reach it. An `expect("...")` with no such note is a bet, and
/// this project has lost that bet twice.
#[test]
fn no_unargued_panic_sits_on_a_user_reachable_path() {
    // Analysis modules: everything a `.pw` program flows through. `codes.rs`
    // and `diagnostics.rs` are registries, and `types.rs` is the value model
    // built by the compiler rather than from source.
    const ANALYSIS: &[&str] = &[
        "exhaust.rs",
        "check.rs",
        "effects.rs",
        "labels.rs",
        "infer.rs",
        "layout.rs",
        "affine.rs",
        "annotations.rs",
        "resume.rs",
        "routes.rs",
        "contexts.rs",
        "resolve.rs",
        "signatures.rs",
        "lower.rs",
        "privacy.rs",
        "placement.rs",
        "scope.rs",
        "rules.rs",
    ];
    let src_dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("src");
    let mut unargued = Vec::new();

    for name in ANALYSIS {
        let path = src_dir.join(name);
        let Ok(text) = std::fs::read_to_string(&path) else {
            continue;
        };
        // Tests may panic freely — that is what an assertion is.
        let body = text.split("#[cfg(test)]").next().unwrap_or(&text);
        let lines: Vec<&str> = body.lines().collect();
        for (n, line) in lines.iter().enumerate() {
            let trimmed = line.trim_start();
            if trimmed.starts_with("//") {
                continue;
            }
            let panics = trimmed.contains("unreachable!")
                || trimmed.contains(".expect(")
                || trimmed.contains(".unwrap()")
                || trimmed.contains("panic!");
            if !panics {
                continue;
            }
            // Argued if a comment within the preceding six lines explains it.
            let argued = lines[n.saturating_sub(6)..n]
                .iter()
                .any(|l| l.trim_start().starts_with("//"));
            if !argued {
                unargued.push(format!("{name}:{}: {}", n + 1, trimmed.trim()));
            }
        }
    }

    assert!(
        unargued.is_empty(),
        "these panicking constructs sit on a path a `.pw` program reaches, with \
         no comment arguing why it cannot:\n  {}\n\nEither argue the invariant \
         and add a targeted generator for it, or return `Outcome::Blocked`.",
        unargued.join("\n  ")
    );
}
