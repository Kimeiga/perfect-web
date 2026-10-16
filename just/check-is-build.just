# One executable-program validation authority; check writes no artifacts.
e14-check-is-build:
    @mkdir -p docs/evidence/E14
    @{ set -eu; echo 'Check and build validate one executable program'; echo; echo 'produced by: just e14-check-is-build'; echo "commit: $(git rev-parse HEAD)"; echo "rust: $(rustc --version)"; echo; cargo test --locked -p pw-core --test check_is_build -- --nocapture; cargo test --locked -p pw-cli --test check_is_build -- --nocapture; echo; cargo test --locked -p pw-cli --test check_build_corpus -- --nocapture; echo; python3 scripts/check_build_programs.py target/debug/pw; echo; python3 scripts/tests/test_check_build_gate.py; echo; python3 scripts/check_is_build_mutations.py; echo; date -u; cat /proc/loadavg; /usr/bin/time -v cargo test --locked -p pw-core --test check_latency -- --nocapture; cat /proc/loadavg; } > docs/evidence/E14/check-is-build.txt 2>&1
    @grep -E 'test result|mutants killed|check-latency|^baseline:' docs/evidence/E14/check-is-build.txt
