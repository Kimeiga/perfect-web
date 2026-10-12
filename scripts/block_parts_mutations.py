#!/usr/bin/env python3
"""Mutation controls for ADR-XXXX: a block is told part by part while its arm
stays.

Each mutant undoes one piece:
- the plan: a block that renders a query's value inside it not planned
  unless a query decides it; what a host computes from a query, or what a
  handler captures of one, not read as a query's; a block's insides read one
  level deep; a block inside one planned too;
- the renderer: a `{#match}` that shows another arm looked into; its names
  read from one side; a match rendered again whatever changed inside it;
  and a block always rendered again whole;
- the host: a block always rendered again whole; one a speculation renders
  told part by part; the values a block was rendered from not kept; and
  what it showed read from what it shows now;
- the browser: a block rendered again whole, which takes its status region
  with it.

A plan mutant must fail `compiler/pw-core/tests/blocks_that_render_a_query.rs`
or `page_blocks.rs`; a renderer mutant, `runtime/pw-render/tests/
block_changes.rs` or `instance_changes.rs`; a host mutant, the host's tests
of a block told part by part, of one a speculation renders, and of the
search's sentence; the browser's, `e2e/store-search.spec.mjs` in Chromium.

Run from the repository root; `just e14-block-parts` records the output.
The source is restored after every mutant, whatever happens.
"""

import os
import pathlib
import re
import signal
import subprocess
import sys

ROOT = pathlib.Path(__file__).resolve().parent.parent
PLAN = ROOT / "compiler/pw-core/src/page_values.rs"
RENDER = ROOT / "runtime/pw-render/src/lib.rs"
SERVER = ROOT / "spikes/own-renderer/server/src/main.rs"

IN_PLACE = (
    "            let in_place = match &before {\n"
    "                // **Not one a speculation renders** (ADR-0172): the browser\n"
)

# (what, suite, file, anchor, replacement)
MUTANTS = [
    (
        "a block that renders a query's value is planned only where a query decides it",
        "core",
        PLAN,
        "                || part.nested().into_iter().any(|r| renders(r, &queried)))\n",
        "                || false)\n",
    ),
    (
        "a value a host computes from a query is no query's",
        "core",
        PLAN,
        "            .any(|v| found_root(v) || derived_values.iter().any(|d| d.path == *v))\n",
        "            .any(|v| found_root(v))\n",
    ),
    (
        "what a handler captures of a query is no query's",
        "core",
        PLAN,
        "                        && r.kind == crate::template_ir::ReadKind::Captures\n",
        "                        && r.kind == crate::template_ir::ReadKind::Captures\n"
        "                        && false\n",
    ),
    (
        "a block's insides are read one level deep",
        "core",
        PLAN,
        "            queried(p) || p.nested().into_iter().any(|r| renders(r, queried))\n",
        "            queried(p)\n",
    ),
    (
        "a block inside one is planned too",
        "core",
        PLAN,
        "            && let Some(part) = template.chunks.iter().find_map(|c| match c {\n"
        "                crate::template_ir::Chunk::Dynamic(p) if p.id() == Some(entry.id) => Some(p),\n"
        "                _ => None,\n"
        "            })\n",
        "            && let Some(part) = find_part(&template.chunks, entry.id.0)\n",
    ),
    (
        "a match that shows another arm is looked into",
        "render",
        RENDER,
        "                if x.case != y.case {\n                    return Ok(false);\n                }\n",
        "                if x.case != y.case && false {\n                    return Ok(false);\n                }\n",
    ),
    (
        "what a match's arm binds is read from one side",
        "render",
        RENDER,
        "                if !changes_within(&y.body, &was, &now, handlers, others, at, out)? {\n",
        "                if !changes_within(&y.body, &now, &now, handlers, others, at, out)? {\n",
    ),
    (
        "a match is rendered again whatever changed inside it",
        "render",
        RENDER,
        "                let (x, was) = arm_shown(arms, value, before)?;\n",
        "                if true {\n                    return Ok(false);\n                }\n"
        "                let (x, was) = arm_shown(arms, value, before)?;\n",
    ),
    (
        "a block is always rendered again whole",
        "render",
        RENDER,
        "    let chunk = std::slice::from_ref(chunk);\n",
        "    if true {\n        return Ok(None);\n    }\n    let chunk = std::slice::from_ref(chunk);\n",
    ),
    (
        "the host renders every block again whole",
        "host",
        SERVER,
        IN_PLACE,
        "            let in_place: Option<Vec<(PartId, pw_render::InstanceChange)>> = None;\n"
        "            let _ = match &before {\n"
        "                // **Not one a speculation renders** (ADR-0172): the browser\n",
    ),
    (
        "a block a speculation renders is told part by part",
        "host",
        SERVER,
        "                    if was.blocks.contains_key(id) && !self.speculation_renders(page, *id) =>\n",
        "                    if was.blocks.contains_key(id) =>\n",
    ),
    (
        "the values a block was rendered from are not kept",
        "host",
        SERVER,
        "            .any(|id| !self.speculation_renders(page, *id))\n        {\n"
        "            shown.bindings = Some(Arc::new(bindings.clone()));\n",
        "            .any(|id| !self.speculation_renders(page, *id))\n        {\n"
        "            let _ = bindings;\n",
    ),
    (
        "what a block showed is read from what it shows now",
        "host",
        SERVER,
        "            .map(|b| self.document_env(page, session, params, b));\n",
        "            .map(|_| self.document_env(page, session, params, bindings));\n",
    ),
    (
        "the browser is sent every block whole, its status region with it",
        "browser",
        SERVER,
        IN_PLACE,
        "            let in_place: Option<Vec<(PartId, pw_render::InstanceChange)>> = None;\n"
        "            let _ = match &before {\n"
        "                // **Not one a speculation renders** (ADR-0172): the browser\n",
    ),
]

CARGO = {
    "core": [
        "cargo", "test", "--quiet", "--locked", "-p", "pw-core",
        "--test", "blocks_that_render_a_query", "--test", "page_blocks",
    ],
    "render": [
        "cargo", "test", "--quiet", "--locked", "-p", "pw-render",
        "--test", "block_changes", "--test", "instance_changes",
    ],
    "host": [
        "cargo", "test", "--quiet", "--locked", "-p", "pw-dev-server", "--",
        "a_block_that_shows_the_arm_it_showed_is_told_part_by_part",
        "a_block_a_speculation_renders_is_rendered_again_whole",
        "a_block_whose_rendering_changed_is_rendered_again",
        "the_sentence_inside_its_arm_is_told_where_it_is",
        "an_open_search_page_is_told_of_a_menu_change",
    ],
}
SPEC = ["e2e/store-search.spec.mjs", "--project=chromium"]

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
    """(built, passed, failed), after a build: the pages, and the servers the
    suite starts."""
    built = subprocess.run(
        ["bash", "spikes/own-renderer/run.sh"],
        cwd=ROOT,
        env={**os.environ, "BUILD_ONLY": "1"},
        capture_output=True,
        text=True,
    )
    if built.returncode != 0:
        return False, 0, 0
    server = subprocess.run(
        ["cargo", "build", "--quiet", "--locked", "-p", "pw-dev-server", "-p", "kiokun-server"],
        cwd=ROOT,
        capture_output=True,
        text=True,
    )
    if server.returncode != 0:
        return False, 0, 0
    out, code = bounded(
        ["pnpm", "exec", "playwright", "test", *SPEC, "--reporter=line"],
        cwd=ROOT / "spikes/own-renderer",
    )
    if code is None:
        return True, 0, 1
    out = re.sub(r"\x1b\[[0-9;]*[A-Za-z]", "", out)
    passed = sum(int(n) for n in re.findall(r"(\d+) passed", out))
    failed = sum(int(n) for n in re.findall(r"(\d+) failed", out))
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
