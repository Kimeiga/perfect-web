#!/usr/bin/env python3
"""Mutation controls for ADR-0323: a member read is computed wherever a page
reads it.

Each mutant undoes one piece:
- the plan: a member read inside a block left to the renderer, which reads
  by field; one computed as a part at the top of the page; the member taken
  for a field; one in an attribute, a block's subject, the title or the
  metadata computed by nothing; and a loop's list read through a member
  computed too, where a host keeps a list it reads by field;
- the speculation: a member read inside a block the browser renders again,
  in text or in an attribute, computed by nothing there;
- the template: a member read inside a view that contains itself built,
  which nothing computes.

Each must fail `compiler/pw-core/tests/member_reads_in_blocks.rs`.

Run from the repository root; `just e14-member-reads` records the output.
The source is restored after every mutant, whatever happens.
"""

import os
import pathlib
import re
import signal
import subprocess
import sys

ROOT = pathlib.Path(__file__).resolve().parent.parent
PLAN = ROOT / "compiler/pw-core/src/page_values.rs"
SPECULATION = ROOT / "compiler/pw-core/src/backend/speculation.rs"
TEMPLATE = ROOT / "compiler/pw-core/src/template_ir.rs"

# (what, file, anchor, replacement)
MUTANTS = [
    (
        "a member read inside a block is the renderer's, read by field",
        PLAN,
        "        if hole.nested && !through_a_member {\n",
        "        if hole.nested {\n",
    ),
    (
        "a member read inside a block is a part at the top of the page",
        PLAN,
        "        if hole.nested {\n"
        "            derived_values.push(part);\n"
        "        } else {\n"
        "            parts.push(part);\n"
        "        }\n",
        "        parts.push(part);\n",
    ),
    (
        "a member is taken for a field",
        PLAN,
        "                through_a_member = true;\n",
        "",
    ),
    (
        "a member read outside text is computed by nothing",
        PLAN,
        "        if read.kind != crate::template_ir::ReadKind::List {\n",
        "        if false {\n",
    ),
    (
        "a loop's list read through a member is computed too",
        PLAN,
        "        if read.kind != crate::template_ir::ReadKind::List {\n",
        "        if true {\n",
    ),
    (
        "a speculated block's member read in text is computed by nothing",
        SPECULATION,
        "            None if h.nested && in_a_region(h.part.0) => {\n",
        "            None if false => {\n",
    ),
    (
        "a speculated block's member read in an attribute is computed by nothing",
        SPECULATION,
        "            None if matches!(\n",
        "            None if false && matches!(\n",
    ),
    (
        "a member read inside a view that contains itself builds",
        TEMPLATE,
        "        if let Some(read) = member_read(ctx, def, view) {\n",
        "        if let Some(read) = member_read(ctx, def, view).filter(|_| false) {\n",
    ),
]

CARGO = [
    "cargo", "test", "--quiet", "--locked", "-p", "pw-core", "--test", "member_reads_in_blocks",
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
    """(built, passed, failed) over the tests."""
    out, code = bounded(CARGO, cwd=ROOT)
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
