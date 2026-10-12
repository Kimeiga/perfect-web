#!/usr/bin/env python3
"""Copy files of kiokun.com's app from its commit at HEAD, for an oracle.

The kiokun track's oracles run kiokun.com's own code (the integrator's
ruling of 2026-10-09), and read the checkout's committed tree at HEAD,
never its working tree (its ruling of 2026-10-10): the evidence names a
commit, and is reproducible only if what was read is that commit. Each
file is taken with `git show HEAD:./PATH` into the oracle's temporary
directory, and the line the evidence prints is written: the commit, and
each file's SHA-256 as committed. Work the owner has not committed under
the files' paths is named as not read; it is information, not a failure.

Nothing is written into the checkout: git is asked only for the commit,
the files and their status, without taking its optional locks
(`GIT_OPTIONAL_LOCKS=0`).

usage: kiokun_app_source.py APP OUT PATH[=NAME]...
  APP   the checkout's sveltekit-app
  OUT   where the files go: each at OUT/NAME, NAME the path's last part
        unless given
  PATH  a path under APP, as committed
"""

import hashlib
import os
import pathlib
import subprocess
import sys


def git(app: pathlib.Path, *args: str) -> subprocess.CompletedProcess:
    env = {**os.environ, "GIT_OPTIONAL_LOCKS": "0"}
    return subprocess.run(["git", "-C", str(app), *args], capture_output=True, env=env)


def main(argv: list[str]) -> int:
    if len(argv) < 4:
        print("usage: kiokun_app_source.py APP OUT PATH[=NAME]...", file=sys.stderr)
        return 2
    app, out = pathlib.Path(argv[1]), pathlib.Path(argv[2])
    head = git(app, "rev-parse", "HEAD").stdout.decode().strip()
    if not head:
        print(f"kiokun_app_source: {app} has no commit to read", file=sys.stderr)
        return 1
    digests = []
    paths = []
    for spec in argv[3:]:
        path, _, name = spec.partition("=")
        shown = git(app, "show", f"HEAD:./{path}")
        if shown.returncode != 0:
            print(f"kiokun_app_source: {path} is not in {head}", file=sys.stderr)
            return 1
        target = out / (name or pathlib.PurePosixPath(path).name)
        target.parent.mkdir(parents=True, exist_ok=True)
        target.write_bytes(shown.stdout)
        digests.append(f"{path} {hashlib.sha256(shown.stdout).hexdigest()[:12]}")
        paths.append(path)
    print(f"kiokun.com's source: {head}, its committed tree ({'; '.join(digests)})")
    unread = git(app, "status", "--porcelain", "--", *paths).stdout.decode().splitlines()
    if unread:
        print(f"the checkout has uncommitted work, not read: {', '.join(l[3:] for l in unread)}")
    return 0


if __name__ == "__main__":
    sys.exit(main(sys.argv))
