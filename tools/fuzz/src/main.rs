//! A coverage-guided fuzzer, on the pinned stable toolchain.
//!
//! Architect ruling, 2026-08-06:
//!
//! > The important change is coverage feedback and evolving corpora, not merely
//! > more random cases.
//!
//! # Why this exists rather than `cargo-fuzz`
//!
//! `cargo-fuzz` needs a nightly toolchain and `libfuzzer-sys`. This project
//! pins its toolchain (`tools/versions.lock`) and keeps its dependency set
//! small, and neither is worth spending to get a fuzzer.
//!
//! `-C instrument-coverage` is **stable**, and `llvm-tools` is a component of
//! the rustc already pinned — adding it changes no recorded version. So the
//! loop is:
//!
//! ```text
//! run an input under an instrumented binary
//!     -> LLVM writes a .profraw
//!     -> llvm-profdata reports covered regions
//!     -> more regions than the corpus has seen? keep it and mutate it
//!     -> otherwise discard
//! ```
//!
//! That is genuine coverage feedback with an evolving corpus. It is
//! **slower** than libFuzzer — a process per input rather than an in-process
//! loop — so it explores fewer inputs per second and this is stated in the
//! evidence rather than glossed.
//!
//! # What it is not
//!
//! Not a replacement for the structured generators. Those encode *what a
//! correct answer looks like*; this finds inputs neither of us thought of. The
//! report keeps them separate, because merging them into one "fuzzed" badge
//! would hide that the structured suites are the ones with properties.
//!
//! # It refuses to fuzz blind
//!
//! The coverage is read with `llvm-profdata`, a program of the rustup component
//! `llvm-tools` (listed in `rust-toolchain.toml`). It is looked for in the
//! active toolchain's own `lib/rustlib/<host>/bin/` (`rustc --print sysroot`,
//! and the `host:` line of `rustc -vV`), or in the directory `PW_LLVM_BIN_DIR`
//! names.
//!
//! An earlier version looked in one machine's
//! `~/.rustup/toolchains/1.97.1-aarch64-apple-darwin/...`, found nothing on any
//! other, and read nothing as "no regions": every target reported
//! "0 covered regions ... 0 finding(s)" and the process exited 0 (2026-10-20, on
//! the PC). A fuzzer that reports success having measured nothing is worse than
//! none, so it now stops, with a message naming what is missing, when
//!
//! - `llvm-profdata` cannot be found or run: before anything is built;
//! - `llvm-profdata` fails on a run's profile, or lists no executed function in
//!   it (its output is no longer what `covered` reads, which is how the first
//!   version measured 69 regions for a whole compiler);
//! - a target ends with no covered region.
//!
//! Exit status: 0 clean, 1 a finding, 2 the run measured nothing (or was
//! called wrongly).
//!
//! # A campaign
//!
//! ```text
//! pw-fuzz [--iterations N] [--seconds S] [--hang S] [--only TARGET[,TARGET...]]
//! ```
//!
//! `--iterations` and `--seconds` bound each target: the first one reached
//! ends it, and one input always runs. `--only` runs the named targets alone,
//! so that a process for each can run side by side.
//!
//! While it runs, each target says on stderr where it has got to every 30
//! seconds (executions, covered regions, findings, executions a second), and
//! each finding as it is found, with the file the input was saved in:
//! `target/fuzz/<target>/finding-<n>.bin`, which
//! `pw-fuzz-harness <target> <file>` runs again. A finding is a panic, an abort,
//! or an input still running after `--hang` seconds (60; a hang, and the
//! process is killed). The same failure at the same place is one finding,
//! however many inputs reach it.

use std::collections::BTreeSet;
use std::ffi::{OsStr, OsString};
use std::fs::File;
use std::io::{Read, Write};
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::time::{Duration, Instant};

#[cfg(test)]
mod tests;

/// The LLVM program the coverage is read with.
const PROFDATA: &str = "llvm-profdata";

/// Names the directory of the LLVM programs, where the active toolchain's own
/// is not the one wanted.
const BIN_DIR_ENV: &str = "PW_LLVM_BIN_DIR";

/// An input still running after this is a finding: a hang.
const HANG: Duration = Duration::from_secs(60);

/// How often a target says where it has got to.
const SAY_EVERY: Duration = Duration::from_secs(30);

const USAGE: &str =
    "usage: pw-fuzz [--iterations N] [--seconds S] [--hang S] [--only TARGET[,TARGET...]]";

/// One fuzz target: a name, and the harness binary that runs one input.
#[derive(Debug)]
struct Target {
    name: &'static str,
    /// Seeds, from the deployment matrix and the corpus.
    seeds: Vec<Vec<u8>>,
}

/// What one target's run found.
#[derive(Debug)]
struct Outcome {
    execs: usize,
    /// Distinct regions the run's inputs covered.
    regions: usize,
    findings: Vec<String>,
}

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    match run(&args) {
        Ok(status) => std::process::exit(status),
        Err(why) => {
            eprintln!("pw-fuzz: {why}");
            std::process::exit(2);
        }
    }
}

/// What a run was asked to do.
#[derive(Debug)]
struct Options {
    /// Executions per target.
    iterations: usize,
    /// Wall-clock seconds per target, as well: a target's run ends at
    /// whichever comes first.
    seconds: Option<u64>,
    /// Seconds an input may run before it is a finding: a hang.
    hang: Option<u64>,
    /// The targets to run; all of them when empty.
    only: Vec<String>,
}

fn options(args: &[String]) -> Result<Options, String> {
    let mut options = Options {
        iterations: 2000,
        seconds: None,
        hang: None,
        only: Vec::new(),
    };
    let mut args = args.iter();
    while let Some(flag) = args.next() {
        let mut value = |wants: &str| {
            args.next()
                .map(String::as_str)
                .ok_or_else(|| format!("{flag} wants {wants}\n{USAGE}"))
        };
        match flag.as_str() {
            "--iterations" => options.iterations = whole(flag, value("a count")?)?,
            "--seconds" => options.seconds = Some(whole(flag, value("a number of seconds")?)?),
            "--hang" => options.hang = Some(whole(flag, value("a number of seconds")?)?),
            "--only" => {
                options.only = value("target names, separated by commas")?
                    .split(',')
                    .map(String::from)
                    .collect();
            }
            other => return Err(format!("unknown argument {other:?}\n{USAGE}")),
        }
    }
    Ok(options)
}

/// A whole number, or an error naming the `flag` that wanted it.
fn whole<T: std::str::FromStr>(flag: &str, text: &str) -> Result<T, String> {
    text.parse()
        .map_err(|_| format!("{flag} wants a whole number, not {text:?}\n{USAGE}"))
}

fn run(args: &[String]) -> Result<i32, String> {
    let options = options(args)?;
    let root = repo_root();
    let targets = chosen(targets(&root), &options.only)?;

    // Before anything slow is built: can the coverage be read at all?
    let profdata = llvm_tool(&root, PROFDATA)?;

    let work = root.join("target/fuzz");
    std::fs::create_dir_all(&work).map_err(|e| format!("cannot make {}: {e}", work.display()))?;

    // Build the instrumented harness once. Every target runs through it, with
    // the target name as its first argument.
    say("building the instrumented harness...");
    let harness = build_instrumented(&root, &work);

    campaign(
        &harness,
        &work,
        &targets,
        &profdata,
        &limits_of(&options),
        &mut say,
    )
}

/// What ends a target's run, as the command line asked.
fn limits_of(options: &Options) -> Limits {
    Limits {
        execs: options.iterations,
        time: options.seconds.map(Duration::from_secs),
        hang: options.hang.map_or(HANG, Duration::from_secs),
        say_every: SAY_EVERY,
    }
}

/// The targets named, or all of them when none is.
fn chosen(all: Vec<Target>, only: &[String]) -> Result<Vec<Target>, String> {
    if only.is_empty() {
        return Ok(all);
    }
    if let Some(unknown) = only
        .iter()
        .find(|n| !all.iter().any(|t| t.name == n.as_str()))
    {
        let known: Vec<&str> = all.iter().map(|t| t.name).collect();
        return Err(format!(
            "no target {unknown:?}; the targets are {}\n{USAGE}",
            known.join(", ")
        ));
    }
    Ok(all
        .into_iter()
        .filter(|t| only.iter().any(|n| n == t.name))
        .collect())
}

/// One line on stderr, written whole: processes appending to one log do not
/// split each other's lines.
fn say(line: &str) {
    let mut bytes = line.as_bytes().to_vec();
    bytes.push(b'\n');
    let _ = std::io::stderr().write_all(&bytes);
}

/// What ends a target's run, and how it reports on the way.
struct Limits {
    /// Executions.
    execs: usize,
    /// Wall-clock time. One input always runs, whatever this is.
    time: Option<Duration>,
    /// An input still running after this is a finding: a hang.
    hang: Duration,
    /// How often a target says where it has got to.
    say_every: Duration,
}

/// Fuzz every target, print the report, and say how the process should end:
/// `Ok(0)` clean, `Ok(1)` a finding, `Err` the run measured nothing.
fn campaign(
    harness: &Path,
    work: &Path,
    targets: &[Target],
    profdata: &Path,
    limits: &Limits,
    say: &mut dyn FnMut(&str),
) -> Result<i32, String> {
    let mut report = String::new();
    let mut outcomes = Vec::new();
    for t in targets {
        let outcome = fuzz_one(harness, work, t, profdata, limits, say)?;
        report.push_str(&format!(
            "  {:28} {:>7} execs  {:>6} covered regions  {} finding(s)\n",
            t.name,
            outcome.execs,
            outcome.regions,
            outcome.findings.len()
        ));
        for f in &outcome.findings {
            report.push_str(&format!("      FINDING: {f}\n"));
        }
        outcomes.push((t.name, outcome));
    }
    let execs: usize = outcomes.iter().map(|(_, o)| o.execs).sum();
    let findings: usize = outcomes.iter().map(|(_, o)| o.findings.len()).sum();

    println!("coverage-guided fuzzing");
    println!("  toolchain: rustc 1.97.1 stable, -C instrument-coverage, llvm-profdata");
    println!("  llvm-profdata: {}", profdata.display());
    println!("  NOT libFuzzer: a process per input, so executions/second are low");
    println!();
    print!("{report}");
    println!();
    println!(
        "  {} targets, {execs} executions, {findings} findings",
        targets.len()
    );

    floor(&outcomes)?;
    Ok(if findings > 0 { 1 } else { 0 })
}

/// The non-vacuity floor: a target that ended having covered no region
/// measured nothing, whatever else it says, and "0 findings" from it means
/// nothing.
fn floor(outcomes: &[(&str, Outcome)]) -> Result<(), String> {
    let empty: Vec<String> = outcomes
        .iter()
        .filter(|(_, o)| o.regions == 0)
        .map(|(name, o)| format!("{name} ({} executions)", o.execs))
        .collect();
    if empty.is_empty() {
        return Ok(());
    }
    Err(format!(
        "covered 0 regions: {}\n  \
         nothing was measured, so 0 findings from it would mean nothing.\n  \
         Is llvm-profdata the one of the toolchain that built the harness, \
         and was the harness built with -C instrument-coverage?",
        empty.join(", ")
    ))
}

fn repo_root() -> PathBuf {
    let mut p = std::env::current_dir().expect("cwd");
    while !p.join("PROJECT_CHARTER.md").exists() {
        p = p.parent().expect("repo root").to_path_buf();
    }
    p
}

/// Build the harness with coverage instrumentation, and return its path.
fn build_instrumented(root: &Path, work: &Path) -> PathBuf {
    let out = work.join("build");
    let status = Command::new("cargo")
        .args([
            "build",
            "--quiet",
            "-p",
            "pw-fuzz",
            "--bin",
            "pw-fuzz-harness",
        ])
        .env("RUSTFLAGS", "-C instrument-coverage")
        .env("CARGO_TARGET_DIR", &out)
        .current_dir(root)
        .status()
        .expect("cargo build");
    assert!(status.success(), "instrumented build failed");
    out.join("debug/pw-fuzz-harness")
}

fn targets(root: &Path) -> Vec<Target> {
    // Seeds are real inputs: corpus source for the compiler targets, and the
    // encoded manifests the deployment matrix uses for the resume ones.
    let corpus = |dir: &str| -> Vec<Vec<u8>> {
        let mut out = Vec::new();
        let d = root.join(dir);
        if let Ok(entries) = std::fs::read_dir(&d) {
            for e in entries.flatten() {
                let p = e.path();
                if p.extension().is_some_and(|x| x == "pw")
                    && let Ok(b) = std::fs::read(&p)
                {
                    out.push(b);
                }
            }
        }
        out
    };

    vec![
        Target {
            name: "parser-to-hir",
            seeds: corpus("examples/accepted"),
        },
        Target {
            name: "resolution-to-typing",
            seeds: corpus("examples/rejected"),
        },
        Target {
            name: "labels-and-effects",
            seeds: corpus("examples/generality/value_exceeds_sink_level"),
        },
        Target {
            name: "exhaustiveness",
            seeds: corpus("examples/robustness/regressions"),
        },
        Target {
            name: "manifest-decoding",
            seeds: vec![
                b"2|1|B1|h|s|d|public|payload".to_vec(),
                b"2|1|B1|h|s|d|session:s-1|payload".to_vec(),
                b"1|1|B1|h|s|d|public|payload".to_vec(),
            ],
        },
        Target {
            name: "resume-compatibility",
            seeds: vec![
                b"2|1|B1|h1|s1|d1|public|x".to_vec(),
                b"2|1|B2|h2|s2|d1|session:s-1|xy".to_vec(),
            ],
        },
    ]
}

/// The loop: run, measure, keep what covers more, mutate.
///
/// An error is a run that could not be carried out or measured (the harness
/// would not start, `llvm-profdata` failed), not a finding.
fn fuzz_one(
    harness: &Path,
    work: &Path,
    target: &Target,
    profdata: &Path,
    limits: &Limits,
    say: &mut dyn FnMut(&str),
) -> Result<Outcome, String> {
    let dir = work.join(target.name);
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).map_err(|e| format!("cannot make {}: {e}", dir.display()))?;

    let mut corpus: Vec<Vec<u8>> = if target.seeds.is_empty() {
        vec![b"seed".to_vec()]
    } else {
        target.seeds.clone()
    };
    let mut seen_regions: BTreeSet<u64> = BTreeSet::new();
    let mut findings: Vec<String> = Vec::new();
    // What failed and where, not the input that showed it: one panic reached
    // by a thousand inputs of as many lengths is one finding.
    let mut failures: BTreeSet<String> = BTreeSet::new();
    let mut rng = Rng(0x5EED_1234);
    let mut execs = 0usize;
    let began = Instant::now();
    let mut said = began;
    let stderr = dir.join("stderr.txt");

    // Every seed first, so the corpus starts from known-interesting inputs.
    let mut queue: Vec<Vec<u8>> = corpus.clone();
    say(&format!(
        "[{}] starting with {} seed(s)",
        target.name,
        queue.len()
    ));

    while execs < limits.execs && (execs == 0 || limits.time.is_none_or(|t| began.elapsed() < t)) {
        if said.elapsed() >= limits.say_every {
            let line = progress(
                target.name,
                execs,
                seen_regions.len(),
                findings.len(),
                began.elapsed(),
            );
            say(&line);
            said = Instant::now();
        }

        let input = match queue.pop() {
            Some(i) => i,
            None => {
                let base = corpus[rng.below(corpus.len())].clone();
                mutate(&base, &mut rng)
            }
        };
        execs += 1;

        let profraw = dir.join(format!("{execs}.profraw"));
        let input_path = dir.join("input.bin");
        std::fs::write(&input_path, &input)
            .map_err(|e| format!("cannot write {}: {e}", input_path.display()))?;

        let run = Run {
            harness,
            target: target.name,
            input: &input_path,
            profraw: &profraw,
            stderr: &stderr,
            hang: limits.hang,
        };
        if let Some(failure) = run_input(&run)? {
            // The profile of a run that did not survive is of no use, and a
            // campaign that fails often would fill the disk with them.
            let _ = std::fs::remove_file(&profraw);
            if failures.insert(format!("{}: {}", failure.kind, failure.why)) {
                let saved = dir.join(format!("finding-{}.bin", findings.len()));
                std::fs::write(&saved, &input)
                    .map_err(|e| format!("cannot save {}: {e}", saved.display()))?;
                let finding = format!("{} on {} bytes: {}", failure.kind, input.len(), failure.why);
                let line = format!(
                    "FINDING [{}] {finding} (saved: {})",
                    target.name,
                    saved.display()
                );
                say(&line);
                findings.push(finding);
            }
            continue;
        }

        // Coverage feedback: did this input reach regions the corpus has not?
        let new = regions(profdata, &profraw)?
            .into_iter()
            .filter(|r| !seen_regions.contains(r))
            .collect::<Vec<_>>();
        let _ = std::fs::remove_file(&profraw);
        if !new.is_empty() {
            seen_regions.extend(new);
            corpus.push(input.clone());
            // A newly interesting input is worth mutating immediately.
            for _ in 0..3 {
                queue.push(mutate(&input, &mut rng));
            }
        }
    }

    let line = progress(
        target.name,
        execs,
        seen_regions.len(),
        findings.len(),
        began.elapsed(),
    );
    say(&format!("done {line}"));
    Ok(Outcome {
        execs,
        regions: seen_regions.len(),
        findings,
    })
}

/// Where a target has got to, as a line of a log.
fn progress(
    name: &str,
    execs: usize,
    regions: usize,
    findings: usize,
    elapsed: Duration,
) -> String {
    let rate = execs as f64 / elapsed.as_secs_f64().max(0.001);
    format!(
        "[{name}] {execs} execs, {regions} covered regions, {findings} finding(s), {rate:.1} execs/s, {}s",
        elapsed.as_secs()
    )
}

/// One run of the harness.
struct Run<'a> {
    harness: &'a Path,
    target: &'a str,
    input: &'a Path,
    /// Where the harness is told to write its profile.
    profraw: &'a Path,
    /// Where its stderr goes.
    stderr: &'a Path,
    /// How long it may run.
    hang: Duration,
}

/// A run that did not survive.
struct Failure {
    /// `panic` (the harness caught it, and exits 101), `abort` (any other way
    /// to die) or `hang` (still running after [`Run::hang`], and killed).
    kind: &'static str,
    /// The first line the harness wrote to stderr.
    why: String,
}

/// Run the harness once: `None` if it survived.
fn run_input(run: &Run) -> Result<Option<Failure>, String> {
    let log = File::create(run.stderr)
        .map_err(|e| format!("cannot write {}: {e}", run.stderr.display()))?;
    let mut child = Command::new(run.harness)
        .arg(run.target)
        .arg(run.input)
        .env("LLVM_PROFILE_FILE", run.profraw)
        .stdout(Stdio::null())
        .stderr(log)
        .spawn()
        .map_err(|e| format!("cannot run {}: {e}", run.harness.display()))?;
    let began = Instant::now();
    let status = loop {
        match child.try_wait() {
            Ok(Some(status)) => break status,
            Ok(None) if began.elapsed() >= run.hang => {
                let _ = child.kill();
                let _ = child.wait();
                return Ok(Some(Failure {
                    kind: "hang",
                    why: format!("still running after {} s", run.hang.as_secs()),
                }));
            }
            Ok(None) => std::thread::sleep(Duration::from_millis(1)),
            Err(e) => return Err(format!("waiting for {}: {e}", run.harness.display())),
        }
    };
    if status.success() {
        return Ok(None);
    }
    // A crash is a finding. The harness returns 101 for a panic it caught and
    // reported; anything else non-zero is an uncaught abort.
    let mut head = Vec::new();
    if let Ok(log) = File::open(run.stderr) {
        let _ = log.take(8192).read_to_end(&mut head);
    }
    Ok(Some(Failure {
        kind: if status.code() == Some(101) {
            "panic"
        } else {
            "abort"
        },
        why: first_line(&head),
    }))
}

/// Covered region hashes, from `llvm-profdata show` on one run's profile.
///
/// An error is a measurement that did not happen. It must not read as "this
/// input covered nothing new": that is how the fuzzer ran blind, reporting
/// success, on every machine but the one whose path it had hard-coded.
fn regions(profdata: &Path, profraw: &Path) -> Result<Vec<u64>, String> {
    let out = match Command::new(profdata)
        .args(["show", "--counts", "--all-functions"])
        .arg(profraw)
        .output()
    {
        Ok(out) => out,
        Err(e) => return Err(format!("cannot run {}: {e}", profdata.display())),
    };
    if !out.status.success() {
        return Err(format!(
            "{} could not read {}: {}\n  was the harness built with -C instrument-coverage?",
            profdata.display(),
            profraw.display(),
            first_line(&out.stderr)
        ));
    }
    let found = covered(&String::from_utf8_lossy(&out.stdout));
    if found.is_empty() {
        // A run of the instrumented harness executes at least its `main`.
        return Err(format!(
            "{} lists no executed function in {}: its output is no longer what pw-fuzz reads",
            profdata.display(),
            profraw.display()
        ));
    }
    Ok(found)
}

/// The first line of what a program wrote, or a note that it wrote none.
fn first_line(bytes: &[u8]) -> String {
    String::from_utf8_lossy(bytes)
        .lines()
        .next()
        .unwrap_or("<no message>")
        .to_string()
}

/// The regions that ran, in the text `llvm-profdata show --counts
/// --all-functions` prints.
fn covered(text: &str) -> Vec<u64> {
    // One hash per (function, counter index) that executed at least once. The
    // exact numbers do not matter; what matters is the SET, and whether an
    // input grows it.
    // The format is
    //
    //     <mangled name>:
    //       Hash: 0x..
    //       Counters: N
    //       Function count: N
    //       Block counts: [1, 0, 0]
    //
    // and NOT `Function name: ..`, which is what a first version looked for —
    // so it matched nothing in the dependencies and reported 69 regions for a
    // whole compiler. The number being implausibly small is what gave it away.
    let mut function = String::new();
    let mut out_regions = Vec::new();
    for line in text.lines() {
        let trimmed = line.trim_end();
        if let Some(name) = trimmed.strip_prefix("  ")
            && name.ends_with(':')
            && !name.starts_with(' ')
        {
            function = name.trim_end_matches(':').to_string();
            continue;
        }
        if let Some(n) = trimmed.trim_start().strip_prefix("Function count: ")
            && n.trim().parse::<u64>().unwrap_or(0) > 0
        {
            out_regions.push(fnv(&function));
        }
        if let Some(counts) = trimmed.trim_start().strip_prefix("Block counts: [") {
            for (i, c) in counts.trim_end_matches(']').split(',').enumerate() {
                if c.trim().parse::<u64>().unwrap_or(0) > 0 {
                    out_regions.push(fnv(&format!("{function}#{i}")));
                }
            }
        }
    }
    out_regions
}

/// The LLVM program `tool`, found where the active toolchain keeps it, and
/// checked: it is there and it runs.
///
/// `root` is where `rustc` is asked, so that it is the toolchain `cargo build`
/// builds the harness with there (`rust-toolchain.toml`).
fn llvm_tool(root: &Path, tool: &str) -> Result<PathBuf, String> {
    let rustc = std::env::var_os("RUSTC").unwrap_or_else(|| "rustc".into());
    let dir = bin_dir(&rustc, root, std::env::var_os(BIN_DIR_ENV))?;
    runnable(&dir, tool)
}

/// The directory of the LLVM programs: `over_ride` (`PW_LLVM_BIN_DIR`) if
/// one is given, else the active toolchain's
/// `<sysroot>/lib/rustlib/<host>/bin`, which `rustc` itself says.
fn bin_dir(rustc: &OsStr, cwd: &Path, over_ride: Option<OsString>) -> Result<PathBuf, String> {
    if let Some(dir) = over_ride.filter(|d| !d.is_empty()) {
        return Ok(PathBuf::from(dir));
    }
    let sysroot = ask_rustc(rustc, cwd, &["--print", "sysroot"])?;
    let version = ask_rustc(rustc, cwd, &["-vV"])?;
    let host = host_of(&version).ok_or_else(|| "`rustc -vV` names no host".to_string())?;
    Ok(Path::new(sysroot.trim())
        .join("lib/rustlib")
        .join(host)
        .join("bin"))
}

/// What `rustc` prints for `args`, run in `cwd`.
fn ask_rustc(rustc: &OsStr, cwd: &Path, args: &[&str]) -> Result<String, String> {
    let shown = Path::new(rustc).display();
    let out = Command::new(rustc)
        .args(args)
        .current_dir(cwd)
        .output()
        .map_err(|e| format!("cannot run {shown}: {e}"))?;
    if !out.status.success() {
        return Err(format!(
            "`{shown} {}` failed ({}): {}",
            args.join(" "),
            out.status,
            first_line(&out.stderr)
        ));
    }
    Ok(String::from_utf8_lossy(&out.stdout).into_owned())
}

/// The target triple of the machine, from the output of `rustc -vV`.
fn host_of(verbose_version: &str) -> Option<&str> {
    verbose_version
        .lines()
        .find_map(|line| line.strip_prefix("host: "))
        .map(str::trim)
}

/// `dir/tool`, if it exists and runs.
fn runnable(dir: &Path, tool: &str) -> Result<PathBuf, String> {
    let path = dir.join(tool);
    if !path.is_file() {
        return Err(format!(
            "{tool} not found: {} is not a file\n  \
             pw-fuzz reads its coverage with it; without it every target would report 0 covered regions.\n  \
             Get it with `rustup component add llvm-tools` (rust-toolchain.toml lists it),\n  \
             or set {BIN_DIR_ENV} to a directory that holds it.",
            path.display()
        ));
    }
    match Command::new(&path).arg("--version").output() {
        Ok(out) if out.status.success() => Ok(path),
        Ok(out) => Err(format!(
            "{} --version failed ({}): {}",
            path.display(),
            out.status,
            first_line(&out.stderr)
        )),
        Err(e) => Err(format!("cannot run {}: {e}", path.display())),
    }
}

fn fnv(s: &str) -> u64 {
    let mut h: u64 = 0xcbf2_9ce4_8422_2325;
    for b in s.as_bytes() {
        h ^= *b as u64;
        h = h.wrapping_mul(0x0000_0100_0000_01b3);
    }
    h
}

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
        (self.next() % n.max(1) as u64) as usize
    }
}

/// Byte-level mutations. Deliberately dumb: the structured generators already
/// explore semantically-shaped inputs, and this is here to find the shapes
/// neither of us would write.
fn mutate(base: &[u8], rng: &mut Rng) -> Vec<u8> {
    let mut out = base.to_vec();
    if out.is_empty() {
        return vec![rng.below(256) as u8];
    }
    match rng.below(6) {
        0 => {
            let at = rng.below(out.len());
            out[at] = rng.below(256) as u8;
        }
        1 => {
            let at = rng.below(out.len());
            out.insert(at, rng.below(256) as u8);
        }
        2 => {
            let at = rng.below(out.len());
            out.remove(at);
        }
        3 => out.truncate(rng.below(out.len())),
        4 => {
            // Splice: take a chunk from elsewhere in the same input.
            let a = rng.below(out.len());
            let b = rng.below(out.len());
            let (lo, hi) = (a.min(b), a.max(b));
            let chunk: Vec<u8> = out[lo..hi].to_vec();
            let at = rng.below(out.len());
            out.splice(at..at, chunk);
        }
        _ => {
            // Interesting bytes: the ones that break parsers.
            let at = rng.below(out.len());
            out[at] = [0u8, b'{', b'}', b'"', b'\\', 0xff, b'\n'][rng.below(7)];
        }
    }
    out.truncate(1 << 16);
    out
}
