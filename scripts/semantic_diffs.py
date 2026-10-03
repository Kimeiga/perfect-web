#!/usr/bin/env python3
"""E14-D's evidence: `pw diff` over each benchmark task's Pleris patches
(ADR-0149, charter §19.2).

For each task, the store as the task starts (the canonical store, with the
task's setup patch when it has one) is compared with the same store after
the task's reference patch, and after its unsafe patch. The report is what a
reviewer of that change would be shown. A side that does not check has no
model, and `pw diff` says so: for most unsafe patches that refusal is the
point. A task that starts from a store that does not check (T12) is also
compared with the canonical store.

Run from the repository root after `cargo build -p pw-cli`;
`just e14-diffs` records the output.
"""

import pathlib
import shutil
import subprocess
import sys
import tempfile

ROOT = pathlib.Path(__file__).resolve().parent.parent
PW = ROOT / "target/debug/pw"
TASKS = sorted((ROOT / "benchmarks/tasks").glob("T*"))


def store(at: pathlib.Path, patches: list[pathlib.Path]) -> None:
    """The canonical store's sources at `at`, with `patches` applied in order."""
    at.mkdir(parents=True)
    shutil.copyfile(ROOT / "examples/domain.pw", at / "domain.pw")
    for d in ["lib", "store"]:
        shutil.copytree(ROOT / "examples" / d, at / d)
    for p in patches:
        subprocess.run(["git", "apply", str(p)], cwd=at, check=True, capture_output=True)


def diff(old: pathlib.Path, new: pathlib.Path) -> str:
    r = subprocess.run([str(PW), "diff", str(old), str(new)], capture_output=True, text=True)
    said = r.stdout if r.returncode == 0 else r.stderr
    # Each side by its name, not the temporary directory it was put in.
    return said.replace(str(old), old.name).replace(str(new), new.name)


def main() -> int:
    for task in TASKS:
        setup = task / "setup/pleris.patch"
        given = [setup] if setup.exists() else []
        with tempfile.TemporaryDirectory() as tmp:
            work = pathlib.Path(tmp)
            store(work / "before", given)
            store(work / "canonical", [])
            for change in ["reference", "unsafe"]:
                patch = task / change / "pleris.patch"
                if not patch.exists():
                    continue
                store(work / change, given + [patch])
                print(f"=== {task.name}: {change}")
                print()
                said = diff(work / "before", work / change)
                if said.startswith("pw diff: before: it does not check"):
                    # A task that starts from a program the compiler refuses
                    # (T12) has nothing before it to compare with. What the
                    # change means is told against the canonical store.
                    print(said.rstrip())
                    print()
                    print("Against the canonical store, which checks:")
                    print()
                    said = diff(work / "canonical", work / change)
                print(said.rstrip())
                print()
    return 0


if __name__ == "__main__":
    sys.exit(main())
