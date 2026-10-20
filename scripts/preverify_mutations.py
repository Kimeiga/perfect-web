#!/usr/bin/env python3
"""Mutation controls for ADR-XXXX: a train is pre-verified here, before it is
pushed (`scripts/preverify.py`, `just preverify`).

Each mutant undoes one thing the tool does: the plan it asks for (a track's
push as verify.yml plans it, the recipes named, what is current or stale, the
queue longest first); the ports of a lane (the host map's stride apart, inside
the range, every family of them, the ones in use refused); what a lane runs
in (its own target, temporary directory, jobs and marker, a runner's cargo
settings, no deployment's variables, a database only for a recipe that needs
one); what it sets up (the toolchain linked or copied, the node install, the
build once and only where a recipe needs it, a lane that cannot set up
stopping and leaving its recipes to the others); how a recipe is judged (as
CI's summary judges it, the failing lines, the tail, the report); the tree it
is run in (clean, untouched), the earlier run kept beside it, the disk, a run
still going; what it stops (a signal, a recipe past its time, what a recipe
left running); and the exit status. And the workflow it follows: verify.yml's
plan, its train rule, its Build step and its cargo settings, which the tests
read, so that a change there fails them until the tool follows.

At least one test of `scripts/tests/test_preverify.py` must then fail. Its
classes run in this order, the cheap ones first, and the run stops at the
first failure: a mutant is killed by the first test that sees it.

Run from the repository root; `just e14-preverify` records the output. The
sources are restored after every mutant, whatever happens.
"""

import os
import pathlib
import re
import signal
import subprocess
import sys

ROOT = pathlib.Path(__file__).resolve().parent.parent
PV = ROOT / "scripts/preverify.py"
VERIFY = ROOT / ".github/workflows/verify.yml"
JUST = ROOT / "just/preverify.just"

# (what is undone, file, anchor, replacement)
MUTANTS = [
    # -- the ports a lane may use, and the cargo settings of a runner
    ("the range for lanes starts below its assignment", PV, "FIRST_PORT = 12741\n", "FIRST_PORT = 12740\n"),
    ("the range for lanes ends past its assignment", PV, "LAST_PORT = 14740\n", "LAST_PORT = 14741\n"),
    ("a lane's incremental builds are the caller's", PV, '    "CARGO_INCREMENTAL": "0",\n', '    "CARGO_INCREMENTAL": "1",\n'),
    ("a lane's cargo output is coloured", PV, '    "CARGO_TERM_COLOR": "never",\n', '    "CARGO_TERM_COLOR": "auto",\n'),
    ("a lane's debug builds carry debug info", PV, '    "CARGO_PROFILE_DEV_DEBUG": "0",\n', '    "CARGO_PROFILE_DEV_DEBUG": "1",\n'),
    ("a lane's test builds carry debug info", PV, '    "CARGO_PROFILE_TEST_DEBUG": "0",\n', '    "CARGO_PROFILE_TEST_DEBUG": "1",\n'),
    ("a deployment's kiokun data reaches a lane", PV, '    "KIOKUN_DATA",\n    "KIOKUN_APP",\n', '    "KIOKUN_APP",\n'),
    ("a deployment's kiokun app reaches a lane", PV, '    "KIOKUN_APP",\n    "PW_FEED_DATABASE_URL",\n', '    "PW_FEED_DATABASE_URL",\n'),
    ("the caller's feed database reaches every recipe", PV, '    "PW_FEED_DATABASE_URL",\n    "PW_STORE_DATABASE_URL",\n', '    "PW_STORE_DATABASE_URL",\n'),
    ("the caller's store database reaches every recipe", PV, '    "PW_STORE_DATABASE_URL",\n    "PW_MUTANT_SLOTS",\n', '    "PW_MUTANT_SLOTS",\n'),
    ("the caller's mutant slots reach a lane", PV, '    "PW_MUTANT_SLOTS",\n    "PW_MUTANT_SLOT_DIR",\n', '    "PW_MUTANT_SLOT_DIR",\n'),
    ("the caller's server reuse reaches a lane", PV, '    "PW_REUSE",\n    "BENCH_REUSE",\n', '    "BENCH_REUSE",\n'),
    (
        "a lane's PORT is the caller's",
        PV,
        'PORT_FAMILIES = ("PORT", "PW_MESSAGES_PORT", "PW_NOTIFICATIONS_PORT", "BENCH_PORT")\n',
        'PORT_FAMILIES = ("PW_MESSAGES_PORT", "PW_NOTIFICATIONS_PORT", "BENCH_PORT")\n',
    ),
    (
        "the messages mutation script's ports are the machine's, not the lane's",
        PV,
        'PORT_FAMILIES = ("PORT", "PW_MESSAGES_PORT", "PW_NOTIFICATIONS_PORT", "BENCH_PORT")\n',
        'PORT_FAMILIES = ("PORT", "PW_NOTIFICATIONS_PORT", "BENCH_PORT")\n',
    ),
    (
        "the notifications mutation script's ports are the machine's, not the lane's",
        PV,
        'PORT_FAMILIES = ("PORT", "PW_MESSAGES_PORT", "PW_NOTIFICATIONS_PORT", "BENCH_PORT")\n',
        'PORT_FAMILIES = ("PORT", "PW_MESSAGES_PORT", "BENCH_PORT")\n',
    ),
    (
        "the benchmark harness's ports are the machine's, not the lane's",
        PV,
        'PORT_FAMILIES = ("PORT", "PW_MESSAGES_PORT", "PW_NOTIFICATIONS_PORT", "BENCH_PORT")\n',
        'PORT_FAMILIES = ("PORT", "PW_MESSAGES_PORT", "PW_NOTIFICATIONS_PORT")\n',
    ),
    ("a lane's processes are marked by another name than the tests look for", PV, 'MARKER = "PW_PREVERIFY_LANE"\n', 'MARKER = "PW_PREVERIFY_LANES"\n'),
    # -- what a shard does before its recipes, as verify.yml does it
    ("a lane's node install may download", PV, 'BOOTSTRAP_STEPS = ["pnpm install --frozen-lockfile --offline"]\n', 'BOOTSTRAP_STEPS = ["pnpm install --frozen-lockfile"]\n'),
    ("a lane's node install may change the lockfile", PV, 'BOOTSTRAP_STEPS = ["pnpm install --frozen-lockfile --offline"]\n', 'BOOTSTRAP_STEPS = ["pnpm install --offline"]\n'),
    ("a lane's build is not locked", PV, '    "cargo build --workspace --locked",\n', '    "cargo build --workspace",\n'),
    ("a lane's build of the store's pages starts the suite", PV, '    "BUILD_ONLY=1 bash spikes/own-renderer/run.sh",\n', '    "bash spikes/own-renderer/run.sh",\n'),
    ("a lane does not build the keyed store", PV, '    "bash spikes/own-renderer/keyed-store.sh",\n', ""),
    ("a lane does not build the feed", PV, '    "bash spikes/own-renderer/feed.sh",\n', ""),
    ("a track's push is planned in other shards", PV, "PLAN_SHARDS = 9\n", "PLAN_SHARDS = 8\n"),
    ("every track's push plans what is stale", PV, 'STALE_BRANCHES = "track/train-"\n', 'STALE_BRANCHES = "track/"\n'),
    # -- the lines a report quotes of a failure
    ("a failed libtest test is not named", PV, r'    r"^test .+ \.\.\. FAILED$"' + "\n", r'    r"^test .+ \.\.\. FAILEDX$"' + "\n"),
    ("a libtest failure's header is not named", PV, r'    r"|^---- .+ stdout ----$"' + "\n", r'    r"|^---- .+ stdoutX ----$"' + "\n"),
    ("a panic is not named", PV, '    r"|panicked at "\n', '    r"|panicked atX "\n'),
    ("a failed libtest summary is not named", PV, r'    r"|^test result: FAILED\."' + "\n", r'    r"|^test result: FAILEDX\."' + "\n"),
    ("a Playwright failure's listing is not named", PV, r'    r"|^ +\d+\) \[[\w-]+\] › "' + "\n", r'    r"|^ +\d+\) \[[\w-]+\] ›X "' + "\n"),
    ("a list reporter's cross is not named", PV, '    r"|^ +✘ "\n', '    r"|^ +✘X "\n'),
    (
        "a count of no failures is a failure",
        PV,
        r'    r"|^ +[1-9]\d* (failed|flaky|interrupted|did not run)\b"' + "\n",
        r'    r"|^ +\d+ (failed|flaky|interrupted|did not run)\b"' + "\n",
    ),
    (
        "a flaky count is not named",
        PV,
        r'    r"|^ +[1-9]\d* (failed|flaky|interrupted|did not run)\b"' + "\n",
        r'    r"|^ +[1-9]\d* (failed|interrupted|did not run)\b"' + "\n",
    ),
    ("an errored unittest is not named", PV, '    r"|^(FAIL|ERROR): "\n', '    r"|^(FAIL): "\n'),
    (
        "a mutation script's missing anchor is not named",
        PV,
        '    r"|: (SURVIVED|ANCHOR NOT FOUND EXACTLY ONCE in .*)$"\n',
        '    r"|: (SURVIVED)$"\n',
    ),
    (
        "a baseline of no failures is a failure",
        PV,
        r'    r"|^baseline \(.*\): \d+ passed, [1-9]\d* failed"' + "\n",
        r'    r"|^baseline \(.*\): \d+ passed, \d+ failed"' + "\n",
    ),
    ("a compiler error is not named", PV, r'    r"|^error(\[E\d+\])?: "' + "\n", r'    r"|^errorX(\[E\d+\])?: "' + "\n"),
    ("an indented error is not named", PV, r'    r"|^\s*(AssertionError|Error): "' + "\n", r'    r"|^(AssertionError|Error): "' + "\n"),
    (
        "a colour with a semicolon is left in a quoted line",
        PV,
        'ANSI = re.compile(r"\\x1b\\[[0-9;]*[A-Za-z]")\n',
        'ANSI = re.compile(r"\\x1b\\[[0-9]*[A-Za-z]")\n',
    ),
    # -- the lanes' ports
    (
        "no lanes is a number of lanes",
        PV,
        '    if lanes < 1:\n        raise Refusal("give a whole number of lanes, 1 or more")\n    try:\n',
        '    if lanes < 0:\n        raise Refusal("give a whole number of lanes, 1 or more")\n    try:\n',
    ),
    (
        "lanes are spaced by a number of the tool's own",
        PV,
        '        ports = [mutation_slots.port_for_slot(k, {"PORT": str(first)}, root) for k in range(lanes)]\n',
        "        ports = [first + k * 200 for k in range(lanes)]\n",
    ),
    ("the last lane may end a port past the range", PV, "    if ports[-1] + stride - 1 > last:\n", "    if ports[-1] + stride > last:\n"),
    ("the last lane's ports are not counted against the range", PV, "    if ports[-1] + stride - 1 > last:\n", "    if ports[-1] > last:\n"),
    ("the room in the range is counted short", PV, "        room = (last - first + 1) // stride\n", "        room = (last - first) // stride\n"),
    (
        "a registry that cannot be read is a traceback",
        PV,
        "    except ValueError as error:\n        raise Refusal(str(error)) from error\n",
        "    except ValueError as error:\n        raise error\n",
    ),
    ("a lane has no cargo jobs where the machine has fewer threads than lanes", PV, "    return max(1, cpus // lanes)\n", "    return cpus // lanes\n"),
    ("the threads are divided among one lane more", PV, "    return max(1, cpus // lanes)\n", "    return max(1, cpus // (lanes + 1))\n"),
    # -- what a lane runs in
    (
        "the caller's environment reaches a lane whole",
        PV,
        "    env = {k: v for k, v in base.items() if k not in NOT_A_RUNNERS}\n",
        "    env = dict(base)\n",
    ),
    ("a lane's cargo settings are the caller's", PV, "    env.update(CI_ENV)\n", ""),
    ("a lane's ports are one past its own", PV, "        env[name] = str(port)\n", "        env[name] = str(port + 1)\n"),
    ("a lane builds in its tree, not its target", PV, '    env["CARGO_TARGET_DIR"] = str(tree / "target")\n', '    env["CARGO_TARGET_DIR"] = str(tree)\n'),
    ("a lane builds with one cargo job too many", PV, '    env["CARGO_BUILD_JOBS"] = str(jobs)\n', '    env["CARGO_BUILD_JOBS"] = str(jobs + 1)\n'),
    ("a lane's temporary directory is its tree", PV, '    env["TMPDIR"] = str(tmp)\n', '    env["TMPDIR"] = str(tree)\n'),
    ("what a lane starts is not marked", PV, "    env[MARKER] = marker\n", ""),
    (
        "every recipe is given a database",
        PV,
        '    if database:\n        env["PW_FEED_DATABASE_URL"] = database\n',
        '    if True:\n        env["PW_FEED_DATABASE_URL"] = database\n',
    ),
    (
        "a database is given under the store's name",
        PV,
        '        env["PW_FEED_DATABASE_URL"] = database\n',
        '        env["PW_STORE_DATABASE_URL"] = database\n',
    ),
    ("the highest reserved port is not checked", PV, "    for n in range(port, port + highest + 1):\n", "    for n in range(port, port + highest):\n"),
    ("the first port of a range is not checked", PV, "    for n in range(port, port + highest + 1):\n", "    for n in range(port + 1, port + highest + 1):\n"),
    ("a port in TIME_WAIT is busy", PV, "            s.setsockopt(socket.SOL_SOCKET, socket.SO_REUSEADDR, 1)\n", "            pass\n"),
    ("no port is busy", PV, "            except OSError:\n                busy.append(n)\n", "            except OSError:\n                pass\n"),
    # -- the plan
    (
        "an untracked file is not a change",
        PV,
        '    out = git(root, "status", "--porcelain", "--untracked-files=normal").stdout\n',
        '    out = git(root, "status", "--porcelain", "--untracked-files=no").stdout\n',
    ),
    ("recipes named are planned from a range", PV, "    if recipes:\n        return command + recipes\n", ""),
    (
        "the range is planned backwards",
        PV,
        '    command += ["--changed", str(base), head]\n',
        '    command += ["--changed", head, str(base)]\n',
    ),
    (
        "current evidence is always left out",
        PV,
        '    if current:\n        command.append("--current")\n',
        '    if True:\n        command.append("--current")\n',
    ),
    (
        "current evidence is never left out",
        PV,
        '    if current:\n        command.append("--current")\n',
        '    if False:\n        command.append("--current")\n',
    ),
    (
        "what is stale is always planned",
        PV,
        '    if stale:\n        command.append("--stale")\n',
        '    if True:\n        command.append("--stale")\n',
    ),
    (
        "a plan is dealt in the planner's default shards",
        PV,
        '    command = [sys.executable, "scripts/ci_plan.py", "--shards", str(PLAN_SHARDS)]\n',
        '    command = [sys.executable, "scripts/ci_plan.py"]\n',
    ),
    (
        "a recipe in two shards is run twice",
        PV,
        "            if recipe not in names:\n                names.append(recipe)\n",
        "            names.append(recipe)\n",
    ),
    (
        "a plan with no plan is read as empty",
        PV,
        '    if line is None:\n        raise Refusal("scripts/ci_plan.py printed no plan (no `shards=` line)")\n',
        "    if line is None:\n        return []\n",
    ),
    (
        "a planner's refusal is not one",
        PV,
        "    if done.returncode != 0:\n        raise Refusal(",
        "    if False:\n        raise Refusal(",
    ),
    (
        "the planner's notes are dropped",
        PV,
        "    return parse_plan(done.stdout), [l for l in done.stderr.splitlines() if l.strip()]\n",
        "    return parse_plan(done.stdout), []\n",
    ),
    ("the cheapest recipe is taken first", PV, "    return sorted(names, key=lambda n: (-costs.get(n, 1), n))\n", "    return sorted(names, key=lambda n: (costs.get(n, 1), n))\n"),
    ("recipes of one cost are taken in the order given", PV, "    return sorted(names, key=lambda n: (-costs.get(n, 1), n))\n", "    return sorted(names, key=lambda n: (-costs.get(n, 1),))\n"),
    ("a recipe without a cost is taken as nothing", PV, "    return sorted(names, key=lambda n: (-costs.get(n, 1), n))\n", "    return sorted(names, key=lambda n: (-costs.get(n, 0), n))\n"),
    (
        "no recipe needs a database",
        PV,
        "            Job(name, costs[name], seconds.get(name), browsers, build, name in ci_plan.NEEDS_DATABASE)\n",
        "            Job(name, costs[name], seconds.get(name), browsers, build, False)\n",
    ),
    (
        "CI's seconds are not kept with a recipe",
        PV,
        "            Job(name, costs[name], seconds.get(name), browsers, build, name in ci_plan.NEEDS_DATABASE)\n",
        "            Job(name, costs[name], None, browsers, build, name in ci_plan.NEEDS_DATABASE)\n",
    ),
    ("a recipe's seconds are read from another tree", PV, '    seconds = ci_plan.measured(root / "scripts/ci_seconds.json")\n', "    seconds = ci_plan.measured()\n"),
    ("a recipe's body is read from another tree", PV, "    body = ci_plan.bodies(root)\n", "    body = ci_plan.bodies()\n"),
    # -- judging a recipe as CI's summary does
    ("a survivor does not fail a recipe", PV, "    return status == 0 and not survived\n", "    return status == 0\n"),
    ("a failed exit with no survivor passes", PV, "    return status == 0 and not survived\n", "    return not survived\n"),
    (
        "a survivor needs both of CI's phrases",
        PV,
        "    return [line.strip() for line in text.splitlines() if any(s in line for s in ci_summary.SURVIVED)]\n",
        "    return [line.strip() for line in text.splitlines() if all(s in line for s in ci_summary.SURVIVED)]\n",
    ),
    (
        "a memory bound that stopped nothing is listed",
        PV,
        "        if line.startswith(ci_summary.BOUND) and line != ci_summary.UNTOUCHED\n",
        "        if line.startswith(ci_summary.BOUND)\n",
    ),
    ("the failing lines are not capped", PV, "    return found[:limit]\n", "    return found\n"),
    (
        "a failing line is quoted each time it is said",
        PV,
        "        if FAILING.search(line) and line.strip() not in found:\n",
        "        if FAILING.search(line):\n",
    ),
    ("colours are left in the failing lines", PV, '    for line in ANSI.sub("", text).splitlines():\n', "    for line in text.splitlines():\n"),
    (
        "what a recipe left running is not said",
        PV,
        '    notes = [f"left {len(left)} process(es) running, stopped after it"] if left else []\n',
        "    notes = []\n",
    ),
    (
        "a recipe that timed out is said interrupted",
        PV,
        '{"timeout": "timed out", "stopped": "interrupted"}',
        '{"timeout": "interrupted", "stopped": "timed out"}',
    ),
    (
        "a recipe with no record passes",
        PV,
        '        return Result(recipe, "FAILED", lane, why, notes=notes, out=str(out))\n',
        '        return Result(recipe, "ok", lane, why, notes=notes, out=str(out))\n',
    ),
    ("a survivor is not read from the evidence", PV, "        survived += survivors(text)\n", "        survived += []\n"),
    ("the memory bound is not read from the evidence", PV, "        bound += bound_lines(recipe, text)\n", "        bound += []\n"),
    ("an exit is not a failure's reason", PV, "    elif row[\"status\"]:\n        why = f\"exit {row['status']}\"\n", "    elif False:\n        why = f\"exit {row['status']}\"\n"),
    ("five survivors are the reason", PV, '        why = "; ".join(survived[:3])\n', '        why = "; ".join(survived[:5])\n'),
    (
        "a recipe's seconds are not the runner's",
        PV,
        '        recipe, "ok" if ok else "FAILED", lane, why, row["status"], row["seconds"], row["wrote"],\n',
        '        recipe, "ok" if ok else "FAILED", lane, why, row["status"], 0.0, row["wrote"],\n',
    ),
    ("a survivor is not weighed in the result", PV, '    ok = judge(row["status"], survived)\n', '    ok = judge(row["status"], [])\n'),
    # -- processes
    (
        "a zombie is alive",
        PV,
        '        return pathlib.Path(f"/proc/{pid}/stat").read_text().rsplit(")", 1)[1].split()[0] != "Z"\n',
        '        return pathlib.Path(f"/proc/{pid}/stat").read_text().rsplit(")", 1)[1].split()[0] != "Zombie"\n',
    ),
    ("a process that is gone is alive", PV, "    except OSError:\n        return False\n    try:\n        return pathlib.Path", "    except OSError:\n        return True\n    try:\n        return pathlib.Path"),
    ("a marker matches as a prefix", PV, '        if needle in environ.split(b"\\0"):\n', "        if needle in environ:\n"),
    ("a lane's marker is not what is looked for", PV, '    needle = f"{MARKER}={marker}".encode()\n', '    needle = f"{MARKER}-{marker}".encode()\n'),
    (
        "what a lane left is stopped by SIGTERM alone",
        PV,
        "    for sig, wait in ((signal.SIGTERM, grace), (signal.SIGKILL, 0.0)):\n",
        "    for sig, wait in ((signal.SIGTERM, grace),):\n",
    ),
    (
        "what a lane left is killed with no grace",
        PV,
        "        while time.monotonic() < deadline and any(alive(p) for p in found):\n",
        "        while False:\n",
    ),
    ("a process stopped is stopped alone, not its group", PV, "            os.killpg(proc.pid, sig)\n", "            os.kill(proc.pid, sig)\n"),
    (
        "a process that ignores SIGTERM is never killed",
        PV,
        "    for sig, wait in ((signal.SIGTERM, grace), (signal.SIGKILL, 5.0)):\n",
        "    for sig, wait in ((signal.SIGTERM, grace),):\n",
    ),
    (
        "a run stopped from outside waits for its recipes",
        PV,
        '        outcome = "stopped" if stop.is_set() else "timeout" if time.monotonic() >= deadline else "done"\n',
        '        outcome = "timeout" if time.monotonic() >= deadline else "done"\n',
    ),
    (
        "a recipe has no time limit",
        PV,
        '        outcome = "stopped" if stop.is_set() else "timeout" if time.monotonic() >= deadline else "done"\n',
        '        outcome = "stopped" if stop.is_set() else "done"\n',
    ),
    (
        "a watched process that overstays is left running",
        PV,
        '        if outcome != "done":\n            terminate(proc)\n            return outcome\n',
        '        if outcome != "done":\n            return outcome\n',
    ),
    # -- the words a run says
    ("the minutes of a stamp run past the hour", PV, "{elapsed % 3600 // 60:02d}", "{elapsed // 60:02d}"),
    ("the hours of a stamp are not counted", PV, "{elapsed // 3600:02d}:", "{0:02d}:"),
    # -- a lane
    (
        "a lane that exists is overwritten",
        PV,
        "        if added.returncode != 0:\n            raise Refusal(",
        "        if False:\n            raise Refusal(",
    ),
    ("a worktree git does not know is left", PV, "        if removed.returncode != 0 or self.tree.exists():\n", "        if False:\n"),
    ("a lane's temporary directory is left", PV, "        shutil.rmtree(self.tmp, ignore_errors=True)\n", "        pass\n"),
    ("lanes to be kept are taken away", PV, "        if keep:\n            return\n", "        if False:\n            return\n"),
    ("a database reaches a lane's setup", PV, "            self.database if database else None,\n", "            self.database,\n"),
    ("a setup command that fails passes", PV, '        return outcome == "done" and proc.returncode == 0\n', '        return outcome == "done"\n'),
    ("the toolchain is not copied", PV, '        if toolchain.is_dir() and not (self.tree / ".toolchain").exists():\n', "        if False:\n"),
    (
        "the toolchain is copied, not linked",
        PV,
        '["cp", "-al", str(toolchain), str(self.tree / ".toolchain")]',
        '["cp", "-a", str(toolchain), str(self.tree / ".toolchain")]',
    ),
    ("a toolchain that cannot be linked is not copied", PV, "            if linked.returncode != 0:\n", "            if False:\n"),
    (
        "setup runs in a lane told to skip it",
        PV,
        "        if self.setup:\n            for command in BOOTSTRAP_STEPS:\n",
        "        if True:\n            for command in BOOTSTRAP_STEPS:\n",
    ),
    ("a lane builds again for each recipe", PV, "        if self.built or not self.setup:\n", "        if not self.setup:\n"),
    ("a lane never knows it has built", PV, "            self.built = True\n            return True\n        finally:\n", "            return True\n        finally:\n"),
    (
        "a database recipe is given no database",
        PV,
        "                command, cwd=self.tree, env=self.environment(job.database), stdout=runner,\n",
        "                command, cwd=self.tree, env=self.environment(), stdout=runner,\n",
    ),
    ("what a recipe left running is left", PV, "        left = stop_marked(self.marker)\n        return result_of", "        left = []\n        return result_of"),
    ("a database recipe is run with no database", PV, "            if job.database and not self.database:\n", "            if False:\n"),
    ("every recipe wants a database", PV, "            if job.database and not self.database:\n", "            if not self.database:\n"),
    ("every recipe wants the build", PV, "            if job.build and not self.build():\n", "            if not self.build():\n"),
    (
        "a recipe whose lane could not build is lost",
        PV,
        '                run.queue.put(job)\n                run.say(f"lane {self.index}: stopped, {self.broken}")\n',
        '                run.say(f"lane {self.index}: stopped, {self.broken}")\n',
    ),
    ("a lane whose setup fails does not say so", PV, "            self.broken = str(error)\n", "            pass\n"),
    ("a lane goes on after the run is stopped", PV, "        while not run.stop.is_set():\n", "        while True:\n"),
    # -- the report
    (
        "a recipe not run has a block of lines",
        PV,
        '    if not result.out:\n        return f"--- {result.recipe}: {result.reason}"\n',
        '    if False:\n        return f"--- {result.recipe}: {result.reason}"\n',
    ),
    ("a failure shows no failing lines", PV, '    found = failing_lines("\\n".join(texts))\n', "    found = []\n"),
    ("a failure shows the last twenty lines", PV, "def failure_block(result: Result, lines: int = 40) -> str:\n", "def failure_block(result: Result, lines: int = 20) -> str:\n"),
    ("the summary is not sorted by recipe", PV, "    rows = sorted(results, key=lambda r: r.recipe)\n", "    rows = list(results)\n"),
    ("a recipe not run is not counted failed", PV, '    failed = [r for r in rows if r.verdict != "ok"]\n    lanes_down', '    failed = [r for r in rows if r.verdict == "FAILED"]\n    lanes_down'),
    ("a lane that stopped is not listed", PV, "    out += [f\"- lane {s['lane']} stopped: {s['broken']}\" for s in lanes_down]\n", "    out += []\n"),
    ("a clean summary has a blank line more", PV, '    out += [""] if failed or lanes_down else []\n', '    out += [""]\n'),
    (
        "the memory bound is not listed",
        PV,
        '    if bound:\n        out += ["Killed, perhaps by the memory bound alone, not by a test:", ""]\n',
        '    if False:\n        out += ["Killed, perhaps by the memory bound alone, not by a test:", ""]\n',
    ),
    (
        "what a recipe left running is not listed",
        PV,
        '    if left:\n        out += ["Left running by a recipe, and stopped',
        '    if False:\n        out += ["Left running by a recipe, and stopped',
    ),
    (
        "CI's seconds are not in the table",
        PV,
        "            f\"{'' if ci is None else format(ci, 'g')} | {len(r.wrote)} |\"\n",
        "            f\"{'' if ci is None else 0} | {len(r.wrote)} |\"\n",
    ),
    ("CI's seconds are counted, not summed", PV, "CI recorded {sum(ci_all):g} s for the ", "CI recorded {len(ci_all):g} s for the "),
    ("the wall-clock is said in hours", PV, "({meta['wall'] / 60:.1f} min)", "({meta['wall'] / 3600:.1f} min)"),
    ("the load at the end is the load at the start", PV, "{meta['load_end']:.1f} at the end", "{meta['load_start']:.1f} at the end"),
    (
        "a lane's setup is not said",
        PV,
        "        out.append(f\"lane {s['lane']}: PORT {s['port']}, setup {s['setup']:.0f} s, {s['recipes']} recipes\")\n",
        "        pass\n",
    ),
    ("a failure's lines are not shown", PV, '    for r in failed:\n        out += ["", failure_block(r)]\n', "    for r in failed:\n        pass\n"),
    ("every recipe is named to run again", PV, '        names = " ".join(r.recipe for r in failed)\n', '        names = " ".join(r.recipe for r in rows)\n'),
    # -- the run directory, and the tree
    ("a run directory is named by a shorter head", PV, 'f"preverify-{head[:12]}"', 'f"preverify-{head[:8]}"'),
    ("any directory is an earlier run's", PV, '    return (directory / "plan.json").is_file()\n', "    return directory.is_dir()\n"),
    ("a previous run's directory is not protected", PV, "    for d in (directory, previous):\n", "    for d in (directory,):\n"),
    ("a run still going is taken from", PV, "        if owner and alive(owner):\n", "        if False:\n"),
    ("a run that died holds its directory", PV, "        if owner and alive(owner):\n", "        if owner:\n"),
    (
        "a run that died leaves its lanes registered",
        PV,
        '        for tree in sorted(directory.glob("lane-*/tree")):\n',
        "        for tree in []:\n",
    ),
    ("an older previous run is not replaced", PV, "        shutil.rmtree(previous, ignore_errors=True)\n        directory.rename(previous)\n", "        directory.rename(previous)\n"),
    ("an earlier run is not kept", PV, "        directory.rename(previous)\n", "        shutil.rmtree(directory, ignore_errors=True)\n"),
    ("the disk is read where nothing is", PV, "    while not path.exists():\n        path = path.parent\n", "    while False:\n        path = path.parent\n"),
    ("the load is not sampled at the end", PV, "        self.thread.join()\n        self.samples.append(os.getloadavg()[0])\n", "        self.thread.join()\n"),
    ("the load is not sampled at the start", PV, "        self.samples = [os.getloadavg()[0]]\n", "        self.samples = []\n"),
    ("a base that is no ref is not refused", PV, "    if done.returncode != 0 or not done.stdout.strip():\n", "    if False:\n"),
    # -- the options
    ("the default lanes are three", PV, 'p.add_argument("--lanes", type=int, default=4,', 'p.add_argument("--lanes", type=int, default=3,'),
    ("the default base is another ref", PV, 'p.add_argument("base", nargs="?", default="origin/master",', 'p.add_argument("base", nargs="?", default="origin/main",'),
    (
        "current evidence is not left out by default",
        PV,
        'p.add_argument("--current", action=argparse.BooleanOptionalAction, default=True,',
        'p.add_argument("--current", action=argparse.BooleanOptionalAction, default=False,',
    ),
    ("a lane needs four GiB of disk", PV, 'p.add_argument("--min-free-gib", type=float, default=8.0,', 'p.add_argument("--min-free-gib", type=float, default=4.0,'),
    ("a recipe may take an hour", PV, 'p.add_argument("--timeout", type=float, default=float(ci_plan.LIMIT),', 'p.add_argument("--timeout", type=float, default=3600.0,'),
    (
        "the default ports are one short",
        PV,
        'p.add_argument("--ports", default=f"{FIRST_PORT}-{LAST_PORT}", metavar="FIRST-LAST",',
        'p.add_argument("--ports", default="12741-14739", metavar="FIRST-LAST",',
    ),
    (
        "the help does not say it is a pre-check only",
        PV,
        '"train is pushed. A pre-check only: it writes no evidence into this tree and never "\n',
        '"train is pushed. A check: it writes no evidence into this tree and never "\n',
    ),
    ("the module does not say it is no evidence", PV, "**A pre-check, not evidence.** It never", "**A check.** It never"),
    ("a one-port range is no range", PV, "    if not m or int(m.group(1)) > int(m.group(2)) or int(m.group(2)) > 65535:\n", "    if not m or int(m.group(1)) >= int(m.group(2)) or int(m.group(2)) > 65535:\n"),
    ("the last port is out of range", PV, "    if not m or int(m.group(1)) > int(m.group(2)) or int(m.group(2)) > 65535:\n", "    if not m or int(m.group(1)) > int(m.group(2)) or int(m.group(2)) >= 65535:\n"),
    ("a range past the ports passes", PV, "    if not m or int(m.group(1)) > int(m.group(2)) or int(m.group(2)) > 65535:\n", "    if not m or int(m.group(1)) > int(m.group(2)):\n"),
    # -- main and the checks before a run
    (
        "a refusal exits 1",
        PV,
        '        print(f"preverify: {error}", file=sys.stderr)\n        return 2\n',
        '        print(f"preverify: {error}", file=sys.stderr)\n        return 1\n',
    ),
    ("git's failure is a traceback", PV, "    except subprocess.CalledProcessError as error:\n", "    except KeyError as error:\n"),
    (
        "a train's branch does not plan what is stale",
        PV,
        "    stale = branch.startswith(STALE_BRANCHES) if args.stale is None else args.stale\n",
        "    stale = False if args.stale is None else args.stale\n",
    ),
    ("a base is merged though recipes are named", PV, '    base = "" if recipes else merge_base(root, args.base, head)\n', "    base = merge_base(root, args.base, head)\n"),
    ("the planner's notes are not shown", PV, "    for note in notes:\n        print(note, flush=True)\n", "    for note in notes:\n        pass\n"),
    (
        "nothing to run is a failure",
        PV,
        '        print(f"preverify: nothing to run: {where}")\n        return 0\n',
        '        print(f"preverify: nothing to run: {where}")\n        return 1\n',
    ),
    ("more lanes than recipes are made", PV, "    lanes = min(args.lanes, len(jobs))\n", "    lanes = args.lanes\n"),
    ("a plan starts a run", PV, "    if args.plan:\n", "    if False:\n"),
    ("a dirty tree is run", PV, "    if dirt:\n        shown =", "    if False:\n        shown ="),
    ("the changes shown are not tidied", PV, "d.strip() for d in dirt[:5]", "d for d in dirt[:5]"),
    ("more than five changes are not counted", PV, '(f"; and {len(dirt) - 5} more" if len(dirt) > 5 else "")', '(f"; and {len(dirt) - 5} more" if len(dirt) > 50 else "")'),
    (
        "ports in use are run on",
        PV,
        '        if taken:\n            raise Refusal(f"port(s)',
        '        if False:\n            raise Refusal(f"port(s)',
    ),
    ("the disk is counted for one lane", PV, "    if free < args.min_free_gib * lanes:\n", "    if free < args.min_free_gib:\n"),
    ("an earlier run is not moved aside", PV, "    retire(root, directory)\n", "    pass\n"),
    ("a tree with no toolchain is not told so", PV, '    if not (root / ".toolchain").is_dir():\n', "    if False:\n"),
    # -- the run
    ("the lanes are given the machine's threads each", PV, "    cargo_jobs = jobs_per_lane(lanes, args.cpus)\n", "    cargo_jobs = args.cpus\n"),
    ("the database is not read from the caller", PV, '    database = os.environ.get("PW_FEED_DATABASE_URL") or None\n', "    database = None\n"),
    ("the lanes set up though told not to", PV, "args.timeout, not args.no_setup) for k in range(lanes)]\n", "args.timeout, args.no_setup) for k in range(lanes)]\n"),
    ("every lane listens on the first lane's ports", PV, "    team = [Lane(run, k, ports[k],", "    team = [Lane(run, k, ports[0],"),
    ("the lanes take the recipes cheapest first", PV, "    for job in jobs:\n        run.queue.put(job)\n", "    for job in reversed(jobs):\n        run.queue.put(job)\n"),
    ("a signal does not stop the lanes", PV, "        interrupted.append(signum)\n        run.stop.set()\n", "        interrupted.append(signum)\n"),
    ("a run does not write its pid", PV, '    (directory / "pid").write_text(f"{os.getpid()}\\n")\n', "    pass\n"),
    ("a run leaves its pid behind", PV, '        (directory / "pid").unlink(missing_ok=True)\n', "        pass\n"),
    (
        "the caller's signal handlers are not put back",
        PV,
        "        for s, handler in previous.items():\n            signal.signal(s, handler)\n",
        "        pass\n",
    ),
    ("lanes to be kept are taken away at the end of a run", PV, "lane.remove(keep=args.keep_lanes)", "lane.remove()"),
    (
        "a run does not say it takes the lanes away",
        PV,
        '                if not args.keep_lanes:\n                    run.say("taking the lanes\' worktrees away (the logs and evidence stay)")\n',
        "                pass\n",
    ),
    (
        "what is left in the queue is lost",
        PV,
        "    while not run.queue.empty():\n        run.record(not_run(run.queue.get_nowait().recipe, why))\n",
        "    pass\n",
    ),
    (
        "a signal is not said",
        PV,
        '    why = "interrupted" if interrupted else "no lane could run it (see the lanes that stopped)"\n',
        '    why = "no lane could run it (see the lanes that stopped)"\n',
    ),
    ("the status is never said", PV, "                    if time.monotonic() - last >= STATUS_EVERY:\n", "                    if False:\n"),
    ("the wall-clock is the clock's", PV, '"wall": time.monotonic() - run.start,', '"wall": time.monotonic() + run.start,'),
    ("a run's plan is not kept", PV, '    (directory / "plan.json").write_text(json.dumps({\n', '    (directory / "plan.txt").write_text(json.dumps({\n'),
    ("a run's summary is not kept", PV, '    (directory / "summary.txt").write_text(text)\n', "    pass\n"),
    (
        "a failed recipe is not a failed run",
        PV,
        '    return 1 if interrupted or any(r.verdict != "ok" for r in results) or any(s["broken"] for s in setups) else 0\n',
        '    return 1 if interrupted or any(s["broken"] for s in setups) else 0\n',
    ),
    (
        "a lane that stopped is not a failed run",
        PV,
        '    return 1 if interrupted or any(r.verdict != "ok" for r in results) or any(s["broken"] for s in setups) else 0\n',
        '    return 1 if interrupted or any(r.verdict != "ok" for r in results) else 0\n',
    ),
    (
        "a run stopped from outside is not a failed run",
        PV,
        '    return 1 if interrupted or any(r.verdict != "ok" for r in results) or any(s["broken"] for s in setups) else 0\n',
        '    return 1 if any(r.verdict != "ok" for r in results) or any(s["broken"] for s in setups) else 0\n',
    ),
    (
        "the status names no lane",
        PV,
        'f"lane {l.index} {l.running[0]} ({now - l.running[1]:.0f} s)"',
        'f"{l.running[0]} ({now - l.running[1]:.0f} s)"',
    ),
    (
        "the status counts only the recipes that failed, not those not run",
        PV,
        '    failed = sum(1 for r in results if r.verdict != "ok")\n',
        '    failed = sum(1 for r in results if r.verdict == "FAILED")\n',
    ),
    # -- a fault in the tool, the ports a run holds, the plan it says
    (
        "a fault in a lane ends it unsaid",
        PV,
        '            self.broken = f"preverify failed: {error!r}"\n            self.run.say(f"lane {self.index}: stopped, {self.broken}")\n',
        '            self.run.say(f"lane {self.index}: stopped")\n',
    ),
    (
        "a fault in a recipe's run loses its result",
        PV,
        "            try:\n                result = self.attempt(job)\n            except Exception as error:  # noqa: BLE001 - a fault here is this recipe's, said\n"
        '                result = Result(job.recipe, "FAILED", self.index, f"preverify failed: {error!r}")\n',
        "            result = self.attempt(job)\n",
    ),
    (
        "evidence the runner did not copy passes",
        PV,
        '            survived.append(f"{path} was written, and the runner\'s copy of it is missing")\n            continue\n',
        "            continue\n",
    ),
    ("a second run takes the ports a run holds", PV, "            fcntl.flock(handle, fcntl.LOCK_EX | fcntl.LOCK_NB)\n", "            pass\n"),
    ("the lock does not name its owner", PV, '        handle.write(f"{os.getpid()}\\n")\n', '        handle.write("\\n")\n'),
    ("every range of ports shares one lock", PV, 'f"preverify-ports-{first}-{last}.lock"', '"preverify-ports.lock"'),
    (
        "a run holds no ports",
        PV,
        "    with port_lock(first, last):\n        return start(args, root, head, base, jobs, lanes, ports)\n",
        "    return start(args, root, head, base, jobs, lanes, ports)\n",
    ),
    (
        "the plan is not said as CI would be asked it",
        PV,
        '    print("preverify: planned as verify.yml plans a track\'s push: " + shlex.join(["python3", *command[1:]]), flush=True)\n',
        "",
    ),
    ("the plan is said with the caller's interpreter", PV, '["python3", *command[1:]]', "[*command]"),
    ("a directory named relatively is kept relative", PV, "    return pathlib.Path(out).resolve() if out else", "    return pathlib.Path(out) if out else"),
    ("a run interrupted before its lanes exist is a traceback", PV, "    except KeyboardInterrupt:\n", "    except SystemExit:\n"),
    # -- what a running recipe's evidence says already
    (
        "a failure already in a running recipe's evidence is not said",
        PV,
        '            for line in failing_lines(whole.decode(errors="replace")):\n                self.tell(path, line)\n',
        '            for line in []:\n                self.tell(path, line)\n',
    ),
    ("a line already said is said at each look", PV, "            self.offsets[path] = offset + len(whole)\n", "            self.offsets[path] = offset\n"),
    ("a line not yet whole is read", PV, '            whole = chunk[: chunk.rfind(b"\\n") + 1]\n', "            whole = chunk\n"),
    (
        "a file written before the recipe began is read",
        PV,
        "                if stat.st_mtime < self.since:\n                    continue\n",
        "",
    ),
    (
        "a file that starts again is read from where the last ended",
        PV,
        "                if stat.st_size < offset:\n                    offset = 0\n",
        "",
    ),
    (
        "files outside the evidence directory are read",
        PV,
        '        for path in sorted(self.root.rglob("*.txt")):\n',
        '        for path in sorted(self.root.parent.rglob("*.txt")):\n',
    ),
    ("files that are not text are read", PV, '        for path in sorted(self.root.rglob("*.txt")):\n', '        for path in sorted(self.root.rglob("*")):\n'),
    ("a recipe is told of every failing line", PV, "        if self.said < GLANCE_LINES:\n", "        if True:\n"),
    ("a recipe is never told there are more", PV, "        elif self.said == GLANCE_LINES:\n", "        elif False:\n"),
    ("a recipe is told of six lines", PV, "GLANCE_LINES = 5\n", "GLANCE_LINES = 6\n"),
    ("the evidence is looked at every quarter of an hour", PV, "GLANCE_EVERY = 15.0\n", "GLANCE_EVERY = 900.0\n"),
    (
        "a running recipe's evidence is not looked at",
        PV,
        "            outcome = watch(proc, self.run.stop, self.timeout, glance)\n",
        "            outcome = watch(proc, self.run.stop, self.timeout)\n",
    ),
    (
        "what the tree carried is taken for the recipe's",
        PV,
        "glance = Glance(self.tree, job.recipe, self.index, self.run.say, os.fstat(runner.fileno()).st_mtime)",
        "glance = Glance(self.tree, job.recipe, self.index, self.run.say, 0.0)",
    ),
    (
        "a watched process is never looked at",
        PV,
        "        if tick is not None and time.monotonic() - looked >= every:\n",
        "        if False:\n",
    ),
    (
        "a watched process is looked at at every wait",
        PV,
        "        if tick is not None and time.monotonic() - looked >= every:\n",
        "        if tick is not None:\n",
    ),
    ("a watch looks at its own pace, not the one asked", PV, "    every = GLANCE_EVERY if every is None else every\n", "    every = GLANCE_EVERY\n"),
    (
        "a look that fails ends the watch",
        PV,
        "            except Exception:  # noqa: BLE001 - a look at the evidence must never change a recipe's verdict\n",
        "            except KeyError:  # noqa: BLE001 - a look at the evidence must never change a recipe's verdict\n",
    ),
    # -- the workflow the tool follows, read by its tests
    (
        "verify.yml plans a track's push in other shards",
        VERIFY,
        'python3 scripts/ci_plan.py --shards 9 --changed "$base" "$GITHUB_SHA" --current $stale',
        'python3 scripts/ci_plan.py --shards 8 --changed "$base" "$GITHUB_SHA" --current $stale',
    ),
    (
        "verify.yml plans a track's push with no regard to its evidence",
        VERIFY,
        'python3 scripts/ci_plan.py --shards 9 --changed "$base" "$GITHUB_SHA" --current $stale',
        'python3 scripts/ci_plan.py --shards 9 --changed "$base" "$GITHUB_SHA" $stale',
    ),
    (
        "verify.yml asks for what is stale on every track",
        VERIFY,
        'case "$GITHUB_REF_NAME" in track/train-*) stale="--stale" ;; esac',
        'case "$GITHUB_REF_NAME" in track/*) stale="--stale" ;; esac',
    ),
    (
        "verify.yml's merge base is another ref",
        VERIFY,
        'base=$(git merge-base origin/master "$GITHUB_SHA")',
        'base=$(git merge-base origin/main "$GITHUB_SHA")',
    ),
    (
        "verify.yml's Build step is not locked",
        VERIFY,
        "          cargo build --workspace --locked\n          BUILD_ONLY=1 bash spikes/own-renderer/run.sh\n",
        "          cargo build --workspace\n          BUILD_ONLY=1 bash spikes/own-renderer/run.sh\n",
    ),
    (
        "verify.yml's Build step builds one thing more",
        VERIFY,
        "          bash spikes/own-renderer/feed.sh\n      - name: Recipes\n",
        "          bash spikes/own-renderer/feed.sh\n          bash spikes/own-renderer/todomvc.sh\n      - name: Recipes\n",
    ),
    ("verify.yml's cargo is incremental", VERIFY, '  CARGO_INCREMENTAL: "0"\n', '  CARGO_INCREMENTAL: "1"\n'),
    # -- the recipe, and the repository the tool must not name
    ("the recipe takes a base and flags, not the tool's arguments", JUST, 'preverify *args:\n    @python3 scripts/preverify.py "$@"\n', 'preverify base="origin/master" *flags:\n    @python3 scripts/preverify.py "$@"\n'),
    ("the recipe joins its arguments into one", JUST, '    @python3 scripts/preverify.py "$@"\n', "    @python3 scripts/preverify.py {{args}}\n"),
    (
        "the tool names the repository it was written for",
        PV,
        "  run could not start. The lanes' worktrees are removed at the end; logs and\n  evidence stay.\n",
        "  run could not start. The lanes' worktrees are removed at the end; logs and\n  evidence stay. See github.com/studiokimeiga/perfect-web.\n",
    ),
    (
        "the recipe asks GitHub before it runs",
        JUST,
        '    @python3 scripts/preverify.py "$@"\n',
        '    @gh run list --repo studiokimeiga/perfect-web > /dev/null\n    @python3 scripts/preverify.py "$@"\n',
    ),
]

# The test classes, cheap first. The first failure ends a mutant's run.
CLASSES = [
    "Ports", "Environment", "Plan", "Steps", "Glancing", "Glances", "Judging", "FailingLines", "Report", "Status", "ExitStatus", "Say", "Help",
    "Load", "LaneUnits", "Processes", "Planner", "CleanTree", "RunTwice", "Locks", "Lanes", "Outcomes", "Setup",
]
MODULE = "scripts.tests.test_preverify"
ALL = [sys.executable, "-m", "unittest", "scripts/tests/test_preverify.py"]
FAST_FIRST = [sys.executable, "-B", "-m", "unittest", "-f", *[f"{MODULE}.{c}" for c in CLASSES]]
# A mutant that makes the tool hang must not hang this: its tests are stopped.
LIMIT = 900


def run_command(cmd):
    """(output, timed out) of a command, in a session of its own, so that a
    mutant that hangs leaves nothing running."""
    env = {**os.environ, "PYTHONDONTWRITEBYTECODE": "1"}
    proc = subprocess.Popen(
        cmd, cwd=ROOT, env=env, stdout=subprocess.PIPE, stderr=subprocess.STDOUT, text=True, start_new_session=True,
    )
    try:
        out, _ = proc.communicate(timeout=LIMIT)
        return out, False
    except subprocess.TimeoutExpired:
        try:
            os.killpg(proc.pid, signal.SIGKILL)
        except OSError:
            pass
        out, _ = proc.communicate()
        return out, True


def run_tests(cmd):
    """(built, passed, failed, timed out) over a unittest command."""
    out, timed_out = run_command(cmd)
    ran = re.search(r"^Ran (\d+) tests?", out, re.M)
    if ran is None:
        return False, 0, 0, timed_out
    bad = re.search(r"^FAILED \((.*)\)", out, re.M)
    n_bad = sum(int(n) for n in re.findall(r"=(\d+)", bad.group(1))) if bad else 0
    return True, int(ran.group(1)) - n_bad, n_bad, timed_out


def main():
    import mutation_baseline  # keeps what each command says, for a red baseline

    # Stopped from outside, the sources are still restored: SIGTERM raises
    # here, and the `finally` below runs.
    signal.signal(signal.SIGTERM, lambda *_: sys.exit(1))
    descriptions = [m[0] for m in MUTANTS]
    if len(set(descriptions)) != len(descriptions):
        dup = sorted({d for d in descriptions if descriptions.count(d) > 1})
        print(f"FAIL: mutants described alike (each is one line of the evidence): {dup}")
        return 1
    built, passed, failed, _ = run_tests(ALL)
    print(f"baseline: {passed} passed, {failed} failed", flush=True)
    if not built or failed or not passed:
        print("FAIL: the unmutated baseline is not green; no mutant can mean anything")
        mutation_baseline.explain()
        return 1
    ordered = run_tests(FAST_FIRST[:4] + [a for a in FAST_FIRST[4:] if a != "-f"])
    if ordered[1:3] != (passed, 0):
        print(f"FAIL: the classes run first ({ordered[1]} tests) are not all the tests ({passed}): list a class in CLASSES")
        return 1

    survivors = 0
    for what, path, anchor, replacement in MUTANTS:
        original = path.read_text()
        if original.count(anchor) != 1:
            print(f"{what}: ANCHOR NOT FOUND EXACTLY ONCE in {path.name}")
            survivors += 1
            continue
        try:
            path.write_text(original.replace(anchor, replacement, 1))
            built, ran, bad, timed_out = run_tests(FAST_FIRST)
        finally:
            path.write_text(original)
        if timed_out:
            verdict = f"KILLED (its tests did not end in {LIMIT} s)"
        elif not built:
            verdict = "KILLED (does not run)"
        elif bad:
            verdict = f"KILLED ({bad} of {ran + bad} tests fail)"
        else:
            verdict = "SURVIVED"
            survivors += 1
        print(f"{what}: {verdict}", flush=True)

    print(f"{len(MUTANTS) - survivors} of {len(MUTANTS)} mutants killed")
    return 1 if survivors else 0


if __name__ == "__main__":
    sys.exit(main())
