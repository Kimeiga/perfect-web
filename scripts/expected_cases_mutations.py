#!/usr/bin/env python3
"""Mutation controls for ADR-0201: a case written alone that several types
have is the case of the type expected where it is written (ADR-0195, ruling
5, its second half).

Each mutant undoes one piece: each position the expected type is read from,
the case taken from it, a call's callee resolved by it, PW0022 where nothing
expected says, the backend building the case the checker typed, and the
guard that stops a question leading back to itself.

Every mutant must fail the tests of the rule, or the compiled programs'.

Run from the repository root; `just e14-expected-cases` records the output.
The source is restored after every mutant, whatever happens.
"""

import os
import pathlib
import re
import signal
import subprocess
import sys

ROOT = pathlib.Path(__file__).resolve().parent.parent
VALUES = ROOT / "compiler/pw-core/src/values.rs"

# (what, file, anchor, replacement)
MUTANTS = [
    (
        "nothing expected resolves a case",
        VALUES,
        "        let Some(Ty::Nominal(def, _)) = self.expected(site) else {\n",
        "        let Some(Ty::Nominal(def, _)) = None::<Ty> else {\n",
    ),
    (
        "the other type's case is taken",
        VALUES,
        "        owners.into_iter().find(|(owner, _)| *owner == def)\n",
        "        owners.into_iter().find(|(owner, _)| *owner != def)\n",
    ),
    (
        "a question that leads back to itself is asked again",
        VALUES,
        "        if !self.expecting.borrow_mut().insert(e) {\n",
        "        if !self.expecting.borrow_mut().insert(e) && false {\n",
    ),
    (
        "a result is expected as nothing",
        VALUES,
        "        if self.result_sites().contains(&e) {\n",
        "        if self.result_sites().contains(&e) && false {\n",
    ),
    (
        "an annotation is expected as nothing",
        VALUES,
        "            } if *init == e => self.written(*t),\n",
        "            } if *init == e => None,\n",
    ),
    (
        "an argument is expected as nothing",
        VALUES,
        "                self.parameter(parent, *callee, args, k)\n",
        "                self.parameter(parent, *callee, args, k).filter(|_| false)\n",
    ),
    (
        "a comparison's side is expected as nothing",
        VALUES,
        "            } => Some(self.of(if *lhs == e { *rhs } else { *lhs })),\n",
        "            } => None,\n",
    ),
    (
        "a field is expected as nothing",
        VALUES,
        "                self.field_type(record, &field.name)\n",
        "                self.field_type(record, &field.name).filter(|_| false)\n",
    ),
    (
        "a list's item is expected as nothing",
        VALUES,
        "            Expr::List { items } => match self.expected(parent) {\n",
        "            Expr::List { items } if false => match self.expected(parent) {\n",
    ),
    (
        "a branch is expected as nothing",
        VALUES,
        "            Expr::If { then, els, .. } if *then == e || *els == Some(e) => self.expected(parent),\n",
        "            Expr::If { then, els, .. } if *then == e || *els == Some(e) => None,\n",
    ),
    (
        "an arm is expected as nothing",
        VALUES,
        "            Expr::Match { arms, .. } if arms.iter().any(|a| a.body == e) => self.expected(parent),\n",
        "            Expr::Match { arms, .. } if arms.iter().any(|a| a.body == e) => None,\n",
    ),
    (
        "a callee written alone is not resolved",
        VALUES,
        "        if let Some((def, index)) = self.case_from_expected(callee) {\n",
        "        if let Some((def, index)) = self.case_from_expected(callee).filter(|_| false) {\n",
    ),
    (
        "where nothing expected says, PW0022 is not reported",
        VALUES,
        "            Some(_) => Outcome::Agree,\n",
        "            _ => Outcome::Agree,\n",
    ),
    (
        "the backend does not build the case the checker typed",
        VALUES,
        "            Typer::new(sigs, ws, at, module, decl, body, lexical, Vec::new()).case_from_expected(e)\n",
        "            Typer::new(sigs, ws, at, module, decl, body, lexical, Vec::new()).case_from_expected(e).filter(|_| false)\n",
    ),
]

TESTS = [
    ["cargo", "test", "--quiet", "--locked", "-p", "pw-core", "--test", "sum_types"],
    ["cargo", "test", "--quiet", "--locked", "-p", "pw-conformance", "--test", "sum_types", "a_case_written_alone"],
    ["cargo", "test", "--quiet", "--locked", "-p", "pw-conformance", "--test", "javascript", "a_case_from_its_expected"],
]

# How long one command may run. Past the bound it is killed with everything
# it started, and the run counts as failing.
BOUND = 1200


def run_tests():
    """(built, passed, failed) over the tests. A test process that aborts,
    as a stack overflow does, fails."""
    passed = failed = 0
    for command in TESTS:
        p = subprocess.Popen(command, cwd=ROOT, start_new_session=True,
                             stdout=subprocess.PIPE, stderr=subprocess.STDOUT, text=True)
        try:
            out, _ = p.communicate(timeout=BOUND)
        except subprocess.TimeoutExpired:
            os.killpg(p.pid, signal.SIGKILL)
            p.communicate()
            return True, passed, failed + 1
        found = re.findall(r"test result: \w+\. (\d+) passed; (\d+) failed", out)
        if not found:
            if "could not compile" in out or "error[E" in out:
                return False, passed, failed
            # It built, and the process ended before it reported.
            failed += 1
            continue
        passed += sum(int(n) for n, _ in found)
        failed += sum(int(f) for _, f in found)
    return True, passed, failed


def main():
    import mutation_baseline  # keeps what each command says, for a red baseline
    # Stopped from outside, the source is still restored: SIGTERM raises
    # here, and the `finally` below runs.
    signal.signal(signal.SIGTERM, lambda *_: sys.exit(1))
    built, passed, failed = run_tests()
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
