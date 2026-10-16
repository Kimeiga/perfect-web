#!/usr/bin/env python3
"""Compare all repository .pw programs using two already-built checkers.

Run while .pw files are stable. Rust mutation tests do not relink the pw
binaries, so an audit can run beside them. Coverage includes historical
sources without changing them. Each changed diagnostic is recorded.
"""

import argparse
import hashlib
import json
from pathlib import Path
import re
import shlex
import subprocess
import sys

ROOT = Path(__file__).resolve().parent.parent
ANSI = re.compile(r"\x1b\[[0-9;]*m")


def module(path):
    return next(
        (line.removeprefix("module ").strip()
         for line in path.read_text().splitlines() if line.startswith("module ")),
        None,
    )


def programs():
    standard = sorted(ROOT.glob("packages/*/*.pw"))
    library = [ROOT / "examples/domain.pw", *sorted(ROOT.glob("examples/lib/*.pw"))]
    accepted = sorted(ROOT.glob("examples/accepted/*.pw"))
    groups = {}

    def add(name, files):
        groups[name] = list(dict.fromkeys(files))

    add("packages", standard)
    add("accepted", standard + library + accepted)
    for name in ["store", "demo", "kiokun", "kiokun-site", "feed", "counter",
                 "hello-static", "koka", "render", "streamed"]:
        files = sorted(ROOT.glob(f"examples/{name}/*.pw"))
        base = standard + library if name in ["store", "demo", "streamed"] else list(standard)
        if name == "demo":
            base += sorted(ROOT.glob("examples/store/*.pw"))
        if name == "kiokun-site":
            base += [ROOT / "examples/kiokun/Shards.pw", ROOT / "examples/kiokun/dictionary.pw"]
        add(name, base + files)
    add("benchmark-pleris", standard + sorted(ROOT.glob("benchmarks/baselines/pleris/**/*.pw")))
    for pattern in ["examples/rejected/*.pw", "examples/generality/*/*.pw",
                    "examples/rules/*/*.pw", "examples/robustness/**/*.pw",
                    "examples/history/**/*.pw"]:
        for path in sorted(ROOT.glob(pattern)):
            base = standard + library + accepted
            if "/rules/" in str(path) or "/robustness/" in str(path):
                base = standard + library
            own = module(path)
            # Historical accepted modules replace their current version in
            # this program; checking both would manufacture a duplicate.
            base = [other for other in base if other == path or not own or module(other) != own]
            add(str(path.relative_to(ROOT)), base + [path])

    tracked = subprocess.check_output(
        ["git", "ls-files", "--cached", "--others", "--exclude-standard"],
        cwd=ROOT, text=True,
    ).splitlines()
    expected = {str(ROOT / path) for path in tracked if path.endswith(".pw")}
    covered = {str(path) for files in groups.values() for path in files}
    missing = sorted(expected - covered)
    if missing:
        raise RuntimeError(f"repository programs omitted from the audit: {missing}")
    return groups, len(expected)


def check(binary, files):
    result = subprocess.run(
        [str(binary), "check", *map(str, files)],
        cwd=ROOT, capture_output=True, text=True, check=False,
    )
    output = ANSI.sub("", result.stdout + result.stderr)
    diagnostics = []
    message = None
    for line in output.splitlines():
        if re.match(r"^(error|warning)(\[|:)", line):
            message = line
        elif re.match(r"^\s*--> ", line) and message:
            diagnostics.append(message + " @ " + line.strip().removeprefix("--> "))
            message = None
    if result.returncode not in [0, 1]:
        raise RuntimeError(f"{binary} failed with exit {result.returncode}: {output}")
    return {"status": result.returncode, "diagnostics": sorted(diagnostics)}


def fingerprint(binary):
    return str(binary) + " SHA256 " + hashlib.sha256(binary.read_bytes()).hexdigest()


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--baseline-pw", required=True, type=Path)
    parser.add_argument("--baseline-ref", required=True)
    parser.add_argument("--pw", type=Path, default=ROOT / "target/debug/pw")
    parser.add_argument("--output", type=Path,
                        default=ROOT / "docs/evidence/E14/generic-effects-programs.txt")
    args = parser.parse_args()
    old, new = args.baseline_pw.resolve(), args.pw.resolve()
    groups, count = programs()
    rows = []
    for index, (name, files) in enumerate(groups.items()):
        before, after = check(old, files), check(new, files)
        rows.append({
            "program": name, "before": before, "after": after,
            "added": sorted(set(after["diagnostics"]) - set(before["diagnostics"])),
            "removed": sorted(set(before["diagnostics"]) - set(after["diagnostics"])),
        })
        if index % 60 == 0:
            print(f"checked {index + 1}/{len(groups)} programs", flush=True)
    changed = [row for row in rows if row["added"] or row["removed"]]
    clean = sum(row["after"]["status"] == 0 for row in rows)
    lines = [
        "produced by: python3 scripts/generic_effects_programs.py " + shlex.join(sys.argv[1:]),
        "commit: " + subprocess.check_output(["git", "rev-parse", "HEAD"], cwd=ROOT, text=True).strip(),
        "baseline ref: " + args.baseline_ref,
        "baseline binary: " + fingerprint(old),
        "current binary: " + fingerprint(new),
        "Each program checked using both binaries, against identical current source files.",
        f"Coverage: {count} .pw files, {len(rows)} program sets, {clean} clean after, "
        f"{len(changed)} diagnostic sets changed.",
        "All tracked and new .pw files covered; original historical sources are retained.",
        "",
    ]
    for row in changed:
        lines.append(row["program"])
        lines.extend("  ADDED " + diagnostic for diagnostic in row["added"])
        lines.extend("  REMOVED " + diagnostic for diagnostic in row["removed"])
    lines.extend(["", "Every program after:"])
    for row in rows:
        lines.append(f'{row["program"]}: exit {row["after"]["status"]}, '
                     f'{len(row["after"]["diagnostics"])} diagnostics')
    args.output.parent.mkdir(parents=True, exist_ok=True)
    args.output.write_text("\n".join(lines) + "\n")
    (ROOT / "work").mkdir(exist_ok=True)
    (ROOT / "work/generic-checks.json").write_text(json.dumps({
        "files": count, "programs": len(rows), "clean_after": clean,
        "changed": changed, "all": rows,
    }, indent=2))
    print("\n".join(lines[:20]), flush=True)


if __name__ == "__main__":
    main()
