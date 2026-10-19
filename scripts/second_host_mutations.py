#!/usr/bin/env python3
"""Mutation controls for track second-host (ADR-XXXX): hosts that share one
PostgreSQL database hear each other's commits from the outbox, answer each
other's retries from the record a commit wrote, and keep their sessions in
the database, by hash.

Each mutant undoes one rule:
- the outbox in commit order: a commit through either layer that takes no
  lock, or takes its rows' ids before it; the lock that is no lock; a commit
  that wakes no host;
- the outbox a log every host reads: opening either layer deletes it; a
  reader that tells its own host's commits, or passes the others' by; a
  reader behind the trim not told it missed rows; the trim's mark read
  before the rows; a trim that records no mark; a cursor that never moves;
  a host opened that reads from the start; a layer that listens when it
  opens; a dropped listening connection never made again; rows that carry
  no host, in either layer; the other hosts' commits heard and not told; a
  host that never reads them (the browser's);
- sessions kept by hash: a sign-in not kept, a sign-out leaving its kept
  session, a request that reads no session or reads it twice, a session the
  database no longer has kept signed in, a sign-in completed twice, its age
  not the database's, an abandoned sign-in kept, a session kept by its id,
  the sessions' connections unbounded;
- a retry across hosts: a press on another host's page not answered from
  its record, or looked for under the page's session alone;
- the deployment's schema (`PW_FEED_SCHEMA`) not the one its tables are in
  (the browser's).

The tests then fail: `tests::second_host::` (`hosts`), two of the store's
PostgreSQL tests (`store`), or the two-host browser suite in Chromium
(`browser`, `e2e-hosts/second-host.spec.mjs` through
`playwright.hosts.config.mjs`; it needs the feed's build, `feed.sh`). Every
suite needs a database: PW_FEED_DATABASE_URL and PW_STORE_DATABASE_URL, a
throwaway one, which each test uses in a schema of its own.

Run from the repository root; `just e14-second-host` records the output.
The source is restored after every mutant, whatever happens.
"""

import os
import pathlib
import re
import signal
import subprocess
import sys

from cargo_test_results import parse as cargo_result

ROOT = pathlib.Path(__file__).resolve().parent.parent
SRC = ROOT / "spikes/own-renderer/server/src"
HOSTS = SRC / "hosts.rs"
FEED = SRC / "feed_pg.rs"
STORE = SRC / "store_pg.rs"
SERVER = SRC / "main.rs"
IDENTITY = SRC / "identity.rs"

# (what, suite, file, anchor, replacement)
MUTANTS = [
    # --- The outbox in commit order.
    (
        "a commit through the feed takes no outbox lock",
        "hosts",
        FEED,
        "        if !handed.is_empty() {\n"
        "            crate::hosts::ordered(c, self.outbox).map_err(pg)?;\n"
        "        }\n",
        "",
    ),
    (
        "a commit through the store takes no outbox lock",
        "hosts",
        STORE,
        "        if !handed.is_empty() {\n"
        "            crate::hosts::ordered(c, self.outbox).map_err(pg)?;\n"
        "            Counted::ran(1);\n"
        "        }\n",
        "",
    ),
    (
        "a commit through the feed takes its rows' ids before the outbox's lock",
        "hosts",
        FEED,
        "        if !handed.is_empty() {\n"
        "            crate::hosts::ordered(c, self.outbox).map_err(pg)?;\n"
        "        }\n"
        "        let mut ids: Vec<i64> = Vec::new();\n"
        "        for (kind, name, values) in handed {\n"
        "            let args = json_of(name, &values)?;\n"
        "            let row = c\n"
        "                .query_one(\n"
        '                    "INSERT INTO outbox (kind, name, args, host) \\\n'
        '                     VALUES ($1, $2, $3::text::jsonb, $4) RETURNING id",\n'
        "                    &[&kind, name, &args, &self.host],\n"
        "                )\n"
        "                .map_err(pg)?;\n"
        "            ids.push(row.get(0));\n"
        "        }\n",
        "        let mut ids: Vec<i64> = Vec::new();\n"
        "        for (kind, name, values) in handed {\n"
        "            let args = json_of(name, &values)?;\n"
        "            let row = c\n"
        "                .query_one(\n"
        '                    "INSERT INTO outbox (kind, name, args, host) \\\n'
        '                     VALUES ($1, $2, $3::text::jsonb, $4) RETURNING id",\n'
        "                    &[&kind, name, &args, &self.host],\n"
        "                )\n"
        "                .map_err(pg)?;\n"
        "            ids.push(row.get(0));\n"
        "        }\n"
        "        if !ids.is_empty() {\n"
        "            crate::hosts::ordered(c, self.outbox).map_err(pg)?;\n"
        "        }\n",
    ),
    (
        "a commit through the store takes its rows' ids before the outbox's lock",
        "hosts",
        STORE,
        "        if !handed.is_empty() {\n"
        "            crate::hosts::ordered(c, self.outbox).map_err(pg)?;\n"
        "            Counted::ran(1);\n"
        "        }\n"
        "        let mut ids: Vec<i64> = Vec::new();\n"
        "        for (kind, name, values) in handed {\n"
        "            let args = json_of(name, &values)?;\n"
        "            let row = Counted(c)\n"
        "                .query_one(\n"
        '                    "INSERT INTO outbox (kind, name, args, host) \\\n'
        '                     VALUES ($1, $2, $3::text::jsonb, $4) RETURNING id",\n'
        "                    &[&kind, name, &args, &self.host],\n"
        "                )\n"
        "                .map_err(pg)?;\n"
        "            ids.push(row.get(0));\n"
        "        }\n",
        "        let mut ids: Vec<i64> = Vec::new();\n"
        "        for (kind, name, values) in handed {\n"
        "            let args = json_of(name, &values)?;\n"
        "            let row = Counted(c)\n"
        "                .query_one(\n"
        '                    "INSERT INTO outbox (kind, name, args, host) \\\n'
        '                     VALUES ($1, $2, $3::text::jsonb, $4) RETURNING id",\n'
        "                    &[&kind, name, &args, &self.host],\n"
        "                )\n"
        "                .map_err(pg)?;\n"
        "            ids.push(row.get(0));\n"
        "        }\n"
        "        if !ids.is_empty() {\n"
        "            crate::hosts::ordered(c, self.outbox).map_err(pg)?;\n"
        "            Counted::ran(1);\n"
        "        }\n",
    ),
    (
        "the outbox's lock is no lock",
        "hosts",
        HOSTS,
        "        \"SELECT pg_advisory_xact_lock(hashtext($1)), pg_notify($2, '')\",\n",
        "        \"SELECT hashtext($1), pg_notify($2, '')\",\n",
    ),
    (
        "a commit wakes no host",
        "hosts",
        HOSTS,
        "        \"SELECT pg_advisory_xact_lock(hashtext($1)), pg_notify($2, '')\",\n",
        "        \"SELECT pg_advisory_xact_lock(hashtext($1)), $2::text\",\n",
    ),
    # --- The outbox a log every host reads.
    (
        "opening the feed deletes its outbox",
        "hosts",
        FEED,
        "        let highest = crate::hosts::highest(&mut c)?;\n",
        '        c.batch_execute("DELETE FROM outbox").map_err(pg)?;\n'
        "        let highest = crate::hosts::highest(&mut c)?;\n",
    ),
    (
        "opening the store deletes its outbox",
        "hosts",
        STORE,
        "        let highest = crate::hosts::highest(c.client())?;\n",
        '        c.client().batch_execute("DELETE FROM outbox").map_err(pg)?;\n'
        "        let highest = crate::hosts::highest(c.client())?;\n",
    ),
    (
        "a host's reader tells its own host's commits",
        "hosts",
        HOSTS,
        "            if host == self.host || id <= trimmed {\n",
        "            if id <= trimmed {\n",
    ),
    (
        "a host's reader passes the other hosts' commits by",
        "hosts",
        HOSTS,
        "            if host == self.host || id <= trimmed {\n",
        "            if host != self.host || id <= trimmed {\n",
    ),
    (
        "a reader behind the trim is not told it missed rows",
        "hosts",
        HOSTS,
        "            missed: trimmed > cursor,\n",
        "            missed: false,\n",
    ),
    (
        "the trim's mark is read before the rows",
        "hosts",
        HOSTS,
        "            let rows = c.query(\n"
        '                "SELECT id, host, committed_in::text, kind, name, args::text FROM outbox \\\n'
        '                 WHERE id > $1 ORDER BY id LIMIT $2",\n'
        "                &[&cursor, &BATCH],\n"
        "            )?;\n"
        "            #[cfg(test)]\n"
        "            self.between_reads();\n"
        "            let trimmed: i64 = c\n"
        '                .query_one("SELECT through FROM outbox_trimmed", &[])?\n'
        "                .get(0);\n",
        "            let trimmed: i64 = c\n"
        '                .query_one("SELECT through FROM outbox_trimmed", &[])?\n'
        "                .get(0);\n"
        "            #[cfg(test)]\n"
        "            self.between_reads();\n"
        "            let rows = c.query(\n"
        '                "SELECT id, host, committed_in::text, kind, name, args::text FROM outbox \\\n'
        '                 WHERE id > $1 ORDER BY id LIMIT $2",\n'
        "                &[&cursor, &BATCH],\n"
        "            )?;\n",
    ),
    (
        "a trim records no mark",
        "hosts",
        HOSTS,
        "UPDATE outbox_trimmed SET through = greatest(through, (SELECT max(id) FROM gone))",
        "UPDATE outbox_trimmed SET through = through",
    ),
    (
        "a reader's cursor never moves",
        "hosts",
        HOSTS,
        "        let mut cursor = self.cursor.lock().expect(\"cursor\");\n"
        "        *cursor = (*cursor).max(id);\n",
        "        let _ = id;\n",
    ),
    (
        "a host opened reads the outbox from its start",
        "hosts",
        HOSTS,
        "            cursor: Mutex::new(highest),\n",
        "            cursor: Mutex::new(0 * highest),\n",
    ),
    (
        "a layer listens as it opens",
        "hosts",
        HOSTS,
        "            conn: Mutex::new(None),\n",
        "            conn: Mutex::new(Client::connect(url, NoTls).ok()),\n",
    ),
    (
        "a dropped listening connection is never made again",
        "hosts",
        HOSTS,
        "        if conn.as_ref().is_none_or(Client::is_closed) {\n"
        "            let mut c = Client::connect(&self.url, NoTls).map_err(pg)?;\n"
        "            c.batch_execute(&format!(\n"
        '                "SET search_path TO {}; LISTEN {}",\n'
        "                ident(&self.outbox.schema),\n"
        "                ident(&self.outbox.channel)\n"
        "            ))\n"
        "            .map_err(pg)?;\n"
        "            *conn = Some(c);\n"
        "        }\n"
        "        let c = conn.as_mut().expect(\"connected\");\n"
        "        let out = f(c);\n"
        "        if out.is_err() && c.is_closed() {\n"
        "            *conn = None;\n"
        "        }\n",
        "        if conn.is_none() {\n"
        "            let mut c = Client::connect(&self.url, NoTls).map_err(pg)?;\n"
        "            c.batch_execute(&format!(\n"
        '                "SET search_path TO {}; LISTEN {}",\n'
        "                ident(&self.outbox.schema),\n"
        "                ident(&self.outbox.channel)\n"
        "            ))\n"
        "            .map_err(pg)?;\n"
        "            *conn = Some(c);\n"
        "        }\n"
        "        let c = conn.as_mut().expect(\"connected\");\n"
        "        let out = f(c);\n",
    ),
    (
        "the feed's outbox rows carry no host",
        "hosts",
        FEED,
        "                    &[&kind, name, &args, &self.host],\n",
        '                    &[&kind, name, &args, &""],\n',
    ),
    (
        "the store's outbox rows carry no host",
        "store",
        STORE,
        "                    &[&kind, name, &args, &self.host],\n",
        '                    &[&kind, name, &args, &""],\n',
    ),
    (
        "the other hosts' commits are heard and not told",
        "hosts",
        SERVER,
        "            let stale = self.invalidate_queries(\"\", invalidated, events);\n"
        "            if !stale.is_empty() {\n",
        "            let stale = self.invalidate_queries(\"\", invalidated, events);\n"
        "            if false && !stale.is_empty() {\n",
    ),
    (
        "a served host never reads the other hosts' commits",
        "browser",
        SERVER,
        "            std::thread::spawn(move || server.hear_other_hosts());\n",
        "            let _ = server;\n",
    ),
    # --- Sessions kept by hash.
    (
        "a sign-in is not kept where the hosts keep sessions",
        "hosts",
        IDENTITY,
        "            kept.open(&kept_hash(session), &principal)?;\n",
        "",
    ),
    (
        "a sign-out leaves its kept session",
        "hosts",
        IDENTITY,
        "            kept.close(&kept_hash(session))?;\n",
        "",
    ),
    (
        "a request does not read its session",
        "hosts",
        SERVER,
        "    if !fresh {\n"
        "        server.identity.principals().refresh(&session);\n"
        "    }\n",
        "",
    ),
    (
        "a request reads its session twice",
        "hosts",
        SERVER,
        "    if !fresh {\n"
        "        server.identity.principals().refresh(&session);\n"
        "    }\n",
        "    if !fresh {\n"
        "        server.identity.principals().refresh(&session);\n"
        "        server.identity.principals().refresh(&session);\n"
        "    }\n",
    ),
    (
        "a session the database no longer has stays signed in here",
        "hosts",
        IDENTITY,
        "            Ok(None) => self.close(session),\n",
        "            Ok(None) => {}\n",
    ),
    (
        "a sign-in is completed twice",
        "hosts",
        HOSTS,
        '                "DELETE FROM sign_ins WHERE hash = $1 RETURNING nonce, verifier, \\\n'
        '                 (extract(epoch FROM clock_timestamp() - started) * 1000)::bigint",\n',
        '                "SELECT nonce, verifier, \\\n'
        '                 (extract(epoch FROM clock_timestamp() - started) * 1000)::bigint \\\n'
        '                 FROM sign_ins WHERE hash = $1",\n',
    ),
    (
        "a sign-in's age is not the database's",
        "hosts",
        HOSTS,
        "                    std::time::Duration::from_millis(u64::try_from(ms).unwrap_or(0)),\n",
        "                    std::time::Duration::from_millis(0 * u64::try_from(ms).unwrap_or(0)),\n",
    ),
    (
        "an abandoned sign-in is kept",
        "hosts",
        HOSTS,
        '                "WITH gone AS (DELETE FROM sign_ins \\\n'
        "                 WHERE started < clock_timestamp() - interval '10 minutes') \\\n"
        '                 INSERT INTO sign_ins (hash, nonce, verifier) VALUES ($1, $2, $3)",\n',
        '                "INSERT INTO sign_ins (hash, nonce, verifier) VALUES ($1, $2, $3)",\n',
    ),
    (
        "a session is kept by its id, not its hash",
        "hosts",
        IDENTITY,
        "    sha2::Sha256::digest(secret.as_bytes()).to_vec()\n",
        "    secret.as_bytes().to_vec()\n",
    ),
    (
        "a layer's sessions are read on as many connections as ask",
        "hosts",
        HOSTS,
        "            if idle.1 < SESSION_CONNECTIONS {\n",
        "            if true {\n",
    ),
    # --- A retry across hosts.
    (
        "a press on another host's page is not answered from its record",
        "hosts",
        SERVER,
        "            for owner in owners {\n",
        "            for owner in owners.into_iter().take(0) {\n",
    ),
    (
        "a press on another host's page is looked for under the page's session alone",
        "hosts",
        SERVER,
        "            let mut owners = vec![session];\n"
        "            if session != shown_to {\n"
        "                owners.push(shown_to.as_str());\n"
        "            }\n",
        "            let owners = vec![shown_to.as_str()];\n",
    ),
    # --- The deployment's schema.
    (
        "the deployment's schema is not the one its tables are in",
        "browser",
        SERVER,
        "            match feed_pg::FeedPg::open_with(&url, feed_schema.as_deref(), isolation) {\n",
        "            match feed_pg::FeedPg::open_with(\n"
        "                &url,\n"
        '                feed_schema.as_deref().map(|_| "pw_e2e_hosts_ignored"),\n'
        "                isolation,\n"
        "            ) {\n",
    ),
]

CARGO = ["cargo", "test", "--quiet", "--locked", "-p", "pw-dev-server", "--"]
STORE_TESTS = [
    "tests::store_pg::on_postgres_an_order_keeps_its_lines_and_the_outboxes_hold_only_its_hosts_rows",
    "tests::store_pg::on_postgres_a_cart_change_commits_with_its_event_in_one_transaction",
]
SUITES = {
    "hosts": (CARGO + ["tests::second_host::"], {}),
    "store": (CARGO + STORE_TESTS, {}),
}

# The browser suite: the development server built from the source as it is,
# then the two-host spec in Chromium.
SPIKE = ROOT / "spikes/own-renderer"
BUILD = ["cargo", "build", "--quiet", "--locked", "-p", "pw-dev-server"]
BROWSER = [
    "pnpm", "exec", "playwright", "test", "--config", "playwright.hosts.config.mjs",
    "--project=chromium", "--reporter=line",
]

# How long one command may run. Past the bound it is killed with everything
# it started, and the run counts as failing.
BOUND = 1200


def bounded(cmd, env, cwd=ROOT):
    """(output, returncode), or (output, None) when it ran past the bound."""
    p = subprocess.Popen(cmd, cwd=cwd, env={**os.environ, **env}, start_new_session=True,
                         stdout=subprocess.PIPE, stderr=subprocess.STDOUT, text=True)
    try:
        out, _ = p.communicate(timeout=BOUND)
        return out, p.returncode
    except subprocess.TimeoutExpired:
        os.killpg(p.pid, signal.SIGKILL)
        out, _ = p.communicate()
        return out, None


def cargo_tests(suite):
    """(valid, passed, failed) over the suite's tests."""
    cmd, env = SUITES[suite]
    out, code = bounded(cmd, env)
    result = cargo_result(code, out)
    if not result.valid:
        return False, 0, 0
    return True, result.passed, result.failed


def browser_tests():
    """(valid, passed, failed): the server built, then the spec's run. A
    build that fails, a run past the bound, or one that names no test, or a
    count its exit contradicts, is no result."""
    out, code = bounded(BUILD, {})
    if code != 0:
        return False, 0, 0
    out, code = bounded(BROWSER, {}, cwd=SPIKE)
    if code is None:
        return False, 0, 0
    out = re.sub(r"\x1b\[[0-9;]*[A-Za-z]", "", out)
    passed = sum(int(n) for n in re.findall(r"^\s*(\d+) passed", out, re.M))
    failed = sum(int(n) for n in re.findall(r"^\s*(\d+) failed", out, re.M))
    if not passed + failed or (code == 0) != (failed == 0):
        return False, 0, 0
    return True, passed, failed


def run(suite):
    return browser_tests() if suite == "browser" else cargo_tests(suite)


def main():
    import mutation_baseline  # keeps what each command says, for a red baseline
    watch = mutation_baseline.mutation_bound.start()
    # Stopped from outside, the source is still restored: SIGTERM raises
    # here, and the `finally` below runs.
    signal.signal(signal.SIGTERM, lambda *_: sys.exit(1))
    for needed in ("PW_FEED_DATABASE_URL", "PW_STORE_DATABASE_URL"):
        if not os.environ.get(needed):
            print(f"FAIL: every suite needs {needed}, a throwaway database")
            return 1
    for suite in ("hosts", "store", "browser"):
        valid, passed, failed = run(suite)
        print(f"baseline ({suite}): {passed} passed, {failed} failed", flush=True)
        if not valid or failed or not passed or watch.stopped:
            print("FAIL: the unmutated baseline is not green; no mutant can mean anything")
            mutation_baseline.explain()
            return 1

    survivors = 0
    try:
        for what, suite, path, anchor, replacement in MUTANTS:
            original = path.read_text()
            if original.count(anchor) != 1:
                print(f"{what}: ANCHOR NOT FOUND EXACTLY ONCE in {path.name}")
                survivors += 1
                continue
            try:
                path.write_text(original.replace(anchor, replacement, 1))
                valid, passed, failed = run(suite)
            finally:
                path.write_text(original)
            if not valid or watch.stopped:
                print(f"{what} [{suite}]: REFUSED (invalid or stopped producer; no kill)", flush=True)
                mutation_baseline.explain()
                return 1
            if failed:
                verdict = f"KILLED ({failed} of {passed + failed} tests fail)"
            else:
                verdict = "SURVIVED"
                survivors += 1
            print(f"{what} [{suite}]: {verdict}", flush=True)
    finally:
        # The development server built again from the restored source, so
        # what the browser suite runs next is not a mutant's.
        bounded(BUILD, {})

    print(f"{len(MUTANTS) - survivors} of {len(MUTANTS)} mutants killed")
    return 1 if survivors else 0


if __name__ == "__main__":
    sys.exit(main())
