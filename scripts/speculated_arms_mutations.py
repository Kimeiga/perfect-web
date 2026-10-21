#!/usr/bin/env python3
"""Mutation controls for ADR-0238: a command speculates on several entries,
a page on those it shows.

Each mutant undoes one piece: the arms parsed as nodes of their own, each
lowered, each transition paired with its own target and its name typed as
that target's value; an arm whose target a page does not show left to the
server, and a page that shows none given no module; and the feed's like on
the thread page.

The compiler's tests must fail for each ("cargo"); the feed's, in three
engines, for the feed's ("browser").

Run from the repository root; `just e14-speculated-arms` records the output.
The source is restored after every mutant, whatever happens.
"""

import os
import json
import secrets
import tempfile
import pathlib
import signal
import subprocess
import sys

import browser_test_results
import cargo_assertion_results
import mutation_source
import producer_process

ROOT = pathlib.Path(__file__).resolve().parent.parent
SPECULATION = ROOT / "compiler/pw-core/src/backend/speculation.rs"
LOWER = ROOT / "compiler/pw-core/src/lower.rs"
HIR = ROOT / "compiler/pw-core/src/hir.rs"
VALUES = ROOT / "compiler/pw-core/src/values.rs"
GRAMMAR = ROOT / "compiler/pw-syntax/src/grammar.rs"
FEED = ROOT / "examples/feed/app.pw"

# (what is undone, suite, file, anchor, replacement)
MUTANTS = [
    (
        "an arm whose target the page does not show refuses the page",
        "cargo",
        SPECULATION,
        "                if !bindings.iter().any(|b| b.resource == resource) {\n"
        "                    continue;\n"
        "                }\n",
        "",
    ),
    (
        "a page that speculates on nothing has a module",
        "cargo",
        SPECULATION,
        "            if let Encoding::Encoded(m) = &module\n"
        "                && m.commands.is_empty()\n"
        "            {\n"
        "                continue;\n"
        "            }\n",
        "",
    ),
    (
        "the arms are no nodes of their own",
        "cargo",
        GRAMMAR,
        "        self.start(K::TransitionArm);\n",
        "        self.start(K::ErrorExpr);\n",
    ),
    (
        "an arm with no transition after its arrow takes the comma after it",
        "cargo",
        GRAMMAR,
        "        if !self.at_eof() && !self.at(Kind::Comma) {\n",
        "        if !self.at_eof() {\n",
    ),
    (
        "only a clause's first arm is lowered",
        "cargo",
        LOWER,
        "                .filter(|c| c.kind() == K::TransitionArm)\n",
        "                .filter(|c| c.kind() == K::TransitionArm)\n"
        "                .take(1)\n",
    ),
    (
        "each transition is paired with the clause's first target",
        "cargo",
        HIR,
        "                        if let Some(t) = target.take() {\n",
        "                        if let Some(t) = p.roots.first().filter(|_| target.take().is_some()) {\n",
    ),
    (
        "an arm's name is the clause's last target's value",
        "cargo",
        VALUES,
        "                    .take_while(|r| !std::ptr::eq(*r, root))\n",
        "                    .take_while(|_| true)\n",
    ),
    (
        "the feed's like on the thread page is not speculated",
        "browser",
        FEED,
        # Re-anchored by ADR-0276: the feed formatted, its continued value
        # one level in from its clause.
        "    optimistic    Timeline(current_session(), _) as feed => liked(feed, post),\n"
        "        FollowingTimeline(current_session(), _) as feed => liked(feed, post),\n"
        "        Thread(post) as thread => liked_thread(thread, post)\n",
        "    optimistic    Timeline(current_session(), _) as feed => liked(feed, post),\n"
        "        FollowingTimeline(current_session(), _) as feed => liked(feed, post)\n",
    ),
]

CARGO = {
    "core": ["cargo", "test", "--quiet", "--locked", "-p", "pw-core",
             "--test", "speculated_arms", "--", "--format=pretty"],
    "syntax": ["cargo", "test", "--quiet", "--locked", "-p", "pw-syntax", "--lib", "--",
               "an_optimistic_clause_has_an_arm_for_each_entry", "--format=pretty"],
}
NAMES = {
    "core": {"each_page_speculates_on_the_arm_whose_target_it_shows",
             "an_arm_whose_target_a_page_shows_under_another_key_is_refused",
             "a_page_that_shows_no_target_has_no_speculation",
             "each_arms_name_is_its_own_targets_value", "arms_are_separated_by_commas"},
    "syntax": {"grammar::tests::an_optimistic_clause_has_an_arm_for_each_entry"},
}
SOURCES = {"core": ROOT / "compiler/pw-core/tests/speculated_arms.rs", "syntax": GRAMMAR}
SPEC = ["e2e/feed.spec.mjs"]
BOUND = 1200
INVENTORIES = {}


def bounded(cmd, **kw):
    """Shared native process ownership; timeout is invalid, never a test failure."""
    try:
        result = producer_process.run(cmd, timeout=BOUND, **kw)
        return result.stdout, result.returncode
    except subprocess.TimeoutExpired as expired:
        return expired.output, None


def cargo_result(suite, code, output):
    result = cargo_assertion_results.validate(code, output, root=ROOT,
        sources=[SOURCES[suite]], expected=NAMES[suite])
    return (True, result.passed, result.failed) if result.valid else (False, 0, 0)


def cargo_tests():
    valid, passed, failed = True, 0, 0
    for suite, cmd in CARGO.items():
        out, code = bounded(cmd, cwd=ROOT)
        ok, p, f = cargo_result(suite, code, out)
        if not ok or f:
            print(out, flush=True)
        valid &= ok
        passed += p
        failed += f
    return valid, passed, failed


def build():
    """Build the actual source; refusal cannot earn a mutation kill."""
    for command, env in (
        (["bash", "spikes/own-renderer/run.sh"], {"BUILD_ONLY": "1"}),
        (["bash", "spikes/own-renderer/feed.sh"], {}),
        (["cargo", "build", "--quiet", "--locked", "-p", "pw-dev-server"], {}),
    ):
        out, code = bounded(command, cwd=ROOT, env={**os.environ, **env})
        if code != 0:
            print(out, flush=True)
            return False
    return True


def browser_tests():
    if not build():
        return False, 0, 0
    with tempfile.TemporaryDirectory(prefix="pw-speculated-arms-") as directory:
        receipt = pathlib.Path(directory) / "result.json"
        nonce = secrets.token_hex(32)
        out, code = bounded(
            ["bash", str(ROOT / "scripts/run_suite.sh"), "-C", ".", "--",
             "pnpm", "exec", "playwright", "test", *SPEC, "--retries=0",
             "--reporter=line,./assertion-result-reporter.mjs"],
            cwd=ROOT / "spikes/own-renderer",
            env={**os.environ, "PW_ASSERTION_REPORT": str(receipt), "PW_ASSERTION_NONCE": nonce},
        )
        try:
            report = json.loads(receipt.read_text())
        except (OSError, ValueError):
            report = None
        result = browser_test_results.parse(code, report, nonce, INVENTORIES.get("browser"))
        if not result.valid or result.failed:
            print(out, flush=True)
        if not result.valid:
            print(f"REFUSED browser producer: {result.reason}", flush=True)
            return False, 0, 0
        INVENTORIES.setdefault("browser", result.inventory)
        return True, result.passed, result.failed


SUITES = {"cargo": cargo_tests, "browser": browser_tests}


def main():
    import mutation_baseline  # retain native baseline output before starting producers
    signal.signal(signal.SIGTERM, lambda *_: sys.exit(1))
    watch = mutation_baseline.mutation_bound.start()
    INVENTORIES.clear()
    for suite in sorted({m[1] for m in MUTANTS}):
        valid, passed, failed = SUITES[suite]()
        print(f"baseline ({suite}): {passed} passed, {failed} failed", flush=True)
        if not valid or failed or not passed or watch.stopped:
            print("FAIL: the unmutated baseline is not green; no mutant can mean anything")
            mutation_baseline.explain()
            return 1

    killed = refused = survivors = 0
    for what, suite, path, anchor, replacement in MUTANTS:
        original = path.read_text()
        if original.count(anchor) != 1:
            print(f"{what}: REFUSED (anchor not unique in {path.name})")
            refused += 1
            continue
        with mutation_source.changed(path, original, original.replace(anchor, replacement, 1)):
            valid, passed, failed = SUITES[suite]()
        if not valid or watch.stopped:
            verdict = "REFUSED (invalid or stopped producer; no kill)"
            refused += 1
        elif failed:
            verdict = f"KILLED ({failed} of {passed + failed} tests fail)"
            killed += 1
        else:
            verdict = "SURVIVED"
            survivors += 1
        print(f"{what} [{suite}]: {verdict}", flush=True)
        if watch.stopped:
            print("REFUSED: memory guard stopped the campaign")
            return 1
    if not build() or watch.stopped:
        print("REFUSED: restored source did not build")
        refused += 1
    print(f"{killed} of {len(MUTANTS)} mutants killed; {refused} refused; {survivors} survived")
    return int(bool(refused or survivors))


if __name__ == "__main__":
    sys.exit(main())
