#!/usr/bin/env python3
"""Mutation controls for ADR-0173: a command is sent again where no answer
came, as its `retry` clause bounds it, and a command that is retried is
idempotent.

Each mutant undoes one piece:
- the checker: PW0312 lets `transport_only` retry a command that is not
  idempotent again; or refuses `retry none`, which retries nothing;
- the compiler: the clause not read, or the
  policy not passed where the handler sends the command;
- the program: the store's `add_to_cart` declares no retry;
- the browser runtime: nothing sent again; an answer sent again; no bound;
  a request sent again under another interaction.

A checker, compiler or program mutant must fail `pw-core`'s tests, all of
them run; a runtime mutant, `e2e/retry.spec.mjs` in Chromium.

Run from the repository root; `just e14-command-retry` records the output.
The source is restored after every mutant, whatever happens.
"""

import os
import pathlib
import re
import signal
import subprocess
import sys

ROOT = pathlib.Path(__file__).resolve().parent.parent
RULES = ROOT / "compiler/pw-core/src/rules.rs"
LOWER = ROOT / "compiler/pw-core/src/backend/lower.rs"
JS_PURE = ROOT / "compiler/pw-core/src/backend/js_pure.rs"
STORE = ROOT / "examples/store/app.pw"
RUNTIME = ROOT / "spikes/own-renderer/public/pw-runtime.mjs"

# (what, suite, file, anchor, replacement)
MUTANTS = [
    (
        "`transport_only` retries a command that is not idempotent",
        "core",
        RULES,
        "        && r.value.trim() != \"none\"\n",
        "        && r.value.trim() != \"none\"\n"
        "        && !crate::policy::applied(\"retry\", &r.value)\n"
        "            .is_some_and(|o| o.id == \"policy.retry.transport_only\")\n",
    ),
    (
        "`retry none` is refused as a retry",
        "core",
        RULES,
        "        && r.value.trim() != \"none\"\n",
        "",
    ),
    (
        "the clause is not read",
        "core",
        LOWER,
        "                .and_then(|p| crate::manifest::parse_retry(&p.value))\n",
        "                .and_then(|p| crate::manifest::parse_retry(&p.value))\n"
        "                .filter(|_| false)\n",
    ),
    # "`fixed` is read as `exponential`" is retired with `fixed` (ADR-0215).
    (
        "the policy is not passed where the command is sent",
        "core",
        JS_PURE,
        "                    \"const {answered} = await context.command({}, [{}]{how});\",\n",
        "                    \"const {answered} = await context.command({}, [{}]);\",\n",
    ),
    (
        "the store's `add_to_cart` declares no retry",
        "core",
        STORE,
        "    idempotent_by InteractionId\n"
        "    retry         transport_only(max = 2, jitter = true)\n"
        "    transaction   serializable\n"
        "    optimistic    Cart(current_user()) as cart => UserCarts.with_line(cart, item, quantity)\n",
        "    idempotent_by InteractionId\n"
        "    transaction   serializable\n"
        "    optimistic    Cart(current_user()) as cart => UserCarts.with_line(cart, item, quantity)\n",
    ),
    (
        # Re-anchored by ADR-0302: a request with no answer is thrown as
        # unreachable, which the press tells.
        "nothing is sent again",
        "browser",
        RUNTIME,
        "      if (!retry || attempt >= retry.max) throw new Unreachable(component, error);\n",
        "      throw new Unreachable(component, error);\n",
    ),
    (
        "an answer is sent again",
        "browser",
        RUNTIME,
        "      if (response.ok) body = await response.text();\n",
        "      if (!response.ok) throw new Error(`HTTP ${response.status}`);\n"
        "      body = await response.text();\n",
    ),
    (
        # Re-anchored by ADR-0302.
        "there is no bound",
        "browser",
        RUNTIME,
        "      if (!retry || attempt >= retry.max) throw new Unreachable(component, error);\n",
        "      if (!retry) throw new Unreachable(component, error);\n",
    ),
    (
        "a request is sent again under another interaction",
        "browser",
        RUNTIME,
        # Re-anchored by track store-accounts: the headers on lines of their
        # own, the page's document beside the interaction (Q3).
        '          "pw-interaction": interaction,\n',
        '          "pw-interaction": attempt === 0 ? interaction : `${interaction}-${attempt}`,\n',
    ),
]

CARGO = {
    "core": ["cargo", "test", "--quiet", "--locked", "-p", "pw-core"],
}
BROWSER = ["e2e/retry.spec.mjs", "--project=chromium"]

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


SUITES = {"core": cargo_tests, "browser": browser_tests}


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
