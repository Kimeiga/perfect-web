# The identity track's recipes (ADR-0253, docs/PARALLEL.md): accounts,
# sign-in and the `requires` evaluator. The track adds its recipes here and
# nowhere else; the root justfile imports this file, and its settings and
# variables (`set shell`, `PATH`) apply here as there. An evidence recipe is
# `e14-<name>:`, writes `docs/evidence/E14/<name>.txt`, and is planned by the
# verification run like any other (scripts/ci_plan.py reads this file too).

# Track `identity` (ADR-0258): accounts and sign-in on the feed. The server's
# tests of the identity, the development provider and the feed's accounts
# (on PostgreSQL too where PW_FEED_DATABASE_URL names a throwaway database;
# skipped otherwise), the flows in three engines (e2e/identity.spec.mjs,
# PW_IDENTITY=dev-accounts), and the mutation controls.
e14-identity:
    @BUILD_ONLY=1 bash spikes/own-renderer/run.sh > /dev/null
    @bash spikes/own-renderer/feed.sh > /dev/null
    @cargo build --quiet --locked -p pw-dev-server -p kiokun-server
    @mkdir -p docs/evidence/E14
    @{ echo "ADR-0258 - accounts and sign-in: Pleris the relying party, the provider the deployment's"; echo; \
       echo "produced by: just e14-identity"; \
       echo "commit: $(git rev-parse HEAD)$(git diff --quiet HEAD -- . ':(exclude)docs/evidence' ':(exclude)spikes/own-renderer/store-ir.json' || echo ' + uncommitted changes')"; \
       echo "rust: $(rustc --version)"; echo "node: $(node --version)"; \
       echo "playwright: $(cd spikes/own-renderer && pnpm exec playwright --version)"; \
       if [ -n "${PW_FEED_DATABASE_URL:-}" ]; then \
         echo "postgres: $(psql "$PW_FEED_DATABASE_URL" -Atc 'SHOW server_version' 2>/dev/null || echo 'unknown (psql is not on PATH)')"; \
       else echo "postgres: PW_FEED_DATABASE_URL is not set, so the PostgreSQL tests pass doing nothing"; fi; echo; \
       echo "== the server (identity.rs, accounts.rs, tests/sign_in.rs)"; echo; \
       cargo test --locked -p pw-dev-server -- identity:: accounts:: sign_in:: 2>&1 | grep -E '^(test |test result)'; \
       echo; echo "== the flows in three engines (e2e/identity.spec.mjs)"; echo; \
       (cd spikes/own-renderer && pnpm exec playwright test e2e/identity.spec.mjs --reporter=line 2>&1) \
         | sed 's/\x1b\[[0-9;]*m//g;s/\x1b\[1A\x1b\[2K//g' \
         | grep -E "^ +[0-9]+\) |Error:|^ +[0-9]+ (passed|failed|flaky|skipped|interrupted|did not run)" || true; \
       echo; echo "== mutation controls (scripts/identity_mutations.py)"; echo; \
       CARGO_INCREMENTAL=0 python3 scripts/identity_mutations.py; \
     } > docs/evidence/E14/identity.txt
    @grep -E "^test result|passed|mutants killed|^---- " docs/evidence/E14/identity.txt
