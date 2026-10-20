//! `pw-fuzz` run as a person runs it, on what must stop it before it builds
//! anything: with no `llvm-profdata` to read coverage with it exits 2, naming
//! what is missing, and prints no report of zeros.
//!
//! (Before 2026-10-20 it printed "0 covered regions ... 0 finding(s)" for every
//! target, and exited 0.)

use std::fs::File;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::time::{Duration, Instant};

struct Ran {
    status: Option<i32>,
    stdout: String,
    stderr: String,
}

/// A fresh empty directory of this test's own.
fn scratch(name: &str) -> PathBuf {
    let dir = Path::new(env!("CARGO_TARGET_TMPDIR")).join(name);
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    dir
}

/// Runs `pw-fuzz` with `args`, and `PW_LLVM_BIN_DIR` set to `bin_dir` or unset,
/// but not for long: a run that goes past its checks goes on to build the
/// instrumented harness, which is a failure of the test and not a wait of
/// minutes.
fn pw_fuzz(name: &str, args: &[&str], bin_dir: Option<&Path>) -> Ran {
    let dir = scratch(name);
    let (out, err) = (dir.join("stdout"), dir.join("stderr"));
    let mut command = Command::new(env!("CARGO_BIN_EXE_pw-fuzz"));
    command
        .args(args)
        .stdout(File::create(&out).unwrap())
        .stderr(File::create(&err).unwrap());
    match bin_dir {
        Some(dir) => command.env("PW_LLVM_BIN_DIR", dir),
        None => command.env_remove("PW_LLVM_BIN_DIR"),
    };
    let mut child = command.spawn().unwrap();
    let began = Instant::now();
    let status = loop {
        if let Some(status) = child.try_wait().unwrap() {
            break status;
        }
        if began.elapsed() > Duration::from_secs(120) {
            let _ = child.kill();
            let _ = child.wait();
            panic!(
                "pw-fuzz {args:?} still ran after 120 s: it went on to build, not stopped at its checks"
            );
        }
        std::thread::sleep(Duration::from_millis(20));
    };
    Ran {
        status: status.code(),
        stdout: std::fs::read_to_string(out).unwrap(),
        stderr: std::fs::read_to_string(err).unwrap(),
    }
}

#[test]
fn without_llvm_profdata_it_stops_and_says_what_is_missing() {
    let empty = scratch("no-tool-dir");
    let ran = pw_fuzz("no-tool", &["--iterations", "1"], Some(&empty));
    assert_eq!(ran.status, Some(2), "{}{}", ran.stdout, ran.stderr);
    assert!(
        ran.stderr.contains("llvm-profdata not found"),
        "{}",
        ran.stderr
    );
    assert!(
        ran.stderr.contains(&empty.display().to_string()),
        "{}",
        ran.stderr
    );
    assert!(
        ran.stderr.contains("rustup component add llvm-tools"),
        "{}",
        ran.stderr
    );
    assert!(
        !ran.stdout.contains("covered regions"),
        "a report of zeros: {}",
        ran.stdout
    );
}

#[test]
fn an_argument_it_does_not_know_stops_it() {
    let ran = pw_fuzz("unknown-argument", &["--iteration", "5"], None);
    assert_eq!(ran.status, Some(2), "{}{}", ran.stdout, ran.stderr);
    assert!(
        ran.stderr.contains("unknown argument \"--iteration\""),
        "{}",
        ran.stderr
    );
}

#[test]
fn a_target_that_does_not_exist_stops_it_and_lists_the_ones_that_do() {
    let ran = pw_fuzz("unknown-target", &["--only", "parser-to-hir,nope"], None);
    assert_eq!(ran.status, Some(2), "{}{}", ran.stdout, ran.stderr);
    assert!(ran.stderr.contains("no target \"nope\""), "{}", ran.stderr);
    assert!(
        ran.stderr
            .contains("the targets are parser-to-hir, resolution-to-typing,"),
        "{}",
        ran.stderr
    );
}
