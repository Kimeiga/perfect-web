#!/usr/bin/env python3
"""Mutation controls for ADR-0196: only a word that begins a statement or an
expression is reserved (ADR-0195, ruling 3).

Each mutant undoes one piece of the parser's rule:
- the words that begin an expression are reserved;
- a pattern's binding is checked, and `true` and `false` are literals there;
- a declaration may be named by a statement word, not an expression word;
- the repair suggests a name.

Every mutant must fail the parser's test of the rule.

Run from the repository root; `just e14-reserved-words` records the output.
The source is restored after every mutant, whatever happens.
"""

import os
import pathlib
import re
import signal
import subprocess
import sys

ROOT = pathlib.Path(__file__).resolve().parent.parent
GRAMMAR = ROOT / "compiler/pw-syntax/src/grammar.rs"

# (what, file, anchor, replacement)
MUTANTS = [
    (
        "a word that begins an expression names a binding",
        GRAMMAR,
        "    STMT_KEYWORDS.contains(&word) || EXPR_KEYWORDS.contains(&word) || word == \"derived\"\n",
        "    STMT_KEYWORDS.contains(&word) || word == \"derived\"\n",
    ),
    # Re-anchored by ADR-0197: the parser decides a pattern's name by its
    # capital, and `true` and `false` are `Bool`'s cases there.
    (
        "a pattern's binding is not checked",
        GRAMMAR,
        "                if !is_ctor {\n                    self.not_a_statement_keyword(\"a binding\");\n",
        "                if false {\n                    self.not_a_statement_keyword(\"a binding\");\n",
    ),
    (
        "`true` in a pattern is refused as a name",
        GRAMMAR,
        "                    || matches!(self.cur_text(), \"true\" | \"false\");\n",
        "                    ;\n",
    ),
    (
        "a declaration is refused a statement word",
        GRAMMAR,
        "            && (self.cur_text() == \"derived\"\n"
        "                || (EXPR_KEYWORDS.contains(&self.cur_text())\n"
        "                    && !matches!(self.cur_text(), \"signal\" | \"provide\")))\n",
        "            && reserved(self.cur_text())\n",
    ),
    (
        "a function named by an expression word is accepted",
        GRAMMAR,
        "            self.not_an_expression_keyword(\"a function\");\n",
        "",
    ),
    (
        "the repair suggests no name",
        GRAMMAR,
        "                format!(\"name it `{word}_`, or after what it holds\"),\n",
        "                String::new(),\n",
    ),
]

TESTS = [
    "cargo", "test", "--quiet", "--locked", "-p", "pw-syntax", "--",
    "a_statement_keyword_cannot_name_a_value",
]

# How long one command may run. Past the bound it is killed with everything
# it started, and the run counts as failing.
BOUND = 1200


def run_tests():
    """(built, passed, failed) over the tests."""
    p = subprocess.Popen(TESTS, cwd=ROOT, start_new_session=True, stdout=subprocess.PIPE,
                         stderr=subprocess.STDOUT, text=True)
    try:
        out, _ = p.communicate(timeout=BOUND)
    except subprocess.TimeoutExpired:
        os.killpg(p.pid, signal.SIGKILL)
        p.communicate()
        return True, 0, 1
    found = re.findall(r"test result: \w+\. (\d+) passed; (\d+) failed", out)
    if not found:
        return False, 0, 0
    return True, sum(int(n) for n, _ in found), sum(int(f) for _, f in found)


def main():
    # Stopped from outside, the source is still restored: SIGTERM raises
    # here, and the `finally` below runs.
    signal.signal(signal.SIGTERM, lambda *_: sys.exit(1))
    built, passed, failed = run_tests()
    print(f"baseline: {passed} passed, {failed} failed", flush=True)
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
        print(f"{what}: {verdict}", flush=True)

    print(f"{len(MUTANTS) - survivors} of {len(MUTANTS)} mutants killed")
    return 1 if survivors else 0


if __name__ == "__main__":
    sys.exit(main())
