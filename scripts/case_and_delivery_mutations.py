#!/usr/bin/env python3
"""Mutation controls for ADR-0153 and ADR-0154: two forms gate item 5 found
open, closed.

Each mutant undoes one piece:
- an `{#if}` condition is not read for a case test;
- an `{:else if}` condition is not;
- a case on the left of `==` is not;
- a command a page's handler calls is not found;
- an `idempotent_by` is not asked of it.

A mutant must fail `case_and_delivery.rs`.

Run from the repository root; `just e14-case-and-delivery` records the
output. The source is restored after every mutant, whatever happens.
"""

import pathlib
import os
import re
import signal
import subprocess
import sys

import cargo_test_results
import mutation_source

ROOT = pathlib.Path(__file__).resolve().parent.parent
CHECK = ROOT / "compiler/pw-core/src/check.rs"
RESUME = ROOT / "compiler/pw-core/src/resume.rs"

# (what, file, anchor, replacement)
MUTANTS = [
    (
        "an {#if} that tests a case is not refused",
        CHECK,
        "                        case_test(body, sigs, &types, sigs.unit_of(hir.module_of(id)), c)\n",
        "                        case_test(body, sigs, &types, sigs.unit_of(hir.module_of(id)), c).filter(|_| false)\n",
    ),
    (
        "an {:else if} condition is not read",
        CHECK,
        "                        Node::Branch { condition, .. } => *condition,\n",
        "                        Node::Branch { .. } => None,\n",
    ),
    (
        "a case written on the left is not seen",
        CHECK,
        "    for (value, other) in [(*lhs, *rhs), (*rhs, *lhs)] {\n",
        "    for (value, other) in [(*lhs, *rhs)] {\n",
    ),
    (
        "a qualified case's qualifier is not checked",
        CHECK,
        # ADR-0351 types qualified cases through the canonical value solver.
        # Their identity is now compared here; the untyped fallback is no
        # longer reached by this witness. Keep the same qualifier fault.
        "            (None, None) => x.def_id().is_some() && x.def_id() == y.def_id(),\n",
        "            (None, None) => x.def_id().is_some(),\n",
    ),
    (
        "a command a handler calls is not found",
        RESUME,
        "                    if sigs.kind_of(def) == Some(crate::hir::DeclKind::Command) {\n",
        "                    if sigs.kind_of(def) == Some(crate::hir::DeclKind::Command) && false {\n",
    ),
    (
        "a sent command is not asked for idempotent_by",
        CHECK,
        "        if decl.kind != hir::DeclKind::Command || decl.policy(\"idempotent_by\").is_some() {\n",
        "        if decl.kind != hir::DeclKind::Command || decl.policy(\"idempotent_by\").is_none() {\n",
    ),
]

TESTS = [
    ["cargo", "test", "--quiet", "--locked", "-p", "pw-core", "--test", "case_and_delivery", "--", "--format=pretty"],
]

NAMES = {
    "a_template_tests_a_case_with_match",
    "every_case_named_by_a_match_is_the_control",
    "a_case_of_another_type_spelled_alike_is_not_this_ones",
    "a_value_compared_with_a_computed_one_is_not_a_case_test",
    "a_command_a_page_handler_calls_declares_idempotent_by",
    "an_idempotent_command_and_one_no_handler_calls_are_the_controls",
}


def result_of(code, output):
    result = cargo_test_results.parse(code, output)
    tests = re.findall(r"^test (\S+) \.\.\. (ok|FAILED)$", output, re.M)
    if (not result.valid or len(tests) != len(NAMES)
            or {name for name, _ in tests} != NAMES
            or result.passed + result.failed != len(NAMES)
            or re.search(r"; [1-9]\d* (?:ignored|measured)", output)):
        return False, 0, 0
    if result.failed:
        blocks = re.findall(r"^---- (\S+) stdout ----\n(.*?)(?=^---- |^failures:|^test result:|\Z)", output, re.M | re.S)
        failed = {name for name, status in tests if status == "FAILED"}
        if (len(blocks) != result.failed or {name for name, _ in blocks} != failed
                or set(result.failure_names) != failed):
            return False, 0, 0
        source = ROOT / "compiler/pw-core/tests/case_and_delivery.rs"
        lines = source.read_text().splitlines()
        for _, block in blocks:
            sites = re.findall(r"panicked at ([^\n]+?):(\d+):(\d+):", block)
            if len(sites) != 1:
                return False, 0, 0
            path, line, column = sites[0]
            if ((ROOT / path).resolve() != source.resolve()
                    or not 1 <= int(line) <= len(lines)
                    or not (start := re.match(r"(\s*)assert(?:_eq|_ne)?!", lines[int(line) - 1]))
                    or int(column) != len(start.group(1)) + 1):
                return False, 0, 0
    return True, result.passed, result.failed


def run_tests():
    """Only complete native results and test-body assertions earn credit."""
    built, passed, failed = True, 0, 0
    for cmd in TESTS:
        child = subprocess.Popen(cmd, cwd=ROOT, stdout=subprocess.PIPE,
                                 stderr=subprocess.STDOUT, text=True, start_new_session=True)
        try:
            output, _ = child.communicate(timeout=300)
        except BaseException:
            try:
                os.killpg(child.pid, signal.SIGKILL)
            except ProcessLookupError:
                pass
            child.communicate()
            raise
        valid, p, f = result_of(child.returncode, output)
        if not valid or f:
            print(output, flush=True)
        built &= valid
        passed += p
        failed += f
    return built, passed, failed


def main():
    import mutation_baseline  # keeps what each command says, for a red baseline
    signal.signal(signal.SIGTERM, lambda *_: sys.exit(1))
    watch = mutation_baseline.mutation_bound.start()
    built, passed, failed = run_tests()
    print(f"baseline: {passed} passed, {failed} failed")
    if not built or failed or not passed or watch.stopped:
        print("FAIL: the unmutated baseline is not green; no mutant can mean anything")
        mutation_baseline.explain()
        return 1

    survivors = refused = killed = 0
    for what, path, anchor, replacement in MUTANTS:
        original = path.read_text()
        if original.count(anchor) != 1:
            print(f"{what}: ANCHOR NOT FOUND EXACTLY ONCE in {path.name}")
            refused += 1
            continue
        with mutation_source.changed(path, original, original.replace(anchor, replacement, 1)):
            built, passed, failed = run_tests()
        if not built or watch.stopped:
            verdict = "REFUSED (invalid or stopped producer; no kill)"
            refused += 1
        elif failed:
            verdict = f"KILLED ({failed} of {passed + failed} tests fail)"
            killed += 1
        else:
            verdict = "SURVIVED"
            survivors += 1
        print(f"{what}: {verdict}")

    print(f"{killed} of {len(MUTANTS)} mutants killed; {refused} refused; {survivors} survived")
    return int(bool(survivors or refused))


if __name__ == "__main__":
    sys.exit(main())
