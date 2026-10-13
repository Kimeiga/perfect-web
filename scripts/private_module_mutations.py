#!/usr/bin/env python3
"""Mutation controls for ADR-0324: `private` says a declaration is not
importable, and nothing more.

Each mutant undoes one piece:
- the checker: `private` the user's again, in the labels;
- the resolution: `private` on a declaration that holds data accepted, and
  each kind the refusal names (a query, a page, a layout, a command) left
  out; and PW0023 telling a reader to make a declaration public;
- the parser: a `user` that opens a declaration read as a policy of the one
  before, or as its clause's value.

Each must fail `compiler/pw-core/tests/private_is_not_importable.rs` or
`user_visibility.rs`, or `pw-syntax`'s
`a_user_declaration_after_one_with_no_body_is_its_own`.

Run from the repository root; `just e14-private-module` records the output.
The source is restored after every mutant, whatever happens.
"""

import os
import pathlib
import re
import signal
import subprocess
import sys

ROOT = pathlib.Path(__file__).resolve().parent.parent
CHECK = ROOT / "compiler/pw-core/src/check.rs"
RESOLVE = ROOT / "compiler/pw-core/src/resolve.rs"
GRAMMAR = ROOT / "compiler/pw-syntax/src/grammar.rs"

# (what, file, anchor, replacement)
MUTANTS = [
    (
        "`private` is the user's in the labels again",
        CHECK,
        "        Some(\"user\") => Label::user(\"UserId\"),\n",
        "        Some(\"private\") | Some(\"user\") => Label::user(\"UserId\"),\n",
    ),
    (
        "`private` on a declaration that holds data is accepted",
        RESOLVE,
        "                if decl.visibility.as_deref() == Some(\"private\")\n",
        "                if decl.visibility.as_deref() == Some(\"nothing\")\n",
    ),
    (
        "a `private` query is accepted",
        RESOLVE,
        "        DeclKind::Query => \"query\",\n",
        "",
    ),
    (
        "a `private` page is accepted",
        RESOLVE,
        "        DeclKind::Page => \"page\",\n",
        "",
    ),
    (
        "a `private` layout is accepted",
        RESOLVE,
        "        DeclKind::Layout => \"layout\",\n",
        "",
    ),
    (
        "a `private` command is accepted",
        RESOLVE,
        "        DeclKind::Command => \"command\",\n",
        "",
    ),
    (
        "PW0023 tells a reader to make the declaration public",
        CHECK,
        "            format!(\"drop `private` from `{name}` in `{module}`, or stop importing it\"),\n",
        "            format!(\"make `{name}` public in `{module}`, or stop importing it\"),\n",
    ),
    (
        "a `user` that opens a declaration is the last one's policy",
        GRAMMAR,
        "        if self.at_visibility() {\n            return false;\n        }\n        // A value must follow, on this line.",
        "        // A value must follow, on this line.",
    ),
    (
        "a `user` that opens a declaration is the last clause's value",
        GRAMMAR,
        "                        || self.at_unknown_policy()\n"
        "                        // And a declaration a contextual `user` opens\n"
        "                        // (ADR-0324): `user query` after `host \"..\"`.\n"
        "                        || self.at_visibility())\n",
        "                        || self.at_unknown_policy())\n",
    ),
]

CARGO = [
    [
        "cargo", "test", "--quiet", "--locked", "-p", "pw-core",
        "--test", "private_is_not_importable", "--test", "user_visibility",
    ],
    [
        "cargo", "test", "--quiet", "--locked", "-p", "pw-syntax", "--lib",
        "a_user_declaration_after_one_with_no_body_is_its_own",
    ],
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


def cargo_tests():
    """(built, passed, failed) over every command."""
    built, passed, failed = True, 0, 0
    for cmd in CARGO:
        out, code = bounded(cmd, cwd=ROOT)
        if code is None:
            failed += 1
            continue
        found = re.findall(r"test result: \w+\. (\d+) passed; (\d+) failed", out)
        if not found:
            built = False
            continue
        passed += sum(int(p) for p, _ in found)
        failed += sum(int(f) for _, f in found)
    return built, passed, failed


def main():
    import mutation_baseline  # keeps what each command says, for a red baseline
    # Stopped from outside, the source is still restored: SIGTERM raises
    # here, and the `finally` below runs.
    signal.signal(signal.SIGTERM, lambda *_: sys.exit(1))
    built, passed, failed = cargo_tests()
    print(f"baseline: {passed} passed, {failed} failed", flush=True)
    if not built or failed or not passed:
        print("FAIL: the unmutated baseline is not green; no mutant can mean anything")
        mutation_baseline.explain()
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
            built, passed, failed = cargo_tests()
        finally:
            path.write_text(original)
        if not built:
            verdict = "KILLED (does not build)"
        elif failed:
            verdict = f"KILLED ({failed} of {passed + failed} tests fail)"
        else:
            verdict = "SURVIVED"
            survivors += 1
        print(f"{what}: {verdict}", flush=True)

    print(f"{len(MUTANTS) - survivors} of {len(MUTANTS)} mutants killed")
    return 1 if survivors else 0


if __name__ == "__main__":
    sys.exit(main())
