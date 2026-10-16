#!/usr/bin/env python3
"""Mutation controls for the argument-invariants track (ADR-0346): a value of
an opaque type with a `where` invariant exists only if it passed the
invariant, and every boundary a value from outside crosses holds it.

Each mutant removes one boundary's check, or one piece of how it reads:
- the host's call APIs: each of the three holds nothing; a predicate reads the
  arguments before they are decoded; the check holds nothing; the arguments
  are held as given, not as the component is passed them (a type that
  contains itself crosses as its nodes);
- the build: a page's plan states nothing its parameters hold; a map's
  entries, a map's key, a set's items are not reached;
- the development server: a routed page's parameters, and a page's asked for
  by name, given as the address says them; the plan's checks not read; a
  refused parameter answered as not found;
- the fixture (`examples/demo/invariants.pw`): each of its three types states
  nothing. Each such mutant must still be a program: it is checked before its
  tests run, and one the checker refuses is a failure of this script.

Each must fail the tests that see it: `pw-core`'s, `pw-host`'s with its
engine, `pw-conformance`'s and the development server's, each its track's
target (below). The browser's `e2e/argument-invariants.spec.mjs`, in three
engines, is recorded beside them by the recipe.

Run from the repository root; `just e14-argument-invariants` records the
output. The source is restored after every mutant, whatever happens.
"""

import glob
import os
import pathlib
import re
import signal
import subprocess
import sys

ROOT = pathlib.Path(__file__).resolve().parent.parent
HOST = ROOT / "runtime/pw-host/src/lib.rs"
CONTRACT = ROOT / "compiler/pw-core/src/contract.rs"
PAGE_VALUES = ROOT / "compiler/pw-core/src/page_values.rs"
SERVER = ROOT / "spikes/own-renderer/server/src/main.rs"
FIXTURE = ROOT / "examples/demo/invariants.pw"

# The fixture's program, as the development server's tests build it.
PROGRAM = [
    "packages/pw-std/*.pw",
    "packages/pw-platform-web/*.pw",
    "examples/domain.pw",
    "examples/demo/invariants.pw",
]

# (what is undone, the suites that see it, file, anchor, replacement)
MUTANTS = [
    (
        "a call within its budget holds nothing",
        "host",
        HOST,
        "            // Its arguments decoded first, before anything else.\n"
        "            self.hold_arguments(contract, export, args)?;\n",
        "            // Its arguments decoded first, before anything else.\n",
    ),
    (
        "a call with its `requires` evaluated holds nothing",
        "host,conformance,server",
        HOST,
        "            // Its arguments decoded before a predicate reads them.\n"
        "            self.hold_arguments(contract, export, args)?;\n"
        "            authorize_export(contract, export, args, evaluate)?;\n",
        "            // Its arguments decoded before a predicate reads them.\n"
        "            authorize_export(contract, export, args, evaluate)?;\n",
    ),
    (
        "a predicate reads the arguments before they are decoded",
        "host",
        HOST,
        "            // Its arguments decoded before a predicate reads them.\n"
        "            self.hold_arguments(contract, export, args)?;\n"
        "            authorize_export(contract, export, args, evaluate)?;\n",
        "            // Its arguments decoded before a predicate reads them.\n"
        "            authorize_export(contract, export, args, evaluate)?;\n"
        "            self.hold_arguments(contract, export, args)?;\n",
    ),
    (
        "a measured call holds nothing",
        "host",
        HOST,
        "            // Its arguments decoded as every call's are.\n"
        "            self.hold_arguments(contract, export, args)?;\n",
        "            // Its arguments decoded as every call's are.\n",
    ),
    (
        "a call's check holds nothing",
        "host,conformance,server",
        HOST,
        '            holds(bounded, &passed).map_err(|why| format!("argument refused: {why}"))\n',
        '            holds(&[], &passed).map_err(|why| format!("argument refused: {why}"))\n',
    ),
    (
        "the arguments are held as given, not as the component is passed them",
        "conformance",
        HOST,
        "                .map(|(given, (_, ty))| graph::tangle(given, &ty))\n"
        "                .collect::<Result<_, _>>()?;\n"
        "            holds(bounded, &passed)",
        "                .map(|(given, (_, _ty))| Ok::<Val, String>(given))\n"
        "                .collect::<Result<_, _>>()?;\n"
        "            holds(bounded, &passed)",
    ),
    (
        "a page's plan states nothing its parameters hold",
        "core,server",
        PAGE_VALUES,
        "            bounded: crate::contract::bounded_params(sigs, DefId { unit, decl: id.0 }),\n",
        "            bounded: Vec::new(),\n",
    ),
    (
        "a map's entries are not reached",
        "core,conformance",
        CONTRACT,
        "        TypeKey::Builtin(Builtin::Map, args) => {\n"
        '            path.push("*".into());\n',
        "        TypeKey::Builtin(Builtin::Map, args) if false => {\n"
        '            path.push("*".into());\n',
    ),
    (
        "a map's key is read where its value is",
        "core,conformance",
        CONTRACT,
        "                step(i.to_string(), arg, path, seen, out);\n",
        '                step("1".to_string(), arg, path, seen, out);\n',
    ),
    (
        "a set's items are not reached",
        "core,conformance",
        CONTRACT,
        "        TypeKey::Builtin(Builtin::List | Builtin::Set, args) => {\n",
        "        TypeKey::Builtin(Builtin::List, args) => {\n",
    ),
    (
        "a routed page is given its parameters as the address says them",
        "server",
        SERVER,
        "                    held_params(plan, &params).map(|()| (page.clone(), params))\n",
        "                    Ok((page.clone(), params))\n",
    ),
    (
        "a page asked for by name is given its parameters as the address says them",
        "server",
        SERVER,
        "                let held = held_params(server.plan_of(path), &params);\n",
        "                let held: Result<(), String> = Ok(());\n",
    ),
    (
        "a page's plan's checks are not read",
        "server",
        SERVER,
        '    let Some(bounded) = plan.get("bounded") else {\n',
        '    let Some(bounded) = plan.get("bounded-not-stated") else {\n',
    ),
    (
        "a parameter refused by name is answered as a page not found",
        "server",
        SERVER,
        "            if let Ok((_, Err(why))) = &params {\n"
        "                respond(\n"
        "                    &mut stream,\n"
        "                    400,\n",
        "            if let Ok((_, Err(why))) = &params {\n"
        "                respond(\n"
        "                    &mut stream,\n"
        "                    404,\n",
    ),
    (
        "`Stars` states nothing",
        "server",
        FIXTURE,
        "opaque type Stars = Int where value >= 1 & value <= 5\n",
        "opaque type Stars = Int\n",
    ),
    (
        "`Note` states nothing",
        "server",
        FIXTURE,
        "opaque type Note = String where String.length(value) <= 20\n",
        "opaque type Note = String\n",
    ),
    (
        "`Code` states nothing",
        "server",
        FIXTURE,
        "opaque type Code = String where String.length(value) >= 2 & String.length(value) <= 8\n",
        "opaque type Code = String\n",
    ),
]

CARGO = {
    "core": ["cargo", "test", "--quiet", "--locked", "-p", "pw-core", "--test", "argument_invariants"],
    "host": [
        "cargo", "test", "--quiet", "--locked", "-p", "pw-host", "--features", "engine",
        "--test", "pleris_component",
    ],
    "conformance": [
        "cargo", "test", "--quiet", "--locked", "-p", "pw-conformance", "--test", "argument_invariants",
    ],
    "server": [
        "cargo", "test", "--quiet", "--locked", "-p", "pw-dev-server", "--",
        "tests::argument_invariants::",
    ],
}

# How long one command may run. Past the bound it is killed with everything
# it started, and the run counts as failing.
BOUND = 1800


def bounded(cmd, **kw):
    """(output, returncode), or (output, None) when it ran past the bound."""
    p = subprocess.Popen(cmd, start_new_session=True, stdout=subprocess.PIPE,
                         stderr=subprocess.STDOUT, text=True, **kw)
    try:
        out, _ = p.communicate(timeout=BOUND)
        return out, p.returncode
    except subprocess.TimeoutExpired:
        os.killpg(p.pid, signal.SIGKILL)
        out, _ = p.communicate()
        return out, None


def cargo_tests(suite):
    """(built, passed, failed) over the suite's tests."""
    out, code = bounded(CARGO[suite], cwd=ROOT)
    if code is None:
        return True, 0, 1
    found = re.findall(r"test result: \w+\. (\d+) passed; (\d+) failed", out)
    if not found:
        return False, 0, 0
    return True, sum(int(p) for p, _ in found), sum(int(f) for _, f in found)


def program_checks():
    """Whether the fixture's program checks clean: a `.pw` mutant must
    still be a program, so what kills it is its tests, not the checker."""
    pw = pathlib.Path(os.environ.get("CARGO_TARGET_DIR", ROOT / "target")) / "debug/pw"
    files = [f for pattern in PROGRAM for f in sorted(glob.glob(str(ROOT / pattern)))]
    out, code = bounded([str(pw), "check", "--plain", *files], cwd=ROOT)
    return code == 0 and "no diagnostics" in out, out.strip().splitlines()[-1:] if out else []


def main():
    import mutation_baseline  # keeps what each command says, for a red baseline
    # Stopped from outside, the source is still restored: SIGTERM raises
    # here, and the `finally` below runs.
    signal.signal(signal.SIGTERM, lambda *_: sys.exit(1))
    built = subprocess.run(["cargo", "build", "--quiet", "--locked", "-p", "pw-cli"], cwd=ROOT)
    if built.returncode != 0:
        print("FAIL: `pw` does not build")
        return 1
    clean, said = program_checks()
    if not clean:
        print(f"FAIL: the fixture's program does not check: {said}")
        return 1
    for suite in CARGO:
        built, passed, failed = cargo_tests(suite)
        print(f"baseline ({suite}): {passed} passed, {failed} failed", flush=True)
        if not built or failed or not passed:
            print("FAIL: the unmutated baseline is not green; no mutant can mean anything")
            mutation_baseline.explain()
            return 1

    survivors = 0
    for what, suites, path, anchor, replacement in MUTANTS:
        original = path.read_text()
        if original.count(anchor) != 1:
            print(f"{what}: ANCHOR NOT FOUND EXACTLY ONCE in {path.name}")
            survivors += 1
            continue
        results = []
        try:
            path.write_text(original.replace(anchor, replacement, 1))
            if path.suffix == ".pw":
                clean, said = program_checks()
                if not clean:
                    print(f"{what}: NOT A PROGRAM, the checker refuses it: {said}", flush=True)
                    survivors += 1
                    continue
            for suite in suites.split(","):
                results.append(cargo_tests(suite))
        finally:
            path.write_text(original)
        failed = sum(f for _, _, f in results)
        total = sum(p + f for _, p, f in results)
        if not all(b for b, _, _ in results):
            verdict = "KILLED (does not build)"
        elif failed:
            verdict = f"KILLED ({failed} of {total} tests fail)"
        else:
            verdict = "SURVIVED"
            survivors += 1
        print(f"{what} [{suites}]: {verdict}", flush=True)

    print(f"{len(MUTANTS) - survivors} of {len(MUTANTS)} mutants killed")
    return 1 if survivors else 0


if __name__ == "__main__":
    sys.exit(main())
