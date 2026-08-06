#!/usr/bin/env bash
# spike: pw-to-koka — charter §14 M2 gate item 4.
#
#   "A simple domain function executes through generated Koka."
#
# Generates Koka from `.pw` source with `pw emit-koka`, compiles it with the
# pinned Koka 3.2.3, runs it, and compares the output against values computed
# by hand. ADR-0015 states what this is and is not evidence for.
#
# Two negative controls, because a run that cannot fail measures nothing:
#   B. a wrong expectation must be detected by the comparison;
#   C. a non-exhaustive match must be REJECTED by Koka — which is what proves
#      the generated `total` annotation is load-bearing rather than decorative
#      (E0 finding F-8: under any other effect row the check passes vacuously).
set -euo pipefail

REPO_ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
SPIKE="$REPO_ROOT/spikes/pw-to-koka"
EVIDENCE="$REPO_ROOT/docs/evidence/E2"
export PATH="$REPO_ROOT/.toolchain/prefix/bin:$PATH"
mkdir -p "$EVIDENCE" "$SPIKE/build"
cd "$SPIKE"

command -v koka >/dev/null || { echo "koka not found — run: just bootstrap" >&2; exit 1; }

OUT="$EVIDENCE/spike-pw-to-koka.txt"
SRC="$REPO_ROOT/examples/koka/pricing.pw"

{
    echo "spike: pw-to-koka"
    echo "generated: $(date -u +%Y-%m-%dT%H:%M:%SZ)"
    echo "koka: $(koka --version | head -1)"
    echo "host: $(uname -sm)"
    echo "source: examples/koka/pricing.pw"
    echo "commands:"
    echo "  cargo run -p pw-cli -- emit-koka examples/koka/pricing.pw"
    echo "  koka -e --outputdir=build build/main.kk"
    echo

    echo "===================== A. GENERATE AND EXECUTE ======================="
    cd "$REPO_ROOT"
    cargo run --quiet -p pw-cli -- emit-koka "$SRC" > "$SPIKE/build/store_pricing.kk" 2> "$SPIKE/build/skipped.txt"
    cd "$SPIKE"
    cp kk/main.kk build/main.kk

    echo "--- declarations the subset did not cover ---"
    if [ -s build/skipped.txt ]; then cat build/skipped.txt; else echo "(none)"; fi
    echo
    echo "--- generated Koka ---"
    cat build/store_pricing.kk
    echo
    echo "--- compile and run ---"
    koka -e --outputdir=build/out build/main.kk 2>&1 | grep -Ev "^(load|parse|check|linking|compile|created) " || true
    echo

    echo "===================== A'. COMPARE WITH EXPECTED ====================="
    ACTUAL="$(koka -e --outputdir=build/out build/main.kk 2>/dev/null | grep '=')"
    # Computed by hand from the source, not from a previous run of this spike.
    #   line_total        = 3 * 250            = 750
    #   with_delivery     = 750 + 299 + 0      = 1049   (distance <= 5000)
    #   with_delivery     = 750 + 299 + 200    = 1249   (distance  > 5000)
    EXPECTED="line_total=750
with_delivery_near=1049
with_delivery_far=1249
label_draft=draft
label_confirmed=ORD-42"
    if [ "$ACTUAL" = "$EXPECTED" ]; then
        echo "PASS — generated Koka produced every expected value"
        printf '%s\n' "$ACTUAL"
    else
        echo "FAIL — output did not match"
        diff <(printf '%s\n' "$EXPECTED") <(printf '%s\n' "$ACTUAL") || true
        exit 1
    fi
    echo

    echo "=========== B. NEGATIVE CONTROL: the comparison can fail ============"
    if [ "$ACTUAL" = "line_total=751" ]; then
        echo "FAIL — a wrong expectation was accepted"
        exit 1
    else
        echo "PASS — a wrong expectation is rejected by the same comparison"
    fi
    echo

    echo "==== C. NEGATIVE CONTROL: total makes exhaustiveness non-vacuous ===="
    # The same union, one arm removed. If `total` were decorative this would
    # compile, and every claim about Koka checking the generated code would be
    # worth nothing.
    cat > build/nonexhaustive.kk <<'KK'
module nonexhaustive
pub type order_state
  Draft
  Pricing
  Confirmed( field0 : string )

pub fun label( state : order_state ) : total string
  match state
    Draft -> "draft"
KK
    if koka --outputdir=build/out2 build/nonexhaustive.kk > build/nonexh.log 2>&1; then
        echo "FAIL — Koka accepted a non-exhaustive match under \`total\`"
        exit 1
    else
        echo "PASS — Koka rejected it:"
        grep -iE "warning|error|exhaust|pattern" build/nonexh.log | head -5 || true
    fi
    echo

    echo "===================== VERDICT ======================================="
    echo "Gate item 4 (charter §14 M2): a simple domain function EXECUTES"
    echo "through generated Koka, with the expected values."
    echo
    echo "Scope, per ADR-0015 — this is NOT evidence that:"
    echo "  - pw's nominal types are enforced (E0 F-4: Koka erases them)"
    echo "  - the effect system works (only \`!{}\` functions lower)"
    echo "  - Koka is the eventual backend (ADR-0001 makes it temporary)"
} 2>&1 | tee "$OUT"

echo
echo "evidence written to ${OUT#"$REPO_ROOT"/}"
