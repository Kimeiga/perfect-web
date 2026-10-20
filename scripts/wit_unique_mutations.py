#!/usr/bin/env python3
"""Controls for namespace-preserving component identities (ADR-0373).

Run whole through just e14-wit-unique. A broken build or stopped producer is
refused, never a kill; source is restored after every mutation.
"""
import os
import pathlib
import signal
import subprocess
import sys

from cargo_test_results import parse as cargo_result

ROOT = pathlib.Path(__file__).resolve().parent.parent
CORE = ROOT / "compiler/pw-core/src"

MUTANTS = [
    (
        "component IDs erase the UI namespace",
        CORE / "contract.rs",
        '        format!("{path}~ui")\n',
        '        path\n',
    ),
    (
        "graph paths erase the UI namespace",
        CORE / "graph.rs",
        'pub(crate) fn path_of(hir: &Hir, id: crate::hir::DeclId) -> String {\n    crate::contract::component_id(hir, id)\n}',
        'pub(crate) fn path_of(hir: &Hir, id: crate::hir::DeclId) -> String {\n    crate::contract::component_id(hir, id).trim_end_matches("~ui").to_string()\n}',
    ),
    (
        "template paths erase the UI namespace",
        CORE / "template_ir.rs",
        '    let template = Template {\n        path: crate::contract::component_id(hir, id),\n',
        '    let template = Template {\n        path: crate::contract::component_id(hir, id).trim_end_matches("~ui").to_string(),\n',
    ),
    (
        "page value plans erase the UI namespace",
        CORE / "page_values.rs",
        '    let page = crate::contract::component_id(hir, id);\n\n    // The page\'s own signals',
        '    let page = crate::contract::component_id(hir, id).trim_end_matches("~ui").to_string();\n\n    // The page\'s own signals',
    ),
    (
        "speculation modules erase the UI namespace",
        CORE / "backend/speculation.rs",
        'fn template_path(hir: &Hir, id: DeclId) -> String {\n    crate::contract::component_id(hir, id)\n}',
        'fn template_path(hir: &Hir, id: DeclId) -> String {\n    crate::contract::component_id(hir, id).trim_end_matches("~ui").to_string()\n}',
    ),
    (
        "view tags choose a same-named term",
        CORE / "check.rs",
        'let resolved = match workspace.resolve_in(unit, crate::resolve::Namespace::Ui, tag) {',
        'let resolved = match workspace.resolve(unit, tag) {',
    ),
    (
        "view composition chooses a same-named term",
        CORE / "template_ir.rs",
        '.resolve_in(self.unit, crate::resolve::Namespace::Ui, tag)',
        '.resolve(self.unit, tag)',
    ),
    (
        "view cycles hide behind a same-named term",
        CORE / "template_ir.rs",
        'ws.resolve_in(def.unit, crate::resolve::Namespace::Ui, tag)',
        'ws.resolve(def.unit, tag)',
    ),
    (
        "semantic review erases UI declaration identity",
        CORE / "semantic.rs",
        'let declaration_path = declaration_path(hir, id);',
        'let declaration_path = declaration_path(hir, id).trim_end_matches("~ui").to_string();',
    ),
    (
        "semantic review erases noncomponent namespace identity",
        CORE / "semantic.rs",
        '    format!("{path}{suffix}")',
        '    format!("{path}{}", &suffix[..0])',
    ),
]

TESTS = ["cargo", "test", "--quiet", "--locked", "-p", "pw-core", "--test", "component_names"]

# How long one command may run. Past the bound it is killed with everything
# it started, and the run stops: a command stopped is no kill.
BOUND = 1200


def bounded(cmd):
    """(output, returncode), or (output, None) when it ran past the bound."""
    p = subprocess.Popen(cmd, cwd=ROOT, start_new_session=True,
                         stdout=subprocess.PIPE, stderr=subprocess.STDOUT, text=True)
    try:
        out, _ = p.communicate(timeout=BOUND)
        return out, p.returncode
    except subprocess.TimeoutExpired:
        os.killpg(p.pid, signal.SIGKILL)
        out, _ = p.communicate()
        return out, None


def cargo_tests():
    """(valid, passed, failed) over the tests (`cargo_test_results`)."""
    out, code = bounded(TESTS)
    result = cargo_result(code, out)
    if not result.valid:
        return False, 0, 0
    return True, result.passed, result.failed


def main():
    import mutation_baseline  # keeps what each command says, for a red baseline
    watch = mutation_baseline.mutation_bound.start()
    # Stopped from outside, the source is still restored: SIGTERM raises
    # here, and the `finally` below runs.
    signal.signal(signal.SIGTERM, lambda *_: sys.exit(1))
    built, passed, failed = cargo_tests()
    print(f"baseline: {passed} passed, {failed} failed", flush=True)
    if not built or failed or not passed or watch.stopped:
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
            built, passed, failed = cargo_tests()
        finally:
            path.write_text(original)
        if not built or watch.stopped:
            print(f"{what}: REFUSED (invalid or stopped Cargo producer; no kill)", flush=True)
            mutation_baseline.explain()
            return 1
        if failed:
            verdict = f"KILLED ({failed} of {passed + failed} tests fail)"
        else:
            verdict = "SURVIVED"
            survivors += 1
        print(f"{what}: {verdict}", flush=True)

    print(f"{len(MUTANTS) - survivors} of {len(MUTANTS)} mutants killed")
    return 1 if survivors else 0


if __name__ == "__main__":
    sys.exit(main())
