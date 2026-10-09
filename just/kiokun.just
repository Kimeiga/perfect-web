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
