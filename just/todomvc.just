# The todomvc track's recipes (ADR-0253, docs/PARALLEL.md, W7's plan): TodoMVC
# in Pleris, held to TodoMVC's own spec and official tests. The track adds
# its recipes here and nowhere else; the root justfile imports this file, and
# its settings and variables (`set shell`, `PATH`) apply here as there. An
# evidence recipe is `e14-<name>:`, writes `docs/evidence/E14/<name>.txt`, and
# is planned by the verification run like any other (scripts/ci_plan.py reads
# this file too).

# Track `todomvc` (ADR-XXXX): a page binds a signal to its address's
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
    @{ echo "ADR-XXXX - a page binds a signal to its address's fragment"; echo; \
       echo "produced by: just e14-address-fragment"; \
       echo "commit: $(git rev-parse HEAD)$(git diff --quiet HEAD -- . ':(exclude)docs/evidence' ':(exclude)spikes/own-renderer/store-ir.json' || echo ' + uncommitted changes')"; \
       echo "rust: $(rustc --version)"; echo "node: $(node --version)"; \
       echo "playwright: $(cd spikes/own-renderer && pnpm exec playwright --version)"; echo; \
       echo "== the compiler (tests/address_fragment.rs, the policy table's tests)"; echo; \
       cargo test --locked -p pw-core --test address_fragment 2>&1 | grep -E '^(test |test result)|panicked at'; \
       cargo test --locked -p pw-core --lib policy:: codes:: 2>&1 | grep -E '^(test |test result)|panicked at'; \
       echo; echo "== the host (tests::address_fragment)"; echo; \
       cargo test --locked -p pw-dev-server -- tests::address_fragment:: 2>&1 | grep -E '^(test |test result)|panicked at'; \
       echo; echo "== the browser in three engines (e2e/fragment.spec.mjs)"; echo; \
       (cd spikes/own-renderer && pnpm exec playwright test e2e/fragment.spec.mjs --reporter=line 2>&1) \
         | sed 's/\x1b\[[0-9;]*[A-Za-z]//g' \
         | grep -E "^ +[0-9]+\) |Error:|^ +[0-9]+ (passed|failed|flaky|skipped|interrupted|did not run)" || true; \
       echo; echo "== mutation controls (scripts/address_fragment_mutations.py)"; echo; \
       CARGO_INCREMENTAL=0 python3 scripts/address_fragment_mutations.py; \
     } > docs/evidence/E14/address-fragment.txt
    @grep -E "^test result|passed|mutants killed|^---- |panicked at" docs/evidence/E14/address-fragment.txt

# Track `todomvc` (ADR-XXXX): the platform declares a double press
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
    @{ echo "ADR-XXXX - a double press and a control's losing focus are events the platform declares"; echo; \
       echo "produced by: just e14-dom-events"; \
       echo "commit: $(git rev-parse HEAD)$(git diff --quiet HEAD -- . ':(exclude)docs/evidence' ':(exclude)spikes/own-renderer/store-ir.json' || echo ' + uncommitted changes')"; \
       echo "rust: $(rustc --version)"; echo "node: $(node --version)"; \
       echo "playwright: $(cd spikes/own-renderer && pnpm exec playwright --version)"; echo; \
       echo "== the compiler (tests/declared_events.rs, events.rs, platform_contracts.rs)"; echo; \
       cargo test --locked -p pw-core --test declared_events --test events --test platform_contracts 2>&1 | grep -E '^(test |test result)|panicked at'; \
       echo; echo "== the browser in three engines (e2e/events.spec.mjs)"; echo; \
       (cd spikes/own-renderer && pnpm exec playwright test e2e/events.spec.mjs --reporter=line 2>&1) \
         | sed 's/\x1b\[[0-9;]*[A-Za-z]//g' \
         | grep -E "^ +[0-9]+\) |Error:|^ +[0-9]+ (passed|failed|flaky|skipped|interrupted|did not run)" || true; \
       echo; echo "== mutation controls (scripts/dom_events_mutations.py)"; echo; \
       CARGO_INCREMENTAL=0 python3 scripts/dom_events_mutations.py; \
     } > docs/evidence/E14/dom-events.txt
    @grep -E "^test result|passed|mutants killed|^---- |panicked at" docs/evidence/E14/dom-events.txt

# Track `todomvc` (ADR-XXXX): TodoMVC in Pleris, held to TodoMVC's own spec
# (`app-spec.md`): every behaviour it states, each with its test. The program
# (examples/todomvc) and its data layer's tests on the host (tests::todomvc::),
# the browser in three engines over the program built by
# spikes/own-renderer/todomvc.sh and served on a host per engine
# (e2e/todomvc.spec.mjs), and the mutation controls
# (scripts/todomvc_mutations.py), each program mutant checked to be a
# well-typed program.
e14-todomvc:
    @bash spikes/own-renderer/todomvc.sh > /dev/null
    @cargo build --quiet --locked -p pw-dev-server -p kiokun-server
    @mkdir -p docs/evidence/E14
    @{ echo "ADR-XXXX - TodoMVC in Pleris, held to its own spec and its own tests"; echo; \
       echo "produced by: just e14-todomvc"; \
       echo "commit: $(git rev-parse HEAD)$(git diff --quiet HEAD -- . ':(exclude)docs/evidence' ':(exclude)spikes/own-renderer/store-ir.json' || echo ' + uncommitted changes')"; \
       echo "rust: $(rustc --version)"; echo "node: $(node --version)"; \
       echo "playwright: $(cd spikes/own-renderer && pnpm exec playwright --version)"; \
       echo "todomvc-app-css: $(node -p "require('./spikes/own-renderer/node_modules/todomvc-app-css/package.json').version") ($(node -p "require('./spikes/own-renderer/node_modules/todomvc-app-css/package.json').license"))"; \
       echo "todomvc-common: $(node -p "require('./spikes/own-renderer/node_modules/todomvc-common/package.json').version") ($(node -p "require('./spikes/own-renderer/node_modules/todomvc-common/package.json').license"))"; echo; \
       echo "== the host: a session's todos, the commands over them, and the page (tests::todomvc::)"; echo; \
       cargo test --locked -p pw-dev-server -- tests::todomvc:: 2>&1 | grep -E '^(test |test result)|panicked at'; \
       echo; echo "== the browser in three engines (e2e/todomvc.spec.mjs)"; echo; \
       (cd spikes/own-renderer && pnpm exec playwright test e2e/todomvc.spec.mjs --reporter=line 2>&1) \
         | sed 's/\x1b\[[0-9;]*[A-Za-z]//g' \
         | grep -E "^ +[0-9]+\) |Error:|^ +[0-9]+ (passed|failed|flaky|skipped|interrupted|did not run)" || true; \
       echo; echo "== mutation controls (scripts/todomvc_mutations.py)"; echo; \
       CARGO_INCREMENTAL=0 python3 scripts/todomvc_mutations.py; \
     } > docs/evidence/E14/todomvc.txt
    @grep -E "^test result|passed|mutants killed|^---- |panicked at" docs/evidence/E14/todomvc.txt
