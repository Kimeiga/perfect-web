#!/usr/bin/env python3
"""Measure the native default, explicit LLD and mold on identical Rust builds."""
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

VARIANTS = {
    "default": "",
    "lld": "-C link-arg=-fuse-ld=lld",
    "mold": "-C link-arg=-fuse-ld=mold",
}
ROUNDS = (("default", "lld", "mold"),
          ("mold", "default", "lld"),
          ("lld", "mold", "default"))
BUILD = ["cargo", "test", "--locked", "--no-run", "--lib", "--tests",
         "-p", "pw-core", "-p", "pw-table-memory", "-p", "pw-host"]
TIME = re.compile(r"BENCH_TIME:(\d+(?:\.\d+)?):(\d+)")


def medians(rows):
    result = {}
    for name in VARIANTS:
        values = [r["seconds"] for r in rows if r["kind"] == "build"
                  and r["variant"] == name and r["exit_code"] == 0]
        result[name] = statistics.median(values) if len(values) == 3 else None
    return result


class Benchmark:
    def __init__(self, out):
        self.out, self.target = out, out / "target"
        out.mkdir(parents=True, exist_ok=True)
        self.rows, self.errors, self.versions = [], [], []

    def env(self, variant):
        env = os.environ.copy()
        # This experiment preserves x64 CI's default dev/test debug settings.
        # The earlier link experiments set both to zero, unlike x64 CI.
        for k in ("RUSTFLAGS", "RUSTC_WRAPPER", "RUSTC_WORKSPACE_WRAPPER",
                  "CARGO_PROFILE_DEV_DEBUG", "CARGO_PROFILE_TEST_DEBUG"):
            env.pop(k, None)
        env.update({"CARGO_TARGET_DIR": str(self.target), "CARGO_INCREMENTAL": "0",
                    "CARGO_TERM_COLOR": "never"})
        if VARIANTS[variant]:
            env["RUSTFLAGS"] = VARIANTS[variant]
        return env

    def save(self):
        data = {"sha": os.getenv("GITHUB_SHA"), "arch": os.uname().machine,
                "cpus": os.cpu_count(), "variants": VARIANTS, "rounds": ROUNDS,
                "versions": self.versions, "rows": self.rows, "errors": self.errors,
                "medians": medians(self.rows)}
        (self.out / "results.json").write_text(json.dumps(data, indent=2) + "\n")

    def run(self, name, variant, cmd, kind="probe", timeout=950):
        log = self.out / (name + ".log")
        timer = self.out / (name + ".time")
        command = ["/usr/bin/time", "-o", str(timer), "-f", "BENCH_TIME:%e:%M", *cmd]
        started, rc = time.monotonic(), 127
        print("RUN", name, " ".join(cmd), flush=True)
        with log.open("w") as handle:
            try:
                process = subprocess.Popen(command, stdout=handle,
                    stderr=subprocess.STDOUT, env=self.env(variant),
                    start_new_session=True)
                try:
                    rc = process.wait(timeout=timeout)
                except subprocess.TimeoutExpired:
                    rc = 124
                    os.killpg(process.pid, signal.SIGTERM)
                    try:
                        process.wait(timeout=10)
                    except subprocess.TimeoutExpired:
                        os.killpg(process.pid, signal.SIGKILL)
                        process.wait()
            except OSError as exc:
                handle.write(str(exc) + "\n")
        matched = TIME.search(timer.read_text()) if timer.exists() else None
        row = {"name": name, "variant": variant, "kind": kind,
               "seconds": round(time.monotonic() - started, 3), "exit_code": rc,
               "peak_rss_kib": int(matched[2]) if matched else None,
               "log": log.name}
        self.rows.append(row)
        self.save()
        print("DONE", name, row, flush=True)
        if rc != 0:
            tail = "\n".join(log.read_text(errors="replace").splitlines()[-16:])
            raise RuntimeError(f"{name}: exit {rc}\n{tail[-1200:]}")
        return log.read_text(errors="replace")

    def probes(self):
        for name, cmd in (("rustc", ["rustc", "--version", "--verbose"]),
                          ("cc", ["cc", "--version"]), ("ld", ["ld", "--version"]),
                          ("lld", ["ld.lld", "--version"]),
                          ("mold", ["mold", "--version"])):
            self.versions.append({"name": name,
                                  "output": self.run("version-"+name, "default", cmd)[:1200]})
        source = self.out / "probe.rs"
        source.write_text("fn main() { println!(\"linker probe\"); }\n")
        for name in VARIANTS:
            cmd = ["rustc", str(source), "-o", str(self.out / ("probe-"+name)),
                   "-C", "link-arg=-Wl,-v"]
            if name != "default":
                cmd += ["-C", VARIANTS[name].removeprefix("-C ")]
            result = self.run("probe-"+name, name, cmd)
            self.versions.append({"name": "actual-"+name, "output": result[-3000:]})
        self.save()

    def main(self):
        self.probes()
        for idx, round_variants in enumerate(ROUNDS, 1):
            for name in round_variants:
                shutil.rmtree(self.target, ignore_errors=True)
                self.run(f"{name}-build-{idx}", name, BUILD, kind="build")
                self.run(f"{name}-noop-{idx}", name, BUILD, kind="noop")
                if idx == 1:
                    self.run(f"{name}-smoke", name,
                             ["cargo", "test", "--locked", "-p",
                              "pw-table-memory", "--test", "memory"], kind="smoke")
        shutil.rmtree(self.target, ignore_errors=True)

    def summary(self):
        med = medians(self.rows)
        lines = ["## Rust linkers on " + os.uname().machine,
                 "Commit: "+str(os.getenv("GITHUB_SHA")),
                 "| variant | clean builds (seconds) | median |",
                 "|---|---|---:|"]
        for name in VARIANTS:
            trials = [r for r in self.rows if r["kind"] == "build"
                      and r["variant"] == name]
            times = ", ".join(f"{r['seconds']} (rc {r['exit_code']})" for r in trials)
            lines.append(f"| {name} | {times} | {med[name]} |")
        for other in ("lld", "mold"):
            if med["default"] and med[other]:
                lines.append(f"{other} vs default: "
                             f"{100*(1-med[other]/med['default']):.1f}% shorter build "
                             "(negative = slower).")
        if med["mold"] and med["lld"]:
            lines.append(f"mold vs LLD: {100*(1-med['mold']/med['lld']):.1f}% "
                         "shorter build (negative = slower).")
        lines += ["Three successful clean builds per variant are required.",
                  "Same Rust version, same GCC driver; only -fuse-ld changes.",
                  "Cold Cargo target, OS caches not cleared. Timings include compilation.",
                  "Peak RSS is GNU time command RSS, not process tree peak.",
                  "Tests and version probes must succeed; no active CI changes."]
        lines += ["Failure: " + s[:500] for s in self.errors]
        return "\n\n".join(lines) + "\n"

    def execute(self):
        error = False
        try:
            self.main()
        except Exception as exc:
            error = True
            self.errors.append(str(exc)[:1800])
            print("LINKER EXPERIMENT FAILED:", exc, file=sys.stderr)
        finally:
            shutil.rmtree(self.target, ignore_errors=True)
            self.save()
            summary = self.summary()
            (self.out / "summary.md").write_text(summary)
            if os.getenv("GITHUB_STEP_SUMMARY"):
                with Path(os.environ["GITHUB_STEP_SUMMARY"]).open("a") as f:
                    f.write(summary)
            print(summary, flush=True)
        return int(error)


if __name__ == "__main__":
    if len(sys.argv) != 2:
        sys.exit("usage: linker_choice.py OUTPUT")
    sys.exit(Benchmark(Path(sys.argv[1])).execute())
