#!/usr/bin/env python3
"""Mutation controls for ADR-0042: `{:else}`, `{#match}` and interpolated
attributes in templates.

A test that passes with the mechanism removed is not evidence for it. Each
mutant undoes one piece of ADR-0042, and at least one test assertion must
then fail. Build errors and incomplete runs earn no kill. The first defect
ADR-0042 found was the lowering that dropped
`{:else}`, so both branches of an `if` rendered together.

`evidence_is_current` is left out of the tests run here: it would kill every
mutant that changes kiokun's committed build by noticing only that the bytes
differ, which says nothing about what they mean.

Run from the repository root; `just e10-templates` records the output. The
sources are restored after every mutant, whatever happens.
"""

import pathlib
import os
import re
import signal
import subprocess
import producer_process
import sys

import cargo_test_results
import mutation_source

ROOT = pathlib.Path(__file__).resolve().parent.parent
LOWER = ROOT / "compiler/pw-core/src/lower.rs"
IR = ROOT / "compiler/pw-core/src/template_ir.rs"
CHECK = ROOT / "compiler/pw-core/src/check.rs"
INFER = ROOT / "compiler/pw-core/src/infer.rs"
RENDER = ROOT / "runtime/pw-render/src/lib.rs"

# (what is undone, file, anchor, replacement)
MUTANTS = [
    (
        "the lowering drops `{:else}` again",
        LOWER,
        "                    children.push(match is_block_marker(self.src, &c) {\n"
        "                        true => self.branch(b, &c),\n"
        "                        false => self.markup(b, &c),\n"
        "                    });",
        "                    if is_block_marker(self.src, &c) {\n"
        "                        continue;\n"
        "                    }\n"
        "                    children.push(self.markup(b, &c));",
    ),
    (
        "an attribute's holes are static text again",
        LOWER,
        "                    let parts = match raw.contains('{') {",
        "                    let parts = match false {",
    ),
    (
        "`{:else}` renders nothing",
        IR,
        "=>\n            {\n                lower_run(body, run, ctx, ix)\n            }",
        "=>\n            {\n                Vec::new()\n            }",
    ),
    (
        "`{:else if}` loses its branch",
        IR,
        "let chunk = conditional(body, nested, v, run, more, ctx, ix);",
        "let chunk = conditional(body, nested, v, &[], more, ctx, ix);",
    ),
    (
        "a URL may begin with a value",
        IR,
        "    if context == Context::Url && !matches!(segments.first(), Some(Segment::Static(_))) {",
        "    if false && !matches!(segments.first(), Some(Segment::Static(_))) {",
    ),
    (
        "a block may close with another's name",
        CHECK,
        "        } else if closer != name {",
        "        } else if false {",
    ),
    (
        "an unknown directive passes `pw check`",
        CHECK,
        '        if !matches!(name.as_str(), "if" | "each" | "match") {',
        "        if false {",
    ),
    (
        "an `{#if}` takes any marker",
        CHECK,
        '                    if !(*else_if || (marker == "{:else}" && last)) {',
        "                    if false {",
    ),
    (
        "`{:else}` in `{#each}` passes",
        CHECK,
        "                if let Some((b, marker, ..)) = branches.first() {",
        "                if let Some((b, marker, ..)) = branches.first().filter(|_| false) {",
    ),
    (
        "a template match need not be exhaustive",
        CHECK,
        "                if !missing.is_empty() {",
        "                if false {",
    ),
    (
        "`{#if}` over an Option passes",
        CHECK,
        "                    && matches!(ty.as_builtin(), Some(Builtin::Option | Builtin::Result))",
        "                    && matches!(ty.as_builtin(), Some(Builtin::Function))",
    ),
    (
        "an arm's binding has no type",
        INFER,
        # ADR-0351's solved fallback types the arm when its declaration-only
        # binding is absent. Remove the answer at the shared consumer boundary
        # so this still models an untyped arm, not a redundant insertion.
        "    pub fn of(&self, body: &Body, id: ExprId) -> Option<ResolvedType> {\n",
        "    pub fn of(&self, body: &Body, id: ExprId) -> Option<ResolvedType> {\n"
        "        if matches!(self.lexical.binder(id), Some(Binder::Arm(_, _))) { return None; }\n",
    ),
    (
        "a match renders its first arm whatever the case",
        RENDER,
        # Re-anchored by ADR-0320, whose `arm_shown` both a render and a
        # block's change read.
        "    let arm =\n        arms.iter()\n            .find(|a| a.case == *case)\n",
        "    let arm =\n        arms.first()\n",
    ),
    (
        "an arm's payload is not bound",
        RENDER,
        "        (Some(name), Some(p)) => env.with(name, (**p).clone()),",
        "        (Some(_), Some(_)) => env.clone(),",
    ),
    (
        "a value in a URL is attribute-escaped, not one component",
        RENDER,
        "                            Context::Url => escape::url_component(&v),",
        "                            Context::Url => escape::attribute(&v),",
    ),
    (
        "an Option reads as a condition",
        RENDER,
        "            Value::Variant { .. } => Err(Blocked::UnrepresentedConstruct {",
        "            Value::Bool(_) if false => Err(Blocked::UnrepresentedConstruct {",
    ),
]

TESTS = [
    ["cargo", "test", "--quiet", "--locked", "-p", "pw-core", "--test", "template_blocks"],
    ["cargo", "test", "--quiet", "--locked", "-p", "pw-render", "--test", "branches"],
    ["cargo", "test", "--quiet", "--locked", "-p", "kiokun-server"],
]

INVENTORIES = {}
SOURCES = (
    {"compiler/pw-core/tests/template_blocks.rs"},
    {"runtime/pw-render/tests/branches.rs"},
    {"spikes/kiokun/server/src/main.rs", "spikes/kiokun/server/src/data.rs"},
)
# These four tests were already ignored in this campaign: three require an
# external whole-shard checkout, and one is the separate release benchmark.
# Explicitly exclude them from this selected run so every selected test must
# finish. The same 10 active kiokun tests still execute for every control.
KIOKUN_OTHER_RUNS = (
    "tests::every_entry_in_the_whole_shard_looks_up_and_renders",
    "tests::memory_and_work_per_call",
    "tests::the_compiled_search_is_kiokuns_ranking_on_the_whole_shard",
    "tests::the_compiled_shard_rule_places_every_word_kiokun_has",
)


def result_of(index, code, output):
    result = cargo_test_results.parse(code, output)
    tests = re.findall(r"^test (\S+) \.\.\. (ok|FAILED)$", output, re.M)
    names = tuple(sorted(name for name, _ in tests))
    if (not result.valid or len(tests) != result.passed + result.failed
            or len(set(names)) != len(names)
            or re.search(r"; [1-9]\d* (?:ignored|measured)", output)
            or (index in INVENTORIES and INVENTORIES[index] != names)):
        return False, 0, 0
    if result.failed:
        blocks = re.findall(r"^---- (\S+) stdout ----\n(.*?)(?=^---- |^failures:|^test result:|\Z)", output, re.M | re.S)
        failed = {name for name, status in tests if status == "FAILED"}
        if (len(blocks) != result.failed or {name for name, _ in blocks} != failed
                or set(result.failure_names) != failed):
            return False, 0, 0
        for _, block in blocks:
            sites = re.findall(r"panicked at ([^\n]+?):(\d+):(\d+):", block)
            if len(sites) != 1:
                return False, 0, 0
            path, line, column = sites[0]
            try:
                source = (ROOT / path).resolve().relative_to(ROOT)
            except ValueError:
                return False, 0, 0
            if source.as_posix() not in SOURCES[index]:
                return False, 0, 0
            lines = (ROOT / source).read_text().splitlines()
            if (not 1 <= int(line) <= len(lines)
                    or not (start := re.match(r"(\s*)assert(?:_eq|_ne)?!", lines[int(line) - 1]))
                    or int(column) != len(start.group(1)) + 1):
                return False, 0, 0
    INVENTORIES.setdefault(index, names)
    return True, result.passed, result.failed


def run_tests():
    """Complete native runs with the baseline's frozen test inventory."""
    built, passed, failed = True, 0, 0
    for index, cmd in enumerate(TESTS):
        flags = ["--", "--format=pretty"]
        if index == 2:
            flags.extend(arg for name in KIOKUN_OTHER_RUNS for arg in ("--skip", name))
        result = producer_process.run([*cmd, *flags], cwd=ROOT, timeout=600)
        out = result.stdout
        valid, p, f = result_of(index, result.returncode, out)
        if not valid or f:
            print(out, flush=True)
        built &= valid
        passed += p
        failed += f
    return built, passed, failed


def main():
    import mutation_baseline  # keeps what each command says, for a red baseline
    signal.signal(signal.SIGTERM, lambda *_: sys.exit(1))
    watch = mutation_baseline.mutation_bound.start()
    INVENTORIES.clear()
    built, passed, failed = run_tests()
    print(f"baseline: {passed} passed, {failed} failed")
    if not built or failed or not passed or watch.stopped:
        print("FAIL: the unmutated baseline is not green; no mutant can mean anything")
        mutation_baseline.explain()
        return 1

    survivors = refused = killed = 0
    for what, path, anchor, replacement in MUTANTS:
        original = path.read_text()
        if original.count(anchor) != 1:
            print(f"{what}: ANCHOR NOT FOUND EXACTLY ONCE in {path.name}")
            refused += 1
            continue
        with mutation_source.changed(path, original, original.replace(anchor, replacement, 1)):
            built, passed, failed = run_tests()
        if not built or watch.stopped:
            verdict = "REFUSED (invalid or stopped producer; no kill)"
            refused += 1
        elif failed:
            verdict = f"KILLED ({failed} of {passed + failed} tests fail)"
            killed += 1
        else:
            verdict = "SURVIVED"
            survivors += 1
        print(f"{what}: {verdict}")

    print(f"{killed} of {len(MUTANTS)} mutants killed; {refused} refused; {survivors} survived")
    return int(bool(survivors or refused))


if __name__ == "__main__":
    sys.exit(main())
