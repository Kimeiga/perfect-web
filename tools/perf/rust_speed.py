#!/usr/bin/env python3
"""Evidence-producing Rust accelerator experiment; does not change project defaults."""
import argparse
from collections import Counter
import json
import os
from pathlib import Path
import re
import shutil
import signal
import statistics
import subprocess
import sys
import time

PKGS = ("pw-core", "pw-table-memory", "pw-host")
FLAGS = [part for name in PKGS for part in ("-p", name)]
BUILD = ["cargo", "test", "--locked", "--no-run", "--lib", "--tests", *FLAGS]
CARGO_RUN = ["cargo", "test", "--locked", "--no-fail-fast", "--lib", "--tests", *FLAGS]
NEXT_RUN = ["cargo", "nextest", "run", "--locked", "--no-fail-fast", "--lib", "--tests", *FLAGS]
TIME_RE = re.compile(r"BENCH_TIME:(\d+(?:\.\d+)?):(\d+)")
TEST_RE = re.compile(r"^(.+?): test$", re.MULTILINE)


def parse_time(s):
    m = TIME_RE.search(s)
    return (float(m[1]), int(m[2])) if m else (None, None)


def inventory(cargo_text, nextest_text):
    cargo = Counter(TEST_RE.findall(cargo_text))
    doc = json.loads(nextest_text)
    if not isinstance(doc.get("rust-suites"), dict):
        raise ValueError("nextest list lacks rust-suites")
    nxt = Counter()
    for suite in doc["rust-suites"].values():
        if suite.get("status") != "listed":
            raise ValueError("nextest returned an unlisted suite")
        for name, test in suite["testcases"].items():
            if test.get("filter-match", {}).get("status") == "matches":
                nxt[name] += 1
    return {
        "cargo_count": sum(cargo.values()), "nextest_count": sum(nxt.values()),
        "equal": bool(cargo) and cargo == nxt,
        "missing": list((cargo - nxt).elements())[:15],
        "extra": list((nxt - cargo).elements())[:15],
    }


def median(records, prefix):
    values = [r["seconds"] for r in records
              if r["name"].startswith(prefix) and r["exit_code"] == 0]
    return statistics.median(values) if values else None


class Benchmark:
    def __init__(self, mode, output):
        self.mode, self.output = mode, output
        output.mkdir(parents=True, exist_ok=True)
        self.target, self.cache = output / "target", output / "sccache"
        self.data = {
            "mode": mode, "sha": os.environ.get("GITHUB_SHA"),
            "arch": os.uname().machine, "cpus": os.cpu_count(),
            "packages": list(PKGS),
            "versions": {n: self.version(cmd) for n, cmd in {
                "rustc": ["rustc", "--version"], "cargo": ["cargo", "--version"],
                "nextest": ["cargo", "nextest", "--version"],
                "sccache": ["sccache", "--version"],
                "mold": ["mold", "--version"], "cc": ["cc", "--version"],
            }.items()},
            "runs": [], "errors": [],
        }

    @staticmethod
    def version(cmd):
        try:
            p = subprocess.run(cmd, text=True, capture_output=True, timeout=15, check=False)
            return ((p.stdout or p.stderr).strip().splitlines() or ["unavailable"])[0][:200]
        except (OSError, subprocess.TimeoutExpired):
            return "unavailable"

    def env(self, **extra):
        env = os.environ.copy()
        for k in ("RUSTFLAGS", "RUSTC_WRAPPER", "RUSTC_WORKSPACE_WRAPPER"):
            env.pop(k, None)
        env.update({"CARGO_TARGET_DIR": str(self.target), "CARGO_TERM_COLOR": "never"})
        env.update(extra)
        return env

    def save(self):
        (self.output / "results.json").write_text(json.dumps(self.data, indent=2) + "\n")

    def clear(self):
        shutil.rmtree(self.target, ignore_errors=True)

    def measure(self, name, cmd, env=None, timeout=1100):
        log = self.output / (name + ".log")
        times = self.output / (name + ".time")
        wrapped = ["/usr/bin/time", "-f", "BENCH_TIME:%e:%M", "-o", str(times), *cmd]
        started, code = time.monotonic(), 127
        print(f"RUN {name}: {' '.join(cmd)}", flush=True)
        with log.open("w") as f:
            try:
                child = subprocess.Popen(wrapped, stdout=f, stderr=subprocess.STDOUT,
                                         env=env or self.env(), start_new_session=True)
                try:
                    code = child.wait(timeout=timeout)
                except subprocess.TimeoutExpired:
                    code = 124
                    os.killpg(child.pid, signal.SIGTERM)
                    try:
                        child.wait(timeout=10)
                    except subprocess.TimeoutExpired:
                        os.killpg(child.pid, signal.SIGKILL)
                        child.wait()
            except OSError as err:
                f.write(str(err) + "\n")
        elapsed, rss = parse_time(times.read_text()) if times.exists() else (None, None)
        row = {
            "name": name, "command": cmd, "exit_code": code,
            "seconds": round(time.monotonic() - started, 3),
            "gnu_time_seconds": elapsed, "peak_rss_kib": rss,
            "disk_free_gib": round(shutil.disk_usage(self.output).free / 2**30, 2),
            "log": log.name,
        }
        self.data["runs"].append(row)
        self.save()
        print(f"{name}: rc={code} time={row['seconds']}s peak_rss={rss}KiB", flush=True)
        if code != 0:
            tail = "\n".join(log.read_text(errors="replace").splitlines()[-20:])
            raise RuntimeError(f"{name} failed (exit {code})\n{tail[-1400:]}")
        return row

    def test_runner(self):
        env = self.env()
        self.measure("prebuild", BUILD, env)
        self.measure("cargo_inventory", ["cargo", "test", "--locked", "--lib",
                                          "--tests", *FLAGS, "--", "--list"], env)
        args = ["cargo", "nextest", "list", "--locked", "--lib", "--tests",
                *FLAGS, "--message-format", "json"]
        p = subprocess.run(args, capture_output=True, text=True, env=env,
                           timeout=240, check=False)
        (self.output / "nextest_list.json").write_text(p.stdout)
        (self.output / "nextest_list.stderr").write_text(p.stderr)
        if p.returncode:
            raise RuntimeError("nextest list failed: " + p.stderr[-1200:])
        comparison = inventory((self.output / "cargo_inventory.log").read_text(), p.stdout)
        self.data["inventory"] = comparison
        self.save()
        if not comparison["equal"]:
            raise RuntimeError("test inventory mismatch: " + str(comparison))
        # Nextest excludes doctests; preserve these with Cargo in all adoption plans.
        self.measure("cargo_doctests", ["cargo", "test", "--locked", "--doc", *FLAGS], env)
        for i, order in ((1, ("cargo", "nextest")), (2, ("nextest", "cargo"))):
            for tool in order:
                self.measure(f"{tool}_run_{i}", CARGO_RUN if tool == "cargo" else NEXT_RUN, env)

    def linker(self):
        variants = {"default": self.env(),
                    "mold": self.env(RUSTFLAGS="-C link-arg=-fuse-ld=mold")}
        for i, order in ((1, ("default", "mold")), (2, ("mold", "default"))):
            for name in order:
                self.clear()
                self.measure(f"{name}_cold_{i}", BUILD, variants[name])
                self.measure(f"{name}_noop_{i}", BUILD, variants[name])
        self.clear()

    def sccache(self):
        plain = self.env()
        cached = self.env(RUSTC_WRAPPER=shutil.which("sccache") or "sccache",
                          SCCACHE_DIR=str(self.cache), SCCACHE_CACHE_SIZE="3G")
        self.clear()
        self.measure("plain_cold_1", BUILD, plain)
        self.measure("plain_noop_1", BUILD, plain)
        self.clear()
        shutil.rmtree(self.cache, ignore_errors=True)
        self.measure("cache_reset_cold", ["sccache", "--zero-stats"], cached)
        self.measure("cache_cold", BUILD, cached)
        self.measure("cache_cold_stats", ["sccache", "--show-stats", "--stats-format", "json"], cached)
        self.clear()
        self.measure("cache_reset_warm", ["sccache", "--zero-stats"], cached)
        self.measure("cache_warm_rebuild", BUILD, cached)
        self.measure("cache_warm_stats", ["sccache", "--show-stats", "--stats-format", "json"], cached)
        self.clear()
        self.measure("plain_cold_2", BUILD, plain)
        self.measure("plain_noop_2", BUILD, plain)
        self.clear()

    def summary(self):
        rows = [
            f"## Rust accelerator trial: {self.mode}, {self.data['arch']}",
            f"Commit: {self.data['sha']}; Rust: {self.data['versions']['rustc']}",
            "| Trial | Exit code | Wall seconds | Peak command RSS MiB |",
            "|---|---:|---:|---:|",
        ]
        for r in self.data["runs"]:
            mem = f"{r['peak_rss_kib'] / 1024:.1f}" if r["peak_rss_kib"] is not None else "n/a"
            rows.append(f"| {r['name']} | {r['exit_code']} | {r['seconds']:.3f} | {mem} |")
        if self.mode == "test-runner":
            a, b = median(self.data["runs"], "cargo_run_"), median(self.data["runs"], "nextest_run_")
            rows.append(f"Non-doctest runtime median: Cargo {a}s; Nextest {b}s; "
                        f"ratio {round(a/b,2) if a is not None and b else 'n/a'}x.")
            rows.append("Inventory: " + str(self.data.get("inventory")))
        elif self.mode == "linker":
            a, b = median(self.data["runs"], "default_cold_"), median(self.data["runs"], "mold_cold_")
            rows.append(f"Clean-build median: default {a}s; mold {b}s; "
                        f"ratio {round(a/b,2) if a is not None and b else 'n/a'}x.")
        else:
            a, b = median(self.data["runs"], "plain_cold_"), median(self.data["runs"], "cache_warm_rebuild")
            rows.append(f"Fresh-Cargo-target median: without wrapper {a}s; warm sccache {b}s; "
                        f"ratio {round(a/b,2) if a is not None and b else 'n/a'}x.")
        rows.append("Cold means cleared Cargo target, not cleared OS/download cache.")
        rows.append("Peak RSS is GNU time command memory, not process-tree total.")
        rows.append("Tool installation cost is excluded; failures are NOT benchmark successes.")
        rows.extend(["ERROR: " + e[:300] for e in self.data["errors"]])
        return "\n\n".join(rows) + "\n"

    def execute(self):
        failed = False
        try:
            if self.mode == "test-runner":
                self.test_runner()
            elif self.mode == "linker":
                self.linker()
            else:
                self.sccache()
        except Exception as e:
            failed = True
            self.data["errors"].append(str(e)[:2000])
            print("BENCHMARK FAILURE:", e, file=sys.stderr)
        finally:
            # Preserve logs and metrics, never upload huge compiled binaries or
            # the local sccache object store as GitHub Actions artifacts.
            self.clear()
            shutil.rmtree(self.cache, ignore_errors=True)
            self.save()
            summary = self.summary()
            (self.output / "summary.md").write_text(summary)
            if os.environ.get("GITHUB_STEP_SUMMARY"):
                with Path(os.environ["GITHUB_STEP_SUMMARY"]).open("a") as f:
                    f.write(summary)
            print(summary, flush=True)
        return 1 if failed else 0


if __name__ == "__main__":
    parser = argparse.ArgumentParser()
    parser.add_argument("--mode", choices=["test-runner", "linker", "sccache"], required=True)
    parser.add_argument("--output", type=Path, required=True)
    args = parser.parse_args()
    raise SystemExit(Benchmark(args.mode, args.output).execute())
