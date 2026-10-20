"""**A mutant's builds compile incrementally where the run asks, and the cache
is bounded** (track ci-diagnostics, 2026-10-20).

Each mutant undoes one line, and the build after it is a rebuild of a crate
whose other lines stand. Built with `CARGO_INCREMENTAL=0`, rustc redoes all of
it; incrementally it redoes what the line reaches. On the PC (load 37) a
one-line `pw-core` mutant rebuilt its test binaries in 33 to 37 s with
incremental compilation off and 21 to 23 s with it on; train-9's verify ran
1,391 mutants.

It was off for two reasons, neither about mutants. The recipes say
`CARGO_INCREMENTAL=0 python3 scripts/..._mutations.py` (104 of them) since
2026-09-24, when the Mac's disk filled (STATUS: the incremental cache was
3.6 GB), and `verify.yml` sets it for every job, where `Swatinem/rust-cache`
"automatically sets `CARGO_INCREMENTAL=0` to disable incremental compilation,
so that the Rust compiler doesn't waste time creating the additional artifacts
required for incremental builds" (its README, `v2`, on caching the
dependencies; its `restore.ts` exports it) and its cache "is cleaned of ...
incremental build artifacts" before it is persisted (`cleanTargetDir` keeps a
profile's `build`, `.fingerprint` and `deps` and removes the rest, so
`target/*/incremental` never reaches the saved cache).

`PW_MUTANT_INCREMENTAL=1`, which `verify.yml`'s `Recipes` step sets, removes
`CARGO_INCREMENTAL` from the environment of a mutation script and every process
it starts (`mutation_baseline.py` calls `start()` as the script begins),
its recipe's `CARGO_INCREMENTAL=0` with it: no recipe's line is edited, so none
is planned for it. **Removed, not set to 1**: cargo's own defaults then apply,
incremental for the dev and test profiles, which are a mutant's rebuilds, and
never for a release-like profile; `CARGO_INCREMENTAL=1` forces it for every
profile, the `browser` profile's wasm among them (`run.sh`, `--profile browser`:
opt-level `z`, LTO), whose bytes a spec gates by size. Nothing else is
changed: the `Build` steps that populate the cache, the browser jobs and a
recipe's own commands outside its mutation scripts keep the workflow's `0`.

**It is bounded.** A runner has about 14 GB free, and 25 GB more once what
the image ships is removed (`verify.yml`), and the target directory is some
5 to 9 GB. After each cargo command a mutation script ran, `bounded()` adds up
the incremental caches under the target directory (`target/*/incremental`,
and a target triple's `target/*/*/incremental`) and removes them all when
they hold more than `PW_INCREMENTAL_MAX_MIB` (default 6,144): the next build
starts them again, correct and no slower than it was with them off. And
`ci_recipes.py` removes them between recipes.
"""

from __future__ import annotations

import os
import pathlib
import shutil
import sys

ROOT = pathlib.Path(__file__).resolve().parent.parent

# The most the incremental caches may hold, in MiB, unless
# `PW_INCREMENTAL_MAX_MIB` says another.
MAX_MIB = 6 * 1024


def asked(env=None) -> bool:
    """Whether the run asks for its mutants' builds to be incremental."""
    return (os.environ if env is None else env).get("PW_MUTANT_INCREMENTAL") == "1"


def start() -> None:
    """Leave incremental compilation to cargo's defaults for this process and
    every process it starts (the variable removed, a recipe's zero with it),
    where the run asks; leave the environment alone otherwise."""
    if asked():
        os.environ.pop("CARGO_INCREMENTAL", None)


def limit_bytes() -> int:
    """The most the caches may hold, in bytes."""
    said = os.environ.get("PW_INCREMENTAL_MAX_MIB", "")
    return (int(said) if said.isdigit() else MAX_MIB) * 1024 * 1024


def target_dir() -> pathlib.Path:
    """The target directory cargo builds into."""
    return pathlib.Path(os.environ.get("CARGO_TARGET_DIR") or ROOT / "target")


def caches(target: pathlib.Path) -> list[pathlib.Path]:
    """Every incremental cache under `target`: a profile's (`debug`) and a
    target triple's profile's (`wasm32-unknown-unknown/debug`)."""
    found: list[pathlib.Path] = []
    for pattern in ("*/incremental", "*/*/incremental"):
        found += [p for p in target.glob(pattern) if p.is_dir()]
    return sorted(found)


def tree_bytes(path: pathlib.Path) -> int:
    """What a directory tree takes: its files' sizes, its links not followed."""
    total = 0
    stack = [str(path)]
    while stack:
        here = stack.pop()
        try:
            with os.scandir(here) as entries:
                for entry in entries:
                    if entry.is_dir(follow_symlinks=False):
                        stack.append(entry.path)
                    else:
                        total += entry.stat(follow_symlinks=False).st_size
        except OSError:
            continue
    return total


def runs_cargo(args) -> bool:
    """Whether a command is cargo's: its first word, as a list or as a line."""
    words = [str(a) for a in args] if isinstance(args, (list, tuple)) else str(args).split()
    return bool(words) and pathlib.Path(words[0]).name == "cargo"


def bounded(args, target: pathlib.Path | None = None, limit: int | None = None) -> int:
    """After a command a mutation script ran: where it was cargo's and the
    run asked for incremental builds, remove every incremental cache if
    together they hold more than the limit. The bytes removed (none where they
    hold less, or the command was no cargo's, or the run did not ask)."""
    if not asked() or not runs_cargo(args):
        return 0
    target = target or target_dir()
    limit = limit_bytes() if limit is None else limit
    found = caches(target)
    held = sum(tree_bytes(p) for p in found)
    if held <= limit:
        return 0
    for cache in found:
        shutil.rmtree(cache, ignore_errors=True)
    print(
        f"mutant_incremental: the incremental caches held {held / 2**20:.0f} MiB, past "
        f"{limit / 2**20:.0f}; removed",
        file=sys.stderr,
        flush=True,
    )
    return held
