#!/usr/bin/env python3
"""Effects reach every inferred helper and its capability contract.

Only complete Cargo results for the four selected witnesses earn credit.
Build errors, interrupted commands and missing witnesses fail the campaign.
"""
import pathlib
import signal
import subprocess
import sys

import cargo_assertion_results
import mutation_source
import producer_process

ROOT = pathlib.Path(__file__).resolve().parent.parent
EFFECTS = ROOT / "compiler/pw-core/src/effects.rs"
MUTANTS = [
    ("effect propagation stops after eight queue waves", EFFECTS,
     "        while let Some(index) = pending.pop_front() {\n            queued[index] = false;",
     "        let mut rounds_left = 8;\n"
     "        let mut remaining_in_round = pending.len();\n"
     "        while let Some(index) = pending.pop_front() {\n"
     "            if remaining_in_round == 0 {\n"
     "                rounds_left -= 1;\n"
     "                if rounds_left == 0 { break; }\n"
     "                remaining_in_round = pending.len() + 1;\n"
     "            }\n"
     "            remaining_in_round -= 1;\n"
     "            queued[index] = false;"),
    ("effect propagation stops before reaching its fixed point", EFFECTS,
     "        while let Some(index) = pending.pop_front() {\n            queued[index] = false;",
     "        let mut remaining_in_round = pending.len();\n"
     "        while let Some(index) = pending.pop_front() {\n"
     "            if remaining_in_round == 0 { break; }\n"
     "            remaining_in_round -= 1;\n"
     "            queued[index] = false;"),
]
NAMES = [
    "a_deep_direct_helper_chain_is_effectful_in_either_declaration_order",
    "a_deep_member_helper_chain_is_effectful_in_either_declaration_order",
    "recursive_helper_cycles_converge_with_and_without_an_effect",
    "deep_helper_effects_reach_the_query_contract_in_either_declaration_order",
    "late_named_function_values_reach_the_query_contract_through_cycles",
    "late_noninvoked_member_values_reach_purity_and_contracts",
]
COMMAND = ["cargo", "test", "--quiet", "--locked", "-p", "pw-core",
           "--test", "effect_convergence", "--", "--format=pretty", "--exact", *NAMES]
SOURCES = [ROOT / "compiler/pw-core/tests/effect_convergence.rs"]


def run(command=COMMAND, expected=NAMES, sources=SOURCES):
    try:
        child = producer_process.run(command, timeout=600, cwd=ROOT)
    except subprocess.TimeoutExpired as expired:
        print(expired.output, flush=True)
        print("INVALID RESULT: timed out native producer", flush=True)
        return None
    result = cargo_assertion_results.validate(child.returncode, child.stdout,
        root=ROOT, sources=sources, expected=expected)
    print(child.stdout, flush=True)
    if not result.valid:
        print(f"INVALID RESULT: {result}", flush=True)
        return None
    return result


def main():
    import mutation_baseline
    watch = mutation_baseline.mutation_bound.start()
    signal.signal(signal.SIGTERM, lambda *_: sys.exit(1))
    baseline = run()
    if baseline is None or baseline.failed or watch.stopped:
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
        if result is None or watch.stopped:
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
