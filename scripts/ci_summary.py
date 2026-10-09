#!/usr/bin/env python3
"""Say whether a verification run passed (ADR-0245), from every shard's
results: each recipe's exit status, and the evidence it wrote read for a
mutant that survived. Writes a table to `$GITHUB_STEP_SUMMARY` where that is
set, and exits 1 if any recipe failed, any mutant survived, or a shard left
no results.

It lists apart each recipe whose mutation script's memory bound stopped a
process (`mutation_bound.py`): a mutant whose run that was is killed, but
perhaps by the bound alone, not by a test. The list fails nothing.

    python3 scripts/ci_summary.py <artifacts-dir> --shards N
"""

import argparse
import json
import os
import pathlib
import sys

# What a recipe's evidence says when a mutation control did not hold.
SURVIVED = ("SURVIVED", "ANCHOR NOT FOUND")

# A mutation script's last line, where its memory bound stopped a process.
BOUND = "memory bound: "
UNTOUCHED = "memory bound: no process was stopped"


def shards_of(root: pathlib.Path) -> list[pathlib.Path]:
    """Each shard's directory: `evidence-N` under the root, or the root
    itself where the run had one shard. `download-artifact` extracts a lone
    match into the path it is given, not into a directory of its name, so a
    run of one shard reported none until 2026-10-09 (W6's 1b, run
    37889326640)."""
    found = sorted(p.parent for p in root.glob("evidence-*/results.json"))
    if not found and (root / "results.json").exists():
        found = [root]
    return found


def main() -> int:
    parser = argparse.ArgumentParser()
    parser.add_argument("artifacts")
    parser.add_argument("--shards", type=int, required=True)
    args = parser.parse_args()
    root = pathlib.Path(args.artifacts)
    rows, failed, bound = [], [], []
    shards = shards_of(root)
    if len(shards) != args.shards:
        failed.append(f"{args.shards} shards planned, {len(shards)} reported")
    for shard in shards:
        for r in json.loads((shard / "results.json").read_text()):
            survived = []
            for path in r["wrote"]:
                text = (shard / "evidence" / path).read_text(errors="replace")
                survived += [line.strip() for line in text.splitlines() if any(s in line for s in SURVIVED)]
                bound += [
                    f"{r['recipe']}: {line[len(BOUND):]}"
                    for line in text.splitlines()
                    if line.startswith(BOUND) and line != UNTOUCHED
                ]
            ok = r["status"] == 0 and not survived
            rows.append((r["recipe"], "ok" if ok else "FAILED", r["seconds"], len(r["wrote"])))
            if not ok:
                why = f"exit {r['status']}" if r["status"] else "; ".join(survived[:3])
                failed.append(f"{r['recipe']}: {why}")
    rows.sort()
    table = ["| recipe | result | seconds | files |", "|---|---|---|---|"]
    table += [f"| `{n}` | {res} | {s} | {f} |" for n, res, s, f in rows]
    head = f"**{len(rows)} recipes, {len(failed)} failed**\n\n"
    body = head + "\n".join(f"- {f}" for f in failed) + ("\n\n" if failed else "")
    if bound:
        body += "Killed, perhaps by the memory bound alone, not by a test:\n\n"
        body += "\n".join(f"- {b}" for b in bound) + "\n\n"
    body += "\n".join(table) + "\n"
    print(body)
    summary = os.environ.get("GITHUB_STEP_SUMMARY")
    if summary:
        with open(summary, "a") as f:
            f.write(body)
    return 1 if failed else 0


if __name__ == "__main__":
    sys.exit(main())
