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
//!
//! **And so the ceiling measured the program** (train-10, the integrator's
//! ruling 4, 2026-10-17). GitHub's `ubuntu-24.04-arm` runner checked
//! train-10's store program, a fifth more to check than train-9's for its
//! scheduled orders, in 2.11 s of CPU time, past the fixed 2 s, where the
//! x86 runner and the PC passed: a program that grows crosses a fixed
//! ceiling by its growth. The ceiling is per thousand lines checked now, two
//! and a half times what the slowest runner measured, so both of CI's
//! runners pass it with margin; a pathological check (ten checks where one
//! is asked) still fails it on each.
//!
//! **And so the ceiling bounds the fastest check** (track ci-diagnostics,
//! 2026-10-19). CPU time is not immune to load either (above): on the PC, a
//! hybrid 32-thread processor a dozen workers share, one check of 6,568 lines
//! took 0.94 s of CPU alone and up to 2.2 s beside a build, and three of five
//! local `just ci` runs were reported failing this ceiling at loads of 40 to
//! 80. A machine's load only ever adds to a check's time, so the median of
//! seven checks, which it moves, is the wrong statistic for a ceiling, and
//! the fastest check, which it moves least, is the right one. The ceiling
//! bounds the fastest of at least seven checks (`MIN_RUNS`) of the thread's
//! CPU time, and is the same, 800 ms a thousand lines: not raised. Where the
//! fastest so far is over it, more checks are taken, for as long as `WINDOW`
//! of wall time, so that a machine that quiets down is waited for (`sample`);
//! a quiet one takes exactly seven. The median is still printed, as it was.
//! The control is the checker made pathological, which no number of checks
//! lets through, its fastest check as slow as its slowest
//! (`e14-ci-diagnostics`, and `e14-evidence-integrity`'s checker run ten
//! times over, hold it). Hardware instruction counts, the other way to count
//! work, are not to be had here: WSL2's virtual PMU has none (`perf stat -e
//! instructions` says "not supported"), nor has a GitHub-hosted runner's.

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

/// What one check cost: the checking thread's CPU time, and the wall clock.
#[derive(Clone, Copy, Debug)]
struct Run {
    cpu: Duration,
    wall: Duration,
}

/// What checking costs: the fastest check's CPU time, which the ceilings
/// bound, the median of the checks' CPU time and of their wall clock, which
/// are printed, and how many checks they are of.
#[derive(Clone, Copy, Debug)]
struct Cost {
    fastest: Duration,
    cpu: Duration,
    wall: Duration,
    runs: usize,
}

fn ms(d: Duration) -> f64 {
    d.as_secs_f64() * 1000.0
}

/// One measurement at a time: this file's two tests ran at once, each timing
/// its checks while the other's ran.
static MEASURING: std::sync::Mutex<()> = std::sync::Mutex::new(());

/// **The ceiling, per thousand lines checked** (train-10, the integrator's
/// ruling 4 of 2026-10-17): two and a half times what a check cost a
/// thousand lines on CI's slowest runner, GitHub's `ubuntu-24.04-arm`,
/// where train-10's store program, 56 files and 6,568 lines, took 2,108.54
/// ms of the checking thread's CPU time (ci run 39054436550): 321 ms, so
/// 802.6, rounded down to 800. Until then the ceiling was 2 s whatever the
/// program's size, two and a half times the Mac's 850 ms of 2026-10-13: the
/// ARM runner crossed it at train-10 by the program's growth, a fifth more
/// to check for its scheduled orders, not by a pathological change.
const CEILING_PER_THOUSAND_LINES: Duration = Duration::from_millis(800);

/// How many lines `files` hold, what the ceiling is per thousand of.
fn lines_of(files: &[(String, String)]) -> usize {
    files.iter().map(|(_, text)| text.lines().count()).sum()
}

/// What a check of `lines` lines costs a thousand of them, as the ceiling
/// counts.
fn per_thousand(cpu: Duration, lines: usize) -> Duration {
    cpu.mul_f64(1000.0 / lines as f64)
}

/// **Whether a check of `lines` lines that took `fastest` is within the
/// ceiling.** Asked of the fastest check, by `sample` to know whether to take
/// another and by the test to judge: one question, one answer.
fn fits(fastest: Duration, lines: usize) -> bool {
    per_thousand(fastest, lines) < CEILING_PER_THOUSAND_LINES
}

/// **Whether producing diagnostics is no more than five times as slow as a
/// clean check**, both at their fastest: a load that slows the one measured
/// first and not the other is not the compiler's slow path.
fn rejection_is_no_slower(rejected: &Cost, clean: &Cost) -> bool {
    rejected.fastest < clean.fastest * 5
}

/// The machine's load average, as the kernel reports it, for a failure to
/// say what it ran beside: not every platform has one.
fn load_average() -> String {
    std::fs::read_to_string("/proc/loadavg")
        .map(|text| {
            text.split_whitespace()
                .take(3)
                .collect::<Vec<_>>()
                .join(" ")
        })
        .unwrap_or_else(|_| "unknown".to_string())
}

/// **Whether a measurement is fast enough**: its fastest check within the
/// ceiling (`fits`), not its median, which a loaded machine moves.
fn fast_enough(cost: &Cost, lines: usize) -> bool {
    fits(cost.fastest, lines)
}

/// **The control**: whether the measurement is of a check that happened. A
/// check of a whole program in under 200 microseconds of CPU time did not
/// happen, or the clock that read it read nothing: a `0 ms` result would
/// pass every ceiling.
fn happened(cost: &Cost) -> bool {
    cost.fastest > Duration::from_micros(200)
}

/// How many checks are measured at least: seven, as the printed medians have
/// always been of.
const MIN_RUNS: usize = 7;

/// The most, whatever the machine does: a program checked this often is not
/// waiting for a machine to quiet down.
const MAX_RUNS: usize = 40;

/// How much wall time the checks of one measurement may take in all, once
/// `MIN_RUNS` have been taken, before no more are waited for: four minutes,
/// for a machine whose load comes and goes over minutes (a build, another
/// worker's test run) to be waited on. It binds only where the fastest check
/// is over the ceiling, which is where a run fails otherwise.
const WINDOW: Duration = Duration::from_secs(240);

/// **A measurement, said past the test harness's capture**, so that a run
/// that passes says it too: CI's logs on each of its runners are where the
/// ceiling above is measured (train-10's ruling 4).
fn report(line: std::fmt::Arguments) {
    use std::io::Write;
    let _ = writeln!(std::io::stderr(), "{line}");
}

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

/// **The checks of one measurement.** At least `MIN_RUNS` of `once`; then more
/// only while `enough` says the fastest CPU time so far is not (it is `None`
/// where nothing is to be waited for), and until `MAX_RUNS` have been taken or
/// their wall clocks add up to `WINDOW`. A quiet machine is asked exactly
/// `MIN_RUNS` times, as before. A machine that was busy and quiets down is
/// waited for, and what it adds to a check is left out of the fastest, which
/// is what the ceilings bound (see the module). A checker that is slow itself
/// is not let through by the waiting: its fastest check is as slow as any.
fn sample(enough: Option<&dyn Fn(Duration) -> bool>, mut once: impl FnMut() -> Run) -> Cost {
    let mut runs: Vec<Run> = Vec::new();
    let mut spent = Duration::ZERO;
    loop {
        let run = once();
        spent += run.wall;
        runs.push(run);
        if runs.len() < MIN_RUNS {
            continue;
        }
        let fastest = runs.iter().map(|r| r.cpu).min().expect("a run");
        let waiting = enough.is_some_and(|enough| !enough(fastest));
        if !waiting || runs.len() >= MAX_RUNS || spent >= WINDOW {
            break;
        }
    }
    Cost {
        fastest: runs.iter().map(|r| r.cpu).min().expect("a run"),
        cpu: median(runs.iter().map(|r| r.cpu).collect()),
        wall: median(runs.iter().map(|r| r.wall).collect()),
        runs: runs.len(),
    }
}

/// The checks of `files`, each `clean` or rejected as said, as `sample` takes
/// them.
fn time_check(
    files: &[(String, String)],
    clean: bool,
    enough: Option<&dyn Fn(Duration) -> bool>,
) -> Cost {
    sample(enough, || {
        let (start, start_cpu) = (Instant::now(), thread_cpu());
        let results = pw_core::check::check_program_sources(files);
        // Consume the result, so nothing above can be optimized away and the
        // measurement is of work that actually happened.
        let n: usize = results.iter().map(|(_, d)| d.len()).sum();
        if clean {
            assert_eq!(n, 0, "the store program checks clean");
        } else {
            assert!(n > 0, "the broken program must actually be rejected");
        }
        let elapsed = start.elapsed();
        Run {
            wall: elapsed,
            cpu: match (start_cpu, thread_cpu()) {
                (Some(before), Some(after)) => after - before,
                _ => elapsed,
            },
        }
    })
}

#[test]
fn the_whole_program_and_one_edit_are_both_measured() {
    let _alone = MEASURING.lock().unwrap_or_else(|held| held.into_inner());
    let files = program();
    assert!(files.len() >= 30, "only {} files", files.len());

    let lines = lines_of(&files);
    // The fastest of at least seven checks is what the ceiling bounds: more
    // are taken while it is over (see `sample`).
    let cold = time_check(&files, true, Some(&|fastest| fits(fastest, lines)));

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
    let after_edit = time_check(&edited, true, None);

    report(format_args!(
        "check-latency cold  {:>8.2} ms   (cpu {:.2} ms; {} lines, cpu {:.2} ms a thousand lines; \
         the fastest of {} checks, which the ceiling bounds: cpu {:.2} ms, {:.2} ms a thousand lines)",
        ms(cold.wall),
        ms(cold.cpu),
        lines,
        ms(per_thousand(cold.cpu, lines)),
        cold.runs,
        ms(cold.fastest),
        ms(per_thousand(cold.fastest, lines))
    ));
    report(format_args!(
        "check-latency edit  {:>8.2} ms   (cpu {:.2} ms; {} files, no query system: \
         an edit costs a full check)",
        ms(after_edit.wall),
        ms(after_edit.cpu),
        files.len()
    ));

    // The loose ceiling, for pathological work, of the checking thread's
    // CPU time (since 2026-10-15: the wall clock is the machine's), per
    // thousand lines checked (since 2026-10-17: a fixed ceiling over a
    // program that grows is crossed by its growth), of the fastest check
    // (since 2026-10-19: a loaded machine only adds to a check's time): see
    // `CEILING_PER_THOUSAND_LINES`.
    assert!(
        fast_enough(&cold, lines),
        "a full check of {} files, {} lines, cost {:?} of its thread's CPU time \
         at the fastest of {} checks (the median {:?}; {:?} on the wall clock), \
         {:?} a thousand lines, past the {:?} that is two and a half times CI's \
         slowest runner's: not editor-feedback territory by any reading (load \
         average {})",
        files.len(),
        lines,
        cold.fastest,
        cold.runs,
        cold.cpu,
        cold.wall,
        per_thousand(cold.fastest, lines),
        CEILING_PER_THOUSAND_LINES,
        load_average()
    );

    // **The control.** Without it the numbers above could be a check that
    // returns immediately because it does nothing — which is what a `0 ms`
    // result would actually mean. Or a clock that reads nothing.
    assert!(
        happened(&cold),
        "a whole-program check in {:?} of CPU time is too fast to have happened",
        cold.fastest
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

    let rejected = time_check(&broken, false, None);
    let clean = time_check(&files, true, None);

    report(format_args!(
        "check-latency reject{:>8.2} ms   (cpu {:.2} ms; clean {:.2} ms, cpu {:.2} ms; \
         the fastest of {} and {} checks, which the ratio bounds: cpu {:.2} ms, clean cpu {:.2} ms)",
        ms(rejected.wall),
        ms(rejected.cpu),
        ms(clean.wall),
        ms(clean.cpu),
        rejected.runs,
        clean.runs,
        ms(rejected.fastest),
        ms(clean.fastest)
    ));
    assert!(
        rejection_is_no_slower(&rejected, &clean),
        "producing diagnostics cost {:?} of CPU time at the fastest of {} checks \
         against {:?} clean (of {}), which means the slow path is the one a \
         developer is on (load average {})",
        rejected.fastest,
        rejected.runs,
        clean.fastest,
        clean.runs,
        load_average()
    );
}

// **The statistic, tested on checks that cost what a script says**
// (track ci-diagnostics, 2026-10-19): no check is run, so these hold on any
// machine, loaded or not, and take no time. `ms` of CPU and of wall clock, a
// run each.

/// A run that cost `cpu` ms of CPU time and `wall` ms of the wall clock.
fn run(cpu: u64, wall: u64) -> Run {
    Run {
        cpu: Duration::from_millis(cpu),
        wall: Duration::from_millis(wall),
    }
}

/// Runs that cost what `cost` says of the run's number, from 0. A measurement
/// that asks for a thousand is one that does not end.
fn script(cost: impl Fn(usize) -> (u64, u64)) -> impl FnMut() -> Run {
    let mut n = 0;
    move || {
        assert!(n < 1_000, "the waiting for a quiet machine never ended");
        let (cpu, wall) = cost(n);
        n += 1;
        run(cpu, wall)
    }
}

/// The store program's size, and what the ceiling comes to over it: 800 ms a
/// thousand lines, of 6,568, is 5,254.4 ms.
const LINES: usize = 6_568;
const FITS_UNDER: u64 = 5_254;

fn within(fastest: Duration) -> bool {
    fits(fastest, LINES)
}

#[test]
fn the_ceiling_is_800_ms_a_thousand_lines_and_a_check_is_judged_by_it() {
    // Not raised: 800 ms a thousand lines, exactly as ruled (2026-10-17).
    assert_eq!(CEILING_PER_THOUSAND_LINES, Duration::from_millis(800));
    // Seven checks at least, as the printed medians have always been of.
    assert_eq!(MIN_RUNS, 7);
    assert!(fits(Duration::from_millis(FITS_UNDER), LINES));
    assert!(!fits(Duration::from_millis(FITS_UNDER + 1), LINES));
    // And per thousand lines, whatever the program: twice the lines, twice
    // the time.
    assert!(fits(Duration::from_millis(2 * FITS_UNDER), 2 * LINES));
    assert!(!fits(Duration::from_millis(2 * FITS_UNDER + 2), 2 * LINES));
}

#[test]
fn the_ceiling_bounds_the_fastest_check_not_the_median() {
    // A loaded machine: six of seven checks stretched to a median past the
    // ceiling, and one, at a quieter moment, not. The median of these is
    // what failed three of five local runs; the fastest is the compiler's.
    let loaded = [
        (5_800, 9_000),
        (6_100, 9_500),
        (5_900, 9_100),
        (1_400, 1_400),
        (5_700, 8_800),
        (6_000, 9_300),
        (5_600, 8_700),
    ];
    let cost = sample(Some(&within), script(|n| loaded[n % loaded.len()]));
    assert_eq!(cost.runs, MIN_RUNS);
    assert_eq!(cost.fastest, Duration::from_millis(1_400));
    assert_eq!(
        cost.cpu,
        Duration::from_millis(5_800),
        "the median is still reported"
    );
    assert!(!within(cost.cpu), "the median is over the ceiling");
    assert!(within(cost.fastest), "the fastest is not");
    assert!(
        fast_enough(&cost, LINES),
        "and it is the fastest that is judged"
    );
}

#[test]
fn the_fastest_is_of_the_threads_cpu_time_not_of_the_wall_clock() {
    // The fastest wall clock is not the fastest CPU time.
    let runs = [(3_000, 1_000), (1_000, 6_000), (2_000, 2_000)];
    let cost = sample(None, script(|n| runs[n % runs.len()]));
    assert_eq!(cost.fastest, Duration::from_millis(1_000));
    assert_eq!(cost.wall, Duration::from_millis(2_000));
}

#[test]
fn a_quiet_machine_is_asked_for_seven_checks_and_no_more() {
    let cost = sample(Some(&within), script(|_| (1_300, 1_300)));
    assert_eq!(cost.runs, MIN_RUNS);
    // And where nothing is to be waited for, however slow the checks.
    let cost = sample(None, script(|_| (9_000, 9_000)));
    assert_eq!(cost.runs, MIN_RUNS);
}

#[test]
fn a_loaded_machine_is_waited_on_while_it_may_quiet_down() {
    // Ten checks at twice the ceiling, then one at a quieter moment: the
    // eleventh is the first that fits, and the last that is taken.
    let cost = sample(
        Some(&within),
        script(|n| {
            if n < 10 {
                (10_000, 6_000)
            } else {
                (1_300, 1_300)
            }
        }),
    );
    assert_eq!(cost.runs, 11);
    assert_eq!(cost.fastest, Duration::from_millis(1_300));
    assert!(within(cost.fastest));
}

#[test]
fn the_waiting_ends_with_the_window_or_the_most_checks() {
    // Never quiet, a check at 10 s: the window ends it, at the check that
    // takes the total to it.
    let cost = sample(Some(&within), script(|_| (10_000, 10_000)));
    assert_eq!(cost.runs, (WINDOW.as_millis() / 10_000) as usize);
    assert!(cost.runs < MAX_RUNS, "the window, not the most, ended it");
    assert!(!within(cost.fastest));
    // Never quiet, a check taking no time on the wall clock: the most.
    let cost = sample(Some(&within), script(|_| (10_000, 1)));
    assert_eq!(cost.runs, MAX_RUNS);
    assert!(!within(cost.fastest));
}

#[test]
fn a_checker_made_slow_is_not_let_through_by_the_waiting() {
    // The control. The checker run ten times over costs ten times what it
    // does, on every check: 13 s of CPU time where 1.3 s is a quiet one.
    // However many checks are waited for, and however the machine's load
    // moves them about, none is within the ceiling.
    let noisy = |n: usize| (13_000 + (n as u64 * 977) % 4_000, 14_000);
    let long = sample(Some(&within), script(noisy));
    assert!(!fast_enough(&long, LINES), "{:?}", long);
    let brief = sample(Some(&within), script(|n| (noisy(n).0, 1)));
    assert_eq!(brief.runs, MAX_RUNS);
    assert!(!fast_enough(&brief, LINES), "{:?}", brief);
}

#[test]
fn a_clock_that_reads_nothing_is_refused_by_the_control() {
    // Its checks fit any ceiling, and the control (more than 200
    // microseconds of CPU time, the fastest of them) refuses them.
    let nothing = sample(Some(&within), script(|_| (0, 1_300)));
    assert!(fast_enough(&nothing, LINES));
    assert!(!happened(&nothing));
    // One check that read nothing among those that read something is as
    // refused: the fastest is what is judged.
    let once = sample(
        None,
        script(|n| if n == 3 { (0, 1_300) } else { (1_300, 1_300) }),
    );
    assert!(!happened(&once));
    // A quiet machine's checks happened.
    assert!(happened(&sample(None, script(|_| (1_300, 1_300)))));
}

#[test]
fn producing_diagnostics_is_judged_at_the_fastest_of_both() {
    let cost = |fastest: u64, others: u64| {
        sample(
            None,
            script(move |n| if n == 0 { (fastest, 1) } else { (others, 1) }),
        )
    };
    // Rejected checks mostly 9 s and clean ones mostly 2 s, a ratio of 4.5
    // by their medians; at their fastest, 1.5 and 1.4 s, about 1.1: fine.
    assert!(rejection_is_no_slower(
        &cost(1_500, 9_000),
        &cost(1_400, 2_000)
    ));
    // Six times as slow at the fastest, though the medians are close: not.
    assert!(!rejection_is_no_slower(
        &cost(8_500, 9_000),
        &cost(1_400, 9_000)
    ));
}
