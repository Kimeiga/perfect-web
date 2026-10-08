#!/usr/bin/env python3
"""Mutation controls for ADR-0256 (the first half of ADR-0195's ruling 10):
an entry written `_` is every entry at the rest, in every session's
partition.

Each mutant undoes one piece: the checker taking an `invalidates` key's `_`
as every value, refusing an `emits` key's by the parameter it leaves out,
and telling a bare key how every entry is written; the backend leaving the
position out of what the command computes, and naming the function by it;
the contract recording it; the host restoring each position, naming an
entry by the values given, in every session's partition, and telling
another session of a private drop no key pins to the committing one; and
the runtime's cache dropping what it names. The tests of each must then
fail.

Run from the repository root; `just e14-every-entry` records the output.
The source is restored after every mutant, whatever happens.
"""

import pathlib
import re
import subprocess
import sys

ROOT = pathlib.Path(__file__).resolve().parent.parent
CHECK = ROOT / "compiler/pw-core/src/check.rs"
BACKEND = ROOT / "compiler/pw-core/src/backend/mod.rs"
LOWER = ROOT / "compiler/pw-core/src/backend/lower.rs"
CONTRACT = ROOT / "compiler/pw-core/src/contract.rs"
SERVER = ROOT / "spikes/own-renderer/server/src/main.rs"
RESOURCE = ROOT / "runtime/pw-resource/src/lib.rs"

# (what is undone, file, anchor, replacement)
MUTANTS = [
    (
        "an `invalidates` key's `_` is a name",
        CHECK,
        "                    crate::hir::ExecutionContext::Invalidated => continue,\n",
        "                    crate::hir::ExecutionContext::Invalidated => {}\n",
    ),
    (
        "an `emits` key's `_` is refused as a name",
        CHECK,
        "                        out.push(emitted_no_value(\n"
        "                            hirs, workspace, unit, hir, decl, body, root.root,\n"
        "                        ));\n"
        "                        continue;\n",
        "                        let _ = emitted_no_value(\n"
        "                            hirs, workspace, unit, hir, decl, body, root.root,\n"
        "                        );\n",
    ),
    (
        "a bare key is not told how every entry is written",
        CHECK,
        "                .filter(|d| d.code == crate::codes::CALL_ARITY.id && d.primary_span == key.span)\n",
        "                .filter(|_| false)\n",
    ),
    (
        "a position written `_` is computed",
        LOWER,
        '            "invalidates" => given\n',
        '            "invalidates" if false => given\n',
    ),
    (
        "the function is named by no position",
        BACKEND,
        "    if !every.is_empty() {\n        name.push_str(\"-EVERY\");\n",
        "    if false {\n        name.push_str(\"-EVERY\");\n",
    ),
    (
        "the contract records no position",
        CONTRACT,
        "            invalidates: Some(signature.path.clone()),\n            every,\n",
        "            invalidates: Some(signature.path.clone()),\n            every: Vec::new(),\n",
    ),
    (
        "the host restores no position",
        SERVER,
        "                    let every = import.every.clone();\n",
        "                    let every: Vec<usize> = Vec::new();\n",
    ),
    (
        "an entry is named whatever its values",
        SERVER,
        "                    .is_ok_and(|p| p == val_to_json(v)),\n",
        "                    .is_ok_and(|_| true),\n",
    ),
    (
        "a private entry is dropped in the committing session's partition alone",
        SERVER,
        "                .invalidate_where(resource, |key| names_entry(&policy, args, key));\n",
        "                .invalidate_where(resource, |key| names_entry(&policy, args, key) && (!entry_is_private(&policy) || key.starts_with(&format!(\"session={session}\"))));\n",
    ),
    (
        "a private drop is told to no other session",
        SERVER,
        "            (!entry_is_private(&policy) || !pinned).then(|| resource.to_string())\n",
        "            (!entry_is_private(&policy) || (!pinned && false)).then(|| resource.to_string())\n",
    ),
    (
        "the cache keeps what it is told to drop",
        RESOURCE,
        "        for key in &dropped {\n            st.cache.remove(key);\n        }\n",
        "        for key in &dropped {\n            let _ = key;\n        }\n",
    ),
]

TESTS = [
    ["cargo", "test", "--quiet", "--locked", "-p", "pw-core", "--test", "every_entry"],
    ["cargo", "test", "--quiet", "--locked", "-p", "pw-conformance", "--test", "invalidations"],
    ["cargo", "test", "--quiet", "--locked", "-p", "pw-dev-server", "--", "every_entry"],
    ["cargo", "test", "--quiet", "--locked", "-p", "pw-resource", "--test", "concurrency_regressions", "--", "named"],
]


def run_tests():
    """(built, passed, failed) over every test command."""
    built, passed, failed = True, 0, 0
    for cmd in TESTS:
        r = subprocess.run(cmd, cwd=ROOT, capture_output=True, text=True)
        out = r.stdout + r.stderr
        results = re.findall(r"test result: \w+\. (\d+) passed; (\d+) failed", out)
        if not results:
            built = False
            continue
        for p, f in results:
            passed += int(p)
            failed += int(f)
    return built, passed, failed


def main():
    import mutation_baseline  # keeps what each command says, for a red baseline
    built, passed, failed = run_tests()
    print(f"baseline: {passed} passed, {failed} failed")
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
        print(f"{what}: {verdict}")

    print(f"{len(MUTANTS) - survivors} of {len(MUTANTS)} mutants killed")
    return 1 if survivors else 0


if __name__ == "__main__":
    sys.exit(main())
