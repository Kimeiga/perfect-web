#!/usr/bin/env bash
# E8 — the final artifact audit, against real components.
#
# The compiler says what a program needs. This reads what the BUILT THING
# actually imports, and refuses anything the contract does not allow.
#
# The adversarial guest is the ordinary one: `spikes/wasmtime-component/guest`
# declares one interface in its WIT world and demands fifteen, because Rust
# `std` on `wasm32-wasip2` injects fourteen `wasi:*` interfaces during runtime
# initialization. Nobody wrote it to attack anything, which is exactly what
# makes it the right adversary — undeclared authority arrives by DEFAULT.
set -euo pipefail
REPO_ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
EVIDENCE="$REPO_ROOT/docs/evidence/E8"
MINIMAL="$REPO_ROOT/spikes/wasmtime-component/guest-minimal/target/wasm32-wasip2/release/spike_wasmtime_guest_minimal.wasm"

if [ ! -f "$MINIMAL" ]; then
  echo "the guests are not built - run 'just spike-wasmtime' first" >&2
  exit 1
fi

mkdir -p "$EVIDENCE"
# `--nocapture`, because the import lists ARE the evidence. A summary line
# saying three tests passed is a claim about a test run; the fourteen interface
# names the refusal enumerates are the finding.
out="$(cd "$REPO_ROOT" && cargo test -p pw-host --features engine -- --nocapture --test-threads=1 2>&1)"
echo "$out"

{
  echo "E8 — the artifact audit"
  echo
  echo "produced by: just e8-host"
  echo
  echo "Three checks on data (no engine), fifteen more in tests/admission.rs:"
  echo "  placement and capability are separate and can disagree either way"
  echo "  a capability's type argument is part of what is granted"
  echo "  an instance receives its contract, not the node's whole capability set"
  echo "  a handle never carries the value behind it"
  echo "  a refused admission yields no capabilities at all"
  echo
  echo "And the artifact audit against real components:"
  echo
  # Unanchored: with `--test-threads=1 --nocapture` a print lands on the
  # same line as the test name that produced it, which is more useful than
  # either alone and defeats a `^` anchor.
  echo "$out" | grep -oE "(minimal guest imports|std guest imports|undeclared).*" || true
  echo
  echo "Typed linking, and resource limits per instance. Every refusal below is"
  echo "the ENGINE's, in its own words - a pre-flight check comparing lists"
  echo "would be a second implementation of instantiation's own rule:"
  echo
  echo "$out" | grep -oE "(linked:|refused by the engine|refused for|instantiation spent|the guest returned|ungranted call refused).*" | sed 's/^/  /' || true
  echo
  echo "And the positive control the architect added on 2026-08-08: authority"
  echo "USED, not only refused. Everything above shows the host saying no or"
  echo "linking; the line naming the guest's return value is the host saying"
  echo "yes and the guest receiving the answer - the id is the guest's own"
  echo "argument, the name is the host's data."
  echo
  echo "$out" | grep -E "^test |test result:" || true
} > "$EVIDENCE/artifact-audit.txt"
echo
echo "evidence written to $EVIDENCE/artifact-audit.txt"
