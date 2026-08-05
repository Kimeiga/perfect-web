#!/usr/bin/env bash
# just bootstrap — fetch the pinned toolchains that are not managed by rustup/pnpm.
#
# PROJECT_CHARTER.md §3.7: "Before executing remote install scripts, download them
# from the official source, inspect them, and record the installed version."
# This script therefore downloads *release tarballs* and unpacks them into a
# repo-local prefix. It never pipes a remote script into a shell, never uses
# sudo, and never writes outside $REPO_ROOT/.toolchain.
#
# Everything it installs lands in .toolchain/ (gitignored). Removing that
# directory fully reverses this script.

set -euo pipefail

REPO_ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
TC="$REPO_ROOT/.toolchain"
DIST="$TC/dist"
PREFIX="$TC/prefix"

# Pinned in tools/versions.lock. Verified against upstream release APIs 2026-08-05.
KOKA_VERSION="3.2.3"
WASMTIME_VERSION="47.0.3"

# SHA-256 of the exact artifacts this repository was validated against.
KOKA_SHA256="ffe84e8c679876894ac67da23066ceae1fd433e244038a7c884a4ca8b4698eb8"
WASMTIME_SHA256="c2684249e5d9ef9351942cf2d315982cf201fe0300f05d63bc1527446f0cd37f"

arch="$(uname -m)"
os="$(uname -s)"
if [ "$os" != "Darwin" ] || [ "$arch" != "arm64" ]; then
    echo "bootstrap: this script currently only pins macOS/arm64 artifacts." >&2
    echo "  detected: $os/$arch" >&2
    echo "  Linux CI installs these from the same upstream releases; see .github/workflows/ci.yml" >&2
    exit 1
fi

mkdir -p "$DIST" "$PREFIX/bin"

verify() {  # verify <file> <expected-sha256>
    local actual
    actual="$(shasum -a 256 "$1" | cut -d' ' -f1)"
    if [ "$actual" != "$2" ]; then
        echo "bootstrap: CHECKSUM MISMATCH for $1" >&2
        echo "  expected $2" >&2
        echo "  actual   $actual" >&2
        echo "  Refusing to unpack. Upstream may have re-published, or the download is corrupt." >&2
        echo "  Investigate before changing the pin in this script." >&2
        exit 1
    fi
    echo "  checksum ok: $(basename "$1")"
}

# --- Koka ------------------------------------------------------------------
if [ -x "$PREFIX/bin/koka" ] && "$PREFIX/bin/koka" --version 2>/dev/null | grep -q "$KOKA_VERSION"; then
    echo "koka $KOKA_VERSION already present"
else
    tarball="$DIST/koka-v${KOKA_VERSION}-macos-arm64.tar.gz"
    url="https://github.com/koka-lang/koka/releases/download/v${KOKA_VERSION}/koka-v${KOKA_VERSION}-macos-arm64.tar.gz"
    echo "fetching koka $KOKA_VERSION"
    [ -f "$tarball" ] || curl -sSL --fail --max-time 300 -o "$tarball" "$url"
    verify "$tarball" "$KOKA_SHA256"
    tar xzf "$tarball" -C "$PREFIX"
    echo "  installed: $("$PREFIX/bin/koka" --version | head -1)"
fi

# --- Wasmtime --------------------------------------------------------------
if [ -x "$PREFIX/bin/wasmtime" ] && "$PREFIX/bin/wasmtime" --version 2>/dev/null | grep -q "$WASMTIME_VERSION"; then
    echo "wasmtime $WASMTIME_VERSION already present"
else
    tarball="$DIST/wasmtime-v${WASMTIME_VERSION}-aarch64-macos.tar.xz"
    url="https://github.com/bytecodealliance/wasmtime/releases/download/v${WASMTIME_VERSION}/wasmtime-v${WASMTIME_VERSION}-aarch64-macos.tar.xz"
    echo "fetching wasmtime $WASMTIME_VERSION"
    [ -f "$tarball" ] || curl -sSL --fail --max-time 300 -o "$tarball" "$url"
    verify "$tarball" "$WASMTIME_SHA256"
    tar xJf "$tarball" -C "$DIST"
    cp "$DIST/wasmtime-v${WASMTIME_VERSION}-aarch64-macos/wasmtime" "$PREFIX/bin/wasmtime"
    cp "$DIST/wasmtime-v${WASMTIME_VERSION}-aarch64-macos/LICENSE" "$PREFIX/WASMTIME-LICENSE"
    echo "  installed: $("$PREFIX/bin/wasmtime" --version)"
fi

# --- Rust target -----------------------------------------------------------
if command -v rustup >/dev/null 2>&1; then
    if ! rustup target list --installed 2>/dev/null | grep -qx 'wasm32-wasip2'; then
        echo "adding rust target wasm32-wasip2"
        rustup target add wasm32-wasip2
    else
        echo "rust target wasm32-wasip2 already present"
    fi
fi

# --- Node workspace --------------------------------------------------------
if command -v pnpm >/dev/null 2>&1; then
    echo "installing node workspace dependencies (pnpm, frozen lockfile if present)"
    if [ -f "$REPO_ROOT/pnpm-lock.yaml" ]; then
        (cd "$REPO_ROOT" && pnpm install --frozen-lockfile)
    else
        (cd "$REPO_ROOT" && pnpm install)
    fi
else
    echo "pnpm not found — enable it with: corepack enable pnpm" >&2
fi

echo
echo "bootstrap: done. Everything above lives in .toolchain/ and node_modules/."
echo "Add to PATH for direct use:  export PATH=\"$PREFIX/bin:\$PATH\""
echo "Run 'just doctor' to verify."
