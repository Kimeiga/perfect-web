#!/usr/bin/env python3
"""Mutation controls for track evidence-integrity-2: the integrator's rulings
of 2026-10-15 on its predecessor's findings, and the coordinator's ruling on
`mutant-programs`.

Each mutant undoes one piece:
1. a failed browser test fails its recipe (ruling 6): `scripts/run_suite.sh`
   exits 0 whatever the suite did, stops at the first failed run, passes a
   run that passed no test, or keeps no failed count; a recipe swallows its
   suite again (read by the gate, and by the recipes run with stand-ins), or
   the benchmark's controls; `scripts/suite_status.py` reads no `||`, no
   condition, no holding group or loop, no `set +e`, no `-` line, no shell
   without `-e`, no suite outside the helper, no `bench.mjs`, or no floor;
2. the planner reads a recipe whose parameters have defaults, and gives the
   benchmark's controls browsers (`ci_plan.py`);
3. a change to `playwright.config.mjs` plans every recipe that runs a suite
   under it (ruling 7), a mutation script's suite among them; a changed e2e
   module reaches the specs that import it; a recipe that runs every spec
   is planned by a changed spec;
4. `mutant_programs.py` finds every program a mutation script mutates: the
   old list again, no floor, a library checked only in its own program, a
   fixture's directory one program, a tie resolved, a fixture's own
   directory a source of its imports;
5. no comparison translates an old number (ruling 1): `codes::rule_status`
   translates it again, or the corpus's test in `rules.rs` does;
6. the committed enforcement table is held to the corpus (ruling 2): the
   table drifts from what the corpus checks;
7. `e9-latency` names the machine it measured (ruling 3): it names
   `macbook.md` again wherever it runs, or records no load.

Each runs the suite that holds it: `scripts/tests/`'s, by `unittest`, or
`pw-core`'s, by `cargo test`. At least one test must fail.

Run from the repository root; `just e14-evidence-integrity-2` records the
output. The sources are restored after every mutant, whatever happens.
"""

import os
import pathlib
import re
import signal
import subprocess
import sys

ROOT = pathlib.Path(__file__).resolve().parent.parent
HELPER = ROOT / "scripts/run_suite.sh"
GATE = ROOT / "scripts/suite_status.py"
JUSTFILE = ROOT / "justfile"
PLAN = ROOT / "scripts/ci_plan.py"
PROGRAMS = ROOT / "scripts/mutant_programs.py"
CODES = ROOT / "compiler/pw-core/src/codes.rs"
RULES = ROOT / "compiler/pw-core/src/rules.rs"
TABLE = ROOT / "docs/evidence/E2D/corpus-enforcement.txt"
HISTORY = ROOT / "scripts/recorded_browser_failures.py"

# (what, suite, file, anchor, replacement)
MUTANTS = [
    # 1. A failed browser test fails its recipe.
    ("the helper exits 0 whatever the suite did", "helper", HELPER, 'exit "$first"\n', "exit 0\n"),
    (
        "the helper stops at the first failed run",
        "helper",
        HELPER,
        "      first=$status\n",
        "      first=$status\n      break\n",
    ),
    (
        "the helper passes a run that passed no test",
        "helper",
        HELPER,
        "  if [ \"$status\" -eq 0 ] && ! grep -aqE '^ +[1-9][0-9]* passed' \"$out\"; then\n",
        "  if false; then\n",
    ),
    (
        "the helper keeps no failed count",
        "helper",
        HELPER,
        "EVIDENCE|^ +[0-9]+ (passed|failed|flaky|skipped|interrupted|did not run)'\n",
        "EVIDENCE|^ +[0-9]+ (passed|skipped)'\n",
    ),
    (
        "a recipe swallows its suite again, read by the gate",
        "gate",
        JUSTFILE,
        "         bash scripts/run_suite.sh -- pnpm exec playwright test e2e/lazy-handler.spec.mjs --project=$e --reporter=list; \\\n",
        "         bash scripts/run_suite.sh -- pnpm exec playwright test e2e/lazy-handler.spec.mjs --project=$e --reporter=list || true; \\\n",
    ),
    (
        "a recipe swallows its suite again, run with stand-ins",
        "recipes",
        JUSTFILE,
        "         bash scripts/run_suite.sh -- pnpm exec playwright test e2e/lazy-handler.spec.mjs --project=$e --reporter=list; \\\n",
        "         bash scripts/run_suite.sh -- pnpm exec playwright test e2e/lazy-handler.spec.mjs --project=$e --reporter=list || true; \\\n",
    ),
    (
        "the benchmark's controls are swallowed again",
        "gate",
        JUSTFILE,
        "       (cd benchmarks/harness && node bench.mjs controls --task {{TASK}} 2>&1); \\\n",
        "       (cd benchmarks/harness && node bench.mjs controls --task {{TASK}} 2>&1) || true; \\\n",
    ),
    (
        "the gate reads no `||` after a suite",
        "gate",
        GATE,
        '        if t.kind == "op" and t.text in ("||", "&&", "|", "&"):\n',
        '        if t.kind == "op" and t.text in ("&&", "|", "&"):\n',
    ),
    (
        "the gate reads no condition",
        "gate",
        GATE,
        '        elif depth == 0 and t.kind == "word" and t.text in CONDITIONS and (k == 0 or toks[k - 1].kind == "op" or toks[k - 1].text in STARTS_AFTER):\n',
        "        elif False:\n",
    ),
    (
        "the gate reads no group or loop that holds a suite",
        "gate",
        GATE,
        '        problems += [f"its status is swallowed: the {names[kind]} holding it {p}" for p in swallowed(toks, opened, closed)]\n',
        "        pass\n",
    ),
    (
        "the gate reads no `set +e`",
        "gate",
        GATE,
        "                if any(ERREXIT_OFF.search(l) for l in by_recipe[recipe]):\n",
        "                if False:\n",
    ),
    ("the gate reads no `-` line", "gate", GATE, '                if "-" in prefix:\n', "                if False:\n"),
    (
        "the gate reads no shell without `-e`",
        "gate",
        GATE,
        '    if not any("e" in f for f in flags):\n',
        "    if False:\n",
    ),
    (
        "the gate lets a suite outside the helper be",
        "gate",
        GATE,
        "                    if not is_helper(words):\n",
        "                    if False:\n",
    ),
    (
        "the gate reads no `bench.mjs`",
        "gate",
        GATE,
        '                bench = any(a.endswith("bench.mjs") and b == "controls" for a, b in pairs_of)\n',
        "                bench = False\n",
    ),
    ("the gate has no floor", "gate", GATE, "    elif recognised != occurrences:\n", "    elif False:\n"),
    (
        "the historical report is read as another failed suite",
        "history",
        HISTORY,
        '            print(f"    | {line[:150]}")\n',
        '            print(f"    {line[:150]}")\n',
    ),
    (
        "a zero run count with leading zeros passes",
        "helper",
        HELPER,
        'if ! [ "$runs" -gt 0 ] 2>/dev/null; then\n  echo "run_suite: --runs takes a count of 1 or more, not \'$runs\'" >&2\n  exit 2\nfi\n',
        "",
    ),
    # 2. The planner reads every recipe's body.
    (
        "a parameter's default stops a recipe's header again",
        "plan",
        PLAN,
        r'''HEADER = re.compile(
    r"""^([a-z0-9][a-z0-9-]*)"""
    r"""(?:\s+[$*+]?[A-Za-z_][A-Za-z0-9_-]*(?:=(?:"[^"]*"|'[^']*'|[^\s'":=]+))?)*"""
    r"""\s*:(?!=)"""
)
''',
        r'''HEADER = re.compile(r"^([a-z0-9][a-z0-9-]*)\s*(?:[^:=]*)?:(?!=)")
''',
    ),
    (
        "the benchmark's controls get no browsers",
        "plan",
        PLAN,
        '    browsers = "playwright" in body or "bench.mjs" in body\n',
        '    browsers = "playwright" in body\n',
    ),
    # 3. The suite's configuration plans what runs under it.
    (
        "a change to the configuration plans nothing by it",
        "plan",
        PLAN,
        '        or (PLAYWRIGHT_CONFIG in changed and runs_browser_suite(body.get(name, "")))\n',
        "        or False\n",
    ),
    (
        "a mutation script's suite does not count",
        "plan",
        PLAN,
        "        if re.search(r'\"playwright\",\\s*\"test\"|playwright\\s+test', text) and re.search(\n",
        "        if False and re.search(\n",
    ),
    (
        "a changed e2e module reaches no spec",
        "plan",
        PLAN,
        '            if re.search(rf"""from\\s+["\']\\./{re.escape(path.name)}["\']""", text):\n',
        "            if False:\n",
    ),
    (
        "a recipe that runs every spec is planned by none",
        "plan",
        PLAN,
        '        or (every_spec and runs_every_spec(body.get(name, "")))\n',
        "        or False\n",
    ),
    # 4. Every program a mutation script mutates.
    (
        "the programs are the old list again",
        "programs",
        PROGRAMS,
        "    roots = sorted({programs.root_of(m[2]) for m in found if m[2] in programs.text})\n",
        "    roots = sorted({programs.root_of(m[2]) for m in found if m[2] in programs.text"
        ' and m[2].startswith(("examples/store/", "examples/lib/", "examples/feed/", "examples/kiokun"))})\n',
    ),
    (
        "a mutant in no program is let be",
        "programs",
        PROGRAMS,
        '            failures.append(f"{m[0]}: {m[1]}: its source {m[2]} is in no program found")\n',
        "            pass\n",
    ),
    (
        "a library's mutant is checked only in its own program",
        "programs",
        PROGRAMS,
        "        within = [r for r in roots if m[2] in held[r]]\n",
        "        within = [r for r in roots if r == programs.root_of(m[2])]\n",
    ),
    (
        "a fixture's directory is one program",
        "programs",
        PROGRAMS,
        "        return rel if len(names) != len(set(names)) else parent(rel)\n",
        "        return parent(rel)\n",
    ),
    ("a tie between sources is resolved", "programs", PROGRAMS, "        if len(nearest) > 1:\n", "        if False:\n"),
    (
        "a fixture's own directory is a source of its imports",
        "programs",
        PROGRAMS,
        "            excluded = {rel for rel in self.text if parent(rel) == parent(root) and rel != root}\n",
        "            excluded = set()\n",
    ),
    # 5. No comparison translates an old number.
    (
        "the registry translates an old number again",
        "core",
        CODES,
        "    if let Some(c) = lookup(declared) {\n",
        "    let declared = crate::diagnostics::DEPRECATED_ALIASES\n"
        "        .iter()\n"
        "        .find(|(old, _)| *old == declared)\n"
        "        .map(|(_, now)| *now)\n"
        "        .unwrap_or(declared);\n"
        "    if let Some(c) = lookup(declared) {\n",
    ),
    (
        "the corpus's test translates an old number again",
        "core",
        RULES,
        "            if !found.contains(&expected) {\n",
        "            let expected = crate::diagnostics::DEPRECATED_ALIASES\n"
        "                .iter()\n"
        "                .find(|(old, _)| *old == expected)\n"
        "                .map(|(_, now)| *now)\n"
        "                .unwrap_or(expected);\n"
        "            if !found.contains(&expected) {\n",
    ),
    # 6. The committed enforcement table is held to the corpus.
    (
        "the committed table drifts from what the corpus checks",
        "corpus",
        TABLE,
        "R-001  enforced                  PW0401\n",
        "R-001  MISSED                    PW0401\n",
    ),
    # 7. e9-latency names the machine it measured.
    (
        "e9-latency names macbook.md wherever it runs",
        "latency",
        JUSTFILE,
        '       case "$(uname -s)" in \\\n'
        "         Darwin) machine=macbook ;; \\\n"
        "         Linux) if grep -qi microsoft /proc/version 2>/dev/null; then machine=pc-wsl; else machine=linux; fi ;; \\\n"
        "         *) machine=unknown ;; \\\n"
        "       esac; \\\n",
        "       machine=macbook; \\\n",
    ),
    (
        "e9-latency records no load",
        "latency",
        JUSTFILE,
        "       echo \"load before: $(uptime | sed 's/.*load averages*: //')\"; echo; \\\n",
        "       echo; \\\n",
    ),
]

UNITTEST = [sys.executable, "-m", "unittest"]
SUITES = {
    "helper": UNITTEST + ["scripts/tests/test_run_suite.py"],
    "history": UNITTEST + ["scripts/tests/test_recorded_browser_failures.py"],
    "gate": UNITTEST + ["scripts/tests/test_suite_status.py"],
    "recipes": UNITTEST + ["scripts/tests/test_suites_fail_their_recipes.py"],
    "plan": UNITTEST + ["scripts/tests/test_ci_scripts.py"],
    "programs": UNITTEST + ["scripts/tests/test_mutant_programs.py"],
    "core": ["cargo", "test", "--quiet", "--locked", "-p", "pw-core", "--lib"],
    "corpus": [
        "cargo", "test", "--quiet", "--locked", "-p", "pw-core", "--test", "checking_source", "--",
        "the_committed_enforcement_table",
    ],
    "latency": UNITTEST + ["scripts/tests/test_e9_latency_recipe.py"],
}

# How long one command may run. Past the bound it is killed with everything
# it started, and the run counts as failing.
BOUND = 1800


def counted(out):
    """(built, passed, failed) as a suite's output says: `unittest`'s `Ran`
    and `OK (...)` or `FAILED (...)` lines (a skipped test neither passed nor
    failed), or cargo's `test result:` lines."""
    ran = [int(n) for n in re.findall(r"^Ran (\d+) tests? in", out, re.M)]
    if ran:
        ended = re.findall(r"^(?:OK|FAILED) \((.*)\)$", out, re.M)
        failed = sum(int(n) for line in ended for n in re.findall(r"(?:failures|errors)=(\d+)", line))
        skipped = sum(int(n) for line in ended for n in re.findall(r"skipped=(\d+)", line))
        return True, sum(ran) - failed - skipped, failed
    found = re.findall(r"test result: \w+\. (\d+) passed; (\d+) failed", out)
    if not found:
        return False, 0, 0
    return True, sum(int(p) for p, _ in found), sum(int(f) for _, f in found)


def run_tests(suite):
    """(built, passed, failed) over the suite's tests."""
    p = subprocess.Popen(SUITES[suite], cwd=ROOT, start_new_session=True,
                         stdout=subprocess.PIPE, stderr=subprocess.STDOUT, text=True)
    try:
        out, _ = p.communicate(timeout=BOUND)
    except subprocess.TimeoutExpired:
        os.killpg(p.pid, signal.SIGKILL)
        p.communicate()
        return True, 0, 1
    return counted(out)


def main():
    import mutation_baseline  # keeps what each command says, for a red baseline
    # Stopped from outside, the sources are still restored: SIGTERM raises
    # here, and the `finally` below runs.
    signal.signal(signal.SIGTERM, lambda *_: sys.exit(1))
    for suite in SUITES:
        built, passed, failed = run_tests(suite)
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
            built, passed, failed = run_tests(suite)
        finally:
            path.write_text(original)
        if not built:
            verdict = "KILLED (does not build)"
        elif failed:
            verdict = f"KILLED ({failed} failure reports in {passed + failed} tests run)"
        else:
            verdict = "SURVIVED"
            survivors += 1
        print(f"{what} [{suite}]: {verdict}", flush=True)

    print(f"{len(MUTANTS) - survivors} of {len(MUTANTS)} mutants killed")
    return 1 if survivors else 0


if __name__ == "__main__":
    sys.exit(main())
