#!/usr/bin/env python3
"""Strict controls for qualified Device reads (ADR-0366 integration amendment)."""
import os
import pathlib
import re
import signal
import subprocess
import producer_process
import sys
from cargo_test_results import parse

ROOT = pathlib.Path(__file__).resolve().parent.parent
CORE = ROOT / 'compiler/pw-core/src'
TEST = ROOT / 'compiler/pw-core/tests/qualified_device.rs'
MUTANTS = [
    ('qualified reads lose the Device label', CORE / 'labels.rs',
     '                if self.types.lexical().unbound_path(body, id)\n                    && let Some(label)',
     '                if false && self.types.lexical().unbound_path(body, id)\n                    && let Some(label)'),
    ('a lexical record borrows the modules Device label', CORE / 'labels.rs',
     '                if self.types.lexical().unbound_path(body, id)\n                    && let Some(label)',
     '                if true\n                    && let Some(label)'),
    ('qualified signals lose their declared type', CORE / 'values.rs',
     '                    .or_else(|| self.sigs.signal_type(d)?.resolved().map(Ty::of))\n', ''),
    ('qualified reads never enter the Device checker', CORE / 'device_store.rs',
     '            Expr::Name(n) | Expr::Field { name: n, .. } => names.contains(n.as_str()),\n',
     '            Expr::Name(n) => names.contains(n.as_str()),\n'),
    ('qualified diagnostics lose their declarations origin', CORE / 'device_store.rs',
     '        self.kept.contains_key(&def).then_some((path, def))\n',
     '        (self.kept.contains_key(&def) && false).then_some((path, def))\n'),
    ('qualified signal names resolve as bare names', CORE / 'signatures.rs',
     '    pub(crate) fn signal_named(&self, unit: usize, name: &str) -> Option<DefId> {\n        let found = if name.contains(\'.\') {\n            self.workspace.resolve_path_in(unit, Namespace::Term, name)',
     '    pub(crate) fn signal_named(&self, unit: usize, name: &str) -> Option<DefId> {\n        let found = if name.contains(\'.\') {\n            self.workspace.resolve_in(unit, Namespace::Term, name)'),
    ('captured record prefixes are mistaken for module paths', CORE / 'device_store.rs',
     '            lexical: crate::lexical::Lexical::build_in(hir, id)\n                .unwrap_or_else(|| crate::lexical::Lexical::build(decl, body)),',
     '            lexical: crate::lexical::Lexical::build(decl, body),'),
]
BOUND = 300
INVENTORY = None


def run():
    try:
        result = producer_process.run(['cargo', 'test', '--quiet', '--locked', '-p', 'pw-core',
            '--test', 'qualified_device', '--', '--format=pretty'], cwd=ROOT, timeout=BOUND)
    except subprocess.TimeoutExpired as expired:
        print(expired.output, flush=True)
        return False, 0
    return validate(result.returncode, result.stdout)


def validate(code, out):
    global INVENTORY
    result = parse(code, out)
    names = re.findall(r'^test (\S+) \.\.\. (?:ok|FAILED)$', out, re.M)
    summaries = re.findall(r'^test result: (?:ok|FAILED)\. \d+ passed; \d+ failed; (\d+) ignored; (\d+) measured;', out, re.M)
    valid = (result.valid and len(summaries) == 1 and summaries == [('0', '0')]
        and len(names) == result.passed + result.failed and len(set(names)) == len(names)
        and (INVENTORY is None or sorted(names) == INVENTORY))
    blocks = re.findall(r'^---- (\S+) stdout ----\n(.*?)(?=^---- |^failures:|^test result:|\Z)', out, re.M | re.S)
    if result.failed:
        valid = valid and len(blocks) == result.failed and {n for n, _ in blocks} == set(result.failure_names)
        for _, block in blocks:
            sites = re.findall(r'panicked at ([^\n]+?):(\d+):(\d+):', block)
            if len(sites) != 1:
                valid = False
                continue
            path, line, column = sites[0]
            source = pathlib.Path(path)
            source = source if source.is_absolute() else ROOT / source
            lines = TEST.read_text().splitlines()
            if (source != TEST or not 1 <= int(line) <= len(lines)
                    or not re.match(r'\s*assert(?:_eq|_ne)?!', lines[int(line) - 1])
                    or int(column) != len(lines[int(line) - 1]) - len(lines[int(line) - 1].lstrip()) + 1):
                valid = False
    print(out, flush=True)
    if valid and INVENTORY is None:
        INVENTORY = sorted(names)
    return valid, result.failed


def main():
    import mutation_baseline
    watch = mutation_baseline.mutation_bound.start()
    signal.signal(signal.SIGTERM, lambda *_: sys.exit(1))
    valid, failed = run()
    if not valid or failed or watch.stopped:
        print('REFUSED: baseline is not a complete green producer')
        return 1
    print(f'baseline: {len(INVENTORY)} passed, 0 failed', flush=True)
    killed = 0
    for name, path, anchor, replacement in MUTANTS:
        original = path.read_text()
        if original.count(anchor) != 1:
            print(f'REFUSED: {name}: anchor count {original.count(anchor)}')
            return 1
        try:
            path.write_text(original.replace(anchor, replacement, 1))
            valid, failed = run()
        finally:
            path.write_text(original)
        if not valid or watch.stopped:
            print(f'REFUSED: {name}: invalid or stopped producer, no kill')
            return 1
        if failed:
            killed += 1
            print(f'{name}: KILLED ({failed} of {len(INVENTORY)} assertions fail)', flush=True)
        else:
            print(f'{name}: SURVIVED', flush=True)
    print(f'{killed} of {len(MUTANTS)} mutants killed')
    return int(killed != len(MUTANTS))


if __name__ == '__main__':
    sys.exit(main())