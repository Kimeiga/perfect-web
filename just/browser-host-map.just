# The browser host map's recipes (ADR-XXXX): which development servers a
# browser run starts. The root justfile imports this file; an evidence recipe
# is `e14-<name>:`, writes `docs/evidence/E14/<name>.txt`, and is planned by
# the verification run like any other.

# ADR-XXXX: each browser run starts only the hosts its selected tests need,
# by the map of the suite's hosts (spikes/own-renderer/host-registry.json,
# host-metadata.mjs), the selection asked of Playwright itself
# (host-plan.mjs). The map held to every spec, the registry, the build
# policy and the receipts (host-plan.test.mjs, also in `just ci`); the
# planner against the real Playwright CLI on private fixtures
# (host-plan.native.test.mjs); three specs of three kinds of host in three
# engines, each run on its own hosts; and the mutation controls
# (scripts/browser_host_map_mutations.py).
e14-browser-host-map:
    @BUILD_ONLY=1 bash spikes/own-renderer/run.sh > /dev/null
    @bash spikes/own-renderer/feed.sh > /dev/null
    @cargo build --quiet --locked -p pw-dev-server
    @mkdir -p docs/evidence/E14
    @{ echo "ADR-XXXX - the browser host map: a run starts only the hosts its selected tests need"; echo; \
       echo "produced by: just e14-browser-host-map"; \
       echo "commit: $(git rev-parse HEAD)$(git diff --quiet HEAD -- . ':(exclude)docs/evidence' ':(exclude)spikes/own-renderer/store-ir.json' || echo ' + uncommitted changes')"; \
       echo "node: $(node --version)"; \
       echo "playwright: $(cd spikes/own-renderer && pnpm exec playwright --version)"; echo; \
       echo "== the map held to every spec, the registry, the builds, the declared hosts, the receipts (host-plan.test.mjs)"; echo; \
       (cd spikes/own-renderer && node --test --test-reporter=tap host-plan.test.mjs) 2>&1 | grep -E '^(ok|not ok) |^# (tests|pass|fail) '; \
       echo; echo "== the plan against the real Playwright CLI, on private fixtures (host-plan.native.test.mjs)"; echo; \
       (cd spikes/own-renderer && node --test --test-reporter=tap host-plan.native.test.mjs) 2>&1 | grep -E '^(ok|not ok) |^# (tests|pass|fail) '; \
       echo; echo "== a spec on the shared host, one on a mutating suite's, one on the feed's, in three engines"; echo; \
       bash scripts/run_suite.sh -- pnpm exec playwright test e2e/signals.spec.mjs e2e/cart.spec.mjs e2e/feed.spec.mjs --reporter=line; \
       echo; echo "== mutation controls (scripts/browser_host_map_mutations.py)"; echo; \
       python3 scripts/browser_host_map_mutations.py; \
     } > docs/evidence/E14/browser-host-map.txt
    @grep -E "^# (tests|pass|fail) |^ +[0-9]+ (passed|failed|flaky|skipped)|mutants killed|^baseline|NOT KILLED|SURVIVED|^FAIL" docs/evidence/E14/browser-host-map.txt

# The integrator's ruling of 2026-10-19 (train-11): a spec whose optional
# build is missing is left out and said, never refused nor left out silently,
# and each spec an optional build can leave out runs in at least one recipe
# that makes that build (scripts/skipped_specs.py; its tests, in `just ci`,
# scripts/tests/test_skipped_specs.py); and the mutation controls, each
# undoing one part of the check (scripts/skipped_specs_mutations.py).
e14-skipped-specs:
    @mkdir -p docs/evidence/E14
    @{ echo "Train-11 - every spec an optional build can leave out runs where that build is made"; echo; \
       echo "produced by: just e14-skipped-specs"; \
       echo "commit: $(git rev-parse HEAD)$(git diff --quiet HEAD -- . ':(exclude)docs/evidence' || echo ' + uncommitted changes')"; \
       echo "python: $(python3 --version)"; echo; \
       echo "== the check (scripts/skipped_specs.py) and its tests (scripts/tests/test_skipped_specs.py)"; echo; \
       python3 scripts/skipped_specs.py; \
       python3 -m unittest -v scripts/tests/test_skipped_specs.py 2>&1 | grep -E ' \.\.\. |^Ran |^OK|^FAILED'; \
       echo; echo "== mutation controls (scripts/skipped_specs_mutations.py)"; echo; \
       python3 scripts/skipped_specs_mutations.py; \
     } > docs/evidence/E14/skipped-specs.txt
    @grep -E "^Ran |^OK|^FAILED|mutants killed" docs/evidence/E14/skipped-specs.txt
