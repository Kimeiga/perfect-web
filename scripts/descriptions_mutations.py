#!/usr/bin/env python3
"""Mutation controls for ADR-0166: the store and its items say what they
are, and a host's answer is read through the program's own types.

Each mutant undoes one piece:
- the engine: a host's answer passed as it is; a field the type does not
  declare passed in; a field it declares and the answer lacks filled with
  nothing; a list's rows, or a result's value, passed as they are;
- the store's page: its description, or its items', not shown;
- WebKit: the initial document is withheld while its slots settle, so
  it cannot paint before they are filled.

An engine mutant must fail `runtime/pw-host/tests/host_answers.rs` or the
development server's tests, which serve the benchmark's store from rows
that hold more than its types declare. A page mutant must fail the server's
tests; the WebKit mutant, `e2e/slots.spec.mjs` in WebKit.

Run from the repository root; `just e14-descriptions` records the output.
The source is restored after every mutant, whatever happens.
"""

import os
import pathlib
import re
import signal
import subprocess
import sys

ROOT = pathlib.Path(__file__).resolve().parent.parent
ENGINE = ROOT / "runtime/pw-host/src/lib.rs"
STORE = ROOT / "examples/store/app.pw"
SERVER = ROOT / "spikes/own-renderer/server/src/main.rs"

# (what, suite, file, anchor, replacement)
MUTANTS = [
    # Re-anchored by ADR-0194: a nested answer is made its nodes first.
    (
        "a host's answer is passed as it is",
        "engine",
        ENGINE,
        "                        for ((slot, v), ty) in results.iter_mut().zip(out).zip(ty.results()) {\n"
        "                            *slot = graph::tangle(v, &ty)\n"
        "                                .and_then(|v| project(v, &ty))\n"
        "                                .map_err(|e| wasmtime::Error::msg(format!(\"`{named}`: {e}\")))?;\n"
        "                        }\n",
        "                        for ((slot, v), _) in results.iter_mut().zip(out).zip(ty.results()) {\n"
        "                            let _ = &named;\n"
        "                            *slot = v;\n"
        "                        }\n",
    ),
    (
        "a field the type does not declare is passed in",
        "engine",
        ENGINE,
        "                    out.push((field.name.to_string(), project(v, &field.ty)?));\n"
        "                }\n"
        "                Val::Record(out)\n",
        "                    out.push((field.name.to_string(), project(v, &field.ty)?));\n"
        "                }\n"
        "                out.extend(given);\n"
        "                Val::Record(out)\n",
    ),
    (
        "a field the answer lacks is filled with nothing",
        "engine",
        ENGINE,
        # Re-anchored by ADR-0291: a field read by either of its names.
        "                        .or_else(|| given.remove(&written))\n"
        "                        .ok_or_else(|| match written == field.name {\n",
        "                        .or_else(|| given.remove(&written))\n"
        "                        .or(Some(Val::String(String::new())))\n"
        "                        .ok_or_else(|| match written == field.name {\n",
    ),
    (
        "a field is read by its WIT name alone",
        "engine",
        ENGINE,
        "                        .or_else(|| given.remove(&written))\n",
        "                        .or_else(|| given.remove(field.name))\n",
    ),
    (
        "a list's rows are passed as they are",
        "engine",
        ENGINE,
        "            (Val::List(items), Type::List(list)) => Val::List(each(items, &list.ty())?),\n",
        "            (Val::List(items), Type::List(_)) => Val::List(items),\n",
    ),
    (
        "a result's value is passed as it is",
        "engine",
        ENGINE,
        "            (Val::Result(Ok(v)), Type::Result(result)) => Val::Result(Ok(inner(v, result.ok())?)),\n",
        "            (Val::Result(Ok(v)), Type::Result(_)) => Val::Result(Ok(v)),\n",
    ),
    (
        "the store's page does not say what the store is",
        "server",
        STORE,
        "            <p id=\"store-description\">{store.description}</p>\n",
        "",
    ),
    (
        "the store's page does not say what each item is",
        "server",
        STORE,
        "                            <p>{item.description}</p>\n",
        "",
    ),
    (
        # The former content-size proxy was absorbed as the page grew.
        # Remove the behavior itself, independent of today's page text.
        "the initial store document is withheld while its slots settle",
        "webkit",
        SERVER,
        "            .and_then(|_| stream.write_all(shell.as_bytes()))\n",
        "            .and_then(|_| { let _ = shell; Ok(()) })\n",
    ),
]

CARGO = {
    "engine": [
        ["cargo", "test", "--quiet", "--locked", "-p", "pw-host", "--features", "engine",
         "--test", "host_answers"],
        # The development server's tests that see the engine mutants the
        # host's own tests above do not, and the server mutants', measured
        # by `scripts/mutant_killers.py` on 2026-10-12 (W6's ruling of
        # 2026-10-09): two per mutant where two fail.
        [
            "cargo", "test", "--quiet", "--locked", "-p", "pw-dev-server", "--", "--exact",
            "tests::a_block_a_query_decides_is_rendered",
            "tests::a_block_whose_rendering_changed_is_rendered_again",
        ],
    ],
    "server": [[
        "cargo", "test", "--quiet", "--locked", "-p", "pw-dev-server", "--", "--exact",
        "tests::the_store_and_each_item_say_what_they_are",
        "tests::the_store_sends_its_slots_after_its_own_content",
    ]],
}
BROWSER = [
    "e2e/slots.spec.mjs", "--project=webkit", "--grep",
    "the store is shown before its slots are filled",
]

# How long one command may run. Past the bound it is killed with everything
# it started, and the run counts as failing.
BOUND = 900


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
    """(built, passed, failed) over every cargo test command of the suite."""
    built, passed, failed = True, 0, 0
    for cmd in CARGO[suite]:
        out, code = bounded(cmd, cwd=ROOT)
        if code is None:
            return True, passed, failed + 1
        found = re.findall(r"test result: \w+\. (\d+) passed; (\d+) failed", out)
        if not found:
            built = False
            continue
        for p, f in found:
            passed += int(p)
            failed += int(f)
    return built, passed, failed


def browser_tests(_suite="webkit"):
    """(built, passed, failed) for the browser tests, after a build: the page,
    and the server the suite runs, which `run.sh` does not build."""
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
    out, code = bounded(
        ["pnpm", "exec", "playwright", "test", *BROWSER, "--reporter=line"],
        cwd=ROOT / "spikes/own-renderer",
    )
    if code is None:
        return True, 0, 1
    out = re.sub(r"\x1b\[[0-9;]*[A-Za-z]", "", out)
    passed = sum(int(n) for n in re.findall(r"(\d+) passed", out))
    failed = sum(int(n) for n in re.findall(r"(\d+) failed", out))
    return passed + failed > 0, passed, failed


SUITES = {"engine": cargo_tests, "server": cargo_tests, "webkit": browser_tests}


def main():
    import mutation_baseline  # keeps what each command says, for a red baseline
    # Stopped from outside, the source is still restored: SIGTERM raises
    # here, and the `finally` below runs.
    signal.signal(signal.SIGTERM, lambda *_: sys.exit(1))
    for suite, run in SUITES.items():
        built, passed, failed = run(suite)
        print(f"baseline ({suite}): {passed} passed, {failed} failed")
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
        print(f"{what} [{suite}]: {verdict}")
    # The page and the server are built again from the restored source.
    built, passed, failed = browser_tests()
    if not built or failed or not passed:
        print("FAIL: the restored-source browser check is not green")
        mutation_baseline.explain()
        return 1

    print(f"{len(MUTANTS) - survivors} of {len(MUTANTS)} mutants killed")
    return 1 if survivors else 0


if __name__ == "__main__":
    sys.exit(main())
