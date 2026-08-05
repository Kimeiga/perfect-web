//! spike: compiler-diagnostic
//!
//! Charter §14 Milestone 0 task 11:
//!   "build a tiny Rust CLI that parses one toy declaration and emits a
//!    source-span diagnostic. This validates the development ergonomics before
//!    choosing parser libraries."
//!
//! Usage:
//!   pw-spike-diag <file.pw>        parse + check one file (styled output)
//!   pw-spike-diag --plain <file>   ANSI-free output for snapshot tests
//!   pw-spike-diag --demo           run the built-in demo corpus
//!
//! Exit status: 0 = no diagnostics, 1 = at least one error, 2 = usage error.

mod check;
mod diag;
mod lexer;
mod parser;

use std::process::ExitCode;

/// The built-in demo corpus. Each entry is (name, source). Kept in the binary so
/// the spike is runnable with no arguments and no fixture files.
const DEMO: &[(&str, &str)] = &[
    (
        "accepted/public-store.pw",
        "public query store(id: StoreId)\n    freshness 30.seconds\n    consistency snapshot\n    cache shared\n",
    ),
    (
        "accepted/session-cart.pw",
        "session query cart(id: SessionId)\n    freshness 0.seconds\n    consistency read_your_writes\n    cache private\n",
    ),
    (
        "rejected/session-cart-in-shared-cache.pw",
        "session query cart(id: SessionId)\n    freshness 0.seconds\n    consistency read_your_writes\n    cache shared\n",
    ),
    (
        "rejected/public-read-your-writes.pw",
        "public query store(id: StoreId)\n    consistency read_your_writes\n    cache shared\n",
    ),
    (
        "rejected/stale-session-query.pw",
        "session query cart(id: SessionId)\n    freshness 30.seconds\n    cache private\n",
    ),
    (
        "rejected/two-independent-typos.pw",
        "public query store(id: StoreId)\n    consistency evantual\n    cache sharedd\n",
    ),
    (
        "rejected/missing-paren.pw",
        "public query store(id: StoreId\n    cache shared\n",
    ),
];

/// Returns (errors, warnings).
fn run_one(path: &str, source: &str, styled: bool, explain: bool) -> (usize, usize) {
    let outcome = parser::parse(source);
    let mut diagnostics = outcome.diagnostics;
    // Finding F-4 (see README): semantic checks must not run on a declaration
    // that failed to parse. Recovery fills in plausible-looking holes, and
    // checking those holes produces diagnostics that describe the recovery
    // rather than the user's code. rustc suppresses derived errors the same way.
    if let (Some(decl), true) = (&outcome.decl, diagnostics.is_empty()) {
        diagnostics.extend(check::check(decl));
    }
    if !diagnostics.is_empty() {
        print!("{}", diag::render(source, path, &diagnostics, styled));
    }
    if explain && let Some(decl) = &outcome.decl {
        println!("{}", check::explain(decl, source));
    }
    let errors = diagnostics.iter().filter(|d| d.is_error()).count();
    (errors, diagnostics.len() - errors)
}

fn main() -> ExitCode {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let plain = args.iter().any(|a| a == "--plain");
    let demo = args.iter().any(|a| a == "--demo");
    let explain = args.iter().any(|a| a == "--explain");
    let files: Vec<&String> = args.iter().filter(|a| !a.starts_with("--")).collect();

    if demo || files.is_empty() {
        let (mut errors, mut warnings, mut clean) = (0usize, 0usize, 0usize);
        for (name, src) in DEMO {
            println!(
                "── {name} {}",
                "─".repeat(60usize.saturating_sub(name.len()))
            );
            let (e, w) = run_one(name, src, !plain, explain);
            if e + w == 0 {
                println!("(accepted — no diagnostics)\n");
                clean += 1;
            }
            errors += e;
            warnings += w;
        }
        println!(
            "demo: {} sources, {clean} clean, {errors} errors, {warnings} warnings",
            DEMO.len()
        );
        // The demo intentionally contains rejected sources, so a nonzero error
        // count is the expected outcome, not a failure of the spike.
        return ExitCode::SUCCESS;
    }

    let mut errors = 0usize;
    for f in files {
        match std::fs::read_to_string(f) {
            Ok(src) => errors += run_one(f, &src, !plain, explain).0,
            Err(e) => {
                eprintln!("pw-spike-diag: cannot read {f}: {e}");
                return ExitCode::from(2);
            }
        }
    }
    if errors > 0 {
        ExitCode::from(1)
    } else {
        ExitCode::SUCCESS
    }
}
