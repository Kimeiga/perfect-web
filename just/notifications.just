# The notifications track's recipes (ADR-0253, docs/PARALLEL.md): the typed
# principal, and notifications on it. The track adds its recipes here and
# nowhere else; the root justfile imports this file, and its settings and
# variables (`set shell`, `PATH`) apply here as there. An evidence recipe is
# `e14-<name>:`, writes `docs/evidence/E14/<name>.txt`, and is planned by the
# verification run like any other (scripts/ci_plan.py reads this file too).

# Track `notifications` (ADR-0274): the reader's user the host's, a
# listener's handle binding an event's id (ADR-0091 as amended), and what
# others do that involves you. The compiler's tests
# (compiler/pw-core/tests/principal.rs); the server's (notifications.rs and
# tests/notifications.rs, on PostgreSQL too where PW_FEED_DATABASE_URL names
# a throwaway database, passing doing nothing otherwise); the browser in
# three engines (e2e/notifications.spec.mjs, PW_IDENTITY=dev-accounts); and
# the mutation controls (scripts/notifications_mutations.py).
e14-notifications:
    @BUILD_ONLY=1 bash spikes/own-renderer/run.sh > /dev/null
    @bash spikes/own-renderer/feed.sh > /dev/null
    @cargo build --quiet --locked -p pw-dev-server -p kiokun-server
    @mkdir -p docs/evidence/E14
    @{ echo "ADR-0270 and ADR-0274 - the reader's user the host's, and notifications on it"; echo; \
       echo "produced by: just e14-notifications"; \
       echo "commit: $(git rev-parse HEAD)$(git diff --quiet HEAD -- . ':(exclude)docs/evidence' ':(exclude)spikes/own-renderer/store-ir.json' || echo ' + uncommitted changes')"; \
       echo "rust: $(rustc --version)"; echo "node: $(node --version)"; \
       echo "playwright: $(cd spikes/own-renderer && pnpm exec playwright --version)"; \
       if [ -n "${PW_FEED_DATABASE_URL:-}" ]; then \
         echo "postgres: $(psql "$PW_FEED_DATABASE_URL" -Atc 'SHOW server_version' 2>/dev/null || echo 'unknown (psql is not on PATH)')"; \
       else echo "postgres: PW_FEED_DATABASE_URL is not set, so the PostgreSQL tests pass doing nothing"; fi; echo; \
       echo "== the compiler (compiler/pw-core/tests/principal.rs)"; echo; \
       cargo test --locked -p pw-core --test principal 2>&1 | awk '/^(test |test result)/ { print } /panicked at/ { p = 5 } p > 0 { print; p-- }'; \
       echo; echo "== the server (notifications.rs, tests/notifications.rs)"; echo; \
       cargo test --locked -p pw-dev-server -- notifications 2>&1 | awk '/^(test |test result)/ { print } /panicked at/ { p = 5 } p > 0 { print; p-- }'; \
       echo; echo "== the browser in three engines (e2e/notifications.spec.mjs)"; echo; \
       (cd spikes/own-renderer && pnpm exec playwright test e2e/notifications.spec.mjs --reporter=line 2>&1) \
         | sed 's/\x1b\[[0-9;]*[A-Za-z]//g' \
         | grep -E "^ +[0-9]+\) |Error:|^ +[0-9]+ (passed|failed|flaky|skipped|interrupted|did not run)" || true; \
       echo; echo "== mutation controls (scripts/notifications_mutations.py)"; echo; \
       CARGO_INCREMENTAL=0 python3 scripts/notifications_mutations.py; \
     } > docs/evidence/E14/notifications.txt
    @grep -E "^test result|passed|mutants killed|^---- |panicked at" docs/evidence/E14/notifications.txt
