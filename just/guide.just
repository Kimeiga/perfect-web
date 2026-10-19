# The guide track's recipes (ADR-0343, a guide whose examples compile;
# docs/PARALLEL.md): the Pleris guide in `docs/guide/`, whose examples a test
# compiles, and the guestbook its tutorial builds. The track adds its recipes
# here and nowhere else; the root justfile imports this file, and its settings
# and variables (`set shell`, `PATH`) apply here as there. An evidence recipe
# is `e14-<name>:`, writes `docs/evidence/E14/<name>.txt`, and is planned by
# the verification run like any other (scripts/ci_plan.py reads this file
# too).

# Track `guide` (ADR-0343): every example in `docs/guide/*.md` checked as the
# guide says, through the `pw` binary and the standard packages (the guide's
# test, compiler/pw-cli/tests/guide.rs); the reference of codes written from
# the registry and held to it (scripts/guide_diagnostics.py and its test);
# the tutorial's app, `examples/guestbook/*.pw`, which the guide's test holds
# byte for byte to the tutorial's page, served by the development server and
# driven in three engines (e2e/guestbook.spec.mjs, built by guestbook.sh); and
# the mutation controls of all three (scripts/guide_mutations.py).
e14-guide:
    @bash spikes/own-renderer/guestbook.sh > /dev/null
    @cargo build --quiet --locked -p pw-dev-server
    @mkdir -p docs/evidence/E14
    @{ echo "ADR-0343 (track guide) - a guide whose examples compile: every example of docs/guide checked, and the tutorial's app run in three engines"; echo; \
       echo "produced by: just e14-guide"; \
       echo "commit: $(git rev-parse HEAD)$(git diff --quiet HEAD -- . ':(exclude)docs/evidence' ':(exclude)spikes/own-renderer/store-ir.json' || echo ' + uncommitted changes')"; \
       echo "rust: $(rustc --version)"; echo "node: $(node --version)"; \
       echo "playwright: $(cd spikes/own-renderer && pnpm exec playwright --version)"; echo; \
       echo "== every example of the guide, through the pw binary (compiler/pw-cli/tests/guide.rs)"; echo; \
       cargo test --locked -p pw-cli --test guide 2>&1 | grep -E '^(test |test result)|panicked at'; \
       echo; echo "== the reference of codes, from the registry (scripts/guide_diagnostics.py)"; echo; \
       python3 scripts/guide_diagnostics.py --check; \
       python3 -m unittest discover -s scripts/tests -p test_guide_diagnostics.py -v 2>&1 | grep -E '^(test_|Ran |OK|FAILED)'; \
       echo; echo "== the tutorial's app, examples/guestbook/*.pw, in three engines (e2e/guestbook.spec.mjs)"; echo; \
       (cd spikes/own-renderer && pnpm exec playwright test e2e/guestbook.spec.mjs --reporter=line 2>&1) \
         | sed 's/\x1b\[[0-9;]*[A-Za-z]//g' \
         | grep -E "^ +[0-9]+\) |Error:|^ +[0-9]+ (passed|failed|flaky|skipped|interrupted|did not run)"; \
       echo; echo "== mutation controls (scripts/guide_mutations.py)"; echo; \
       CARGO_INCREMENTAL=0 python3 scripts/guide_mutations.py; \
     } > docs/evidence/E14/guide.txt
    @grep -E "^test result|passed|mutants killed|^---- |panicked at" docs/evidence/E14/guide.txt
