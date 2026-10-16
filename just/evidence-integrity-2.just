# Track `evidence-integrity-2`'s recipes (the integrator's rulings of
# 2026-10-15 on track evidence-integrity's findings). The root justfile
# imports this file; an evidence recipe is `e14-<name>:`, writes
# `docs/evidence/E14/<name>.txt`, and is planned by the verification run like
# any other.

# Track `evidence-integrity-2` (ADR-0349): a failed browser test fails its
# recipe, every suite run through scripts/run_suite.sh and held by a gate,
# with what the swallowing hid; the planner reads every recipe's body, and a
# change to the suite's configuration plans what runs under it;
# mutant-programs finds every program from the mutants' paths; no comparison
# translates an old number; the corpus's enforcement table is held current;
# e9-latency names its machine. The tests of each, and the mutation controls
# (scripts/evidence_integrity_2_mutations.py).
e14-evidence-integrity-2:
    @cargo build --quiet --locked -p pw-cli
    @cargo test --locked -p pw-core --test checking_source --no-run 2> /dev/null
    @mkdir -p docs/evidence/E14
    @{ echo "Track evidence-integrity-2 - a failed browser test fails its recipe, and five more rulings"; echo; \
       echo "produced by: just e14-evidence-integrity-2"; \
       echo "commit: $(git rev-parse HEAD)$(git diff --quiet HEAD -- . ':(exclude)docs/evidence' ':(exclude)spikes/own-renderer/store-ir.json' || echo ' + uncommitted changes')"; \
       echo "rust: $(rustc --version)"; echo "python: $(python3 --version)"; echo "just: $(just --version)"; echo; \
       echo "== 1. what the evidence recorded of failed browser tests while their recipes passed (scripts/recorded_browser_failures.py; a shallow checkout reads what it holds)"; echo; \
       python3 scripts/recorded_browser_failures.py; \
       python3 -m unittest -v scripts/tests/test_recorded_browser_failures.py 2>&1 | grep -E ' \.\.\. |^Ran |^OK|^FAILED'; \
       echo; echo "== 1. a failed browser test fails its recipe: the helper (scripts/run_suite.sh, test_run_suite.py)"; echo; \
       python3 -m unittest -v scripts/tests/test_run_suite.py 2>&1 | grep -E ' \.\.\. |^Ran |^OK|^FAILED'; \
       echo; echo "== 1. every suite in the justfile runs by it, its status its recipe's (scripts/suite_status.py, test_suite_status.py)"; echo; \
       python3 scripts/suite_status.py; \
       python3 -m unittest -v scripts/tests/test_suite_status.py 2>&1 | grep -E ' \.\.\. |^Ran |^OK|^FAILED'; \
       echo; echo "== 1. each suite failed in turn fails its recipe, and its evidence says so (test_suites_fail_their_recipes.py, test_builds_before_suites.py)"; echo; \
       python3 -m unittest -v scripts/tests/test_suites_fail_their_recipes.py scripts/tests/test_builds_before_suites.py 2>&1 \
         | grep -E ' \.\.\. |suites fail their recipes|builds before suites|^Ran |^OK|^FAILED'; \
       echo; echo "== 2, 3. the planner reads every recipe, and the suite's configuration plans what runs under it (test_ci_scripts.py)"; echo; \
       python3 -m unittest -v scripts/tests/test_ci_scripts.py 2>&1 \
         | grep -E 'defaults_is_read|configuration_plans|module_its_specs_import|^Ran |^OK|^FAILED'; \
       echo; echo "== 4. mutant-programs finds every program a mutation script mutates (scripts/mutant_programs.py, test_mutant_programs.py)"; echo; \
       python3 -m unittest -v scripts/tests/test_mutant_programs.py 2>&1 | grep -E ' \.\.\. |^Ran |^OK|^FAILED'; \
       python3 scripts/mutant_programs.py "${CARGO_TARGET_DIR:-target}/debug/pw"; \
       echo; echo "== 5. no comparison translates an old number (compiler/pw-core: diagnostics.rs, codes.rs)"; echo; \
       cargo test --locked -p pw-core --lib -- no_comparison_translates_an_old_number an_old_number_is_no_registered_rule 2>&1 \
         | grep -E '^(test |test result)|panicked at'; \
       echo; echo "== 6. the corpus's enforcement table is what the corpus checks now (checking_source.rs)"; echo; \
       cargo test --locked -p pw-core --test checking_source -- the_committed_enforcement_table 2>&1 | grep -E '^(test |test result)|panicked at'; \
       grep -m1 "enforce their declared invariant" docs/evidence/E2D/corpus-enforcement.txt; \
       echo; echo "== 7. e9-latency names the machine it measured (test_e9_latency_recipe.py; the measurement is the integrator's, in a quiet window)"; echo; \
       python3 -m unittest -v scripts/tests/test_e9_latency_recipe.py 2>&1 | grep -E ' \.\.\. |^Ran |^OK|^FAILED'; \
       echo; echo "== mutation controls (scripts/evidence_integrity_2_mutations.py)"; echo; \
       python3 scripts/evidence_integrity_2_mutations.py; \
     } > docs/evidence/E14/evidence-integrity-2.txt
    @grep -E "^test result|^Ran |^OK|^FAILED|recorded browser failures|suite status|mutant programs|suites fail|builds before|mutants killed|^---- |panicked at|memory bound" docs/evidence/E14/evidence-integrity-2.txt
