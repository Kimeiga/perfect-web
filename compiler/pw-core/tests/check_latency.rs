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
//! to re-run it. The ceiling here catches a change that makes checking
//! pathological and says nothing about a 20% regression. The RAW NUMBER is the
//! evidence, and `docs/evidence/E9/check-latency.txt` is where it lives.
//!
//! **And so the number grew unremarked** (found 2026-10-13). The ceiling was
//! fifty times the 37 ms measured for 36 files on 2026-08-08; the measurement
//! reached about 850 ms for 52 files, within two and a half times it, as every
//! analysis added since resolves and types on its own and nothing is shared.
//! Nothing read the number again: the recipe that records it runs on the
//! development machine alone. The two tests here measure one at a time, as
//! measuring both at once measured their contention, which failed train-5's
//! local `just ci` at 3.4 s.
//!
//! **And so the ceiling measured the machine** (track evidence-integrity,
//! finding 4, 2026-10-15). On the PC a dozen workers share, train-7's local
//! `just ci` and four workers' failed here at 2.77 to 3.7 s, at load averages
//! of 40 to 110, while GitHub's quiet runners passed: a wall clock counts the
//! time the checking thread waits for a CPU, which is the machine's load and
//! not the compiler. The ceiling and the ratio below now bound the checking
//! thread's CPU time (`clock_gettime(2)`'s `CLOCK_THREAD_CPUTIME_ID`): the
//! checker runs on the thread that calls it, and nothing else of the binary
//! runs while it is measured. The wall clock is still measured and printed,
//! for the E9 evidence. CPU time is not the work itself: a thread on a busy
//! core, or on one of a hybrid processor's slower cores, takes more of it for
//! the same work, so the ceiling stays loose.

use std::time::{Duration, Instant};

/// **What the checking thread has used of a CPU**: `clock_gettime(2)`'s
/// `CLOCK_THREAD_CPUTIME_ID`, "CPU time consumed by this thread" (Linux since
/// 2.6.12, and macOS), read through `libc`.
#[cfg(any(target_os = "linux", target_os = "macos"))]
fn thread_cpu() -> Option<Duration> {
    // SAFETY: a zeroed `timespec` is a valid one, and `clock_gettime` writes
    // one, through the pointer it is given, and nothing else.
    let mut now: libc::timespec = unsafe { std::mem::zeroed() };
    let read = unsafe { libc::clock_gettime(libc::CLOCK_THREAD_CPUTIME_ID, &mut now) };
    assert_eq!(read, 0, "clock_gettime(CLOCK_THREAD_CPUTIME_ID) failed");
    Some(Duration::new(now.tv_sec as u64, now.tv_nsec as u32))
}

/// Elsewhere no thread's clock is read, and the wall clock stands in for it.
#[cfg(not(any(target_os = "linux", target_os = "macos")))]
fn thread_cpu() -> Option<Duration> {
    None
}

/// What checking costs: the median of the checking thread's CPU time, which
/// the ceilings bound, and the median of the wall clock, which is printed.
#[derive(Clone, Copy, Debug)]
struct Cost {
    cpu: Duration,
    wall: Duration,
}

fn ms(d: Duration) -> f64 {
    d.as_secs_f64() * 1000.0
}

/// One measurement at a time: this file's two tests ran at once, each timing
/// its checks while the other's ran.
static MEASURING: std::sync::Mutex<()> = std::sync::Mutex::new(());

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

/// `runs` checks of `files`, each `clean` or rejected as said.
fn time_check(files: &[(String, String)], runs: usize, clean: bool) -> Cost {
    let (mut cpu, mut wall) = (Vec::new(), Vec::new());
    for _ in 0..runs {
        let (start, start_cpu) = (Instant::now(), thread_cpu());
        let results = pw_core::check::check_sources(files);
        // Consume the result, so nothing above can be optimized away and the
        // measurement is of work that actually happened.
        let n: usize = results.iter().map(|(_, d)| d.len()).sum();
        if clean {
            assert_eq!(n, 0, "the store program checks clean");
        } else {
            assert!(n > 0, "the broken program must actually be rejected");
        }
        let elapsed = start.elapsed();
        wall.push(elapsed);
        cpu.push(match (start_cpu, thread_cpu()) {
            (Some(before), Some(after)) => after - before,
            _ => elapsed,
        });
    }
    Cost {
        cpu: median(cpu),
        wall: median(wall),
    }
}

#[test]
fn the_whole_program_and_one_edit_are_both_measured() {
    let _alone = MEASURING.lock().unwrap_or_else(|held| held.into_inner());
    let files = program();
    assert!(files.len() >= 30, "only {} files", files.len());

    let cold = time_check(&files, 7, true);

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
    let after_edit = time_check(&edited, 7, true);

    println!(
        "check-latency cold  {:>8.2} ms   (cpu {:.2} ms)",
        ms(cold.wall),
        ms(cold.cpu)
    );
    println!(
        "check-latency edit  {:>8.2} ms   (cpu {:.2} ms; {} files, no query system: \
         an edit costs a full check)",
        ms(after_edit.wall),
        ms(after_edit.cpu),
        files.len()
    );

    // The loose ceiling, for pathological work: two and a half times the
    // 2026-10-13 measurement, where it was fifty times the 2026-08-08 one
    // (the module's comment says how that happened). Of the checking
    // thread's CPU time, since 2026-10-15: the wall clock is the machine's.
    assert!(
        cold.cpu < Duration::from_millis(2000),
        "a full check of {} files cost {:?} of its thread's CPU time ({:?} on \
         the wall clock), which is not editor-feedback territory by any reading",
        files.len(),
        cold.cpu,
        cold.wall
    );

    // **The control.** Without it the numbers above could be a check that
    // returns immediately because it does nothing — which is what a `0 ms`
    // result would actually mean. Or a clock that reads nothing.
    assert!(
        cold.cpu > Duration::from_micros(200),
        "a whole-program check in {:?} of CPU time is too fast to have happened",
        cold.cpu
    );
}

#[test]
fn a_rejected_program_is_not_measurably_slower_than_a_clean_one() {
    // The shape a latency claim usually hides: fast on the happy path, and slow
    // exactly when a developer is iterating on a broken file. Editor feedback
    // is worth most when the program does not compile.
    let _alone = MEASURING.lock().unwrap_or_else(|held| held.into_inner());
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

    let rejected = time_check(&broken, 7, false);
    let clean = time_check(&files, 7, true);

    println!(
        "check-latency reject{:>8.2} ms   (cpu {:.2} ms; clean {:.2} ms, cpu {:.2} ms)",
        ms(rejected.wall),
        ms(rejected.cpu),
        ms(clean.wall),
        ms(clean.cpu)
    );
    assert!(
        rejected.cpu < clean.cpu * 5,
        "producing diagnostics cost {:?} of CPU time against {:?} clean, which \
         means the slow path is the one a developer is on",
        rejected.cpu,
        clean.cpu
    );
}
