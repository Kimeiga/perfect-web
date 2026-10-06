#!/usr/bin/env python3
"""Mutation controls for ADR-0229: a computed condition decides its block.

Each mutant undoes one piece: a subject read by the path the compiler names,
with what it reads; the browser's subject, its block rendered again as the
signal changes, and its arms holding no view's signals; the host's subject,
its block among those it renders again; a value the host computes inside a
block, and one the browser would, refused; the block a computed subject
decides as one a signal does; the host's first value of a subject; and the
browser's renderer given the values computed now, and setting no block in
place.

The compiler's and the server's tests must fail for a mutant of the compiler
or the server ("cargo"). A mutant of the runtime must fail
`e2e/feed.spec.mjs` in three engines, against a build of the mutated source
("browser").

Run from the repository root; `just e14-computed-conditions` records the
output. The source is restored after every mutant, whatever happens.
"""

import os
import pathlib
import re
import signal
import subprocess
import sys

ROOT = pathlib.Path(__file__).resolve().parent.parent
TEMPLATE = ROOT / "compiler/pw-core/src/template_ir.rs"
VALUES = ROOT / "compiler/pw-core/src/page_values.rs"
SERVER = ROOT / "spikes/own-renderer/server/src/main.rs"
RUNTIME = ROOT / "spikes/own-renderer/public/pw-runtime.mjs"

# (what is undone, suite, file, anchor, replacement)
MUTANTS = [
    (
        "a computed subject reads nothing",
        "cargo",
        TEMPLATE,
        "    let inputs = inputs_of(body, e, ctx);\n",
        "    let inputs = Vec::new();\n",
    ),
    (
        "a computed subject is read as a path",
        "cargo",
        TEMPLATE,
        "        read.inputs = Some(inputs);\n    }\n    value\n",
        "        let _ = (read, inputs);\n    }\n    value\n",
    ),
    (
        "a subject the browser computes decides no block",
        "cargo",
        VALUES,
        "                    browser_subjects\n                        .insert(read.part.0, (signal, path, derived.component_id.clone()));\n",
        "                    let _ = (signal, path);\n",
    ),
    (
        "a block whose subject the browser computes is not rendered again for its signal",
        "cargo",
        VALUES,
        "                reads.insert(0, signal.clone());\n",
        "",
    ),
    (
        "a block whose subject the browser computes may hold a view's signals",
        "cargo",
        VALUES,
        "            if !owned(entry.id.0).is_empty() {\n",
        "            if false {\n",
    ),
    (
        "a block a host's subject decides is not rendered again",
        "cargo",
        VALUES,
        "                || derived_values.iter().any(|d| d.path == entry.value))\n",
        "                || false)\n",
    ),
    (
        "a value the host computes in a block is a text part of its own",
        "cargo",
        VALUES,
        "                Computes::Host(part) if hole.nested => derived_values.push(part),\n",
        "",
    ),
    (
        "a value the browser would compute in a block is built",
        "cargo",
        VALUES,
        "        if nested {\n",
        "        if false && nested {\n",
    ),
    (
        "a block a computed subject decides is not one a signal does",
        "cargo",
        VALUES,
        "                    Reach::Top if signal(value) || browser.contains(value) => {\n"
        "                        Reach::Live(Vec::new())\n",
        "                    Reach::Top if signal(value) => {\n"
        "                        Reach::Live(Vec::new())\n",
    ),
    (
        "the host renders no first value of a subject",
        "cargo",
        SERVER,
        "                    | pw_render::ir::Part::Conditional { value, .. }\n",
        "",
    ),
    (
        "the browser renders a block without what it computes",
        "browser",
        RUNTIME,
        "  const values = [...signals, ...(await computedNow())]\n",
        "  const values = [...signals]\n",
    ),
    (
        "the browser sets a block in place",
        "browser",
        RUNTIME,
        "    if (live.derived && SET_IN_PLACE.has(live.kind)) {\n",
        "    if (live.derived) {\n",
    ),
]

CARGO = [
    [
        "cargo", "test", "--quiet", "--locked", "-p", "pw-core",
        "--test", "computed_conditions", "--test", "computed_holes", "--test", "computed_signals",
        "--test", "template_values",
    ],
    [
        "cargo", "test", "--quiet", "--locked", "-p", "pw-dev-server", "--",
        "a_condition_", "from_a_signal",
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
    # Stopped from outside, the source is still restored: SIGTERM raises
    # here, and the `finally` below runs.
    signal.signal(signal.SIGTERM, lambda *_: sys.exit(1))
    for suite in sorted({m[1] for m in MUTANTS}):
        built, passed, failed = SUITES[suite]()
        print(f"baseline ({suite}): {passed} passed, {failed} failed", flush=True)
        if not built or failed or not passed:
            print("FAIL: the unmutated baseline is not green; no mutant can mean anything")
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
