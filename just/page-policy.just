# The page-csp track's recipes (ADR-0195's ruling 13, the integrator's):
# what a page this host serves may load and run. The root justfile imports
# this file; an evidence recipe is `e14-<name>:`, writes
# `docs/evidence/E14/<name>.txt`, and is planned by the verification run
# like any other.

# ADR-0195's ruling 13, the host's half: every HTML answer carries a
# Content-Security-Policy generated from the build and the deployment. The
# server's tests (csp.rs, tests/page_policy.rs, and the moved page's and
# the identity provider's page's); the pages the suite serves, run under it
# in three engines with nothing refused, and its controls
# (e2e/page-policy.spec.mjs); and the mutation controls
# (scripts/page_policy_mutations.py).
e14-page-policy:
    @mkdir -p docs/evidence/E14
    @{ echo "ADR-0195 ruling 13 - every HTML answer carries a Content-Security-Policy generated from its build"; echo; \
       echo "produced by: just e14-page-policy"; \
       echo "commit: $(git rev-parse HEAD)$(git diff --quiet HEAD -- . ':(exclude)docs/evidence' ':(exclude)spikes/own-renderer/store-ir.json' || echo ' + uncommitted changes')"; \
       echo "rust: $(rustc --version)"; \
       echo "playwright: $(cd spikes/own-renderer && pnpm exec playwright --version)"; echo; \
       echo "== the policy, and the answers it is sent with (csp.rs, tests/page_policy.rs)"; echo; \
       cargo test --locked -p pw-dev-server -- csp:: tests::page_policy:: \
         tests::redirects::a_page_at_another_address_of_the_page_is_moved_there_for_good \
         tests::sign_in::the_development_provider_signs_up_and_in_through_the_flow 2>&1 \
         | grep -E '^(test |test result)|panicked at'; \
       echo; echo "== the pages the suite serves, under it, in three engines (e2e/page-policy.spec.mjs)"; echo; \
       BUILD_ONLY=1 bash spikes/own-renderer/run.sh > /dev/null 2>&1; \
       bash spikes/own-renderer/feed.sh > /dev/null 2>&1; \
       bash spikes/own-renderer/kiokun-site.sh > /dev/null 2>&1; \
       cargo build --quiet --locked -p pw-dev-server -p kiokun-server; \
       (cd spikes/own-renderer && pnpm exec playwright test e2e/page-policy.spec.mjs --reporter=line 2>&1) \
         | sed 's/\x1b\[[0-9;]*[A-Za-z]//g' \
         | grep -E "^ +[0-9]+\) |Error:|^ +[0-9]+ (passed|failed|flaky|skipped|interrupted|did not run)" || true; \
       echo; echo "== mutation controls (scripts/page_policy_mutations.py)"; echo; \
       CARGO_INCREMENTAL=0 python3 scripts/page_policy_mutations.py; \
     } > docs/evidence/E14/page-policy.txt
    @grep -E "^test result|mutants killed|^---- |panicked at|^ +[0-9]+ (passed|failed)" docs/evidence/E14/page-policy.txt
