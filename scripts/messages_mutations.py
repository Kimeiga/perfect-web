#!/usr/bin/env python3
"""Mutation controls for track `messages` (ADR-XXXX).

Each mutant undoes one piece of direct messages: a conversation read from
one side, its reader's alone; who may message whom (X's rule, `requires
MayMessage(to)`); no one messaging themselves; a message reaching both
users' sessions and no third user's; sending marking the sender's side
read; reading as a command that reaches each of the reader's sessions; the
conversation, the list and the count cached private; a message writing no
notification; and a message shown before the server answers waiting, by
ADR-0275's rule, whose mutants in `waiting_rows_mutations.py` reach it too.
The tests in
`spikes/own-renderer/server/src/tests/messages.rs` and `messages.rs`, or the
browser suite `e2e/messages.spec.mjs`, must then fail.

The PostgreSQL mutants run where `PW_FEED_DATABASE_URL` names a database,
and are reported as not run otherwise.

Run from the repository root; `just e14-messages` records the output. The
source is restored after every mutant, whatever happens.
"""

import os
import pathlib
import re
import subprocess
import sys

ROOT = pathlib.Path(__file__).resolve().parent.parent
MESSAGES = ROOT / "spikes/own-renderer/server/src/messages.rs"
FEED = ROOT / "spikes/own-renderer/server/src/feed.rs"
IDENTITY = ROOT / "spikes/own-renderer/server/src/identity.rs"
APP = ROOT / "examples/feed/app.pw"

SERVER = "server"
POSTGRES = "postgres"
BROWSER = "browser"
PROGRAM = "program in the browser"

# (what is undone, which tests, file, anchor, replacement)
MUTANTS = [
    (
        "a third user sees a conversation: its rows are anyone's with either user",
        SERVER,
        MESSAGES,
        "            .filter(move |m| (m.from == a && m.to == b) || (m.from == b && m.to == a))\n",
        "            .filter(move |m| m.from == b || m.to == b)\n",
    ),
    (
        "another user's conversations are listed",
        SERVER,
        MESSAGES,
        "            .filter(|m| m.from == reader || m.to == reader)\n",
        "            .filter(|m| !m.from.is_empty())\n",
    ),
    (
        "another user's messages are counted unread",
        SERVER,
        MESSAGES,
        "        m.to == reader && m.seq > self.mark(reader, &m.from)\n",
        "        m.seq > self.mark(reader, &m.from)\n",
    ),
    (
        "the conversation is cached shared, every reader's",
        SERVER,
        APP,
        "private query Conversation(reader: capability.User<UserId>, with: UserId, limit: Int) -> "
        "Result<Conversation, FeedError>\n"
        "    freshness      0.seconds\n"
        "    consistency    read_your_writes\n"
        "    cache          private\n",
        "private query Conversation(reader: capability.User<UserId>, with: UserId, limit: Int) -> "
        "Result<Conversation, FeedError>\n"
        "    freshness      0.seconds\n"
        "    consistency    read_your_writes\n"
        "    cache          shared\n",
    ),
    (
        "MayMessage inverted",
        SERVER,
        IDENTITY,
        '            "MayMessage" => crate::messages::may_message(self.principals.of(session), bound, host),\n',
        '            "MayMessage" => crate::messages::may_message(self.principals.of(session), bound, host)\n'
        "                .map(|held| !held),\n",
    ),
    (
        "MayMessage skipped: anyone messages anyone",
        SERVER,
        IDENTITY,
        '            "MayMessage" => crate::messages::may_message(self.principals.of(session), bound, host),\n',
        '            "MayMessage" => crate::messages::may_message(self.principals.of(session), bound, host)\n'
        "                .map(|_| true),\n",
    ),
    (
        "being followed is not leave: following someone is",
        SERVER,
        MESSAGES,
        "        && (follows.contains(&(to.to_string(), from.to_string()))\n",
        "        && (follows.contains(&(from.to_string(), to.to_string()))\n",
    ),
    (
        "messaging yourself allowed",
        SERVER,
        MESSAGES,
        "    from != to\n        && (follows.contains(",
        "    from == to\n        || (follows.contains(",
    ),
    (
        "the recipient's sessions are not told: the count listens for what the reader sends alone",
        SERVER,
        APP,
        "private query UnreadMessages(reader: capability.User<UserId>) -> Int\n"
        "    freshness      0.seconds\n"
        "    consistency    read_your_writes\n"
        "    cache          private\n"
        "    key            reader\n"
        "    invalidates_on Messaged(reader, _), Messaged(_, reader)\n",
        "private query UnreadMessages(reader: capability.User<UserId>) -> Int\n"
        "    freshness      0.seconds\n"
        "    consistency    read_your_writes\n"
        "    cache          private\n"
        "    key            reader\n"
        "    invalidates_on Messaged(reader, _)\n",
    ),
    (
        "the recipient's open conversation is not told",
        SERVER,
        APP,
        "    invalidates_on Messaged(reader, with), Messaged(with, reader), Followed(with, reader),\n",
        "    invalidates_on Messaged(reader, with), Followed(with, reader),\n",
    ),
    (
        "a follow does not open the composer: the conversation listens for no follow",
        SERVER,
        APP,
        "    invalidates_on Messaged(reader, with), Messaged(with, reader), Followed(with, reader),\n"
        "        Unfollowed(with, reader)\n",
        "    invalidates_on Messaged(reader, with), Messaged(with, reader)\n",
    ),
    (
        "sending does not mark the sender's side read",
        SERVER,
        MESSAGES,
        "        self.marks\n            .insert((sent.from.clone(), sent.to.clone()), sent.seq);\n",
        "",
    ),
    (
        "reading a conversation reads nothing",
        SERVER,
        MESSAGES,
        "            *mark = (*mark).max(last);\n",
        "            *mark = *mark;\n            let _ = last;\n",
    ),
    (
        "reading reaches no other session: it invalidates nothing",
        SERVER,
        APP,
        "    invalidates   Conversation(current_user(), with, _), Conversations(current_user()),\n"
        "        UnreadMessages(current_user())\n",
        "",
    ),
    (
        "a message writes a notification",
        SERVER,
        FEED,
        "                    staged.push(Change::Message(sent));\n",
        "                    if let Some(note) = crate::notifications::noted(\n"
        "                        to,\n"
        "                        &from,\n"
        "                        crate::notifications::Act::Followed,\n"
        "                        None,\n"
        "                    ) {\n"
        "                        staged.push(Change::Notify(note));\n"
        "                    }\n"
        "                    staged.push(Change::Message(sent));\n",
    ),
    (
        "on PostgreSQL, following someone is leave to message them",
        POSTGRES,
        MESSAGES,
        "         EXISTS (SELECT 1 FROM follows WHERE follower = $2 AND followee = $1) \\\n",
        "         EXISTS (SELECT 1 FROM follows WHERE follower = $1 AND followee = $2) \\\n",
    ),
    (
        "on PostgreSQL, sending does not mark the sender's side read",
        POSTGRES,
        MESSAGES,
        "                             SELECT $1, $2, seq FROM sent \\\n",
        "                             SELECT $1, $2, 0 FROM sent \\\n",
    ),
    (
        "on PostgreSQL, a third user sees a conversation",
        POSTGRES,
        MESSAGES,
        "                         FROM (SELECT * FROM messages \\\n"
        "                               WHERE (sender = $1 AND recipient = $2) \\\n"
        "                                  OR (sender = $2 AND recipient = $1) \\\n",
        "                         FROM (SELECT * FROM messages \\\n"
        "                               WHERE sender = $2 \\\n"
        "                                  OR recipient = $2 \\\n",
    ),
    (
        "in the browser, a third user sees a conversation",
        BROWSER,
        MESSAGES,
        "            .filter(move |m| (m.from == a && m.to == b) || (m.from == b && m.to == a))\n",
        "            .filter(move |m| m.from == b || m.to == b)\n",
    ),
    (
        "in the browser, the composer is shown where its reader may not send",
        BROWSER,
        MESSAGES,
        '        ("closed".into(), Val::Bool(!may_send)),\n',
        '        ("closed".into(), Val::Bool(may_send && !may_send)),\n',
    ),
    (
        "in the browser, MayMessage skipped",
        BROWSER,
        IDENTITY,
        '            "MayMessage" => crate::messages::may_message(self.principals.of(session), bound, host),\n',
        '            "MayMessage" => crate::messages::may_message(self.principals.of(session), bound, host)\n'
        "                .map(|_| true),\n",
    ),
    (
        "in the browser, a message shown before the server answers does not wait",
        PROGRAM,
        APP,
        "    waits(PostId(id.value))\n",
        "    false\n",
    ),
    # ADR-0275's rule, which `message_waits` delegates to: waiting_rows'
    # mutants of it, as they reach the conversation's pending message.
    (
        "in the browser, no row waits (waiting_rows' mutant)",
        PROGRAM,
        APP,
        '    String.starts_with(id.value, "pending-")\n',
        '    String.starts_with(id.value, "pending-none-")\n',
    ),
    (
        "in the browser, a reply waits, and nothing else (waiting_rows' mutant)",
        PROGRAM,
        APP,
        '    String.starts_with(id.value, "pending-")\n',
        '    String.starts_with(id.value, "pending-reply-")\n',
    ),
    (
        "in the browser, a row the server made waits too (waiting_rows' mutant)",
        PROGRAM,
        APP,
        '    String.starts_with(id.value, "pending-")\n',
        '    String.starts_with(id.value, "")\n',
    ),
]

# The browser's: the server built again, then the suite in three engines on
# hosts of their own ports, away from any other suite's (`PORT`). A mutant of
# the program's source builds the feed again first (`FEED`), and the feed is
# built again as it is once the mutants are done.
BUILD = ["cargo", "build", "--quiet", "--locked", "-p", "pw-dev-server"]
FEED = ["bash", "spikes/own-renderer/feed.sh"]
PLAYWRIGHT = ["pnpm", "exec", "playwright", "test", "e2e/messages.spec.mjs", "--reporter=line"]

TESTS = {
    SERVER: [["cargo", "test", "--quiet", "--locked", "-p", "pw-dev-server", "--", "messages"]],
    BROWSER: [BUILD, PLAYWRIGHT],
    PROGRAM: [FEED, BUILD, PLAYWRIGHT],
    POSTGRES: [
        ["cargo", "test", "--quiet", "--locked", "-p", "pw-dev-server", "--", "tests::messages::on_postgres_"]
    ],
}

DATABASE = os.environ.get("PW_FEED_DATABASE_URL", "")


def run_tests(group):
    """(built, passed, failed) over the group's test commands."""
    built, passed, failed = True, 0, 0
    for cmd in TESTS[group]:
        if cmd is BUILD or cmd is FEED:
            if subprocess.run(cmd, cwd=ROOT, capture_output=cmd is FEED).returncode != 0:
                return False, 0, 0
            continue
        if cmd is PLAYWRIGHT:
            env = {**os.environ, "PORT": os.environ.get("PW_MESSAGES_PORT", "6300")}
            r = subprocess.run(
                cmd, cwd=ROOT / "spikes/own-renderer", capture_output=True, text=True, env=env
            )
            out = re.sub(r"\x1b\[[0-9;]*[A-Za-z]", "", r.stdout + r.stderr)
            ok = re.search(r"^ +(\d+) passed", out, re.M)
            bad = re.search(r"^ +(\d+) failed", out, re.M)
            if not ok and not bad:
                built = False
                continue
            passed += int(ok.group(1)) if ok else 0
            failed += int(bad.group(1)) if bad else 0
            continue
        r = subprocess.run(cmd, cwd=ROOT, capture_output=True, text=True)
        out = r.stdout + r.stderr
        results = re.findall(r"test result: \w+\. (\d+) passed; (\d+) failed", out)
        if not results:
            built = False
            continue
        for p, f in results:
            passed += int(p)
            failed += int(f)
    return built, passed, failed


def main():
    import mutation_baseline  # keeps what each command says, for a red baseline

    groups = [SERVER, BROWSER] + ([POSTGRES] if DATABASE else [])
    for group in groups:
        built, passed, failed = run_tests(group)
        print(f"baseline ({group}): {passed} passed, {failed} failed")
        if not built or failed or not passed:
            print("FAIL: the unmutated baseline is not green; no mutant can mean anything")
            mutation_baseline.explain()
            return 1
    # The program's mutants run the browser's suite, whose baseline is above.
    groups.append(PROGRAM)

    survivors, run = 0, 0
    for what, group, path, anchor, replacement in MUTANTS:
        if group not in groups:
            print(f"{what}: NOT RUN (PW_FEED_DATABASE_URL is not set)")
            continue
        run += 1
        original = path.read_text()
        if original.count(anchor) != 1:
            print(f"{what}: ANCHOR NOT FOUND EXACTLY ONCE in {path.name}")
            survivors += 1
            continue
        try:
            path.write_text(original.replace(anchor, replacement, 1))
            built, passed, failed = run_tests(group)
        finally:
            path.write_text(original)
        if not built:
            verdict = "KILLED (does not build)"
        elif failed:
            verdict = f"KILLED ({failed} of {passed + failed} tests fail)"
        else:
            verdict = "SURVIVED"
            survivors += 1
        print(f"{what}: {verdict}")

    # The feed and the server the browser's mutants built, built again as
    # they are.
    subprocess.run(FEED, cwd=ROOT, capture_output=True)
    subprocess.run(BUILD, cwd=ROOT)
    print(f"{run - survivors} of {run} mutants killed")
    return 1 if survivors else 0


if __name__ == "__main__":
    sys.exit(main())
