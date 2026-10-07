#!/usr/bin/env python3
"""Say whether a verification run passed (ADR-0245), from every shard's
results: each recipe's exit status, and the evidence it wrote read for a
mutant that survived. Writes a table to `$GITHUB_STEP_SUMMARY` where that is
set, and exits 1 if any recipe failed, any mutant survived, or a shard left
no results.

    python3 scripts/ci_summary.py <artifacts-dir> --shards N
"""

import argparse
import json
import os
import pathlib
import sys

# What a recipe's evidence says when a mutation control did not hold.
SURVIVED = ("SURVIVED", "ANCHOR NOT FOUND")


def main() -> int:
    parser = argparse.ArgumentParser()
    parser.add_argument("artifacts")
    parser.add_argument("--shards", type=int, required=True)
    args = parser.parse_args()
    root = pathlib.Path(args.artifacts)
    rows, failed = [], []
    shards = sorted(root.glob("evidence-*/results.json"))
    if len(shards) != args.shards:
        failed.append(f"{args.shards} shards planned, {len(shards)} reported")
    for results in shards:
        for r in json.loads(results.read_text()):
            survived = []
            for path in r["wrote"]:
                text = (results.parent / "evidence" / path).read_text(errors="replace")
                survived += [line.strip() for line in text.splitlines() if any(s in line for s in SURVIVED)]
            ok = r["status"] == 0 and not survived
            rows.append((r["recipe"], "ok" if ok else "FAILED", r["seconds"], len(r["wrote"])))
            if not ok:
                why = f"exit {r['status']}" if r["status"] else "; ".join(survived[:3])
                failed.append(f"{r['recipe']}: {why}")
    rows.sort()
    table = ["| recipe | result | seconds | files |", "|---|---|---|---|"]
    table += [f"| `{n}` | {res} | {s} | {f} |" for n, res, s, f in rows]
    head = f"**{len(rows)} recipes, {len(failed)} failed**\n\n"
    body = head + "\n".join(f"- {f}" for f in failed) + ("\n\n" if failed else "") + "\n".join(table) + "\n"
    print(body)
    summary = os.environ.get("GITHUB_STEP_SUMMARY")
    if summary:
        with open(summary, "a") as f:
            f.write(body)
    return 1 if failed else 0


if __name__ == "__main__":
    sys.exit(main())
