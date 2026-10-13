#!/usr/bin/env python3
"""Mutation controls for ADR-XXXX: an arm's binding is read through a
member, as a block's value is (the integrator's, 2026-10-13; W9's finding).

Each mutant undoes one piece: the arm's read not planned in text, or in an
attribute or a block's subject; `some`'s payload typed as nothing, `err`'s
as `ok`'s, a case of several fields' as its first; the payload step left
out; two arms reading one path from different values taken, or the same
read twice refused; an arm whose match decides by no query's value taken;
the speculated region's refusal undone; and the host's payload of another
case's value given, a case of several's field taken as its first.

Each must fail `compiler/pw-core/tests/arm_member_reads.rs` or the server's
`payload_tests`.

Run from the repository root; `just e14-arm-member-reads` records the
output. The source is restored after every mutant, whatever happens.
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
SERVER = ROOT / "spikes/own-renderer/server/src/main.rs"

# (what, file, anchor, replacement)
MUTANTS = [
    (
        "an arm's read through a member is not planned in text",
        PLAN,
        "                arm_part(&mut derived_values, read)?;\n",
        "                let _ = read;\n",
    ),
    (
        "an arm's read is not planned in an attribute or a block's subject",
        PLAN,
        "            crate::template_ir::ReadKind::List => None,\n            _ => arm_read(\n",
        "            _ => None,\n            crate::template_ir::ReadKind::Title => arm_read(\n",
    ),
    (
        "`some`'s payload is typed as nothing",
        PLAN,
        '        (Some(Builtin::Option), "some", None) | (Some(Builtin::Result), "ok", None) => {\n',
        '        (Some(Builtin::Option), "none", None) | (Some(Builtin::Result), "ok", None) => {\n',
    ),
    (
        "`err`'s payload is typed as `ok`'s",
        PLAN,
        '        (Some(Builtin::Result), "err", None) => return ty.args().get(1).cloned(),\n',
        '        (Some(Builtin::Result), "err", None) => return ty.args().first().cloned(),\n',
    ),
    (
        "a case of several fields' payload is typed as its first",
        PLAN,
        "        (Some(i), _) if fields.len() > 1 => fields.get(i)?.resolved().cloned(),\n",
        "        (Some(_), _) if fields.len() > 1 => fields.first()?.resolved().cloned(),\n",
    ),
    (
        "the payload step is left out of the plan",
        PLAN,
        "    planned.push((\n        Step::Payload {\n            case: arm.case.clone(),\n"
        "            field: arm.field,\n        },\n        None,\n    ));\n",
        "",
    ),
    (
        "two arms reading one path from different values are taken",
        PLAN,
        "        Some(p) => Err(format!(\n",
        "        Some(_) if true => Ok(()),\n        Some(p) => Err(format!(\n",
    ),
    (
        "the same read twice is refused",
        PLAN,
        "        Some(p) if p.binding == read.binding && p.steps == read.steps => Ok(()),\n",
        "",
    ),
    (
        "an arm whose match decides by no query's value is not refused by name",
        PLAN,
        "    let Some((_, resource, _)) = found.iter().find(|(n, ..)| *n == query) else {\n"
        "        return Err(format!(\n",
        "    let Some((_, resource, _)) = found.iter().find(|(n, ..)| *n == query) else {\n"
        "        return Ok(None);\n        return Err(format!(\n",
    ),
    (
        "an arm's member read in a speculated region is taken",
        SPECULATION,
        "            if let Some((part, path, ..)) = reads_one {\n",
        "            if let Some((part, path, ..)) = reads_one.filter(|_| false) {\n",
    ),
    (
        "the host gives a payload of another case's value",
        SERVER,
        "        (Val::Option(_), _) => None,\n",
        "        (Val::Option(v), _) => v.as_deref().cloned(),\n",
    ),
    (
        "the host takes a case of several fields' first",
        SERVER,
        "            fields\n                .get(i)\n",
        "            fields\n                .get(i.min(0))\n",
    ),
]

TESTS = [
    ["cargo", "test", "--quiet", "--locked", "-p", "pw-core", "--test", "arm_member_reads"],
    ["cargo", "test", "--quiet", "--locked", "-p", "pw-dev-server", "--", "payload_tests"],
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
    """(built, passed, failed) over every test command."""
    built, passed, failed = True, 0, 0
    for cmd in TESTS:
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
