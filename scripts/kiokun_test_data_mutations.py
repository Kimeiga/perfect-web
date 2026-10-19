#!/usr/bin/env python3
"""Unit-server sample selection and real deployment configuration stay distinct."""
from pathlib import Path
import re
import subprocess
import sys

ROOT = Path(__file__).resolve().parent.parent
MAIN = ROOT / "spikes/own-renderer/server/src/main.rs"
KIOKUN = ROOT / "spikes/own-renderer/server/src/kiokun.rs"
TEST = "tests::kiokun::a_unit_test_server_uses_the_sample_despite_deployment_paths"
MUTANTS = [
    (
        "a unit-test server reads the deployment dataset",
        MAIN,
        "            let kiokun = kiokun::KiokunData::at(kiokun::sample(), None);\n",
        "            let kiokun = kiokun::KiokunData::new()?;\n",
    ),
    (
        "the deployment loader ignores its configured label directory",
        KIOKUN,
        '        let app = match std::env::var_os("KIOKUN_APP") {\n',
        "        let app = match None::<std::ffi::OsString> {\n",
    ),
]


def run():
    result = subprocess.run(
        ["cargo", "test", "--quiet", "--locked", "-p", "pw-dev-server",
         "--bin", "pw-dev-server", "--", "--exact", TEST, "--nocapture"],
        cwd=ROOT, capture_output=True, text=True,
    )
    output = result.stdout + result.stderr
    summaries = re.findall(r"test result: (?:ok|FAILED)\. (\d+) passed; (\d+) failed", output)
    if not summaries:
        return None, output
    passed, failed = map(int, summaries[-1])
    if (result.returncode == 0) != (failed == 0) or passed + failed != 1:
        return None, output
    return (passed, failed), output


def main():
    import mutation_baseline  # source freshness, process bounds and refusal diagnostics
    baseline, output = run()
    if baseline != (1, 0):
        print("FAIL: unmutated sample-selection baseline is not green")
        print(output)
        mutation_baseline.explain()
        return 1
    print("baseline: 1 passed, 0 failed", flush=True)
    killed = 0
    for label, path, anchor, replacement in MUTANTS:
        original = path.read_text()
        if original.count(anchor) != 1:
            print(f"{label}: ANCHOR NOT FOUND EXACTLY ONCE")
            continue
        try:
            path.write_text(original.replace(anchor, replacement, 1))
            outcome, output = run()
        finally:
            path.write_text(original)
        if outcome == (0, 1):
            killed += 1
            print(f"{label}: KILLED (1 of 1 tests fail)", flush=True)
        elif outcome is None:
            print(f"{label}: REFUSED (no assertion-failure result)", flush=True)
            print(output)
        else:
            print(f"{label}: SURVIVED", flush=True)
    print(f"{killed} of {len(MUTANTS)} mutants killed")
    return 0 if killed == len(MUTANTS) else 1


if __name__ == "__main__":
    sys.exit(main())
