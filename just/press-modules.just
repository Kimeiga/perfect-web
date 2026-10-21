# ADR-0382: the modules a page loads when it first needs them are loaded
# within the deadline, and a load that failed is not kept. A press's modules,
# a feed page's computed values, in three engines, and the mutation controls.
# A failed suite stops the recipe and stays in its evidence.
e14-press-modules:
    @BUILD_ONLY=1 bash spikes/own-renderer/run.sh > /dev/null
    @bash spikes/own-renderer/feed.sh > /dev/null
    @cargo build --quiet --locked -p pw-dev-server
    @mkdir -p docs/evidence/E14
    @{ echo "ADR-0382 - the modules a page loads when it first needs them are loaded within the deadline, and a failed load is not kept"; echo; \
       echo "produced by: just e14-press-modules"; \
       echo "commit: $(git rev-parse HEAD)$(git diff --quiet HEAD -- . ':(exclude)docs/evidence' ':(exclude)spikes/own-renderer/store-ir.json' || echo ' + uncommitted changes')"; \
       echo "node: $(node --version)"; \
       echo "playwright: $(cd spikes/own-renderer && pnpm exec playwright --version)"; echo; \
       echo "== a page's modules in three engines (e2e/press-modules.spec.mjs, e2e/computed-modules.spec.mjs)"; echo; \
       bash scripts/run_suite.sh -- pnpm exec playwright test e2e/press-modules.spec.mjs e2e/computed-modules.spec.mjs --reporter=line; \
       echo; echo "== mutation controls (scripts/press_modules_mutations.py)"; echo; \
       python3 scripts/press_modules_mutations.py; \
     } > docs/evidence/E14/press-modules.txt
    @grep -E "passed|failed|mutants killed" docs/evidence/E14/press-modules.txt
