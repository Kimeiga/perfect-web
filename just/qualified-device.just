# Qualified signal paths retain their declaration's type and Device privacy.
e14-qualified-device:
    @mkdir -p docs/evidence/E14
    @{ set -eu; echo 'Qualified Device reads retain type and privacy'; echo 'produced by: just e14-qualified-device'; echo "commit: $(git rev-parse HEAD)"; echo "rust: $(rustc --version)"; python3 -m unittest discover -s scripts/tests -p test_qualified_device_results.py -v; cargo test --locked -p pw-core --test qualified_device; python3 scripts/qualified_device_mutations.py; } > docs/evidence/E14/qualified-device.txt 2>&1
    @grep -E 'test result|mutants killed|^baseline:' docs/evidence/E14/qualified-device.txt