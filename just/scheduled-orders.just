# Track `scheduled-orders`' recipes (W9, NEXT 4's last step): an order placed
# for a time its reader chooses. The root justfile imports this file; an
# evidence recipe is `e14-<name>:`, writes `docs/evidence/E14/<name>.txt`,
# and is planned by the verification run like any other.

# Track `scheduled-orders` (ADR-0347): the times offered, fifteen-minute
# windows from an hour to a week ahead that every store of the cart would
# take an order for as soon as possible at when its stores are told (the
# window less the estimate and five minutes), by its hours in its zone,
# compared with the clock; a time held to them
# when chosen and when the order is placed; an order for one paid now and
# sent to its stores when its preparation is due. The checker
# (compiler/pw-core/tests/scheduled_orders.rs), the server's tests
# (tests/scheduled_orders.rs, scheduled_orders.rs) in memory and on
# PostgreSQL, the store's statements counted (tests/bounded_statements.rs),
# the store in three engines (e2e/scheduled-orders.spec.mjs), and the
# mutation controls (scripts/scheduled_orders_mutations.py). It needs a
# throwaway PostgreSQL database, PW_STORE_DATABASE_URL (or
# PW_FEED_DATABASE_URL); it skips without one, and writes no evidence.
e14-scheduled-orders:
    @url="${PW_STORE_DATABASE_URL:-${PW_FEED_DATABASE_URL:-}}"; \
     if [ -z "$url" ]; then \
       echo "e14-scheduled-orders: skipped, PW_STORE_DATABASE_URL is not set (a throwaway PostgreSQL database, e.g. postgresql://localhost/pw_store_test)"; \
       exit 0; \
     fi; \
     export PW_STORE_DATABASE_URL="$url"; \
     mkdir -p docs/evidence/E14; \
     { echo "Track scheduled-orders - an order placed for a time its reader chooses"; echo; \
       echo "produced by: just e14-scheduled-orders"; \
       echo "commit: $(git rev-parse HEAD)$(git diff --quiet HEAD -- . ':(exclude)docs/evidence' ':(exclude)spikes/own-renderer/store-ir.json' || echo ' + uncommitted changes')"; \
       echo "rust: $(rustc --version)"; echo "node: $(node --version)"; \
       echo "postgres: $(psql "$url" -Atc 'SHOW server_version' 2>/dev/null || echo 'unknown (psql is not on PATH)')"; \
       echo "playwright: $(cd spikes/own-renderer && pnpm exec playwright --version)"; echo; \
       echo "== the clock's arithmetic and the times a page cannot compute (compiler/pw-core/tests/scheduled_orders.rs)"; echo; \
       cargo test --locked -p pw-core --test scheduled_orders 2>&1 | grep -E '^(test |test result)|panicked at'; \
       echo; echo "== the times offered and an order for one, the store in memory (tests/scheduled_orders.rs, scheduled_orders.rs)"; echo; \
       PW_STORE_TEST_LAYER=memory cargo test --locked -p pw-dev-server -- tests::scheduled_orders:: scheduled_orders::tests 2>&1 | grep -E '^(test |test result)|panicked at'; \
       echo; echo "== the same, the store on PostgreSQL (PW_STORE_TEST_LAYER=postgres)"; echo; \
       PW_STORE_TEST_LAYER=postgres cargo test --locked -p pw-dev-server -- tests::scheduled_orders:: 2>&1 | grep -E '^(test |test result)|panicked at'; \
       echo; echo "== each operation, page and command counted on PostgreSQL (tests/bounded_statements.rs)"; echo; \
       PW_STORE_TEST_LAYER=postgres cargo test --locked -p pw-dev-server -- tests::bounded_statements:: 2>&1 | grep -E '^(test |test result)|panicked at'; \
       echo; echo "== the store in three engines (e2e/scheduled-orders.spec.mjs)"; echo; \
       BUILD_ONLY=1 bash spikes/own-renderer/run.sh > /dev/null 2>&1; \
       cargo build --quiet --locked -p pw-dev-server -p kiokun-server; \
       bash scripts/run_suite.sh -- pnpm exec playwright test e2e/scheduled-orders.spec.mjs --reporter=line; \
       echo; echo "== mutation controls (scripts/scheduled_orders_mutations.py)"; echo; \
       CARGO_INCREMENTAL=0 python3 scripts/scheduled_orders_mutations.py; \
     } > docs/evidence/E14/scheduled-orders.txt; \
     grep -E "^test result|mutants killed|^---- |panicked at|memory bound|^ +[0-9]+ (passed|failed)" docs/evidence/E14/scheduled-orders.txt
