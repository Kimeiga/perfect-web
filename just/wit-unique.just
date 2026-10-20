# Declaration namespaces survive every executable component identity projection.
e14-wit-unique:
    @BUILD_ONLY=1 bash spikes/own-renderer/run.sh > /dev/null
    @cargo build --quiet --locked -p pw-dev-server
    @mkdir -p docs/evidence/E14
    @{ set -eu; echo 'UI component identities retain their namespace'; echo; echo 'produced by: just e14-wit-unique'; echo "commit: $(git rev-parse HEAD)"; echo "rust: $(rustc --version)"; cargo test --locked -p pw-core --test component_names -- --nocapture; python3 scripts/wit_unique_mutations.py; bash scripts/run_suite.sh -- pnpm exec playwright test e2e/wit-unique.spec.mjs --reporter=line; } > docs/evidence/E14/wit-unique.txt 2>&1
    @grep -E 'test result|mutants killed|^baseline:' docs/evidence/E14/wit-unique.txt