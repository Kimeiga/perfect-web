# The argument-invariants track's recipes: a value from outside held to its
# type's invariant at every boundary it crosses (ADR-XXXX). The root justfile
# imports this file; an evidence recipe is `e14-<name>:`, writes
# `docs/evidence/E14/<name>.txt`, and is planned by the verification run like
# any other.

# A value of an opaque type with a `where` invariant exists only if it passed
# the invariant. The build states where, from each type's declaration: a
# command's arguments, a page's parameters, inside records, lists, options,
# cases, maps and sets (compiler/pw-core/tests/argument_invariants.rs). The
# host's every call holds an export's arguments before a predicate or the
# component reads them, a type that contains itself as its nodes
# (runtime/pw-host/tests/pleris_component.rs, compiler/pw-conformance/tests/
# argument_invariants.rs). The development server holds a page's parameters
# where its address gives them, a command's arguments where they arrive, and
# a data layer's answer on PostgreSQL where `PW_STORE_DATABASE_URL` names a
# database (tests/argument_invariants.rs). The browser, in three engines
# (e2e/argument-invariants.spec.mjs); and the mutation controls
# (scripts/argument_invariants_mutations.py).
e14-argument-invariants:
    @BUILD_ONLY=1 bash spikes/own-renderer/run.sh > /dev/null
    @mkdir -p docs/evidence/E14
    @{ echo "The argument-invariants track - a value from outside held to its type's invariant at every boundary"; echo; \
       echo "produced by: just e14-argument-invariants"; \
       echo "commit: $(git rev-parse HEAD)$(git diff --quiet HEAD -- . ':(exclude)docs/evidence' ':(exclude)spikes/own-renderer/store-ir.json' || echo ' + uncommitted changes')"; \
       echo "rust: $(rustc --version)"; \
       echo "postgres: $(if [ -n "${PW_STORE_DATABASE_URL:-}" ]; then psql "$PW_STORE_DATABASE_URL" -Atc 'SHOW server_version' 2>/dev/null || echo 'unknown (psql is not on PATH)'; else echo 'none: PW_STORE_DATABASE_URL is not set, and the PostgreSQL test passes doing nothing'; fi)"; echo; \
       echo "== what the build states (compiler/pw-core/tests/argument_invariants.rs)"; echo; \
       cargo test --locked -p pw-core --test argument_invariants 2>&1 | grep -E '^(test |test result)|panicked at'; \
       echo; echo "== every call holds its export's arguments (runtime/pw-host/tests/pleris_component.rs)"; echo; \
       cargo test --locked -p pw-host --features engine --test pleris_component 2>&1 | grep -E '^(test |test result)|panicked at'; \
       echo; echo "== a type that contains itself, a map and a set (compiler/pw-conformance/tests/argument_invariants.rs)"; echo; \
       cargo test --locked -p pw-conformance --test argument_invariants 2>&1 | grep -E '^(test |test result)|panicked at'; \
       echo; echo "== the development server, the store in memory: a page's parameters, a command's arguments, a row on PostgreSQL (tests/argument_invariants.rs)"; echo; \
       PW_STORE_TEST_LAYER=memory cargo test --locked -p pw-dev-server -- tests::argument_invariants:: 2>&1 | grep -E '^(test |test result)|panicked at'; \
       echo; echo "== the same, the store on PostgreSQL (PW_STORE_TEST_LAYER=postgres)"; echo; \
       if [ -n "${PW_STORE_DATABASE_URL:-}" ]; then \
         PW_STORE_TEST_LAYER=postgres cargo test --locked -p pw-dev-server -- tests::argument_invariants:: 2>&1 | grep -E '^(test |test result)|panicked at'; \
       else echo "skipped: PW_STORE_DATABASE_URL is not set"; fi; \
       echo; echo "== the browser (e2e/argument-invariants.spec.mjs), three engines"; echo; \
       (cd spikes/own-renderer && pnpm exec playwright test e2e/argument-invariants.spec.mjs --reporter=line 2>&1) \
         | sed 's/\x1b\[[0-9;]*m//g;s/\x1b\[1A\x1b\[2K//g' \
         | grep -E "^ +[0-9]+\) |Error:|^ +[0-9]+ (passed|failed|flaky|skipped|interrupted|did not run)" || true; \
       echo; echo "== mutation controls (scripts/argument_invariants_mutations.py)"; echo; \
       python3 scripts/argument_invariants_mutations.py; \
     } > docs/evidence/E14/argument-invariants.txt
    @grep -E "^test result|passed|mutants killed|^---- |panicked at" docs/evidence/E14/argument-invariants.txt
