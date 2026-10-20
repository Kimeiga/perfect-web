//! The fuzzer's own tests: where it looks for `llvm-profdata`, what it makes of
//! a profile, and that it stops, rather than report success, when it measured
//! nothing.
//!
//! The stand-ins are shell scripts: a harness whose end the first bytes of its
//! input choose, and an `llvm-profdata` that prints the profile text the
//! harness left where the profile goes. The real tool is used once, on a real
//! profile of a program built here
//! (`the_real_llvm_profdata_reads_a_real_profile`), and that is the test that
//! fails, naming the component to install, on a machine without it.

use super::*;
use std::os::unix::fs::PermissionsExt;
use std::sync::{Mutex, MutexGuard, OnceLock};

/// What `llvm-profdata show --counts --all-functions` (LLVM 22.1.6, rustc
/// 1.97.1's) printed for [`PROBE_SOURCE`], built with `-C instrument-coverage`
/// and run once. A function that never ran (`unused`, in the source that was
/// profiled) is not listed.
const PROBE_PROFILE: &str = r#"Counters:
  _RNvCs5JAvRntY6Pu_5probe4main:
    Hash: 0xdaab0724083e7cb7
    Counters: 2
    Function count: 1
    Block counts: [4]
  _RNvCs5JAvRntY6Pu_5probe6parity:
    Hash: 0xa3cc66c25ff9a9c0
    Counters: 2
    Function count: 1
    Block counts: [0]
Instrumentation level: Front-end
Functions shown: 2
Total functions: 2
Maximum function count: 1
Maximum internal block count: 4
Total number of blocks: 4
Total count: 6
"#;

/// A program with three regions that run: `main`, the loop in it, `parity`.
const PROBE_SOURCE: &str = r#"fn parity(n: u32) -> &'static str {
    if n % 2 == 0 { "even" } else { "odd" }
}

fn main() {
    let mut total = 0;
    for i in 0..3 {
        total += i;
    }
    println!("{} {}", parity(total), total);
}
"#;

/// Tests that start processes run one at a time. A script written in one
/// thread while another forks stays "text file busy" to its first exec: the
/// forked child holds the write descriptor until it execs.
static PROCESSES: Mutex<()> = Mutex::new(());

fn one_at_a_time() -> MutexGuard<'static, ()> {
    PROCESSES
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner())
}

/// A fresh empty directory of this test run's own: `target/fuzz-tests/<pid>/<name>`.
fn scratch(name: &str) -> PathBuf {
    let dir = repo_root()
        .join("target/fuzz-tests")
        .join(std::process::id().to_string())
        .join(name);
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    dir
}

fn script(dir: &Path, name: &str, body: &str) {
    let path = dir.join(name);
    std::fs::write(&path, body).unwrap();
    std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o755)).unwrap();
}

/// The stand-in programs, written once per test run. Call it with
/// [`one_at_a_time`] held.
fn stubs() -> &'static Path {
    static STUBS: OnceLock<PathBuf> = OnceLock::new();
    STUBS.get_or_init(|| {
        let dir = scratch("stubs");
        std::fs::write(dir.join("probe-profile.txt"), PROBE_PROFILE).unwrap();
        // `harness.sh TARGET INPUT`: how the run ends is what the input starts
        // with. Like the real harness it leaves its profile whatever happens.
        script(
            &dir,
            "harness.sh",
            r#"#!/bin/sh
cp "$(dirname "$0")/probe-profile.txt" "$LLVM_PROFILE_FILE"
case "$(head -c 5 "$2")" in
  PANIC) echo "thread 'main' panicked at stub.rs:1:1:" >&2; exit 101 ;;
  ABORT) echo "stub: aborted" >&2; exit 134 ;;
esac
"#,
        );
        // `show --counts --all-functions PROFILE` prints the profile, which the
        // stub harness wrote as text.
        script(
            &dir,
            "profdata.sh",
            r#"#!/bin/sh
case "$1" in
  --version) echo "stub llvm-profdata" ;;
  show) cat "$4" ;;
  *) exit 64 ;;
esac
"#,
        );
        script(
            &dir,
            "profdata-fails.sh",
            "#!/bin/sh\necho 'error: stub: truncated profile data' >&2\nexit 1\n",
        );
        // Runs, and lists no function at all.
        script(
            &dir,
            "profdata-silent.sh",
            r#"#!/bin/sh
case "$1" in
  --version) echo "stub llvm-profdata" ;;
  show) printf 'Counters:\nInstrumentation level: Front-end\nFunctions shown: 0\nTotal functions: 0\n' ;;
esac
"#,
        );
        script(
            &dir,
            "rustc-stub.sh",
            r#"#!/bin/sh
case "$*" in
  "--print sysroot") echo "/stub/sysroot" ;;
  "-vV") printf 'rustc 9.9.9 (stub)\nbinary: rustc\nhost: stub-host-triple\nrelease: 9.9.9\n' ;;
  *) echo "stub rustc: no such question" >&2; exit 64 ;;
esac
"#,
        );
        script(
            &dir,
            "rustc-fails.sh",
            "#!/bin/sh\necho 'error: stub: no toolchain' >&2\nexit 1\n",
        );
        dir
    })
}

fn target(name: &'static str, seeds: &[&str]) -> Target {
    Target {
        name,
        seeds: seeds.iter().map(|s| s.as_bytes().to_vec()).collect(),
    }
}

// ---------------------------------------------------------------------------
// Where the tool is
// ---------------------------------------------------------------------------

#[test]
fn the_tool_is_in_the_active_toolchains_own_directory() {
    let _lock = one_at_a_time();
    let rustc = stubs().join("rustc-stub.sh");
    let dir = bin_dir(rustc.as_os_str(), stubs(), None).unwrap();
    assert_eq!(
        dir,
        Path::new("/stub/sysroot/lib/rustlib/stub-host-triple/bin")
    );
}

#[test]
fn an_override_names_the_directory_instead() {
    // With an override rustc is not asked at all: this one does not exist.
    let dir = bin_dir(
        OsStr::new("/no/such/rustc"),
        Path::new("."),
        Some("/elsewhere/bin".into()),
    )
    .unwrap();
    assert_eq!(dir, Path::new("/elsewhere/bin"));
}

#[test]
fn an_empty_override_is_no_override() {
    let _lock = one_at_a_time();
    let rustc = stubs().join("rustc-stub.sh");
    let dir = bin_dir(rustc.as_os_str(), stubs(), Some(OsString::new())).unwrap();
    assert_eq!(
        dir,
        Path::new("/stub/sysroot/lib/rustlib/stub-host-triple/bin")
    );
}

#[test]
fn a_rustc_that_fails_or_is_missing_is_an_error() {
    let _lock = one_at_a_time();
    let fails = stubs().join("rustc-fails.sh");
    let why = bin_dir(fails.as_os_str(), stubs(), None).unwrap_err();
    assert!(why.contains("stub: no toolchain"), "{why}");
    let why = bin_dir(OsStr::new("/no/such/rustc"), stubs(), None).unwrap_err();
    assert!(why.contains("cannot run /no/such/rustc"), "{why}");
}

#[test]
fn the_host_is_the_host_line_of_rustc_vv() {
    let text = "rustc 1.97.1 (8bab26f4f 2026-07-14)\nbinary: rustc\n\
                commit-hash: 8bab26f4f68e0e26f0bb7960be334d5b520ea452\n\
                host: x86_64-unknown-linux-gnu\nrelease: 1.97.1\nLLVM version: 22.1.6\n";
    assert_eq!(host_of(text), Some("x86_64-unknown-linux-gnu"));
    assert_eq!(host_of("release: 1.97.1\n"), None);
}

#[test]
fn a_directory_without_the_tool_is_an_error_that_says_how_to_get_it() {
    let _lock = one_at_a_time();
    let dir = scratch("no-tool");
    let why = runnable(&dir, PROFDATA).unwrap_err();
    assert!(why.contains("llvm-profdata not found"), "{why}");
    assert!(why.contains(&dir.display().to_string()), "{why}");
    assert!(why.contains("rustup component add llvm-tools"), "{why}");
    assert!(why.contains(BIN_DIR_ENV), "{why}");
}

#[test]
fn a_tool_that_exists_and_runs_is_found() {
    let _lock = one_at_a_time();
    let found = runnable(stubs(), "profdata.sh").unwrap();
    assert_eq!(found, stubs().join("profdata.sh"));
}

#[test]
fn a_tool_that_will_not_run_is_an_error() {
    let _lock = one_at_a_time();
    let why = runnable(stubs(), "profdata-fails.sh").unwrap_err();
    assert!(why.contains("--version failed"), "{why}");
    assert!(why.contains("truncated profile data"), "{why}");

    // There, and not a program.
    let dir = scratch("not-a-program");
    std::fs::write(dir.join(PROFDATA), "not a program").unwrap();
    let why = runnable(&dir, PROFDATA).unwrap_err();
    assert!(why.contains("cannot run"), "{why}");
}

// ---------------------------------------------------------------------------
// What a profile says
// ---------------------------------------------------------------------------

#[test]
fn a_profile_reads_as_the_functions_and_the_blocks_that_ran() {
    let main = "_RNvCs5JAvRntY6Pu_5probe4main";
    let parity = "_RNvCs5JAvRntY6Pu_5probe6parity";
    let found = covered(PROBE_PROFILE);
    let wanted = [fnv(main), fnv(&format!("{main}#0")), fnv(parity)];
    assert_eq!(found.len(), 3, "{found:?}");
    for hash in wanted {
        assert!(found.contains(&hash), "{hash:x} is missing from {found:x?}");
    }
}

#[test]
fn what_never_ran_adds_nothing() {
    let text = "Counters:\n  _RNvCs_5probe5never:\n    Hash: 0x1\n    Counters: 3\n    \
                Function count: 0\n    Block counts: [0, 0]\n";
    assert!(covered(text).is_empty());
}

#[test]
fn regions_are_read_through_the_tool() {
    let _lock = one_at_a_time();
    let dir = scratch("regions-read");
    let profile = dir.join("run.profraw");
    std::fs::write(&profile, PROBE_PROFILE).unwrap();
    let found = regions(&stubs().join("profdata.sh"), &profile).unwrap();
    assert_eq!(found.len(), 3, "{found:?}");
}

#[test]
fn regions_fail_when_the_tool_fails() {
    let _lock = one_at_a_time();
    let dir = scratch("regions-tool-fails");
    let profile = dir.join("run.profraw");
    std::fs::write(&profile, PROBE_PROFILE).unwrap();
    let why = regions(&stubs().join("profdata-fails.sh"), &profile).unwrap_err();
    assert!(why.contains("could not read"), "{why}");
    assert!(why.contains("truncated profile data"), "{why}");
}

#[test]
fn regions_fail_when_the_tool_is_missing() {
    let _lock = one_at_a_time();
    let why = regions(
        Path::new("/no/such/llvm-profdata"),
        Path::new("/no/such/run.profraw"),
    )
    .unwrap_err();
    assert!(why.contains("cannot run /no/such/llvm-profdata"), "{why}");
}

#[test]
fn regions_fail_when_the_profile_lists_no_function() {
    let _lock = one_at_a_time();
    let dir = scratch("regions-silent");
    let profile = dir.join("run.profraw");
    std::fs::write(&profile, "").unwrap();
    let why = regions(&stubs().join("profdata-silent.sh"), &profile).unwrap_err();
    assert!(why.contains("lists no executed function"), "{why}");
}

/// The one test on the real tool. It fails, saying what to install, on a
/// machine without `llvm-tools`: the tests that stand in for it say nothing of
/// that.
#[test]
fn the_real_llvm_profdata_reads_a_real_profile() {
    let _lock = one_at_a_time();
    let root = repo_root();
    let profdata = llvm_tool(&root, PROFDATA).unwrap_or_else(|why| panic!("{why}"));

    let dir = scratch("real");
    std::fs::write(dir.join("probe.rs"), PROBE_SOURCE).unwrap();
    let rustc = std::env::var_os("RUSTC").unwrap_or_else(|| "rustc".into());
    let built = Command::new(&rustc)
        .args(["-C", "instrument-coverage", "--crate-name", "probe"])
        .arg(dir.join("probe.rs"))
        .arg("-o")
        .arg(dir.join("probe"))
        .current_dir(&root)
        .output()
        .unwrap();
    assert!(
        built.status.success(),
        "{}",
        String::from_utf8_lossy(&built.stderr)
    );
    let profile = dir.join("probe.profraw");
    let ran = Command::new(dir.join("probe"))
        .env("LLVM_PROFILE_FILE", &profile)
        .output()
        .unwrap();
    assert!(ran.status.success());

    let found = regions(&profdata, &profile).unwrap();
    assert_eq!(found.len(), 3, "{found:x?}");
}

// ---------------------------------------------------------------------------
// A run
// ---------------------------------------------------------------------------

#[test]
fn a_run_covers_what_its_profile_lists() {
    let _lock = one_at_a_time();
    let work = scratch("run-covers");
    let outcome = fuzz_one(
        &stubs().join("harness.sh"),
        &work,
        &target("stub", &["fine"]),
        &stubs().join("profdata.sh"),
        5,
    )
    .unwrap();
    assert_eq!(outcome.execs, 5);
    assert_eq!(outcome.regions, 3);
    assert!(outcome.findings.is_empty(), "{:?}", outcome.findings);
}

#[test]
fn a_panic_is_a_finding_and_its_input_is_kept() {
    let _lock = one_at_a_time();
    let work = scratch("run-panics");
    let outcome = fuzz_one(
        &stubs().join("harness.sh"),
        &work,
        &target("stub", &["PANIC with this"]),
        &stubs().join("profdata.sh"),
        1,
    )
    .unwrap();
    assert_eq!(outcome.findings.len(), 1, "{:?}", outcome.findings);
    assert_eq!(
        outcome.findings[0],
        "panic on 15 bytes: thread 'main' panicked at stub.rs:1:1:"
    );
    assert_eq!(
        std::fs::read(work.join("stub/finding-0.bin")).unwrap(),
        b"PANIC with this"
    );
}

#[test]
fn an_abort_is_a_finding() {
    let _lock = one_at_a_time();
    let work = scratch("run-aborts");
    let outcome = fuzz_one(
        &stubs().join("harness.sh"),
        &work,
        &target("stub", &["ABORT"]),
        &stubs().join("profdata.sh"),
        1,
    )
    .unwrap();
    assert_eq!(outcome.findings, ["abort on 5 bytes: stub: aborted"]);
}

#[test]
fn a_run_that_cannot_be_measured_is_an_error_not_a_quiet_zero() {
    let _lock = one_at_a_time();
    let work = scratch("run-unmeasurable");
    let why = fuzz_one(
        &stubs().join("harness.sh"),
        &work,
        &target("stub", &["fine"]),
        &stubs().join("profdata-fails.sh"),
        3,
    )
    .unwrap_err();
    assert!(why.contains("could not read"), "{why}");

    let why = fuzz_one(
        Path::new("/no/such/harness"),
        &work,
        &target("stub", &["fine"]),
        &stubs().join("profdata.sh"),
        3,
    )
    .unwrap_err();
    assert!(why.contains("cannot run /no/such/harness"), "{why}");
}

// ---------------------------------------------------------------------------
// The floor
// ---------------------------------------------------------------------------

#[test]
fn the_floor_names_the_targets_that_covered_nothing() {
    let outcomes = [
        (
            "covers",
            Outcome {
                execs: 9,
                regions: 4,
                findings: Vec::new(),
            },
        ),
        (
            "blind",
            Outcome {
                execs: 7,
                regions: 0,
                findings: Vec::new(),
            },
        ),
    ];
    let why = floor(&outcomes).unwrap_err();
    assert!(
        why.contains("covered 0 regions: blind (7 executions)"),
        "{why}"
    );
    assert!(!why.contains("covers"), "{why}");
    assert!(floor(&outcomes[..1]).is_ok());
}

#[test]
fn a_clean_run_ends_with_status_0() {
    let _lock = one_at_a_time();
    let work = scratch("campaign-clean");
    let status = campaign(
        &stubs().join("harness.sh"),
        &work,
        &[target("one", &["fine"]), target("two", &["fine"])],
        &stubs().join("profdata.sh"),
        3,
    );
    assert_eq!(status, Ok(0));
}

#[test]
fn a_finding_ends_the_run_with_status_1() {
    let _lock = one_at_a_time();
    let work = scratch("campaign-finding");
    // The queue is taken from its end: the panic first, then the input that covers.
    let status = campaign(
        &stubs().join("harness.sh"),
        &work,
        &[target("one", &["fine", "PANIC with this"])],
        &stubs().join("profdata.sh"),
        2,
    );
    assert_eq!(status, Ok(1));
}

#[test]
fn a_run_that_executes_nothing_measured_nothing_and_fails() {
    let _lock = one_at_a_time();
    let work = scratch("campaign-nothing");
    let why = campaign(
        &stubs().join("harness.sh"),
        &work,
        &[target("stub", &["fine"])],
        &stubs().join("profdata.sh"),
        0,
    )
    .unwrap_err();
    assert!(
        why.contains("covered 0 regions: stub (0 executions)"),
        "{why}"
    );
}

#[test]
fn a_target_whose_every_run_failed_measured_nothing_and_fails() {
    let _lock = one_at_a_time();
    let work = scratch("campaign-all-fail");
    // A finding, and nothing covered: the floor still stops it. Its "1 finding"
    // would otherwise read as a clean measurement of the rest.
    let why = campaign(
        &stubs().join("harness.sh"),
        &work,
        &[target("stub", &["PANIC with this"])],
        &stubs().join("profdata.sh"),
        1,
    )
    .unwrap_err();
    assert!(
        why.contains("covered 0 regions: stub (1 executions)"),
        "{why}"
    );
}

// ---------------------------------------------------------------------------
// The command line
// ---------------------------------------------------------------------------

fn words(line: &str) -> Vec<String> {
    line.split_whitespace().map(String::from).collect()
}

#[test]
fn a_run_is_two_thousand_executions_a_target_unless_asked() {
    assert_eq!(options(&[]).unwrap().iterations, 2000);
    assert_eq!(options(&words("--iterations 7")).unwrap().iterations, 7);
}

#[test]
fn an_argument_it_does_not_know_is_an_error() {
    let why = options(&words("--iteration 7")).unwrap_err();
    assert!(why.contains("unknown argument \"--iteration\""), "{why}");
    assert!(why.contains(USAGE), "{why}");
}

#[test]
fn a_count_that_is_missing_or_no_number_is_an_error() {
    let why = options(&words("--iterations")).unwrap_err();
    assert!(why.contains("--iterations wants a count"), "{why}");
    let why = options(&words("--iterations 5k")).unwrap_err();
    assert!(why.contains("wants a whole number, not \"5k\""), "{why}");
}
