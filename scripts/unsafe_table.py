#!/usr/bin/env python3
"""E14's gate item 5: which bug classes `pw check` refuses.

For each benchmark task, where each stack's plausible wrong fix (its unsafe
patch) was caught, as the task's recorded controls say
(`docs/evidence/E14/harness-T*.txt`, written by `just e14-harness`). And
what refused Pleris's: `pw check`, run here on the canonical store with the
task's setup and unsafe patches applied, as the harness applies them.

The bug class each task's wrong fixes are instances of is the task's own
statement, in CLASSES below: one line a reviewer can check against the
patches. A stack's wrong fix may differ in shape from another's. E14.md
says why, task by task.

Run from the repository root after `cargo build -p pw-cli`;
`just e14-unsafe-table` records the output.
"""

import pathlib
import re
import shutil
import subprocess
import sys
import tempfile

ROOT = pathlib.Path(__file__).resolve().parent.parent
PW = ROOT / "target/debug/pw"
TASKS = sorted((ROOT / "benchmarks/tasks").glob("T*"))
STACKS = ["pleris", "next-react", "sveltekit"]

# What each task's wrong fixes get wrong, whatever the stack.
CLASSES = {
    "T01": "an optimistic update that cannot be rolled back",
    "T02": "a live value served stale, to spare its source",
    "T03": "one session's data in a cache other sessions read",
    "T04": "a state the page does not show",
    "T05": "a page held back by a slow source it could stream",
    "T06": "a form field with no accessible name",
    "T07": "a stale response shown for a newer request",
    "T08": "a command delivered twice and applied twice",
    "T09": "a value kept longer than the product allows",
    "T10": "a loading state with no failed state",
    "T11": "a modal dialog whose state does not hear it close",
    "T12": "one session's data in a cache other sessions read",
}


def controls(task: str) -> tuple[str, dict[str, str]]:
    """The commit a task's controls were recorded at, and each stack's line."""
    evidence = ROOT / f"docs/evidence/E14/harness-{task}.txt"
    if not evidence.exists():
        return "", {}
    text = evidence.read_text()
    commit = re.search(r"^commit: (\S+)(.*)$", text, re.M)
    lines = {}
    for stack in STACKS:
        m = re.search(rf"^{task} {stack}: (.*)$", text, re.M)
        if m:
            lines[stack] = m.group(1)
    stamp = (commit.group(1)[:7] + commit.group(2)) if commit else "?"
    return stamp, lines


def caught(line: str) -> str:
    """Where the unsafe patch was caught, or why the row is not evidence."""
    held = all(
        f"{control} ok" in line for control in ["negative", "positive", "unsafe", "harness"]
    )
    if not held:
        return "NOT ADMISSIBLE: " + line
    m = re.search(r"unsafe ok \(caught at (\w+)\)", line)
    return m.group(1) if m else "?"


def refusals(task_dir: pathlib.Path) -> str:
    """The codes `pw check` refuses Pleris's unsafe patch with."""
    patches = [task_dir / p / "pleris.patch" for p in ["setup", "unsafe"]]
    if not patches[1].exists():
        return ""
    with tempfile.TemporaryDirectory() as tmp:
        at = pathlib.Path(tmp)
        shutil.copyfile(ROOT / "examples/domain.pw", at / "domain.pw")
        for d in ["lib", "store"]:
            shutil.copytree(ROOT / "examples" / d, at / d)
        for p in patches:
            if p.exists():
                subprocess.run(["git", "apply", str(p)], cwd=at, check=True, capture_output=True)
        files = sorted(str(f) for f in ROOT.glob("packages/pw-std/*.pw"))
        files += sorted(str(f) for f in ROOT.glob("packages/pw-platform-web/*.pw"))
        files += [str(at / "domain.pw")]
        files += sorted(str(f) for f in at.glob("lib/*.pw"))
        files += sorted(str(f) for f in at.glob("store/*.pw"))
        r = subprocess.run([str(PW), "check", "--plain", *files], capture_output=True, text=True)
        said = re.sub(r"\x1b\[[0-9;]*m", "", r.stdout + r.stderr)
        codes = sorted(set(re.findall(r"error: \[(PW\d{4})\]", said)))
        return ", ".join(codes)


def main() -> int:
    rows = []
    for task_dir in TASKS:
        task = task_dir.name.split("-")[0]
        stamp, lines = controls(task)
        if not lines:
            continue
        where = {stack: caught(lines.get(stack, "no line")) for stack in STACKS}
        if where["pleris"] == "check":
            where["pleris"] = f"check ({refusals(task_dir)})"
        rows.append((task, CLASSES.get(task, "?"), where, stamp))

    print("E14 gate item 5 - where each task's plausible wrong fix is caught")
    print()
    print("Stages: check is the stack's static checker (`pw check`, `tsc --noEmit`")
    print("or `svelte-check`), contract is the shared contract tests, and hidden")
    print("is the task's hidden tests. A row is admissible only when all four of")
    print("the task's controls hold on that stack.")
    print()
    width = max(len(c) for _, c, _, _ in rows)
    for task, cls, where, stamp in rows:
        print(f"{task}  {cls:<{width}}  controls at {stamp}")
        for stack in STACKS:
            print(f"      {stack:<11} {where[stack]}")
    print()
    for stack in STACKS:
        at_check = [t for t, _, w, _ in rows if w[stack].startswith("check")]
        print(f"{stack}: {len(at_check)} of {len(rows)} wrong fixes refused by its checker"
              + (f" ({', '.join(at_check)})" if at_check else ""))
    admissible = all(not w[s].startswith("NOT") for _, _, w, _ in rows for s in STACKS)
    return 0 if admissible else 1


if __name__ == "__main__":
    sys.exit(main())
