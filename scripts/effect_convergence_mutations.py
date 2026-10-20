#!/usr/bin/env python3
"""Effects reach every inferred helper and its capability contract.

Only complete Cargo results for the four selected witnesses earn credit.
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
EFFECTS = ROOT / "compiler/pw-core/src/effects.rs"
MUTANTS = [
    ("effect propagation stops after eight sweeps", EFFECTS,
     "        loop {\n            let mut changed = false;",
     "        for _ in 0..8 {\n            let mut changed = false;"),
    ("effect propagation stops before reaching its fixed point", EFFECTS,
     "            if !changed {\n                break;",
     "            if true {\n                break;"),
]
NAMES = [
    "a_deep_direct_helper_chain_is_effectful_in_either_declaration_order",
    "a_deep_member_helper_chain_is_effectful_in_either_declaration_order",
    "recursive_helper_cycles_converge_with_and_without_an_effect",
    "deep_helper_effects_reach_the_query_contract_in_either_declaration_order",
]
COMMAND = ["cargo", "test", "--quiet", "--locked", "-p", "pw-core",
           "--test", "effect_convergence", "--", "--exact", *NAMES]

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
