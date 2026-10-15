#!/usr/bin/env python3
"""Mutation controls for NEXT 4c's first step (track `render-merge`): within
one render the host makes each identical operation call once, the
operation and its arguments as the key, every binding that asks given the
same answer; and the render's calls read one snapshot of the data layer.

Each mutant undoes one rule:

- the merge (`render_merge.rs`): a kept answer never given; the key without
  the arguments, or without the session; a failure kept; nothing ended with
  the render; a render begun inside another for the same session not that
  one, or one for another session that one; a render answering for another
  layer;
- the host (`main.rs`): the drain's, the documents' and a page's renders
  not begun; a call answered again counted as made (`/metrics`); the
  platform's calls merged (the clock's, which note what a read holds until);
- the snapshot: a render's queries each reading as a query of their own
  (the host), the store's and the feed's in memory each reading the data as
  it is now, the store's and the feed's on PostgreSQL ending their read
  transaction after every read, or keeping it after a read that failed; and
  the store's render waiting on the pool's bound.

A mutant must fail the suite it names: `memory`, the merge's own tests and
the served ones, the store in memory (`render_merge::`); `postgres`, the
served ones with the store on PostgreSQL, and NEXT 4b's counts re-pinned
(`render_merge::`, `tests::bounded_statements::`), which needs
PW_STORE_DATABASE_URL and PW_FEED_DATABASE_URL.

Run from the repository root; `just e14-render-merge` records the output.
The source is restored after every mutant, whatever happens.
"""

import os
import pathlib
import re
import signal
import subprocess
import sys

ROOT = pathlib.Path(__file__).resolve().parent.parent
SRC = ROOT / "spikes/own-renderer/server/src"
MERGE = SRC / "render_merge.rs"
MAIN = SRC / "main.rs"
STORE = SRC / "store.rs"
STORE_PG = SRC / "store_pg.rs"
FEED = SRC / "feed.rs"
FEED_PG = SRC / "feed_pg.rs"

# (what, suite, file, anchor, replacement)
MUTANTS = [
    (
        "a render's kept answer is never given",
        "memory",
        MERGE,
        "            if let Some(answer) = render.answered(&session, &name, args) {\n",
        "            if let Some(answer) = render.answered(&session, &name, args).filter(|_| false) {\n",
    ),
    (
        "a call is answered as one with other arguments was",
        "memory",
        MERGE,
        "            .find(|((s, n, a), _)| s == session && n == name && a.as_slice() == args)\n",
        "            .find(|((s, n, a), _)| s == session && n == name && (a.as_slice() == args || !args.is_empty()))\n",
    ),
    (
        "a call is answered as one for another session was",
        "memory",
        MERGE,
        "            .find(|((s, n, a), _)| s == session && n == name && a.as_slice() == args)\n",
        "            .find(|((s, n, a), _)| (s == session || s != session) && n == name && a.as_slice() == args)\n",
    ),
    (
        "a failed call is kept as its answer",
        "memory",
        MERGE,
        "            let answer = op(args)?;\n",
        "            let answer = match op(args) {\n"
        "                Ok(answer) => answer,\n"
        "                Err(why) => vec![Val::String(why)],\n"
        "            };\n",
    ),
    (
        "a render is kept past its end",
        "memory",
        MERGE,
        "                reading.retain(|r| !Arc::ptr_eq(r, &mine));\n",
        "                reading.retain(|r| !Arc::ptr_eq(r, &mine) || Arc::ptr_eq(r, &mine));\n",
    ),
    (
        "a render begun inside another for its session is its own",
        "memory",
        MERGE,
        "            .is_some_and(|r| r.layer == layer && r.session == session)\n",
        "            .is_some_and(|r| r.layer == layer && r.session == session && r.layer != layer)\n",
    ),
    (
        "a render begun inside another for another session is that one",
        "memory",
        MERGE,
        "            .is_some_and(|r| r.layer == layer && r.session == session)\n",
        "            .is_some_and(|r| r.layer == layer && (r.session == session || r.session != session))\n",
    ),
    (
        "a render answers for another layer",
        "memory",
        MERGE,
        "            .find(|r| r.layer == layer)\n",
        "            .find(|r| r.layer == layer || r.layer != layer)\n",
    ),
    (
        "a render's queries each read as a query of their own",
        "memory",
        MERGE,
        "            Some(snapshot) => data.reads_in(snapshot, session, stopped),\n",
        "            Some(_) => data.reads(session, stopped),\n",
    ),
    (
        "the drain's render is not begun",
        "memory",
        MAIN,
        "        let _render = render_merge::begin(&*self.data, session);\n"
        "        // The count the page shows, as the page computes it: the cart query's\n",
        "        // The count the page shows, as the page computes it: the cart query's\n",
    ),
    (
        "the documents read again are not one render",
        "memory",
        MAIN,
        "        let render = render_merge::begin(&*self.data, session);\n"
        "        let mut read = Vec::new();\n",
        "        let render = std::cell::Cell::new(0);\n"
        "        let mut read = Vec::new();\n",
    ),
    (
        "a page's bindings are not one render",
        "memory",
        MAIN,
        "        let _render = render_merge::begin(&*self.data, session);\n"
        "        let page = document.map_or_else(\n",
        "        let page = document.map_or_else(\n",
    ),
    (
        "a call answered again is counted as made",
        "memory",
        MAIN,
        "                let f = counted(&name, f);\n"
        "                let f = match &render {\n"
        "                    Some(render) => render.once(session, &name, f),\n"
        "                    None => f,\n"
        "                };\n",
        "                let f = match &render {\n"
        "                    Some(render) => render.once(session, &name, f),\n"
        "                    None => f,\n"
        "                };\n"
        "                let f = counted(&name, f);\n",
    ),
    (
        "the platform's calls are merged",
        "memory",
        MAIN,
        "        host.extend(platform.into_iter().map(|(name, f)| {\n"
        "            let f = counted(&name, f);\n",
        "        host.extend(platform.into_iter().map(|(name, f)| {\n"
        "            let f = counted(&name, f);\n"
        "            let f = match &render {\n"
        "                Some(render) => render.once(session, &name, f),\n"
        "                None => f,\n"
        "            };\n",
    ),
    (
        "the store's render in memory reads the store as it is now",
        "memory",
        STORE,
        "            Some(rows) => reads(with_mem(rows.clone()), session, &self.faults, stopped),\n",
        "            Some(_) => self.reads(session, stopped),\n",
    ),
    (
        "the feed's render in memory reads the feed as it is now",
        "memory",
        FEED,
        "            Some(state) => reads_of(state.clone(), self.principals()),\n",
        "            Some(_) => self.reads(session, stopped),\n",
    ),
    (
        "the store's render on PostgreSQL ends its transaction after every read",
        "postgres",
        STORE_PG,
        "        let out = tally.around(|| f(&mut PgRows(Counted(c))));\n"
        "        if out.is_err() {\n",
        "        let out = tally.around(|| f(&mut PgRows(Counted(c))));\n"
        "        if out.is_err() || out.is_ok() {\n",
    ),
    (
        "the store's render on PostgreSQL keeps its transaction after a read that failed",
        "postgres",
        STORE_PG,
        "            self.end(conn.take().expect(\"opened\"), \"ROLLBACK\");\n",
        "            let _ = &conn;\n",
    ),
    (
        "the store's render on PostgreSQL waits on the pool's bound",
        "postgres",
        STORE_PG,
        "        let idle = self.idle.lock().expect(\"idle connections\").0.pop();\n",
        "        let idle = {\n"
        "            let mut held = self.idle.lock().expect(\"idle connections\");\n"
        "            while held.1 >= CONNECTIONS {\n"
        "                held = self.given_back.wait(held).expect(\"idle connections\");\n"
        "            }\n"
        "            held.0.pop()\n"
        "        };\n",
    ),
    (
        "the store's render's connection is kept idle past the pool's bound",
        "postgres",
        STORE_PG,
        "        if !c.is_closed() && idle.0.len() + idle.1 < CONNECTIONS {\n",
        "        if !c.is_closed() && idle.0.len() < CONNECTIONS {\n",
    ),
    (
        "the feed's render on PostgreSQL ends its transaction after every read",
        "postgres",
        FEED_PG,
        "        let out = f(client.as_mut().expect(\"opened\"));\n"
        "        if out.is_err() {\n",
        "        let out = f(client.as_mut().expect(\"opened\"));\n"
        "        if out.is_err() || out.is_ok() {\n",
    ),
    (
        "the feed's render on PostgreSQL keeps its transaction after a read that failed",
        "postgres",
        FEED_PG,
        "            self.end(client.take().expect(\"opened\"), \"ROLLBACK\");\n",
        "            let _ = &client;\n",
    ),
]

TESTS = ["cargo", "test", "--quiet", "--locked", "-p", "pw-dev-server", "--"]
CARGO = {
    "memory": (TESTS + ["render_merge::"], {"PW_STORE_TEST_LAYER": "memory"}),
    "postgres": (
        TESTS + ["render_merge::", "tests::bounded_statements::"],
        {"PW_STORE_TEST_LAYER": "postgres"},
    ),
}

# How long one command may run. Past the bound it is killed with everything
# it started, and the run counts as failing.
BOUND = 1200


def bounded(cmd, env):
    """(output, returncode), or (output, None) when it ran past the bound."""
    p = subprocess.Popen(cmd, cwd=ROOT, env={**os.environ, **env}, start_new_session=True,
                         stdout=subprocess.PIPE, stderr=subprocess.STDOUT, text=True)
    try:
        out, _ = p.communicate(timeout=BOUND)
        return out, p.returncode
    except subprocess.TimeoutExpired:
        os.killpg(p.pid, signal.SIGKILL)
        out, _ = p.communicate()
        return out, None


def cargo_tests(suite):
    """(built, passed, failed) over the suite's tests."""
    cmd, env = CARGO[suite]
    out, code = bounded(cmd, env)
    if code is None:
        return True, 0, 1
    found = re.findall(r"test result: \w+\. (\d+) passed; (\d+) failed", out)
    if not found:
        return False, 0, 0
    return True, sum(int(p) for p, _ in found), sum(int(f) for _, f in found)


def main():
    import mutation_baseline  # keeps what each command says, for a red baseline
    # Stopped from outside, the source is still restored: SIGTERM raises
    # here, and the `finally` below runs.
    signal.signal(signal.SIGTERM, lambda *_: sys.exit(1))
    for need in ("PW_STORE_DATABASE_URL", "PW_FEED_DATABASE_URL"):
        if not os.environ.get(need):
            print(f"FAIL: the postgres suite needs {need}, a throwaway database")
            return 1
    for suite in CARGO:
        built, passed, failed = cargo_tests(suite)
        print(f"baseline ({suite}): {passed} passed, {failed} failed", flush=True)
        if not built or failed or not passed:
            print("FAIL: the unmutated baseline is not green; no mutant can mean anything")
            mutation_baseline.explain()
            return 1

    survivors = 0
    for what, suite, path, anchor, replacement in MUTANTS:
        original = path.read_text()
        if original.count(anchor) != 1:
            print(f"{what}: ANCHOR NOT FOUND EXACTLY ONCE in {path.name}")
            survivors += 1
            continue
        try:
            path.write_text(original.replace(anchor, replacement, 1))
            built, passed, failed = cargo_tests(suite)
        finally:
            path.write_text(original)
        if not built:
            verdict = "KILLED (does not build)"
        elif failed:
            verdict = f"KILLED ({failed} of {passed + failed} tests fail)"
        else:
            verdict = "SURVIVED"
            survivors += 1
        print(f"{what} [{suite}]: {verdict}", flush=True)

    print(f"{len(MUTANTS) - survivors} of {len(MUTANTS)} mutants killed")
    return 1 if survivors else 0


if __name__ == "__main__":
    sys.exit(main())
