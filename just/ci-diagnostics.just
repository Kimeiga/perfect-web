# Track `ci-diagnostics`'s recipes (two CI-reliability defects, found
# 2026-10-19). The root justfile imports this file; an evidence recipe is
# `e14-<name>:`, writes `docs/evidence/E14/<name>.txt`, and is planned by the
# verification run like any other.

# Track `ci-diagnostics` (ADR-XXXX): a failing browser run's trace is kept
# where the next run does not clear it (scripts/keep_traces.py, called by
# scripts/run_suite.sh and by a red mutation baseline), bounded in size, and
# uploaded by verify.yml, a mutation control's intended failures not among
# them. The tests of each, and the mutation controls
# (scripts/ci_diagnostics_mutations.py).
e14-ci-diagnostics:
    @mkdir -p docs/evidence/E14
    @report="$(mktemp docs/evidence/E14/.ci-diagnostics.XXXXXX)"; \
     trap 'status=$?; if [ "$status" -ne 0 ]; then cat "$report" >&2; fi; rm -f "$report"; exit "$status"' EXIT; \
     { echo "Track ci-diagnostics - a failure's trace is kept, and the checker's latency is measured under load"; echo; \
       echo "produced by: just e14-ci-diagnostics"; \
       echo "commit: $(git rev-parse HEAD)$(git diff --quiet HEAD -- . ':(exclude)docs/evidence' ':(exclude)spikes/own-renderer/store-ir.json' || echo ' + uncommitted changes')"; \
       echo "rust: $(rustc --version)"; echo "python: $(python3 --version)"; echo; \
       echo "== 1. a failing browser run's output is kept where the next run does not clear it (scripts/keep_traces.py, run_suite.sh, mutation_baseline.py, ci_recipes.py, verify.yml)"; echo; \
       python3 -m unittest -v scripts/tests/test_keep_traces.py scripts/tests/test_run_suite.py scripts/tests/test_mutation_baseline.py 2>&1 | grep -E ' \.\.\. |^Ran |^OK|^FAILED'; \
       echo; echo "== mutation controls (scripts/ci_diagnostics_mutations.py)"; echo; \
       python3 scripts/ci_diagnostics_mutations.py; \
     } > "$report"; \
     mv "$report" docs/evidence/E14/ci-diagnostics.txt
    @grep -E "^Ran |^OK|^FAILED|mutants killed|^---- |panicked at|memory bound" docs/evidence/E14/ci-diagnostics.txt
