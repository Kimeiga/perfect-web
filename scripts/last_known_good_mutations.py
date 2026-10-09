#!/usr/bin/env python3
"""Mutation controls for ADR-0177: last-known-good, for declared public data
only (charter §15.6 test 18).

Each mutant undoes one piece:
- the runtime: nothing answered with what was kept; a private manifest's
  read answered with it; a private value kept under a public read's key
  answered with it; `expire` expiring nothing; an expired value still
  fresh;
- the checker: PW0343 not raised; raised for public data;
- the compiler: the plan not carrying `fallback`;
- the server: the manifest not reading it; the store's control not
  expiring what was kept; the store's origin not failing.

A runtime mutant must fail `pw-resource`'s tests, all of them run; a
checker or compiler mutant, `pw-core`'s; a server mutant, the development
server's.

Run from the repository root; `just e14-last-known-good` records the
output. The source is restored after every mutant, whatever happens.
"""

import os
import pathlib
import re
import signal
import subprocess
import sys

ROOT = pathlib.Path(__file__).resolve().parent.parent
RESOURCE = ROOT / "runtime/pw-resource/src/lib.rs"
RULES = ROOT / "compiler/pw-core/src/rules.rs"
PLAN = ROOT / "compiler/pw-core/src/page_values.rs"
SERVER = ROOT / "spikes/own-renderer/server/src/main.rs"
STORE_DATA = ROOT / "spikes/own-renderer/server/src/store.rs"

# (what, suite, file, anchor, replacement)
MUTANTS = [
    (
        "nothing is answered with what was kept",
        "resource",
        RESOURCE,
        "        if matches!(fetched, Fetched::Failed(_) | Fetched::TimedOut)\n",
        "        if false && matches!(fetched, Fetched::Failed(_) | Fetched::TimedOut)\n",
    ),
    (
        "a private manifest's read is answered with what was kept",
        "resource",
        RESOURCE,
        "            && manifest.fallback == Fallback::LastKnownGood\n"
        "            && manifest.privacy == Privacy::Public\n",
        "            && manifest.fallback == Fallback::LastKnownGood\n",
    ),
    (
        "a private value kept under a public read's key answers it",
        "resource",
        RESOURCE,
        "                .filter(|e| e.privacy == Privacy::Public)\n",
        "",
    ),
    (
        "`expire` expires nothing",
        "resource",
        RESOURCE,
        "                entry.expired = true;\n",
        "                let _ = &entry;\n",
    ),
    (
        "an expired value is still fresh",
        "resource",
        RESOURCE,
        "                if !entry.expired && now.saturating_sub(entry.stored_at) < manifest.freshness {\n",
        "                if now.saturating_sub(entry.stored_at) < manifest.freshness {\n",
    ),
    (
        "PW0343 is not raised",
        "core",
        RULES,
        "        && f.value.trim() == \"last_known_good\"\n"
        "        && (matches!(visibility, \"session\" | \"private\")\n",
        "        && f.value.trim() == \"last_known_good\"\n"
        "        && false && (matches!(visibility, \"session\" | \"private\")\n",
    ),
    (
        "PW0343 is raised for public data",
        "core",
        RULES,
        "        && (matches!(visibility, \"session\" | \"private\")\n",
        "        && (true\n",
    ),
    (
        "the plan does not carry `fallback`",
        "core",
        PLAN,
        "        fallback: decl.policy(\"fallback\").map(|p| p.value.trim().to_string()),\n",
        "        fallback: None,\n",
    ),
    (
        "the manifest does not read `fallback`",
        "server",
        SERVER,
        "    if policy[\"fallback\"] == \"last_known_good\" {\n",
        "    if false && policy[\"fallback\"] == \"last_known_good\" {\n",
    ),
    (
        "the store's control does not expire what was kept",
        "server",
        SERVER,
        "                    server.queries.expire(&resource);\n",
        "                    let _ = &resource;\n",
    ),
    (
        "the store's origin does not fail",
        "server",
        STORE_DATA,
        # Re-anchored by track store-pg: the store's operations, built once
        # over either layer's rows.
        "                if store_fails.swap(false, Ordering::SeqCst) {\n",
        "                if false && store_fails.swap(false, Ordering::SeqCst) {\n",
    ),
]

CARGO = {
    "resource": ["cargo", "test", "--quiet", "--locked", "-p", "pw-resource"],
    "core": ["cargo", "test", "--quiet", "--locked", "-p", "pw-core"],
    "server": ["cargo", "test", "--quiet", "--locked", "-p", "pw-dev-server"],
}

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


def main():
    import mutation_baseline  # keeps what each command says, for a red baseline
    # Stopped from outside, the source is still restored: SIGTERM raises
    # here, and the `finally` below runs.
    signal.signal(signal.SIGTERM, lambda *_: sys.exit(1))
    for suite in CARGO:
        built, passed, failed = cargo_tests(suite)
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
            built, passed, failed = cargo_tests(suite)
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

    print(f"{len(MUTANTS) - survivors} of {len(MUTANTS)} mutants killed")
    return 1 if survivors else 0


if __name__ == "__main__":
    sys.exit(main())
