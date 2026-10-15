# Track `evidence-integrity`'s recipes (the integrator's findings of
# 2026-10-15). The root justfile imports this file; an evidence recipe is
# `e14-<name>:`, writes `docs/evidence/E14/<name>.txt`, and is planned by the
# verification run like any other.

# Track `evidence-integrity` (ADR-XXXX): six ways evidence could be wrong, or
# a gate fail for the wrong reason, without anything noticing. A mutation
# script's sources run as they are (no bytecode written, none left read);
# evidence gone stale is planned at a train's push, and an evidence file's
# counts are its script's; a rejected fixture's `@rule` is the code `pw
# check` reports; `check_latency` bounds its thread's CPU time, here and
# beside twice as many busy loops as the machine has CPUs; a browser suite
# runs on what its recipe built; and master holds no unnumbered ADR. The
# tests of each, and the mutation controls
# (scripts/evidence_integrity_mutations.py).
e14-evidence-integrity:
    @cargo test --locked -p pw-core --test check_latency --test checking_source --no-run 2> /dev/null
    @mkdir -p docs/evidence/E14
    @{ echo "Track evidence-integrity - six ways evidence could be wrong, or a gate fail for the wrong reason"; echo; \
       echo "produced by: just e14-evidence-integrity"; \
       echo "commit: $(git rev-parse HEAD)$(git diff --quiet HEAD -- . ':(exclude)docs/evidence' ':(exclude)spikes/own-renderer/store-ir.json' || echo ' + uncommitted changes')"; \
       echo "rust: $(rustc --version)"; echo "python: $(python3 --version)"; echo "node: $(node --version)"; echo; \
       echo "== 1. a mutation script's sources run as they are (scripts/tests/test_mutation_baseline.py)"; echo; \
       python3 -m unittest -v scripts/tests/test_mutation_baseline.py 2>&1 | grep -E ' \.\.\. |^Ran |^OK|^FAILED'; \
       echo; echo "== 2. stale evidence planned, an evidence file's counts its script's (test_ci_scripts.py, test_evidence_counts.py)"; echo; \
       python3 -m unittest -v scripts/tests/test_ci_scripts.py scripts/tests/test_evidence_counts.py 2>&1 | grep -E 'Stale|Counts|PlannedAgain|Repository|^Ran |^OK|^FAILED'; \
       python3 scripts/evidence_counts.py; \
       echo; echo "== 3. a rejected fixture's @rule is the code pw check reports (compiler/pw-core/tests/checking_source.rs)"; echo; \
       cargo test --locked -p pw-core --test checking_source 2>&1 | grep -E '^(test |test result)|panicked at'; \
       echo; echo "== 4. check_latency bounds its thread's CPU time (compiler/pw-core/tests/check_latency.rs)"; echo; \
       echo "load: $(cat /proc/loadavg 2> /dev/null || sysctl -n vm.loadavg)"; \
       cargo test --locked -p pw-core --test check_latency -- --nocapture 2>&1 | grep -E '^(check-latency|test result)|panicked at'; \
       loops=$(( 2 * $(nproc 2> /dev/null || sysctl -n hw.ncpu) )); pids=""; \
       trap 'kill $pids 2> /dev/null || true' EXIT; \
       for i in $(seq 1 "$loops"); do timeout 180 sh -c 'while :; do :; done' & pids="$pids $!"; done; \
       sleep 10; echo; echo "beside $loops busy loops, load: $(cat /proc/loadavg 2> /dev/null || sysctl -n vm.loadavg)"; \
       cargo test --locked -p pw-core --test check_latency -- --nocapture 2>&1 | grep -E '^(check-latency|test result)|panicked at'; \
       kill $pids 2> /dev/null || true; wait; \
       echo; echo "== 5. a browser suite runs on what its recipe built (test_builds_before_suites.py)"; echo; \
       python3 -m unittest -v scripts/tests/test_builds_before_suites.py 2>&1 | grep -E ' \.\.\. |builds before suites|^Ran |^OK|^FAILED'; \
       echo; echo "== 6. master holds no unnumbered ADR (test_unnumbered_adrs.py; the repository's own on master's CI and a train's)"; echo; \
       python3 -m unittest -v scripts/tests/test_unnumbered_adrs.py 2>&1 | grep -E ' \.\.\. |^Ran |^OK|^FAILED'; \
       echo; echo "== mutation controls (scripts/evidence_integrity_mutations.py)"; echo; \
       python3 scripts/evidence_integrity_mutations.py; \
     } > docs/evidence/E14/evidence-integrity.txt
    @grep -E "^test result|^Ran |^OK|^FAILED|check-latency|beside|mutants killed|^---- |panicked at|memory bound" docs/evidence/E14/evidence-integrity.txt
