#!/usr/bin/env python3
"""Fail-closed comparison of complete worker-count benchmark JSON records."""
import argparse
import json
import math
from pathlib import Path
import sys

WORKERS = {2,4,6}


def inspect(records):
    if {v["workers"] for v in records} != WORKERS or len(records) != len(WORKERS):
        raise ValueError("missing, duplicate or extra worker-count trial")
    sha = {v["sha"] for v in records}
    fingerprints = {json.dumps(v["summary"], sort_keys=True) for v in records}
    if len(sha) != 1 or None in sha or len(fingerprints) != 1:
        raise ValueError("mutation coverage, head or provenance differs across workers")
    for record in records:
        t = record.get("wall_seconds")
        if (record.get("check_exit") != 0 or
            record["summary"]["failures"] != 0 or
            not isinstance(t,(int,float)) or not math.isfinite(t) or t <= 0):
            raise ValueError("failed/invalid worker trial cannot establish a speedup")
    return sorted(records, key=lambda x:x["wall_seconds"])


def main():
    parser = argparse.ArgumentParser()
    parser.add_argument("directory", type=Path)
    a = parser.parse_args()
    try:
        paths = sorted(a.directory.glob("**/results.json"))
        ranked = inspect([json.loads(p.read_text()) for p in paths])
        for item in ranked:
            print(f"workers={item['workers']}, wall={item['wall_seconds']}s, "
                  f"checks={item['summary']['checks']}")
        quickest, old = ranked[0], next(x for x in ranked if x["workers"] == 6)
        delta = 100*(1-quickest["wall_seconds"]/old["wall_seconds"])
        print(f"Best observed workers: {quickest['workers']} "
              f"({delta:.1f}% shorter than current six workers)")
        if __import__("os").environ.get("GITHUB_STEP_SUMMARY"):
            with Path(__import__("os").environ["GITHUB_STEP_SUMMARY"]).open("a") as f:
                f.write("## Full mutation-census worker comparison\n\n")
                for item in ranked:
                    f.write(f"- {item['workers']} workers: {item['wall_seconds']}s\n")
                f.write("\nAll worker counts judged the same complete source mutants. "
                        "A single run is directional, not guaranteed optimal.\n")
        return 0
    except (OSError,ValueError,KeyError,TypeError) as exc:
        print("Invalid mutation experiment: "+str(exc),file=sys.stderr)
        return 2


if __name__ == "__main__":
    raise SystemExit(main())
