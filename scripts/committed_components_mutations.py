#!/usr/bin/env python3
"""Mutation controls for the committed components' freshness (train-11, the
integrator's ruling of 2026-10-19): every component the store's build emits
is committed in `docs/evidence/E10/`, byte for byte, and none it no longer
emits; the list is the build's, never one kept beside it.

Each mutant undoes one part of the check
(`compiler/pw-core/tests/evidence_is_current.rs`): the build's components
not read; a built component not committed, one committed stale, or one
committed that the build no longer emits passing; and a kept few of the
build's components checked, as the 14 kept by hand were until train-11.

A mutant must fail `evidence_is_current`'s tests: the committed components
against the store's build, and the control, a component the build adds,
which fails the check until it is committed current. Each command's result
is read by `cargo_test_results` (ADR-0352): a command that does not build,
is stopped, or says something inconsistent is no kill, and stops the run.

Run from the repository root; `just e14-committed-components` records the
output. The source is restored after every mutant, whatever happens.
"""

import os
import pathlib
import signal
import subprocess
import sys

from cargo_test_results import parse as cargo_result

ROOT = pathlib.Path(__file__).resolve().parent.parent
TEST = ROOT / "compiler/pw-core/tests/evidence_is_current.rs"

# (what, file, anchor, replacement)
MUTANTS = [
    (
        "the build's components are not read",
        TEST,
        "            pw_core::backend::component::Built::Component { compiled, .. } => {\n"
        "                Some((id.clone(), compiled.component.bytes.clone()))\n"
        "            }\n",
        "            pw_core::backend::component::Built::Component { .. } => None,\n",
    ),
    (
        "a component built and not committed passes",
        TEST,
        "            Err(_) => problems.push(format!(\"{id}: built, not committed\")),\n",
        "            Err(_) => {}\n",
    ),
    (
        "a component committed stale passes",
        TEST,
        "            Ok(committed) if committed != *bytes => problems.push(format!(\n",
        "            Ok(committed) if committed != *bytes && false => problems.push(format!(\n",
    ),
    (
        "a component committed that the build no longer emits passes",
        TEST,
        "        if !emitted.contains(id.as_str()) {\n",
        "        if !emitted.contains(id.as_str()) && false {\n",
    ),
    (
        "a kept few of the build's components are checked",
        TEST,
        "    let problems = components_against(&built, &root.join(\"docs/evidence/E10\"));\n",
        "    let problems = components_against(&built[..14], &root.join(\"docs/evidence/E10\"));\n",
    ),
]

TESTS = ["cargo", "test", "--quiet", "--locked", "-p", "pw-core", "--test", "evidence_is_current"]

# How long one command may run. Past the bound it is killed with everything
# it started, and the run stops: a command stopped is no kill.
BOUND = 1200


def bounded(cmd):
    """(output, returncode), or (output, None) when it ran past the bound."""
    p = subprocess.Popen(cmd, cwd=ROOT, start_new_session=True,
                         stdout=subprocess.PIPE, stderr=subprocess.STDOUT, text=True)
    try:
        out, _ = p.communicate(timeout=BOUND)
        return out, p.returncode
    except subprocess.TimeoutExpired:
        os.killpg(p.pid, signal.SIGKILL)
        out, _ = p.communicate()
        return out, None


def cargo_tests():
    """(valid, passed, failed) over the tests (`cargo_test_results`)."""
    out, code = bounded(TESTS)
    result = cargo_result(code, out)
    if not result.valid:
        return False, 0, 0
    return True, result.passed, result.failed


def main():
    import mutation_baseline  # keeps what each command says, for a red baseline
    watch = mutation_baseline.mutation_bound.start()
    # Stopped from outside, the source is still restored: SIGTERM raises
    # here, and the `finally` below runs.
    signal.signal(signal.SIGTERM, lambda *_: sys.exit(1))
    built, passed, failed = cargo_tests()
    print(f"baseline: {passed} passed, {failed} failed", flush=True)
    if not built or failed or not passed or watch.stopped:
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
            built, passed, failed = cargo_tests()
        finally:
            path.write_text(original)
        if not built or watch.stopped:
            print(f"{what}: REFUSED (invalid or stopped Cargo producer; no kill)", flush=True)
            mutation_baseline.explain()
            return 1
        if failed:
            verdict = f"KILLED ({failed} of {passed + failed} tests fail)"
        else:
            verdict = "SURVIVED"
            survivors += 1
        print(f"{what}: {verdict}", flush=True)

    print(f"{len(MUTANTS) - survivors} of {len(MUTANTS)} mutants killed")
    return 1 if survivors else 0


if __name__ == "__main__":
    sys.exit(main())
