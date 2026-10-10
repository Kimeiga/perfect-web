#!/usr/bin/env python3
"""Mutation controls for ADR-XXXX: a refusal is told where the press was.

Each mutant undoes one piece:
- the checker (PW0351): a predicate that says nothing, words that are two
  strings, empty or with a hole, a `requires` not held to a declared
  predicate's count or types, a predicate declared twice, `says` on what is
  not a predicate;
- the build and the host: no words written; a refusal answered without its
  words, without the program's, or without the deployment's; a refusal not
  kept with its press; a predicate the deployment cannot evaluate, required
  or declared, served; a page, of either kind, served without its
  announcer;
- the browser runtime: a refusal's predicate not read; its words not told;
  the message not named by the control, or not beside it; the announcer not
  emptied first, saying nothing, or a second one added beside the served
  one; a press keeping what its last was told; a press no answer came for,
  a stale page's and a failed handler's each told wrong or nothing; a page
  a build renders served without its announcer;
- the program: the feed's composer shown to a reader signed out.

A checker mutant must fail `pw-core`'s tests, all of them run; the build's
page's, `pw-render`'s binary's (`tests/titles.rs`); a host mutant, the
host's identity and announcer tests; a runtime or program mutant, the
browser tests below, in Chromium.

Run from the repository root; `just e14-refusal` records the output. The
source is restored after every mutant, whatever happens.
"""

import os
import pathlib
import re
import signal
import subprocess
import sys

ROOT = pathlib.Path(__file__).resolve().parent.parent
PREDICATES = ROOT / "compiler/pw-core/src/predicates.rs"
POLICY = ROOT / "compiler/pw-core/src/policy.rs"
BUILD = ROOT / "compiler/pw-core/src/build.rs"
SERVER = ROOT / "spikes/own-renderer/server/src/main.rs"
RENDER = ROOT / "runtime/pw-render/src/bin/pw-render.rs"
RUNTIME = ROOT / "spikes/own-renderer/public/pw-runtime.mjs"
FEED = ROOT / "examples/feed/app.pw"

# (what, suite, file, anchor, replacement)
MUTANTS = [
    (
        "a predicate that says nothing is accepted",
        "core",
        PREDICATES,
        "        if decl.kind == DeclKind::Predicate {\n",
        '        if decl.kind == DeclKind::Predicate && decl.policy("says").is_some() {\n',
    ),
    (
        "two strings are one predicate's words",
        "core",
        PREDICATES,
        "        .filter(|inner| !inner.contains('\"'))\n",
        "",
    ),
    (
        "a hole is words",
        "core",
        PREDICATES,
        "    if inner.contains('{') || inner.contains('}') {\n",
        "    if false {\n",
    ),
    (
        "empty words are words",
        "core",
        PREDICATES,
        "    if inner.trim().is_empty() {\n",
        "    if false {\n",
    ),
    (
        "a `requires` is not held to a predicate's count",
        "core",
        PREDICATES,
        "            if declared.params.len() != predicate.arguments.len() {\n",
        "            if false {\n",
    ),
    (
        "a `requires` is not held to a predicate's types",
        "core",
        PREDICATES,
        "                    && !given.same_as(taken)\n",
        "                    && false\n",
    ),
    (
        "a predicate declared twice is accepted",
        "core",
        PREDICATES,
        "                Some((_, module)) if u == unit => out.push(Diagnostic {\n",
        "                Some((_, module)) if u == unit && false => out.push(Diagnostic {\n",
    ),
    (
        "`says` is any declaration's",
        "core",
        POLICY,
        '        "says" => (&[K::Predicate], "a predicate"),\n',
        '        "says" => (EVERY, "a predicate"),\n',
    ),
    (
        "the build writes no words",
        "host",
        BUILD,
        "        predicates: crate::predicates::words_of(&hirs),\n",
        "        predicates: Default::default(),\n",
    ),
    (
        "a refusal is answered without its words",
        "host",
        SERVER,
        '                    "says": says,\n',
        "",
    ),
    (
        "the program's words are not told",
        "host",
        SERVER,
        "                let says = server\n                    .words\n                    .get(predicate)\n",
        '                let says = server\n                    .words\n                    .get("")\n',
    ),
    (
        "the deployment's words are not told",
        "host",
        SERVER,
        "                    .or_else(|| identity::says(predicate))\n",
        "                    .or_else(|| None)\n",
    ),
    (
        "a refusal is not kept with its press",
        "host",
        SERVER,
        '            "refused": self.refused,\n',
        "",
    ),
    (
        "a predicate the deployment cannot evaluate is served",
        "host",
        SERVER,
        "        if !unknown.is_empty() {\n",
        "        if false {\n",
    ),
    (
        "a predicate the program declares is not asked of the deployment",
        "host",
        SERVER,
        "            .chain(words.keys().map(String::as_str))\n",
        "",
    ),
    (
        "a page that binds no query is served without its announcer",
        "host",
        SERVER,
        ".unwrap_or_default());\n"
        "    // Where a failed press is said (ADR-XXXX), from the first byte.\n"
        "    let announcer = pw_render::ANNOUNCER;\n",
        ".unwrap_or_default());\n"
        "    // Where a failed press is said (ADR-XXXX), from the first byte.\n"
        '    let announcer = "";\n',
    ),
    (
        "a page that binds a query is served without its announcer",
        "host",
        SERVER,
        "    };\n"
        "    // Where a failed press is said (ADR-XXXX), from the first byte.\n"
        "    let announcer = pw_render::ANNOUNCER;\n",
        "    };\n"
        "    // Where a failed press is said (ADR-XXXX), from the first byte.\n"
        '    let announcer = "";\n',
    ),
    (
        "a page a build renders holds no announcer",
        "render",
        RENDER,
        "        tail.push_str(pw_render::ANNOUNCER);\n",
        "",
    ),
    (
        "a refusal's predicate is not read",
        "browser",
        RUNTIME,
        '        if (refusal?.refused) throw new Refused(component, refusal.refused, refusal.says ?? "");\n',
        "",
    ),
    (
        "a refusal is not told in its predicate's words",
        "browser",
        RUNTIME,
        "          tell(el, error.says || PLATFORM_SAYS.failed, `refused:${error.predicate}`);\n",
        "          tell(el, PLATFORM_SAYS.failed, `refused:${error.predicate}`);\n",
    ),
    (
        "the message is not beside the control",
        "browser",
        RUNTIME,
        "    el.after(message);\n",
        "    document.body.append(message);\n",
    ),
    (
        "the message is not named by the control",
        "browser",
        RUNTIME,
        '    el.setAttribute("aria-describedby", [...described, message.id].join(" "));\n',
        "",
    ),
    (
        "the announcer is not emptied first",
        "browser",
        RUNTIME,
        '  status.textContent = "";\n  clearTimeout(saying);\n',
        "  clearTimeout(saying);\n",
    ),
    (
        "the announcer says nothing",
        "browser",
        RUNTIME,
        "    status.textContent = words;\n",
        "",
    ),
    (
        "a second announcer is added beside the served one",
        "browser",
        RUNTIME,
        '  announcer ??= document.querySelector(".pw-announcer[role=status]");\n',
        "",
    ),
    (
        "a press keeps what its last was told",
        "browser",
        RUNTIME,
        "      untell(el);\n",
        "",
    ),
    (
        "a press no answer came for is told it failed",
        "browser",
        RUNTIME,
        '          tell(el, PLATFORM_SAYS.unreachable, "unreachable");\n',
        '          tell(el, PLATFORM_SAYS.failed, "unreachable");\n',
    ),
    (
        "a stale page's press is told nothing",
        "browser",
        RUNTIME,
        '        tell(el, PLATFORM_SAYS.stale, "reload-loop");\n',
        '        el.dataset.pwHandlerError = "reload-loop";\n',
    ),
    (
        "a failed handler is told nothing",
        "browser",
        RUNTIME,
        '          tell(el, PLATFORM_SAYS.failed, "failed");\n',
        '          el.dataset.pwHandlerError = "failed";\n',
    ),
    (
        "the feed's composer is shown to a reader signed out",
        "browser",
        FEED,
        "            <form hidden={!me.signed_in} on:submit|prevent={() => match post_text(draft) {\n",
        "            <form on:submit|prevent={() => match post_text(draft) {\n",
    ),
]

CARGO = {
    "core": ["cargo", "test", "--quiet", "--locked", "-p", "pw-core"],
    "render": ["cargo", "test", "--quiet", "--locked", "-p", "pw-render", "--test", "titles"],
    "host": [
        "cargo", "test", "--quiet", "--locked", "-p", "pw-dev-server", "--",
        "sign_in::", "a_signal_page_holds_its_announcer",
    ],
}
SPECS = [
    ["e2e/identity.spec.mjs", "--project=chromium"],
    ["e2e/recovery.spec.mjs", "-g", "still stale", "--project=chromium"],
    ["e2e/lazy-handler.spec.mjs", "-g", "fails to load", "--project=chromium"],
    ["e2e/accessibility.spec.mjs", "-g", "announcer|same node from the start", "--project=chromium"],
]

# How long one command may run. Past the bound it is killed with everything
# it started, and the run counts as failing.
BOUND = 1200


def bounded(cmd, **kw):
    """(output, returncode), or (output, None) when it ran past the bound."""
    p = subprocess.Popen(cmd, start_new_session=True, stdout=subprocess.PIPE,
                         stderr=subprocess.STDOUT, text=True, **kw)
    try:
        out, _ = p.communicate(timeout=BOUND)
        return out, p.returncode
    except subprocess.TimeoutExpired:
        os.killpg(p.pid, signal.SIGKILL)
        out, _ = p.communicate()
        return out, None


def cargo_tests(suite):
    """(built, passed, failed) over the suite's tests."""
    out, code = bounded(CARGO[suite], cwd=ROOT)
    if code is None:
        return True, 0, 1
    found = re.findall(r"test result: \w+\. (\d+) passed; (\d+) failed", out)
    if not found:
        return False, 0, 0
    return True, sum(int(p) for p, _ in found), sum(int(f) for _, f in found)


def browser_tests(_suite="browser"):
    """(built, passed, failed), after a build: the store's page and the
    feed's, each of which serves the runtime it was built with, and the
    server the suite runs, which neither script builds."""
    for script in ["run.sh", "feed.sh"]:
        built = subprocess.run(
            ["bash", f"spikes/own-renderer/{script}"],
            cwd=ROOT,
            env={**os.environ, "BUILD_ONLY": "1"},
            capture_output=True,
            text=True,
        )
        if built.returncode != 0:
            return False, 0, 0
    server = subprocess.run(
        ["cargo", "build", "--quiet", "--locked", "-p", "pw-dev-server"],
        cwd=ROOT,
        capture_output=True,
        text=True,
    )
    if server.returncode != 0:
        return False, 0, 0
    passed = failed = 0
    for spec in SPECS:
        out, code = bounded(
            ["pnpm", "exec", "playwright", "test", *spec, "--reporter=line"],
            cwd=ROOT / "spikes/own-renderer",
        )
        if code is None:
            failed += 1
            continue
        out = re.sub(r"\x1b\[[0-9;]*[A-Za-z]", "", out)
        passed += sum(int(n) for n in re.findall(r"(\d+) passed", out))
        failed += sum(int(n) for n in re.findall(r"(\d+) failed", out))
    return passed + failed > 0, passed, failed


SUITES = {
    "core": cargo_tests,
    "render": cargo_tests,
    "host": cargo_tests,
    "browser": browser_tests,
}


def main():
    import mutation_baseline  # keeps what each command says, for a red baseline
    # Stopped from outside, the source is still restored: SIGTERM raises
    # here, and the `finally` below runs.
    signal.signal(signal.SIGTERM, lambda *_: sys.exit(1))
    for suite, run in SUITES.items():
        built, passed, failed = run(suite)
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
            built, passed, failed = SUITES[suite](suite)
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
    # The pages and the server are built again from the restored source.
    browser_tests()

    print(f"{len(MUTANTS) - survivors} of {len(MUTANTS)} mutants killed")
    return 1 if survivors else 0


if __name__ == "__main__":
    sys.exit(main())
