#!/usr/bin/env python3
"""Mutation controls for ADR-0078: an effect is performed where its function
is named.

Each mutant undoes one piece of how a declaration named as a value
contributes its effects: that it is counted at all, that a binding in scope
is its own value, that a helper declaring no row carries everything it
performs, and that the diagnostic says the function was named as a value.
The effect tests and the generality witnesses must then fail.

Run from the repository root; `just e10-effects-through-values` records the
output. The source is restored after every mutant, whatever happens.
"""

import pathlib
import subprocess
import sys
import cargo_assertion_results
import mutation_source
import producer_process

ROOT = pathlib.Path(__file__).resolve().parent.parent
EFFECTS = ROOT / "compiler/pw-core/src/effects.rs"

# (what is undone, file, anchor, replacement)
MUTANTS = [
    (
        "a function named as a value is not counted",
        EFFECTS,
        "        self.value_effects(unit, body, body.root, types, &mut out, reads);\n",
        "",
    ),
    (
        "a binding in scope is read as the declaration it shadows",
        EFFECTS,
        "                Expr::Name(_) if types.lexical().binder(id).is_some() => continue,\n",
        "",
    ),
    (
        "a helper declaring no row carries only what it calls",
        EFFECTS,
        "            let found = self.infer_in_reading(*unit, body, types, Some(&mut reads));",
        "            let found = self.calls_with_types(*unit, body, body.root, Some(types), Some(&mut reads));",
    ),
    (
        "a function named as a value is said to be called",
        EFFECTS,
        "                    via: Via::Value {",
        "                    via: Via::Direct {",
    ),
]

TESTS = [
    ["cargo", "test", "--quiet", "--locked", "-p", "pw-core", "--test", "effects_through_values"],
    ["cargo", "test", "--quiet", "--locked", "-p", "pw-core", "--test", "generality"],
]
INVENTORY = {}

def run_tests():
    """Complete named inventories; only native body assertions earn kills."""
    passed = failed = 0
    for cmd in TESTS:
        try:
            child = producer_process.run([*cmd, '--', '--format=pretty'], cwd=ROOT, timeout=600)
        except subprocess.TimeoutExpired as expired:
            print(expired.output, flush=True)
            return False, 0, 0
        key = tuple(cmd)
        source = ROOT / 'compiler/pw-core/tests' / (cmd[-1] + '.rs')
        result = cargo_assertion_results.validate(child.returncode, child.stdout,
            root=ROOT, sources=[source], expected=INVENTORY.get(key))
        print(child.stdout, flush=True)
        if not result.valid:
            return False, 0, 0
        INVENTORY.setdefault(key, cargo_assertion_results.names(child.stdout))
        passed += result.passed
        failed += result.failed
    return True, passed, failed


def main():
    import mutation_baseline  # keeps what each command says, for a red baseline
    watch = mutation_baseline.mutation_bound.start()
    built, passed, failed = run_tests()
    print(f"baseline: {passed} passed, {failed} failed")
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
        with mutation_source.changed(path, original, original.replace(anchor, replacement, 1)):
            built, passed, failed = run_tests()
        if not built or watch.stopped:
            verdict = "REFUSED (invalid native assertion producer)"
            survivors += 1
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
