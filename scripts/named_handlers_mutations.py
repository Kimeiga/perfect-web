#!/usr/bin/env python3
"""Mutation controls for ADR-0199: a function or a command named as a
handler, `on:submit={save}`, is `(e) => save(e)` (ADR-0195, ruling 12).

Each mutant undoes one piece: what an `on:` value names, the backend
compiling it and giving it its event, the rules it is held to as its lambda
form is (its event, its answer, idempotency, what it performs in the
browser), its identity, and the refusal, when checked, of a name that is no
function or command.

Every mutant must fail the tests of the rule.

Run from the repository root; `just e14-named-handlers` records the output.
The source is restored after every mutant, whatever happens.
"""

import os
import pathlib
import re
import signal
import subprocess
import sys

ROOT = pathlib.Path(__file__).resolve().parent.parent
RESUME = ROOT / "compiler/pw-core/src/resume.rs"
INFER = ROOT / "compiler/pw-core/src/infer.rs"
BACKEND = ROOT / "compiler/pw-core/src/backend/lower.rs"
ANNOTATIONS = ROOT / "compiler/pw-core/src/annotations.rs"
CHECK = ROOT / "compiler/pw-core/src/check.rs"
ARTIFACTS = ROOT / "compiler/pw-core/src/resume_artifacts.rs"
TEMPLATE = ROOT / "compiler/pw-core/src/template_ir.rs"

# (what, file, anchor, replacement)
MUTANTS = [
    (
        "a declaration's name is no handler",
        RESUME,
        "        Expr::Lambda { .. } | Expr::Name(_) | Expr::Field { .. }\n    )\n",
        "        Expr::Lambda { .. }\n    )\n",
    ),
    (
        "a local's value is a named handler",
        INFER,
        "        if self.lexical.binder(root).is_some() {\n            return None;\n        }\n",
        "",
    ),
    (
        "any declaration is a named handler",
        INFER,
        "                Some(DeclKind::Fn | DeclKind::Command)\n",
        "                Some(_)\n",
    ),
    (
        "the backend does not compile a named handler",
        BACKEND,
        "        Expr::Name(_) | Expr::Field { .. } => {\n            let types = crate::infer::Types::of_decl(cx.sigs, hir, decl_id, body);\n",
        "        Expr::Name(_) | Expr::Field { .. } if false => {\n            let types = crate::infer::Types::of_decl(cx.sigs, hir, decl_id, body);\n",
    ),
    (
        "a named handler is not given its event",
        BACKEND,
        "        None => f.call(body, lambda, &[], event_value, None, span.clone()),\n",
        "        None => f.call(body, lambda, &[], None, None, span.clone()),\n",
    ),
    (
        "a named handler's answer is not the runtime's",
        ANNOTATIONS,
        "            .named_handler(body, h)\n",
        "            .named_handler(body, h)\n            .filter(|_| false)\n",
    ),
    (
        "a named handler's other parameters are not counted",
        ANNOTATIONS,
        "            if sig.params.len() > 1 {\n",
        "            if sig.params.len() > 99 {\n",
    ),
    (
        "a name that is no function or command is not refused",
        ANNOTATIONS,
        "                if let Some(what) = names_no_handler(decl, body, sigs, types, module, e, &handler) {\n",
        "                if let Some(what) = names_no_handler(decl, body, sigs, types, module, e, &handler).filter(|_| false) {\n",
    ),
    (
        "a named command is not sent",
        RESUME,
        "                            if e == handler\n",
        "                            if false\n",
    ),
    (
        "a local's name is sent as the command it shadows",
        RESUME,
        "                                && !root_name(body, e)\n                                    .is_some_and(|r| lexical.binder(r).is_some()) =>\n",
        "                                && !root_name(body, e).is_some_and(|_| false) =>\n",
    ),
    (
        "a named function's effects are not held to the browser",
        CHECK,
        "            if matches!(body.expr(lambda), Expr::Name(_) | Expr::Field { .. }) {\n                if crate::resume::root_name(body, lambda)\n",
        "            if false {\n                if crate::resume::root_name(body, lambda)\n",
    ),
    (
        "a local's name is held to the browser as what it shadows",
        CHECK,
        "                if crate::resume::root_name(body, lambda)\n                    .is_some_and(|r| lexical.binder(r).is_some())\n",
        "                if crate::resume::root_name(body, lambda)\n                    .is_some_and(|_| false)\n",
    ),
    (
        "a named function is held to its row alone",
        CHECK,
        "                for effect in inference.effects_of_def(def).into_iter().flatten() {\n",
        "                for effect in crate::resolve::declaration(hirs, def).and_then(|d| d.declared_effects.as_ref()).into_iter().flatten().map(|e| &e.written) {\n",
    ),
    (
        "a command named as the handler is held to the browser",
        CHECK,
        "                    .is_none_or(|d| d.kind == DeclKind::Command)\n",
        "                    .is_none_or(|d| d.kind == DeclKind::Command && false)\n",
    ),
    (
        "a named handler's identity ignores what it names",
        ARTIFACTS,
        "            (body.expr_span(lambda), vec![reference(lambda)])\n",
        "            (body.expr_span(lambda), vec![])\n",
    ),
    (
        "a named handler has no artifact",
        ARTIFACTS,
        "    types.named_handler(body, e).is_some()\n",
        "    types.named_handler(body, e).is_some() && false\n",
    ),
    (
        "a named handler is named by nothing",
        TEMPLATE,
        "        Expr::Name(n) => n.clone(),\n",
        "        Expr::Name(_) => String::new(),\n",
    ),
]

TESTS = [
    ["cargo", "test", "--quiet", "--locked", "-p", "pw-core", "--test", "named_handlers"],
    ["cargo", "test", "--quiet", "--locked", "-p", "pw-core", "--test", "handlers_in_the_browser"],
    ["cargo", "test", "--quiet", "--locked", "-p", "pw-core", "--test", "every_handler_is_resumable"],
]

# How long one command may run. Past the bound it is killed with everything
# it started, and the run counts as failing.
BOUND = 1200


def run_tests():
    """(built, passed, failed) over the tests. A test process that aborts,
    as a stack overflow does, fails."""
    passed = failed = 0
    for command in TESTS:
        p = subprocess.Popen(command, cwd=ROOT, start_new_session=True,
                             stdout=subprocess.PIPE, stderr=subprocess.STDOUT, text=True)
        try:
            out, _ = p.communicate(timeout=BOUND)
        except subprocess.TimeoutExpired:
            os.killpg(p.pid, signal.SIGKILL)
            p.communicate()
            return True, passed, failed + 1
        found = re.findall(r"test result: \w+\. (\d+) passed; (\d+) failed", out)
        if not found:
            if "could not compile" in out or "error[E" in out:
                return False, passed, failed
            # It built, and the process ended before it reported.
            failed += 1
            continue
        passed += sum(int(n) for n, _ in found)
        failed += sum(int(f) for _, f in found)
    return True, passed, failed


def main():
    import mutation_baseline  # keeps what each command says, for a red baseline
    # Stopped from outside, the source is still restored: SIGTERM raises
    # here, and the `finally` below runs.
    signal.signal(signal.SIGTERM, lambda *_: sys.exit(1))
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
        print(f"{what}: {verdict}", flush=True)

    print(f"{len(MUTANTS) - survivors} of {len(MUTANTS)} mutants killed")
    return 1 if survivors else 0


if __name__ == "__main__":
    sys.exit(main())
