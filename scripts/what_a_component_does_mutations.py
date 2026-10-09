#!/usr/bin/env python3
"""Mutation controls for ADR-0283 and ADR-0284, from W6's report.

ADR-0283, a component's contract is what its code does. Each mutant undoes
one piece:
- the code compiled into a component walked for its host calls: functions
  called, functions named as values, a handler's code left to the handler,
  and a local's call left to the local. (A
  recursion walked again never ends, which no bounded run reports sooner
  than its bound; `a_recursion_is_walked_once` hangs instead.)
- what a declaration performs read whole, function values included, by
  placement and the contract;
- only a handler's work deferred: an `on:` lambda's body and the declaration
  an `on:` attribute names.

ADR-0284, a value of any type is fixed by the call that meets it:
- each part of any type a variable of its own, closing to any type where
  nothing fixes it;
- a fold's untyped seed given the fold's solved type, and none where a part
  is of any type.

The tests of each must then fail: `what_a_component_does.rs`, `any_type.rs`
and `component_contract.rs`, whose control holds that a page does not
inherit its handlers' authority.

Run from the repository root; `just e14-what-a-component-does` records the
output. The source is restored after every mutant, whatever happens.
"""

import os
import pathlib
import re
import signal
import subprocess
import sys

ROOT = pathlib.Path(__file__).resolve().parent.parent
CONTRACT = ROOT / "compiler/pw-core/src/contract.rs"
EFFECTS = ROOT / "compiler/pw-core/src/effects.rs"
VALUES = ROOT / "compiler/pw-core/src/values.rs"
LOWER = ROOT / "compiler/pw-core/src/backend/lower.rs"

MUTANTS = [
    # ADR-0283, decision 1: the code compiled in.
    (
        "the declaration's own body is read alone",
        CONTRACT,
        "                pending.push((def, hirs[def.unit].body(b).walk()));\n",
        "                let _ = b;\n",
    ),
    (
        "a function named as a value is not compiled in",
        CONTRACT,
        "                Expr::Name(_) | Expr::Field { .. } if !called.contains(&expr) => (expr, false),\n",
        "",
    ),
    (
        "a handler's code is walked as the page's",
        CONTRACT,
        "    let exprs: Vec<crate::hir::ExprId> = exprs.into_iter().filter(|e| !in_handler(*e)).collect();\n",
        "    let _ = in_handler;\n",
    ),
    (
        "a call through a local is its namesake's",
        CONTRACT,
        "            if local(named) {\n                continue;\n            }\n            let path = crate::infer::path_of(body, named);\n",
        "            let path = crate::infer::path_of(body, named);\n",
    ),
    # ADR-0283, decision 2: what a declaration performs, read whole.
    (
        "effective effects count calls alone",
        EFFECTS,
        "                let mut out: Vec<String> = self\n                    .performed(unit, hir, id, b)\n",
        "                let mut out: Vec<String> = self\n                    .infer_at(unit, b)\n",
    ),
    (
        "the contract's effects count calls alone",
        EFFECTS,
        "subtracted(self.performed(unit, hir, id, b), b, exclude)\n",
        "subtracted(self.infer_at(unit, b), b, exclude)\n",
    ),
    # ADR-0283, decision 3: only a handler's work deferred.
    (
        "every lambda's body is deferred",
        CONTRACT,
        "                    let mut deferred: Vec<_> = crate::resume::handler_lambdas(body)\n                        .into_iter()\n                        .map(|e| match body.expr(e) {\n                            Expr::Lambda { body: inner, .. } => *inner,\n                            _ => e,\n                        })\n                        .collect();\n",
        "                    let mut deferred: Vec<_> = body\n                        .walk()\n                        .into_iter()\n                        .filter_map(|e| match body.expr(e) {\n                            Expr::Lambda { body: inner, .. } => Some(*inner),\n                            _ => None,\n                        })\n                        .collect();\n",
    ),
    (
        "a declaration an `on:` attribute names is not deferred",
        CONTRACT,
        "                        .map(|e| match body.expr(e) {\n                            Expr::Lambda { body: inner, .. } => *inner,\n                            _ => e,\n                        })\n",
        "                        .filter_map(|e| match body.expr(e) {\n                            Expr::Lambda { body: inner, .. } => Some(*inner),\n                            _ => None,\n                        })\n",
    ),
    # ADR-0284, decision 1: each part of any type a variable.
    (
        "a part of any type is bound whole",
        VALUES,
        "            let t = s.holes(t);\n            s.bound.insert(*v, t);\n",
        "            s.bound.insert(*v, t.clone());\n",
    ),
    (
        "a part nothing fixes closes to unknown",
        VALUES,
        "                self.holes += 1;\n                self.any.insert(v);\n",
        "                self.holes += 1;\n",
    ),
    # ADR-0284, decision 2: the seed's solved type.
    (
        "a fold's untyped seed is given no type",
        LOWER,
        "            (EachKind::Fold, None, Some(g)) => self.solved_seed(body, g),\n",
        "            (EachKind::Fold, None, Some(_)) => None,\n",
    ),
    (
        "a part of any type is laid out as the unit type",
        LOWER,
        "            Ty::Var(_) | Ty::Unknown | Ty::Any => return None,\n",
        "            Ty::Any => Type::Unit,\n            Ty::Var(_) | Ty::Unknown => return None,\n",
    ),
]

TESTS = [
    "cargo", "test", "--quiet", "--locked", "-p", "pw-core",
    "--test", "what_a_component_does", "--test", "any_type",
    "--test", "component_contract",
]

# How long one command may run. Past it, it and what it started are stopped.
BOUND = 1800


def run_tests():
    """(built, passed, failed)."""
    p = subprocess.Popen(
        TESTS, cwd=ROOT, stdout=subprocess.PIPE, stderr=subprocess.STDOUT, text=True,
        start_new_session=True,
    )
    try:
        out, _ = p.communicate(timeout=BOUND)
    except subprocess.TimeoutExpired:
        os.killpg(p.pid, signal.SIGKILL)
        out, _ = p.communicate()
        return True, 0, 1
    results = re.findall(r"test result: \w+\. (\d+) passed; (\d+) failed", out)
    if not results:
        return False, 0, 0
    return True, sum(int(a) for a, _ in results), sum(int(b) for _, b in results)


def main():
    import mutation_baseline  # keeps what each command says, for a red baseline
    # Stopped from outside, the source is still restored: a stop is an
    # exception here, which the `finally` below meets.
    signal.signal(signal.SIGTERM, lambda *_: sys.exit(143))
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
