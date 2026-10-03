#!/usr/bin/env python3
"""Mutation controls for ADR-0152: a key a page changes, and what its stale
work does.

Each mutant undoes one piece:
- in the compiler: a page query's signal argument read once again (PW5301),
  a key of any type (PW5308), a keyed query with no stale-work policy
  (PW5309), a signal argument not planned as the binding's key;
- in the server: an older read applied, a replaced page's read applied,
  `cancel` letting go of nothing, a slow source blind to its read's stop,
  `keep`'s reads run at once, and a flight stopped while another reader
  still waits for it.

A mutant must fail `keyed.rs` or the server's tests, all of them run.

Run from the repository root; `just e14-keyed-reads` records the output.
The source is restored after every mutant, whatever happens.
"""

import pathlib
import re
import subprocess
import sys

ROOT = pathlib.Path(__file__).resolve().parent.parent
SIGNALS = ROOT / "compiler/pw-core/src/signals.rs"
PLAN = ROOT / "compiler/pw-core/src/page_values.rs"
SERVER = ROOT / "spikes/own-renderer/server/src/main.rs"

# (what, file, anchor, replacement)
MUTANTS = [
    (
        "a page query's signal argument is read once",
        SIGNALS,
        "                if let Some((binding, query)) = keys.get(&id) {\n",
        "                if let Some((binding, query)) = keys.get(&id).filter(|_| false) {\n",
    ),
    (
        "a key of any type",
        SIGNALS,
        "    if !matches!(written.as_str(), \"String\" | \"Int\" | \"Bool\") {\n",
        "    if false {\n",
    ),
    (
        "a keyed query needs no stale-work policy",
        SIGNALS,
        "        .get(&query)\n        .is_some_and(Option::is_none)\n",
        "        .get(&query)\n        .is_some_and(|_| false)\n",
    ),
    (
        "a signal argument is not planned as the binding's key",
        PLAN,
        "                    keyed_by.push(n.clone());\n",
        "                    let _ = &keyed_by;\n",
    ),
    (
        "an older read is applied",
        SERVER,
        "            if seq <= read.latest {\n",
        "            if seq < read.latest && false {\n",
    ),
    (
        # Re-anchored by ADR-0161: a document is never replaced, and a read
        # for one the server does not hold must take nothing.
        "a read for a document the server does not hold is taken",
        SERVER,
        "            let Some(page) = keyed.get_mut(&doc) else {\n"
        "                return Ok(KeyOutcome::Superseded);\n"
        "            };\n",
        "            let page = keyed.entry(doc.clone()).or_default();\n",
    ),
    (
        "cancel lets go of nothing",
        SERVER,
        "        let cancel = b[\"policy\"][\"on_key_change\"] == \"cancel\";\n",
        "        let cancel = false;\n",
    ),
    (
        "a slow source does not see its read stopped",
        SERVER,
        "                let stopped: Stopped = Arc::new(move || cancellation.is_cancelled());\n",
        "                let stopped: Stopped = Arc::new(move || {\n                    let _ = &cancellation;\n                    false\n                });\n",
    ),
    (
        "keep's reads run at once",
        SERVER,
        "        let _turn = keep.then(|| turn.lock().expect(\"turn\"));\n",
        "        let _turn = keep.then(|| ());\n",
    ),
    (
        "a flight is stopped while another reader waits for it",
        SERVER,
        "        let _held = self.queries.subscribe(&key);\n",
        "        let _held = ();\n",
    ),
]

TESTS = [
    ["cargo", "test", "--quiet", "--locked", "-p", "pw-core", "--test", "keyed"],
    ["cargo", "test", "--quiet", "--locked", "-p", "pw-dev-server"],
]


def run_tests():
    """(built, passed, failed) over every test command."""
    built, passed, failed = True, 0, 0
    for cmd in TESTS:
        r = subprocess.run(cmd, cwd=ROOT, capture_output=True, text=True)
        found = re.findall(r"test result: \w+\. (\d+) passed; (\d+) failed", r.stdout + r.stderr)
        if not found:
            built = False
            continue
        for p, f in found:
            passed += int(p)
            failed += int(f)
    return built, passed, failed


def main():
    built, passed, failed = run_tests()
    print(f"baseline: {passed} passed, {failed} failed")
    if not built or failed or not passed:
        print("FAIL: the unmutated baseline is not green; no mutant can mean anything")
        return 1

    survivors = 0
    for what, path, anchor, replacement in MUTANTS:
        original = path.read_text()
        if original.count(anchor) != 1:
            print(f"{what}: ANCHOR NOT FOUND EXACTLY ONCE in {path.name}")
            survivors += 1
            continue
        try:
            path.write_text(original.replace(anchor, replacement, 1))
            built, passed, failed = run_tests()
        finally:
            path.write_text(original)
        if not built:
            verdict = "KILLED (does not build)"
        elif failed:
            verdict = f"KILLED ({failed} of {passed + failed} tests fail)"
        else:
            verdict = "SURVIVED"
            survivors += 1
        print(f"{what}: {verdict}")

    print(f"{len(MUTANTS) - survivors} of {len(MUTANTS)} mutants killed")
    return 1 if survivors else 0


if __name__ == "__main__":
    sys.exit(main())
