#!/usr/bin/env python3
"""Mutation controls for ADR-0207: a data source states what it guarantees,
and nothing asks it for more.

Each mutant undoes one piece:
- the syntax: `source` a declaration, and its clauses' words;
- a source held to what the program's effects name, each held by one;
- a query's `consistency` held to the source's `reads`, `strong` giving
  every promise and every source giving `eventual`;
- a command's writes in one source, a resource no source holds the host's
  database's;
- a command's `transaction` held to the source's `transactions`, in the
  order of what each prevents;
- a command's events committed with its writes, or told by a feed;
- a command's record of an interaction committed with its writes, which a
  feed is not.

Every mutant must fail `data_sources.rs`.

Run from the repository root; `just e14-data-sources` records the output.
The source is restored after every mutant, whatever happens.
"""

import pathlib
import re
import subprocess
import sys

ROOT = pathlib.Path(__file__).resolve().parent.parent
CHECK = ROOT / "compiler/pw-core/src/check.rs"
POLICY = ROOT / "compiler/pw-core/src/policy.rs"
GRAMMAR = ROOT / "compiler/pw-syntax/src/grammar.rs"

# (what, file, anchor, replacement)
MUTANTS = [
    (
        "a source is no declaration",
        GRAMMAR,
        "    // `source StoreData  holds Carts, Orders  transactions serializable`.\n    \"source\",\n",
        "    // `source StoreData  holds Carts, Orders  transactions serializable`.\n",
    ),
    (
        "a source's transactions take any word",
        POLICY,
        "        \"transactions\" => Domain::Word(&[\"serializable\", \"snapshot\", \"read_committed\", \"none\"]),\n",
        "        \"transactions\" => Domain::Held,\n",
    ),
    (
        "a list of words takes any word",
        POLICY,
        "            .any(|w| !words.contains(&w))\n",
        "            .any(|_| false)\n",
    ),
    (
        "a source holds nothing, silently",
        CHECK,
        "        if source.holds.is_empty() {\n",
        "        if false && source.holds.is_empty() {\n",
    ),
    (
        "a source holds what nothing declares",
        CHECK,
        "            if !visible.contains(held) {\n",
        "            if false && !visible.contains(held) {\n",
    ),
    (
        "two sources hold one resource",
        CHECK,
        "                .find(|o| o.name != source.name && o.holds.iter().any(|(h, _)| h == held))\n",
        "                .find(|o| o.name != source.name && o.holds.is_empty())\n",
    ),
    (
        "a query asks what it likes",
        CHECK,
        "                    if reads_give(&source.reads, asked) {\n",
        "                    if true || reads_give(&source.reads, asked) {\n",
    ),
    (
        "strong gives no promise but itself",
        CHECK,
        "    asked == \"eventual\" || given.contains(\"strong\") || given.contains(asked)\n",
        "    asked == \"eventual\" || given.contains(asked)\n",
    ),
    (
        "a source gives eventual only where it says so",
        CHECK,
        "    asked == \"eventual\" || given.contains(\"strong\") || given.contains(asked)\n",
        "    given.contains(\"strong\") || given.contains(asked)\n",
    ),
    (
        "a command writes two sources",
        CHECK,
        "                if by_source.len() > 1 {\n",
        "                if by_source.len() > 2 {\n",
    ),
    (
        "a resource no source holds is in the source beside it",
        CHECK,
        "                    let name = held_by(w).map_or_else(\n"
        "                        || \"the host's database\".to_string(),\n",
        "                    let name = held_by(w).or_else(|| sources.first()).map_or_else(\n"
        "                        || \"the host's database\".to_string(),\n",
    ),
    (
        "a command asks what isolation it likes",
        CHECK,
        "                    && isolation(asked) > isolation(&source.transactions)\n",
        "                    && isolation(asked) > 3\n",
    ),
    (
        "a snapshot is no more than read committed",
        CHECK,
        "        \"snapshot\" => 2,\n",
        "        \"snapshot\" => 1,\n",
    ),
    (
        "events are sent from writes no transaction commits",
        CHECK,
        "                if decl.policy(\"emits\").is_some() && !atomic && !source.feed {\n",
        "                if false && decl.policy(\"emits\").is_some() && !atomic && !source.feed {\n",
    ),
    (
        "a feed tells no event",
        CHECK,
        "                if decl.policy(\"emits\").is_some() && !atomic && !source.feed {\n",
        "                if decl.policy(\"emits\").is_some() && !atomic {\n",
    ),
    (
        "an interaction's record commits with nothing",
        CHECK,
        "                if decl.policy(\"idempotent_by\").is_some() && !atomic {\n",
        "                if false && decl.policy(\"idempotent_by\").is_some() && !atomic {\n",
    ),
    (
        "a feed is taken for a transaction",
        CHECK,
        "                if decl.policy(\"idempotent_by\").is_some() && !atomic {\n",
        "                if decl.policy(\"idempotent_by\").is_some() && !atomic && !source.feed {\n",
    ),
]

TESTS = ["cargo", "test", "--quiet", "--locked", "-p", "pw-core", "--test", "data_sources"]


def run():
    """(built, passed, failed)."""
    r = subprocess.run(TESTS, cwd=ROOT, capture_output=True, text=True)
    out = r.stdout + r.stderr
    m = re.search(r"test result: \w+\. (\d+) passed; (\d+) failed", out)
    if m is None:
        if "error[" in out or "could not compile" in out:
            return False, 0, 0
        return True, 0, 1
    return True, int(m.group(1)), int(m.group(2))


def main():
    import mutation_baseline  # keeps what each command says, for a red baseline
    built, passed, failed = run()
    print(f"baseline: {passed} passed, {failed} failed")
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
            built, passed, failed = run()
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
