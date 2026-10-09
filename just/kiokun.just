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
       python3 scripts/kiokun_inventory.py docs/DECISIONS/ADR-0285-kiokun-parity-inventory.md "$KIOKUN_DATA/../sveltekit-app"; \
       echo; echo "== mutation controls (scripts/kiokun_inventory_mutations.py)"; echo; \
       python3 scripts/kiokun_inventory_mutations.py; \
     } > docs/evidence/E14/kiokun-inventory.txt
    @cat docs/evidence/E14/kiokun-inventory.txt
    @grep -q "^inventory: OK" docs/evidence/E14/kiokun-inventory.txt
    @! grep -qE "SURVIVED|ANCHOR NOT FOUND|^FAIL" docs/evidence/E14/kiokun-inventory.txt

# A sample of the whole dictionary, served and timed (the integrator's order
# of 2026-10-09: numbers before the word page's component grows further).
# Every KIOKUN_SAMPLE_EVERY'th file (256 by default) of every subdirectory
# of KIOKUN_DATA is looked up through the development server's HTTP path, in
# release: each page's status, and the time to its last byte. Then the same
# sample served by the word page's first milestone (ADR-0286's `app.pw`, at
# the merge), for a baseline. Local: it reads the owner's kiokun-data
# checkout (scripts/ci_plan.py's LOCAL_ONLY), and its times are this
# machine's.
e14-kiokun-sample:
    @test -n "${KIOKUN_DATA:-}" || { echo "KIOKUN_DATA must name a kiokun-data checkout's output_dictionary"; exit 1; }
    @mkdir -p docs/evidence/E14
    @git show 3036497:examples/kiokun-site/app.pw > "${TMPDIR:-/tmp}/kiokun-site-0286.pw"
    @# The baseline's source holds what today's layer grants (Label, CharGloss),
    @# declared and unused, so the host holds it to the guarantees it states.
    @sed -i.bak 's/^    holds        Entry$/    holds        Entry, Label, CharGloss/' "${TMPDIR:-/tmp}/kiokun-site-0286.pw"
    @printf '\ntype Label = Label {\n    kind: String,\n    code: String,\n    text: String,\n}\n\ntype CharGloss = CharGloss {\n    character: String,\n    gloss: String,\n}\n' >> "${TMPDIR:-/tmp}/kiokun-site-0286.pw"
    @{ echo "A sample of the whole dictionary, served by the development server and timed"; echo; \
       echo "produced by: just e14-kiokun-sample (local: reads a kiokun-data checkout)"; \
       echo "commit: $(git rev-parse HEAD)$(git diff --quiet HEAD -- . ':(exclude)docs/evidence' || echo ' + uncommitted changes')"; \
       echo "rust: $(rustc --version)"; echo "host: $(uname -sm), $(sysctl -n hw.ncpu 2>/dev/null || nproc) cores, $(uptime | sed 's/.*load/load/')"; \
       echo "KIOKUN_APP: ${KIOKUN_APP:-not set}"; echo; \
       echo "== this branch's word page"; echo; \
       cargo test --release --locked -p pw-dev-server -- --ignored --nocapture kiokun::a_sample 2>&1 \
         | grep -E '^(server|files|status|per request|slowest|defect|test result)|panicked'; \
       echo; echo "== the word page's first milestone (ADR-0286), the same sample"; echo; \
       KIOKUN_SAMPLE_APP="${TMPDIR:-/tmp}/kiokun-site-0286.pw" \
         cargo test --release --locked -p pw-dev-server -- --ignored --nocapture kiokun::a_sample 2>&1 \
         | grep -E '^(server|files|status|per request|slowest|defect|test result)|panicked'; \
     } > docs/evidence/E14/kiokun-sample.txt
    @cat docs/evidence/E14/kiokun-sample.txt
    @! grep -qE "panicked|FAILED" docs/evidence/E14/kiokun-sample.txt

# The word page's head against kiokun.com's own: its `buildDictionarySeo`
# (`src/lib/seo.ts` and `character-forms.ts`, copied from KIOKUN_APP into a
# temporary directory and never into this repository) run by Node over every
# 256th entry of KIOKUN_DATA that its loader shows as it is, and each word's
# served title and description compared with its answer. Local: it reads the
# owner's checkout (scripts/ci_plan.py's LOCAL_ONLY).
e14-kiokun-seo:
    @test -n "${KIOKUN_DATA:-}" || { echo "KIOKUN_DATA must name a kiokun-data checkout's output_dictionary"; exit 1; }
    @test -n "${KIOKUN_APP:-}" || { echo "KIOKUN_APP must name a kiokun-data checkout's sveltekit-app"; exit 1; }
    @mkdir -p docs/evidence/E14
    @lib="$(mktemp -d)"; \
     cp "$KIOKUN_APP/src/lib/seo.ts" "$KIOKUN_APP/src/lib/character-forms.ts" "$lib/"; \
     sed -i.bak "s#from './character-forms'#from './character-forms.ts'#" "$lib/seo.ts"; \
     { echo "The word page's head, against kiokun.com's own buildDictionarySeo"; echo; \
       echo "produced by: just e14-kiokun-seo (local: reads a kiokun-data checkout)"; \
       echo "commit: $(git rev-parse HEAD)$(git diff --quiet HEAD -- . ':(exclude)docs/evidence' || echo ' + uncommitted changes')"; \
       echo "kiokun.com's source: $(GIT_OPTIONAL_LOCKS=0 git -C "$KIOKUN_APP" rev-parse HEAD) (seo.ts $(shasum -a 256 "$KIOKUN_APP/src/lib/seo.ts" | cut -c1-12), character-forms.ts $(shasum -a 256 "$KIOKUN_APP/src/lib/character-forms.ts" | cut -c1-12))"; \
       echo "node: $(node --version)"; echo; \
       echo "== kiokun.com's answers (spikes/own-renderer/kiokun-oracle/seo.mjs)"; echo; \
       node --experimental-strip-types --no-warnings spikes/own-renderer/kiokun-oracle/seo.mjs "$lib" "$KIOKUN_DATA" 256 "$lib/seo.json"; \
       echo; echo "== the rewrite's, compared"; echo; \
       KIOKUN_SEO_ORACLE="$lib/seo.json" cargo test --release --locked -p pw-dev-server -- --ignored --nocapture kiokun::the_page_head_matches 2>&1 \
         | grep -E '^(compared|named|unnamed|difference|  kiokun|  rewrite)|test result|panicked'; \
       echo; echo "== the CI fixture: kiokun.com's answers for the repository's sample"; echo; \
       node --experimental-strip-types --no-warnings spikes/own-renderer/kiokun-oracle/seo.mjs "$lib" examples/kiokun/data/han-1char-3 1 "$lib/sample.json"; \
       python3 scripts/kiokun_oracle_fixture.py "$lib/sample.json" spikes/own-renderer/kiokun-oracle/seo-sample.json \
         "just e14-kiokun-seo" "$(GIT_OPTIONAL_LOCKS=0 git -C "$KIOKUN_APP" rev-parse HEAD)" "$(git rev-parse HEAD)" "$(node --version)"; \
     } > docs/evidence/E14/kiokun-seo.txt; rm -rf "$lib"
    @cat docs/evidence/E14/kiokun-seo.txt
    @grep -q "unnamed differences: 0" docs/evidence/E14/kiokun-seo.txt

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
