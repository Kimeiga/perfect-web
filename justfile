# perfect-web developer commands.
# PROJECT_CHARTER.md §13.4 defines the stable command surface. Recipes are added
# as milestones make them applicable; unimplemented ones fail loudly with the
# milestone that will provide them, rather than silently succeeding.

set shell := ["bash", "-uc"]
set positional-arguments

toolchain_bin := justfile_directory() / ".toolchain/prefix/bin"
export PATH := toolchain_bin + ":" + env_var('PATH')

default:
    @just --list

# ---------------------------------------------------------------------------
# Environment
# ---------------------------------------------------------------------------

# Read-only environment check. Installs nothing. Charter §13.4.
doctor:
    @bash scripts/doctor.sh

# Fetch and unpack the pinned toolchains recorded in tools/versions.lock.
bootstrap:
    @bash scripts/bootstrap.sh

# Record the current machine state into docs/environment/macbook.md. Charter §13.1.
env-record:
    @bash scripts/record-environment.sh

# ---------------------------------------------------------------------------
# Quality
# ---------------------------------------------------------------------------

fmt:
    cargo fmt --all
    @if [ -d node_modules ]; then pnpm -r --if-present exec prettier --write . ; else echo "(skip) node_modules absent — run: just bootstrap"; fi

fmt-check:
    cargo fmt --all -- --check

lint:
    cargo clippy --workspace --all-targets -- -D warnings

# ---------------------------------------------------------------------------
# Tests
# ---------------------------------------------------------------------------

test: test-unit test-compile

test-unit:
    cargo test --workspace

# Validates the accepted/rejected corpus. Two phases since E2:
#   1. corpus-check — header well-formedness and full charter §16 coverage
#   2. pw check     — every file must PARSE
# Type checking is not yet wired to the parser; see docs/milestones/E2.md.
test-compile:
    cargo run --quiet -p corpus-check -- examples
    cargo run --quiet -p pw-cli -- check examples/accepted/*.pw examples/rejected/*.pw

# `pw explain` over an example — the semantic facts a developer would otherwise
# have to infer by reading the whole file.
explain FILE:
    @cargo run --quiet -p pw-cli -- explain {{FILE}}

test-integration:
    @echo "test-integration: not yet applicable (first delivered in Milestone 3)"; exit 1

test-e2e:
    @echo "test-e2e: not yet applicable (first delivered in Milestone 3, Playwright)"; exit 1

test-security:
    @echo "test-security: not yet applicable (first delivered in Milestone 5)"; exit 1

bench:
    @echo "bench: not yet applicable (first delivered in Milestone 3)"; exit 1

demo:
    @echo "demo: not yet applicable (first delivered in Milestone 3)"; exit 1

lab-up:
    @echo "lab-up: not yet applicable (first delivered in Milestone 11)"; exit 1

lab-test:
    @echo "lab-test: not yet applicable (first delivered in Milestone 11)"; exit 1

lab-down:
    @echo "lab-down: not yet applicable (first delivered in Milestone 11)"; exit 1

# ---------------------------------------------------------------------------
# Milestone 0 feasibility spikes (charter §14 M0 task 7)
# ---------------------------------------------------------------------------

# Run all six spikes and write evidence to docs/evidence/M0/.
spikes: spike-compiler-diagnostic spike-koka spike-wasmtime spike-marko spike-layout spike-bonsai

# Every risk-retirement experiment.
rq: rq-resumption rq-row-polymorphism

spike-compiler-diagnostic:
    @bash spikes/compiler-diagnostic/run.sh

spike-koka:
    @bash spikes/koka-js-interop/run.sh

spike-wasmtime:
    @bash spikes/wasmtime-component/run.sh

spike-marko:
    @bash spikes/marko-stream-resume/run.sh

# Charter §7.5A / §14 M0 task 13 — forced-synchronous-layout instrumentation.
spike-layout:
    @bash spikes/layout-phase-scheduler/run.sh

# Charter §14 M0 task 12 — Bonsai/Incremental study.
spike-bonsai:
    @bash spikes/bonsai-incremental-model/run.sh

# ---------------------------------------------------------------------------
# Risk-retirement experiments (see docs/RISK_QUEUE.md)
# These may use temporary dependencies to test assumptions early. Passing one
# retires a risk; it does NOT close the corresponding implementation milestone.
# ---------------------------------------------------------------------------

# RQ-1 — is Marko's resumption real, in Chrome AND Safari?
rq-resumption:
    @bash spikes/browser-resumption/run.sh

# RQ-2 — does Koka propagate effects through higher-order abstraction?
rq-row-polymorphism:
    @bash spikes/koka-row-polymorphism/run.sh

# ---------------------------------------------------------------------------
# CI
# ---------------------------------------------------------------------------

# The one command that must pass for the current milestone's gate.
ci: fmt-check lint test-unit test-compile
    @echo ""
    @echo "ci: OK"
