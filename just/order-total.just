# The order-total track's recipes (NEXT's item 6, step 6b; W9): an order's
# total. The root justfile imports this file; an
# evidence recipe is `e14-<name>:`, writes `docs/evidence/E14/<name>.txt`,
# and is planned by the verification run like any other.

# Track `order-total` (ADR-0329): each store's fees and tax, a promo code
# and a tip, priced by the program from the store's data (ADR-0327's money),
# itemized on the cart page and charged as it was shown. The server's tests
# (tests/order_total.rs), the store in memory and on PostgreSQL, and the
# checkout's own on PostgreSQL (one statement for a cart's fees, a code used
# once by two orders at once); the cart and the order in three engines
# (e2e/order-total.spec.mjs); and the mutation controls
# (scripts/order_total_mutations.py). It needs a
# throwaway PostgreSQL database, PW_STORE_DATABASE_URL (or
# PW_FEED_DATABASE_URL); it skips without one, and writes no evidence.
e14-order-total:
    @url="${PW_STORE_DATABASE_URL:-${PW_FEED_DATABASE_URL:-}}"; \
     if [ -z "$url" ]; then \
       echo "e14-order-total: skipped, PW_STORE_DATABASE_URL is not set (a throwaway PostgreSQL database, e.g. postgresql://localhost/pw_store_test)"; \
       exit 0; \
     fi; \
     export PW_STORE_DATABASE_URL="$url"; \
     mkdir -p docs/evidence/E14; \
     { echo "Track order-total - an order's total: each store's fees and tax, a promo code and a tip"; echo; \
       echo "produced by: just e14-order-total"; \
       echo "commit: $(git rev-parse HEAD)$(git diff --quiet HEAD -- . ':(exclude)docs/evidence' ':(exclude)spikes/own-renderer/store-ir.json' || echo ' + uncommitted changes')"; \
       echo "rust: $(rustc --version)"; \
       echo "postgres: $(psql "$url" -Atc 'SHOW server_version' 2>/dev/null || echo 'unknown (psql is not on PATH)')"; echo; \
       echo "== the total, the store in memory (tests/order_total.rs)"; echo; \
       PW_STORE_TEST_LAYER=memory cargo test --locked -p pw-dev-server -- tests::order_total:: 2>&1 | grep -E '^(test |test result)|panicked at'; \
       echo; echo "== the total, the store on PostgreSQL (PW_STORE_TEST_LAYER=postgres), and the checkout's own (checkout.rs)"; echo; \
       PW_STORE_TEST_LAYER=postgres cargo test --locked -p pw-dev-server -- tests::order_total:: checkout::tests:: 2>&1 | grep -E '^(test |test result)|panicked at'; \
       echo; echo "== the cart and the order in three engines (e2e/order-total.spec.mjs)"; echo; \
       BUILD_ONLY=1 bash spikes/own-renderer/run.sh > /dev/null 2>&1; \
       cargo build --quiet --locked -p pw-dev-server -p kiokun-server; \
       (cd spikes/own-renderer && pnpm exec playwright test e2e/order-total.spec.mjs --reporter=line 2>&1) \
         | sed 's/\x1b\[[0-9;]*[A-Za-z]//g' \
         | grep -E "^ +[0-9]+\) |Error:|^ +[0-9]+ (passed|failed|flaky|skipped|interrupted|did not run)" || true; \
       echo; echo "== mutation controls (scripts/order_total_mutations.py)"; echo; \
       CARGO_INCREMENTAL=0 python3 scripts/order_total_mutations.py; \
     } > docs/evidence/E14/order-total.txt; \
     grep -E "^test result|mutants killed|^---- |panicked at|memory bound|^ +[0-9]+ (passed|failed)" docs/evidence/E14/order-total.txt
