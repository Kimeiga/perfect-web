#!/usr/bin/env python3
"""Mutation controls for ADR-0223: a streamed region is filled when the whole
of its arm has arrived.

Each mutant undoes one piece: the runtime's wait for a template's end, the
comment the renderer writes after each one, and the hold a test puts on the
recommender. Every mutant must fail `e2e/slots.spec.mjs` in three engines,
against a build of the mutated source: the page and its runtime (`run.sh`),
and the server.

Run from the repository root; `just e14-whole-fills` records the output.
The source is restored after every mutant, whatever happens.
"""

import os
import pathlib
import re
import signal
import subprocess
import sys

ROOT = pathlib.Path(__file__).resolve().parent.parent
RUNTIME = ROOT / "spikes/own-renderer/public/pw-runtime.mjs"
RENDER = ROOT / "runtime/pw-render/src/lib.rs"
STORE = ROOT / "spikes/own-renderer/server/src/store.rs"

# (what is undone, file, anchor, replacement)
MUTANTS = [
    (
        "a template is applied before its end has arrived",
        RUNTIME,
        '    if (!t.nextSibling && document.readyState === "loading") continue;\n',
        "",
    ),
    (
        "nothing is written after a template",
        RENDER,
        '        "<template for=\\"{name}\\">{inner}</template><!--/{name}-->"\n',
        '        "<template for=\\"{name}\\">{inner}</template>"\n',
    ),
    (
        "the recommender is not held",
        STORE,
        "            if let Some(gate) = &recommender.gate {\n                gate.wait();\n            }\n",
        "",
    ),
]

SPEC = ["e2e/slots.spec.mjs"]

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


def build():
    """The page, with the runtime it loads, and the server, as the source
    says."""
    page, page_code = bounded(["bash", "spikes/own-renderer/run.sh"], cwd=ROOT,
                              env={**os.environ, "BUILD_ONLY": "1"})
    server, code = bounded(["cargo", "build", "--quiet", "--locked", "-p", "pw-dev-server"], cwd=ROOT)
    return page_code == 0 and code == 0


def run_spec():
    """(built, passed, failed) for the spec in three engines, after a build."""
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


def main():
    import mutation_baseline  # keeps what each command says, for a red baseline
    # Stopped from outside, the source is still restored: SIGTERM raises
    # here, and the `finally` below runs.
    signal.signal(signal.SIGTERM, lambda *_: sys.exit(1))
    built, passed, failed = run_spec()
    print(f"baseline: {passed} passed, {failed} failed", flush=True)
    if not built or failed or not passed:
        print("FAIL: the unmutated baseline is not green; no mutant can mean anything")
        mutation_baseline.explain()
        return 1

    survivors = 0
    try:
        for what, path, anchor, replacement in MUTANTS:
            original = path.read_text()
            if original.count(anchor) != 1:
                print(f"{what}: ANCHOR NOT FOUND EXACTLY ONCE in {path.name}")
                survivors += 1
                continue
            try:
                path.write_text(original.replace(anchor, replacement, 1))
                built, passed, failed = run_spec()
            finally:
                path.write_text(original)
            if not built:
                verdict = "KILLED (does not build)"
            elif failed:
                verdict = f"KILLED ({failed} of {passed + failed} tests fail)"
            else:
                verdict = "SURVIVED"
                survivors += 1
            print(f"{what}: {verdict}", flush=True)
    finally:
        # The server as the source says, whatever a mutant left built.
        build()

    print(f"{len(MUTANTS) - survivors} of {len(MUTANTS)} mutants killed")
    return 1 if survivors else 0


if __name__ == "__main__":
    sys.exit(main())
