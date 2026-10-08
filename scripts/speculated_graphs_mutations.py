#!/usr/bin/env python3
"""Mutation controls for ADR-0233: a speculation on a value of a type that
contains itself.

Each mutant undoes one piece: the speculation module decoding such a value
from its nodes; the server writing it by its query's type, the `Ok` of a
`Result`; the host's graph, its nodes in level order with their children by
index, and each such value inside another a graph of its own; and a case's
payload written by its type.

The compiler's, the host's and the server's tests must fail for each
("cargo").

Run from the repository root; `just e14-speculated-graphs` records the
output. The source is restored after every mutant, whatever happens.
"""

import os
import pathlib
import re
import signal
import subprocess
import sys

ROOT = pathlib.Path(__file__).resolve().parent.parent
PURE = ROOT / "compiler/pw-core/src/backend/js_pure.rs"
SPECULATION = ROOT / "compiler/pw-core/src/backend/speculation.rs"
HOST = ROOT / "runtime/pw-host/src/lib.rs"
SERVER = ROOT / "spikes/own-renderer/server/src/main.rs"

# (what is undone, suite, file, anchor, replacement)
MUTANTS = [
    (
        "the speculation module decodes a value by its shape",
        "cargo",
        SPECULATION,
        "        match super::js_pure::decoder_of_nodes(&program, value, \"j\") {\n",
        "        match super::js_pure::decoder(&program, value, \"j\") {\n",
    ),
    (
        "the decoder of nodes reads no graph",
        "cargo",
        PURE,
        "    emitter.graphs = true;\n",
        "    emitter.graphs = false;\n",
    ),
    (
        "the server writes a speculated value nested",
        "cargo",
        SERVER,
        "            .browser_value(\n"
        "                &[&export.interface, &export.function],\n"
        "                value.clone(),\n"
        "                &val_to_json,\n"
        "            )\n",
        "            .browser_value(\n"
        "                &[&export.interface, &export.function],\n"
        "                value.clone(),\n"
        "                &val_to_json,\n"
        "            )\n"
        "            .map(|_| val_to_json(value))\n",
    ),
    (
        "a binding's value is written by the whole result's type",
        "cargo",
        HOST,
        "                (_, wasmtime::component::types::Type::Result(r)) => r\n",
        "                (_, wasmtime::component::types::Type::Result(r)) if false => r\n",
    ),
    (
        "a value of a type that contains itself is written by its fields",
        "cargo",
        HOST,
        "            if node_of(ty).is_some() {\n"
        "                return to_browser(v, ty, &|v, t| browser_json(v, t, leaf));\n",
        "            if node_of(ty).is_some() && false {\n"
        "                return to_browser(v, ty, &|v, t| browser_json(v, t, leaf));\n",
    ),
    (
        "a node's children are written as its other fields are",
        "cargo",
        HOST,
        "                                (Some(k), _) => children(k, v)?,\n",
        "                                (Some(_), _) => field(v, &node)?,\n",
    ),
    (
        "the nodes are written last first",
        "cargo",
        HOST,
        "            Ok(serde_json::json!({ \"$graph\": out }))\n",
        "            out.reverse();\n            Ok(serde_json::json!({ \"$graph\": out }))\n",
    ),
    (
        "a case's payload is written knowing no type",
        "cargo",
        HOST,
        "                    let payload = inner(v, ty)?;\n",
        "                    let payload = inner(v, None)?;\n",
    ),
]

CARGO = [
    [
        "cargo", "test", "--quiet", "--locked", "-p", "pw-core",
        "--test", "speculated_graphs",
    ],
    [
        "cargo", "test", "--quiet", "--locked", "-p", "pw-conformance",
        "--test", "browser_graphs",
    ],
    [
        "cargo", "test", "--quiet", "--locked", "-p", "pw-dev-server", "--",
        "a_value_that_contains_itself_is_written",
    ],
]

SPEC = ["e2e/feed.spec.mjs"]

# How long one command may run. Past the bound it is killed with everything
# it started, and the run counts as failing.
BOUND = 1200


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


def cargo_tests():
    """(built, passed, failed) over every test command."""
    built, passed, failed = True, 0, 0
    for cmd in CARGO:
        out, code = bounded(cmd, cwd=ROOT)
        results = re.findall(r"test result: \w+\. (\d+) passed; (\d+) failed", out or "")
        if code is None:
            failed += 1
            continue
        if not results:
            built = False
            continue
        for p, f in results:
            passed += int(p)
            failed += int(f)
    return built, passed, failed


def build():
    """The store's and the feed's pages, with the runtime they load, and the
    server, as the source says."""
    _, page = bounded(["bash", "spikes/own-renderer/run.sh"], cwd=ROOT,
                      env={**os.environ, "BUILD_ONLY": "1"})
    _, feed = bounded(["bash", "spikes/own-renderer/feed.sh"], cwd=ROOT)
    _, server = bounded(["cargo", "build", "--quiet", "--locked", "-p", "pw-dev-server"], cwd=ROOT)
    return page == 0 and feed == 0 and server == 0


def browser_tests():
    """(built, passed, failed) for the spec in three engines, after a build.
    A spec skipped, as one whose build is stale is, ran nothing: not built."""
    if not build():
        return False, 0, 0
    out, code = bounded(
        ["pnpm", "exec", "playwright", "test", *SPEC, "--reporter=line"],
        cwd=ROOT / "spikes/own-renderer",
    )
    if code is None:
        return True, 0, 1
    out = re.sub(r"\x1b\[[0-9;]*[A-Za-z]", "", out)
    passed = sum(int(n) for n in re.findall(r"(\d+) passed", out))
    failed = sum(int(n) for n in re.findall(r"(\d+) failed", out))
    return passed + failed > 0, passed, failed


SUITES = {"cargo": cargo_tests, "browser": browser_tests}


def main():
    import mutation_baseline  # keeps what each command says, for a red baseline
    # Stopped from outside, the source is still restored: SIGTERM raises
    # here, and the `finally` below runs.
    signal.signal(signal.SIGTERM, lambda *_: sys.exit(1))
    for suite in sorted({m[1] for m in MUTANTS}):
        built, passed, failed = SUITES[suite]()
        print(f"baseline ({suite}): {passed} passed, {failed} failed", flush=True)
        if not built or failed or not passed:
            print("FAIL: the unmutated baseline is not green; no mutant can mean anything")
            mutation_baseline.explain()
            return 1

    survivors = 0
    try:
        for what, suite, path, anchor, replacement in MUTANTS:
            original = path.read_text()
            if original.count(anchor) != 1:
                print(f"{what}: ANCHOR NOT FOUND EXACTLY ONCE in {path.name}")
                survivors += 1
                continue
            try:
                path.write_text(original.replace(anchor, replacement, 1))
                built, passed, failed = SUITES[suite]()
            finally:
                path.write_text(original)
            if not built:
                verdict = "KILLED (does not build)"
            elif failed:
                verdict = f"KILLED ({failed} of {passed + failed} tests fail)"
            else:
                verdict = "SURVIVED"
                survivors += 1
            print(f"{what} [{suite}]: {verdict}", flush=True)
    finally:
        # The pages and the server as the source says, whatever a mutant
        # left built.
        build()

    print(f"{len(MUTANTS) - survivors} of {len(MUTANTS)} mutants killed")
    return 1 if survivors else 0


if __name__ == "__main__":
    sys.exit(main())
