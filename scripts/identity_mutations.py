#!/usr/bin/env python3
"""Mutation controls for the identity track (ADR-XXXX): accounts and sign-in.

Each mutant undoes one guarantee the track claims: a command from another
origin refused (CSRF), the session rotated at sign-in and forgotten at
sign-out, the sign-in bound to the browser that started it, the ID token's
nonce, PKCE's verifier, `SignedIn` and `OwnsPost` evaluated, a refusal
answered as its own case, cookies HttpOnly, session ids unguessable, a
password kept only as its hash, the development identities refused outside
development, and a post attributed to its signed-in author. The tests of
each must then fail.

The browser suite's `e2e/identity.spec.mjs` runs the flows in three engines.

Run from the repository root; `just e14-identity` records the output.
The source is restored after every mutant, whatever happens.
"""

import pathlib
import re
import subprocess
import sys

ROOT = pathlib.Path(__file__).resolve().parent.parent
SRC = ROOT / "spikes/own-renderer/server/src"
IDENTITY = SRC / "identity.rs"
ACCOUNTS = SRC / "accounts.rs"
SERVER = SRC / "main.rs"
FEED = SRC / "feed.rs"

# (what is undone, file, anchor, replacement)
MUTANTS = [
    (
        "a command from another origin runs (no CSRF check)",
        IDENTITY,
        "        if let Err(why) = same_origin(method, headers) {\n",
        "        if let Err(why) = same_origin(\"GET\", headers) {\n",
    ),
    (
        "a sign-in keeps the session the browser came with (fixation)",
        IDENTITY,
        "        let rotated = new_session_id();\n"
        "        self.principals.close(session);\n"
        "        self.principals.open(&rotated, principal);\n",
        "        let rotated = session.to_string();\n"
        "        self.principals.open(&rotated, principal);\n",
    ),
    (
        "a sign-in leaves the session it replaced signed in",
        IDENTITY,
        "        let rotated = new_session_id();\n"
        "        self.principals.close(session);\n"
        "        self.principals.open(&rotated, principal);\n",
        "        let rotated = new_session_id();\n"
        "        self.principals.open(&rotated, principal);\n",
    ),
    (
        "signing out leaves the session signed in",
        IDENTITY,
        "    fn sign_out(&self, session: &str) -> Reply {\n"
        "        self.principals.close(session);\n",
        "    fn sign_out(&self, session: &str) -> Reply {\n",
    ),
    (
        "a callback is not bound to the browser that started it (login CSRF)",
        IDENTITY,
        "        if state.is_empty() || !same(&state, &bound) {\n",
        "        if state.is_empty() {\n",
    ),
    (
        "an ID token's nonce is not checked",
        IDENTITY,
        "        Some(n) if same(n, nonce) => {}\n",
        "        Some(_) | None => {}\n",
    ),
    (
        "every session is SignedIn",
        IDENTITY,
        '            "SignedIn" => Ok(self.principals.of(session).is_some()),\n',
        '            "SignedIn" => Ok(!session.is_empty()),\n',
    ),
    (
        "the ownership predicate is inverted",
        IDENTITY,
        "                    [Val::Result(Ok(Some(found)))] => Ok(author_of(found) == Some(&principal.user)),\n",
        "                    [Val::Result(Ok(Some(found)))] => Ok(author_of(found) != Some(&principal.user)),\n",
    ),
    (
        "a refusal is answered as a command that did not commit",
        SERVER,
        "            if let Some(predicate) = identity::take_refusal() {\n",
        "            if let Some(predicate) = None::<String> {\n",
    ),
    (
        "the session cookie is readable by scripts",
        IDENTITY,
        '    let mut line = format!("set-cookie: {name}={value}; Path={path}; HttpOnly; SameSite=Lax");\n',
        '    let mut line = format!("set-cookie: {name}={value}; Path={path}; SameSite=Lax");\n',
    ),
    (
        "a fresh session's id is guessable",
        IDENTITY,
        '    format!("s-{}", hex(&random_bytes::<16>()))\n',
        '    format!("s-{:032x}", std::process::id())\n',
    ),
    (
        "the guest model runs in production",
        IDENTITY,
        "                if !deployment.development || !deployment.loopback() {\n",
        "                if false {\n",
    ),
    (
        "the development provider starts in production",
        ACCOUNTS,
        "        if !deployment.development {\n",
        "        if false {\n",
    ),
    (
        "the development provider exchanges a code without its verifier",
        ACCOUNTS,
        "        if !is_verifier(verifier) || !same(&s256(verifier), &grant.challenge) {\n",
        "        if !is_verifier(verifier) {\n",
    ),
    (
        "a password is stored as it was typed",
        ACCOUNTS,
        "        let hash = hash_password(password)?;\n",
        "        let hash = password.to_string();\n",
    ),
    (
        "a post is its session's guest's, not its signed-in author's",
        FEED,
        "    match principals.of(session) {\n"
        "        Some(p) => {\n",
        "    match principals.of(session).filter(|_| false) {\n"
        "        Some(p) => {\n",
    ),
]

TESTS = [
    [
        "cargo", "test", "--quiet", "--locked", "-p", "pw-dev-server", "--",
        "identity::", "accounts::", "sign_in::",
    ],
]


def run_tests():
    """(built, passed, failed) over every test command."""
    built, passed, failed = True, 0, 0
    for cmd in TESTS:
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
    built, passed, failed = run_tests()
    print(f"baseline: {passed} passed, {failed} failed")
    if not built or failed or not passed:
        print("FAIL: the unmutated baseline is not green; no mutant can mean anything")
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
        print(f"{what}: {verdict}")

    print(f"{len(MUTANTS) - survivors} of {len(MUTANTS)} mutants killed")
    return 1 if survivors else 0


if __name__ == "__main__":
    sys.exit(main())
