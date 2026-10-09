#!/usr/bin/env python3
"""Mutation controls for ADR-0145: a page keeps every part and list its
queries decide current.

Each mutant undoes one part:
- the server: a part patched only when its text changed; a list's change as
  keyed operations (removed, inserted after the one before, moved, set in
  place); each change derived against what the document shows, recorded
  when it is served and as each change is sent; and the shared menu none of
  a session's lists. (A session's lists rendered from what the document
  shows was a mutant until ADR-0161 removed the loop: since ADR-0146 each
  list is its binding's value, already rendered.)
- the protocol: one change's patches as one `patch_set` frame;
- `pw-render --plan`: a private list the values do not give is empty, and a
  shared one must be given;
- the browser: a patch set applied, held once whole, and a document that
  cannot apply one read again.

A server mutant must fail the server's tests, a protocol mutant
`pw-protocol`'s, a renderer mutant `plan_lists.rs`, and a browser mutant
`e2e/resource-path.spec.mjs` or `e2e/store.spec.mjs` in Chromium, against
the staged build: run `BUILD_ONLY=1 bash spikes/own-renderer/run.sh` first.

ADR-0145's refusal of a block a query decides is ADR-0146's to plan, and
its mutants are `query_blocks_mutations.py`'s.

Run from the repository root; `just e14-patch-set` records the output. The
source is restored after every mutant, whatever happens.
"""

import pathlib
import re
import shutil
import subprocess
import sys

ROOT = pathlib.Path(__file__).resolve().parent.parent
SERVER = ROOT / "spikes/own-renderer/server/src/main.rs"
RENDER = ROOT / "runtime/pw-render/src/lib.rs"
PROTOCOL = ROOT / "runtime/pw-protocol/src/lib.rs"
BIN = ROOT / "runtime/pw-render/src/bin/pw-render.rs"
SPIKE = ROOT / "spikes/own-renderer"
RUNTIME = SPIKE / "public/pw-runtime.mjs"
STAGED = SPIKE / "dist/pw-runtime.mjs"

# (what, which tests, file, anchor, replacement)
MUTANTS = [
    (
        "a part that did not change is patched",
        "server",
        SERVER,
        "            if was.texts.get(id) != Some(text) {\n",
        "            if was.texts.get(id) != Some(text) || true {\n",
    ),
    (
        "an item that left is kept",
        "server",
        # Re-anchored by ADR-0181: the renderer derives a list's change.
        RENDER,
        "        } else {\n            out.push(ListChange::Remove(t));\n        }\n",
        "        } else {\n            let _ = t;\n        }\n",
    ),
    (
        "nothing moves",
        "server",
        # Re-anchored by ADR-0181.
        RENDER,
        "                if found != i {\n",
        "                if found != i && false {\n",
    ),
    (
        "a changed item is always rendered again",
        "server",
        # Re-anchored by ADR-0168: what changed, part by part; and by
        # ADR-0181, where the renderer derives it.
        RENDER,
        "                    match row_changes(part, &was, item, env, others, at)? {\n",
        "                    match row_changes(part, &was, item, env, others, at)?.filter(|_| false) {\n",
    ),
    (
        "a new item goes to the head",
        "server",
        # Re-anchored by ADR-0181.
        RENDER,
        "        prev = Some(t);\n    }\n    Ok(out)\n",
        "        let _ = t;\n    }\n    Ok(out)\n",
    ),
    (
        "what was sent is not remembered",
        "server",
        SERVER,
        # Re-anchored by ADR-0161: what each document shows. And by ADR-0222,
        # whose speculated values are derived with its patches.
        # And by the stream records' branch: derived outside the table.
        "                        shown.insert(doc.clone(), now.clone());\n                        (patches, speculated)\n",
        "                        let _ = &now;\n                        (patches, speculated)\n",
    ),
    (
        "what a served document shows is not recorded",
        "server",
        SERVER,
        "        self.shown\n            .lock()\n            .expect(\"shown\")\n            .insert(doc.clone(), Arc::new(shown));\n",
        "        let _ = shown;\n",
    ),
    (
        "the menu is a session's list",
        "server",
        SERVER,
        "                .any(|b| b[\"binding\"] == name && b[\"policy\"][\"cache\"] == \"shared\")\n",
        "                .any(|b| b[\"binding\"] == name && b[\"policy\"][\"cache\"] == \"never\")\n",
    ),
    (
        "a patch set is no frame of its own",
        "protocol",
        PROTOCOL,
        "    /// One change's patches, applied together (ADR-0145).\n    PatchSet(PatchSet),\n",
        "    /// One change's patches, applied together (ADR-0145).\n    #[serde(rename = \"patches\")]\n    PatchSet(PatchSet),\n",
    ),
    (
        "a private list the values do not give has no value",
        "render",
        BIN,
        "                        if private && !given.contains(name) {\n",
        "                        if false && private && !given.contains(name) {\n",
    ),
    (
        "a shared list renders empty when it is not given",
        "render",
        BIN,
        "                            let private =\n                                plan[\"bindings\"].as_array().into_iter().flatten().any(|b| {\n                                    b[\"binding\"] == name && b[\"policy\"][\"cache\"] == \"private\"\n",
        "                            let private =\n                                plan[\"bindings\"].as_array().into_iter().flatten().any(|b| {\n                                    b[\"binding\"] == name\n",
    ),
    (
        "the browser ignores a patch set",
        "browser",
        RUNTIME,
        "    case \"patch_set\": {\n",
        "    case \"patch_set_ignored\": {\n",
    ),
    (
        "the browser applies a patch set it cannot apply whole",
        "browser",
        RUNTIME,
        "        if (!applied) {\n",
        "        if (false) {\n",
    ),
    (
        "the browser holds no version for a patch set",
        "browser",
        RUNTIME,
        "      for (const r of frame.basis.resources) held.set(r.entry, r.version);\n      // What a change inserted may be pressed.\n",
        "      // What a change inserted may be pressed.\n",
    ),
]

CARGO = {
    "server": ["cargo", "test", "--quiet", "-p", "pw-dev-server"],
    "protocol": ["cargo", "test", "--quiet", "--locked", "-p", "pw-protocol"],
    "render": ["cargo", "test", "--quiet", "--locked", "-p", "pw-render", "--test", "plan_lists"],
}
BROWSER = [
    "pnpm", "exec", "playwright", "test", "e2e/resource-path.spec.mjs", "e2e/store.spec.mjs",
    "--project=chromium", "--reporter=line",
]


def run_cargo(kind):
    """(built, passed, failed) over one crate's tests."""
    r = subprocess.run(CARGO[kind], cwd=ROOT, capture_output=True, text=True)
    out = r.stdout + r.stderr
    results = re.findall(r"test result: \w+\. (\d+) passed; (\d+) failed", out)
    if not results:
        return False, 0, 0
    return True, sum(int(p) for p, _ in results), sum(int(f) for _, f in results)


def run_browser(_kind):
    """(ran, passed, failed) of the browser's specs, against the staged runtime."""
    shutil.copyfile(RUNTIME, STAGED)
    r = subprocess.run(BROWSER, cwd=SPIKE, capture_output=True, text=True)
    out = re.sub(r"\x1b\[[0-9;]*[A-Za-z]", "", r.stdout + r.stderr)
    passed = re.search(r"^\s+(\d+) passed", out, re.M)
    failed = re.search(r"^\s+(\d+) failed", out, re.M)
    if passed is None and failed is None:
        return False, 0, 0
    return True, int(passed.group(1)) if passed else 0, int(failed.group(1)) if failed else 0


RUNS = {
    "server": run_cargo,
    "protocol": run_cargo,
    "render": run_cargo,
    "browser": run_browser,
}


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
