#!/usr/bin/env python3
"""**A mutant of a program's source is still a program** (the integrator's
finding, 2026-10-14), or the checker's refusal of it is what it undoes.

A mutation script's mutant of a `.pw` source runs the tests on the program
it leaves. Where the checker refuses that program, every test that builds
it fails, and the script reads "KILLED (13 of 13 tests fail)": a kill by
the build, whatever the tests hold. `store_hours_mutations.py`'s "an order
is placed whatever the hours" was one from the day order-total gave
`UserOrders.place` three arguments, and its mutant still called it with
one; two more were (a `&&` the language does not have, and a list of
another type). Where the checker's rule is the protection a mutant takes
away (a user's data cached `shared` is refused, PW5001), the script says so,
by that rule: `CHECKED = {"what": {"PW5001"}}` beside its `MUTANTS`.

For each script's mutant of a source of a program below, the program is
checked in a scratch copy before and after it. It passes where the mutant
adds no error, or only errors of the codes its script names for it; a
script that names codes for a mutant that adds none is told so too.

    python3 scripts/mutant_programs.py PW

where PW is a built `pw` (`target/debug/pw`). Exits 1 on any failure.
"""

import concurrent.futures
import importlib.util
import pathlib
import re
import shutil
import subprocess
import sys
import tempfile

ROOT = pathlib.Path(__file__).resolve().parent.parent

STD = ["packages/pw-std/*.pw", "packages/pw-platform-web/*.pw"]

# Each program a mutant may change, by the sources its builds check.
PROGRAMS = {
    "store": STD + ["examples/domain.pw", "examples/lib/*.pw", "examples/store/*.pw"],
    "feed": STD + ["examples/feed/*.pw"],
    "kiokun": STD + ["examples/kiokun/*.pw"],
    "kiokun-site": STD + ["examples/kiokun-site/*.pw"],
}

# Which program a source is of, by its directory: the first that holds it.
OF = [
    ("examples/feed/", "feed"),
    ("examples/kiokun-site/", "kiokun-site"),
    ("examples/kiokun/", "kiokun"),
    ("examples/store/", "store"),
    ("examples/lib/", "store"),
    ("examples/domain.pw", "store"),
    ("packages/", "store"),
]

WORKERS = 6


def program_of(rel: str) -> str | None:
    return next((p for prefix, p in OF if rel.startswith(prefix)), None)


def load(script: pathlib.Path):
    spec = importlib.util.spec_from_file_location(script.stem, script)
    module = importlib.util.module_from_spec(spec)
    sys.path.insert(0, str(script.parent))
    try:
        spec.loader.exec_module(module)
    finally:
        sys.path.pop(0)
    return module


def mutants():
    """Each mutant of a program's source: (script, what, path from the
    root, anchor, replacement, the codes its script names for it)."""
    out = []
    for script in sorted((ROOT / "scripts").glob("*_mutations.py")):
        module = load(script)
        checked = getattr(module, "CHECKED", {})
        for m in getattr(module, "MUTANTS", []):
            path = next((x for x in m if isinstance(x, pathlib.Path)), None)
            strs = [x for x in m if isinstance(x, str)]
            if path is None or path.suffix != ".pw" or len(strs) < 3:
                continue
            try:
                rel = str(path.resolve().relative_to(ROOT))
            except ValueError:
                continue
            if program_of(rel) is None:
                continue
            what = strs[0]
            out.append((script.name, what, rel, strs[-2], strs[-1], set(checked.get(what, ()))))
    return out


def errors(pw: str, root: pathlib.Path, program: str) -> list[tuple[str, str]]:
    """The program's errors, as (code, message), each once."""
    files = []
    for pattern in PROGRAMS[program]:
        files.extend(sorted(str(p.relative_to(root)) for p in root.glob(pattern)))
    out = subprocess.run([pw, "check", *files], cwd=root, capture_output=True, text=True)
    text = re.sub(r"\x1b\[[0-9;]*m", "", out.stdout + out.stderr)
    found = re.findall(r"error: \[(PW\d{4})\] (.*)", text)
    return sorted(set(found))


def judge(pw: str, root: pathlib.Path, base: dict, m) -> str | None:
    """Why the mutant `m` fails the rule, or none where it passes."""
    script, what, rel, anchor, replacement, named = m
    target = root / rel
    original = target.read_text()
    if original.count(anchor) != 1:
        return None  # `mutation_anchors.py`'s to say
    try:
        target.write_text(original.replace(anchor, replacement, 1))
        after = errors(pw, root, program_of(rel))
    finally:
        target.write_text(original)
    added = [e for e in after if e not in base[program_of(rel)]]
    codes = {code for code, _ in added}
    if not added and named:
        return f"{script}: {what}: names {sorted(named)}, and the program it leaves checks"
    if added and not codes <= named:
        shown = "; ".join(f"[{c}] {msg}" for c, msg in added[:3])
        return f"{script}: {what}: leaves a program the checker refuses: {shown}"
    return None


def main() -> int:
    if len(sys.argv) != 2:
        print(__doc__)
        return 2
    pw = str(pathlib.Path(sys.argv[1]).resolve())
    found = mutants()
    scratch = pathlib.Path(tempfile.mkdtemp(prefix="pw-mutant-programs-"))
    roots = []
    try:
        for i in range(WORKERS):
            root = scratch / str(i)
            for d in ["examples", "packages"]:
                shutil.copytree(ROOT / d, root / d)
            roots.append(root)
        base = {p: errors(pw, roots[0], p) for p in PROGRAMS}
        failures = []

        def worker(i: int) -> list[str]:
            return [
                why
                for m in found[i::WORKERS]
                if (why := judge(pw, roots[i], base, m)) is not None
            ]

        with concurrent.futures.ThreadPoolExecutor(WORKERS) as pool:
            for out in pool.map(worker, range(WORKERS)):
                failures.extend(out)
    finally:
        shutil.rmtree(scratch, ignore_errors=True)
    for f in sorted(failures):
        print(f"FAIL: {f}")
    named = sum(1 for m in found if m[5])
    print(
        f"mutant programs: {len(found)} mutants of a program's source, "
        f"{named} refused by the rule their script names, {len(failures)} failing"
    )
    return 1 if failures else 0


if __name__ == "__main__":
    sys.exit(main())
