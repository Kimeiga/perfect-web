#!/usr/bin/env bash
# just doctor — READ-ONLY environment check.
#
# PROJECT_CHARTER.md §13.4: "`just doctor` must be read-only and explain missing
# dependencies." This script must never install, download, modify, or delete
# anything. It only reads versions and compares them against tools/versions.lock.
#
# Exit code 0 = every tool REQUIRED by the current milestone is present.
# Exit code 1 = at least one required tool is missing or too old.

set -uo pipefail

REPO_ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
LOCKFILE="$REPO_ROOT/tools/versions.lock"
PREFIX_BIN="$REPO_ROOT/.toolchain/prefix/bin"
export PATH="$PREFIX_BIN:$PATH"

missing=0
stale=0

bold() { printf '\033[1m%s\033[0m\n' "$*"; }
ok()   { printf '  \033[32m✓\033[0m %-14s %s\n' "$1" "$2"; }
warn() { printf '  \033[33m!\033[0m %-14s %s\n' "$1" "$2"; }
bad()  { printf '  \033[31m✗\033[0m %-14s %s\n' "$1" "$2"; }

# pinned <tool>  -> version string recorded in tools/versions.lock, or "" if absent
pinned() {
    [ -f "$LOCKFILE" ] || { echo ""; return; }
    awk -v k="$1" -F'=' '
        /^[[:space:]]*#/ { next }
        NF >= 2 {
            key = $1; gsub(/^[ \t]+|[ \t]+$/, "", key)
            if (key != k) next
            sub(/^[^=]*=[ \t]*/, "", $0)
            gsub(/[ \t]+$/, "", $0)
            print; exit
        }' "$LOCKFILE"
}

# check <tool> <required|optional> <version-command> <milestone-needing-it> <how-to-get-it>
check() {
    local tool="$1" need="$2" cmd="$3" why="$4" how="$5"
    local found want
    if ! command -v "$tool" >/dev/null 2>&1; then
        if [ "$need" = required ]; then
            bad "$tool" "MISSING — needed by $why"
            printf '      get it: %s\n' "$how"
            missing=$((missing + 1))
        else
            warn "$tool" "absent — needed by $why (not required yet)"
            printf '      get it: %s\n' "$how"
        fi
        return
    fi
    found="$(eval "$cmd" 2>/dev/null | head -1)"
    want="$(pinned "$tool")"
    if [ -n "$want" ] && [ "$found" != "$want" ]; then
        warn "$tool" "$found  (tools/versions.lock pins: $want)"
        stale=$((stale + 1))
    else
        ok "$tool" "$found"
    fi
}

bold "perfect-web doctor — read-only environment check"
echo

bold "Host"
printf '  %-14s %s\n' "os"     "$(sw_vers -productName 2>/dev/null || uname -s) $(sw_vers -productVersion 2>/dev/null || uname -r)"
printf '  %-14s %s\n' "arch"   "$(uname -m)"
printf '  %-14s %s\n' "cpu"    "$(sysctl -n machdep.cpu.brand_string 2>/dev/null || echo unknown)"
printf '  %-14s %s\n' "cores"  "$(sysctl -n hw.ncpu 2>/dev/null || nproc 2>/dev/null || echo '?')"
printf '  %-14s %s\n' "memory" "$(( $(sysctl -n hw.memsize 2>/dev/null || echo 0) / 1073741824 )) GiB"
if [ "$(uname -m)" != "arm64" ]; then
    warn "arch" "expected arm64 (Apple Silicon); found $(uname -m)"
fi
mem_gib=$(( $(sysctl -n hw.memsize 2>/dev/null || echo 0) / 1073741824 ))
if [ "$mem_gib" -lt 32 ]; then
    warn "memory" "${mem_gib} GiB — charter §14 M11 VM table assumes headroom for ~14 GiB of guests."
    printf '      see: docs/ASSUMPTIONS.md A-001 (revised VM sizing for this host)\n'
fi
echo

bold "Required now (Milestone 0)"
check git    required 'git --version'                      "everything"                       "xcode-select --install"
check rustc  required 'rustc --version'                    "spikes/compiler-diagnostic, spikes/wasmtime-component" "rustup toolchain install (rust-toolchain.toml pins it)"
check cargo  required 'cargo --version'                    "the Rust workspace"               "ships with rustup"
check just   required 'just --version'                     "this command surface"             "brew install just"
check node   required 'node --version'                     "spikes/marko-stream-resume"       "brew install node@22 (>=22.12 required by vite 8)"
check pnpm   required 'pnpm --version'                     "spikes/marko-stream-resume"       "corepack enable pnpm"
check koka   required 'koka --version | head -1'           "spikes/koka-js-interop"           "just bootstrap"
check wasmtime required 'wasmtime --version'               "spikes/wasmtime-component"        "just bootstrap"
check jq     required 'jq --version'                       "evidence extraction in spikes"    "brew install jq"

# Charter v2 added two Milestone 0 spikes with their own toolchains.
if [ -x "/Applications/Google Chrome.app/Contents/MacOS/Google Chrome" ]; then
    ok "chrome" "$(/Applications/Google\ Chrome.app/Contents/MacOS/Google\ Chrome --version 2>/dev/null)"
else
    bad "chrome" "MISSING - needed by spikes/layout-phase-scheduler (charter v2 §7.5A)"
    printf '      get it: install Google Chrome, or set CHROME_PATH to another Chromium build\n'
    missing=$((missing + 1))
fi
check opam   optional 'opam --version'                     "spikes/bonsai-incremental-model (charter v2)" "brew install opam && opam switch create pw-bonsai 5.2.0"

echo
bold "Rust targets"
if command -v rustup >/dev/null 2>&1; then
    if rustup target list --installed 2>/dev/null | grep -qx 'wasm32-wasip2'; then
        ok "wasm32-wasip2" "installed"
    else
        bad "wasm32-wasip2" "MISSING — needed by spikes/wasmtime-component"
        printf '      get it: rustup target add wasm32-wasip2\n'
        missing=$((missing + 1))
    fi
else
    bad "rustup" "MISSING — needed to manage the pinned toolchain"
    missing=$((missing + 1))
fi

echo
bold "Needed by later milestones (not required now)"
check limactl  optional 'limactl --version'  "Milestone 11 (network lab)"     "brew install lima"
check mkcert   optional 'mkcert --version'   "Milestone 11 (local TLS)"       "brew install mkcert"
check hyperfine optional 'hyperfine --version' "Milestone 3+ (benchmarks)"    "brew install hyperfine"

echo
bold "Repository"
if [ -d "$REPO_ROOT/.git" ]; then ok "git repo" "initialized"; else bad "git repo" "not initialized"; missing=$((missing + 1)); fi
if [ -f "$LOCKFILE" ]; then ok "versions.lock" "present"; else warn "versions.lock" "absent — run: just env-record"; fi
if [ -f "$REPO_ROOT/docs/STATUS.md" ]; then
    ok "STATUS.md" "$(grep -m1 -i '^current milestone' -A0 "$REPO_ROOT/docs/STATUS.md" 2>/dev/null | head -1 || echo present)"
else
    bad "STATUS.md" "absent"; missing=$((missing + 1))
fi

echo
if [ "$missing" -gt 0 ]; then
    printf '\033[31mdoctor: %d required item(s) missing.\033[0m Run `just bootstrap` for the pinned toolchains.\n' "$missing"
    exit 1
fi
if [ "$stale" -gt 0 ]; then
    printf '\033[33mdoctor: OK, with %d version(s) differing from tools/versions.lock.\033[0m\n' "$stale"
    printf 'That is a warning, not a failure — but benchmarks and gate evidence must record what was actually used.\n'
    exit 0
fi
printf '\033[32mdoctor: OK — every tool required by the current milestone is present.\033[0m\n'
exit 0
