# The proof track's recipes (ADR-XXXX, the claim measured): the evidence
# for the owner's claim that Pleris makes complex web apps more correctly
# than today's frameworks. The root justfile imports this file; an evidence
# recipe is `e14-<name>:`, writes `docs/evidence/E14/<name>.txt`, and is
# planned by the verification run like any other.

# ADR-XXXX: the bug museum. Each exhibit, a failure from the census with a
# dated source, shown happening in Next.js or SvelteKit (its spec passing on
# the bug and failing on its repair, the baselines' installed dependencies
# only), then Pleris's answer to the same change: a named code with a
# neighbour that checks clean, a runtime test, or a miss shown as one. Misses
# first (benchmarks/museum/museum.mjs). Then the mutation controls
# (scripts/proof_mutations.py). Playwright on PORT, 8941 by default.
e14-museum:
    @cargo build --quiet --locked -p pw-cli -p pw-dev-server
    @mkdir -p docs/evidence/E14
    @{ echo "ADR-XXXX - the bug museum: each exhibit's bug shown in Next.js or SvelteKit, and Pleris's answer to it"; echo; \
       echo "produced by: just e14-museum"; \
       echo "commit: $(git rev-parse HEAD)$(git diff --quiet HEAD -- . ':(exclude)docs/evidence' ':(exclude)spikes/own-renderer/store-ir.json' || echo ' + uncommitted changes')"; \
       echo "rust: $(rustc --version)"; \
       echo "node: $(node --version)  pnpm: $(pnpm --version)"; \
       for p in next react svelte @sveltejs/kit vite typescript @playwright/test; do \
         for d in benchmarks/baselines/next-react benchmarks/baselines/sveltekit benchmarks/harness; do \
           f="$d/node_modules/$p/package.json"; [ -f "$f" ] && printf '  %-18s %s  (%s)\n' "$p" "$(node -p "require('./$f').version")" "$d"; \
         done; done | sort -u; echo; \
       echo "== the exhibits (benchmarks/museum/museum.mjs)"; echo; \
       node benchmarks/museum/museum.mjs 2>&1; \
       echo; echo "== mutation controls (scripts/proof_mutations.py)"; echo; \
       python3 scripts/proof_mutations.py; \
     } > docs/evidence/E14/museum.txt
    @grep -E "^museum:|NOT AS|mutants killed|^FAIL" docs/evidence/E14/museum.txt
