#!/usr/bin/env python3
"""**The development server's suite, whole: its peak memory and its time**
(ADR-0317).

A whole-suite mutation baseline runs this binary, four tests at once on CI's
runner, under ADR-0292's bound: on 2026-10-12 it held more than 3.9 GiB and
was stopped. This builds the suite's binary as `cargo test` does, runs it
whole with `--test-threads=N`, and says how much it held at its peak and how
long it took, against the bound a script's processes are held to.

    python3 scripts/suite_memory.py [--threads 4]

The peak is what ADR-0292's bound reads of a process: on Linux its peak
resident set (`getrusage` of the child, which swap does not add to on CI's
runner), on macOS its peak physical footprint, which counts what the kernel
compressed, as `/usr/bin/time -l` reports it.
"""

import argparse
import json
import pathlib
import re
import resource
import subprocess
import sys
import time

ROOT = pathlib.Path(__file__).resolve().parent.parent


def binary():
    """The development server's test binary, built as `cargo test` builds it."""
    out = subprocess.run(
        ["cargo", "test", "--locked", "-p", "pw-dev-server", "--no-run",
         "--message-format=json"],
        cwd=ROOT, capture_output=True, text=True, check=True,
    ).stdout
    for line in out.splitlines():
        try:
            m = json.loads(line)
        except ValueError:
            continue
        if (m.get("reason") == "compiler-artifact"
                and m.get("target", {}).get("name") == "pw-dev-server"
                and m.get("profile", {}).get("test")
                and m.get("executable")):
            return m["executable"]
    sys.exit("no test binary for pw-dev-server")


def main():
    parser = argparse.ArgumentParser()
    parser.add_argument("--threads", type=int, default=4)
    args = parser.parse_args()
    exe = binary()
    command = [exe, f"--test-threads={args.threads}"]
    if sys.platform == "darwin":
        command = ["/usr/bin/time", "-l", *command]
    started = time.monotonic()
    run = subprocess.run(command, cwd=ROOT, capture_output=True, text=True)
    seconds = time.monotonic() - started
    if sys.platform == "darwin":
        found = re.search(r"(\d+)\s+peak memory footprint", run.stderr)
        gib = int(found.group(1)) / (1 << 30) if found else float("inf")
    else:
        gib = resource.getrusage(resource.RUSAGE_CHILDREN).ru_maxrss / (1 << 20)
    results = [l for l in run.stdout.splitlines() if l.startswith("test result:")]
    print(f"threads: {args.threads}")
    print(f"result: {results[-1] if results else 'none'} (exit {run.returncode})")
    print(f"wall: {seconds:.0f} s")
    print(f"peak resident: {gib:.2f} GiB")
    sys.path.insert(0, str(ROOT / "scripts"))
    import mutation_bound  # the bound a mutation script's processes are held to

    bound = mutation_bound.BOUND / (1 << 30)
    total = mutation_bound.total_memory() / (1 << 30)
    print(f"bound (ADR-0292): {bound:.2f} GiB, of this machine's {total:.1f} GiB")
    print("within the bound" if gib < bound else "OVER THE BOUND")
    return 0 if run.returncode == 0 and gib < bound else 1


if __name__ == "__main__":
    sys.exit(main())
