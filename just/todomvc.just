# The todomvc track's recipes (ADR-0253, docs/PARALLEL.md, W7's plan): TodoMVC
# in Pleris, held to TodoMVC's own spec and official tests. The track adds
# its recipes here and nowhere else; the root justfile imports this file, and
# its settings and variables (`set shell`, `PATH`) apply here as there. An
# evidence recipe is `e14-<name>:`, writes `docs/evidence/E14/<name>.txt`, and
# is planned by the verification run like any other (scripts/ci_plan.py reads
# this file too).

# Track `todomvc` (ADR-0367): a page binds a signal to its address's
# fragment, which no server sees (`fragment filter "/" => "all", ..`), and the
# runtime keeps the two in step. The compiler's checks (tests/address_fragment.rs,
# PW6101 to PW6106, and the clause's own tests among the policy table's), the
# host's (tests::address_fragment), the browser in three engines
# (e2e/fragment.spec.mjs over examples/demo/fragment.pw), and the mutation
# controls (scripts/address_fragment_mutations.py).
e14-address-fragment:
    @BUILD_ONLY=1 bash spikes/own-renderer/run.sh > /dev/null
    @cargo build --quiet --locked -p pw-dev-server -p kiokun-server
    @mkdir -p docs/evidence/E14
    @{ echo "ADR-0367 - a page binds a signal to its address's fragment"; echo; \
       echo "produced by: just e14-address-fragment"; \
       echo "commit: $(git rev-parse HEAD)$(git diff --quiet HEAD -- . ':(exclude)docs/evidence' ':(exclude)spikes/own-renderer/store-ir.json' || echo ' + uncommitted changes')"; \
       echo "rust: $(rustc --version)"; echo "node: $(node --version)"; \
       echo "playwright: $(cd spikes/own-renderer && pnpm exec playwright --version)"; echo; \
       echo "== the compiler (tests/address_fragment.rs, the policy table's tests)"; echo; \
       cargo test --locked -p pw-core --test address_fragment 2>&1 | grep -E '^(test |test result)|panicked at'; \
       cargo test --locked -p pw-core --lib -- policy:: codes:: 2>&1 | grep -E '^(test |test result)|panicked at'; \
       echo; echo "== the host (tests::address_fragment)"; echo; \
       cargo test --locked -p pw-dev-server -- tests::address_fragment:: 2>&1 | grep -E '^(test |test result)|panicked at'; \
       echo; echo "== the browser in three engines (e2e/fragment.spec.mjs)"; echo; \
       bash scripts/run_suite.sh -- pnpm exec playwright test e2e/fragment.spec.mjs --reporter=line; \
       echo; echo "== mutation controls (scripts/address_fragment_mutations.py)"; echo; \
       CARGO_INCREMENTAL=0 python3 scripts/address_fragment_mutations.py; \
     } > docs/evidence/E14/address-fragment.txt
    @grep -E "^test result|passed|mutants killed|^---- |panicked at" docs/evidence/E14/address-fragment.txt

# Track `todomvc` (ADR-0367): the platform declares a double press
# (`on:dblclick`) and a control's losing focus (`on:blur`), which the runtime
# listens for and gives its handler the record it reads in the listener: where
# the pointer was, and the value the control held. The compiler's tests
# (tests/declared_events.rs, events.rs, platform_contracts.rs), the browser in
# three engines (e2e/events.spec.mjs over examples/demo/events.pw), and the
# mutation controls (scripts/dom_events_mutations.py).
e14-dom-events:
    @BUILD_ONLY=1 bash spikes/own-renderer/run.sh > /dev/null
    @cargo build --quiet --locked -p pw-dev-server -p kiokun-server
    @mkdir -p docs/evidence/E14
    @{ echo "ADR-0367 - a double press and a control's losing focus are events the platform declares"; echo; \
       echo "produced by: just e14-dom-events"; \
       echo "commit: $(git rev-parse HEAD)$(git diff --quiet HEAD -- . ':(exclude)docs/evidence' ':(exclude)spikes/own-renderer/store-ir.json' || echo ' + uncommitted changes')"; \
       echo "rust: $(rustc --version)"; echo "node: $(node --version)"; \
       echo "playwright: $(cd spikes/own-renderer && pnpm exec playwright --version)"; echo; \
       echo "== the compiler (tests/declared_events.rs, events.rs, platform_contracts.rs)"; echo; \
       cargo test --locked -p pw-core --test declared_events --test events --test platform_contracts 2>&1 | grep -E '^(test |test result)|panicked at'; \
       echo; echo "== the browser in three engines (e2e/events.spec.mjs)"; echo; \
       bash scripts/run_suite.sh -- pnpm exec playwright test e2e/events.spec.mjs --reporter=line; \
       echo; echo "== mutation controls (scripts/dom_events_mutations.py)"; echo; \
       CARGO_INCREMENTAL=0 python3 scripts/dom_events_mutations.py; \
     } > docs/evidence/E14/dom-events.txt
    @grep -E "^test result|passed|mutants killed|^---- |panicked at" docs/evidence/E14/dom-events.txt

# Track `todomvc` (ADR-0367): an element a block shows with `autofocus`, after
# the page has loaded, takes focus as it appears, as TodoMVC's edit field
# does (HTML runs `autofocus` once, when a page loads). The browser in three
# engines (e2e/focus.spec.mjs over examples/demo/focus.pw) and the mutation
# controls (scripts/autofocus_mutations.py); TodoMVC's own edit field is
# held in e14-todomvc.
e14-autofocus:
    @BUILD_ONLY=1 bash spikes/own-renderer/run.sh > /dev/null
    @cargo build --quiet --locked -p pw-dev-server -p kiokun-server
    @mkdir -p docs/evidence/E14
    @{ echo "ADR-0367 - an element a block shows with autofocus takes focus as it appears"; echo; \
       echo "produced by: just e14-autofocus"; \
       echo "commit: $(git rev-parse HEAD)$(git diff --quiet HEAD -- . ':(exclude)docs/evidence' ':(exclude)spikes/own-renderer/store-ir.json' || echo ' + uncommitted changes')"; \
       echo "rust: $(rustc --version)"; echo "node: $(node --version)"; \
       echo "playwright: $(cd spikes/own-renderer && pnpm exec playwright --version)"; echo; \
       echo "== the browser in three engines (e2e/focus.spec.mjs)"; echo; \
       bash scripts/run_suite.sh -- pnpm exec playwright test e2e/focus.spec.mjs --reporter=line; \
       echo; echo "== mutation controls (scripts/autofocus_mutations.py)"; echo; \
       CARGO_INCREMENTAL=0 python3 scripts/autofocus_mutations.py; \
     } > docs/evidence/E14/autofocus.txt
    @grep -E "^test result|passed|mutants killed|^---- |panicked at" docs/evidence/E14/autofocus.txt

# Track `todomvc` (ADR-0367): TodoMVC in Pleris, held to TodoMVC's own spec
# (`app-spec.md`): every behaviour it states, each with its test. The program
# (examples/todomvc) and its document's tests on the host (tests::todomvc::;
# its todos are the reader's device's since track `device-store`, ADR-0366),
# the browser in three engines over the program built by
# spikes/own-renderer/todomvc.sh and served on a host per engine
# (e2e/todomvc.spec.mjs), and the mutation controls
# (scripts/todomvc_mutations.py), each program mutant checked to be a
# well-typed program. TodoMVC's seven Cypress originals remain vendored
# unchanged; an explicit compatibility v1 execution copy adapts two tool
# mismatches (spikes/todomvc-cypress/UPSTREAM.md). Run it against the app twice:
# acceptance (all 29 must pass) and, labelled as not the
# acceptance, under a name its own tables take as keeping nothing in
# localStorage. Each run is judged by scripts/todomvc_cypress_result.py from
# what Cypress printed. THIS RECIPE FAILS where Cypress cannot run (no Xvfb, no
# binary, no results) or any acceptance test fails or skips. The diagnostic
# explains observations only. It fetches Cypress's binary into
# Cypress's own cache (`cypress install` does nothing where it is there). The
# Cypress suite is not part of `just ci`.
e14-todomvc:
    @BUILD_ONLY=1 bash spikes/own-renderer/run.sh > /dev/null
    @bash spikes/own-renderer/todomvc.sh > /dev/null
    @cargo build --quiet --locked -p pw-dev-server -p kiokun-server
    @cd spikes/todomvc-cypress && pnpm exec cypress install > /dev/null
    @mkdir -p docs/evidence/E14
    @{ echo "ADR-0367 - TodoMVC in Pleris, held to its own spec and its own tests"; echo; \
       echo "produced by: just e14-todomvc"; \
       echo "commit: $(git rev-parse HEAD)$(git diff --quiet HEAD -- . ':(exclude)docs/evidence' ':(exclude)spikes/own-renderer/store-ir.json' || echo ' + uncommitted changes')"; \
       echo "rust: $(rustc --version)"; echo "node: $(node --version)"; \
       echo "playwright: $(cd spikes/own-renderer && pnpm exec playwright --version)"; \
       echo "todomvc-app-css: $(node -p "require('./spikes/own-renderer/node_modules/todomvc-app-css/package.json').version") ($(node -p "require('./spikes/own-renderer/node_modules/todomvc-app-css/package.json').license"))"; \
       echo "todomvc-common: $(node -p "require('./spikes/own-renderer/node_modules/todomvc-common/package.json').version") ($(node -p "require('./spikes/own-renderer/node_modules/todomvc-common/package.json').license"))"; \
       echo "cypress: $(cd spikes/todomvc-cypress && pnpm exec cypress --version | tr '\n' ' ')"; echo; \
       echo "== the host: the page's document, its defaults and what the device decides (tests::todomvc::)"; echo; \
       cargo test --locked -p pw-dev-server -- tests::todomvc:: 2>&1 | grep -E '^(test |test result)|panicked at'; \
       echo; echo "== the browser in three engines (e2e/todomvc.spec.mjs)"; echo; \
       bash scripts/run_suite.sh -- pnpm exec playwright test e2e/todomvc.spec.mjs --reporter=line; \
       echo; echo "== compatibility v1 identity and drift controls (original seven upstream blobs remain unchanged)"; echo; \
       python3 scripts/todomvc_compat_mutations.py; \
       echo; echo "== TodoMVC Cypress compatibility v1: the 29-test acceptance (spikes/todomvc-cypress/run.sh)"; echo; \
       bash spikes/todomvc-cypress/run.sh; \
       echo; echo "== TodoMVC Cypress compatibility v1: the diagnostic, not the acceptance (DIAGNOSTIC=1)"; echo; \
       DIAGNOSTIC=1 bash spikes/todomvc-cypress/run.sh; \
       echo; echo "== mutation controls: the Cypress acceptance's checks (scripts/todomvc_cypress_mutations.py)"; echo; \
       python3 scripts/todomvc_cypress_mutations.py; \
       echo; echo "== native mutation-result authority controls (scripts/todomvc_producer_mutations.py)"; echo; \
       python3 scripts/todomvc_producer_mutations.py; \
       echo; echo "== mutation controls (scripts/todomvc_mutations.py)"; echo; \
       CARGO_INCREMENTAL=0 python3 scripts/todomvc_mutations.py; \
     } > docs/evidence/E14/todomvc.txt
    @grep -E "^test result|passed|mutants killed|^---- |panicked at" docs/evidence/E14/todomvc.txt
