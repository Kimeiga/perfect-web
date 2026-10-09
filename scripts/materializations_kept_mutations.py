#!/usr/bin/env python3
"""Mutation controls for ADR-0277: a materialization is kept, and a page
reads it.

Each mutant undoes one piece:
- the compiler's: a read in a body lowered, answering its resource's `Ok`;
  a materialization that derives its value lowered at all; a read naming
  what it reads; a page reading a public one, and refused a private one;
- the host's: a read linked with no grant;
- the materializer's: a chain's order;
- the server's: a page's read of one answered from its entry; an event
  reaching a kept entry; the chain made in its order; an entry out of date
  before it is made again; the documents told, each that reads the entry at
  its key and no other; a query read through its kept answer; a body's
  answers remembered; a fault holding; the last good value served; and a
  value read back as it was kept.

The tests of each must then fail.

Run from the repository root; `just e14-materializations-kept` records the
output. The source is restored after every mutant, whatever happens.
"""

import os
import pathlib
import re
import signal
import subprocess
import sys

ROOT = pathlib.Path(__file__).resolve().parent.parent
LOWER = ROOT / "compiler/pw-core/src/backend/lower.rs"
CONTRACT = ROOT / "compiler/pw-core/src/contract.rs"
CHECK = ROOT / "compiler/pw-core/src/check.rs"
HOST = ROOT / "runtime/pw-host/src/lib.rs"
GRAPH = ROOT / "runtime/pw-materialize/src/graph.rs"
SERVER = ROOT / "spikes/own-renderer/server/src/main.rs"
CODEC = ROOT / "spikes/own-renderer/server/src/materializations.rs"

COMPILER = "compiler"
RUNTIME = "runtime"
SERVED = "server"

# (what is undone, which tests, file, anchor, replacement)
MUTANTS = [
    (
        "a read in a body is no read",
        COMPILER,
        LOWER,
        "            } if self.reads && keyword == \"query\" => self.query_read(body, modifiers, args, span),\n",
        "            } if false && self.reads && keyword == \"query\" => {\n"
        "                self.query_read(body, modifiers, args, span)\n"
        "            }\n",
    ),
    (
        "a read answers its resource's `Result`",
        COMPILER,
        LOWER,
        "        let ty = match self.ty(returns, &span) {\n"
        "            Lowering::Lowered(Type::Result(ok, _)) => *ok,\n",
        "        let ty = match self.ty(returns, &span) {\n",
    ),
    (
        "a materialization that derives its value is not lowered",
        COMPILER,
        LOWER,
        "            let derives = decl.kind == DeclKind::Materialize && decl.ret.is_some();\n"
        "            if !member && !derives",
        "            let derives = false && decl.ret.is_some();\n"
        "            if !member && !derives",
    ),
    (
        "a read names nothing it reads",
        COMPILER,
        CONTRACT,
        "                reads: Some(signature.path.clone()),\n",
        "                reads: None,\n",
    ),
    (
        "a page may not read a public one",
        COMPILER,
        CHECK,
        "                if derives && (deriving || (page && public)) {\n",
        "                if derives && deriving {\n",
    ),
    (
        "a page may read a private one",
        COMPILER,
        CHECK,
        "                if derives && (deriving || (page && public)) {\n",
        "                if derives && (deriving || page) {\n",
    ),
    (
        "a read is linked only with a grant",
        SERVED,
        HOST,
        "                || (i.reads.is_some() && i.capability.is_empty())\n",
        "                || false\n",
    ),
    (
        "a chain's order is its paths'",
        RUNTIME,
        GRAPH,
        "                .find(|(p, among)| !out.contains(p) && among.iter().all(|r| out.contains(r)));\n",
        "                .find(|(p, _)| !out.contains(p));\n",
    ),
    (
        "a page's read of one is a query's",
        SERVED,
        SERVER,
        "        if self.derives(resource) {\n"
        "            return self.materialized(resource, args).map_err(Unread::Failed);\n",
        "        if false && self.derives(resource) {\n"
        "            return self.materialized(resource, args).map_err(Unread::Failed);\n",
    ),
    (
        "an event reaches no kept entry",
        SERVED,
        SERVER,
        "                    self.graph.reaches(path, event, &values, key)\n",
        "                    false && self.graph.reaches(path, event, &values, key)\n",
    ),
    (
        "a chain is made against its order",
        SERVED,
        SERVER,
        "        for path in self.graph.in_dependency_order(&paths) {\n",
        "        for path in self.graph.in_dependency_order(&paths).into_iter().rev() {\n",
    ),
    (
        "an entry is made again while it is current",
        SERVED,
        SERVER,
        "            self.materializer.invalidate(&Self::kept_key(path, args), 0);\n",
        "            let _ = (path, args);\n",
    ),
    (
        "each document of a reader's session is told",
        SERVED,
        SERVER,
        "                |doc| documents.contains(&doc.1),\n",
        "                |_| !documents.is_empty(),\n",
    ),
    (
        "an entry made again with its value is told",
        SERVED,
        SERVER,
        "                if self.materializer.entry(&key).map(|e| e.body) != was {\n",
        "                if true || self.materializer.entry(&key).map(|e| e.body) != was {\n",
    ),
    (
        "a document is told whatever key it reads",
        SERVED,
        SERVER,
        "                            .is_ok_and(|given| given == *args)\n",
        "                            .is_ok_and(|given| given.len() == args.len())\n",
    ),
    (
        "what a menu change made is told to no one",
        SERVED,
        SERVER,
        "        drop(queue);\n"
        "        let made = std::mem::take(&mut *self.made.lock().expect(\"made\"));\n"
        "        self.tell_kept(&made);\n",
        "        drop(queue);\n"
        "        std::mem::take(&mut *self.made.lock().expect(\"made\"));\n",
    ),
    (
        "a query is read past its kept answer",
        SERVED,
        SERVER,
        "        let answer = match policy {\n",
        "        let answer = match policy.filter(|_| false) {\n",
    ),
    (
        "a body's answers are not remembered",
        SERVED,
        SERVER,
        "                        .insert(materializations::asked(&import, &given)?, value);\n",
        "                        .insert(String::new(), value);\n",
    ),
    (
        "a fault does not hold",
        SERVED,
        SERVER,
        "                .contains(path)\n"
        "            {\n"
        "                return Err(\"the materializer failed\".to_string());\n",
        "                .contains(\"\")\n"
        "            {\n"
        "                return Err(\"the materializer failed\".to_string());\n",
    ),
    (
        "what could not be made is served no last good value",
        SERVED,
        SERVER,
        "            pw_materialize::Read::Fresh(body) | pw_materialize::Read::LastKnownGood(body) => {\n"
        "                materializations::decode_text(&body)\n",
        "            pw_materialize::Read::Fresh(body) => {\n"
        "                materializations::decode_text(&body)\n",
    ),
    (
        "a number is read back narrower",
        SERVED,
        CODEC,
        "        Val::S64(n) => json!({ \"s64\": n }),\n",
        "        Val::S64(n) => json!({ \"s32\": n }),\n",
    ),
]

TESTS = {
    COMPILER: [
        ["cargo", "test", "--quiet", "--locked", "-p", "pw-core",
         "--test", "materializations_kept", "--test", "materialization_bodies", "--test", "build"],
    ],
    RUNTIME: [
        ["cargo", "test", "--quiet", "--locked", "-p", "pw-materialize", "--test", "chain_order"],
        ["cargo", "test", "--quiet", "--locked", "-p", "pw-dev-server", "--",
         "materializations", "a_menu_change_drops_that_stores_kept_menu_only"],
    ],
    SERVED: [
        ["cargo", "test", "--quiet", "--locked", "-p", "pw-dev-server", "--",
         "materializations", "a_menu_change_drops_that_stores_kept_menu_only",
         "a_change_to_one_stores_menu_reaches_that_stores_pages_only",
         "an_insert_and_what_changed_with_it_are_derived_from_the_menu"],
    ],
}

# How long one command may run. Past it, it and what it started are stopped.
BOUND = 1800


def bounded(cmd):
    """(exit status or None past the bound, output) of one command."""
    p = subprocess.Popen(
        cmd, cwd=ROOT, stdout=subprocess.PIPE, stderr=subprocess.STDOUT, text=True,
        start_new_session=True,
    )
    try:
        out, _ = p.communicate(timeout=BOUND)
        return p.returncode, out
    except subprocess.TimeoutExpired:
        os.killpg(p.pid, signal.SIGKILL)
        out, _ = p.communicate()
        return None, out


def run_tests(group):
    """(built, passed, failed) over the group's commands."""
    built, passed, failed = True, 0, 0
    for cmd in TESTS[group]:
        _, out = bounded(cmd)
        results = re.findall(r"test result: \w+\. (\d+) passed; (\d+) failed", out)
        if not results:
            built = False
            continue
        for p, f in results:
            passed += int(p)
            failed += int(f)
    return built, passed, failed


def main():
    import mutation_baseline  # keeps what each command says, for a red baseline
    # Stopped from outside, the source is still restored: a stop is an
    # exception here, which the `finally` below meets.
    signal.signal(signal.SIGTERM, lambda *_: sys.exit(143))
    for group in TESTS:
        built, passed, failed = run_tests(group)
        print(f"baseline ({group}): {passed} passed, {failed} failed", flush=True)
        if not built or failed or not passed:
            print("FAIL: the unmutated baseline is not green; no mutant can mean anything")
            mutation_baseline.explain()
            return 1

    survivors = 0
    for what, group, path, anchor, replacement in MUTANTS:
        original = path.read_text()
        if original.count(anchor) != 1:
            print(f"{what}: ANCHOR NOT FOUND EXACTLY ONCE in {path.name}")
            survivors += 1
            continue
        try:
            path.write_text(original.replace(anchor, replacement, 1))
            built, passed, failed = run_tests(group)
        finally:
            path.write_text(original)
        if not built:
            verdict = "KILLED (does not build)"
        elif failed:
            verdict = f"KILLED ({failed} of {passed + failed} tests fail)"
        else:
            verdict = "SURVIVED"
            survivors += 1
        print(f"{what} [{group}]: {verdict}", flush=True)

    print(f"{len(MUTANTS) - survivors} of {len(MUTANTS)} mutants killed")
    return 1 if survivors else 0


if __name__ == "__main__":
    sys.exit(main())
