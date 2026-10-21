#!/usr/bin/env python3
"""Mutation controls for ADR-0351: generic/member types retain their effects.

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
BOUNDED = ROOT / "compiler/pw-core/src/bounded.rs"
LEXICAL = ROOT / "compiler/pw-core/src/lexical.rs"
SIGNATURES = ROOT / "compiler/pw-core/src/signatures.rs"
LABELS = ROOT / "compiler/pw-core/src/labels.rs"
BACKEND = ROOT / "compiler/pw-core/src/backend/lower.rs"

# (what is undone, file, anchor, replacement)
MUTANTS = [
    ('a generic call returns its declared parameter even recursively', INFER, '                if mentions_binder(result, sig.definition) {', '                if false {'),
    ('a function receiver is discarded as a path prefix', EFFECTS, '                        Some(crate::infer::MemberRead::Path)\n', '                        _\n'),
    ('a resolved module path is treated as a value receiver', INFER, '&& self.by_path(&path_of(body, id)).is_some()', '&& false'),
    ('interface term roots silently accept unknown receivers', CHECK, '    for read in types.untyped_receivers(body) {', '    for read in types.untyped_receivers(body).into_iter().filter(|_| decl.body.is_some()) {'),
    ('a missing declared type ignores the call solvers type', INFER, '            _ => self.resolved_from(self.solved.expr(id), body.expr_span(id)),', '            _ => None,'),
    ('solved binder types are not shared with readers', INFER, '        types.bindings.extend(solved);', '        let _ = solved;'),
    ('a generic record field keeps its declared parameter', VALUES, '    s.close(&Ty::of(result).instantiate(owner))', '    Ty::of(result)'),
    ('a callable needing other arguments is treated as its result', INFER, '        if self.sigs.by_def(sig.definition).is_some() && sig.params.len() != 1 {', '        if false {'),
    ('unknown receiver reads are accepted silently', CHECK, '    for read in types.untyped_receivers(body) {', '    for read in types.untyped_receivers(body).into_iter().filter(|_| false) {'),
    ('rigid parameters silently have unknown members', VALUES, '            None if matches!(receiver, Ty::Parameter { .. }) => Outcome::Disagree {', '            None if false => Outcome::Disagree {'),
    ('only bare names have effectful members', EFFECTS, '            let (base, member) = (*base, member.clone());', '            let (base, member) = (*base, member.clone());\n            if !matches!(body.expr(base), Expr::Name(_)) { continue; }'),
    ('a member with no declared row hides its performed effects', EFFECTS, '.get(&sig.definition)', '.get(&sig.definition).filter(|_| false)'),
    ('a rooted effect walk ignores member reads', EFFECTS, '            self.member_effects(body, root, types, &mut out, None);', ''),
    ('a rooted effect walk ignores named function values', EFFECTS, '            self.value_effects(unit, body, root, types, &mut out, None);', ''),
    ('computed receivers resolve by the member spelling', EFFECTS, '                    if !matches!(body.expr(head), Expr::Name(_)) {\n                        continue;\n                    }', ''),
    ('a member carries no label through its receiver', LABELS, '                    self.types.member_read(body, id)', '                    self.types.member_read(body, id).filter(|_| false)'),
    ('draw receives no declared platform context', VALUES, '.language_type("browser", "PaintContext")', '.language_type("browser", "MissingPaintContext")'),
    ('a receiver-only property has no invocation', EFFECTS, '                            _ => self.sigs.property_receiver(sig).is_some(),', '                            _ => false,'),
    ('a member invocation borrows its spelled module path', EFFECTS, '            if let (Some(types), Some(field)) = (types, field) {', '            if let (Some(types), Some(field)) = (types, field) && false {'),
    ('a member callee field is invoked twice', EFFECTS, '                Expr::Field { .. } if !callees.contains(&id) => Some(id),', '                Expr::Field { .. } => Some(id),'),
    ('a structural field is a property invocation', SIGNATURES, '        self.by_def(sig.definition)?;', '        let _ = self;'),
    ('a property gives no bounded receiver', BOUNDED, '                return Some(vec![Some(*base)]);', '                return None;'),
    ('interface effect roots forget their local bindings', EFFECTS, '        for (unit, id, _, _, types) in &typed {\n            self.lexicals.insert((*unit, *id), types.lexical().clone());\n        }', '        for (unit, id, _, implemented, types) in &typed {\n            if *implemented { self.lexicals.insert((*unit, *id), types.lexical().clone()); }\n        }'),
    ('interface terms have no canonical lexical arena', LEXICAL, '        let body = hir.body(decl.terms_body()?);', '        let body = hir.body(decl.body?);'),
    ('a local receiver call is borrowed from a module', VALUES, '    if !lexical.unbound_path(body, callee) {', '    if false {'),
    ('a local receiver property is borrowed from a module', VALUES, '        self.lexical.unbound_path(self.body, id)', '        true'),
    ('a callee borrows a namesake paths result before its receiver type', INFER, '        match body.expr(callee) {\n            Expr::Field { .. } => match self.member_read(body, callee) {', '        if let Some(sig) = self.by_path(&path_of(body, callee)) { return Some(sig); }\n        match body.expr(callee) {\n            Expr::Field { .. } => match self.member_read(body, callee) {'),
    ('a local function call borrows a globals result type', INFER, '            Expr::Name(_) if self.lexical.binder(callee).is_some() => None,', '            Expr::Name(_) if false => None,'),
    ('a local property borrows a type qualified constructor', VALUES, '    if !lexical.unbound_path(body, expr) {', '    if false {'),
    ('the handle constructor scan forgets local callback identity', CHECK, 'crate::values::named_at(sigs, workspace, unit, body, &lexical, *callee)', 'crate::values::named_at(sigs, workspace, unit, body, &crate::lexical::Lexical::default(), *callee)'),
    ('a backend property emits a namesake type constructor', BACKEND, 'self.expression_case(body, e)', 'crate::values::case_named(self.cx.sigs, self.cx.ws, self.unit, &crate::infer::path_of(body, e))'),
    ('a backend member call borrows a module function', BACKEND, '        if !self.declaration_path(body, callee) {', '        if false {'),
    ('a backend member callback borrows a module function', BACKEND, '                if !self.declaration_path(body, f) {', '                if false {'),
]
TESTS = [
    ["cargo", "test", "--quiet", "--locked", "-p", "pw-core", "--test", "generic_effects"],
    ["cargo", "test", "--quiet", "--locked", "-p", "pw-core", "--test", "bounded_tables"],
    ["cargo", "test", "--quiet", "--locked", "-p", "pw-core", "--test", "backend_lowering"],
    ["cargo", "test", "--quiet", "--locked", "-p", "pw-core", "--test", "handles"],
    ["cargo", "test", "--quiet", "--locked", "-p", "pw-dev-server", "--bin", "pw-dev-server", "tests::receiver_identity::a_local_receiver_getter_runs_instead_of_a_namesake_type_constructor", "--", "--exact"],
]

def run_tests():
    from cargo_test_results import parse
    passed = failed = 0
    for command in TESTS:
        result = subprocess.run(command, cwd=ROOT, capture_output=True, text=True)
        outcome = parse(result.returncode, result.stdout + result.stderr)
        if not outcome.valid:
            return False, 0, 0
        passed += outcome.passed
        failed += outcome.failed
    return True, passed, failed


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
            verdict = "REFUSED (invalid Cargo producer)"
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
