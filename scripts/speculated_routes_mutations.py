#!/usr/bin/env python3
"""Mutation controls for ADR-0236: a speculation on the entry a page's
parameter keys (ruling 0122-d).

Each mutant undoes one piece: a target's key from the command's parameter
matched to the page's parameter; that parameter passed unchanged, by its
binding; a call in a composed view not followed; the speculated entry named
by the page's parameters its key reads, read from the document's address;
each frame sent at that entry; a commit's basis naming it; a region the
speculation renders holding the page's parameters, which the document
carries; and the feed's reply speculated.

The compiler's and the server's tests must fail for a mutant of the compiler
or the server ("cargo"). A mutant the browser alone sees must fail
`e2e/feed.spec.mjs` in three engines, against a build of the mutated source
("browser").

Run from the repository root; `just e14-speculated-routes` records the
output. The source is restored after every mutant, whatever happens.
"""

import os
import pathlib
import re
import signal
import subprocess
import sys

ROOT = pathlib.Path(__file__).resolve().parent.parent
SPECULATION = ROOT / "compiler/pw-core/src/backend/speculation.rs"
SERVER = ROOT / "spikes/own-renderer/server/src/main.rs"
FEED = ROOT / "examples/feed/app.pw"

# (what is undone, suite, file, anchor, replacement)
MUTANTS = [
    (
        "a key from the command's parameter matches no binding",
        "cargo",
        SPECULATION,
        "                            page_parameter(hir, page_id, *k).is_some_and(|p| {\n",
        "                            page_parameter(hir, page_id, *k).filter(|_| false).is_some_and(|p| {\n",
    ),
    (
        "a name the handler binds passes the page's parameter",
        "cargo",
        SPECULATION,
        "                    && lexical.binder(a.value) == Some(crate::lexical::Binder::Param(p))\n",
        "                    && (lexical.binder(a.value) == Some(crate::lexical::Binder::Param(p)) || true)\n",
    ),
    (
        "a view's call is followed as the page's",
        "cargo",
        SPECULATION,
        "            && !calls(v.unit, vb).is_empty()\n",
        "            && !calls(v.unit, vb).is_empty()\n            && false\n",
    ),
    (
        "a speculated entry is named without the page's parameters",
        "cargo",
        SERVER,
        "        .chain(route.iter().map(String::as_str))\n",
        "        .chain(route.iter().take(0).map(String::as_str))\n",
    ),
    (
        "a document's parameters name no entry",
        "cargo",
        SERVER,
        "            .filter_map(|k| params.get(k.as_str()?).cloned())\n",
        "            .filter_map(|k| params.get(k.as_str()?).cloned().filter(|_| false))\n",
    ),
    (
        "a frame is sent at the entry of no parameter",
        "cargo",
        SERVER,
        "                entry: speculated_entry(&doc.0, page, binding, &route),\n",
        "                entry: speculated_entry(&doc.0, page, binding, &[]),\n",
    ),
    (
        "a commit's basis names the entry of no parameter",
        "cargo",
        SERVER,
        "                        \"entry\": speculated_entry(s, page, binding, route),\n",
        "                        \"entry\": speculated_entry(s, page, binding, &route[..0]),\n",
    ),
    (
        "a region a speculation renders does not hold the page's parameters",
        "cargo",
        SPECULATION,
        "                || params.contains(root)\n",
        "",
    ),
    (
        "a speculating document carries no parameters",
        "cargo",
        SERVER,
        "        manifest[\"params\"] = serde_json::json!(params);\n",
        "        let _ = &params;\n",
    ),
    (
        "the feed's reply is not speculated",
        "browser",
        FEED,
        "    optimistic    Thread(to) as thread => replied(thread, text)\n",
        "",
    ),
]

CARGO = [
    [
        "cargo", "test", "--quiet", "--locked", "-p", "pw-core",
        "--test", "speculated_routes",
    ],
    [
        "cargo", "test", "--quiet", "--locked", "-p", "pw-dev-server", "--",
        "a_speculated_thread_is_named", "a_speculating_document_carries",
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
