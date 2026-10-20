#!/usr/bin/env python3
"""A superseded keyed read does not retry; a current read still revalidates.

Only complete Cargo results for the two selected witnesses earn credit.
Build errors, interrupted commands and missing witnesses fail the campaign.
"""
import os
import pathlib
import signal
import subprocess
import sys

import cargo_test_results
import mutation_source

ROOT = pathlib.Path(__file__).resolve().parent.parent
SERVER = ROOT / "spikes/own-renderer/server/src/main.rs"
MUTANTS = [
    ("a superseded keyed read retries after newer frames", SERVER,
     "            if self.superseded(session, binding, seq, document, cancel) {\n",
     "            if false && self.superseded(session, binding, seq, document, cancel) {\n"),
    ("a current keyed read ignores a commit received while reading", SERVER,
     "            if !last && sent(self) != before {\n",
     "            if false && !last && sent(self) != before {\n"),
]
NAMES = [
    "tests::a_flight_another_reader_waits_for_is_not_stopped",
    "tests::a_longer_read_is_not_applied_over_a_commit_it_did_not_see",
]
COMMAND = ["cargo", "test", "--quiet", "--locked", "-p", "pw-dev-server",
           "--bin", "pw-dev-server", "--", "--exact", *NAMES]


def run():
    child = subprocess.Popen(COMMAND, cwd=ROOT, stdout=subprocess.PIPE,
                             stderr=subprocess.STDOUT, text=True, start_new_session=True)
    try:
        output, _ = child.communicate(timeout=600)
    except BaseException:
        os.killpg(child.pid, signal.SIGKILL)
        child.communicate()
        raise
    result = cargo_test_results.parse(child.returncode, output)
    if (not result.valid or result.passed + result.failed != len(NAMES)
            or len(result.failure_names) != result.failed
            or not set(result.failure_names) <= set(NAMES)):
        print(output, flush=True)
        print(f"INVALID RESULT: {result}", flush=True)
        return None
    if result.failed:
        # These witnesses fail by equality assertion, not by compilation,
        # timeouts, gate failures or setup/teardown panics.
        sections = output.split("---- ")[1:]
        assertions = {
            section.split(" stdout ----", 1)[0]
            for section in sections
            if "assertion `left == right` failed" in section
        }
        if not set(result.failure_names) <= assertions:
            print(output, flush=True)
            print("INVALID RESULT: a witness failed without its equality assertion", flush=True)
            return None
    return result


def main():
    import mutation_baseline
    signal.signal(signal.SIGTERM, lambda *_: sys.exit(1))
    baseline = run()
    if baseline is None or baseline.failed:
        print("FAIL: every selected baseline witness must pass", flush=True)
        return 1
    print(f"baseline: {baseline.passed} passed, 0 failed", flush=True)
    killed = 0
    for what, path, anchor, replacement in MUTANTS:
        original = path.read_text()
        if original.count(anchor) != 1:
            print(f"{what}: ANCHOR NOT FOUND EXACTLY ONCE", flush=True)
            return 1
        with mutation_source.changed(path, original, original.replace(anchor, replacement, 1)):
            result = run()
        if result is None:
            print(f"{what}: INVALID RESULT (not a kill)", flush=True)
            return 1
        if result.failed:
            killed += 1
            print(f"{what}: KILLED ({result.failed} assertion failures)", flush=True)
        else:
            print(f"{what}: SURVIVED", flush=True)
    print(f"{killed} of {len(MUTANTS)} mutants killed", flush=True)
    return int(killed != len(MUTANTS))


if __name__ == "__main__":
    sys.exit(main())
