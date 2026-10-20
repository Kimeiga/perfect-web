# Track `second-host`'s recipes (ADR-0253, docs/PARALLEL.md): hosts that
# share one PostgreSQL database. The track adds its recipes here and nowhere
# else; the root justfile imports this file, and its settings and variables
# (`set shell`, `PATH`) apply here as there. An evidence recipe is
# `e14-<name>:`, writes `docs/evidence/E14/<name>.txt`, and is planned by the
# verification run like any other (scripts/ci_plan.py reads this file too).

# Track `second-host` (ADR-XXXX): hosts on one database hear each other's
# commits from the outbox, answer each other's retries from the record a
# commit wrote, and keep their sessions in the database, by hash. The
# server's two-host tests (two hosts in one process on one schema, the
# feed's and the store's); the outbox kept, on PostgreSQL (the store's
# statements this track adds to each command are e14-bounded-statements'
# pins, and its evidence); two hosts in three engines
# (e2e-hosts/second-host.spec.mjs, through playwright.hosts.config.mjs, each
# engine's hosts on a schema of their own at PORT and above); and the
# mutation controls. Every claim needs a database: refused without
# PW_FEED_DATABASE_URL, a throwaway one, which stands in for the store's
# (PW_STORE_DATABASE_URL) where that is not set, as on CI's database shards.
e14-second-host:
    @if [ -z "${PW_FEED_DATABASE_URL:-}" ]; then \
       echo "e14-second-host: refused, it needs PW_FEED_DATABASE_URL (a throwaway PostgreSQL database)"; \
       exit 1; \
     fi
    @BUILD_ONLY=1 bash spikes/own-renderer/run.sh > /dev/null
    @bash spikes/own-renderer/feed.sh > /dev/null
    @cargo build --quiet --locked -p pw-dev-server
    @mkdir -p docs/evidence/E14
    @{ export PW_STORE_DATABASE_URL="${PW_STORE_DATABASE_URL:-$PW_FEED_DATABASE_URL}"; \
       echo "ADR-XXXX - hosts share one database: each hears every commit from the outbox, and a session is the database's"; echo; \
       echo "produced by: just e14-second-host"; \
       echo "commit: $(git rev-parse HEAD)$(git diff --quiet HEAD -- . ':(exclude)docs/evidence' ':(exclude)spikes/own-renderer/store-ir.json' || echo ' + uncommitted changes')"; \
       echo "rust: $(rustc --version)"; echo "node: $(node --version)"; \
       echo "playwright: $(cd spikes/own-renderer && pnpm exec playwright --version)"; \
       echo "postgres: $(psql "$PW_FEED_DATABASE_URL" -Atc 'SHOW server_version' 2>/dev/null || echo 'unknown (psql is not on PATH)')"; echo; \
       echo "== two hosts in one process on one schema (tests/second_host.rs)"; echo; \
       cargo test --locked -p pw-dev-server -- tests::second_host:: 2>&1 | grep -E '^(test |test result)|panicked at'; \
       echo; echo "== the outbox kept, read back and read by every host (tests/feed_pg.rs, tests/store_pg.rs)"; echo; \
       PW_STORE_TEST_LAYER=postgres cargo test --locked -p pw-dev-server -- tests::feed_pg:: tests::store_pg:: 2>&1 | grep -E '^(test |test result)|panicked at'; \
       echo; echo "== two hosts in three engines (e2e-hosts/second-host.spec.mjs)"; echo; \
       bash scripts/run_suite.sh -- pnpm exec playwright test --config playwright.hosts.config.mjs --reporter=line; \
       echo; echo "== mutation controls (scripts/second_host_mutations.py)"; echo; \
       CARGO_INCREMENTAL=0 python3 scripts/second_host_mutations.py; \
     } > docs/evidence/E14/second-host.txt
    @grep -E "^test result|passed|mutants killed|^---- |panicked at" docs/evidence/E14/second-host.txt
