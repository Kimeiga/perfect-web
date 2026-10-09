#!/usr/bin/env python3
"""Mutation controls for ADR-0294: a list renders in its length.

Each mutant undoes one piece:
- an item's scope sharing the page's values, by pointer;
- an item's scope sharing the page's fragments.

The tests must then fail: `pw-render`'s scope test (`scopes`) and
`list_scale.rs`, its timing among them (`--include-ignored`).

Run from the repository root; `just e14-shared-scopes` records the output.
The source is restored after every mutant, whatever happens.
"""

import os
import pathlib
import re
import signal
import subprocess
import sys

ROOT = pathlib.Path(__file__).resolve().parent.parent
RENDER = ROOT / "runtime/pw-render/src/lib.rs"

MUTANTS = [
    (
        "an item's scope copies the page's values",
        RENDER,
        "        let mut next = self.clone();\n        next.values.insert(name.to_string(), Arc::new(value));\n",
        "        let mut next = self.clone();\n"
        "        next.values = self.values.iter().map(|(k, v)| (k.clone(), Arc::new((**v).clone()))).collect();\n"
        "        next.values.insert(name.to_string(), Arc::new(value));\n",
    ),
    (
        "an item's scope copies the page's fragments",
        RENDER,
        "        let mut next = self.clone();\n        next.values.insert(name.to_string(), Arc::new(value));\n",
        "        let mut next = self.clone();\n"
        "        next.materialized = Arc::new((*self.materialized).clone());\n"
        "        next.values.insert(name.to_string(), Arc::new(value));\n",
    ),
]

TESTS = [
    ["cargo", "test", "--quiet", "--locked", "-p", "pw-render", "--lib", "scopes"],
    ["cargo", "test", "--quiet", "--locked", "-p", "pw-render", "--test", "list_scale",
     "--", "--include-ignored"],
]

# How long one command may run. Past it, it and what it started are stopped.
BOUND = 1800


def run_tests():
    """(built, passed, failed), over every command."""
    built, passed, failed = True, 0, 0
    for command in TESTS:
        p = subprocess.Popen(
            command, cwd=ROOT, stdout=subprocess.PIPE, stderr=subprocess.STDOUT, text=True,
            start_new_session=True,
        )
        try:
            out, _ = p.communicate(timeout=BOUND)
        except subprocess.TimeoutExpired:
            os.killpg(p.pid, signal.SIGKILL)
            out, _ = p.communicate()
        results = re.findall(r"test result: \w+\. (\d+) passed; (\d+) failed", out)
        if not results:
            built = False
            continue
        passed += sum(int(a) for a, _ in results)
        failed += sum(int(b) for _, b in results)
    return built, passed, failed


def main():
    import mutation_baseline  # keeps what each command says, for a red baseline
    # Stopped from outside, the source is still restored: a stop is an
    # exception here, which the `finally` below meets.
    signal.signal(signal.SIGTERM, lambda *_: sys.exit(143))
    built, passed, failed = run_tests()
    print(f"baseline: {passed} passed, {failed} failed", flush=True)
    if not built or failed or not passed:
        print("FAIL: the unmutated baseline is not green; no mutant can mean anything")
        mutation_baseline.explain()
        return 1

    survivors = 0
    for what, path, anchor, replacement in MUTANTS:
        original = path.read_text()
        if original.count(anchor) != 1:
            print(f"{what}: ANCHOR NOT FOUND EXACTLY ONCE in {path.name}")
            survivors += 1
            continue
        try:
            path.write_text(original.replace(anchor, replacement, 1))
            built, passed, failed = run_tests()
        finally:
            path.write_text(original)
        if not built:
            verdict = "KILLED (does not build)"
        elif failed:
            verdict = f"KILLED ({failed} of {passed + failed} tests fail)"
        else:
            verdict = "SURVIVED"
            survivors += 1
        print(f"{what}: {verdict}", flush=True)

    print(f"{len(MUTANTS) - survivors} of {len(MUTANTS)} mutants killed")
    return 1 if survivors else 0


if __name__ == "__main__":
    sys.exit(main())
