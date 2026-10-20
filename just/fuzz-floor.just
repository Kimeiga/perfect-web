# Track `fuzz-floor`'s recipes (the coverage-guided fuzzer ran blind, found
# 2026-10-20). The root justfile imports this file; an evidence recipe is
# `e14-<name>:`, writes `docs/evidence/E14/<name>.txt`, and is planned by the
# verification run like any other.

# Track `fuzz-floor` (ADR-XXXX): the coverage-guided fuzzer (`tools/fuzz`, `just
# fuzz`) finds `llvm-profdata` in the active toolchain, not at one machine's
# path, and fails, naming what is missing, where it measured nothing: it used
# to report "0 covered regions ... 0 finding(s)" for every target and exit 0
# on every machine but one. The tests of it (where the tool is, what a profile
# says, the floor, the program run as a person runs it); `just doctor` naming
# the component and the toolchain file listing it; the fuzzer on this machine,
# 30 inputs a target, every target covering regions; the same run with no
# `llvm-profdata`, which fails; and the mutation controls
# (scripts/fuzz_floor_mutations.py). Then E9C's own evidence, `just fuzz`'s
# run (600 inputs a target), into docs/evidence/E9C/coverage-guided-fuzzing.txt,
# so that the verification run records it (the integrator's ruling of
# 2026-10-20, train-11): it was the Mac's, recorded by hand when the fuzzer
# measured only there, and no recipe the planner plans wrote it.
e14-fuzz-floor:
    @cargo build --quiet --locked -p pw-fuzz --bins 2> /dev/null
    @mkdir -p docs/evidence/E14
    @report="$(mktemp docs/evidence/E14/.fuzz-floor.XXXXXX)"; \
     empty="$(mktemp -d)"; \
     trap 'status=$?; if [ "$status" -ne 0 ]; then cat "$report" >&2; fi; rm -rf "$report" "$empty" "$empty.out"; exit "$status"' EXIT; \
     { echo "Track fuzz-floor - the coverage-guided fuzzer measures, or it fails"; echo; \
       echo "produced by: just e14-fuzz-floor"; \
       echo "commit: $(git rev-parse HEAD)$(git diff --quiet HEAD -- . ':(exclude)docs/evidence' ':(exclude)spikes/own-renderer/store-ir.json' || echo ' + uncommitted changes')"; \
       echo "rust: $(rustc --version)"; echo "python: $(python3 --version)"; echo "host: $(uname -srm)"; echo; \
       echo "== 1. the tests of the fuzzer: where llvm-profdata is, what a profile says, the floor, the program run as a person runs it (tools/fuzz/src/tests.rs, tools/fuzz/tests/floor.rs)"; echo; \
       cargo test --locked -p pw-fuzz 2>&1 | grep -E '^test |^test result|panicked at'; \
       echo; echo "== 2. just doctor names the component, and the toolchain file lists it (scripts/tests/test_doctor_llvm_tools.py)"; echo; \
       python3 -m unittest -v scripts/tests/test_doctor_llvm_tools.py 2>&1 | grep -E ' \.\.\. |^Ran |^OK|^FAILED'; \
       echo; echo "== 3. the fuzzer on this machine, 30 inputs a target: every target covers regions, the floor passes, the status is 0"; echo; \
       cargo run -q --locked -p pw-fuzz --bin pw-fuzz -- --iterations 30 2>&1 | grep -E 'llvm-profdata:|covered regions|targets,'; \
       echo; echo "== 4. the same run with PW_LLVM_BIN_DIR naming an empty directory: it stops before it builds anything, and the status is not 0"; echo; \
       if PW_LLVM_BIN_DIR="$empty" cargo run -q --locked -p pw-fuzz --bin pw-fuzz -- --iterations 30 > "$empty.out" 2>&1; then \
         echo "UNEXPECTED: it succeeded"; cat "$empty.out"; exit 1; \
       else \
         echo "exit status $?"; sed "s|$empty|<an empty directory>|g" "$empty.out"; \
       fi; \
       echo; echo "== mutation controls (scripts/fuzz_floor_mutations.py)"; echo; \
       python3 scripts/fuzz_floor_mutations.py; \
     } > "$report"; \
     mv "$report" docs/evidence/E14/fuzz-floor.txt
    @grep -E "^Ran |^OK|^FAILED|^test result|covered regions|^exit status|mutants killed|^---- |panicked at|UNEXPECTED" docs/evidence/E14/fuzz-floor.txt
    @mkdir -p docs/evidence/E9C
    @report="$(mktemp docs/evidence/E9C/.coverage-guided-fuzzing.XXXXXX)"; \
     trap 'status=$?; if [ "$status" -ne 0 ]; then cat "$report" >&2; fi; rm -f "$report"; exit "$status"' EXIT; \
     { echo "E9C - coverage-guided fuzzing: each target, 600 inputs (just fuzz's run)"; echo; \
       echo "produced by: just e14-fuzz-floor"; \
       echo "commit: $(git rev-parse HEAD)$(git diff --quiet HEAD -- . ':(exclude)docs/evidence' ':(exclude)spikes/own-renderer/store-ir.json' || echo ' + uncommitted changes')"; \
       echo "rust: $(rustc --version)"; echo "host: $(uname -srm)"; echo; \
       cargo run -q --locked -p pw-fuzz --bin pw-fuzz -- --iterations 600 2>&1; \
     } > "$report"; \
     mv "$report" docs/evidence/E9C/coverage-guided-fuzzing.txt
    @grep -E "covered regions|targets,|finding" docs/evidence/E9C/coverage-guided-fuzzing.txt
