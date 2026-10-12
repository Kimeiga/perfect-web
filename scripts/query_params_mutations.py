#!/usr/bin/env python3
"""Mutation controls for ADR-XXXX: a page's query parameters are typed, and a
GET form makes them.

Each mutant undoes one piece:
- the checker: a route's query part not read; a query's parameter not held
  to an `Option` of text (PW0630); a link's query, or a GET form's fields,
  naming anything (PW5050); two routes that differ in their query two;
- the host: a `+` read as itself in a query; the last of a name taken; bytes
  that are no UTF-8 replaced; a query's parameter given as text;
- the request: a routed page's query not read at all.

A checker mutant must fail `compiler/pw-core/tests/routes.rs`; a host
mutant, the host's test of a query read as a browser's form writes it; the
request's, `e2e/query.spec.mjs` in Chromium.

Run from the repository root; `just e14-query-params` records the output.
The source is restored after every mutant, whatever happens.
"""

import os
import pathlib
import re
import signal
import subprocess
import sys

ROOT = pathlib.Path(__file__).resolve().parent.parent
ROUTES = ROOT / "compiler/pw-core/src/routes.rs"
SERVER = ROOT / "spikes/own-renderer/server/src/main.rs"

# (what, suite, file, anchor, replacement)
MUTANTS = [
    (
        "a route's query part is not read",
        "core",
        ROUTES,
        "            if route.ends_with('}') && !route[at + 2..route.len() - 1].contains(['{', '}']) =>\n",
        "            if false && route.ends_with('}') && !route[at + 2..route.len() - 1].contains(['{', '}']) =>\n",
    ),
    (
        "a query's parameter need not be an Option of text",
        "core",
        ROUTES,
        "    ty.as_builtin() == Some(crate::resolved::Builtin::Option)\n"
        "        && ty.args().first().is_some_and(|a| is_text(sigs, a))\n",
        "    let _ = (sigs, ty);\n    true\n",
    ),
    (
        "a link's query names anything",
        "core",
        ROUTES,
        "    let names = route_parts(route).1;\n    if names.contains(&name) {\n",
        "    let names = route_parts(route).1;\n    if true || names.contains(&name) {\n",
    ),
    (
        "a GET form's fields are not checked",
        "core",
        ROUTES,
        '        if method == "GET" {\n',
        '        if method == "GET" && false {\n',
    ),
    (
        "two routes that differ in their query are two",
        "core",
        ROUTES,
        "    let shape = |route: &str| -> String {\n        route_parts(route)\n            .0\n",
        "    let shape = |route: &str| -> String {\n        route\n",
    ),
    (
        "a `+` is itself in a query",
        "host",
        SERVER,
        "    let decoded = |v: &str| path_decoded(&v.replace('+', \" \"));\n",
        "    let decoded = |v: &str| path_decoded(v);\n",
    ),
    (
        "the last of a name is taken",
        "host",
        SERVER,
        "        if !names.contains(&name.as_str()) || out.contains_key(&name) {\n",
        "        if !names.contains(&name.as_str()) {\n",
    ),
    (
        "bytes that are no UTF-8 are replaced",
        "host",
        SERVER,
        "        out.insert(name, decoded(value)?);\n",
        "        out.insert(name, decoded(value).unwrap_or_default());\n",
    ),
    (
        "a query's parameter is given as text",
        "host",
        SERVER,
        "    if route_query_names(route).contains(&name) {\n",
        "    if false && route_query_names(route).contains(&name) {\n",
    ),
    (
        "a routed page's query is not read",
        "browser",
        SERVER,
        '        "GET" => server.routed(route, query),\n',
        '        "GET" => server.routed(route, ""),\n',
    ),
]

CARGO = {
    "core": ["cargo", "test", "--quiet", "--locked", "-p", "pw-core", "--test", "routes"],
    "host": [
        "cargo", "test", "--quiet", "--locked", "-p", "pw-dev-server", "--",
        "a_query_is_read_as_a_browsers_form_writes_it",
        "each_store_is_served_at_its_route",
    ],
}
SPEC = ["e2e/query.spec.mjs", "--project=chromium"]

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
    out, code = bounded(CARGO[suite], cwd=ROOT)
    if code is None:
        return True, 0, 1
    found = re.findall(r"test result: \w+\. (\d+) passed; (\d+) failed", out)
    if not found:
        return False, 0, 0
    return True, sum(int(p) for p, _ in found), sum(int(f) for _, f in found)


def browser_tests(_suite="browser"):
    """(built, passed, failed), after a build: the store's pages and the
    demo's, and the servers the suite starts."""
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
        ["cargo", "build", "--quiet", "--locked", "-p", "pw-dev-server", "-p", "kiokun-server"],
        cwd=ROOT,
        capture_output=True,
        text=True,
    )
    if server.returncode != 0:
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


SUITES = {
    "core": cargo_tests,
    "host": cargo_tests,
    "browser": browser_tests,
}


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
    # The pages and the server are built again from the restored source.
    browser_tests()

    print(f"{len(MUTANTS) - survivors} of {len(MUTANTS)} mutants killed")
    return 1 if survivors else 0


if __name__ == "__main__":
    sys.exit(main())
