# The uploads track's recipes (ADR-0253, docs/PARALLEL.md): a typed upload,
# a deployment's blob storage, and serving it safely. The track adds its
# recipes here and nowhere else; the root justfile imports this file, and its
# settings and variables (`set shell`, `PATH`) apply here as there. An
# evidence recipe is `e14-<name>:`, writes `docs/evidence/E14/<name>.txt`,
# and is planned by the verification run like any other (scripts/ci_plan.py
# reads this file too).

# Track `uploads` (ADR-XXXX): an image on a post. The header readers, the
# form's body, the blob storage, the leases and serving (the server's
# `uploads` and `blob` tests, PostgreSQL's where PW_FEED_DATABASE_URL names
# a database and passing doing nothing otherwise); the `upload` declaration
# and its checks (compiler/pw-core/tests/uploads.rs); the browser suite in
# three engines (e2e/uploads.spec.mjs); and the mutation controls
# (scripts/uploads_mutations.py).
e14-uploads:
    @BUILD_ONLY=1 bash spikes/own-renderer/run.sh > /dev/null
    @bash spikes/own-renderer/feed.sh > /dev/null
    @cargo build --quiet --locked -p pw-dev-server -p kiokun-server
    @mkdir -p docs/evidence/E14
    @{ echo "ADR-XXXX (track uploads) - an image on a post: a typed upload, a deployment's blob storage, served safely"; echo; \
       echo "produced by: just e14-uploads"; \
       echo "commit: $(git rev-parse HEAD)$(git diff --quiet HEAD -- . ':(exclude)docs/evidence' ':(exclude)spikes/own-renderer/store-ir.json' || echo ' + uncommitted changes')"; \
       echo "rust: $(rustc --version)"; echo "node: $(node --version)"; \
       echo "playwright: $(cd spikes/own-renderer && pnpm exec playwright --version)"; \
       echo "postgres: $([ -n "${PW_FEED_DATABASE_URL:-}" ] && (psql "$PW_FEED_DATABASE_URL" -Atc 'SHOW server_version' 2>/dev/null || echo 'unknown (psql is not on PATH)') || echo 'none: PW_FEED_DATABASE_URL is not set')"; echo; \
       echo "== the server (uploads.rs, uploads/sniff.rs, uploads/multipart.rs, blob.rs, tests/uploads.rs)"; echo; \
       cargo test --locked -p pw-dev-server -- uploads blob 2>&1 | awk '/^(test |test result)/ { print } /panicked at/ { p = 5 } p > 0 { print; p-- }'; \
       echo; echo "== the declaration and its checks (compiler/pw-core/tests/uploads.rs)"; echo; \
       cargo test --locked -p pw-core --test uploads 2>&1 | awk '/^(test |test result)/ { print } /panicked at/ { p = 5 } p > 0 { print; p-- }'; \
       echo; echo "== an image on a post in three engines (e2e/uploads.spec.mjs)"; echo; \
       (cd spikes/own-renderer && pnpm exec playwright test e2e/uploads.spec.mjs --reporter=line 2>&1) \
         | sed 's/\x1b\[[0-9;]*[A-Za-z]//g' \
         | grep -E "^ +[0-9]+\) |Error:|^ +[0-9]+ (passed|failed|flaky|skipped|interrupted|did not run)" || true; \
       echo; echo "== mutation controls (scripts/uploads_mutations.py)"; echo; \
       CARGO_INCREMENTAL=0 python3 scripts/uploads_mutations.py; \
     } > docs/evidence/E14/uploads.txt
    @grep -E "^test result|passed|mutants killed" docs/evidence/E14/uploads.txt
