# The device-store track's recipes (ADR-0366): a value the reader's device
# keeps, charter §9.1's fourth kind of state. The track adds its recipes here
# and nowhere else; the root justfile imports this file, and its settings and
# variables (`set shell`, `PATH`) apply here as there. An evidence recipe is
# `e14-<name>:`, writes `docs/evidence/E14/<name>.txt`, and is planned by the
# verification run like any other (scripts/ci_plan.py reads this file too).

# Track `device-store` (ADR-0366): a module's signal the device keeps, its
# clauses, versions and form, the `Device` label at each read and the sinks
# that keep it off the server (PW6801-PW6808), what the browser computes for
# a value it alone holds; the host's document of a page that holds one (the
# demo's and TodoMVC's); the browser in three engines (e2e/device-store.spec.mjs
# over examples/demo/device.pw, e2e/todomvc.spec.mjs); each engine's
# localStorage limit, measured; and the mutation controls
# (scripts/device_store_mutations.py).
e14-device-store:
    @BUILD_ONLY=1 bash spikes/own-renderer/run.sh > /dev/null
    @bash spikes/own-renderer/todomvc.sh > /dev/null
    @cargo build --quiet --locked -p pw-dev-server -p kiokun-server
    @mkdir -p docs/evidence/E14
    @{ echo "ADR-0366 - a value the reader's device keeps"; echo; \
       echo "produced by: just e14-device-store"; \
       echo "commit: $(git rev-parse HEAD)$(git diff --quiet HEAD -- . ':(exclude)docs/evidence' ':(exclude)spikes/own-renderer/store-ir.json' || echo ' + uncommitted changes')"; \
       echo "rust: $(rustc --version)"; echo "node: $(node --version)"; \
       echo "playwright: $(cd spikes/own-renderer && pnpm exec playwright --version)"; echo; \
       echo "== the compiler (compiler/pw-core/tests/device_store.rs)"; echo; \
       cargo test --locked -p pw-core --test device_store 2>&1 | grep -E '^(test |test result)|panicked at'; \
       echo; echo "== the host (tests::todomvc, tests::device_store: the document of a page that holds one, and its steps)"; echo; \
       cargo test --locked -p pw-dev-server -- tests::todomvc:: tests::device_store:: 2>&1 | grep -E '^(test |test result)|panicked at'; \
       echo; echo "== the browser in three engines (e2e/device-store.spec.mjs, e2e/todomvc.spec.mjs)"; echo; \
       bash scripts/run_suite.sh -- pnpm exec playwright test e2e/device-store.spec.mjs e2e/todomvc.spec.mjs --reporter=line; \
       echo; echo "== mutation controls (scripts/device_store_mutations.py)"; echo; \
       CARGO_INCREMENTAL=0 python3 scripts/device_store_mutations.py; \
     } > docs/evidence/E14/device-store.txt
    @grep -E "^test result|passed|mutants killed|^---- |panicked at" docs/evidence/E14/device-store.txt

# Track `device-store` (ADR-0366): TodoMVC's own Cypress suite, unchanged,
# over TodoMVC on the device (spikes/todomvc-cypress/run.sh). Its acceptance
# asks all 29 to pass, and is NOT met: the run says which pass and which do
# not. The five that fail fail for any app under the Cypress the suite is run
# with, which the probes show with no app (spikes/todomvc-cypress-probes/run.sh).
# This recipe records both. It fails where Cypress cannot run and where a
# probe does not hold (what the ADR says of the five would be wrong then); the
# acceptance's verdict is recorded as it is, and `just e14-todomvc` is the
# acceptance's own recipe.
e14-device-store-cypress:
    @bash spikes/own-renderer/todomvc.sh > /dev/null
    @cargo build --quiet --locked -p pw-dev-server
    @cd spikes/todomvc-cypress && pnpm exec cypress install > /dev/null
    @mkdir -p docs/evidence/E14
    @{ echo "ADR-0366 - TodoMVC's own Cypress suite over TodoMVC on the device, and the Cypress it runs in"; echo; \
       echo "produced by: just e14-device-store-cypress"; \
       echo "commit: $(git rev-parse HEAD)$(git diff --quiet HEAD -- . ':(exclude)docs/evidence' ':(exclude)spikes/own-renderer/store-ir.json' || echo ' + uncommitted changes')"; \
       echo "node: $(node --version)"; \
       echo "cypress: $(cd spikes/todomvc-cypress && pnpm exec cypress --version | tr '\n' ' ')"; echo; \
       echo "== the acceptance (spikes/todomvc-cypress/run.sh): all 29 must pass, and it is not met where it says FAILED"; echo; \
       bash spikes/todomvc-cypress/run.sh && echo "acceptance: exit 0" || echo "acceptance: exit $?: NOT MET"; \
       echo; echo "== the probes, with no app (spikes/todomvc-cypress-probes/run.sh)"; echo; \
       bash spikes/todomvc-cypress-probes/run.sh; \
     } > docs/evidence/E14/device-store-cypress.txt
    @grep -E "^acceptance|^probes|^(spy|alias):" docs/evidence/E14/device-store-cypress.txt

# Strict native test producers for the device campaign, including the pinned
# Playwright lifecycle without launching an application or browser.
e14-device-producer:
    @mkdir -p docs/evidence/E14
    @{ echo "ADR-0366 - complete assertion-only device mutation producers"; echo; \
       echo "produced by: just e14-device-producer"; \
       echo "commit: $(git rev-parse HEAD)"; \
       python3 -m unittest discover -s scripts/tests -p test_device_producer_results.py -v; \
       python3 scripts/device_browser_protocol.py; \
       python3 scripts/device_producer_mutations.py; \
     } > docs/evidence/E14/device-producer.txt 2>&1
    @tail -n 5 docs/evidence/E14/device-producer.txt
