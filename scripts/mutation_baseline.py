"""**What made a mutation script's baseline red** (2026-10-08).

A script whose unmutated baseline fails says so and stops: "no mutant can
mean anything". Until this it said no more, and a red baseline on CI or here
named neither the test that failed nor the build error: on 2026-10-08
`invariants_mutations.py` said "586 passed, 2 failed", and finding the two,
two committed components gone stale, took the suite run again by hand.

A script imports this module first in its `main()`. From then on what each
command it runs says is kept as the command ends, with how it ended: the
last `KEPT` commands, the end of what each said. Where the script reports a
red baseline, `explain()` prints what failed in what its commands said: each
failing test and the first line of its panic, a build's first errors, or,
from a browser suite, each failing test and its error. Nothing is run again:
a failure that comes and goes is named as it came, and the commands are the
script's own, however it holds them, in their own directories and
environments.

It keeps what `subprocess.Popen.communicate` returns, which
`subprocess.run` calls. A script imports it in `main()`, not at its top, so
that loading a script to read its mutants (`ci_plan.py`,
`mutation_anchors.py`) needs nothing beside it.

Imported, it also bounds in memory every process the script starts from
then on (`mutation_bound.py`, 2026-10-09): a mutant's test that holds more
than the bound is stopped, and the script's output says so.
"""

import collections
import re
import subprocess
import sys

import mutation_bound

# Every process the script starts from here on, bounded in memory.
mutation_bound.start()

# How many commands are kept: a baseline's, and whatever ran since.
KEPT = 64
# How much of what one command said is kept: its end, where a test runner
# and a compiler say what failed.
TAIL = 256 * 1024

# Each command kept: [its arguments, its exit status or None where it was
# stopped past its bound, what it said].
heard = collections.deque(maxlen=KEPT)

ESCAPES = re.compile(r"\x1b\[[0-9;]*[A-Za-z]")


def as_text(data):
    """What a stream said, as text."""
    if data is None:
        return ""
    if isinstance(data, bytes):
        return data.decode("utf-8", "replace")
    return data


def keep(popen, status, said):
    """Keep what one command said; said again, its later word stands."""
    said = said[-TAIL:]
    kept = getattr(popen, "_heard", None)
    if kept is None:
        popen._heard = kept = [popen.args, status, said]
        heard.append(kept)
    else:
        kept[1] = status
        kept[2] = said or kept[2]


def keeping(communicate):
    def communicated(self, *args, **kwargs):
        try:
            out, err = communicate(self, *args, **kwargs)
        except subprocess.TimeoutExpired as stopped:
            keep(self, None, as_text(stopped.stdout) + as_text(stopped.stderr))
            raise
        keep(self, self.returncode, as_text(out) + as_text(err))
        return out, err

    communicated.keeps = True
    return communicated


if not getattr(subprocess.Popen.communicate, "keeps", False):
    subprocess.Popen.communicate = keeping(subprocess.Popen.communicate)


def failures(output, failed=True):
    """What a command's output says failed, at most twenty lines. Output
    with no result at all is shown as it ended, where the command failed."""
    text = ESCAPES.sub("", output)
    lines = text.splitlines()
    said = []
    # cargo: a failing test, and the first line of its panic.
    for i, line in enumerate(lines):
        m = re.match(r"^test (\S+) \.\.\. FAILED$", line)
        if m:
            said.append(f"failed: {m.group(1)}")
        m = re.match(r"^thread '([^']+)' .*panicked at (\S+):$", line)
        if m and i + 1 < len(lines):
            said.append(f"  {m.group(1)} at {m.group(2)}: {lines[i + 1].strip()[:160]}")
    # A build that failed: its first errors, where.
    for i, line in enumerate(lines):
        if re.match(r"^error(\[E\d+\])?: ", line):
            where = next(
                (l.strip() for l in lines[i + 1 : i + 4] if l.strip().startswith("-->")), ""
            )
            said.append(f"build: {line.strip()[:160]} {where}".rstrip())
    # Playwright: each failing test, and its error.
    for i, line in enumerate(lines):
        m = re.match(r"^\s+\d+\) (\[\w+\] › .+)$", line)
        if m:
            error = next((l.strip() for l in lines[i + 1 : i + 6] if "Error" in l), "")
            said.append(f"failed: {m.group(1).strip()[:160]} {error[:120]}".rstrip())
    # No result at all: how its output ended, which says why.
    if failed and not said and not result(text) and lines:
        said = ["no test result; its output ended:"] + [f"  {l[:160]}" for l in lines[-12:]]
    return said[:20]


def result(output):
    """A test runner's last result line: cargo's, or Playwright's count."""
    found = re.findall(
        r"^(test result: .*|\s+\d+ (?:passed|failed|flaky|skipped).*)$",
        ESCAPES.sub("", output),
        re.M,
    )
    return found[-1].strip() if found else ""


def shown(args):
    """A command as it was run, shortened."""
    line = " ".join(str(a) for a in args) if isinstance(args, (list, tuple)) else str(args)
    return line if len(line) <= 160 else line[:157] + "..."


def explain(out=None):
    """Print what failed in what the commands kept said. Nothing is run
    again."""
    out = out or sys.stdout
    print("what failed, as its commands said it:", file=out)
    told = False
    for args, status, said in heard:
        lines = failures(said, failed=status != 0)
        if status == 0 and not lines:
            continue
        ended = "stopped past its bound" if status is None else f"exit {status}"
        print(f"  {shown(args)} ({ended}):", file=out)
        for line in lines or ["(what it said was not captured)"]:
            print(f"    {line}", file=out)
        told = True
    if not told and heard:
        # Each command ended well, and the script counted its baseline red:
        # no test passed, or none ran. Each one's last result says which.
        print("  each command ended well; what each counted:", file=out)
        for args, _, said in heard:
            print(f"  {shown(args)}: {result(said) or '(no test result)'}", file=out)
    if not heard:
        print("  (no command it ran was kept)", file=out)
    out.flush()
