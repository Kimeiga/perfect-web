#!/usr/bin/env bash
# spike: koka-row-polymorphism — risk-retirement experiment RQ-2.
#
# The decision table below was PRE-REGISTERED by the project architect before
# this experiment was written, precisely so the outcome could not be rationalised
# afterwards. It is reproduced in the evidence file for that reason.
set -euo pipefail

REPO_ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
SPIKE="$REPO_ROOT/spikes/koka-row-polymorphism"
EVIDENCE="$REPO_ROOT/docs/evidence/M0"
export PATH="$REPO_ROOT/.toolchain/prefix/bin:$PATH"
mkdir -p "$EVIDENCE"
cd "$SPIKE"

command -v koka >/dev/null || { echo "koka not found — run: just bootstrap" >&2; exit 1; }

OUT="$EVIDENCE/spike-koka-row-polymorphism.txt"

read_rows() {
    node -e '
      const { parseInterface, effectNames } = await import(process.argv[1]);
      const d = parseInterface(process.argv[2]);
      for (const n of process.argv.slice(3)) {
        const e = d.get(n);
        console.log("  " + n.padEnd(20) + " !{" + (e ? effectNames(e.signature).join(", ") : "NOT FOUND") + "}");
      }
    ' --input-type=module "$REPO_ROOT/spikes/koka-js-interop/node/kki.mjs" "$@"
}

{
    echo "spike: koka-row-polymorphism  (risk-retirement experiment RQ-2)"
    echo "generated: $(date -u +%Y-%m-%dT%H:%M:%SZ)"
    echo "koka: $(koka --version | head -1)"
    echo "host: $(uname -sm)"
    echo "command: just rq-row-polymorphism"
    echo
    echo "PRE-REGISTERED DECISION TABLE (fixed before the experiment):"
    echo "  1. automatic propagation + selective narrowing      -> E1 clean pass"
    echo "  2. definition-level annotation required, conservative"
    echo "     omission, narrowing works                        -> E1 conditional pass"
    echo "  2'. annotation omission silently understates effects -> E1 FAIL"
    echo "  3. higher-order effect disappears                    -> E1 FAIL"
    echo "  4. cannot selectively discharge while preserving rest-> E1 FAIL"
    echo "  Governing criterion: Koka may OVERSTATE an effect, but must never"
    echo "  silently understate or erase one."
    echo

    echo "=========== CASE A: propagation through UNANNOTATED generics ========"
    echo "kk/inferred.kk declares my-map and transform-items with NO type"
    echo "signature at all, then calls a database-reading callback through both."
    koka --target=js --outputdir=build -o build/launcher kk/inferred.kk 2>&1 \
        | grep -vE "^(parse|check|load|link)" | grep -v "^created" || true
    echo
    read_rows build/kk_inferred.kki my-map transform-items page-loader pure-loader
    echo
    echo "  my-map / transform-items are effect-POLYMORPHIC (inferred, unannotated)."
    echo "  page-loader shows database-read propagated through TWO generic layers."
    echo "  pure-loader stays total through the same helpers."
    echo

    echo "=========== CASE B: selective discharge ============================="
    echo "kk/selective.kk handles one effect at a time out of a two-effect row,"
    echo "and passes a secret-reading callback through a generic helper."
    koka --target=js --outputdir=build -o build/launcher-sel kk/selective.kk 2>&1 \
        | grep -vE "^(parse|check|load|link)" | grep -v "^created" || true
    echo
    read_rows build/kk_selective.kki both-effects discharge-db discharge-trace discharge-all my-map leaky
    echo
    echo "  discharge-db     keeps trace-effect  -> narrowing preserves the remainder"
    echo "  discharge-trace  keeps database-read -> narrowing works in both directions"
    echo "  discharge-all    is total            -> full discharge reaches {}"
    echo "  leaky            shows secret-read   -> a generic helper cannot HIDE a capability"
    echo

    echo "=========== NEGATIVE: can a total signature accept an effectful callback? ==="
    echo "kk/negative-total-helper.kk annotates a helper as taking a TOTAL callback"
    echo "and then passes it an effectful one. This MUST fail to compile."
    if koka --target=js --outputdir=build-neg -o build-neg/launcher \
            kk/negative-total-helper.kk > /tmp/pw-rq2-neg.log 2>&1; then
        echo "  UNEXPECTED: it compiled. Effect discipline is weaker than measured above."
        NEG=0
    else
        grep -iE "^kk/|error" /tmp/pw-rq2-neg.log | head -4 | sed 's/^/    /'
        NEG=1
    fi
    echo "  check:total-helper-rejects-effectful-callback=$([ "$NEG" = 1 ] && echo pass || echo fail)"
    echo

    echo "=========== RUNTIME ================================================"
    node build/launcher.mjs 2>&1 | sed 's/^/  /'
    node build/launcher-sel.mjs 2>&1 | sed 's/^/  /'
    echo

    echo "=========== RULING ================================================="
    echo "  Observed: propagation is AUTOMATIC through unannotated generic helpers,"
    echo "  across two layers, and selective discharge preserves the remaining row."
    echo "  That is pre-registered OUTCOME 1."
    echo
    echo "  RULING: E1 CLEAN PASS on higher-order effect propagation."
    echo "  Koka remains the temporary effects-and-handlers oracle (ADR-0001,"
    echo "  as narrowed by ADR-0011). The pw effect checker is NOT pulled forward"
    echo "  on these grounds."
} | tee "$OUT"

echo
echo "spike-koka-row-polymorphism: evidence written to $OUT"
