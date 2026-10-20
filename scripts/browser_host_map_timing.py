#!/usr/bin/env python3
"""What the browser host map saves, measured (ADR-0356): the same Playwright
selection run through two trees, one before the host map and one after it,
in turn, each run timed from its start to its exit.

    python3 scripts/browser_host_map_timing.py --before DIR --after DIR \\
        [--runs N] [--port-before P] [--port-after P] -- PLAYWRIGHT_ARGUMENTS...

Each tree is a checkout whose `spikes/own-renderer` holds the same builds
(`dist`, and any of `dist-keyed`, `dist-feed`, `dist-kiokun`,
`dist-guestbook`) and whose `target/debug` holds the same development
server: this script builds nothing, and compares nothing but the
configurations. The runs alternate, before then after, so both see the
machine's load as it moves; the load average is recorded beside each run.
Each run's hosts are counted by Playwright's own account of them
(`DEBUG=pw:webserver`: one "Starting WebServer process" a host), and its
tests by its line reporter. A deployment's kiokun data is not given to
either (as CI gives none).
"""

import argparse
import datetime
import os
import pathlib
import re
import signal
import subprocess
import sys
import time


def load():
    try:
        return pathlib.Path("/proc/loadavg").read_text().split()[0]
    except OSError:
        return "?"


def run(tree, port, args, bound):
    env = {**os.environ, "PORT": str(port), "DEBUG": "pw:webserver"}
    for key in ("KIOKUN_DATA", "KIOKUN_APP", "PW_PERFORMANCE", "PW_REUSE", "CI"):
        env.pop(key, None)
    before = load()
    spike = pathlib.Path(tree) / "spikes/own-renderer"
    # Playwright's own CLI, not through `pnpm exec`: a run stopped at its
    # bound is told so itself, and stops the hosts it started (stopped
    # through pnpm, its hosts outlived it).
    cli = spike / "node_modules/@playwright/test/cli.js"
    start = time.monotonic()
    began = datetime.datetime.now(datetime.timezone.utc)
    p = subprocess.Popen(["node", str(cli), "test", *args, "--reporter=line"], cwd=spike, env=env,
                         stdout=subprocess.PIPE, stderr=subprocess.STDOUT, text=True)
    try:
        text, _ = p.communicate(timeout=bound)
        status = p.returncode
    except subprocess.TimeoutExpired:
        p.send_signal(signal.SIGINT)
        try:
            text, _ = p.communicate(timeout=120)
        except subprocess.TimeoutExpired:
            p.kill()
            text, _ = p.communicate()
        status = None
    seconds = time.monotonic() - start
    text = re.sub(r"\x1b\[[0-9;?]*[A-Za-z]", "", text)
    counts = {kind: sum(int(n) for n in re.findall(rf"(\d+) {kind}\b", text)) for kind in ("passed", "failed", "flaky", "skipped")}
    hosts = text.count("Starting WebServer process")
    # Where the time went, by Playwright's own timestamps: to its first host
    # (the configuration loaded and, after the host map, the plan made), and
    # from it until the last host was ready; the rest is the tests'.
    stamp = lambda line: datetime.datetime.fromisoformat(line.split(" ", 1)[0].replace("Z", "+00:00"))
    starts = [stamp(line) for line in text.splitlines() if "pw:webserver Starting WebServer process" in line]
    ready = [stamp(line) for line in text.splitlines() if "pw:webserver WebServer available" in line]
    first = (starts[0] - began).total_seconds() if starts else None
    span = (ready[-1] - starts[0]).total_seconds() if starts and ready else None
    text = f"started: {began.isoformat()}\n" + text
    return {"status": status, "seconds": seconds, "hosts": hosts, "load": f"{before} -> {load()}", "text": text,
            "first": first, "span": span, **counts}


def main():
    parser = argparse.ArgumentParser()
    parser.add_argument("--before", required=True)
    parser.add_argument("--after", required=True)
    parser.add_argument("--runs", type=int, default=1)
    parser.add_argument("--port-before", type=int, default=int(os.environ.get("PORT", "3141")))
    parser.add_argument("--port-after", type=int, default=int(os.environ.get("PORT", "3141")))
    parser.add_argument("--bound", type=int, default=7200, help="seconds one run may take")
    parser.add_argument("--keep", help="a directory to keep each run's whole output in")
    parser.add_argument("args", nargs=argparse.REMAINDER)
    options = parser.parse_args()
    args = options.args[1:] if options.args[:1] == ["--"] else options.args
    print(f"selection: playwright test {' '.join(args)} --reporter=line")
    print(f"before: {options.before} (PORT {options.port_before}); after: {options.after} (PORT {options.port_after})")
    print(f"machine: {os.cpu_count()} threads; node {subprocess.run(['node', '--version'], capture_output=True, text=True).stdout.strip()}")
    results = {"before": [], "after": []}
    for i in range(options.runs):
        for side, tree, port in (("before", options.before, options.port_before), ("after", options.after, options.port_after)):
            r = run(tree, port, args, options.bound)
            results[side].append(r)
            if options.keep:
                pathlib.Path(options.keep).mkdir(parents=True, exist_ok=True)
                (pathlib.Path(options.keep) / f"run-{i + 1}-{side}.txt").write_text(r["text"])
            phases = (f"first host at {r['first']:.1f} s, all ready {r['span']:.1f} s later"
                      if r["first"] is not None and r["span"] is not None else "no host became ready")
            print(f"run {i + 1} {side:6}: {r['seconds']:8.1f} s  hosts {r['hosts']:3}  passed {r['passed']:4}  failed {r['failed']}  "
                  f"flaky {r['flaky']}  skipped {r['skipped']}  exit {r['status']}  load {r['load']}  ({phases})", flush=True)
    failed = False
    for side in ("before", "after"):
        rs = results[side]
        if any(r["status"] != 0 for r in rs):
            failed = True
        print(f"{side}: median {sorted(r['seconds'] for r in rs)[len(rs) // 2]:.1f} s of {len(rs)} run(s), hosts {sorted({r['hosts'] for r in rs})}")
    b = sorted(r["seconds"] for r in results["before"])[len(results["before"]) // 2]
    a = sorted(r["seconds"] for r in results["after"])[len(results["after"]) // 2]
    print(f"saved: {b - a:.1f} s a run ({(b - a) / b * 100:.0f}% of before's median)")
    return 1 if failed else 0


if __name__ == "__main__":
    sys.exit(main())
