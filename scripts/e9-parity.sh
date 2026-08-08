#!/usr/bin/env bash
# E9 gate item 2 — differential AGREEMENT with Koka on the common subset.
#
# `tests/differential_vs_koka.rs` asserts the four places ADR-0011 says Koka is
# WRONG. Those are findings, and they are the opposite of a parity claim. This
# is the other direction: every accepted corpus file that lowers to the pure
# subset is compiled by the real Koka 3.2.3, and both compilers must accept it.
#
# Two negative controls, because a run that cannot fail measures nothing:
#   B. a deliberately non-exhaustive match must be REJECTED by Koka, which
#      proves the harness can see a disagreement at all;
#   C. a deliberately ill-typed term must be REJECTED by Koka, which proves the
#      acceptance above is Koka type-checking rather than Koka shrugging.
#
# ADR-0015 governs what the generated subset is. A file whose emitted module has
# no function bodies is NOT in the common subset and is counted as skipped
# rather than as agreement — silence is not parity.
set -uo pipefail

REPO_ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
EVIDENCE="$REPO_ROOT/docs/evidence/E9"
BUILD="$(mktemp -d)"
export PATH="$REPO_ROOT/.toolchain/prefix/bin:$PATH"
mkdir -p "$EVIDENCE"
trap 'rm -rf "$BUILD"' EXIT

command -v koka >/dev/null || { echo "koka not found — run: just bootstrap" >&2; exit 1; }

agreed=0
skipped=0
disagreed=0
declare -a DISAGREEMENTS=()
declare -a SKIPPED=()

# Compile one Koka module, discarding output. Returns 0 when Koka accepts.
koka_accepts() {
    koka -c --outputdir="$BUILD/out" "$1" >"$BUILD/koka.log" 2>&1
}

for src in "$REPO_ROOT"/examples/accepted/*.pw; do
    name="$(basename "$src")"
    raw="$BUILD/raw.kk"
    if ! (cd "$REPO_ROOT" && cargo run --quiet -p pw-cli -- emit-koka "$src") > "$raw" 2>/dev/null; then
        SKIPPED+=("$name  (emit-koka declined)")
        skipped=$((skipped + 1))
        continue
    fi
    # Koka requires a module's FILE NAME to match its `module` declaration, so
    # the name comes from the emitted source rather than from the `.pw` path.
    # Naming it after the fixture produced `build error: file path cannot be
    # mapped to a valid module name`, which is a harness failure that would have
    # been counted as a disagreement — the exact shape this project keeps
    # finding, in the instrument this time.
    module="$(grep -m1 '^module ' "$raw" | awk '{print $2}')"
    if [ -z "$module" ]; then
        SKIPPED+=("$name  (no module declaration emitted)")
        skipped=$((skipped + 1))
        continue
    fi
    mkdir -p "$BUILD/src"
    kk="$BUILD/src/$module.kk"
    cp "$raw" "$kk"
    # A module with no `pub fun` is a declaration-only lowering: nothing of the
    # program's behaviour crossed, so Koka accepting it says nothing.
    if ! grep -q "^pub fun " "$kk"; then
        SKIPPED+=("$name  (no function body in the pure subset)")
        skipped=$((skipped + 1))
        continue
    fi
    if koka_accepts "$kk"; then
        agreed=$((agreed + 1))
    else
        err="$(grep -iE 'error' "$BUILD/koka.log" | head -2 | tr '\n' ' ')"
        # **A type the emitted module does not declare is a limitation of the
        # BRIDGE, not a disagreement between the compilers.** `emit-koka` lowers
        # one module at a time and emits no imports, so a corpus file whose
        # types live in `domain.pw` cannot compile alone — and counting that as
        # "pw and Koka disagree" would be a false finding about the languages.
        if printf '%s' "$err" | grep -q "is not defined"; then
            SKIPPED+=("$name  (types declared in another module; emit-koka is single-module)")
            skipped=$((skipped + 1))
        else
            DISAGREEMENTS+=("$name: $err")
            disagreed=$((disagreed + 1))
        fi
    fi
done

# --- the controls ------------------------------------------------------------

control_b="$BUILD/control_b.kk"
cat > "$control_b" <<'KK'
module control_b

pub type state
  Draft
  Pricing
  Cancelled

// Non-exhaustive, under `total`. E0 finding F-8: under any other effect row
// Koka's exhaustiveness check is vacuous, which is why `emit-koka` writes
// `total` on every function it generates.
pub fun describe( s : state ) : total string
  match s
    Draft   -> "draft"
    Pricing -> "pricing"
KK
if koka_accepts "$control_b"; then control_b_result="FAIL (Koka accepted it)"; else control_b_result="pass"; fi

control_c="$BUILD/control_c.kk"
cat > "$control_c" <<'KK'
module control_c

pub fun bad() : total int
  "not an int"
KK
if koka_accepts "$control_c"; then control_c_result="FAIL (Koka accepted it)"; else control_c_result="pass"; fi

{
    echo "E9 gate item 2 — differential agreement with Koka"
    echo
    echo "produced by: just e9-parity"
    echo "koka: $(koka --version | head -1)"
    echo
    echo "Every accepted corpus file that lowers to the pure subset (ADR-0015),"
    echo "compiled by the real Koka. Both compilers must accept it."
    echo
    printf "  agreed     %3d\n" "$agreed"
    printf "  disagreed  %3d\n" "$disagreed"
    printf "  skipped    %3d   not in the common subset\n" "$skipped"
    echo
    if [ "${#DISAGREEMENTS[@]}" -gt 0 ]; then
        echo "DISAGREEMENTS:"
        printf '  %s\n' "${DISAGREEMENTS[@]}"
        echo
    fi
    echo "SKIPPED — counted as skipped, never as agreement. Silence is not parity:"
    printf '  %s\n' "${SKIPPED[@]}"
    echo
    echo "CONTROLS — a run that cannot fail measures nothing"
    echo "  B. non-exhaustive match under \`total\` is rejected by Koka: $control_b_result"
    echo "  C. ill-typed term is rejected by Koka:                      $control_c_result"
    echo
    echo "WHAT THIS IS NOT"
    echo "  Not a claim that pw and Koka agree on everything. ADR-0011 records"
    echo "  four places they deliberately do not, and tests/differential_vs_koka.rs"
    echo "  asserts each. This is the other direction: where the subsets overlap,"
    echo "  they must not disagree."
    echo "  Not a semantic equivalence proof. Koka accepting a module says its"
    echo "  types and exhaustiveness hold, not that it computes the same values."
    echo "  spikes/pw-to-koka executes one program and compares values; this"
    echo "  compiles many and compares verdicts."
} > "$EVIDENCE/koka-parity.txt"

cat "$EVIDENCE/koka-parity.txt"

# A disagreement, or a control that did not fire, is a failure of the gate item.
[ "$disagreed" -eq 0 ] || exit 1
[ "$control_b_result" = "pass" ] || exit 1
[ "$control_c_result" = "pass" ] || exit 1
# **Not an error, and this is the finding.** The common subset is currently
# empty for the corpus: every accepted file's types live in `domain.pw`, and
# `emit-koka` lowers one module with no imports. The controls above still fire,
# so the harness works; what it measures is a bridge that reaches one
# hand-curated file (`examples/koka/pricing.pw`, which `spikes/pw-to-koka`
# compiles and RUNS). Reported rather than made to pass.
if [ "$agreed" -eq 0 ]; then
    echo
    echo "FINDING: the common subset is EMPTY for the accepted corpus." >&2
    echo "  Not a harness failure — both controls fired. \`emit-koka\` lowers one" >&2
    echo "  module and emits no imports, so a file whose types live in" >&2
    echo "  \`domain.pw\` cannot be compiled alone. Gate item 2 cannot be met by" >&2
    echo "  pointing the oracle at the corpus until the bridge is multi-module." >&2
fi
