#!/usr/bin/env python3
"""Exact compatibility authority controls, with frozen native unittest receipts."""
import ast
from pathlib import Path
import sys
import tempfile

import guide_python
import mutation_baseline
import mutation_source

ROOT = Path(__file__).resolve().parent.parent
SOURCE = ROOT / "scripts/todomvc_cypress_compat.py"
SELECTOR = "test_todomvc_cypress_compat"
TESTS = ROOT / "scripts/tests"
# (fault, source, exact anchor, replacement); witnesses below are frozen too.
MUTANTS = [
    ("changed original blobs are accepted", SOURCE,
     "        if upstream.blob(data) != expected:\n", "        if False:\n"),
    ("installed package and binary pins are ignored", SOURCE,
     "        if version != upstream.CYPRESS:\n", "        if False:\n"),
    ("declared package pin is ignored", SOURCE,
     '    if json.loads(package).get("devDependencies", {}).get("cypress") != upstream.CYPRESS:\n',
     "    if False:\n"),
    ("patch identity can change without a new version", SOURCE,
     "    if digest(patches) != PATCHSET_SHA256:\n", "    if False:\n"),
    ("existing destination paths are no longer refused", SOURCE,
     "    destination.mkdir()\n", "    destination.mkdir(exist_ok=True)\n"),
    ("patched-output digest guard is bypassed", SOURCE,
     "        if digest(files[name]) != wanted:\n", "        if False:\n"),
    ("extra staged specs are accepted", SOURCE,
     "    if actual != set(files):\n", "    if False:\n"),
    ("staged content and manifest drift are accepted", SOURCE,
     "        if regular(destination / name) != data:\n", "        if False:\n"),
]
WITNESSES = [
    "test_each_original_drift_or_missing_file_refuses_before_creation",
    "test_package_and_binary_versions_must_be_exact",
    "test_declared_pin_drift_is_refused",
    "test_patch_identity_cannot_silently_change_under_v1",
    "test_recheck_catches_source_drift_and_never_overwrites",
    "test_output_identity_cannot_silently_change_under_v1",
    "test_staged_added_missing_and_symlink_files_are_refused",
    "test_staged_file_and_manifest_drift_are_refused",
]


def main():
    watch = mutation_baseline.mutation_bound.start()
    evidence = Path(tempfile.mkdtemp(prefix="pw-todomvc-compat-controls-"))
    expected = guide_python.freeze(SELECTOR, TESTS, SELECTOR + ".py", 11, evidence)
    baseline = guide_python.run(SELECTOR, expected, TESTS, evidence)
    print(baseline.output, flush=True)
    if not baseline.result.valid or baseline.result.failed or watch.stopped:
        print("REFUSED: incomplete or failed baseline")
        return 1
    print(f"baseline: {len(expected)} passed, 0 failed", flush=True)
    killed = 0
    for (name, path, anchor, replacement), method in zip(MUTANTS, WITNESSES, strict=True):
        original = path.read_text()
        witness = SELECTOR + ".Compatibility." + method
        if original.count(anchor) != 1 or witness not in expected:
            print(f"REFUSED: {name}: authority or frozen witness changed")
            return 1
        mutant = original.replace(anchor, replacement, 1)
        ast.parse(mutant)
        with mutation_source.changed(path, original, mutant):
            run = guide_python.run(SELECTOR, expected, TESTS, evidence)
        print(run.output, flush=True)
        if not run.result.valid or run.result.body_errors or watch.stopped:
            print(f"REFUSED: {name}: invalid or stopped producer; no kill")
            return 1
        if witness in run.result.assertion_failed:
            killed += 1
            print(f"{name}: KILLED by {witness}", flush=True)
        else:
            print(f"{name}: SURVIVED", flush=True)
    print(f"{killed} of {len(MUTANTS)} mutants killed", flush=True)
    return int(killed != len(MUTANTS))


if __name__ == "__main__":
    sys.exit(main())
