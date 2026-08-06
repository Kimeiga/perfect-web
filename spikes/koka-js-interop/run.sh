#!/usr/bin/env bash
# spike: koka-js-interop — build, test, and record evidence.
# Charter §14 Milestone 0 tasks 7-8.
set -euo pipefail

REPO_ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
SPIKE="$REPO_ROOT/spikes/koka-js-interop"
EVIDENCE="$REPO_ROOT/docs/evidence/E0"
export PATH="$REPO_ROOT/.toolchain/prefix/bin:$PATH"
mkdir -p "$EVIDENCE"
cd "$SPIKE"

command -v koka >/dev/null || { echo "koka not found — run: just bootstrap" >&2; exit 1; }
command -v node >/dev/null || { echo "node not found" >&2; exit 1; }

OUT="$EVIDENCE/spike-koka-js-interop.txt"

{
    echo "spike: koka-js-interop"
    echo "generated: $(date -u +%Y-%m-%dT%H:%M:%SZ)"
    echo "koka: $(koka --version | head -1)"
    echo "node: $(node --version)"
    echo "host: $(uname -sm)"
    echo "commands:"
    echo "  koka --target=js --outputdir=build -o build/store kk/store.kk"
    echo "  node --test node/test.mjs"
    echo

    echo "======================= 1. COMPILE TO JS ============================"
    # Grep away the per-module progress spam; keep warnings and errors.
    koka --target=js --outputdir=build -o build/store kk/store.kk 2>&1 \
        | grep -vE "^(parse|check|load|link)" || true
    echo "compiled: kk/store.kk -> build/kk_store.mjs (+ build/kk_store.kki)"
    echo

    echo "=================== 2. NEGATIVE: EXHAUSTIVENESS ====================="
    echo "A. non-exhaustive match in a TOTAL function — expected to FAIL:"
    if koka --target=js --outputdir=build-negA -o build-negA/launcher \
            kk/negative-nonexhaustive.kk >/tmp/pw-negA.log 2>&1; then
        echo "    UNEXPECTED: it compiled."
        A_REJECTED=0
    else
        grep -iE "error" /tmp/pw-negA.log | sed 's/^/    /' | head -4
        A_REJECTED=1
    fi
    echo "    rejected=$A_REJECTED"
    echo
    echo "B. same omission in a function declaring \`exn\` — expected to COMPILE:"
    if koka --target=js --outputdir=build-negB -o build-negB/launcher \
            kk/negative-nonexhaustive-exn.kk >/tmp/pw-negB.log 2>&1; then
        echo "    compiled cleanly (no exhaustiveness error)"
        B_COMPILED=1
        echo "    running it:"
        node build-negB/launcher.mjs 2>&1 | sed 's/^/      /' | head -4
    else
        echo "    UNEXPECTED: it failed to compile:"
        grep -iE "error" /tmp/pw-negB.log | sed 's/^/    /' | head -4
        B_COMPILED=0
    fi
    echo "    compiled=$B_COMPILED"
    echo
    echo "  => exhaustiveness_statically_enforced_only_for_total_functions=$(( A_REJECTED && B_COMPILED ))"
    echo "     (see finding F-8: Koka models a partial match as the \`exn\` effect,"
    echo "      not as an independent exhaustiveness rule)"
    echo

    echo "=================== 3. GENERATED REPRESENTATION ====================="
    echo "-- ADT constructors as emitted to JavaScript --"
    grep -E "^export (function|const) (QOk|QErr|Draft|Confirmed|StoreUnavailable|Store|Money_usd|Store_id)\b" -A2 build/kk_store.mjs \
        | grep -vE "^--$" | sed 's|/\*.*\*/||' | sed 's/^/  /'
    echo
    echo "-- std_core_types: Nothing / Nil --"
    grep -E "^export const (Nothing|Nil) " build/std_core_types.mjs | sed 's/^/  /'
    echo

    echo "============= 4. INFERRED EFFECT ROWS FROM .kki (no fork) ==========="
    node --input-type=module -e '
      import { parseInterface, effectNames } from "./node/kki.mjs";
      const d = parseInterface("./build/kk_store.kki");
      const names = ["calculate-subtotal","available-items","describe-order",
                     "load-store","load-store-subtotal",
                     "load-store-for-js","load-subtotal-for-js","main"];
      for (const n of names) {
        const e = effectNames(d.get(n).signature);
        console.log(`  ${n.padEnd(22)} !{${e.join(", ")}}${e.length ? "" : "   <- pure"}`);
      }
    '
    echo

    echo "======================== 5. RUNTIME OUTPUT =========================="
    node build/store.mjs 2>&1 | sed 's/^/  /'
    echo

    echo "========================== 6. NODE TESTS ==========================="
    node --test node/test.mjs 2>&1 | grep -E "^(ok|not ok|# (tests|pass|fail|skipped))"
} | tee "$OUT"

echo
echo "spike-koka-js-interop: evidence written to $OUT"
