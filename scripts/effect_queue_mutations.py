#!/usr/bin/env python3
"""Queue scheduling preserves rows and metadata while avoiding repeated work."""
from pathlib import Path
import sys

import effect_convergence_mutations as native
import mutation_source

ROOT = Path(__file__).resolve().parents[1]
EFFECTS = ROOT / 'compiler/pw-core/src/effects.rs'
SOURCE = ROOT / 'compiler/pw-core/src/effects/queue_tests.rs'
PREFIX = 'effects::queue_tests::'
NAMES = [PREFIX + name for name in (
    'late_rows_refresh_declared_metadata_and_noninvoked_member_values',
    'a_self_reader_refreshes_its_own_call_charge_after_row_growth',
    'single_effect_chains_visit_only_the_bodies_whose_rows_changed',
    'many_growing_dependencies_enqueue_their_pending_reader_only_once',
    'readers_remain_registered_when_a_dependency_grows_again',
)]
COMMAND = ['cargo', 'test', '--quiet', '--locked', '-p', 'pw-core', '--lib',
           '--', '--format=pretty', '--exact', *NAMES]
MUTANTS = [
    ('direct callers do not subscribe to row growth', EFFECTS,
     '                    if let (Some(def), Some(reads)) = (def, reads.as_deref_mut()) {\n                        reads.insert(def);\n                    }\n                    let effects = def',
     '                    let effects = def'),
    ('named function values do not subscribe to row growth', EFFECTS,
     '                    if let (Some(def), Some(reads)) = (def, reads.as_deref_mut()) {\n                        reads.insert(def);\n                    }\n                    def.and_then',
     '                    def.and_then'),
    ('typed members do not subscribe to row growth', EFFECTS,
     '        if let Some(reads) = reads {\n            reads.insert(sig.definition);\n        }',
     '        let _ = reads;'),
    ('unchanged permitted rows discard performed metadata', EFFECTS,
     '            self.performed.insert(def, found);',
     '            if !self.performed.contains_key(&def) || entry.len() != before { self.performed.insert(def, found); }'),
    ('already pending readers are queued repeatedly', EFFECTS,
     '                    if !queued[reader] {',
     '                    if true {'),
    ('only the first row growth notifies readers', EFFECTS,
     '            if entry.len() != before {',
     '            if before == 0 && entry.len() != before {'),
]
WITNESSES = [
    'single_effect_chains_visit_only_the_bodies_whose_rows_changed',
    'single_effect_chains_visit_only_the_bodies_whose_rows_changed',
    'late_rows_refresh_declared_metadata_and_noninvoked_member_values',
    'late_rows_refresh_declared_metadata_and_noninvoked_member_values',
    'many_growing_dependencies_enqueue_their_pending_reader_only_once',
    'readers_remain_registered_when_a_dependency_grows_again',
]


def main():
    import mutation_baseline
    watch = mutation_baseline.mutation_bound.start()
    baseline = native.run(COMMAND, NAMES, [SOURCE])
    if baseline is None or baseline.failed or watch.stopped:
        print('REFUSED: complete green queue baseline required')
        return 1
    print(f'baseline: {baseline.passed} passed, 0 failed', flush=True)
    killed = 0
    for (name, path, anchor, replacement), method in zip(MUTANTS, WITNESSES, strict=True):
        original = path.read_text()
        if original.count(anchor) != 1:
            print(f'REFUSED: {name}: anchor is not unique')
            return 1
        with mutation_source.changed(path, original, original.replace(anchor, replacement, 1)):
            result = native.run(COMMAND, NAMES, [SOURCE])
        if result is None or watch.stopped:
            print(f'REFUSED: {name}: invalid native assertion producer')
            return 1
        if PREFIX + method in result.failure_names:
            killed += 1
            print(f'{name}: KILLED by {PREFIX + method} ({result.failed} assertion failures)', flush=True)
        else:
            print(f'{name}: SURVIVED', flush=True)
    print(f'{killed} of {len(MUTANTS)} mutants killed', flush=True)
    return int(killed != len(MUTANTS))


if __name__ == '__main__':
    sys.exit(main())
