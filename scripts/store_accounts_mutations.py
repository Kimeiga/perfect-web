#!/usr/bin/env python3
"""Mutation controls for track store-accounts: a cart and an order a user's,
a guest's cart joining its user's at sign-in, and a record's scope the
scopes of the queries that produce it, joined (ADR-XXXX).

Each mutant undoes one piece:
- a user's cart read and written by the session, not the reader's handle;
- a user's order read for another owner than the reader;
- the store keying a session's cart and order by the session where the
  program reads them by the reader (`Server::owner`);
- a session's cart entry keyed by the session, so the user's event reaches
  it nowhere;
- the guest's lines joined without summing one item's quantities;
- the guest's cart left as it was after joining;
- a sum past a bigint wrapped rather than refused;
- a signed-in session that signs in again joined to the next user;
- the deployment not told of a guest's sign-in;
- on PostgreSQL, a cart read whoever owns it;
- in the compiler, `private` giving a record no scope, and disagreeing
  producers keeping the last one's scope alone;
- `user` not a visibility (the parse refuses `user query`), not lowered as
  the declaration's visibility, and not importable.

The tests then fail: the store's accounts tests (tests/store_accounts.rs)
and the join's unit tests, in memory and with the store on PostgreSQL
(PW_STORE_TEST_LAYER=postgres); identity's sign-in tests; and the
compiler's boundary tests (boundary_matrix.rs, boundary.rs's own).

They need a database: `PW_STORE_DATABASE_URL`, a throwaway one, which each
test uses in a schema of its own. Without it every PostgreSQL run would read
the in-memory layer and no PostgreSQL mutant could mean anything, so this
refuses to run.

Run from the repository root; `just e14-store-accounts` records the output.
The source is restored after every mutant, whatever happens.
"""

import os
import pathlib
import re
import signal
import subprocess
import sys

ROOT = pathlib.Path(__file__).resolve().parent.parent
SERVER = ROOT / "spikes/own-renderer/server/src/main.rs"
STORE = ROOT / "spikes/own-renderer/server/src/store.rs"
LAYER = ROOT / "spikes/own-renderer/server/src/store_pg.rs"
IDENTITY = ROOT / "spikes/own-renderer/server/src/identity.rs"
BOUNDARY = ROOT / "compiler/pw-core/src/boundary.rs"
GRAMMAR = ROOT / "compiler/pw-syntax/src/grammar.rs"
RESOLVE = ROOT / "compiler/pw-core/src/resolve.rs"
LOWER = ROOT / "compiler/pw-core/src/lower.rs"

# (what is undone, file, anchor, replacement)
MUTANTS = [
    (
        "a user's cart is read and written by the session",
        STORE,
        "                    let (owner, rest) = if by_reader {\n"
        "                        reader_of(&op, args)?\n"
        "                    } else {\n"
        "                        (this.as_str(), own_session(&op, &this, args)?)\n"
        "                    };\n"
        "                    if fail {\n",
        "                    let (owner, rest) = if by_reader {\n"
        "                        (this.as_str(), reader_of(&op, args)?.1)\n"
        "                    } else {\n"
        "                        (this.as_str(), own_session(&op, &this, args)?)\n"
        "                    };\n"
        "                    if fail {\n",
    ),
    (
        "a user's order is read for another owner than the reader",
        STORE,
        "                let owner = if by_reader {\n"
        "                    reader_of(&op, args)?.0\n"
        "                } else {\n"
        "                    own_session(&op, &this, args)?;\n"
        "                    this.as_str()\n"
        "                };\n"
        "                w(&mut |r| {\n"
        "                    let status = r\n",
        "                let owner = if by_reader {\n"
        "                    reader_of(&op, args)?;\n"
        "                    \"ada\"\n"
        "                } else {\n"
        "                    own_session(&op, &this, args)?;\n"
        "                    this.as_str()\n"
        "                };\n"
        "                w(&mut |r| {\n"
        "                    let status = r\n",
    ),
    (
        "the store keys a session's cart by the session where the program reads it by the reader",
        SERVER,
        "            Some(true) => notifications::user_of(&self.identity.principals(), session),\n",
        "            Some(true) => session.to_string(),\n",
    ),
    (
        "a session's cart entry is keyed by the session",
        SERVER,
        "        \"store.page.Cart\",\n        &[owner],\n",
        "        \"store.page.Cart\",\n        &[session],\n",
    ),
    (
        "a guest's lines are joined without summing one item's quantities",
        STORE,
        "                held.quantity = held.quantity.checked_add(line.quantity).ok_or_else(|| {\n",
        "                held.quantity = held.quantity.checked_add(0).ok_or_else(|| {\n",
    ),
    (
        "the guest's cart is left as it was after joining",
        STORE,
        "            r.set_cart(user, &lines)?;\n            r.set_cart(guest, &[])?;\n",
        "            r.set_cart(user, &lines)?;\n",
    ),
    (
        "a sum past a bigint is wrapped rather than refused",
        STORE,
        "                held.quantity = held.quantity.checked_add(line.quantity).ok_or_else(|| {\n",
        "                held.quantity = Some(held.quantity.wrapping_add(line.quantity)).ok_or_else(|| {\n",
    ),
    (
        "a signed-in session that signs in again is joined to the next user",
        IDENTITY,
        "        let was_guest = self.principals.of(session).is_none_or(|p| p.is_guest());\n",
        "        let was_guest = true;\n",
    ),
    (
        "the deployment is not told of a guest's sign-in",
        IDENTITY,
        "        if was_guest && !session.is_empty() {\n            on_sign_in(",
        "        if was_guest && session.is_empty() {\n            on_sign_in(",
    ),
    (
        "on PostgreSQL, a cart is read whoever owns it",
        LAYER,
        "                \"SELECT item, name, quantity, price FROM cart_lines \\\n"
        "                 WHERE owner = $1 ORDER BY position\",\n",
        "                \"SELECT item, name, quantity, price FROM cart_lines \\\n"
        "                 WHERE owner = $1 OR true ORDER BY position\",\n",
    ),
    (
        "in the compiler, `private` gives a record no scope",
        BOUNDARY,
        '                    Some("user") | Some("private") => Restriction::User("UserId".into()),\n',
        '                    Some("user") => Restriction::User("UserId".into()),\n',
    ),
    (
        "in the compiler, disagreeing producers keep the last one's scope",
        BOUNDARY,
        "                let held = f.scoped.entry(produced.semantic_key()).or_default();\n"
        "                *held = held.join(&Label::of(restriction));\n",
        "                f.scoped.insert(produced.semantic_key(), Label::of(restriction));\n",
    ),
    (
        "`user` is not a visibility",
        GRAMMAR,
        '        if !self.at_kw("user") {\n            return false;\n        }\n',
        '        if !self.at_kw("user") || true {\n            return false;\n        }\n',
    ),
    (
        "`user` is not lowered as the declaration's visibility",
        LOWER,
        '    matches!(first.as_str(), "public" | "session" | "private" | "user").then_some(first)\n',
        '    matches!(first.as_str(), "public" | "session" | "private").then_some(first)\n',
    ),
    (
        "`user` is not importable",
        RESOLVE,
        '                    if decl.is_some_and(|d| d.visibility.as_deref() == Some("private")) {\n',
        '                    if decl.is_some_and(|d| matches!(d.visibility.as_deref(), Some("private") | Some("user"))) {\n',
    ),
]

# The store's accounts tests, and the join's own, which run on either layer.
STORE_TESTS = ["tests::store_accounts::", "store::joins::"]

CARGO = ["cargo", "test", "--quiet", "--locked", "-p", "pw-dev-server", "--"]

# (what runs, the environment it adds)
TESTS = [
    (CARGO + STORE_TESTS + ["tests::sign_in::"], {"PW_STORE_TEST_LAYER": "memory"}),
    (CARGO + STORE_TESTS, {"PW_STORE_TEST_LAYER": "postgres"}),
    (
        ["cargo", "test", "--quiet", "--locked", "-p", "pw-core", "--test", "boundary_matrix"],
        {},
    ),
    (["cargo", "test", "--quiet", "--locked", "-p", "pw-core", "--lib", "boundary"], {}),
    (
        ["cargo", "test", "--quiet", "--locked", "-p", "pw-core", "--test", "user_visibility"],
        {},
    ),
]

# How long one command may run. Past the bound it is killed with everything
# it started, and the run counts as failing.
BOUND = 1200


def run_tests():
    """(built, passed, failed) over every test command."""
    built, passed, failed = True, 0, 0
    for cmd, extra in TESTS:
        env = dict(os.environ, **extra)
        p = subprocess.Popen(cmd, cwd=ROOT, env=env, start_new_session=True,
                             stdout=subprocess.PIPE, stderr=subprocess.STDOUT, text=True)
        try:
            out, _ = p.communicate(timeout=BOUND)
        except subprocess.TimeoutExpired:
            os.killpg(p.pid, signal.SIGKILL)
            p.communicate()
            failed += 1
            continue
        found = re.findall(r"test result: \w+\. (\d+) passed; (\d+) failed", out)
        if not found:
            built = False
            continue
        for p_, f in found:
            passed += int(p_)
            failed += int(f)
    return built, passed, failed


def main():
    import mutation_baseline  # keeps what each command says, for a red baseline
    # Stopped from outside, the source is still restored: SIGTERM raises
    # here, and the `finally` below runs.
    signal.signal(signal.SIGTERM, lambda *_: sys.exit(1))
    if not os.environ.get("PW_STORE_DATABASE_URL"):
        print("FAIL: PW_STORE_DATABASE_URL is not set; without a database the PostgreSQL")
        print("runs read the in-memory layer, and no PostgreSQL mutant can mean anything")
        return 2
    built, passed, failed = run_tests()
    print(f"baseline: {passed} passed, {failed} failed", flush=True)
    if not built or failed or not passed:
        print("FAIL: the unmutated baseline is not green; no mutant can mean anything")
        mutation_baseline.explain()
        return 1

    survivors = 0
    for what, path, anchor, replacement in MUTANTS:
        original = path.read_text()
        if original.count(anchor) != 1:
            print(f"{what}: ANCHOR NOT FOUND EXACTLY ONCE in {path.name}", flush=True)
            survivors += 1
            continue
        try:
            path.write_text(original.replace(anchor, replacement, 1))
            built, passed, failed = run_tests()
        finally:
            path.write_text(original)
        if not built:
            verdict = "KILLED (does not build)"
        elif failed:
            verdict = f"KILLED ({failed} of {passed + failed} tests fail)"
        else:
            verdict = "SURVIVED"
            survivors += 1
        print(f"{what}: {verdict}", flush=True)

    print(f"{len(MUTANTS) - survivors} of {len(MUTANTS)} mutants killed")
    return 1 if survivors else 0


if __name__ == "__main__":
    sys.exit(main())
