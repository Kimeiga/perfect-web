#!/usr/bin/env python3
"""Mutation controls for ADR-0121: an idempotent command runs once per
interaction.

Each mutant undoes one piece: the contract carrying `idempotent_by`, the host
running a keyed command through the reservation, refusing a request without
an interaction, refusing an interaction reused with other arguments, bounding
what is kept, and keeping an unknown outcome. The tests must then fail.

Run from the repository root; `just e14-idempotent-commands` records the
output. The source is restored after every mutant, whatever happens.
"""

import pathlib
import re
import subprocess
import sys

ROOT = pathlib.Path(__file__).resolve().parent.parent
CONTRACT = ROOT / "compiler/pw-core/src/contract.rs"
SERVER = ROOT / "spikes/own-renderer/server/src/main.rs"
RESOURCE = ROOT / "runtime/pw-resource/src/lib.rs"

# (what is undone, file, anchor, replacement)
MUTANTS = [
    (
        "the contract does not carry idempotent_by",
        CONTRACT,
        "                .map(|p| p.value.trim().to_string());\n",
        "                .map(|p| p.value.trim().to_string()).filter(|_| false);\n",
    ),
    (
        "the host runs a keyed command every time",
        SERVER,
        "        let Some(key_type) = &export.idempotent_by else {\n",
        "        let Some(key_type) = &None::<String> else {\n",
    ),
    (
        "a request without an interaction runs unkeyed",
        SERVER,
        "        let id = interaction.ok_or_else(|| {\n",
        "        let id = interaction.or(Some(\"anonymous\")).ok_or_else(|| {\n",
    ),
    (
        "an interaction reused with other arguments shares the first outcome",
        SERVER,
        "                Some((_, first)) if *first != sent => {\n",
        "                Some((_, first)) if *first != sent && false => {\n",
    ),
    (
        "every interaction is kept",
        SERVER,
        "                    while queue.len() > INTERACTIONS_PER_SESSION {\n",
        "                    while queue.len() > usize::MAX {\n",
    ),
    (
        "an unknown outcome is forgotten",
        RESOURCE,
        "            .is_some_and(|c| matches!(c.get(), Some(Ok(_))));\n",
        "            .is_some_and(|c| c.get().is_some());\n",
    ),
]

TESTS = [
    ["cargo", "test", "--quiet", "--locked", "-p", "pw-core", "--test", "component_contract"],
    ["cargo", "test", "--quiet", "--locked", "-p", "pw-resource", "--test", "concurrency_regressions"],
    ["cargo", "test", "--quiet", "--locked", "-p", "pw-dev-server", "--", "interaction"],
]


def run_tests():
    """(built, passed, failed) over every test command."""
    built, passed, failed = True, 0, 0
    for cmd in TESTS:
        r = subprocess.run(cmd, cwd=ROOT, capture_output=True, text=True)
        out = r.stdout + r.stderr
        found = re.findall(r"test result: \w+\. (\d+) passed; (\d+) failed", out)
        if not found:
            built = False
            continue
        for p, f in found:
            passed += int(p)
            failed += int(f)
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
