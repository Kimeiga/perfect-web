#!/usr/bin/env python3
"""Mutation controls for ADR-0146: a block a query decides is rendered and
kept current.

Each mutant undoes one part:
- the plan: a block a query decides is planned at the top of the page, and
  one inside it is not; what a loop's row reads of another query, a block
  or a value, is refused;
- the server: each binding's whole value given to the renderer; a component's
  case read as a case; each block's rendering recorded, compared, and sent
  again where it changed;
- the browser: a range rendered again where it is.

A plan mutant must fail `page_blocks.rs`, a server mutant the server's
tests, and a browser mutant `e2e/resource-path.spec.mjs` in Chromium,
against the staged build: run `BUILD_ONLY=1 bash spikes/own-renderer/run.sh`
first.

Run from the repository root; `just e14-query-blocks` records the output.
The source is restored after every mutant, whatever happens.
"""

import pathlib
import re
import shutil
import subprocess
import sys

ROOT = pathlib.Path(__file__).resolve().parent.parent
PLAN = ROOT / "compiler/pw-core/src/page_values.rs"
SERVER = ROOT / "spikes/own-renderer/server/src/main.rs"
SPIKE = ROOT / "spikes/own-renderer"
RUNTIME = SPIKE / "public/pw-runtime.mjs"
STAGED = SPIKE / "dist/pw-runtime.mjs"

# (what, which tests, file, anchor, replacement)
MUTANTS = [
    (
        "a block a query decides is not planned",
        "plan",
        PLAN,
        # Re-anchored by ADR-0229, whose computed subject decides one too.
        "                || derived_values.iter().any(|d| d.path == entry.value))\n"
        "            && template.chunks.iter().any(\n"
        "                |c| matches!(c, crate::template_ir::Chunk::Dynamic(p) if p.id() == Some(entry.id)),\n"
        "            )\n        {\n            blocks.push(entry.id.0);\n",
        "                || derived_values.iter().any(|d| d.path == entry.value))\n"
        "            && template.chunks.iter().any(\n"
        "                |c| matches!(c, crate::template_ir::Chunk::Dynamic(p) if p.id() == Some(entry.id)),\n"
        "            )\n        {\n            let _ = entry.id.0;\n",
    ),
    (
        "a block inside one is planned as if at the top",
        "plan",
        PLAN,
        # Re-anchored by ADR-0229, as above.
        "                || derived_values.iter().any(|d| d.path == entry.value))\n"
        "            && template.chunks.iter().any(\n",
        "                || derived_values.iter().any(|d| d.path == entry.value))\n"
        "            && template.chunks.iter().any(|_| true) | template.chunks.iter().any(\n",
    ),
    (
        "a row reads another query",
        "plan",
        PLAN,
        "            if let Some(q) = read.iter().find(|r| queries.contains(&r.as_str())) {\n",
        "            if let Some(q) = read.iter().find(|r| queries.contains(&r.as_str())).filter(|_| false) {\n",
    ),
    (
        "a row's blocks are not read",
        "plan",
        PLAN,
        "                | Part::Conditional { value, .. }\n                | Part::Match { value, .. } => vec![root(value)],\n",
        "                | Part::Match { value, .. } => vec![root(value)],\n",
    ),
    (
        "a binding's whole value is not given",
        "server",
        SERVER,
        "            env = env.set(name, rendered);\n",
        "            let _ = (name, rendered);\n",
    ),
    (
        "a case is text",
        "server",
        SERVER,
        "        Val::Enum(case) => Value::Variant {\n            case: case.clone(),\n            payload: None,\n        },\n",
        "        Val::Enum(case) => Value::Text(case.clone()),\n",
    ),
    (
        "a block's rendering is not compared",
        "server",
        SERVER,
        "            if was.blocks.get(id) != Some(html) {\n",
        "            if was.blocks.get(id) != Some(html) || true {\n",
    ),
    (
        "a block is not rendered again",
        "server",
        SERVER,
        "                    operation: PatchOp::ReplaceRange { html: html.clone() },\n",
        "                    operation: PatchOp::ReplaceText { text: String::new() },\n",
    ),
    (
        "the browser renders no range again",
        "browser",
        RUNTIME,
        "              ? replaceRangeAt(key, op.html) !== null\n",
        "              ? false\n",
    ),
]

CARGO = {
    "plan": ["cargo", "test", "--quiet", "--locked", "-p", "pw-core", "--test", "page_blocks"],
    "server": ["cargo", "test", "--quiet", "-p", "pw-dev-server"],
}
BROWSER = [
    "pnpm", "exec", "playwright", "test", "e2e/resource-path.spec.mjs",
    "--project=chromium", "--reporter=line",
]


def run_cargo(kind):
    """(built, passed, failed) over one crate's tests."""
    r = subprocess.run(CARGO[kind], cwd=ROOT, capture_output=True, text=True)
    results = re.findall(r"test result: \w+\. (\d+) passed; (\d+) failed", r.stdout + r.stderr)
    if not results:
        return False, 0, 0
    return True, sum(int(p) for p, _ in results), sum(int(f) for _, f in results)


def run_browser(_kind):
    """(ran, passed, failed) of the browser's spec, against the staged runtime."""
    shutil.copyfile(RUNTIME, STAGED)
    r = subprocess.run(BROWSER, cwd=SPIKE, capture_output=True, text=True)
    out = re.sub(r"\x1b\[[0-9;]*[A-Za-z]", "", r.stdout + r.stderr)
    passed = re.search(r"^\s+(\d+) passed", out, re.M)
    failed = re.search(r"^\s+(\d+) failed", out, re.M)
    if passed is None and failed is None:
        return False, 0, 0
    return True, int(passed.group(1)) if passed else 0, int(failed.group(1)) if failed else 0


RUNS = {"plan": run_cargo, "server": run_cargo, "browser": run_browser}


def main():
    import mutation_baseline  # keeps what each command says, for a red baseline
    if not STAGED.exists():
        print("FAIL: no staged build; run `BUILD_ONLY=1 bash spikes/own-renderer/run.sh`")
        return 1
    for kind in RUNS:
        built, passed, failed = RUNS[kind](kind)
        print(f"baseline ({kind}): {passed} passed, {failed} failed")
        if not built or failed or not passed:
            print("FAIL: the unmutated baseline is not green; no mutant can mean anything")
            mutation_baseline.explain()
            return 1

    survivors = 0
    for what, kind, path, anchor, replacement in MUTANTS:
        original = path.read_text()
        if original.count(anchor) != 1:
            print(f"{what}: ANCHOR NOT FOUND EXACTLY ONCE in {path.name}")
            survivors += 1
            continue
        try:
            path.write_text(original.replace(anchor, replacement, 1))
            built, passed, failed = RUNS[kind](kind)
        finally:
            path.write_text(original)
            if path == RUNTIME:
                shutil.copyfile(RUNTIME, STAGED)
        if not built:
            verdict = "KILLED (does not build)"
        elif failed:
            verdict = f"KILLED ({failed} of {passed + failed} tests fail)"
        else:
            verdict = "SURVIVED"
            survivors += 1
        print(f"{what} [{kind}]: {verdict}")

    print(f"{len(MUTANTS) - survivors} of {len(MUTANTS)} mutants killed")
    return 1 if survivors else 0


if __name__ == "__main__":
    sys.exit(main())
