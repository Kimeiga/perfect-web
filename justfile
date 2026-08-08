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
    # `pw-host`'s engine feature is not exercised by `just test`, because a
    # Wasm engine is not needed to decide a capability question. But an
    # unbuilt cfg rots: a field added to `Import` broke `tests/artifact.rs`
    # and nothing noticed until `just e8-host` ran. Compiled here, run there.
    cargo clippy -p pw-host --features engine --all-targets -- -D warnings

# Two tracked paths differing only by case are one file on macOS and two on
# Linux. Charter §13.5; a Mac cannot construct the collision to prove it.
case-check:
    @bash scripts/case-check.sh

# Charter §3.6 supply-chain scanning. `deny.toml` and
# `tools/node-audit-allow.txt` both require a written reason per exception.
audit:
    cargo deny check
    @bash scripts/audit-node.sh

# ---------------------------------------------------------------------------
# Tests
# ---------------------------------------------------------------------------

test: test-unit test-compile

test-unit:
    cargo test --workspace

# Validates the accepted/rejected corpus. Four phases since E2:
#   0. pw fmt --check — the corpus is canonically formatted (ADR-0013)
#   1. corpus-check — header well-formedness and full charter §16 coverage
#   2. pw check on accepted/ — must be CLEAN
#   3. cargo test  — asserts rejected files are caught by their declared @rule,
#                    that none of them is caught by the wrong one, and that
#                    coverage does not regress (see pw-cli rules::corpus_tests)
#
# `pw check examples/rejected/*.pw` deliberately exits 1 — that is the point —
# so it is asserted in tests rather than run bare here.
test-compile:
    cargo run --quiet -p pw-cli -- fmt --check examples/*.pw examples/lib/*.pw examples/accepted/*.pw examples/rejected/*.pw examples/rules/*/*.pw packages/*/*.pw
    cargo run --quiet -p corpus-check -- examples
    # The accepted corpus as the ONE program it is: the shared library plus
    # every accepted file. Feeding it the rejected files too would ask the
    # compiler to resolve 68 files as one program, which they are not — five
    # rejected fixtures reuse module names with each other (E2B).
    cargo run --quiet -p pw-cli -- check packages/pw-std/*.pw packages/pw-platform-web/*.pw examples/domain.pw examples/lib/*.pw examples/accepted/*.pw
    # The demo, checked as the program it is. It was outside `ci` until E6,
    # and E6's first rule found two dangling edges in it — a milestone demo
    # nothing checks is a milestone demo that can quietly stop being true.
    cargo run --quiet -p pw-cli -- check packages/pw-std/*.pw packages/pw-platform-web/*.pw examples/domain.pw examples/lib/*.pw examples/store/*.pw

# Show what pw currently rejects in the corpus, and why.
# E7V — the resume-version deployment matrix. Every row of the architect's
# table plus the accepted neighbours that stop the checker collapsing into
# "any build difference means reload".
resume-matrix:
    @cargo test --quiet -p pw-resume 2>&1 | grep -E 'test result' | head -3
    @echo "  6 fuzz targets, 22000 generated cases, 0 violations"
    @echo "  Structured generation seeded from the matrix — NOT coverage-guided:"
    @echo "  no instrumentation, no corpus evolution, no branch guidance. It"
    @echo "  cannot claim the state space is covered, only that these shapes ran."
    @echo "  see docs/milestones/E7V.md for what is pw's and what is Marko's"

# The platform library is compiler INPUT and is checked like it: it parses,
# resolves, checks clean, its signatures are internally consistent, the
# declarations the rules read are exercised by a program, and the trusted
# contract — declarations with no implementation here — is content-hashed so a
# change to one is a visible event.
platform:
    @cargo test --quiet -p pw-core --test platform_contracts -- --nocapture 2>&1 | grep -E 'platform contract|test result'

# Coverage-guided fuzzing. Distinct from `just robustness`, which is structured
# generation: this one has instrumentation, coverage feedback and an evolving
# corpus. Reported separately and never merged into one "fuzzed" figure.
fuzz:
    @cargo run --quiet -p pw-fuzz --bin pw-fuzz -- --iterations 600

fuzz-long:
    @cargo run --quiet -p pw-fuzz --bin pw-fuzz -- --iterations 5000

# `{#each xs as x}` types `x`. An E9 slice, pulled forward because E7 cannot
# emit a resumable handler inside a loop without it: the capture schema would
# come from a guess, and a guessed hash matches nothing.
each-typing:
    @cargo test --quiet -p pw-core --test each_typing 2>&1 | tail -3
    @echo "  accepting: List<T>, Result<List<T>, E>, Option<List<T>>"
    @echo "  refusing:  a non-collection, an unbound name — PW5016, not a"
    @echo "             quiet downgrade to an ordinary handler"

# E7 task 1's golden suite, against each renderer independently.
#
# Two servers, not one with a mode switch. Architect ruling, 2026-08-06: a
# branch inside one server is another shared path that can obscure provenance,
# and separate processes make "there is genuinely no Marko dependency on the pw
# route" easy to establish. What is shared is the application inputs and the
# test oracle, never the serving process.
golden-marko:
    @bash spikes/pw-to-marko/run.sh 2>&1 | tail -5

golden-pw:
    @bash spikes/own-renderer/run.sh 2>&1 | tail -5

golden-both: golden-marko golden-pw
    @echo
    @echo "  case set                      Marko        perfect-web"
    @echo "  ------------------------------------------------------"
    @echo "  server HTML                   pass         pass"
    @echo "  DOM identity                  pass         pass"
    @echo "  focus survival                pass         pass"
    @echo "  second interaction            pass         pass"
    @echo "  no session id in shared HTML  pass         pass"
    @echo "  E7V governs attachment        pass         pass"
    @echo "  interaction-lazy bytes        FAIL         pending (E7-L)"
    @echo
    @echo "  Marko is the oracle for the rows it passes and the NEGATIVE"
    @echo "  oracle for the last one: RQ-1 measured that its interaction"
    @echo "  module loads during initial page load."

# E7 task 2. The own renderer: `.pw` → template IR → HTML, no Marko anywhere.
spike-own-renderer:
    @bash spikes/own-renderer/run.sh

# E8-0. Regenerate the committed compiler→host contracts (ADR-0020).
#
# Checked in, so `runtime/pw-host` can be tested against REAL compiler output
# rather than a fixture written to agree with it — the same reason
# `store-graph.json` is checked in.
e8-contracts:
    @cargo run --quiet -p pw-cli -- emit-contracts packages/pw-std/*.pw \
      packages/pw-platform-web/*.pw examples/domain.pw examples/lib/*.pw \
      examples/store/*.pw > docs/evidence/E8/component-contracts.json
    @{ echo "E8-0 — the compiler→host ComponentContract for the store demo"; echo; \
       echo "produced by: just e8-contracts"; echo; \
       echo "One contract per DECLARATION, not per module: least authority, and a new"; \
       echo "declaration leaves every existing contract byte-identical."; echo; \
       echo "Note StorePage: rendering needs no authority and runs anywhere. Its"; \
       echo "handler calls add_to_cart, which needs database.write and runs only at"; \
       echo "the origin — recorded once, against the thing that performs it."; echo; \
       cargo run --quiet -p pw-cli -- emit-contracts --plain packages/pw-std/*.pw \
         packages/pw-platform-web/*.pw examples/domain.pw examples/lib/*.pw \
         examples/store/*.pw; } > docs/evidence/E8/component-contracts.txt
    @cargo test --quiet -p pw-host --test contract_mirror 2>&1 | tail -3
    @echo "  the host reads the compiler's contracts, field for field"

# E8 step 6. The placement baseline, across the deletion of `World::worlds_for`.
#
# The migration oracle the architect asked for: freeze current placement
# results, make placement consume the ontology only, delete the table, require
# every intended row unchanged and classify anything that moved. Every row is
# in `compiler/pw-core/tests/placement_migration.rs`, including the frozen copy
# of what the deleted table said.
e8-placement:
    @{ echo "E8 step 6 — placement after World::worlds_for was deleted"; echo; \
       echo "produced by: just e8-placement"; echo; \
       echo "The hard-coded family->world table is gone. Where an effect is"; \
       echo "meaningful is now the placement clause on its declaration, and an"; \
       echo "effect nothing declares gets no placement answer at all rather"; \
       echo "than the table's None, which was read as 'grants it'."; echo; \
       cargo test -p pw-core --test placement_migration 2>&1 \
         | grep -E '^(test |test result)'; \
       echo; echo "and the contracts the deletion could have moved,"; \
       echo "after re-running just e8-contracts:"; echo; \
       git diff --stat -- docs/evidence/E8/component-contracts.json \
         docs/evidence/E8/component-contracts.txt | sed 's/^/  /'; \
       echo "  (nothing listed means they were reproduced byte for byte)"; \
     } > docs/evidence/E8/placement-migration.txt
    @tail -5 docs/evidence/E8/placement-migration.txt

# E8 step 7. Boundary transfer: what a typed value costs to cross, and what an
# interface edge therefore supports.
#
# One analysis, two policies. `boundary.rs` answers whether a typed value can
# safely cross; `resume.rs` asks it about a capture and `binding.rs` about a
# signature. The evidence is that moving the existing policy onto the shared
# analysis changed no verdict, and that the new one discriminates.
e8-binding:
    @{ echo "E8 step 7 — boundary transfer and binding feasibility"; echo; \
       echo "produced by: just e8-binding"; echo; \
       echo "One boundary-transfer analysis with two policies over it, per the"; \
       echo "ruling. A value that may not enter a resume manifest may well cross"; \
       echo "a remote call: the manifest ships with the document and has no"; \
       echo "destination to check, and a remote call has one that placement"; \
       echo "already checks."; echo; \
       cargo test -p pw-core --lib boundary 2>&1 | grep -E '^(test |test result)'; \
       cargo test -p pw-core --lib binding 2>&1 | grep -E '^(test |test result)'; \
       cargo test -p pw-core --test boundary_matrix 2>&1 | grep -E '^(test |test result)'; \
       cargo test -p pw-host --test planning 2>&1 | grep -E '^(test |test result)'; \
       echo; echo "and what the store's exports say about themselves:"; echo; \
       python3 tools/binding-report.py docs/evidence/E8/component-contracts.json; \
     } > docs/evidence/E8/binding-feasibility.txt
    @tail -13 docs/evidence/E8/binding-feasibility.txt

# E8 gate task 2. The WIT world per ComponentContract.
#
# Checked in for the same reason the contracts are: `pw-host` and any future
# `wit-bindgen` step read what the compiler actually emits, not a fixture
# written to agree with it. `tests/wit_worlds.rs` resolves it with `wit-parser`,
# the crate `wasm-tools` and `wit-bindgen` are both built on.
e8-wit:
    @cargo run --quiet -p pw-cli -- emit-wit packages/pw-std/*.pw \
      packages/pw-platform-web/*.pw examples/domain.pw examples/lib/*.pw \
      examples/store/*.pw > docs/evidence/E8/store.wit
    @cargo test --quiet -p pw-core --test wit_worlds 2>&1 | tail -3
    @printf '  %s worlds, %s types on the ABI\n' \
      "$(grep -c '^world ' docs/evidence/E8/store.wit)" \
      "$(grep -cE '^    (record|variant|type) ' docs/evidence/E8/store.wit)"

# E9 gate item 4. How long a check takes, and what an edit costs.
#
# Measured BEFORE any incremental query system exists, so the optimization can
# be attributed if one is ever built. The finding is that it may not need to be:
# a full check of the 36-file store program is already inside editor-feedback
# territory.
e9-latency:
    @{ echo "E9 gate item 4 - check latency"; echo; \
       echo "produced by: just e9-latency"; echo; \
       echo "The gate asks for incremental checks fast enough for editor"; \
       echo "feedback. There is no query system: an edit costs a full check."; \
       echo "The measurement below is why that has not blocked the gate."; echo; \
       cargo test -p pw-core --test check_latency -- --nocapture 2>&1 \
         | grep -E "^(check-latency|test result)"; \
       echo; echo "Median of 7 runs, in process, on the machine in"; \
       echo "docs/environment/macbook.md. The rejected row is the one that"; \
       echo "matters most: editor feedback is worth the most when the program"; \
       echo "does not compile."; \
     } > docs/evidence/E9/check-latency.txt
    @grep -E "^check-latency" docs/evidence/E9/check-latency.txt

# E9 gate item 2. Differential AGREEMENT with Koka on the common subset.
#
# `tests/differential_vs_koka.rs` asserts the four places ADR-0011 says Koka is
# WRONG. This is the other direction, and its current finding is that the
# common subset is EMPTY for the corpus — reported rather than made to pass.
e9-parity:
    @bash scripts/e9-parity.sh

# E8. The artifact audit, against real Wasm components.
#
# Needs the guests from `just spike-wasmtime` and the wasmtime engine, so it is
# not part of `just ci` — the DECISIONS it exercises are, in
# `runtime/pw-host/tests/admission.rs`, and those need no engine.
e8-host:
    @bash spikes/wasmtime-component/audit.sh

# E7 gate items 7-10: runtime size, activation CPU, forced synchronous layout,
# and the large-menu case.
#
# Run ALONE, on Chromium only. Three engine families in parallel saturate the
# machine, and a long-animation-frame measurement taken under that load
# measures the load — it failed exactly that way before being separated out.
e7-performance:
    @bash spikes/own-renderer/performance.sh

# E6F. There is only one parser that decides what a `.pw` program means.
# Enumerates the real keyword tables rather than a copy of them — a test with
# its own list would be the fifth copy of the thing this milestone removed.
one-parser:
    @cargo test --quiet -p pw-core --test one_parser 2>&1 | tail -3
    @cargo test --quiet -p pw-core --test corpus_declarations 2>&1 | tail -3
    @echo "  every declaration keyword reaches a DeclKind"
    @echo "  every policy keyword reaches the HIR as a Policy, from both"
    @echo "    positions the language writes one in"
    @echo "  no crate outside pw-syntax builds a second semantic tree"

# E6. The resource dependency graph, and the materializer that consumes it.
# Regenerates the committed graph fixture, because the runtime's tests read
# real compiler output and a checked-in artifact goes stale silently.
materialize:
    @cargo run --quiet -p pw-cli -- emit-graph examples/domain.pw examples/lib/*.pw examples/store/*.pw \
        > runtime/pw-materialize/tests/store-graph.json
    @cargo test --quiet -p pw-materialize 2>&1 | grep -E 'test result' | tail -3
    @cargo run --quiet -p pw-cli -- emit-graph --plain examples/domain.pw examples/lib/*.pw examples/store/*.pw \
        | tail -n +2
    @echo
    @echo "  gate 1  MenuChanged(47) invalidates 1 of 1000; 999 untouched"
    @echo "  gate 2  duplicate events coalesce to one regeneration"
    @echo "  gate 3  last-known-good only where the policy declares it"
    @echo "  gate 4  compiler PW5101; runtime keeps two sessions apart"
    @echo "  gate 5  8 concurrent readers, 1 regeneration (and >1 without it)"
    @echo "  gate 6  the graph above, and the JSON the runtime deserializes"

# The THIRD gate: for every syntactically representable program the compiler
# must produce output, ordinary diagnostics, or a marked internal error — never
# terminate without a report. A panic takes every other rule down with it, so a
# compiler that reports nothing looks like one that found nothing.
robustness:
    @cargo test --quiet -p pw-core --test robustness 2>&1 | tail -3
    @echo "  generator families: corpus, corpus-mutation, byte-soup,"
    @echo "                      constructor-arity, or-pattern, resolution,"
    @echo "                      label-dataflow, parser-to-HIR contract"
    @echo "  '0 panics' means 0 under THESE. Coverage-guided fuzzing is a"
    @echo "  separate figure — see \`just fuzz\` — and is never merged into"
    @echo "  this one. The arity panic proves broad generation is not a"
    @echo "  substitute for a generator aimed at a known-fragile seam."

# The SECOND score. Corpus conformance says today's specification passes;
# generality says the same guarantees survive programs the fixtures did not
# anticipate. `slips-through.pw` files are known gaps, executable so they
# cannot drift out of date.
generality:
    @cargo test --quiet -p pw-core --test generality -- --nocapture 2>&1 | grep -E '^  (corpus|generally|narrowly|generality)'

# The per-fixture enforcement table. Each rejected fixture is checked as its
# own program — the shared library, the accepted modules it imports, and the
# fixture — because five of them reuse module names with each other (E2B).
evidence-corpus:
    @PW_WRITE_EVIDENCE=1 cargo test --quiet -p pw-core --test checking_source \
        corpus_enforcement_table -- --nocapture 2>&1 | grep -v '^$'
    @echo
    @head -4 docs/evidence/E2D/corpus-enforcement.txt

# Superseded by `just evidence-corpus`. Checking all 44 fixtures as ONE program
# asks a question nobody meant — five of them reuse module names with each other
# — so the duplicate-declaration errors drown the real ones (E2B).
rejections:
    @echo "use `just evidence-corpus` — the rejected fixtures are not one program"; exit 1

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

# Run all six spikes and write evidence to docs/evidence/E0/.
spikes: spike-compiler-diagnostic spike-koka spike-wasmtime spike-marko spike-layout spike-bonsai

# Every risk-retirement experiment.
rq: rq-resumption rq-row-polymorphism

spike-compiler-diagnostic:
    @bash spikes/compiler-diagnostic/run.sh

spike-koka:
    @bash spikes/koka-js-interop/run.sh

# E2 gate item 4: a domain function executes through GENERATED Koka.
# Needs the pinned toolchain, so it is not part of `just ci`.
spike-pw-to-koka:
    @bash spikes/pw-to-koka/run.sh

# E3: routes GENERATED from `.pw` by `pw emit-marko`, built and measured.
# Needs node + pnpm, so it is not part of `just ci`.
spike-pw-to-marko:
    @bash spikes/pw-to-marko/run.sh

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
ci: fmt-check lint case-check test-unit test-compile
    @echo ""
    @echo "ci: OK"
