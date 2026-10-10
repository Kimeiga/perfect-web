#!/usr/bin/env python3
"""Mutation controls for ADR-0300: a build is named by what it built, and
the browser's decision holds a document to what its build says of its page.

Each mutant undoes one piece:
- the build: no name written; a name that is not of what was built;
- the page's plan: every page's scope public;
- the host: a build that names none served; the manifest's build, document
  schema or scope a constant again; the table saying no page; a document
  answered without its build;
- the decision: a table's `#` line taken for a handler; the told schema not
  compared;
- the runtime: the decision told nothing of the page's documents.

A build or plan mutant must fail `pw-core`'s build-id tests or the host's; a
host mutant, the host's; a decision mutant, `pw-resume-wasm`'s; the
runtime's, `e2e/recovery.spec.mjs`'s test of a document of another schema,
in Chromium.

Run from the repository root; `just e14-build-id` records the output. The
source is restored after every mutant, whatever happens.
"""

import os
import pathlib
import re
import signal
import subprocess
import sys

ROOT = pathlib.Path(__file__).resolve().parent.parent
BUILD = ROOT / "compiler/pw-core/src/build.rs"
RESUME = ROOT / "compiler/pw-core/src/resume.rs"
SERVER = ROOT / "spikes/own-renderer/server/src/main.rs"
WASM = ROOT / "runtime/pw-resume-wasm/src/lib.rs"
RUNTIME = ROOT / "spikes/own-renderer/public/pw-runtime.mjs"

# (what, suite, file, anchor, replacement)
MUTANTS = [
    (
        "the build writes no name",
        "host",
        BUILD,
        '        write("build-id", format!("{id}\\n").as_bytes())?;\n',
        "",
    ),
    (
        "the name is not of what was built",
        "core",
        BUILD,
        "            named.set(fnv1a(fnv1a(named.get(), rel.as_bytes()), bytes));\n",
        "",
    ),
    (
        "every page's scope is public",
        "host",
        RESUME,
        '        Some(Restriction::Session(_)) => "session:",\n',
        '        Some(Restriction::Session(_)) => "public",\n',
    ),
    (
        "a build that names no build is served",
        "host",
        SERVER,
        '            .map_err(|e| format!("{e}: the build names no build; run `pw build` again"))?\n',
        '            .or_else(|_| Ok::<String, String>(String::new()))?\n',
    ),
    (
        "the manifest names a constant build",
        "host",
        SERVER,
        '        "scheme": "2", "abi": "1", "build": build, "handler": "",\n',
        '        "scheme": "2", "abi": "1", "build": BUILD, "handler": "",\n',
    ),
    (
        "the manifest names a constant document",
        "host",
        SERVER,
        '        "capture": "", "document": template.schema,\n',
        '        "capture": "", "document": "cart-doc",\n',
    ),
    (
        "the manifest says every page is public",
        "host",
        SERVER,
        '        "scope": plan["scope"].as_str().unwrap_or("public"),\n',
        '        "scope": "public",\n',
    ),
    (
        "the table says no page",
        "host",
        SERVER,
        '                    table.push_str(&format!(\n'
        '                        "#page|{page}|{}|{}\\n",\n',
        '                    let _ = table.len();\n'
        '                    let _ = (format!(\n'
        '                        "#page|{page}|{}|{}\\n",\n',
    ),
    (
        "a document is answered without its build",
        "host",
        SERVER,
        '        let build = format!("pw-build: {}\\r\\n", self.build_id);\n',
        "        let build = String::new();\n",
    ),
    (
        "a table's `#` line is taken for a handler",
        "wasm",
        WASM,
        "        .filter(|l| !l.is_empty() && !l.starts_with('#'))\n",
        "        .filter(|l| !l.is_empty())\n",
    ),
    (
        "the told schema is not compared",
        "wasm",
        WASM,
        "        document_schema: told.as_ref().map(|(s, _)| schema(s)),\n",
        "        document_schema: None,\n",
    ),
    (
        "the decision is told nothing of the page's documents",
        "browser",
        RUNTIME,
        "    know_document(...write(`${schema}|${scope}`));\n",
        "",
    ),
]

COMMANDS = {
    "core": ["cargo", "test", "--quiet", "--locked", "-p", "pw-core", "--test", "build_id"],
    "host": [
        "cargo", "test", "--quiet", "--locked", "-p", "pw-dev-server", "--",
        "a_host_serves_its_build", "a_build_that_names_no_build",
    ],
    "wasm": ["cargo", "test", "--quiet", "--locked", "-p", "pw-resume-wasm"],
}
SPECS = [
    ["e2e/recovery.spec.mjs", "-g", "another schema", "--project=chromium"],
]

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


def cargo_tests(suite):
    """(built, passed, failed) over the suite's tests."""
    out, code = bounded(COMMANDS[suite], cwd=ROOT)
    if code is None:
        return True, 0, 1
    found = re.findall(r"test result: \w+\. (\d+) passed; (\d+) failed", out)
    if not found:
        return False, 0, 0
    return True, sum(int(p) for p, _ in found), sum(int(f) for _, f in found)


def browser_tests(_suite="browser"):
    """(built, passed, failed), after a build: the store's page, which serves
    the runtime it was built with, and the server the suite runs."""
    built = subprocess.run(
        ["bash", "spikes/own-renderer/run.sh"],
        cwd=ROOT,
        env={**os.environ, "BUILD_ONLY": "1"},
        capture_output=True,
        text=True,
    )
    if built.returncode != 0:
        return False, 0, 0
    server = subprocess.run(
        ["cargo", "build", "--quiet", "--locked", "-p", "pw-dev-server"],
        cwd=ROOT,
        capture_output=True,
        text=True,
    )
    if server.returncode != 0:
        return False, 0, 0
    passed = failed = 0
    for spec in SPECS:
        out, code = bounded(
            ["pnpm", "exec", "playwright", "test", *spec, "--reporter=line"],
            cwd=ROOT / "spikes/own-renderer",
        )
        if code is None:
            failed += 1
            continue
        out = re.sub(r"\x1b\[[0-9;]*[A-Za-z]", "", out)
        passed += sum(int(n) for n in re.findall(r"(\d+) passed", out))
        failed += sum(int(n) for n in re.findall(r"(\d+) failed", out))
    return passed + failed > 0, passed, failed


SUITES = {"core": cargo_tests, "host": cargo_tests, "wasm": cargo_tests, "browser": browser_tests}


def main():
    import mutation_baseline  # keeps what each command says, for a red baseline
    # Stopped from outside, the source is still restored: SIGTERM raises
    # here, and the `finally` below runs.
    signal.signal(signal.SIGTERM, lambda *_: sys.exit(1))
    for suite, run in SUITES.items():
        built, passed, failed = run(suite)
        print(f"baseline ({suite}): {passed} passed, {failed} failed", flush=True)
        if not built or failed or not passed:
            print("FAIL: the unmutated baseline is not green; no mutant can mean anything")
            mutation_baseline.explain()
            return 1

    survivors = 0
    for what, suite, path, anchor, replacement in MUTANTS:
        original = path.read_text()
        if original.count(anchor) != 1:
            print(f"{what}: ANCHOR NOT FOUND EXACTLY ONCE in {path.name}")
            survivors += 1
            continue
        try:
            path.write_text(original.replace(anchor, replacement, 1))
            built, passed, failed = SUITES[suite](suite)
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
    # The page and the server are built again from the restored source.
    browser_tests()

    print(f"{len(MUTANTS) - survivors} of {len(MUTANTS)} mutants killed")
    return 1 if survivors else 0


if __name__ == "__main__":
    sys.exit(main())
