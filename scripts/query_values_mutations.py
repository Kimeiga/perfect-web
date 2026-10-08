#!/usr/bin/env python3
"""Mutation controls for ADR-0147: a query binding is the query's value, and
a page that cannot be read is answered.

Each mutant undoes one part:
- the typer: a page's query binding typed as the `Ok` value of its declared
  result, and loops and arms typed together until nothing more is learned;
- the server: a page whose queries fail answered rather than panicked, a
  served one told to read itself again, and a session's order given as its
  status's case;
- `pw-render --plan`: a block a private query decides rendered as nothing
  for no session.

A typer mutant must fail `query_values.rs`, a server mutant the server's
tests, and a renderer mutant `plan_lists.rs`.

Run from the repository root; `just e14-query-values` records the output.
The source is restored after every mutant, whatever happens.
"""

import pathlib
import re
import subprocess
import sys

ROOT = pathlib.Path(__file__).resolve().parent.parent
INFER = ROOT / "compiler/pw-core/src/infer.rs"
SERVER = ROOT / "spikes/own-renderer/server/src/main.rs"
STORE_DATA = ROOT / "spikes/own-renderer/server/src/store.rs"
BIN = ROOT / "runtime/pw-render/src/bin/pw-render.rs"

# (what, which tests, file, anchor, replacement)
MUTANTS = [
    (
        "a query binding has no type",
        "typer",
        INFER,
        "                bindings.entry(Binder::Pattern(*pat)).or_insert(v);\n",
        "                let _ = v;\n",
    ),
    (
        "a query binding is its `Result`",
        "typer",
        INFER,
        "                Some(Builtin::Result) => result.args().first().cloned(),\n",
        "                Some(Builtin::Result) => Some(result.clone()),\n",
    ),
    (
        "loops and arms are typed once each",
        "typer",
        INFER,
        "            if types.bindings.len() == learned {\n",
        "            if true || types.bindings.len() == learned {\n",
    ),
    (
        "a page whose queries fail stops the server",
        "server",
        SERVER,
        # Re-anchored by ADR-0163: a failure keeps its kind. And by ADR-0190:
        # it names its page.
        "            .map_err(|e| e.of(&format!(\"`{}`'s queries\", template.name)))?;\n",
        "            .unwrap_or_else(|e| panic!(\"`{}`'s queries: {e}\", template.name));\n",
    ),
    (
        "a served page whose values fail is not told",
        "server",
        SERVER,
        # Re-anchored by ADR-0161: a document that cannot be shown.
        "        if self.shown.lock().expect(\"shown\").remove(doc).is_some()\n",
        "        if self.shown.lock().expect(\"shown\").remove(doc).is_some() && false\n",
    ),
    (
        "the order interface answers no order",
        "server",
        STORE_DATA,
        "                let status = order.clone().map(|case| Box::new(Val::Variant(case, None)));\n",
        "                let status = order.clone().map(|case| Box::new(Val::Variant(case, None))).filter(|_| false);\n",
    ),
    (
        "a no-session block is rendered",
        "render",
        BIN,
        "                env = env.materialized(pw_render::PartId(*id), \"\");\n",
        "                let _ = id;\n",
    ),
]

CARGO = {
    "typer": ["cargo", "test", "--quiet", "--locked", "-p", "pw-core", "--test", "query_values"],
    "server": ["cargo", "test", "--quiet", "-p", "pw-dev-server"],
    "render": ["cargo", "test", "--quiet", "--locked", "-p", "pw-render", "--test", "plan_lists"],
}


def run(kind):
    """(built, passed, failed) over one crate's tests."""
    r = subprocess.run(CARGO[kind], cwd=ROOT, capture_output=True, text=True)
    results = re.findall(r"test result: \w+\. (\d+) passed; (\d+) failed", r.stdout + r.stderr)
    if not results:
        return False, 0, 0
    return True, sum(int(p) for p, _ in results), sum(int(f) for _, f in results)


def main():
    import mutation_baseline  # keeps what each command says, for a red baseline
    for kind in CARGO:
        built, passed, failed = run(kind)
        print(f"baseline ({kind}): {passed} passed, {failed} failed")
        if not built or failed or not passed:
            print("FAIL: the unmutated baseline is not green; no mutant can mean anything")
            mutation_baseline.explain()
            return 1

    survivors = 0
    for what, kind, path, anchor, replacement in MUTANTS:
        original = path.read_text()
        if original.count(anchor) != 1:
            print(f"{what}: ANCHOR NOT FOUND EXACTLY ONCE in {path.name}")
            survivors += 1
            continue
        try:
            path.write_text(original.replace(anchor, replacement, 1))
            built, passed, failed = run(kind)
        finally:
            path.write_text(original)
        if not built:
            verdict = "KILLED (does not build)"
        elif failed:
            verdict = f"KILLED ({failed} of {passed + failed} tests fail)"
        else:
            verdict = "SURVIVED"
            survivors += 1
        print(f"{what} [{kind}]: {verdict}")

    print(f"{len(MUTANTS) - survivors} of {len(MUTANTS)} mutants killed")
    return 1 if survivors else 0


if __name__ == "__main__":
    sys.exit(main())
