#!/usr/bin/env python3
"""Real SQLite cache publication/lifetime controls, with no external data.

Each control runs the whole private cache test module. A compiler error,
missing/empty test result, timeout or vanished test count is a refusal,
never an assertion kill. Every applied source is restored in finally.
"""
from pathlib import Path
import re
import subprocess
import sys

ROOT = Path(__file__).resolve().parent.parent
SEARCH = ROOT / "spikes/own-renderer/server/src/kiokun/search.rs"
TEST = "kiokun::search::cache_tests"
COUNT = 10
MUTANTS = [
    ("builders share one staging directory", SEARCH,
     'let name: String = random.iter().map(|b| format!("{b:02x}")).collect();',
     'let name = "shared".to_string();'),
    ("failed and losing builders leave their own stages", SEARCH,
     "if !self.published {", "if false {"),
    ("failure cleanup removes other builders and winners", SEARCH,
     "let _ = std::fs::remove_dir_all(&self.dir);",
     "for entry in std::fs::read_dir(self.dir.parent().unwrap()).unwrap().flatten() { let _ = std::fs::remove_dir_all(entry.path()); }"),
    ("a delayed publisher replaces the complete winner", SEARCH,
     "            match std::fs::rename(&stage.dir, &directory) {",
     "            if directory.exists() { std::fs::remove_dir_all(&directory).unwrap(); }\n            match std::fs::rename(&stage.dir, &directory) {"),
    ("stored fingerprint is not compared to the input", SEARCH,
     "if actual != expected {", "if false {"),
    ("stored FTS schema is not checked", SEARCH,
     "if schema != SCHEMA {", "if false {"),
    ("the losing publisher skips SQLite winner validation", SEARCH,
     "let winner = open_cached(&directory, &expected)",
     'let winner = Connection::open_with_flags(directory.join("index.db"), rusqlite::OpenFlags::SQLITE_OPEN_READ_ONLY).map_err(|e| e.to_string())'),
    ("published connections are writable", SEARCH,
     "Connection::open_with_flags(&file, rusqlite::OpenFlags::SQLITE_OPEN_READ_ONLY)",
     "Connection::open_with_flags(&file, rusqlite::OpenFlags::SQLITE_OPEN_READ_WRITE)"),
    ("a new version prunes live versions and builders", SEARCH,
     "        Ok(Index {\n            conn: Mutex::new(open_cached(&directory, &expected)?),\n        })",
     "        for entry in std::fs::read_dir(cache).unwrap().flatten() { let path = entry.path(); if path != directory { let _ = std::fs::remove_dir_all(&path); let _ = std::fs::remove_file(&path); } }\n        Ok(Index {\n            conn: Mutex::new(open_cached(&directory, &expected)?),\n        })"),
    ("small inputs create disk cache artifacts", SEARCH,
     "if (bytes.len() as u64) < IN_MEMORY {", "if false {"),
]


def run():
    result = subprocess.run(
        ["cargo", "test", "--quiet", "--locked", "-p", "pw-dev-server",
         "--bin", "pw-dev-server", TEST, "--", "--nocapture", "--test-threads=4"],
        cwd=ROOT, capture_output=True, text=True,
    )
    output = result.stdout + result.stderr
    summaries = re.findall(r"test result: (?:ok|FAILED)\. (\d+) passed; (\d+) failed", output)
    if len(summaries) != 1:
        return None, output
    passed, failed = map(int, summaries[0])
    if passed + failed != COUNT or (result.returncode == 0) != (failed == 0):
        return None, output
    return (passed, failed), output


def main():
    import mutation_baseline  # source freshness, bounded processes and diagnostics
    baseline, output = run()
    if baseline != (COUNT, 0):
        print("FAIL: private SQLite cache baseline is not green")
        print(output)
        mutation_baseline.explain()
        return 1
    print(f"baseline: {COUNT} passed, 0 failed", flush=True)
    logs = ROOT / "work/kiokun-cache-mutants"
    logs.mkdir(parents=True, exist_ok=True)
    killed = 0
    for number, (label, path, anchor, replacement) in enumerate(MUTANTS, 1):
        original = path.read_text()
        if original.count(anchor) != 1:
            print(f"{label}: ANCHOR NOT FOUND EXACTLY ONCE", flush=True)
            continue
        try:
            path.write_text(original.replace(anchor, replacement, 1))
            outcome, output = run()
        finally:
            path.write_text(original)
        (logs / f"{number:02}.txt").write_text(output)
        if outcome is not None and outcome[1] > 0:
            killed += 1
            print(f"{label}: KILLED ({outcome[1]} of {COUNT} tests fail)", flush=True)
        elif outcome is None:
            print(f"{label}: REFUSED (no complete built assertion result)", flush=True)
            print(output, flush=True)
        else:
            print(f"{label}: SURVIVED", flush=True)
    print(f"{killed} of {len(MUTANTS)} mutants killed", flush=True)
    return 0 if killed == len(MUTANTS) else 1


if __name__ == "__main__":
    sys.exit(main())
