#!/usr/bin/env bash
# The store with a key a page changes (ADR-0152): the canonical store with
# T07's setup and reference applied, built into `dist-keyed`. The browser
# suite serves it on hosts of its own, one per engine, so keyed reads run in
# Chromium, Firefox and WebKit, as T07's controls run them in Chromium.
set -euo pipefail

REPO_ROOT="$(cd "$(dirname "$0")/../.." && pwd)"
SPIKE="$REPO_ROOT/spikes/own-renderer"
TASK="$REPO_ROOT/benchmarks/tasks/T07-stale-category"

WORK="$(mktemp -d)"
trap 'rm -rf "$WORK"' EXIT
SOURCES="$WORK/examples"
mkdir -p "$SOURCES"
cp "$REPO_ROOT/examples/domain.pw" "$SOURCES/"
cp -R "$REPO_ROOT/examples/lib" "$REPO_ROOT/examples/store" "$SOURCES/"
(cd "$SOURCES" && git apply "$TASK/setup/pleris.patch" && git apply "$TASK/reference/pleris.patch")

# Its own IR files, so the canonical store's, which the server's tests read,
# stay as they are.
PW_SOURCES="$SOURCES" PW_OUT="$SPIKE/dist-keyed" PW_STORE_IR="$WORK/store-ir.json" \
  PW_TEMPLATE_IR="$WORK/template-ir.json" BUILD_ONLY=1 bash "$SPIKE/run.sh" > /dev/null
echo "built the keyed store into $SPIKE/dist-keyed"
