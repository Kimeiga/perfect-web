#!/usr/bin/env python3
"""Negative controls for E14-A's store contract (ADR-0120).

The contract suite (`benchmarks/harness/contract/`) passing on three stores
is evidence only if it fails on a store that is wrong. Each mutant breaks one
store in one plausible way, rebuilds that store, and runs the suite against
it alone. At least one contract test must then fail. The source is restored
and the store rebuilt after every mutant, whatever happens.

Run from the repository root; `just e14-contract` records the output.
"""

import pathlib
import re
import subprocess
import sys

ROOT = pathlib.Path(__file__).resolve().parent.parent
NEXT = ROOT / "benchmarks/baselines/next-react"
SVELTE = ROOT / "benchmarks/baselines/sveltekit"
HARNESS = ROOT / "benchmarks/harness"
# The benchmark's Pleris store (ADR-0156), which the other two baselines
# match; the canonical store, `examples`, grows past them.
STORE_PW = ROOT / "benchmarks/baselines/pleris/store/app.pw"

COUNT = "  return cart.lines.reduce((n, line) => n + line.quantity, 0);\n"
SHARED = "  const lines = session ? carts.get(session) : undefined;\n"

# (stack, what is broken, file, anchor, replacement)
MUTANTS = [
    (
        "next-react",
        "the count is distinct items, not their quantities",
        NEXT / "lib/store.ts",
        COUNT,
        "  return cart.lines.length;\n",
    ),
    (
        "next-react",
        "every session reads the first session's cart",
        NEXT / "lib/store.ts",
        SHARED,
        "  const lines = carts.values().next().value;\n",
    ),
    (
        "sveltekit",
        "the count is distinct items, not their quantities",
        SVELTE / "src/lib/server/store.ts",
        COUNT,
        "  return cart.lines.length;\n",
    ),
    (
        "sveltekit",
        "every session reads the first session's cart",
        SVELTE / "src/lib/server/store.ts",
        SHARED,
        "  const lines = carts.values().next().value;\n",
    ),
    (
        "pleris",
        "Add adds two",
        STORE_PW,
        "add_to_cart(item.id, PositiveInt(1))}",
        "add_to_cart(item.id, PositiveInt(2))}",
    ),
]


def build(stack):
    """Rebuild one store from its source; True when it built."""
    if stack == "pleris":
        cmd, cwd, env = ["bash", "spikes/own-renderer/baseline-store.sh"], ROOT, {}
    else:
        cmd, cwd, env = ["pnpm", "build"], NEXT if stack == "next-react" else SVELTE, {}
    import os

    r = subprocess.run(cmd, cwd=cwd, env={**os.environ, **env}, capture_output=True, text=True)
    return r.returncode == 0


def run_suite(stack):
    """(passed, failed) for the contract suite against one store."""
    import os

    r = subprocess.run(
        ["pnpm", "exec", "playwright", "test", "--reporter=list"],
        cwd=HARNESS,
        env={**os.environ, "BENCH_STACK": stack},
        capture_output=True,
        text=True,
    )
    out = r.stdout + r.stderr
    passed = re.search(r"(\d+) passed", out)
    failed = re.search(r"(\d+) failed", out)
    return (int(passed.group(1)) if passed else 0, int(failed.group(1)) if failed else 0)


def main():
    for stack in ("pleris", "next-react", "sveltekit"):
        if not build(stack):
            print(f"FAIL: {stack} does not build unmutated")
            return 1
        passed, failed = run_suite(stack)
        print(f"baseline {stack}: {passed} passed, {failed} failed")
        if failed or not passed:
            print("FAIL: the unmutated baseline is not green; no mutant can mean anything")
            return 1

    survivors = 0
    for stack, what, path, anchor, replacement in MUTANTS:
        original = path.read_text()
        if original.count(anchor) != 1:
            print(f"{stack}: {what}: ANCHOR NOT FOUND EXACTLY ONCE in {path.name}")
            survivors += 1
            continue
        try:
            path.write_text(original.replace(anchor, replacement, 1))
            built = build(stack)
            passed, failed = run_suite(stack) if built else (0, 0)
        finally:
            path.write_text(original)
            build(stack)
        if not built:
            verdict = "KILLED (does not build)"
        elif failed:
            verdict = f"KILLED ({failed} of {passed + failed} contract tests fail)"
        else:
            verdict = "SURVIVED"
            survivors += 1
        print(f"{stack}: {what}: {verdict}")

    print(f"{len(MUTANTS) - survivors} of {len(MUTANTS)} mutants killed")
    return 1 if survivors else 0


if __name__ == "__main__":
    sys.exit(main())
