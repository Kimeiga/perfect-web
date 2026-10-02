#!/usr/bin/env python3
"""Mutation controls for ADR-0141: a dialog a signal shows is the browser's
modal dialog.

Each mutant undoes one part of the rule that a `<dialog>` written without
`open` is shown by a signal's block and says what closing it does. The tests
in `dialogs.rs` must then fail. What the browser does with it is
`e2e/dialog.spec.mjs`'s, in the own-renderer suite.

Run from the repository root; `just e14-dialogs` records the output. The
source is restored after every mutant, whatever happens.
"""

import pathlib
import re
import subprocess
import sys

ROOT = pathlib.Path(__file__).resolve().parent.parent
SIGNALS = ROOT / "compiler/pw-core/src/signals.rs"

# (what is undone, file, anchor, replacement)
MUTANTS = [
    (
        "a dialog's closing need not be heard",
        SIGNALS,
        "                        Some(signal) if !closes => {\n",
        "                        Some(signal) if !closes && false => {\n",
    ),
    (
        "a dialog nothing shows is accepted",
        SIGNALS,
        "                        None => out.push(dialog_shown_by_nothing(body, n)),\n",
        "                        None => {}\n",
    ),
    (
        "a block a parameter decides shows a dialog",
        SIGNALS,
        "                let decided = subject.and_then(signal_of).or(each);\n",
        "                let decided = subject.map(|_| \"a block\".to_string()).or(each);\n",
    ),
    (
        "a body with a dialog alone is not read",
        SIGNALS,
        "    if body.signals.is_empty() && handlers.is_empty() && !dialog {\n",
        "    if body.signals.is_empty() && handlers.is_empty() {\n",
    ),
    (
        "an open dialog is read as a modal one",
        SIGNALS,
        "                if tag == \"dialog\" && !attrs.iter().any(|a| a.name == \"open\") {\n",
        "                if tag == \"dialog\" {\n",
    ),
]

TESTS = [
    ["cargo", "test", "--quiet", "--locked", "-p", "pw-core", "--test", "dialogs"],
]


def run_tests():
    """(built, passed, failed) over every test command."""
    built, passed, failed = True, 0, 0
    for cmd in TESTS:
        r = subprocess.run(cmd, cwd=ROOT, capture_output=True, text=True)
        out = r.stdout + r.stderr
        m = re.search(r"test result: \w+\. (\d+) passed; (\d+) failed", out)
        if m is None:
            built = False
            continue
        passed += int(m.group(1))
        failed += int(m.group(2))
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
