#!/usr/bin/env python3
"""Mutation controls for ADR-XXXX: generic/member types retain their effects.

Each mutant removes one root inference, refusal, propagation or rooted-walk
behavior. Every mutant must build and fail a meaningful regression test.

Run from the repository root; `just e14-generic-effects` records the output.
The source is restored after every mutant, whatever happens.
"""

import pathlib
import re
import subprocess
import sys

ROOT = pathlib.Path(__file__).resolve().parent.parent
INFER = ROOT / "compiler/pw-core/src/infer.rs"
EFFECTS = ROOT / "compiler/pw-core/src/effects.rs"
VALUES = ROOT / "compiler/pw-core/src/values.rs"
CHECK = ROOT / "compiler/pw-core/src/check.rs"
LABELS = ROOT / "compiler/pw-core/src/labels.rs"

# (what is undone, file, anchor, replacement)
MUTANTS = [
    ('a function receiver is discarded as a path prefix', EFFECTS, '                        Some(crate::infer::MemberRead::Path)\n', '                        _\n'),
    ('a resolved module path is treated as a value receiver', INFER, '&& self.by_path(&path_of(body, id)).is_some()', '&& false'),
    ('interface term roots silently accept unknown receivers', CHECK, '    for read in types.untyped_receivers(body) {', '    for read in types.untyped_receivers(body).into_iter().filter(|_| decl.body.is_some()) {'),
    ('a generic result keeps the callees unsolved parameter', INFER, '            Some(t) if !self.unsolved(&t) => Some(t),', '            Some(t) => Some(t),'),
    ('a missing declared type ignores the call solvers type', INFER, '            _ => self.resolved_from(self.solved.expr(id), body.expr_span(id)),', '            _ => None,'),
    ('solved binder types are not shared with readers', INFER, '        types.bindings.extend(solved);', '        let _ = solved;'),
    ('a generic record field keeps its declared parameter', VALUES, '    s.close(&Ty::of(result).instantiate(owner))', '    Ty::of(result)'),
    ('a callable needing other arguments is treated as its result', INFER, '        if self.sigs.by_def(sig.definition).is_some() && sig.params.len() != 1 {', '        if false {'),
    ('unknown receiver reads are accepted silently', CHECK, '    for read in types.untyped_receivers(body) {', '    for read in types.untyped_receivers(body).into_iter().filter(|_| false) {'),
    ('rigid parameters silently have unknown members', VALUES, '            None if matches!(receiver, Ty::Parameter { .. }) => Outcome::Disagree {', '            None if false => Outcome::Disagree {'),
    ('only bare names have effectful members', EFFECTS, '            let (base, member) = (*base, member.clone());', '            let (base, member) = (*base, member.clone());\n            if !matches!(body.expr(base), Expr::Name(_)) { continue; }'),
    ('a member with no declared row hides its performed effects', EFFECTS, '                        .get(&sig.definition)', '                        .get(&sig.definition).filter(|_| false)'),
    ('a rooted effect walk ignores member reads', EFFECTS, '            self.member_effects(body, root, types, &mut out);', ''),
    ('a rooted effect walk ignores named function values', EFFECTS, '            self.value_effects(unit, body, root, types, &mut out);', ''),
    ('computed receivers resolve by the member spelling', EFFECTS, '                    if !matches!(body.expr(head), Expr::Name(_)) {\n                        continue;\n                    }', ''),
    ('a member carries no label through its receiver', LABELS, '                    self.types.member_read(body, id)', '                    self.types.member_read(body, id).filter(|_| false)'),
    ('draw receives no declared platform context', VALUES, '.language_type("browser", "PaintContext")', '.language_type("browser", "MissingPaintContext")'),
]
TESTS = [
    ["cargo", "test", "--quiet", "--locked", "-p", "pw-core", "--test", "generic_effects"],
]

def run_tests():
    """(built, passed, failed) over every test command."""
    built, passed, failed = True, 0, 0
    for cmd in TESTS:
        r = subprocess.run(cmd, cwd=ROOT, capture_output=True, text=True)
        out = r.stdout + r.stderr
        m = re.search(r"test result: \w+\. (\d+) passed; (\d+) failed", out)
        if m is None:
            built = False
            continue
        passed += int(m.group(1))
        failed += int(m.group(2))
    return built, passed, failed


def main():
    import mutation_baseline  # keeps what each command says, for a red baseline
    built, passed, failed = run_tests()
    print(f"baseline: {passed} passed, {failed} failed")
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
            verdict = "INVALID (does not build)"
            survivors += 1
        elif failed:
            verdict = f"KILLED ({failed} of {passed + failed} tests fail)"
        else:
            verdict = "SURVIVED"
            survivors += 1
        print(f"{what}: {verdict}")

    print(f"{len(MUTANTS) - survivors} of {len(MUTANTS)} mutants killed")
    return 1 if survivors else 0


if __name__ == "__main__":
    sys.exit(main())
