#!/usr/bin/env python3
"""Mutation controls for ADR-0149: `pw diff`, what a change means, for review.

Each mutant undoes one thing the report says, or one thing the model refuses:
- a new case of a sum type, a changed policy, a capability gained, a stream
  a page gained, each in its section;
- a program that does not check, compared anyway;
- a stream's query charged to its page's authority, which the first use of
  `pw diff` found.

A mutant must fail `semantic_diff.rs`, or `stream_plan.rs` for the contract.

Run from the repository root; `just e14-diffs` records the output. The
source is restored after every mutant, whatever happens.
"""

import pathlib
import re
import subprocess
import sys

ROOT = pathlib.Path(__file__).resolve().parent.parent
SEMANTIC = ROOT / "compiler/pw-core/src/semantic.rs"
CONTRACT = ROOT / "compiler/pw-core/src/contract.rs"

# (what, file, anchor, replacement)
MUTANTS = [
    (
        "a new case is not a domain change",
        SEMANTIC,
        "                        None => domain.push(format!(\"added `{case}` to `{t}`\")),\n",
        "                        None => {}\n",
    ),
    (
        "a changed policy is not reported",
        SEMANTIC,
        "                (Some(was), Some(v)) if was != v => format!(\"`{d}`: `{p}` is `{v}`, was `{was}`\"),\n",
        "                (Some(was), Some(v)) if was != v && false => format!(\"`{d}`: `{p}` is `{v}`, was `{was}`\"),\n",
    ),
    (
        "a capability gained is not reported",
        SEMANTIC,
        "        capabilities.push(format!(\"the node must now grant `{cap}`\"));\n",
        "        let _ = cap;\n",
    ),
    (
        "a stream a page gained is not reported",
        SEMANTIC,
        "            (\"streams\", &a_.streams, &b_.streams),\n",
        "            (\"streams\", &a_.streams, &a_.streams),\n",
    ),
    (
        "a program that does not check is compared",
        SEMANTIC,
        "        if !errors.is_empty() {\n            errors.truncate(3);\n",
        "        if false {\n            errors.truncate(3);\n",
    ),
    (
        "a stream's query is its page's authority",
        CONTRACT,
        "                                deferred.push(query);\n",
        "                                let _ = query;\n",
    ),
]

TESTS = [
    ["cargo", "test", "--quiet", "--locked", "-p", "pw-core", "--test", "semantic_diff"],
    ["cargo", "test", "--quiet", "--locked", "-p", "pw-core", "--test", "stream_plan"],
]


def run_tests():
    """(built, passed, failed) over every test command."""
    built, passed, failed = True, 0, 0
    for cmd in TESTS:
        r = subprocess.run(cmd, cwd=ROOT, capture_output=True, text=True)
        m = re.search(r"test result: \w+\. (\d+) passed; (\d+) failed", r.stdout + r.stderr)
        if m is None:
            built = False
            continue
        passed += int(m.group(1))
        failed += int(m.group(2))
    return built, passed, failed


def main():
    built, passed, failed = run_tests()
    print(f"baseline: {passed} passed, {failed} failed")
    if not built or failed or not passed:
        print("FAIL: the unmutated baseline is not green; no mutant can mean anything")
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
        print(f"{what}: {verdict}")

    print(f"{len(MUTANTS) - survivors} of {len(MUTANTS)} mutants killed")
    return 1 if survivors else 0


if __name__ == "__main__":
    sys.exit(main())
