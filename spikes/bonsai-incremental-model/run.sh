#!/usr/bin/env bash
# spike: bonsai-incremental-model — run and record evidence.
#
# Charter §14 Milestone 0 task 12. Note the charter's own escape clause:
#   "build or run a minimal current Bonsai/Bonsai_web application on Apple
#    Silicon, OR document the exact blocker if the current toolchain cannot be
#    installed reproducibly"
#
# This script therefore never fails the milestone because of an upstream
# toolchain problem — it records precisely what happened instead.
set -uo pipefail

REPO_ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
SPIKE="$REPO_ROOT/spikes/bonsai-incremental-model"
EVIDENCE="$REPO_ROOT/docs/evidence/E0"
mkdir -p "$EVIDENCE"
cd "$SPIKE"

OUT="$EVIDENCE/spike-bonsai-incremental-model.txt"
SWITCH="${OPAM_SWITCH:-pw-bonsai}"

{
    echo "spike: bonsai-incremental-model"
    echo "generated: $(date -u +%Y-%m-%dT%H:%M:%SZ)"
    echo "host: $(uname -sm) / $(sw_vers -productVersion 2>/dev/null)"
    echo "command: just spike-bonsai"
    echo

    if ! command -v opam >/dev/null 2>&1; then
        echo "BLOCKER: opam is not installed."
        echo "  install with: brew install opam"
        echo "  then: opam init -y --bare --disable-sandboxing && opam switch create $SWITCH 5.2.0"
        exit 0
    fi

    echo "opam:  $(opam --version)"
    eval "$(opam env --switch="$SWITCH" 2>/dev/null)" || true
    echo "switch: $(opam switch show 2>/dev/null)"
    echo "ocaml:  $(ocaml -version 2>/dev/null || echo '(absent)')"
    echo

    echo "=========== INSTALLED PACKAGES ==============================="
    opam list --installed 2>/dev/null \
        | grep -iE "^(bonsai|incremental|incr_dom|js_of_ocaml|virtual_dom|core) " \
        || echo "  (none of bonsai/incremental/js_of_ocaml/virtual_dom installed)"
    echo

    have_incremental=$(opam list --installed 2>/dev/null | grep -cE "^incremental " || true)
    have_bonsai=$(opam list --installed 2>/dev/null | grep -cE "^bonsai " || true)

    echo "=========== 1. INCREMENTAL: UNRELATED UPDATE TEST ============"
    echo "Charter §14 M0 task 12: 'demonstrate that an unrelated state update does"
    echo "not recompute an instrumented expensive derived value'."
    echo
    if [ "${have_incremental:-0}" -ge 1 ]; then
        if dune build ./incr_demo/incr_demo.exe 2>&1 | tail -20; then
            ./_build/default/incr_demo/incr_demo.exe 2>&1
        else
            echo "BLOCKER: incr_demo failed to build (output above)."
        fi
    else
        echo "BLOCKER: the 'incremental' opam package is not installed."
        echo "  reproduce with:"
        echo "    opam install -y --assume-depexts incremental"
        echo "  system packages required first: brew install pkgconf libffi zlib gmp openssl@3"
    fi
    echo

    echo "=========== 2. BONSAI APPLICATION ==========================="
    if [ "${have_bonsai:-0}" -ge 1 ]; then
        echo "bonsai is installed: $(opam list --installed 2>/dev/null | grep -E '^bonsai ')"
        echo "virtual_dom:         $(opam list --installed 2>/dev/null | grep -E '^virtual_dom ' || echo '(absent)')"
        echo
        echo "-- does bonsai depend on incremental? (from opam metadata) --"
        opam show bonsai --field depends 2>/dev/null | tr ',' '\n' \
            | grep -iE "incremental|virtual_dom|js_of_ocaml" | sed 's/^ */  /' || true
    else
        echo "BLOCKER: the 'bonsai' opam package is not installed."
        echo "  See docs/evidence/E0/spike-bonsai-incremental-model.txt and the"
        echo "  spike README for the exact failure sequence encountered."
    fi
    echo

    echo "=========== 3. SOURCE INSPECTION ============================"
    echo "Charter §14 M0 task 12: 'confirm from source which portions use Jane"
    echo "Street Incremental and which portions still produce a virtual DOM and"
    echo "diff/patch it'."
    echo
    LIB="$(opam var lib 2>/dev/null || echo '')"
    if [ -n "$LIB" ] && [ -d "$LIB" ]; then
        echo "-- bonsai's declared library dependencies (from its dune-package) --"
        grep -oE "\b(incremental|incr_map|ui_incr|virtual_dom|js_of_ocaml)[a-z_.]*" \
            "$LIB/bonsai/dune-package" 2>/dev/null | sort -u | sed 's/^/    /' || echo "    (unavailable)"
        echo
        echo "-- bonsai modules naming Incremental --"
        ls "$LIB/bonsai/" 2>/dev/null | grep -iE "incr.*\.mli?$" | sed 's/^/    /' | head -6 || echo "    (none)"
        echo
        echo "-- THE VIRTUAL DOM DIFF/PATCH LOOP, from virtual_dom/node.mli --"
        awk '/^module Patch : sig/,/^end/' "$LIB/virtual_dom/node.mli" 2>/dev/null | sed 's/^/    /' \
            || echo "    (virtual_dom not installed)"
        echo
        echo "    ^ Patch.create ~previous ~current compares two whole trees."
        echo "      Charter §8.4: \"Do not conceptually rerender the entire component"
        echo "      and compare virtual trees.\" Bonsai's incrementality governs the"
        echo "      COMPUTATION graph; rendering still goes through a vdom diff."
    else
        echo "  (opam lib dir unavailable)"
    fi
} 2>&1 | tee "$OUT"

echo
echo "spike-bonsai-incremental-model: evidence written to $OUT"
