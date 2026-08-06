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

use std::collections::BTreeSet;
use std::path::{Path, PathBuf};
use std::process::Command;

/// One fuzz target: a name, and the harness binary that runs one input.
struct Target {
    name: &'static str,
    /// Seeds, from the deployment matrix and the corpus.
    seeds: Vec<Vec<u8>>,
}

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let budget: usize = args
        .iter()
        .position(|a| a == "--iterations")
        .and_then(|i| args.get(i + 1))
        .and_then(|n| n.parse().ok())
        .unwrap_or(2000);

    let root = repo_root();
    let work = root.join("target/fuzz");
    std::fs::create_dir_all(&work).expect("work dir");

    // Build the instrumented harness once. Every target runs through it, with
    // the target name as its first argument.
    eprintln!("building the instrumented harness...");
    let harness = build_instrumented(&root, &work);

    let targets = targets(&root);
    let mut report = String::new();
    let mut total_execs = 0usize;
    let mut total_findings = 0usize;

    for t in &targets {
        let (execs, regions, findings) = fuzz_one(&harness, &work, t, budget);
        total_execs += execs;
        total_findings += findings.len();
        report.push_str(&format!(
            "  {:28} {execs:>7} execs  {regions:>6} covered regions  {} finding(s)\n",
            t.name,
            findings.len()
        ));
        for f in &findings {
            report.push_str(&format!("      FINDING: {f}\n"));
        }
    }

    println!("coverage-guided fuzzing");
    println!("  toolchain: rustc 1.97.1 stable, -C instrument-coverage, llvm-profdata");
    println!("  NOT libFuzzer: a process per input, so executions/second are low");
    println!();
    print!("{report}");
    println!();
    println!(
        "  {} targets, {total_execs} executions, {total_findings} findings",
        targets.len()
    );

    if total_findings > 0 {
        std::process::exit(1);
    }
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
fn fuzz_one(
    harness: &Path,
    work: &Path,
    target: &Target,
    budget: usize,
) -> (usize, usize, Vec<String>) {
    let dir = work.join(target.name);
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).expect("target dir");

    let mut corpus: Vec<Vec<u8>> = if target.seeds.is_empty() {
        vec![b"seed".to_vec()]
    } else {
        target.seeds.clone()
    };
    let mut seen_regions: BTreeSet<u64> = BTreeSet::new();
    let mut findings = Vec::new();
    let mut rng = Rng(0x5EED_1234);
    let mut execs = 0usize;

    // Every seed first, so the corpus starts from known-interesting inputs.
    let mut queue: Vec<Vec<u8>> = corpus.clone();

    while execs < budget {
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
        std::fs::write(&input_path, &input).expect("write input");

        let out = Command::new(harness)
            .arg(target.name)
            .arg(&input_path)
            .env("LLVM_PROFILE_FILE", &profraw)
            .output()
            .expect("run harness");

        // A crash is a finding. The harness returns 101 for a panic it caught
        // and reported; anything else non-zero is an uncaught abort.
        if !out.status.success() {
            let why = String::from_utf8_lossy(&out.stderr);
            let finding = format!(
                "{} on {} bytes: {}",
                if out.status.code() == Some(101) {
                    "panic"
                } else {
                    "abort"
                },
                input.len(),
                why.lines().next().unwrap_or("<no message>")
            );
            if !findings.contains(&finding) {
                let saved = dir.join(format!("finding-{}.bin", findings.len()));
                std::fs::write(&saved, &input).expect("save finding");
                findings.push(finding);
            }
            continue;
        }

        // Coverage feedback: did this input reach regions the corpus has not?
        let new = regions(&profraw, harness)
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

    (execs, seen_regions.len(), findings)
}

/// Covered region hashes, from `llvm-profdata show`.
fn regions(profraw: &Path, _harness: &Path) -> Vec<u64> {
    let llvm = llvm_bin("llvm-profdata");
    let out = Command::new(llvm)
        .args(["show", "--counts", "--all-functions"])
        .arg(profraw)
        .output();
    let Ok(out) = out else { return Vec::new() };
    if !out.status.success() {
        return Vec::new();
    }
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
    let text = String::from_utf8_lossy(&out.stdout);
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

fn llvm_bin(tool: &str) -> PathBuf {
    let home = std::env::var("HOME").expect("HOME");
    PathBuf::from(home)
        .join(".rustup/toolchains/1.97.1-aarch64-apple-darwin/lib/rustlib/aarch64-apple-darwin/bin")
        .join(tool)
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
