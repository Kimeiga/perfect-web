#!/usr/bin/env python3
"""Mutation controls for ADR-0231: a page's parameter is rendered on every
page, and the feed's replies.

Each mutant undoes one piece: the document rendered with its parameters; a
block a host renders again, and a row, rendered with them; a text part that
reads one, planned as nothing a host sets again; the refusal of a value
computed from one, named; and the feed's reply, a post that replies to its
`to`, and none to a post that is not there.

The compiler's and the server's tests must fail for a mutant of the compiler
or the server ("cargo"). One mutant of the feed's data must fail
`e2e/feed.spec.mjs` in three engines, against a build of the mutated source
("browser").

Run from the repository root; `just e14-page-parameters` records the output.
The source is restored after every mutant, whatever happens.
"""

import os
import pathlib
import re
import signal
import subprocess
import sys

ROOT = pathlib.Path(__file__).resolve().parent.parent
VALUES = ROOT / "compiler/pw-core/src/page_values.rs"
SERVER = ROOT / "spikes/own-renderer/server/src/main.rs"
FEED = ROOT / "spikes/own-renderer/server/src/feed.rs"

# (what is undone, suite, file, anchor, replacement)
MUTANTS = [
    (
        "a page that binds a query is rendered without its parameters",
        "cargo",
        SERVER,
        "        for (name, given) in params {\n"
        "            env = env.set(name, Value::Text(given.clone()));\n"
        "        }\n"
        "        // Each binding's whole value, so a block a query decides, and what is\n",
        "        let _ = params;\n"
        "        // Each binding's whole value, so a block a query decides, and what is\n",
    ),
    (
        "a block a host renders again is rendered without the page's parameters",
        "cargo",
        SERVER,
        "        // Each block a query decides, as it renders now (ADR-0146).\n"
        "        let env = self.document_env(page, session, params, bindings);\n",
        "        // Each block a query decides, as it renders now (ADR-0146).\n"
        "        let env = self.document_env(page, session, &Params::new(), bindings);\n",
    ),
    (
        "a row a change renders is rendered without the page's parameters",
        "cargo",
        SERVER,
        "        let env = self.document_env(page, session, params, bindings);\n"
        "        for (list, items) in &now.lists {\n",
        "        let env = self.document_env(page, session, &Params::new(), bindings);\n"
        "        for (list, items) in &now.lists {\n",
    ),
    (
        "a text part that reads a page's parameter is refused",
        "cargo",
        VALUES,
        "            if !hole.nested && params.contains(&root) {\n"
        "                continue;\n"
        "            }\n",
        "",
    ),
    (
        "a value computed from a page's parameter is named as no query's",
        "cargo",
        VALUES,
        "        if params.iter().any(|p| p == root) {\n",
        "        if false {\n",
    ),
    (
        "a reply is no reply",
        "cargo",
        FEED,
        "                        reply_to: Some(to.clone()),\n",
        "                        reply_to: None,\n",
    ),
    (
        "a reply to a post that is not there is made",
        "cargo",
        FEED,
        "                    if !s.posts.iter().any(|r| r.id == *to) {\n",
        "                    if false {\n",
    ),
    (
        "a reply is no reply, in three engines",
        "browser",
        FEED,
        "                        reply_to: Some(to.clone()),\n",
        "                        reply_to: None,\n",
    ),
]

CARGO = [
    [
        "cargo", "test", "--quiet", "--locked", "-p", "pw-core",
        "--test", "page_parameters", "--test", "computed_holes",
    ],
    [
        "cargo", "test", "--quiet", "--locked", "-p", "pw-dev-server", "--",
        "a_page_that_binds_a_query_is_rendered", "a_row_a_change_renders",
        "a_reply_is_its_threads",
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
