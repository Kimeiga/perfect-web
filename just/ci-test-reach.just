# Changes to root Rust tests and Python witnesses must plan their consumers.
e14-ci-test-reach:
    @mkdir -p docs/evidence/E14
    @report="$(mktemp docs/evidence/E14/.ci-test-reach.XXXXXX)"; \
     trap 'status=$?; if [ "$status" -ne 0 ]; then cat "$report" >&2; fi; rm -f "$report"; exit "$status"' EXIT; \
     { echo "Changed tests reach the verification that consumes them"; echo; \
       echo "produced by: just e14-ci-test-reach"; \
       echo "commit: $(git rev-parse HEAD)$(git diff --quiet HEAD -- . ':(exclude)docs/evidence' || echo ' + uncommitted changes')"; \
       python3 -m unittest -v scripts/tests/test_ci_scripts.py; \
       python3 scripts/ci_test_reach_mutations.py; \
     } > "$report" 2>&1; \
     mv "$report" docs/evidence/E14/ci-test-reach.txt
    @cat docs/evidence/E14/ci-test-reach.txt
