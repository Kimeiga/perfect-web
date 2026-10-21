# Canonical guide producers retain native files and own their process group.
e14-guide-ownership:
    @mkdir -p docs/evidence/E14
    @{ set -eu; echo 'ADR-0376 - canonical guide process ownership'; echo 'produced by: just e14-guide-ownership'; echo "commit: $(git rev-parse HEAD)"; python3 -m unittest discover -s scripts/tests -p test_guide_process_ownership.py -v; python3 scripts/guide_ownership_mutations.py; } > docs/evidence/E14/guide-ownership.txt 2>&1
    @tail -n 5 docs/evidence/E14/guide-ownership.txt
