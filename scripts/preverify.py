#!/usr/bin/env python3
"""**A train is pre-verified here, before it is pushed** (ADR-0364).

GitHub's `verify` (ADR-0245) runs the recipes a push touches, in shards, on
4-vCPU runners, and train-10 lost two days to failures it found one at a
time, hours apart. This runs the same recipes on this machine, in parallel
lanes, so that a failure shows up in minutes.

    python3 scripts/preverify.py [BASE] [--lanes N] [--recipes "a b"] [--plan]
    just preverify [BASE] [--lanes N] [--recipes "a b"]

**A pre-check, not evidence.** It never writes evidence into the tree it is
run in and never pushes. Each lane's evidence files and logs go under
`/tmp/preverify-<head>/`; CI's `verify` stays the evidence of record
(ADR-0281).

- **What runs is CI's plan.** `BASE` (default `origin/master`) is a ref, and
  the plan is what `verify.yml` plans for a track's push at HEAD:
  `scripts/ci_plan.py --shards 9 --changed $(git merge-base BASE HEAD) HEAD
  --current`, and `--stale` on a branch `track/train-*`. `--recipes "a b"`
  runs those instead (the workflow's dispatch).
- **Lanes.** The tree must be clean: a lane is a detached worktree of HEAD,
  and an uncommitted change would not be in it. Each lane has its own
  `CARGO_TARGET_DIR` (its worktree's `target/`), `TMPDIR`, `PORT` window
  (the host map's stride apart, inside the range for lanes), and
  `CARGO_BUILD_JOBS` of the machine's threads over the lanes. The environment
  is a runner's: CI's cargo settings, no `KIOKUN_DATA`, a database only for a
  recipe that `ci_plan.NEEDS_DATABASE` names.
- **A shared queue.** The recipes are dealt longest first by the seconds the
  planner deals by (`ci_seconds.json`, or its estimate), each to the lane
  that is free. A lane sets up as a shard does (the toolchain, the node
  modules, and `verify.yml`'s Build step once a recipe needs the build) and
  runs a recipe with `scripts/ci_recipes.py`, CI's own runner.
- **The report is CI's** (`ci_summary.py`): a recipe fails on a non-zero exit
  or a mutant its evidence records as survived, and the report adds its
  lane, CI's seconds for it, the tail of its log and its failing test lines.
  The exit status is 1 where any recipe failed or did not run, 2 where the
  run could not start. The lanes' worktrees are removed at the end; logs and
  evidence stay.
"""

import argparse
import contextlib
import dataclasses
import fcntl
import json
import os
import pathlib
import queue
import re
import shlex
import shutil
import signal
import socket
import subprocess
import sys
import threading
import time
from collections.abc import Callable

HERE = pathlib.Path(__file__).resolve().parent
ROOT = HERE.parent
sys.path.insert(0, str(HERE))

import ci_plan  # noqa: E402
import ci_recipes  # noqa: E402
import ci_summary  # noqa: E402
import mutation_slots  # noqa: E402

# **The ports lanes may use** on this machine (an assignment of the PC's
# workers, 2026-10-19): each lane takes the host map's stride, from the first.
FIRST_PORT = 12741
LAST_PORT = 14740

# **A runner's cargo settings** (verify.yml's top-level `env`), which the
# caller's environment may not share; a test holds this to the workflow.
CI_ENV = {
    "CARGO_TERM_COLOR": "never",
    "CARGO_INCREMENTAL": "0",
    "CARGO_PROFILE_DEV_DEBUG": "0",
    "CARGO_PROFILE_TEST_DEBUG": "0",
}

# **What the caller's environment has that a runner's does not**, taken out
# of a lane's: a deployment's kiokun data (a runner's browser hosts read the
# repository's sample), the databases (given, to a recipe that needs one,
# below), another run's slots and ports, and what a lane sets for itself.
NOT_A_RUNNERS = (
    "KIOKUN_DATA",
    "KIOKUN_APP",
    "PW_FEED_DATABASE_URL",
    "PW_STORE_DATABASE_URL",
    "PW_MUTANT_SLOTS",
    "PW_MUTANT_SLOT_DIR",
    "PW_MUTANT_PICK",
    "PW_MUTANT_SLOT",
    "PW_HOST_TEST_PORT",
    "PW_PERFORMANCE",
    "PW_REUSE",
    "BENCH_REUSE",
    "BENCH_STACK",
    "PORT",
    "PW_MESSAGES_PORT",
    "PW_NOTIFICATIONS_PORT",
    "BENCH_PORT",
    "CARGO_TARGET_DIR",
    "CARGO_BUILD_JOBS",
    "TMPDIR",
)

# **The ports of a lane's other families**: the messages and notifications
# mutation scripts run Playwright on `PW_MESSAGES_PORT` (7141 where unset) and
# `PW_NOTIFICATIONS_PORT` (6100), not on `PORT`, and the benchmark harness on
# `BENCH_PORT` (3200). Each is at the host map's offsets from a base like
# `PORT`, and a lane runs one command at a time, so each is the lane's `PORT`.
PORT_FAMILIES = ("PORT", "PW_MESSAGES_PORT", "PW_NOTIFICATIONS_PORT", "BENCH_PORT")

# The environment variable that marks every process a lane started, to find
# the ones that outlive their recipe.
MARKER = "PW_PREVERIFY_LANE"

# **What a shard does before its recipes** (verify.yml's `recipes` job): the
# bootstrap's Node workspace (the toolchain is copied from this tree, and
# nothing is downloaded), and the Build step, which a test holds to the
# workflow's. Browsers are installed on this machine, not by a run.
BOOTSTRAP_STEPS = ["pnpm install --frozen-lockfile --offline"]
BUILD_STEPS = [
    "cargo build --workspace --locked",
    "BUILD_ONLY=1 bash spikes/own-renderer/run.sh",
    "bash spikes/own-renderer/keyed-store.sh",
    "bash spikes/own-renderer/feed.sh",
]

# How a track's push is planned (verify.yml's `plan` job): in this many shards
# (which decides nothing but the deal), and `--stale` on these branches.
PLAN_SHARDS = 9
STALE_BRANCHES = "track/train-"

# **How often a running recipe's evidence is looked at**, in seconds, and how
# many of its lines a run quotes before it says there are more. A mutation
# script prints each mutant's verdict as it has it, so a survivor is in the
# evidence within minutes of the run's start and the recipe, which fails at its
# end, ends hours later: this is what makes a failure show up in minutes.
GLANCE_EVERY = 15.0
GLANCE_LINES = 5

# Lines of a log that name a failure, for the report: cargo's and libtest's,
# Playwright's (a failure's listing, a list reporter's cross, its counts),
# unittest's, a mutation script's, and the compiler's.
FAILING = re.compile(
    r"^test .+ \.\.\. FAILED$"
    r"|^---- .+ stdout ----$"
    r"|panicked at "
    r"|^test result: FAILED\."
    r"|^ +\d+\) \[[\w-]+\] › "
    r"|^ +✘ "
    r"|^ +[1-9]\d* (failed|flaky|interrupted|did not run)\b"
    r"|^(FAIL|ERROR): "
    r"|: (SURVIVED|ANCHOR NOT FOUND EXACTLY ONCE in .*)$"
    r"|^baseline \(.*\): \d+ passed, [1-9]\d* failed"
    r"|^error(\[E\d+\])?: "
    r"|^\s*(AssertionError|Error): "
)
ANSI = re.compile(r"\x1b\[[0-9;]*[A-Za-z]")


class Refusal(Exception):
    """The run cannot start: said, and exit status 2."""


class SetupFailed(Exception):
    """A lane's setup failed: the lane stops, and the recipes it would have
    run are left for the others."""


# ---------------------------------------------------------------------------
# Lanes: ports, jobs, environment
# ---------------------------------------------------------------------------


def lane_ports(
    lanes: int,
    root: pathlib.Path = ROOT,
    first: int = FIRST_PORT,
    last: int = LAST_PORT,
) -> list[int]:
    """Each lane's `PORT`: the host registry's stride apart from `first`
    (the allocator's own convention, `PORT + k x stride`), so that no two
    lanes' ranges overlap, and every one within `first`..`last`."""
    if lanes < 1:
        raise Refusal("give a whole number of lanes, 1 or more")
    try:
        stride, _ = mutation_slots.port_layout(root)
        ports = [mutation_slots.port_for_slot(k, {"PORT": str(first)}, root) for k in range(lanes)]
    except ValueError as error:
        raise Refusal(str(error)) from error
    if ports[-1] + stride - 1 > last:
        room = (last - first + 1) // stride
        raise Refusal(
            f"{lanes} lanes need {lanes * stride} ports ({stride} each, the host registry's stride), "
            f"and {first}-{last} holds {last - first + 1}: room for {room} lanes"
        )
    return ports


def jobs_per_lane(lanes: int, cpus: int) -> int:
    """`CARGO_BUILD_JOBS` of a lane: the machine's threads over the lanes."""
    return max(1, cpus // lanes)


def lane_environment(
    base: dict[str, str],
    port: int,
    tree: pathlib.Path,
    tmp: pathlib.Path,
    jobs: int,
    marker: str,
    database: str | None = None,
) -> dict[str, str]:
    """The environment a lane runs a recipe in: the caller's, less what a
    runner's does not have, with CI's cargo settings, the lane's own ports,
    target, temporary directory and jobs, and a database only where the
    recipe needs one."""
    env = {k: v for k, v in base.items() if k not in NOT_A_RUNNERS}
    env.update(CI_ENV)
    for name in PORT_FAMILIES:
        env[name] = str(port)
    env["CARGO_TARGET_DIR"] = str(tree / "target")
    env["CARGO_BUILD_JOBS"] = str(jobs)
    env["TMPDIR"] = str(tmp)
    env[MARKER] = marker
    if database:
        env["PW_FEED_DATABASE_URL"] = database
    return env


@contextlib.contextmanager
def port_lock(first: int, last: int):
    """Hold the range of ports for this run: two runs on one machine would
    start their hosts on the same ports. The lock is a file's, the machine's,
    and goes when the run does, however it ends."""
    path = pathlib.Path("/tmp") / f"preverify-ports-{first}-{last}.lock"
    with open(path, "a+") as handle:
        try:
            fcntl.flock(handle, fcntl.LOCK_EX | fcntl.LOCK_NB)
        except OSError:
            handle.seek(0)
            owner = handle.read().strip()
            raise Refusal(
                f"another preverify run{f' (pid {owner})' if owner else ''} holds the ports {first}-{last}: "
                "wait for it, or give this run others with --ports"
            ) from None
        handle.seek(0)
        handle.truncate()
        handle.write(f"{os.getpid()}\n")
        handle.flush()
        yield


def busy_ports(port: int, root: pathlib.Path = ROOT) -> list[int]:
    """The ports of a lane's range that something listens on already, among
    the host registry's reserved ones: a host started there would not bind."""
    _, highest = mutation_slots.port_layout(root)
    busy = []
    for n in range(port, port + highest + 1):
        with socket.socket(socket.AF_INET, socket.SOCK_STREAM) as s:
            s.setsockopt(socket.SOL_SOCKET, socket.SO_REUSEADDR, 1)
            try:
                s.bind(("127.0.0.1", n))
            except OSError:
                busy.append(n)
    return busy


# ---------------------------------------------------------------------------
# The plan
# ---------------------------------------------------------------------------


def git(root: pathlib.Path, *args: str, check: bool = True) -> subprocess.CompletedProcess:
    return subprocess.run(["git", *args], cwd=root, capture_output=True, text=True, check=check)


def dirty_paths(root: pathlib.Path) -> list[str]:
    """What the tree holds that HEAD does not: each changed or untracked
    path, as `git status --porcelain` says it. A lane is HEAD."""
    out = git(root, "status", "--porcelain", "--untracked-files=normal").stdout
    return [line for line in out.splitlines() if line.strip()]


def plan_command(
    base: str | None, head: str, stale: bool, current: bool, recipes: list[str] | None
) -> list[str]:
    """`ci_plan.py` as verify.yml runs it: for a track's push, `--changed
    <merge base> HEAD --current` (and `--stale` on a train's); for a
    dispatch, the recipes it names."""
    command = [sys.executable, "scripts/ci_plan.py", "--shards", str(PLAN_SHARDS)]
    if recipes:
        return command + recipes
    command += ["--changed", str(base), head]
    if current:
        command.append("--current")
    if stale:
        command.append("--stale")
    return command


def parse_plan(stdout: str) -> list[str]:
    """The recipes in a plan's `shards=<json>` line, once each, in the order
    the shards deal them."""
    line = next((l for l in stdout.splitlines() if l.startswith("shards=")), None)
    if line is None:
        raise Refusal("scripts/ci_plan.py printed no plan (no `shards=` line)")
    names: list[str] = []
    for shard in json.loads(line[len("shards="):]):
        for recipe in shard["recipes"]:
            if recipe not in names:
                names.append(recipe)
    return names


def planned(root: pathlib.Path, command: list[str]) -> tuple[list[str], list[str]]:
    """Ask the planner (the tree's own `scripts/ci_plan.py`, as CI does):
    the recipes, and what it said besides."""
    done = subprocess.run(command, cwd=root, capture_output=True, text=True)
    if done.returncode != 0:
        raise Refusal("scripts/ci_plan.py refused the plan:\n" + done.stderr.strip())
    return parse_plan(done.stdout), [l for l in done.stderr.splitlines() if l.strip()]


@dataclasses.dataclass
class Job:
    """A recipe to run, with what the plan knows of it."""

    recipe: str
    cost: int
    recorded: float | None
    browsers: bool
    build: bool
    database: bool


def order(names: list[str], costs: dict[str, int]) -> list[str]:
    """The queue: the costliest first, the planner's way (`ci_plan.deal`),
    ties by name so that a deal is the same every time."""
    return sorted(names, key=lambda n: (-costs.get(n, 1), n))


def jobs_of(names: list[str], root: pathlib.Path = ROOT) -> list[Job]:
    """Each recipe, in queue order, with its cost (`ci_plan.estimated`), CI's
    seconds where recorded, and what a shard dealt it would set up."""
    body = ci_plan.bodies(root)
    seconds = ci_plan.measured(root / "scripts/ci_seconds.json")
    costs = ci_plan.estimated(names, body, seconds)
    jobs = []
    for name in order(names, costs):
        browsers, build = ci_plan.needs(body.get(name, ""))
        jobs.append(
            Job(name, costs[name], seconds.get(name), browsers, build, name in ci_plan.NEEDS_DATABASE)
        )
    return jobs


# ---------------------------------------------------------------------------
# Judging a recipe, as CI's summary does
# ---------------------------------------------------------------------------


def judge(status: int | None, survived: list[str]) -> bool:
    """Whether a recipe passed: it exited 0 and its evidence records no mutant
    that survived (`ci_summary.py`'s rule)."""
    return status == 0 and not survived


def survivors(text: str) -> list[str]:
    """The lines of an evidence file that say a mutation control did not hold."""
    return [line.strip() for line in text.splitlines() if any(s in line for s in ci_summary.SURVIVED)]


def bound_lines(recipe: str, text: str) -> list[str]:
    """The lines of an evidence file where a mutation script's memory bound
    stopped a process, as `ci_summary.py` lists them."""
    return [f"{recipe}: {line}" for line in ci_summary.memory_failures(text)]


def failing_lines(text: str, limit: int = 30) -> list[str]:
    """The lines of a log or evidence file that name a failed test, a
    survivor or a compile error, once each, the first `limit`."""
    found: list[str] = []
    for line in ANSI.sub("", text).splitlines():
        if FAILING.search(line) and line.strip() not in found:
            found.append(line.strip())
    return found[:limit]


@dataclasses.dataclass
class Result:
    recipe: str
    verdict: str  # "ok", "FAILED" or "NOT RUN"
    lane: int | None = None
    reason: str = ""
    status: int | None = None
    seconds: float = 0.0
    wrote: list[str] = dataclasses.field(default_factory=list)
    survived: list[str] = dataclasses.field(default_factory=list)
    bound: list[str] = dataclasses.field(default_factory=list)
    notes: list[str] = dataclasses.field(default_factory=list)
    out: str = ""


def not_run(recipe: str, why: str) -> Result:
    return Result(recipe, "NOT RUN", reason=f"not run: {why}")


def result_of(recipe: str, lane: int, out: pathlib.Path, outcome: str, left: list[int]) -> Result:
    """What a lane's run of a recipe came to, from the runner's own record
    (`results.json`) and the evidence it copied. `outcome` is how the wait
    ended: "done", "timeout" or "stopped"; `left` the processes the recipe
    left running."""
    notes = [f"left {len(left)} process(es) running, stopped after it"] if left else []
    try:
        row = next(r for r in json.loads((out / "results.json").read_text()) if r["recipe"] == recipe)
    except (OSError, ValueError, StopIteration):
        why = {"timeout": "timed out", "stopped": "interrupted"}.get(outcome, "the runner recorded no result")
        return Result(recipe, "FAILED", lane, why, notes=notes, out=str(out))
    survived: list[str] = []
    bound: list[str] = []
    for path in row["wrote"]:
        try:
            text = (out / "evidence" / path).read_text(errors="replace")
        except OSError:
            survived.append(f"{path} was written, and the runner's copy of it is missing")
            continue
        survived += survivors(text)
    invalid = ci_summary.recipe_memory_failures(out, row)
    bound = [f"{recipe}: {line}" for line in invalid]
    ok = judge(row["status"], survived + invalid)
    if ok:
        why = ""
    elif row["status"]:
        why = f"exit {row['status']}"
    else:
        why = "; ".join((survived + invalid)[:3])
    return Result(
        recipe, "ok" if ok else "FAILED", lane, why, row["status"], row["seconds"], row["wrote"],
        survived, bound, notes, str(out),
    )


# ---------------------------------------------------------------------------
# Processes
# ---------------------------------------------------------------------------


def alive(pid: int) -> bool:
    """Whether a process runs: a zombie is gone but not yet reaped."""
    try:
        os.kill(pid, 0)
    except OSError:
        return False
    try:
        return pathlib.Path(f"/proc/{pid}/stat").read_text().rsplit(")", 1)[1].split()[0] != "Z"
    except (OSError, IndexError):
        return True


def marked(marker: str) -> list[int]:
    """The processes whose environment carries this lane's marker: those the
    lane started and those they started, in sessions of their own too."""
    proc = pathlib.Path("/proc")
    if not proc.is_dir():
        return []
    needle = f"{MARKER}={marker}".encode()
    found = []
    for entry in proc.iterdir():
        if not entry.name.isdigit() or int(entry.name) == os.getpid():
            continue
        try:
            environ = (entry / "environ").read_bytes()
        except OSError:
            continue
        if needle in environ.split(b"\0"):
            found.append(int(entry.name))
    return found


def stop_marked(marker: str, grace: float = 3.0) -> list[int]:
    """Stop what `marked` finds: SIGTERM, then SIGKILL after `grace` seconds.
    The processes it found."""
    found = marked(marker)
    for sig, wait in ((signal.SIGTERM, grace), (signal.SIGKILL, 0.0)):
        for pid in found:
            try:
                os.kill(pid, sig)
            except OSError:
                pass
        deadline = time.monotonic() + wait
        while time.monotonic() < deadline and any(alive(p) for p in found):
            time.sleep(0.05)
    return found


def terminate(proc: subprocess.Popen, grace: float = 30.0) -> None:
    """Stop a process and its group: SIGTERM, SIGKILL after `grace` seconds."""
    for sig, wait in ((signal.SIGTERM, grace), (signal.SIGKILL, 5.0)):
        try:
            os.killpg(proc.pid, sig)
        except OSError:
            pass
        try:
            proc.wait(timeout=wait)
            return
        except subprocess.TimeoutExpired:
            continue


def watch(
    proc: subprocess.Popen,
    stop: threading.Event,
    timeout: float,
    tick: Callable[[], None] | None = None,
    every: float | None = None,
) -> str:
    """Wait for a process: "done", or "stopped" or "timeout" (the process and
    its group stopped) when the run was stopped or the time passed. `tick`, if
    given, is called every `every` seconds (`GLANCE_EVERY`) while it runs."""
    every = GLANCE_EVERY if every is None else every
    deadline = time.monotonic() + timeout
    looked = time.monotonic()
    while True:
        try:
            proc.wait(timeout=1.0)
            return "done"
        except subprocess.TimeoutExpired:
            pass
        if tick is not None and time.monotonic() - looked >= every:
            looked = time.monotonic()
            try:
                tick()
            except Exception:  # noqa: BLE001 - a look at the evidence must never change a recipe's verdict
                pass
        outcome = "stopped" if stop.is_set() else "timeout" if time.monotonic() >= deadline else "done"
        if outcome != "done":
            terminate(proc)
            return outcome


# ---------------------------------------------------------------------------
# A lane
# ---------------------------------------------------------------------------


class Glance:
    """What a running recipe's evidence says already. A recipe writes its
    evidence as it goes: a survivor, a failed test or a red baseline is in the
    file when it happens, and the run says so then, `GLANCE_LINES` lines to a
    recipe. The verdict is still the recipe's, at its end."""

    def __init__(self, tree: pathlib.Path, recipe: str, lane: int, say: Callable[[str], None], since: float) -> None:
        self.root = tree / "docs" / "evidence"
        self.recipe = recipe
        self.lane = lane
        self.say = say
        self.since = since
        self.offsets: dict[pathlib.Path, int] = {}
        self.said = 0

    def __call__(self) -> None:
        for path in sorted(self.root.rglob("*.txt")):
            try:
                stat = path.stat()
                if stat.st_mtime < self.since:
                    continue
                offset = self.offsets.get(path, 0)
                if stat.st_size < offset:
                    offset = 0
                if stat.st_size == offset:
                    continue
                with open(path, "rb") as handle:
                    handle.seek(offset)
                    chunk = handle.read(stat.st_size - offset)
            except OSError:
                continue
            whole = chunk[: chunk.rfind(b"\n") + 1]
            self.offsets[path] = offset + len(whole)
            for line in failing_lines(whole.decode(errors="replace")):
                self.tell(path, line)

    def tell(self, path: pathlib.Path, line: str) -> None:
        if self.said < GLANCE_LINES:
            self.say(f"lane {self.lane}: {self.recipe} so far: {line}   ({path.name})")
        elif self.said == GLANCE_LINES:
            self.say(f"lane {self.lane}: {self.recipe} so far: more lines like these are in its evidence, and in the report")
        self.said += 1


class Run:
    """What the lanes share: the queue, the results, the stop flag, and a way
    to say what is happening."""

    def __init__(self, root: pathlib.Path, head: str, directory: pathlib.Path) -> None:
        self.root = root
        self.head = head
        self.directory = directory
        self.start = time.monotonic()
        self.stop = threading.Event()
        self.queue: queue.Queue[Job] = queue.Queue()
        self.results: list[Result] = []
        self.lock = threading.Lock()

    def say(self, text: str) -> None:
        elapsed = int(time.monotonic() - self.start)
        stamp = f"{elapsed // 3600:02d}:{elapsed % 3600 // 60:02d}:{elapsed % 60:02d}"
        with self.lock:
            print(f"[{stamp}] {text}", flush=True)

    def record(self, result: Result) -> None:
        with self.lock:
            self.results.append(result)


class Lane:
    def __init__(
        self,
        run: Run,
        index: int,
        port: int,
        jobs: int,
        database: str | None,
        timeout: float,
        setup: bool = True,
    ) -> None:
        self.run = run
        self.index = index
        self.port = port
        self.jobs = jobs
        self.database = database
        self.timeout = timeout
        self.setup = setup
        self.directory = run.directory / f"lane-{index}"
        self.tree = self.directory / "tree"
        self.tmp = self.directory / "tmp"
        self.marker = str(self.directory)
        self.built = False
        self.broken: str | None = None
        self.setup_seconds = 0.0
        self.running: tuple[str, float] | None = None

    # -- the worktree

    def create(self) -> None:
        self.directory.mkdir(parents=True, exist_ok=True)
        self.tmp.mkdir(exist_ok=True)
        added = git(self.run.root, "worktree", "add", "--quiet", "--detach", str(self.tree), self.run.head, check=False)
        if added.returncode != 0:
            raise Refusal(f"cannot make lane {self.index}'s worktree: {added.stderr.strip()}")

    def remove(self, keep: bool = False) -> None:
        """Stop what the lane left, and take the worktree away, its target and
        its temporary directory with it; the logs and the evidence stay."""
        stop_marked(self.marker)
        if keep:
            return
        removed = git(self.run.root, "worktree", "remove", "--force", str(self.tree), check=False)
        if removed.returncode != 0 or self.tree.exists():
            shutil.rmtree(self.tree, ignore_errors=True)
            git(self.run.root, "worktree", "prune", check=False)
        shutil.rmtree(self.tmp, ignore_errors=True)

    # -- what a shard does before its recipes

    def environment(self, database: bool = False) -> dict[str, str]:
        return lane_environment(
            dict(os.environ), self.port, self.tree, self.tmp, self.jobs, self.marker,
            self.database if database else None,
        )

    def step(self, command: str) -> bool:
        """One setup command, in the tree, into the lane's `setup.log`."""
        with open(self.directory / "setup.log", "a") as log:
            log.write(f"$ {command}\n")
            log.flush()
            proc = subprocess.Popen(
                ["bash", "-e", "-c", command], cwd=self.tree, env=self.environment(),
                stdout=log, stderr=subprocess.STDOUT, start_new_session=True,
            )
            outcome = watch(proc, self.run.stop, self.timeout)
            log.write(f"(exit {proc.returncode}, {outcome})\n")
        return outcome == "done" and proc.returncode == 0

    def bootstrap(self) -> None:
        """The toolchain, copied (hard-linked where it can be) from the tree
        this was run in, and the Node workspace, offline: nothing downloaded."""
        started = time.monotonic()
        toolchain = self.run.root / ".toolchain"
        if toolchain.is_dir() and not (self.tree / ".toolchain").exists():
            linked = subprocess.run(["cp", "-al", str(toolchain), str(self.tree / ".toolchain")], capture_output=True)
            if linked.returncode != 0:
                shutil.rmtree(self.tree / ".toolchain", ignore_errors=True)
                shutil.copytree(toolchain, self.tree / ".toolchain", symlinks=True)
        if self.setup:
            for command in BOOTSTRAP_STEPS:
                if not self.step(command):
                    raise SetupFailed(f"`{command}` failed (see {self.directory / 'setup.log'})")
        self.setup_seconds += time.monotonic() - started

    def build(self) -> bool:
        """verify.yml's Build step, once, where a recipe needs the build."""
        if self.built or not self.setup:
            self.built = True
            return True
        started = time.monotonic()
        self.run.say(f"lane {self.index}: building (verify.yml's Build step)")
        try:
            for command in BUILD_STEPS:
                if not self.step(command):
                    self.broken = f"`{command}` failed (see {self.directory / 'setup.log'})"
                    return False
            self.built = True
            return True
        finally:
            self.setup_seconds += time.monotonic() - started

    # -- a recipe

    def attempt(self, job: Job) -> Result:
        """Run one recipe as a shard's runner does, and judge it."""
        out = self.directory / "out" / job.recipe
        out.mkdir(parents=True)
        command = [sys.executable, "scripts/ci_recipes.py", "--out", str(out), job.recipe]
        with open(out / "runner.out", "w") as runner:
            # The files a recipe writes are dated by the filesystem's clock, and so is this one's, made
            # as it starts: what was in the tree before it is not the recipe's.
            glance = Glance(self.tree, job.recipe, self.index, self.run.say, os.fstat(runner.fileno()).st_mtime)
            proc = subprocess.Popen(
                command, cwd=self.tree, env=self.environment(job.database), stdout=runner,
                stderr=subprocess.STDOUT, start_new_session=True,
            )
            outcome = watch(proc, self.run.stop, self.timeout, glance)
        left = stop_marked(self.marker)
        return result_of(job.recipe, self.index, out, outcome, left)

    def work(self) -> None:
        """A lane's life, and a fault in this tool that ends it is said: the
        recipes it did not reach are left for the others."""
        try:
            self.serve()
        except Exception as error:  # noqa: BLE001 - a thread's fault must not pass unsaid
            self.broken = f"preverify failed: {error!r}"
            self.run.say(f"lane {self.index}: stopped, {self.broken}")

    def serve(self) -> None:
        """Set up, then take recipes from the queue until it is empty. A setup
        that fails ends the lane, and what it was to run is left for the
        others."""
        run = self.run
        try:
            self.bootstrap()
        except SetupFailed as error:
            self.broken = str(error)
            run.say(f"lane {self.index}: stopped, {error}")
            return
        run.say(f"lane {self.index}: ready (PORT {self.port}, {self.jobs} cargo jobs)")
        while not run.stop.is_set():
            try:
                job = run.queue.get_nowait()
            except queue.Empty:
                return
            if job.database and not self.database:
                run.record(not_run(job.recipe, "PW_FEED_DATABASE_URL is not set, so it would skip (a shard with "
                                   "a database sets it)"))
                continue
            if job.build and not self.build():
                run.queue.put(job)
                run.say(f"lane {self.index}: stopped, {self.broken}")
                return
            run.say(f"lane {self.index}: {job.recipe} started; log {self.directory / 'out' / job.recipe / 'logs' / (job.recipe + '.log')}")
            self.running = (job.recipe, time.monotonic())
            try:
                result = self.attempt(job)
            except Exception as error:  # noqa: BLE001 - a fault here is this recipe's, said
                result = Result(job.recipe, "FAILED", self.index, f"preverify failed: {error!r}")
            self.running = None
            run.record(result)
            if result.verdict == "ok":
                run.say(f"lane {self.index}: {job.recipe} ok in {result.seconds:.1f} s")
            else:
                run.say(f"lane {self.index}: {job.recipe} FAILED ({result.reason}) in {result.seconds:.1f} s; "
                        f"log {result.out}")


# ---------------------------------------------------------------------------
# The report
# ---------------------------------------------------------------------------


def failure_block(result: Result, lines: int = 40) -> str:
    """What a failed recipe said: its failing test lines, and the tail of its
    log and evidence, as the runner prints them for a job's log."""
    if not result.out:
        return f"--- {result.recipe}: {result.reason}"
    out = pathlib.Path(result.out)
    parts = [f"--- {result.recipe}: {result.reason} (lane {result.lane}, {result.seconds:.1f} s)"]
    texts = []
    for path in [out / "logs" / f"{result.recipe}.log"] + [out / "evidence" / w for w in result.wrote]:
        if path.exists():
            texts.append(path.read_text(errors="replace"))
    found = failing_lines("\n".join(texts))
    if found:
        parts.append("  failing test lines:")
        parts.extend(f"  | {line}" for line in found)
    tail = ci_recipes.failed_tail(result.recipe, out, result.wrote, lines)
    if tail:
        parts.append(tail)
    parts.append(f"  log: {out / 'logs' / (result.recipe + '.log')}")
    return "\n".join(parts)


def render(results: list[Result], jobs: list[Job], meta: dict, setups: list[dict]) -> str:
    """The summary, in the shape of CI's (`ci_summary.py`): the count, each
    failure and why, what a memory bound stopped, a table with each recipe's
    lane and CI's seconds, and then each failure's lines."""
    recorded = {j.recipe: j.recorded for j in jobs}
    rows = sorted(results, key=lambda r: r.recipe)
    failed = [r for r in rows if r.verdict != "ok"]
    lanes_down = [s for s in setups if s["broken"]]
    out = [f"**{len(rows)} recipes, {len(failed)} failed**", ""]
    out += [f"- {r.recipe}: {r.reason}" for r in failed]
    out += [f"- lane {s['lane']} stopped: {s['broken']}" for s in lanes_down]
    out += [""] if failed or lanes_down else []
    bound = [b for r in rows for b in r.bound]
    if bound:
        out += [ci_summary.BOUND_FAILURE, ""]
        out += [f"- {b}" for b in bound] + [""]
    left = [(r.recipe, n) for r in rows for n in r.notes]
    if left:
        out += ["Left running by a recipe, and stopped (CI's runner would have kept them):", ""]
        out += [f"- {recipe}: {note}" for recipe, note in left] + [""]
    out += ["| recipe | result | seconds | lane | CI seconds | files |", "|---|---|---|---|---|---|"]
    for r in rows:
        ci = recorded.get(r.recipe)
        out.append(
            f"| `{r.recipe}` | {r.verdict} | {r.seconds:g} | {'' if r.lane is None else r.lane} | "
            f"{'' if ci is None else format(ci, 'g')} | {len(r.wrote)} |"
        )
    out.append("")
    ci_all = [v for v in recorded.values() if v]
    since = f", changed since {meta['base'][:12]}" if meta["base"] else ", recipes named"
    out.append(
        f"pre-check at {meta['head'][:12]}{since}: {len(rows)} recipes in {meta['lanes']} lanes, "
        f"wall-clock {meta['wall']:.0f} s ({meta['wall'] / 60:.1f} min); CI recorded {sum(ci_all):g} s for the "
        f"{len(ci_all)} it has measured, the longest {max(ci_all, default=0):g} s"
    )
    out.append(
        f"load average (1 min): {meta['load_start']:.1f} at the start, {meta['load_end']:.1f} at the end, "
        f"{meta['load_max']:.1f} at most (sampled every {meta['sample']} s), on {meta['cpus']} threads"
    )
    for s in setups:
        out.append(f"lane {s['lane']}: PORT {s['port']}, setup {s['setup']:.0f} s, {s['recipes']} recipes")
    for r in failed:
        out += ["", failure_block(r)]
    if failed:
        names = " ".join(r.recipe for r in failed)
        out += ["", f'to run only these again: just preverify origin/master --recipes "{names}"']
    return "\n".join(out) + "\n"


# ---------------------------------------------------------------------------
# A run
# ---------------------------------------------------------------------------


# How often a run says what its lanes are doing, in seconds.
STATUS_EVERY = 300.0


def status_line(lanes: list, results: list[Result], total: int, load: float, now: float) -> str:
    """What the lanes are running and how far the queue has got."""
    busy = [f"lane {l.index} {l.running[0]} ({now - l.running[1]:.0f} s)" for l in lanes if l.running]
    failed = sum(1 for r in results if r.verdict != "ok")
    return (
        f"running: {', '.join(busy) or 'nothing'}; {len(results)} of {total} recipes done, {failed} failed; "
        f"load {load:.1f}"
    )


def free_gib(path: pathlib.Path) -> float:
    """The disk free where `path` is, or would be made: at its nearest
    existing parent."""
    while not path.exists():
        path = path.parent
    return shutil.disk_usage(path).free / 2**30


def run_directory(head: str, out: str | None) -> pathlib.Path:
    return pathlib.Path(out).resolve() if out else pathlib.Path("/tmp") / f"preverify-{head[:12]}"


def is_a_run(directory: pathlib.Path) -> bool:
    """Whether a directory is an earlier run's: it holds its plan."""
    return (directory / "plan.json").is_file()


def retire(root: pathlib.Path, directory: pathlib.Path) -> None:
    """An earlier run's directory, at the same head, is kept beside this one
    as `<dir>.prev`: what it left of lanes is taken away first. A directory
    that is no run's is never touched, and a run still going is not taken
    from."""
    git(root, "worktree", "prune", check=False)
    previous = directory.with_name(directory.name + ".prev")
    for d in (directory, previous):
        if d.exists() and any(d.iterdir()) and not is_a_run(d):
            raise Refusal(f"{d} exists and is no earlier run's (it holds no plan.json): name another with --out")
    if directory.exists() and any(directory.iterdir()):
        try:
            owner = int((directory / "pid").read_text())
        except (OSError, ValueError):
            owner = 0
        if owner and alive(owner):
            raise Refusal(f"another preverify run (pid {owner}) is using {directory}")
        for tree in sorted(directory.glob("lane-*/tree")):
            removed = git(root, "worktree", "remove", "--force", str(tree), check=False)
            if removed.returncode != 0:
                shutil.rmtree(tree, ignore_errors=True)
        git(root, "worktree", "prune", check=False)
        shutil.rmtree(previous, ignore_errors=True)
        directory.rename(previous)
    directory.mkdir(parents=True, exist_ok=True)


class Load:
    """The one-minute load average, sampled while a run goes on."""

    def __init__(self, every: float = 30.0) -> None:
        self.every = every
        self.samples = [os.getloadavg()[0]]
        self.done = threading.Event()
        self.thread = threading.Thread(target=self.sample, daemon=True)

    def sample(self) -> None:
        while not self.done.wait(self.every):
            self.samples.append(os.getloadavg()[0])

    def __enter__(self) -> "Load":
        self.thread.start()
        return self

    def __exit__(self, *_: object) -> None:
        self.done.set()
        self.thread.join()
        self.samples.append(os.getloadavg()[0])


def merge_base(root: pathlib.Path, base: str, head: str) -> str:
    done = git(root, "merge-base", base, head, check=False)
    if done.returncode != 0 or not done.stdout.strip():
        raise Refusal(f"no merge base of `{base}` and HEAD: is `{base}` a ref here (git fetch)?")
    return done.stdout.strip()


def parser() -> argparse.ArgumentParser:
    p = argparse.ArgumentParser(
        prog="preverify",
        description=(
            "Run the recipes GitHub's verify would run for HEAD, here, in parallel lanes, before a "
            "train is pushed. A pre-check only: it writes no evidence into this tree and never "
            "pushes; the lanes' evidence and logs go under /tmp/preverify-<head>/, and CI's verify "
            "stays the evidence of record (ADR-0281)."
        ),
    )
    p.add_argument("base", nargs="?", default="origin/master",
                   help="a ref; the plan is what HEAD changed since its merge base with it (default origin/master)")
    p.add_argument("--recipes", action="append", default=[], metavar='"a b"',
                   help="run these recipes instead of planning from BASE (space-separated; may be repeated)")
    p.add_argument("--lanes", type=int, default=4, help="parallel lanes (default 4)")
    p.add_argument("--stale", action=argparse.BooleanOptionalAction, default=None,
                   help="also plan each recipe whose committed evidence is stale (verify.yml does on track/train-*)")
    p.add_argument("--current", action=argparse.BooleanOptionalAction, default=True,
                   help="leave out each recipe whose committed evidence is current at HEAD, as a push does (default)")
    p.add_argument("--plan", action="store_true", help="say what would run, in queue order and on which ports, and stop")
    p.add_argument("--out", metavar="DIR", help="where logs and evidence go (default /tmp/preverify-<head>)")
    p.add_argument("--ports", default=f"{FIRST_PORT}-{LAST_PORT}", metavar="FIRST-LAST",
                   help=f"the range lanes may listen in (default {FIRST_PORT}-{LAST_PORT}); a lane takes the host map's stride")
    p.add_argument("--cpus", type=int, default=os.cpu_count() or 32,
                   help="the threads CARGO_BUILD_JOBS divides among the lanes (default: this machine's)")
    p.add_argument("--timeout", type=float, default=float(ci_plan.LIMIT),
                   help=f"seconds a recipe may take (default {ci_plan.LIMIT}, a CI job's limit)")
    p.add_argument("--min-free-gib", type=float, default=8.0,
                   help="refuse to start with less disk free for each lane (default 8)")
    p.add_argument("--keep-lanes", action="store_true", help="keep the lanes' worktrees, to look into a failure")
    p.add_argument("--no-setup", action="store_true",
                   help="skip the lanes' node install and Build step (for a tree that needs neither)")
    return p


def port_range(text: str) -> tuple[int, int]:
    m = re.fullmatch(r"(\d+)-(\d+)", text)
    if not m or int(m.group(1)) > int(m.group(2)) or int(m.group(2)) > 65535:
        raise Refusal(f"--ports takes FIRST-LAST, ports in order: {text!r}")
    return int(m.group(1)), int(m.group(2))


def main(argv: list[str] | None = None, root: pathlib.Path = ROOT) -> int:
    args = parser().parse_args(argv)
    try:
        return pre_check(args, root)
    except Refusal as error:
        print(f"preverify: {error}", file=sys.stderr)
        return 2
    except subprocess.CalledProcessError as error:
        print(f"preverify: `{' '.join(map(str, error.cmd))}` failed: {(error.stderr or '').strip()}", file=sys.stderr)
        return 2
    except KeyboardInterrupt:
        print("preverify: interrupted", file=sys.stderr)
        return 130


def pre_check(args: argparse.Namespace, root: pathlib.Path) -> int:
    """Plan, check that the run can start, and run it."""
    first, last = port_range(args.ports)
    head = git(root, "rev-parse", "HEAD").stdout.strip()
    branch = git(root, "branch", "--show-current", check=False).stdout.strip()
    recipes = [r for text in args.recipes for r in text.split()]
    stale = branch.startswith(STALE_BRANCHES) if args.stale is None else args.stale
    base = "" if recipes else merge_base(root, args.base, head)
    command = plan_command(base, head, stale, args.current, recipes)
    names, notes = planned(root, command)
    print("preverify: planned as verify.yml plans a track's push: " + shlex.join(["python3", *command[1:]]), flush=True)
    for note in notes:
        print(note, flush=True)
    jobs = jobs_of(names, root)
    where = f"{len(jobs)} recipes at {head[:12]}" + (f", changed since {base[:12]}" if base else ", as named")
    if not jobs:
        print(f"preverify: nothing to run: {where}")
        return 0
    lanes = min(args.lanes, len(jobs))
    ports = lane_ports(lanes, root, first, last)
    if args.plan:
        print(f"preverify: {where}; {lanes} lanes on PORT {', '.join(map(str, ports))}")
        for job in jobs:
            ci = "unmeasured" if job.recorded is None else f"{job.recorded:g} s"
            sets = ", ".join(w for w, on in (("browsers", job.browsers), ("build", job.build), ("database", job.database)) if on)
            print(f"  {job.recipe}: planned {job.cost} s, CI {ci}" + (f" ({sets})" if sets else ""))
        return 0
    with port_lock(first, last):
        return start(args, root, head, base, jobs, lanes, ports)


def start(
    args: argparse.Namespace,
    root: pathlib.Path,
    head: str,
    base: str,
    jobs: list[Job],
    lanes: int,
    ports: list[int],
) -> int:
    """Check that the run can start, and run it."""
    dirt = dirty_paths(root)
    if dirt:
        shown = "; ".join(d.strip() for d in dirt[:5]) + (f"; and {len(dirt) - 5} more" if len(dirt) > 5 else "")
        raise Refusal(
            f"the working tree is not clean ({len(dirt)} paths: {shown}). A lane is a worktree of HEAD "
            f"({head[:12]}), a commit, so it would not hold these: commit or stash them first"
        )
    for port in ports:
        taken = busy_ports(port, root)
        if taken:
            raise Refusal(f"port(s) {', '.join(map(str, taken[:5]))} in a lane's range are in use already: "
                          "another run, or a host one left, holds them")
    directory = run_directory(head, args.out)
    free = free_gib(directory)
    if free < args.min_free_gib * lanes:
        raise Refusal(f"{free:.1f} GiB free under {directory}, and {lanes} lanes need {args.min_free_gib * lanes:g} "
                      f"(--min-free-gib {args.min_free_gib:g} each)")
    retire(root, directory)
    if not (root / ".toolchain").is_dir():
        print("preverify: this tree has no .toolchain (run just bootstrap): a recipe that runs koka or the "
              "wasmtime CLI will fail in a lane, as it would here", file=sys.stderr)
    return execute(args, root, head, base, directory, jobs, lanes, ports)


def execute(
    args: argparse.Namespace,
    root: pathlib.Path,
    head: str,
    base: str,
    directory: pathlib.Path,
    jobs: list[Job],
    lanes: int,
    ports: list[int],
) -> int:
    """Run the queue in the lanes, report, and take the lanes away."""
    run = Run(root, head, directory)
    cargo_jobs = jobs_per_lane(lanes, args.cpus)
    database = os.environ.get("PW_FEED_DATABASE_URL") or None
    team = [Lane(run, k, ports[k], cargo_jobs, database, args.timeout, not args.no_setup) for k in range(lanes)]
    for job in jobs:
        run.queue.put(job)
    interrupted: list[int] = []

    def stop(signum, _frame):
        interrupted.append(signum)
        run.stop.set()

    sample = 30
    previous = {s: signal.signal(s, stop) for s in (signal.SIGINT, signal.SIGTERM)}
    (directory / "pid").write_text(f"{os.getpid()}\n")
    (directory / "plan.json").write_text(json.dumps({
        "head": head, "base": base, "lanes": lanes, "ports": ports,
        "recipes": [dataclasses.asdict(j) for j in jobs]}, indent=2) + "\n")
    run.say(f"preverify {head[:12]}: {len(jobs)} recipes, {lanes} lanes on PORT {', '.join(map(str, ports))}, "
            f"{cargo_jobs} cargo jobs each, load {os.getloadavg()[0]:.1f}; logs under {directory}")
    try:
        with Load(sample) as load:
            try:
                for lane in team:
                    lane.create()
                threads = [threading.Thread(target=lane.work, name=f"lane-{lane.index}") for lane in team]
                for t in threads:
                    t.start()
                last = time.monotonic()
                while any(t.is_alive() for t in threads):
                    for t in threads:
                        t.join(timeout=0.5)
                    if time.monotonic() - last >= STATUS_EVERY:
                        last = time.monotonic()
                        run.say(status_line(team, list(run.results), len(jobs), os.getloadavg()[0], last))
            finally:
                if not args.keep_lanes:
                    run.say("taking the lanes' worktrees away (the logs and evidence stay)")
                for lane in team:
                    lane.remove(keep=args.keep_lanes)
    finally:
        for s, handler in previous.items():
            signal.signal(s, handler)
        (directory / "pid").unlink(missing_ok=True)
    why = "interrupted" if interrupted else "no lane could run it (see the lanes that stopped)"
    while not run.queue.empty():
        run.record(not_run(run.queue.get_nowait().recipe, why))
    meta = {
        "head": head, "base": base, "lanes": lanes, "cpus": args.cpus, "sample": sample,
        "wall": time.monotonic() - run.start, "load_start": load.samples[0], "load_end": load.samples[-1],
        "load_max": max(load.samples),
    }
    setups = [
        {"lane": lane.index, "port": lane.port, "setup": lane.setup_seconds, "broken": lane.broken,
         "recipes": sum(1 for r in run.results if r.lane == lane.index)}
        for lane in team
    ]
    text = render(run.results, jobs, meta, setups)
    (directory / "summary.txt").write_text(text)
    (directory / "summary.json").write_text(json.dumps({
        "meta": meta, "setups": setups,
        "results": [dataclasses.asdict(r) for r in sorted(run.results, key=lambda r: r.recipe)]}, indent=2) + "\n")
    print(text, end="", flush=True)
    return exit_status(run.results, setups, bool(interrupted))


def exit_status(results: list[Result], setups: list[dict], interrupted: bool) -> int:
    """1 where any recipe failed or did not run, a lane stopped, or the run was
    stopped from outside; else 0. (2 is a run that could not start.)"""
    return 1 if interrupted or any(r.verdict != "ok" for r in results) or any(s["broken"] for s in setups) else 0


if __name__ == "__main__":
    sys.exit(main())
