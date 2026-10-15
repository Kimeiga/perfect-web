#!/usr/bin/env bash
# just doctor — READ-ONLY environment check.
#
# PROJECT_CHARTER.md §13.4: "`just doctor` must be read-only and explain missing
# dependencies." This script must never install, download, modify, or delete
# anything. It only reads versions and compares them against tools/versions.lock.
#
# Exit code 0 = every tool REQUIRED by the current milestone is present.
# Exit code 1 = at least one required tool is missing or too old.
#
# Two kinds of host: the Mac the charter is written for (§13), and Linux —
# CI's ubuntu-24.04 runners, x86_64 and arm, and a WSL2 Ubuntu on a Windows
# PC. Where a fact is read from differs by `uname -s`; what is checked does not.

set -uo pipefail

REPO_ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
LOCKFILE="$REPO_ROOT/tools/versions.lock"
PREFIX_BIN="$REPO_ROOT/.toolchain/prefix/bin"
export PATH="$PREFIX_BIN:$PATH"

OS="$(uname -s)"
ARCH="$(uname -m)"

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

# version_of <string> -> the first dotted number in it, or "" when there is none.
version_of() { printf '%s' "$1" | grep -oE '[0-9]+(\.[0-9]+)+' | head -1; }

# differs <found> <pinned> -> true when the two name different versions. A
# build's stamp is no difference: "wasmtime 48.0.5 (563544c62 2026-10-02)" is
# the lock's "wasmtime 48.0.5", and a Koka 3.2.3 built at 01:25:33 is one
# built at 01:29:29. "jq-1.6" is not "jq-1.7.1-apple". A string with no
# version in it is compared whole.
differs() {
    local found want
    found="$(version_of "$1")"
    want="$(version_of "$2")"
    if [ -n "$found" ] && [ -n "$want" ]; then
        [ "$found" != "$want" ]
    else
        [ "$1" != "$2" ]
    fi
}

# hint <tool> -> how to get it on this OS. The Mac's hints are the charter's
# (§13.2: Homebrew for commodity host tools); Linux's are Debian/Ubuntu's, the
# OS CI runs and WSL2 ships by default.
hint() {
    case "$OS/$1" in
        Darwin/git)        echo "xcode-select --install" ;;
        */git)             echo "sudo apt-get install git" ;;
        */rustc)           echo "rustup toolchain install (rust-toolchain.toml pins it)" ;;
        */cargo)           echo "ships with rustup" ;;
        Darwin/just)       echo "brew install just" ;;
        */just)            echo "the release binary from https://github.com/casey/just/releases (tools/versions.lock pins: $(pinned just))" ;;
        Darwin/node)       echo "brew install node@22 (>=22.12 required by vite 8)" ;;
        */node)            echo "Node 22 from nodejs.org, nvm or NodeSource; .nvmrc names the version (>=22.12 required by vite 8)" ;;
        */pnpm)            echo "corepack enable pnpm" ;;
        */koka|*/wasmtime) echo "just bootstrap" ;;
        Darwin/jq)         echo "brew install jq" ;;
        */jq)              echo "sudo apt-get install jq" ;;
        Darwin/opam)       echo "brew install opam && opam switch create pw-bonsai 5.2.0" ;;
        */opam)            echo "sudo apt-get install opam && opam switch create pw-bonsai 5.2.0" ;;
        Darwin/limactl)    echo "brew install lima" ;;
        */limactl)         echo "https://lima-vm.io/docs/installation/" ;;
        Darwin/mkcert)     echo "brew install mkcert" ;;
        */mkcert)          echo "sudo apt-get install mkcert" ;;
        Darwin/hyperfine)  echo "brew install hyperfine" ;;
        */hyperfine)       echo "sudo apt-get install hyperfine" ;;
        *)                 echo "see tools/versions.lock" ;;
    esac
}

# check <tool> <required|optional> <version-command> <milestone-needing-it>
check() {
    local tool="$1" need="$2" cmd="$3" why="$4"
    local found want
    if ! command -v "$tool" >/dev/null 2>&1; then
        if [ "$need" = required ]; then
            bad "$tool" "MISSING — needed by $why"
            printf '      get it: %s\n' "$(hint "$tool")"
            missing=$((missing + 1))
        else
            warn "$tool" "absent — needed by $why (not required yet)"
            printf '      get it: %s\n' "$(hint "$tool")"
        fi
        return
    fi
    found="$(eval "$cmd" 2>/dev/null | head -1)"
    want="$(pinned "$tool")"
    if [ -n "$want" ] && differs "$found" "$want"; then
        warn "$tool" "$found  (tools/versions.lock pins: $want)"
        stale=$((stale + 1))
    else
        ok "$tool" "$found"
    fi
}

bold "perfect-web doctor — read-only environment check"
echo

bold "Host"
case "$OS" in
    Darwin)
        host_os="$(sw_vers -productName 2>/dev/null || uname -s) $(sw_vers -productVersion 2>/dev/null || uname -r)"
        host_cpu="$(sysctl -n machdep.cpu.brand_string 2>/dev/null || echo unknown)"
        host_cores="$(sysctl -n hw.ncpu 2>/dev/null || nproc 2>/dev/null || echo '?')"
        mem_gib=$(( $(sysctl -n hw.memsize 2>/dev/null || echo 0) / 1073741824 ))
        ;;
    *)
        # The distribution's name, then the kernel's: a WSL2 kernel names itself.
        host_os="$(lsb_release -ds 2>/dev/null)"
        [ -n "$host_os" ] || host_os="$(. /etc/os-release 2>/dev/null && printf '%s' "${PRETTY_NAME:-}")"
        host_os="${host_os:-$OS} ($OS $(uname -r))"
        host_cpu="$(awk -F': *' '/^model name/ { print $2; exit }' /proc/cpuinfo 2>/dev/null)"
        host_cpu="${host_cpu:-unknown}"
        host_cores="$(nproc 2>/dev/null || echo '?')"
        mem_kb="$(awk '/^MemTotal:/ { print $2; exit }' /proc/meminfo 2>/dev/null)"
        mem_gib=$(( ${mem_kb:-0} / 1048576 ))
        ;;
esac
printf '  %-14s %s\n' "os"     "$host_os"
printf '  %-14s %s\n' "arch"   "$ARCH"
printf '  %-14s %s\n' "cpu"    "$host_cpu"
printf '  %-14s %s\n' "cores"  "$host_cores"
printf '  %-14s %s\n' "memory" "$mem_gib GiB"
# The charter's machine is Apple Silicon (§13.1), so a Mac is held to arm64.
# Linux is x86_64 or aarch64: CI's ubuntu-24.04 and ubuntu-24.04-arm runners,
# and scripts/bootstrap.sh pins artifacts for both.
case "$OS/$ARCH" in
    Darwin/arm64 | Linux/x86_64 | Linux/aarch64 | Linux/arm64) ;;
    Darwin/*) warn "arch" "expected arm64 (Apple Silicon); found $ARCH" ;;
    *)        warn "arch" "$OS/$ARCH has no pinned toolchain artifacts (scripts/bootstrap.sh)" ;;
esac
if [ "$mem_gib" -lt 32 ]; then
    warn "memory" "${mem_gib} GiB — charter §14 M11 VM table assumes headroom for ~14 GiB of guests."
    printf '      see: docs/ASSUMPTIONS.md A-001 (revised VM sizing for this host)\n'
fi
echo

bold "Required now (Milestone 0)"
check git      required 'git --version'            "everything"
check rustc    required 'rustc --version'          "spikes/compiler-diagnostic, spikes/wasmtime-component"
check cargo    required 'cargo --version'          "the Rust workspace"
check just     required 'just --version'           "this command surface"
check node     required 'node --version'           "spikes/marko-stream-resume"
check pnpm     required 'pnpm --version'           "spikes/marko-stream-resume"
check koka     required 'koka --version | head -1' "spikes/koka-js-interop"
check wasmtime required 'wasmtime --version'       "spikes/wasmtime-component"
check jq       required 'jq --version'             "evidence extraction in spikes"

# Charter v2 added two Milestone 0 spikes with their own toolchains.
# spikes/layout-phase-scheduler runs the browser CHROME_PATH names, else the
# Mac's Google Chrome. Looked for here in that order, then where the spike does
# not look: a Chrome or Chromium on PATH, and the Chrome for Testing that
# Playwright installs (spikes/own-renderer; ~/.cache/ms-playwright on Linux).
# Either of those two reaches the spike through CHROME_PATH, so the line to
# export is printed.
chrome=""
chrome_how=""
if [ -n "${CHROME_PATH:-}" ]; then
    chrome="$CHROME_PATH"
    chrome_how="CHROME_PATH"
elif [ -x "/Applications/Google Chrome.app/Contents/MacOS/Google Chrome" ]; then
    chrome="/Applications/Google Chrome.app/Contents/MacOS/Google Chrome"
    chrome_how="the spike's default"
else
    for name in google-chrome google-chrome-stable chromium chromium-browser; do
        if command -v "$name" >/dev/null 2>&1; then
            chrome="$(command -v "$name")"
            chrome_how="on PATH"
            break
        fi
    done
    if [ -z "$chrome" ]; then
        # The newest build Playwright has installed, if any.
        chrome="$(ls -d "$HOME"/.cache/ms-playwright/chromium-*/chrome-linux*/chrome 2>/dev/null | sort -V | tail -1)"
        [ -z "$chrome" ] || chrome_how="Playwright's"
    fi
fi
if [ -z "$chrome" ]; then
    bad "chrome" "MISSING - needed by spikes/layout-phase-scheduler (charter v2 §7.5A)"
    if [ "$OS" = Darwin ]; then
        printf '      get it: install Google Chrome, or set CHROME_PATH to another Chromium build\n'
    else
        printf "      get it: install Google Chrome or Chromium, or Playwright's (cd spikes/own-renderer && pnpm exec playwright install chromium), or set CHROME_PATH to another Chromium build\n"
    fi
    missing=$((missing + 1))
elif [ ! -x "$chrome" ]; then
    bad "chrome" "$chrome ($chrome_how) is not an executable - needed by spikes/layout-phase-scheduler (charter v2 §7.5A)"
    missing=$((missing + 1))
elif chrome_version="$("$chrome" --version 2>/dev/null | head -1)" && [ -n "$chrome_version" ]; then
    ok "chrome" "$chrome_version"
    case "$chrome_how" in
        "on PATH" | "Playwright's")
            printf '      the spike takes it through CHROME_PATH: export CHROME_PATH="%s"\n' "$chrome"
            ;;
    esac
else
    bad "chrome" "$chrome ($chrome_how) does not run - needed by spikes/layout-phase-scheduler (charter v2 §7.5A)"
    printf '      it said: %s\n' "$("$chrome" --version 2>&1 | head -1)"
    if [ "$chrome_how" = "Playwright's" ]; then
        printf '      get its libraries: cd spikes/own-renderer && pnpm exec playwright install-deps chromium (the list: %s/deb.deps)\n' "$(dirname "$chrome")"
    fi
    missing=$((missing + 1))
fi
check opam optional 'opam --version' "spikes/bonsai-incremental-model (charter v2)"

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
check limactl   optional 'limactl --version'   "Milestone 11 (network lab)"
check mkcert    optional 'mkcert --version'    "Milestone 11 (local TLS)"
check hyperfine optional 'hyperfine --version' "Milestone 3+ (benchmarks)"

echo
bold "Repository"
# `.git` is a directory in a clone and a file in a worktree; either is the repository.
if [ -e "$REPO_ROOT/.git" ]; then ok "git repo" "initialized"; else bad "git repo" "not initialized"; missing=$((missing + 1)); fi
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
