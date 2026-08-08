//! **How long a check takes, and what an edit costs.**
//!
//! E9 gate item: *"incremental checks are fast enough for editor feedback."*
//!
//! Two numbers, because the gate is about the second and the first is what
//! makes it interesting:
//!
//! ```text
//! cold    the whole program, from source text
//! edit    the whole program again, one file's text changed
//! ```
//!
//! Today there is no query system, so `edit` is `cold` — every check redoes
//! everything. That is the honest baseline the incremental work will be
//! measured against, and recording it BEFORE building anything is the
//! `docs/RISK_QUEUE.md` admissibility rule: an instrument that arrives with the
//! optimization it measures cannot say what the optimization did.
//!
//! # Why this asserts a loose ceiling rather than a target
//!
//! A test that asserts "under 50 ms" on a shared CI machine fails for reasons
//! that have nothing to do with the compiler, and a flaky gate teaches people
//! to re-run it. The ceiling here is an order of magnitude above the measured
//! value: it catches a change that makes checking pathological and says nothing
//! about a 20% regression. The RAW NUMBER is the evidence, and
//! `docs/evidence/E9/check-latency.txt` is where it lives.

use std::time::{Duration, Instant};

/// The store demo as one program: the platform packages, the shared library and
/// the app. 36 files, the same set `just ci` checks.
fn program() -> Vec<(String, String)> {
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    let mut out = Vec::new();
    for dir in [
        "packages/pw-std",
        "packages/pw-platform-web",
        "examples/lib",
        "examples/store",
    ] {
        let mut files: Vec<std::path::PathBuf> = std::fs::read_dir(root.join(dir))
            .unwrap_or_else(|e| panic!("{dir}: {e}"))
            .map(|e| e.expect("entry").path())
            .filter(|p| p.extension().is_some_and(|x| x == "pw"))
            .collect();
        files.sort();
        for p in files {
            out.push((
                p.file_name().unwrap().to_string_lossy().to_string(),
                std::fs::read_to_string(&p).expect("read"),
            ));
        }
    }
    out.push((
        "domain.pw".to_string(),
        std::fs::read_to_string(root.join("examples/domain.pw")).expect("domain.pw"),
    ));
    out
}

/// The median of several runs.
///
/// Median rather than mean: one scheduling hiccup should not decide a
/// measurement, and the first run of anything on a laptop is not the run a
/// developer experiences.
fn median(mut runs: Vec<Duration>) -> Duration {
    runs.sort();
    runs[runs.len() / 2]
}

fn time_check(files: &[(String, String)], runs: usize) -> Duration {
    let mut samples = Vec::new();
    for _ in 0..runs {
        let start = Instant::now();
        let results = pw_core::check::check_sources(files);
        // Consume the result, so nothing above can be optimized away and the
        // measurement is of work that actually happened.
        let n: usize = results.iter().map(|(_, d)| d.len()).sum();
        assert_eq!(n, 0, "the store program checks clean");
        samples.push(start.elapsed());
    }
    median(samples)
}

#[test]
fn the_whole_program_and_one_edit_are_both_measured() {
    let files = program();
    assert!(files.len() >= 30, "only {} files", files.len());

    let cold = time_check(&files, 7);

    // One file's text changed, the way an editor changes it: a comment added
    // to the store app. Nothing semantic moves, so a query system would have
    // almost nothing to redo — which is exactly why this is the number worth
    // recording before there is one.
    let mut edited = files.clone();
    let app = edited
        .iter_mut()
        .find(|(n, _)| n == "app.pw")
        .expect("the store app");
    app.1
        .push_str("\n// an edit that changes nothing semantic\n");
    let after_edit = time_check(&edited, 7);

    println!(
        "check-latency cold  {:>8.2} ms",
        cold.as_secs_f64() * 1000.0
    );
    println!(
        "check-latency edit  {:>8.2} ms   ({} files, no query system: an edit \
         costs a full check)",
        after_edit.as_secs_f64() * 1000.0,
        files.len()
    );

    // The loose ceiling. An order of magnitude above the measured value, so it
    // catches pathological work and stays quiet about noise.
    assert!(
        cold < Duration::from_millis(2000),
        "a full check of {} files took {:?}, which is not editor-feedback \
         territory by any reading",
        files.len(),
        cold
    );

    // **The control.** Without it the numbers above could be a check that
    // returns immediately because it does nothing — which is what a `0 ms`
    // result would actually mean.
    assert!(
        cold > Duration::from_micros(200),
        "a whole-program check in {cold:?} is too fast to have happened"
    );
}

#[test]
fn a_rejected_program_is_not_measurably_slower_than_a_clean_one() {
    // The shape a latency claim usually hides: fast on the happy path, and slow
    // exactly when a developer is iterating on a broken file. Editor feedback
    // is worth most when the program does not compile.
    let files = program();
    let mut broken = files.clone();
    let app = broken
        .iter_mut()
        .find(|(n, _)| n == "app.pw")
        .expect("the store app");
    // A database read inside a view: the charter's opening example, and a
    // diagnostic that costs inference, placement and privacy to produce.
    app.1
        .push_str("\nview Broken() !{ database.read<Stores> } {\n    <p>{Stores.get(1)}</p>\n}\n");

    let mut samples = Vec::new();
    for _ in 0..7 {
        let start = Instant::now();
        let results = pw_core::check::check_sources(&broken);
        let n: usize = results.iter().map(|(_, d)| d.len()).sum();
        assert!(n > 0, "the broken program must actually be rejected");
        samples.push(start.elapsed());
    }
    let rejected = median(samples);
    let clean = time_check(&files, 7);

    println!(
        "check-latency reject{:>8.2} ms   (clean {:.2} ms)",
        rejected.as_secs_f64() * 1000.0,
        clean.as_secs_f64() * 1000.0
    );
    assert!(
        rejected < clean * 5,
        "producing diagnostics took {rejected:?} against {clean:?} clean, which \
         means the slow path is the one a developer is on"
    );
}
