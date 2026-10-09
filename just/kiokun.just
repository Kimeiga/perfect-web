# The kiokun track's recipes (ADR-0253, docs/PARALLEL.md, W6's plan):
# kiokun.com, the owner's dictionary, rewritten in Pleris. The track adds its
# recipes here and nowhere else; the root justfile imports this file, and its
# settings and variables (`set shell`, `PATH`) apply here as there. An
# evidence recipe is `e14-<name>:` and writes `docs/evidence/E14/<name>.txt`.
#
# KIOKUN_DATA names the kiokun-data checkout's `output_dictionary`, as `just
# e10-kiokun` reads it. Nothing here writes into that checkout.

# The parity inventory (the track's first ADR): every route of kiokun.com's
# SvelteKit app is a row of the inventory, marked built, partial or missing,
# and every row is a route the app has. Reads the checkout's
# `sveltekit-app` beside KIOKUN_DATA, so it is local (scripts/ci_plan.py's
# LOCAL_ONLY): CI has no checkout. Its script's own tests run in `just ci`
# (scripts/tests/test_kiokun_inventory.py).
e14-kiokun-inventory:
    @test -n "${KIOKUN_DATA:-}" || { echo "KIOKUN_DATA must name a kiokun-data checkout's output_dictionary"; exit 1; }
    @mkdir -p docs/evidence/E14
    @{ echo "kiokun.com's parity inventory, held to the SvelteKit app it inventories"; echo; \
       echo "produced by: just e14-kiokun-inventory (local: reads a kiokun-data checkout)"; \
       echo "commit: $(git rev-parse HEAD)$(git diff --quiet HEAD -- . ':(exclude)docs/evidence' || echo ' + uncommitted changes')"; \
       echo "python: $(python3 --version)"; echo; \
       echo "== the inventory against the app (scripts/kiokun_inventory.py)"; echo; \
       python3 scripts/kiokun_inventory.py docs/DECISIONS/ADR-XXXX-kiokun-parity-inventory.md "$KIOKUN_DATA/../sveltekit-app"; \
       echo; echo "== mutation controls (scripts/kiokun_inventory_mutations.py)"; echo; \
       python3 scripts/kiokun_inventory_mutations.py; \
     } > docs/evidence/E14/kiokun-inventory.txt
    @cat docs/evidence/E14/kiokun-inventory.txt
    @grep -q "^inventory: OK" docs/evidence/E14/kiokun-inventory.txt
    @! grep -qE "SURVIVED|ANCHOR NOT FOUND|^FAIL" docs/evidence/E14/kiokun-inventory.txt

# The word page (the track's step 1), served by the development server as a
# third program beside the store and the feed (docs/PARALLEL.md, "W6's
# inventory, answered", Q2): `examples/kiokun-site`, its entries read by the
# read-only kiokun layer from the repository's sample (ADR-0037), or from
# KIOKUN_DATA where the caller names it. The server's tests
# (server/src/tests/kiokun.rs); the browser in three engines, with script on
# and off (e2e/kiokun-site.spec.mjs, KIOKUN_PORTS); and the mutation controls
# (scripts/kiokun_word_mutations.py).
e14-kiokun-word:
    @BUILD_ONLY=1 bash spikes/own-renderer/run.sh > /dev/null
    @bash spikes/own-renderer/kiokun-site.sh > /dev/null
    @cargo build --quiet --locked -p pw-dev-server -p kiokun-server
    @mkdir -p docs/evidence/E14
    @{ echo "kiokun.com's word page, served by the development server"; echo; \
       echo "produced by: just e14-kiokun-word"; \
       echo "commit: $(git rev-parse HEAD)$(git diff --quiet HEAD -- . ':(exclude)docs/evidence' ':(exclude)spikes/own-renderer/store-ir.json' || echo ' + uncommitted changes')"; \
       echo "rust: $(rustc --version)"; echo "node: $(node --version)"; \
       echo "playwright: $(cd spikes/own-renderer && pnpm exec playwright --version)"; \
       if [ -n "${KIOKUN_DATA:-}" ]; then echo "entries: KIOKUN_DATA, a kiokun-data checkout (local)"; \
       else echo "entries: the repository's sample, examples/kiokun/data/han-1char-3"; fi; echo; \
       echo "== the server (server/src/tests/kiokun.rs)"; echo; \
       cargo test --locked -p pw-dev-server -- kiokun 2>&1 | awk '/^(test |test result)/ { print } /panicked at/ { p = 5 } p > 0 { print; p-- }'; \
       echo; echo "== the browser in three engines (e2e/kiokun-site.spec.mjs)"; echo; \
       (cd spikes/own-renderer && pnpm exec playwright test e2e/kiokun-site.spec.mjs --reporter=line 2>&1) \
         | sed 's/\x1b\[[0-9;]*[A-Za-z]//g' \
         | grep -E "^ +[0-9]+\) |Error:|^ +[0-9]+ (passed|failed|flaky|skipped|interrupted|did not run)" || true; \
       echo; echo "== mutation controls (scripts/kiokun_word_mutations.py)"; echo; \
       CARGO_INCREMENTAL=0 python3 scripts/kiokun_word_mutations.py; \
     } > docs/evidence/E14/kiokun-word.txt
    @grep -E "^test result|passed|failed|mutants killed|^---- |panicked at" docs/evidence/E14/kiokun-word.txt
