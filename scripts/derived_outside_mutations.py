#!/usr/bin/env python3
"""Mutation controls for ADR-XXXX: a change is derived outside the table,
and what reaches a document is recorded.

Each mutant undoes one piece:
- a telling, or a keyed read, derived inside the table again;
- a change derived against what the document no longer shows pushed, by
  the telling's check, the keyed read's, or a comparison of what is shown
  by value;
- the last attempt derived outside too;
- a document's trail not kept, or answered to another session;
- a red baseline that shows no record, or shows every line as one.

A host mutant must fail the development server's tests of it; a script
mutant, `scripts/tests/test_mutation_baseline.py`.

Run from the repository root; `just e14-derived-outside` records the
output. The source is restored after every mutant, whatever happens.
"""

import os
import pathlib
import re
import signal
import subprocess
import sys

ROOT = pathlib.Path(__file__).resolve().parent.parent
SERVER = ROOT / "spikes/own-renderer/server/src/main.rs"
BASELINE = ROOT / "scripts/mutation_baseline.py"

# (what, suite, file, anchor, replacement)
MUTANTS = [
    (
        "a telling derives inside the table",
        "host",
        SERVER,
        "\n                let ahead = (!last).then(|| {\n",
        "\n                let ahead = false.then(|| {\n",
    ),
    (
        "a keyed read derives inside the table",
        "host",
        SERVER,
        "            let last = attempt >= DOCUMENT_ATTEMPTS;\n"
        "            let ahead = (!last).then(|| {\n",
        "            let last = attempt >= DOCUMENT_ATTEMPTS;\n"
        "            let ahead = false.then(|| {\n",
    ),
    (
        "a telling pushes what it derived against what is no longer shown",
        "host",
        SERVER,
        "                    Some((was, derived)) if same_shown(&was, &current) => derived,\n",
        "                    Some((_, derived)) => derived,\n",
    ),
    (
        "a keyed read applies what it derived against what is no longer shown",
        "host",
        SERVER,
        "                Some((before, Some(derived))) if same_shown(&before, &current) => derived?,\n",
        "                Some((_, Some(derived))) => derived?,\n",
    ),
    (
        "what is shown is the same whatever replaced it",
        "host",
        SERVER,
        "        (Some(a), Some(b)) => Arc::ptr_eq(a, b),\n",
        "        (Some(_), Some(_)) => true,\n",
    ),
    (
        "the last attempt is derived outside too",
        "host",
        SERVER,
        "                let last = attempt >= DOCUMENT_ATTEMPTS;\n",
        "                let last = attempt > DOCUMENT_ATTEMPTS;\n",
    ),
    (
        "a document's trail is not kept",
        "host",
        SERVER,
        '        self.trail.push_back(format!("{} ms {what}", uptime_ms()));\n',
        "",
    ),
    (
        "a document's trail is answered to another session",
        "host",
        SERVER,
        "            let doc: Doc = (session.clone(), document);\n",
        "            let doc: Doc = server\n"
        "                .pending\n"
        "                .lock()\n"
        '                .expect("pending")\n'
        "                .keys()\n"
        "                .find(|d| d.1 == document)\n"
        "                .cloned()\n"
        "                .unwrap_or((session.clone(), document));\n",
    ),
    (
        "a red baseline shows no record",
        "scripts",
        BASELINE,
        "        for line in records(said):\n"
        '            print(f"    {line}", file=out)\n',
        "",
    ),
    (
        "every line is a record",
        "scripts",
        BASELINE,
        '    found = [l.strip() for l in text.splitlines() if l.lstrip().startswith("pw-record")]\n',
        "    found = [l.strip() for l in text.splitlines()]\n",
    ),
]

COMMANDS = {
    "host": [
        "cargo", "test", "--quiet", "--locked", "-p", "pw-dev-server", "--",
        "the_table_is_free", "a_document_changed", "a_documents_trail",
        "a_keyed_read_whose_document_changed",
    ],
    "scripts": [sys.executable, "-m", "unittest", "scripts/tests/test_mutation_baseline.py"],
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


def tests(suite):
    """(built, passed, failed) over the suite's tests."""
    out, code = bounded(COMMANDS[suite], cwd=ROOT)
    if code is None:
        return True, 0, 1
    if suite == "scripts":
        ran = re.search(r"^Ran (\d+) tests?", out, re.M)
        if not ran:
            return False, 0, 0
        failed = sum(int(n) for n in re.findall(r"(?:failures|errors)=(\d+)", out))
        return True, int(ran.group(1)) - failed, failed
    found = re.findall(r"test result: \w+\. (\d+) passed; (\d+) failed", out)
    if not found:
        return False, 0, 0
    return True, sum(int(p) for p, _ in found), sum(int(f) for _, f in found)


def main():
    import mutation_baseline  # keeps what each command says, for a red baseline
    # Stopped from outside, the source is still restored: SIGTERM raises
    # here, and the `finally` below runs.
    signal.signal(signal.SIGTERM, lambda *_: sys.exit(1))
    for suite in COMMANDS:
        built, passed, failed = tests(suite)
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
            built, passed, failed = tests(suite)
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
