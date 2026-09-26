#!/usr/bin/env python3
"""Mutation controls for ADR-0088: a clause names a declaration of its kind,
and gives it its key.

Each mutant undoes one piece: lowering a clause's keys, making their
arguments roots, relating a key to what it names, looking its name up in the
clause's namespace and kind, giving an event a signature, the graph's kind
rule, and reading a clause where the source writes it. The tests in
`clause_keys.rs` must then fail.

Run from the repository root; `just e10-clause-keys` records the output. The
source is restored after every mutant, whatever happens.
"""

import pathlib
import re
import subprocess
import sys

ROOT = pathlib.Path(__file__).resolve().parent.parent
LOWER = ROOT / "compiler/pw-core/src/lower.rs"
VALUES = ROOT / "compiler/pw-core/src/values.rs"
GRAPH = ROOT / "compiler/pw-core/src/graph.rs"
SIGNATURES = ROOT / "compiler/pw-core/src/signatures.rs"
POLICY = ROOT / "compiler/pw-core/src/policy.rs"

# (what is undone, file, anchor, replacement)
MUTANTS = [
    (
        "a clause's keys are not lowered",
        LOWER,
        "            if crate::policy::keyed(&p.name).is_some() {",
        "            if crate::policy::keyed(&p.name).is_some() && false {",
    ),
    (
        "a key's arguments are not roots",
        LOWER,
        "            .flat_map(|k| &k.args)\n",
        "            .flat_map(|k| &k.args)\n            .filter(|_| false)\n",
    ),
    (
        "a key is not related to what it names",
        VALUES,
        "                if let Some(sig) = self.keyed(&policy.name, key) {",
        "                if let Some(sig) = self.keyed(&policy.name, key).filter(|_| false) {",
    ),
    (
        "a key's name is looked up as a term",
        VALUES,
        "            false => self.ws.resolve_in(self.at, ns, &key.name),",
        "            false => self.ws.resolve_in(self.at, crate::resolve::Namespace::Term, &key.name),",
    ),
    (
        "a key of another kind is related",
        VALUES,
        "        if !self.sigs.kind_of(def).is_some_and(|k| kinds.contains(&k)) {",
        "        if false && !self.sigs.kind_of(def).is_some_and(|k| kinds.contains(&k)) {",
    ),
    (
        "an `emits` clause names no kind of declaration",
        POLICY,
        "        Domain::EventRef | Domain::Listener => Some((Namespace::Event, &[K::Event])),",
        "        Domain::EventRef | Domain::Listener => Some((Namespace::Event, &[])),",
    ),
    (
        "an event has no signature",
        SIGNATURES,
        "                        | DeclKind::Event\n",
        "",
    ),
    (
        "a resource fits any clause",
        GRAPH,
        "                    NodeKind::Resource { .. } if wants_resource => continue,",
        "                    NodeKind::Resource { .. } => continue,",
    ),
    (
        "an event fits any clause",
        GRAPH,
        "                    NodeKind::Event if !wants_resource => continue,",
        "                    NodeKind::Event => continue,",
    ),
    (
        "a clause is read from its collapsed value",
        LOWER,
        "            value.to_string(),\n            p.span.start + head + (rest.len() - value.len()),",
        "            p.value.clone(),\n            p.span.start + head + (rest.len() - value.len()),",
    ),
]

TESTS = [
    ["cargo", "test", "--quiet", "--locked", "-p", "pw-core", "--test", "clause_keys"],
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
