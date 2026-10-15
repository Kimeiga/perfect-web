#!/usr/bin/env python3
"""Mutation controls for the bug museum (ADR-XXXX, the claim measured).

An exhibit's "refused" is evidence only if the museum says otherwise when
the refusal is gone. Each mutant undoes the rule one exhibit's refusal
names, rebuilds `pw`, and runs that exhibit's `pw check` halves
(`benchmarks/museum/museum.mjs --checks-only`): the museum must then report
the exhibit not refused, and exit 1. The source is restored after every
mutant, whatever happens, and `pw` rebuilt as the source says.

The bug's half has its control in each exhibit: its repair must fail its
spec. The runtime mechanisms an exhibit's runtime test relies on have their
own scripts (`idempotent_commands_mutations.py`, `navigate_mutations.py`).

Run from the repository root; `just e14-museum` records the output.
"""

import pathlib
import subprocess
import sys

ROOT = pathlib.Path(__file__).resolve().parent.parent
CHECK = ROOT / "compiler/pw-core/src/check.rs"

# (exhibit, what is undone, file, anchor, replacement)
MUTANTS = [
    (
        "J08",
        "PW5001: a shared cache is not asked what it holds",
        CHECK,
        '        declared_cache(hir, decl).filter(|(v, _)| v == "shared" && !held.safe_in_shared_cache())\n',
        '        declared_cache(hir, decl).filter(|(v, _)| v == "shared" && !held.safe_in_shared_cache() && false)\n',
    ),
    (
        "F01",
        "PW5010: an unsafe effect reached with no audit record is not refused",
        CHECK,
        "    if !has_audit_record(body) {\n",
        "    if !has_audit_record(body) && false {\n",
    ),
    (
        "G01",
        "PW0338: a command a page sends is not asked for idempotent_by",
        CHECK,
        '        if decl.kind != hir::DeclKind::Command || decl.policy("idempotent_by").is_some() {\n',
        '        if decl.kind != hir::DeclKind::Command || decl.policy("idempotent_by").is_some() || true {\n',
    ),
]


def build():
    """Build `pw` from the source as it stands; True when it built."""
    r = subprocess.run(
        ["cargo", "build", "--quiet", "--locked", "-p", "pw-cli"],
        cwd=ROOT,
        capture_output=True,
        text=True,
    )
    return r.returncode == 0


def museum(exhibits):
    """(exit code, output) of the museum's `pw check` halves for these exhibits."""
    r = subprocess.run(
        ["node", "benchmarks/museum/museum.mjs", "--checks-only", "--exhibit", exhibits],
        cwd=ROOT,
        capture_output=True,
        text=True,
    )
    return r.returncode, r.stdout + r.stderr


def summary(out):
    """The museum's last line, or what it said last when it said none."""
    lines = [l for l in out.splitlines() if l.startswith("museum:")]
    if lines:
        return lines[-1]
    rest = out.strip().splitlines()
    return rest[-1] if rest else "(no output)"


def main():
    import mutation_baseline  # keeps what each command says, for a red baseline

    if not build():
        print("FAIL: pw does not build unmutated")
        mutation_baseline.explain()
        return 1
    code, out = museum(",".join(sorted({m[0] for m in MUTANTS})))
    print(f"baseline: {summary(out)}")
    if code != 0:
        print("FAIL: the unmutated museum is not as its exhibits expect; no mutant can mean anything")
        print(out)
        return 1

    survivors = 0
    for exhibit, what, path, anchor, replacement in MUTANTS:
        original = path.read_text()
        if original.count(anchor) != 1:
            print(f"{exhibit}: {what}: ANCHOR NOT FOUND EXACTLY ONCE in {path.name}")
            survivors += 1
            continue
        try:
            path.write_text(original.replace(anchor, replacement, 1))
            built = build()
            code, out = museum(exhibit) if built else (None, "")
        finally:
            path.write_text(original)
        if not built:
            verdict = "KILLED (does not build)"
        elif code != 0 and "NOT refused" in out:
            verdict = f"KILLED (the museum reports it not refused: {summary(out)})"
        else:
            verdict = "SURVIVED"
            survivors += 1
        print(f"{exhibit}: {what}: {verdict}")

    if not build():
        print("FAIL: pw does not build after the mutants were restored")
        return 1
    print(f"{len(MUTANTS) - survivors} of {len(MUTANTS)} mutants killed")
    return 1 if survivors else 0


if __name__ == "__main__":
    sys.exit(main())
