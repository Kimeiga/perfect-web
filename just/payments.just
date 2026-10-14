# The payments track's recipes (NEXT 6c, the integrator's): a payment at the
# deployment's provider, settled only by the provider's word. The root
# justfile imports this file; an evidence recipe is `e14-<name>:`, writes
# `docs/evidence/E14/<name>.txt`, and is planned by the verification run
# like any other.

# NEXT 6c, step six: a command the platform invokes, which no reader presses
# (`invoked_by Payments`, PW64xx). The compiler's tests
# (tests/invoked_by.rs), the server's (payments::invoked_tests), and the
# mutation controls (scripts/invoked_by_mutations.py).
e14-invoked-by:
    @mkdir -p docs/evidence/E14
    @{ echo "NEXT 6c - a command the platform invokes, which no reader presses"; echo; \
       echo "produced by: just e14-invoked-by"; \
       echo "commit: $(git rev-parse HEAD)$(git diff --quiet HEAD -- . ':(exclude)docs/evidence' ':(exclude)spikes/own-renderer/store-ir.json' || echo ' + uncommitted changes')"; \
       echo "rust: $(rustc --version)"; echo; \
       echo "== the clause and its refusals (compiler/pw-core/tests/invoked_by.rs)"; echo; \
       cargo test --locked -p pw-core --test invoked_by 2>&1 | grep -E '^(test |test result)|panicked at'; \
       echo; echo "== a reader's press refused (spikes/own-renderer/server/src/payments.rs)"; echo; \
       cargo test --locked -p pw-dev-server -- invoked_tests 2>&1 | grep -E '^(test |test result)|panicked at'; \
       echo; echo "== mutation controls (scripts/invoked_by_mutations.py)"; echo; \
       python3 scripts/invoked_by_mutations.py; \
     } > docs/evidence/E14/invoked-by.txt
    @grep -E "^test result|mutants killed|panicked at" docs/evidence/E14/invoked-by.txt

# NEXT 6c, steps one to seven: a payment at the deployment's provider,
# settled only by the provider's word. The signature (HMAC-SHA256 held to
# RFC 4231), the development provider over the wire, the host's hearing of
# it, a payment begun once per interaction, and the press refused
# (payments.rs); the store's order awaiting its payment and settled on the
# reader's behalf, in memory and on PostgreSQL (tests/store_payments.rs);
# the reader paying in the provider's page the order page frames, in three
# engines (e2e/payments.spec.mjs); and the mutation controls
# (scripts/payments_mutations.py). It needs a throwaway PostgreSQL
# database, PW_STORE_DATABASE_URL (or PW_FEED_DATABASE_URL); it skips
# without one, and writes no evidence.
e14-payments:
    @url="${PW_STORE_DATABASE_URL:-${PW_FEED_DATABASE_URL:-}}"; \
     if [ -z "$url" ]; then \
       echo "e14-payments: skipped, PW_STORE_DATABASE_URL is not set (a throwaway PostgreSQL database, e.g. postgresql://localhost/pw_store_test)"; \
       exit 0; \
     fi; \
     export PW_STORE_DATABASE_URL="$url"; \
     mkdir -p docs/evidence/E14; \
     { echo "NEXT 6c - a payment at the deployment's provider, settled only by the provider's word"; echo; \
       echo "produced by: just e14-payments"; \
       echo "commit: $(git rev-parse HEAD)$(git diff --quiet HEAD -- . ':(exclude)docs/evidence' ':(exclude)spikes/own-renderer/store-ir.json' || echo ' + uncommitted changes')"; \
       echo "rust: $(rustc --version)"; \
       echo "postgres: $(psql "$url" -Atc 'SHOW server_version' 2>/dev/null || echo 'unknown (psql is not on PATH)')"; echo; \
       echo "== the signature, the provider, the hearing and a payment begun (payments.rs)"; echo; \
       PW_STORE_TEST_LAYER=memory cargo test --locked -p pw-dev-server -- payments:: --skip tests::store_payments:: 2>&1 | grep -E '^(test |test result)|panicked at'; \
       echo; echo "== an order awaiting its payment, the store in memory (tests/store_payments.rs)"; echo; \
       PW_STORE_TEST_LAYER=memory cargo test --locked -p pw-dev-server -- tests::store_payments:: 2>&1 | grep -E '^(test |test result)|panicked at'; \
       echo; echo "== the same, the store on PostgreSQL (PW_STORE_TEST_LAYER=postgres)"; echo; \
       PW_STORE_TEST_LAYER=postgres cargo test --locked -p pw-dev-server -- tests::store_payments:: 2>&1 | grep -E '^(test |test result)|panicked at'; \
       echo; echo "== the reader pays in the provider's page, in three engines (e2e/payments.spec.mjs)"; echo; \
       BUILD_ONLY=1 bash spikes/own-renderer/run.sh > /dev/null 2>&1; \
       cargo build --quiet --locked -p pw-dev-server -p kiokun-server; \
       (cd spikes/own-renderer && pnpm exec playwright test e2e/payments.spec.mjs --reporter=line 2>&1) \
         | sed 's/\x1b\[[0-9;]*[A-Za-z]//g' \
         | grep -E "^ +[0-9]+\) |Error:|^ +[0-9]+ (passed|failed|flaky|skipped|interrupted|did not run)" || true; \
       echo; echo "== mutation controls (scripts/payments_mutations.py)"; echo; \
       CARGO_INCREMENTAL=0 python3 scripts/payments_mutations.py; \
     } > docs/evidence/E14/payments.txt; \
     grep -E "^test result|mutants killed|^---- |panicked at|^ +[0-9]+ (passed|failed)" docs/evidence/E14/payments.txt
