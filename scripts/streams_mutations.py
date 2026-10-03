#!/usr/bin/env python3
"""Mutation controls for ADR-0148: a stream region shows its query's state,
and its settled arm comes in the same response.

Each mutant undoes one part:
- the rules: PW5400, a streamed query read only by a stream; PW5401, each of
  a stream's arms, and its parts nowhere else; PW5402, a streamed query's
  budget;
- the typer: a failed arm given `Option` of the declared error, in both type
  passes;
- the IR and the plan: a stream at the top of a page only, its delivery, a
  `fallback` and a signal refused;
- the renderer: a pending region in a patchable range, and a settled one's
  arm given its value;
- the server: the document not waiting for a streamed region, a budget
  honoured, a declared error told from the host's and not kept, the response
  ended, the handlers in a region known, and the runtime started while the
  response is open;
- the runtime: a patch the browser left applied, and a settled region's
  handlers bound.

A rule or typer mutant must fail `streams.rs`; an IR or plan mutant
`stream_plan.rs`; a renderer mutant `pw-render`'s `streams.rs`; a server
mutant the server's tests, or the browser's spec where only a browser can
see it; a runtime mutant the browser's spec, in Chromium, whose Playwright
build has no out-of-order streaming of its own.

Run from the repository root after `BUILD_ONLY=1 bash spikes/own-renderer/
run.sh`; `just e14-streams` records the output. The source is restored after
every mutant, whatever happens.
"""

import pathlib
import re
import shutil
import subprocess
import sys

ROOT = pathlib.Path(__file__).resolve().parent.parent
CORE = ROOT / "compiler/pw-core/src"
STREAMS = CORE / "streams.rs"
INFER = CORE / "infer.rs"
VALUES = CORE / "values.rs"
IR = CORE / "template_ir.rs"
PLAN = CORE / "page_values.rs"
RENDER = ROOT / "runtime/pw-render/src/lib.rs"
SPIKE = ROOT / "spikes/own-renderer"
SERVER = SPIKE / "server/src/main.rs"
RUNTIME = SPIKE / "public/pw-runtime.mjs"
STAGED = SPIKE / "dist/pw-runtime.mjs"

# (what, which tests, file, anchor, replacement)
MUTANTS = [
    (
        "a page may wait for a streamed query",
        "rules",
        STREAMS,
        "                    if q.kind == DeclKind::Query && streamed(q) {\n",
        "                    if q.kind == DeclKind::Query && streamed(q) && false {\n",
    ),
    (
        "a streamed stream needs no placeholder",
        "rules",
        STREAMS,
        "        (true, None) => out.push(states(\n",
        "        (true, None) if false => out.push(states(\n",
    ),
    (
        "a placeholder no one sees is accepted",
        "rules",
        STREAMS,
        "        (false, Some(p)) => out.push(states(\n",
        "        (false, Some(p)) if false => out.push(states(\n",
    ),
    (
        "a stream needs no failed arm",
        "rules",
        STREAMS,
        "    if !found.contains_key(\"failed\") {\n",
        "    if false && !found.contains_key(\"failed\") {\n",
    ),
    (
        "a stream needs no ready arm",
        "rules",
        STREAMS,
        "    if !found.contains_key(\"ready\") {\n",
        "    if false && !found.contains_key(\"ready\") {\n",
    ),
    (
        "a stream's part may stand outside it",
        "rules",
        STREAMS,
        "                            && !held.contains(&n)\n",
        "                            && !held.contains(&n)\n                            && false\n",
    ),
    (
        "a failed arm may bind a failure no query declared",
        "rules",
        STREAMS,
        "    if !declares_error\n",
        "    if false && !declares_error\n",
    ),
    (
        "a streamed query needs no budget",
        "rules",
        STREAMS,
        "        if decl.kind == DeclKind::Query && streamed(decl) && decl.policy(\"timeout\").is_none() {\n",
        "        if decl.kind == DeclKind::Query && streamed(decl) && decl.policy(\"timeout\").is_none() && false {\n",
    ),
    (
        "the failed arm is the declared error (the typer)",
        "rules",
        INFER,
        "                    t.args().get(1).cloned().map(ResolvedType::optional)\n",
        "                    t.args().get(1).cloned()\n",
    ),
    (
        "the failed arm is the declared error (the value relations)",
        "rules",
        VALUES,
        "                    Ty::Builtin(Builtin::Option, vec![args.swap_remove(1)])\n",
        "                    args.swap_remove(1)\n",
    ),
    (
        "a stream inside a block is lowered",
        "plan",
        IR,
        "    if ix.depth > 0 {\n        out.push(blocked(\n            \"a `<stream>` inside a block",
        "    if false {\n        out.push(blocked(\n            \"a `<stream>` inside a block",
    ),
    (
        "every stream is streamed",
        "plan",
        IR,
        "        streamed: crate::streams::streamed(decl),\n",
        "        streamed: true,\n",
    ),
    (
        "a fallback no host executes is planned",
        "plan",
        PLAN,
        "        if let Some(f) = decl.policy(\"fallback\") {\n",
        "        if let Some(f) = decl.policy(\"fallback\").filter(|_| false) {\n",
    ),
    (
        "a signal shown in a stream is planned",
        "plan",
        PLAN,
        "                if signals.contains(&root(&read)) {\n",
        "                if false && signals.contains(&root(&read)) {\n",
    ),
    (
        "a pending region has no range to patch",
        "render",
        RENDER,
        "                    out.push_str(&format!(\"<?start name=\\\"{}\\\">\", stream_name(*id)));\n",
        "                    let _ = stream_name(*id);\n",
    ),
    (
        "a region the page waits for may be pending",
        "render",
        RENDER,
        "                None if *streamed => {\n",
        "                None if true => {\n",
    ),
    (
        "a declared error is shown as the host's failure",
        "render",
        RENDER,
        "                case: if why.is_some() { \"Some\" } else { \"None\" }.to_string(),\n",
        "                case: \"None\".to_string(),\n",
    ),
    (
        "the document waits for its streamed regions",
        "server",
        SERVER,
        "        while self.waiting.values().any(|(streamed, _)| !streamed) {\n",
        "        while !self.waiting.is_empty() {\n",
    ),
    (
        "a region's budget is not honoured",
        "server",
        SERVER,
        "                .map(|ms| started + std::time::Duration::from_millis(ms));\n",
        "                .map(|_| started + std::time::Duration::from_secs(3600));\n",
    ),
    (
        "a declared error is told as the host's failure",
        "server",
        SERVER,
        "        Ok(Val::Result(Err(Some(e)))) => Settled::Failed(Some(val_to_value(&e))),\n",
        "        Ok(Val::Result(Err(Some(_)))) => Settled::Failed(None),\n",
    ),
    (
        "a declared error is kept",
        "server",
        SERVER,
        "                    if matches!(*v, Val::Result(Err(_))) {\n",
        "                    if false {\n",
    ),
    (
        "the response is not ended with its last region",
        "server",
        SERVER,
        "        let _ = stream.shutdown(std::net::Shutdown::Write);\n",
        "",
    ),
    (
        "a handler in a stream is in no table",
        "server-browser",
        SERVER,
        "            for region in p.nested() {\n                walk(region, out);\n",
        "            for region in p.nested().into_iter().filter(|_| false) {\n                walk(region, out);\n",
    ),
    (
        "the runtime starts when the response ends",
        "server-browser",
        SERVER,
        "const RUNTIME: &str = \"<script>import(\\\"/pw-runtime.mjs\\\")</script>\";\n",
        "const RUNTIME: &str = \"<script type=\\\"module\\\" src=\\\"/pw-runtime.mjs\\\"></script>\";\n",
    ),
    (
        "a patch the browser left is not applied",
        "browser",
        RUNTIME,
        "  for (const t of document.querySelectorAll(\"template[for]\")) {\n",
        "  for (const t of []) {\n",
    ),
    (
        "a settled region's handlers are not bound",
        "browser",
        RUNTIME,
        "  buildIndex();\n  bindEvents();\n}\n\n/** Watch the document",
        "  buildIndex();\n}\n\n/** Watch the document",
    ),
]

CARGO = {
    "rules": ["cargo", "test", "--quiet", "--locked", "-p", "pw-core", "--test", "streams"],
    "plan": ["cargo", "test", "--quiet", "--locked", "-p", "pw-core", "--test", "stream_plan"],
    "render": ["cargo", "test", "--quiet", "--locked", "-p", "pw-render", "--test", "streams"],
    "server": ["cargo", "test", "--quiet", "--locked", "-p", "pw-dev-server", "stream"],
}
BROWSER = [
    "pnpm", "exec", "playwright", "test", "e2e/stream.spec.mjs",
    "--project=chromium", "--reporter=line",
]
DEV_SERVER = ["cargo", "build", "--quiet", "--locked", "-p", "pw-dev-server"]


def run_cargo(kind):
    """(built, passed, failed) over one crate's tests."""
    r = subprocess.run(CARGO[kind], cwd=ROOT, capture_output=True, text=True)
    results = re.findall(r"test result: \w+\. (\d+) passed; (\d+) failed", r.stdout + r.stderr)
    if not results:
        return False, 0, 0
    return True, sum(int(p) for p, _ in results), sum(int(f) for _, f in results)


def run_browser():
    """(ran, passed, failed) of the browser's spec, against the staged runtime
    and the development server as built now."""
    shutil.copyfile(RUNTIME, STAGED)
    r = subprocess.run(BROWSER, cwd=SPIKE, capture_output=True, text=True)
    out = re.sub(r"\x1b\[[0-9;]*[A-Za-z]", "", r.stdout + r.stderr)
    passed = re.search(r"^\s+(\d+) passed", out, re.M)
    failed = re.search(r"^\s+(\d+) failed", out, re.M)
    if passed is None and failed is None:
        return False, 0, 0
    return True, int(passed.group(1)) if passed else 0, int(failed.group(1)) if failed else 0


def run_server_browser():
    """The browser's spec, against the development server rebuilt."""
    if subprocess.run(DEV_SERVER, cwd=ROOT, capture_output=True).returncode != 0:
        return False, 0, 0
    return run_browser()


RUNS = {kind: (lambda k=kind: run_cargo(k)) for kind in CARGO}
RUNS["browser"] = run_browser
RUNS["server-browser"] = run_server_browser


def main():
    if not STAGED.exists():
        print("FAIL: no staged build; run `BUILD_ONLY=1 bash spikes/own-renderer/run.sh`")
        return 1
    for kind in ["rules", "plan", "render", "server", "server-browser"]:
        built, passed, failed = RUNS[kind]()
        print(f"baseline ({kind}): {passed} passed, {failed} failed")
        if not built or failed or not passed:
            print("FAIL: the unmutated baseline is not green; no mutant can mean anything")
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
            built, passed, failed = RUNS[kind]()
        finally:
            path.write_text(original)
            if path == RUNTIME:
                shutil.copyfile(RUNTIME, STAGED)
            if kind == "server-browser":
                subprocess.run(DEV_SERVER, cwd=ROOT, capture_output=True)
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
