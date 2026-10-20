#!/usr/bin/env python3
"""Query/discard producer controls; only completed unittest body assertions kill."""
from pathlib import Path
import sys

ROOT = Path(__file__).resolve().parent.parent
QUERY = ROOT / "scripts/query_values_mutations.py"
DISCARD = ROOT / "scripts/let_discard_mutations.py"
MUTANTS = [
    ("query results ignore the actual Cargo status", QUERY,
     "outcome = parse(r.returncode, r.stdout + r.stderr)",
     'outcome = parse(101 if "FAILED" in r.stdout + r.stderr else 0, r.stdout + r.stderr)'),
    ("discard results ignore the actual Cargo status", DISCARD,
     "outcome = parse(r.returncode, r.stdout + r.stderr)",
     'outcome = parse(101 if "FAILED" in r.stdout + r.stderr else 0, r.stdout + r.stderr)'),
    ("query invalid mutants pass the campaign", QUERY,
     '''            print(f"{what} [{kind}]: REFUSED (invalid Cargo producer)")
            return 1''',
     '''            print(f"{what} [{kind}]: REFUSED (invalid Cargo producer)")
            return 0'''),
    ("discard invalid mutants pass the campaign", DISCARD,
     '''            print(f"{what}: REFUSED (invalid Cargo producer)")
            return 1''',
     '''            print(f"{what}: REFUSED (invalid Cargo producer)")
            return 0'''),
]


def main():
    import guide_python
    selector = "test_generic_finish_producers"
    search = ROOT / "scripts/tests"
    expected = guide_python.freeze(selector, search, "test_generic_finish_producers.py", expected_count=5)
    baseline = guide_python.run(selector, expected, search)
    print(baseline.output)
    if not baseline.result.valid or baseline.result.failed or baseline.result.executed != len(expected):
        print("REFUSED: producer baseline is not green")
        return 1
    print(f"baseline: {len(baseline.result.passed)} passed, 0 failed")
    killed = 0
    for name, path, anchor, replacement in MUTANTS:
        original = path.read_text()
        if original.count(anchor) != 1:
            print(name + ": REFUSED (anchor not unique)")
            return 1
        try:
            path.write_text(original.replace(anchor, replacement, 1))
            run = guide_python.run(selector, expected, search)
        finally:
            path.write_text(original)
        print(run.output)
        if not run.result.valid or run.result.body_errors:
            print(name + ": REFUSED (invalid Python producer)")
            return 1
        if run.result.assertion_failed:
            killed += 1
            print(f"{name}: KILLED ({len(run.result.assertion_failed)} assertion methods fail)")
        else:
            print(name + ": SURVIVED")
    print(f"{killed} of {len(MUTANTS)} mutants killed")
    return int(killed != len(MUTANTS))


if __name__ == "__main__":
    sys.exit(main())
