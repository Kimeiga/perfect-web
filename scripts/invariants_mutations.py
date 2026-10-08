#!/usr/bin/env python3
"""Mutation controls for ADR-0179: an opaque type states its invariant, and
every construction and every boundary holds it.

Each mutant undoes one piece:
- the lowering: `>` read as `>=`; a bound written the other way round read as
  written;
- PW0623: an unread predicate, another representation, an empty range, or a
  bound beyond an `Int` let through;
- PW0622: no construction checked; no test narrowing; an `else` narrowed as
  its `if`; a conjunction narrowing where it fails; a bounded value's
  `.value` unbounded; a `let`'s local unbounded; subtraction read as
  addition; a branch read as its `then` alone;
- the contract: an export, or an import's answer, stating no invariant; a
  list's items not reached;
- the host: nothing held; an upper bound not held; a lower bound off by one;
  `arguments_for` decoding as `arguments`; an answer read unchecked; a value
  of another shape passed over;
- the server: decoding as the ABI alone;
- the store: `PositiveInt` stating nothing.

A lowering, rule, value-analysis or contract mutant must fail `pw-core`'s
tests, all of them run; a host mutant, `pw-host`'s with its engine; a server
or store mutant, the development server's.

Run from the repository root; `just e14-invariants` records the output. The
source is restored after every mutant, whatever happens.
"""

import os
import pathlib
import re
import signal
import subprocess
import sys

ROOT = pathlib.Path(__file__).resolve().parent.parent
LOWER = ROOT / "compiler/pw-core/src/lower.rs"
RULES = ROOT / "compiler/pw-core/src/rules.rs"
VALUES = ROOT / "compiler/pw-core/src/values.rs"
CONTRACT = ROOT / "compiler/pw-core/src/contract.rs"
HOST = ROOT / "runtime/pw-host/src/lib.rs"
SERVER = ROOT / "spikes/own-renderer/server/src/main.rs"
DOMAIN = ROOT / "examples/domain.pw"

# (what, suite, file, anchor, replacement)
MUTANTS = [
    (
        "`>` is read as `>=`",
        "core",
        LOWER,
        '                ">" => out.at_least = Some(out.at_least.map_or(k + 1, |lo| lo.max(k + 1))),\n',
        '                ">" => out.at_least = Some(out.at_least.map_or(k, |lo| lo.max(k))),\n',
    ),
    (
        "a bound written the other way round is read as written",
        "core",
        LOWER,
        # Re-anchored by ADR-0225: a side bounds a value or a length.
        '                    (Some(k), Some(m)) => {\n'
        '                        let flipped = match op.text() {\n',
        '                    (Some(k), Some(m)) => {\n'
        '                        let flipped = match "" {\n'
        '                            _ if true => op.text(),\n',
    ),
    (
        "PW0623 lets an unread predicate through",
        "core",
        RULES,
        "    for (span, why) in &inv.unread {\n",
        "    for (span, why) in inv.unread.iter().filter(|_| false) {\n",
    ),
    (
        "PW0623 reads any representation",
        "core",
        RULES,
        # Re-anchored by ADR-0225: an `Int`'s value, or a `String`'s length.
        "    if representation.as_deref() != Some(wanted) {\n",
        "    if false {\n",
    ),
    (
        "PW0623 lets an empty range through",
        "core",
        RULES,
        "        && lo > hi\n",
        "        && false\n",
    ),
    (
        "PW0623 lets a bound beyond an `Int` through",
        "core",
        RULES,
        "    if beyond(inv.at_least) || beyond(inv.at_most) {\n",
        "    if false {\n",
    ),
    (
        "no construction is checked",
        "core",
        VALUES,
        "                outcome: match found.within(inv) {\n",
        "                outcome: match true || found.within(inv) {\n",
    ),
    (
        "a test narrows nothing",
        "core",
        VALUES,
        "        while let Some(&parent) = self.parents.get(&child) {\n",
        "        while let Some(&parent) = self.parents.get(&child).filter(|_| false) {\n",
    ),
    (
        "an `else` is narrowed as its `if`",
        "core",
        VALUES,
        "                    } else if *els == Some(child) {\n"
        "                        out.extend(self.facts(*cond, false));\n",
        "                    } else if *els == Some(child) {\n"
        "                        out.extend(self.facts(*cond, true));\n",
    ),
    (
        "a conjunction narrows where it fails",
        "core",
        VALUES,
        "                op: BinOp::And,\n"
        "                lhs,\n"
        "                rhs,\n"
        "            } if holds => {\n",
        "                op: BinOp::And,\n"
        "                lhs,\n"
        "                rhs,\n"
        "            } => {\n",
    ),
    (
        "a bounded value's `.value` is any `Int`",
        "core",
        VALUES,
        "                    .map_or(Interval::ANY, |i| Interval {\n"
        "                        lo: i.at_least,\n",
        "                    .map_or(Interval::ANY, |i| Interval {\n"
        "                        lo: i.at_least.filter(|_| false),\n",
    ),
    (
        "a `let`'s local is any `Int`",
        "core",
        VALUES,
        # Re-anchored by ADR-0225, whose length reads a `let` the same way.
        "            Expr::Name(_) => match self.lexical.binder(e) {\n"
        "                Some(Binder::Pattern(p)) => match self.lets.get(&p) {\n"
        "                    Some(init) => {\n"
        "                        let there = self.facts_at(*init);\n"
        "                        self.interval(*init, &there, next)\n",
        "            Expr::Name(_) if false => match self.lexical.binder(e) {\n"
        "                Some(Binder::Pattern(p)) => match self.lets.get(&p) {\n"
        "                    Some(init) => {\n"
        "                        let there = self.facts_at(*init);\n"
        "                        self.interval(*init, &there, next)\n",
    ),
    (
        "subtraction is read as addition",
        "core",
        VALUES,
        "                    BinOp::Sub => a.add(b.neg()),\n",
        "                    BinOp::Sub => a.add(b),\n",
    ),
    (
        "a branch is its `then` alone",
        "core",
        VALUES,
        "                self.interval(*then, &yes, next)\n"
        "                    .hull(self.interval(*els, &no, next))\n",
        "                self.interval(*then, &yes, next)\n",
    ),
    (
        "an export states no argument's invariant",
        "core",
        CONTRACT,
        "            component.bounded = sigs\n",
        "            let _: Vec<Bounded> = sigs\n",
    ),
    (
        "an import states no answer's invariant",
        "core",
        CONTRACT,
        "                .map(|t| bounded_in(sigs, t, 0))\n",
        "                .map(|t| bounded_in(sigs, t, 0))\n"
        "                .filter(|_| false)\n",
    ),
    (
        "a list's items are not reached",
        "core",
        CONTRACT,
        '                step("*".into(), item, path, seen, out);\n',
        "                let _ = item;\n",
    ),
    (
        "the host holds nothing",
        "host",
        HOST,
        "        for check in checks {\n",
        "        for check in checks.iter().filter(|_| false) {\n",
    ),
    (
        "an upper bound is not held",
        "host",
        HOST,
        # Re-anchored by ADR-0225: a value or a length, compared alike.
        "            let below = check.at_most.is_none_or(|b| n <= i128::from(b));\n",
        "            let below = true;\n",
    ),
    (
        "a lower bound admits the value below it",
        "host",
        HOST,
        "            let above = check.at_least.is_none_or(|b| n >= i128::from(b));\n",
        "            let above = check.at_least.is_none_or(|b| n >= i128::from(b) - 1);\n",
    ),
    (
        "`arguments_for` decodes as `arguments`",
        "host",
        HOST,
        '            holds(&export.bounded, &args).map_err(|why| format!("argument refused: {why}"))?;\n',
        "            let _ = &export.bounded;\n",
    ),
    (
        "an answer is read without its invariants",
        "host",
        HOST,
        "                        holds(&bounded, results).map_err(|why| {\n",
        "                        Ok::<(), String>(()).map_err(|why| {\n"
        "                            let _ = &bounded;\n",
    ),
    (
        "a value of another shape is passed over",
        "host",
        HOST,
        '                None => Err(format!("{} has no field `{field}`", place(check, at))),\n',
        "                None => Ok(()),\n",
    ),
    (
        "the server decodes as the ABI alone",
        "server",
        SERVER,
        "        let args = loaded.prepared.arguments_for(&export, json)?;\n",
        "        let args = loaded\n"
        "            .prepared\n"
        "            .arguments(&[&export.interface, &export.function], json)?;\n",
    ),
    (
        "the store's `PositiveInt` states nothing",
        "server",
        DOMAIN,
        "opaque type PositiveInt = Int where value >= 1\n",
        "opaque type PositiveInt = Int\n",
    ),
]

CARGO = {
    "core": ["cargo", "test", "--quiet", "--locked", "-p", "pw-core"],
    "host": ["cargo", "test", "--quiet", "--locked", "-p", "pw-host", "--features", "engine"],
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
