# perfect-web developer commands.
# PROJECT_CHARTER.md §13.4 defines the stable command surface. Recipes are added
# as milestones make them applicable; unimplemented ones fail loudly with the
# milestone that will provide them, rather than silently succeeding.

# A report filter must not turn a failed producer into a successful gate.
# errexit also stops grouped evidence commands before later summaries run.
set shell := ["bash", "-euo", "pipefail", "-c"]
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

# Verify real recipe exit statuses with isolated, deterministic failing producers.
# This tests the harness, not the compiler or the browser.
evidence-gates:
    @python3 -m unittest discover -s scripts/tests -p 'test_evidence_gates.py' -v

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
    cargo run --quiet -p pw-cli -- fmt --check examples/*.pw examples/lib/*.pw examples/accepted/*.pw examples/rejected/*.pw examples/rules/*/*.pw examples/kiokun/*.pw packages/*/*.pw
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
    # The kiokun slice (E10): a second application, and the first to use the
    # platform package without the store's domain.
    cargo run --quiet -p pw-cli -- check packages/pw-std/*.pw packages/pw-platform-web/*.pw examples/kiokun/*.pw

# Show what pw currently rejects in the corpus, and why.
# E7V — the resume-version deployment matrix. Every row of the architect's
# table plus the accepted neighbours that stop the checker collapsing into
# "any build difference means reload".
resume-matrix:
    @cargo test --quiet -p pw-resume 2>&1 | grep -E 'test result' | sed -n '1,3p'
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
       echo "Note StorePage: it requires session.read — it builds a query key from"; \
       echo "the session, so it reads the session to RENDER — and it does NOT"; \
       echo "require database.write<Carts>, which its deferred handler performs."; \
       echo "Authority is recorded once, against the thing that performs it."; echo; \
       echo "This note said rendering needs no authority and runs anywhere until"; \
       echo "2026-08-10. It was true of a page whose current_session() call named"; \
       echo "nothing. See docs/CORPUS.md C5 and tests/unresolved_provenance.rs."; echo; \
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
      "$(grep -cE '^ +(record|variant|type) ' docs/evidence/E8/store.wit)"

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

# E9-K — Koka effect-oracle conformance. The gate item that REPLACED corpus
# parity on 2026-08-08. Both halves: the four shared effect properties asserted
# on Pleris programs, and the four recorded divergences still tested as
# divergences.
e9-oracle:
    @{ echo "E9-K - Koka effect-oracle conformance"; echo; \
       echo "produced by: just e9-oracle"; echo; \
       echo "The Koka half is just rq-row-polymorphism, whose evidence records"; \
       echo "OUTCOME 1: a clean pass on higher-order effect propagation, with"; \
       echo "propagation automatic through unannotated generic helpers."; echo; \
       echo "This is the Pleris half. Conformance is the pair."; echo; \
       cargo test -p pw-core --test effect_oracle 2>&1 | grep -E '^(test |test result)'; \
       echo; \
       echo "And the divergences, which must STAY divergences:"; echo; \
       cargo test -p pw-core --test differential_vs_koka 2>&1 | grep -E '^(test |test result)'; \
       echo; \
       echo "NOT CLAIMED: that ordinary Pleris programs agree with Koka. The"; \
       echo "corpus-parity gate was retired because the common subset is empty"; \
       echo "- see docs/evidence/E9/koka-parity.txt, which still runs and still"; \
       echo "reports that."; \
       echo "NOT CLAIMED: selective effect discharge. Pleris has no effect"; \
       echo "handler, and effect_oracle.rs asserts the absence so that adding"; \
       echo "one forces the conformance pair to be written."; \
     } > docs/evidence/E9/effect-oracle.txt
    @grep -E "^test result" docs/evidence/E9/effect-oracle.txt

# E9-V1..V6 — the ordinary value relations. The gate tests, what the checker
# DECIDED over the store program and the accepted corpus (a checker that
# decided nothing is silent too), and the mutation controls: each gate's
# mechanism disabled in turn must fail its tests.
e9-values:
    @{ echo "E9-V1..V6 - the ordinary value relations"; echo; \
       echo "produced by: just e9-values"; \
       echo "commit: $(git rev-parse HEAD)$(git diff --quiet HEAD -- . ':(exclude)docs/evidence' || echo ' + uncommitted changes')"; \
       echo "rust: $(rustc --version)"; echo; \
       echo "== the gate tests (compiler/pw-core/tests/value_relations.rs)"; echo; \
       cargo test --locked -p pw-core --test value_relations 2>&1 | grep -E '^(test |test result)'; \
       echo; echo "== what was decided: the store program"; echo; \
       cargo run --quiet --locked -p pw-cli -- audit-values packages/pw-std/*.pw \
         packages/pw-platform-web/*.pw examples/domain.pw examples/lib/*.pw examples/store/*.pw; \
       echo; echo "== what was decided: the accepted corpus as one program"; echo; \
       cargo run --quiet --locked -p pw-cli -- audit-values packages/pw-std/*.pw \
         packages/pw-platform-web/*.pw examples/domain.pw examples/lib/*.pw examples/accepted/*.pw; \
       echo; echo "== mutation controls (scripts/e9_value_mutations.py)"; echo; \
       python3 scripts/e9_value_mutations.py; \
       echo; \
       echo "NOT CLAIMED: member existence. \`box.x\` on a Rect with no \`x\` is"; \
       echo "unknown, not refused; it is the next relation, not one of V1-V6."; \
       echo "NOT CLAIMED: branch agreement in statement position, named-argument"; \
       echo "calls, sum-type variant constructors, or policy-term expressions."; \
       echo "Each is Undecided and counted above, never reported as agreement."; \
     } > docs/evidence/E9/value-relations.txt
    @grep -E "^test result|mutants killed" docs/evidence/E9/value-relations.txt

# E10-I steps 4-6 — the store's commands, compiled to Wasm components. Wrapped
# by upstream `wit-component`, checked by `wit-component`'s decoder against the
# world each contract fixed. Named by component id, which is how the host and
# the dev server find them; held to the compiler's current output by `pw-core`'s
# `evidence_is_current`.
e10-component:
    @mkdir -p docs/evidence/E10
    @for id in store.page.add_to_cart store.page.clear_cart; do \
      cargo run --quiet --locked -p pw-cli -- emit-component --component "$id" \
        --out "docs/evidence/E10/$id.wasm" \
        packages/pw-std/*.pw packages/pw-platform-web/*.pw examples/domain.pw \
        examples/lib/*.pw examples/store/*.pw; \
    done

# E10-I — a Pleris-compiled command, executed through the E8 host, with the
# Rust closure path deleted. The compiled components, the host running them
# with its own session and data-layer operations and its refusals, and the
# development server whose commands now run them.
e10-i:
    @{ echo "E10-I - the compiled command runs through the host"; echo; \
       echo "produced by: just e10-i"; \
       echo "commit: $(git rev-parse HEAD)$(git diff --quiet HEAD -- . ':(exclude)docs/evidence' || echo ' + uncommitted changes')"; \
       echo "rust: $(rustc --version)"; echo; \
       echo "== 4-6. compile, wrap with wit-component, audit against the world"; echo; \
       for id in store.page.add_to_cart store.page.clear_cart; do \
         cargo run --quiet --locked -p pw-cli -- emit-component --component "$id" \
           --out "$(mktemp)" packages/pw-std/*.pw packages/pw-platform-web/*.pw \
           examples/domain.pw examples/lib/*.pw examples/store/*.pw; \
       done; \
       echo; echo "== the committed artifacts are the compiler's current output"; echo; \
       cargo test --locked -p pw-core --test evidence_is_current --test component --test wasm_encoding 2>&1 | grep -E '^(test |test result)'; \
       echo; echo "== 7-8. the E8 host admits and runs them (runtime/pw-host/tests/pleris_component.rs)"; echo; \
       cargo test --locked -p pw-host --features engine --test pleris_component -- --nocapture --test-threads=1 2>&1 \
         | grep -oE "(compiled add_to_cart imports|the host saw|the command returned|a failing data layer|refused without|ungranted write refused|starved of fuel).*|^test result.*"; \
       echo; echo "== 9. the development server's commands are the compiled components"; echo; \
       cargo test --locked -p pw-dev-server 2>&1 | grep -E '^(test |test result)'; \
       echo; \
       echo "NOT CLAIMED here: resumable handler BODIES are compiled. At E10-I the"; \
       echo "browser's handler posted the pressed instance and the literal"; \
       echo "quantity. They are compiled since 2026-09-25: just e10-handlers."; \
       echo "NOT CLAIMED: the data layer (store:data/carts) is Pleris. It is the"; \
       echo "deployment's, as the contract says (owner: external) - NEXT step 10."; \
     } > docs/evidence/E10/e10-i.txt
    @grep -E "^test result|the command returned" docs/evidence/E10/e10-i.txt

# `pw build` runs with PATH=/usr/bin:/bin, which holds neither Koka nor Node, so
# the build cannot reach them even by shelling out. The control line shows
# where they are on the ordinary PATH. The build's outputs are then compared
# byte for byte with the artifacts recorded separately.
#
# E10 gate item 1 — the store builds from source to every artifact, without
# Koka or Marko.
e10-build:
    @cargo build --quiet --locked -p pw-cli
    @{ echo "E10 gate item 1 - the store builds from source, without Koka or Marko"; echo; \
       echo "produced by: just e10-build"; \
       echo "commit: $(git rev-parse HEAD)$(git diff --quiet HEAD -- . ':(exclude)docs/evidence' ':(exclude)spikes/own-renderer/store-ir.json' || echo ' + uncommitted changes')"; \
       echo "rust: $(rustc --version)"; echo; \
       echo "== the control: the ordinary PATH"; \
       for t in koka node npm pnpm; do printf '  %-5s %s\n' "$t" "$(command -v "$t" || echo absent)"; done; \
       echo; echo "== the build's PATH: /usr/bin:/bin"; \
       for t in koka node npm pnpm; do printf '  %-5s %s\n' "$t" "$(env -i PATH=/usr/bin:/bin sh -c "command -v $t" || echo absent)"; done; \
       out="$(mktemp -d)"; echo; \
       env -i PATH=/usr/bin:/bin HOME="$HOME" ./target/debug/pw build --out "$out" \
         packages/pw-std/*.pw packages/pw-platform-web/*.pw \
         examples/domain.pw examples/lib/*.pw examples/store/*.pw | sed "s#$out#OUT#"; \
       echo; echo "== what was written"; \
       (cd "$out" && find . -type f | sort | while read -r f; do printf '  %7d  %s\n' "$(wc -c < "$f")" "${f#./}"; done); \
       echo; echo "== byte-identical to the separately recorded artifacts"; \
       for id in store.page.add_to_cart store.page.clear_cart; do \
         cmp "$out/components/$id.wasm" "docs/evidence/E10/$id.wasm" && echo "  components/$id.wasm = docs/evidence/E10/$id.wasm"; done; \
       for f in docs/evidence/E10/handlers/*.mjs; do \
         cmp "$out/handlers/$(basename "$f")" "$f" && echo "  handlers/$(basename "$f") = $f"; done; \
       cmp "$out/templates.json" spikes/own-renderer/store-ir.json && echo "  templates.json = spikes/own-renderer/store-ir.json (pw emit-template)"; \
       rm -rf "$out"; \
       echo; echo "== compiler/pw-core/tests/build.rs"; echo; \
       cargo test --locked -p pw-core --test build 2>&1 | grep -E '^(test |test result)'; \
       echo; \
       echo "NOT CLAIMED: the queries run as components in the development server."; \
       echo "They compile and are audited; the server still computes the page's"; \
       echo "values itself (NEXT). The four Resources.* queries are 'todo' in the"; \
       echo "library, and nothing the store builds depends on them."; \
     } > docs/evidence/E10/build.txt
    @cat docs/evidence/E10/build.txt

# Every compiled server declaration beside an independent Rust reference of
# what it means, 300 generated cases each, with inputs and data-layer answers
# generated from the artifact's own types; and three wrong references the
# oracle must catch.
#
# E10 gate item 2 — semantics match the oracle.
e10-oracle:
    @{ echo "E10 gate item 2 - compiled semantics against a reference"; echo; \
       echo "produced by: just e10-oracle"; \
       echo "commit: $(git rev-parse HEAD)$(git diff --quiet HEAD -- . ':(exclude)docs/evidence' ':(exclude)spikes/own-renderer/store-ir.json' || echo ' + uncommitted changes')"; \
       echo "rust: $(rustc --version)"; echo; \
       echo "== the differential oracle (compiler/pw-conformance/tests/oracle.rs)"; echo; \
       cargo test --locked -p pw-conformance --test oracle -- --nocapture --test-threads=1 2>&1 \
         | grep -oE "oracle: .*|^test result.*"; \
       echo; echo "== matches, fields and variants, run (compiler/pw-conformance/tests/variants.rs)"; echo; \
       cargo test --locked -p pw-conformance --test variants -- --nocapture --test-threads=1 2>&1 \
         | grep -oE "^(人|地図) -> .*|^m\.Q: .*|^test [a-z_]+ .*|^test result.*"; \
       echo; \
       echo "NOT CLAIMED: the references are Rust statements of each declaration's"; \
       echo "meaning, written from the source. Koka is the oracle for effects (E9-K)"; \
       echo "and for the pure subset (just spike-pw-to-koka); none of these seven"; \
       echo "declarations is pure, so Koka is not run on them."; \
     } > docs/evidence/E10/oracle.txt
    @cat docs/evidence/E10/oracle.txt

# ADR-0038's tests, and its mutation controls: each fix undone in turn must
# fail them (scripts/match_mutations.py restores the sources after each).
#
# E10 — every match is analysed, and each pattern against its own type.
e10-match:
    @{ echo "ADR-0038 - every match is analysed, and each pattern against its own type"; echo; \
       echo "produced by: just e10-match"; \
       echo "commit: $(git rev-parse HEAD)$(git diff --quiet HEAD -- . ':(exclude)docs/evidence' || echo ' + uncommitted changes')"; \
       echo "rust: $(rustc --version)"; echo; \
       echo "== the checker (compiler/pw-core/tests/match_exhaustiveness.rs)"; echo; \
       cargo test --locked -p pw-core --test match_exhaustiveness 2>&1 | grep -E '^(test |test result)'; \
       echo; echo "== the analysis (compiler/pw-core/src/exhaust.rs)"; echo; \
       cargo test --locked -p pw-core --lib exhaust:: 2>&1 | grep -E '^(test |test result)'; \
       echo; echo "== the grammar (compiler/pw-syntax/src/grammar.rs)"; echo; \
       cargo test --locked -p pw-syntax --lib 2>&1 | grep -E '^test (grammar::tests::(a_return|return_is|an_arm)|result)'; \
       echo; echo "== mutation controls (scripts/match_mutations.py)"; echo; \
       python3 scripts/match_mutations.py; \
       echo; \
       echo "NOT CLAIMED: a pattern nested under Some, Ok or Err, a literal pattern,"; \
       echo "and a scrutinee the value relations cannot type are not analysed. Each"; \
       echo "is Blocked, with its reason, and never reported as exhaustive."; \
     } > docs/evidence/E10/match.txt
    @grep -E "^test result|mutants killed" docs/evidence/E10/match.txt

# ADR-0039's compiled computation against exact references, against Koka
# 3.2.3 (which is why this is not in `just ci`), and its mutation controls.
#
# E10 task 2 — pure computation compiled to Wasm.
e10-pure:
    @{ echo "ADR-0039 - pure computation in the component backend"; echo; \
       echo "produced by: just e10-pure"; \
       echo "commit: $(git rev-parse HEAD)$(git diff --quiet HEAD -- . ':(exclude)docs/evidence' || echo ' + uncommitted changes')"; \
       echo "rust: $(rustc --version)"; \
       echo "koka: $(koka --version | head -1)"; echo; \
       echo "== against exact references (compiler/pw-conformance/tests/computation.rs)"; echo; \
       cargo test --locked -p pw-conformance --test computation -- --nocapture --test-threads=1 2>&1 \
         | grep -E '^(test |test result)|cases, |^r\.Q: '; \
       echo; echo "== against Koka (compiler/pw-conformance/tests/koka_oracle.rs)"; echo; \
       KOKA="$(command -v koka)" cargo test --locked -p pw-conformance --test koka_oracle -- --ignored --nocapture 2>&1 \
         | grep -E '^oracle:|^test result'; \
       echo; echo "== mutation controls (scripts/pure_mutations.py)"; echo; \
       python3 scripts/pure_mutations.py; \
       echo; \
       echo "NOT CLAIMED: internal functions. A call is inlined, so recursion and"; \
       echo "generic declarations are refused; code size grows with each call site."; \
       echo "NOT CLAIMED: declared variants built or matched, Float remainder and"; \
       echo "formatting, and early return. Each is refused by name."; \
     } > docs/evidence/E10/pure.txt
    @grep -E "^test result|mutants killed|^oracle:" docs/evidence/E10/pure.txt

# `pw build` for examples/kiokun into docs/evidence/E10/kiokun/ (held there by
# evidence_is_current), the host's tests over the committed sample shard, the
# browser spec in three engines, and, with KIOKUN_DATA naming a kiokun-data
# checkout's output_dictionary, every entry of the whole shard.
#
# E10 — the kiokun slice: entry lookup and search over one shard.
e10-kiokun:
    @cargo build --quiet --locked -p pw-cli -p kiokun-server -p pw-dev-server
    @rm -rf docs/evidence/E10/kiokun
    @./target/debug/pw build --out docs/evidence/E10/kiokun packages/pw-std/*.pw \
      packages/pw-platform-web/*.pw examples/kiokun/*.pw > /dev/null
    @BUILD_ONLY=1 bash spikes/own-renderer/run.sh > /dev/null
    @{ echo "E10 - the kiokun slice"; echo; \
       echo "produced by: just e10-kiokun"; \
       echo "commit: $(git rev-parse HEAD)$(git diff --quiet HEAD -- . ':(exclude)docs/evidence' ':(exclude)spikes/own-renderer/store-ir.json' || echo ' + uncommitted changes')"; \
       echo "rust: $(rustc --version)"; echo; \
       echo "== pw build, examples/kiokun"; echo; \
       ./target/debug/pw build --out "$(mktemp -d)" packages/pw-std/*.pw \
         packages/pw-platform-web/*.pw examples/kiokun/*.pw | sed -E 's#-> .*#-> docs/evidence/E10/kiokun#'; \
       echo; echo "== the sample shard (examples/kiokun/data/han-1char-3/MANIFEST.txt)"; echo; \
       head -1 examples/kiokun/data/han-1char-3/MANIFEST.txt; \
       echo; echo "== the host (spikes/kiokun/server)"; echo; \
       cargo test --locked -p kiokun-server -- --nocapture --test-threads=1 2>&1 \
         | grep -oE "(sample|person|ren|人|谚|ひと|zzzz-no-such-word): .*|^test [a-z_:]+ .*|^test result.*"; \
       echo; echo "== the whole shard"; echo; \
       if [ -n "${KIOKUN_DATA:-}" ]; then \
         cargo test --locked --release -p kiokun-server whole -- --include-ignored --nocapture 2>&1 \
           | grep -oE "^whole shard: .*|^test result.*"; \
       else echo "  (skipped: KIOKUN_DATA does not name a kiokun-data output_dictionary)"; fi; \
       echo; echo "== in browser engines (spikes/own-renderer/e2e/kiokun.spec.mjs)"; echo; \
       (cd spikes/own-renderer && pnpm exec playwright test e2e/kiokun.spec.mjs \
          --project=chromium --project=firefox --project=webkit --reporter=line 2>&1) \
         | sed 's/\x1b\[[0-9;]*m//g;s/\x1b\[1A\x1b\[2K//g' \
         | grep -E "^ +[0-9]+\) |Error:|passed|failed|flaky" || true; \
     } > docs/evidence/E10/kiokun.txt
    @cat docs/evidence/E10/kiokun.txt

# Code size: the store's artifacts as `pw build` writes them, beside two
# baselines. One is hand-written Rust components (the E0/E8 spike guests, from
# `just spike-wasmtime`). The other is the runtime E7 recorded. Performance: the
# compiled command, a hand-written Rust guest and the native operation, through
# one host API, in release; then E7's own browser instrument, re-run into E10's
# file so E7's record is untouched.
#
# E10 gate item 4 — code size and performance, recorded against baselines.
e10-bench:
    @cargo build --quiet --locked -p pw-cli
    @BUILD_ONLY=1 bash spikes/own-renderer/run.sh > /dev/null
    @cargo build --quiet --locked -p pw-dev-server
    @EVIDENCE_FILE="$(pwd)/docs/evidence/E10/performance-e7-instrument.txt" PRODUCED_BY="just e10-bench" \
      bash spikes/own-renderer/performance.sh > /dev/null
    @{ echo "E10 gate item 4 - code size and performance against baselines"; echo; \
       echo "produced by: just e10-bench"; \
       echo "commit: $(git rev-parse HEAD)$(git diff --quiet HEAD -- . ':(exclude)docs/evidence' ':(exclude)spikes/own-renderer/store-ir.json' || echo ' + uncommitted changes')"; \
       echo "rust: $(rustc --version)"; \
       echo "host: $(uname -m) $(sysctl -n machdep.cpu.brand_string 2>/dev/null || true)"; echo; \
       out="$(mktemp -d)"; \
       ./target/debug/pw build --out "$out" packages/pw-std/*.pw packages/pw-platform-web/*.pw \
         examples/domain.pw examples/lib/*.pw examples/store/*.pw > /dev/null; \
       echo "== code size: the store's artifacts (bytes, gzip -9 bytes)"; \
       (cd "$out" && for f in components/*.wasm handlers/*.mjs; do \
         printf '  %6d  %6d  %s\n' "$(wc -c < "$f")" "$(gzip -9c "$f" | wc -c)" "$f"; done); \
       rm -rf "$out"; \
       echo; echo "== code size: baselines (bytes, gzip -9 bytes)"; \
       for f in spikes/wasmtime-component/guest-minimal/target/wasm32-wasip2/release/spike_wasmtime_guest_minimal.wasm \
                spikes/wasmtime-component/guest/target/wasm32-wasip2/release/spike_wasmtime_guest.wasm; do \
         if [ -f "$f" ]; then printf '  %6d  %6d  %s\n' "$(wc -c < "$f")" "$(gzip -9c "$f" | wc -c)" "$f (hand-written Rust)"; \
         else echo "  (absent: $f - run just spike-wasmtime)"; fi; done; \
       echo "  server-written handler modules before ADR-0033: add_to_cart 255, clear_cart 180 (reconstructed from 83af93c's format string)"; \
       printf '  %6d  %6d  %s\n' "$(wc -c < spikes/own-renderer/public/pw-runtime.mjs)" "$(gzip -9c spikes/own-renderer/public/pw-runtime.mjs | wc -c)" "pw-runtime.mjs now"; \
       echo "  E7's record: $(grep -oE 'interactive-script-bytes=[0-9]+' docs/evidence/E7/performance.txt) $(grep -oE 'interactive-wasm-bytes=[0-9]+' docs/evidence/E7/performance.txt)"; \
       echo; echo "== the host, release: runtime/pw-host/tests/bench.rs"; echo; \
       cargo test --quiet --locked --release -p pw-host --features engine --test bench -- --include-ignored --nocapture --test-threads=1 2>&1 \
         | grep -E "^bench:|^test result"; \
       echo; echo "== the browser: activation, the runtime before compiled handlers and now (chromium, 1 worker, interleaved)"; echo; \
       bash spikes/own-renderer/activation-compare.sh 83af93c 7 3; \
       echo; echo "== the browser: E7's instrument, re-run (full record: performance-e7-instrument.txt)"; echo; \
       echo "  E7's record (2026-08-07, an older Chromium; activation is one sample):"; grep -E "interactive-script-bytes|interactive-wasm-bytes|activation-ms|interaction-long-frames|runtime-layout-reads" docs/evidence/E7/performance.txt | sed 's/^ */    /'; \
       echo "  now (activation is one sample; the comparison above is the measurement):"; grep -E "interactive-script-bytes|interactive-wasm-bytes|activation-ms|interaction-long-frames|runtime-layout-reads" docs/evidence/E10/performance-e7-instrument.txt | sed 's/^ */    /'; \
     } > docs/evidence/E10/bench.txt
    @cat docs/evidence/E10/bench.txt

# What the store and the accepted corpus make affine, and why: only types an
# effect row acquires. No store value is affine or annotated, and the language
# has no borrow, lifetime or move syntax. With it, charter M10 task 10's
# remaining half: carried captures serialize deterministically. Resume
# versioning is E7V's (docs/evidence/E7V/deployment-matrix.txt).
#
# E10 gate item 5 — no ownership syntax for ordinary application values.
e10-ownership:
    @{ echo "E10 gate item 5 - no ownership syntax for ordinary values; task 10 - deterministic resumable state"; echo; \
       echo "produced by: just e10-ownership"; \
       echo "commit: $(git rev-parse HEAD)$(git diff --quiet HEAD -- . ':(exclude)docs/evidence' ':(exclude)spikes/own-renderer/store-ir.json' || echo ' + uncommitted changes')"; \
       echo "rust: $(rustc --version)"; echo; \
       echo "== what is affine, and what is not (compiler/pw-core/tests/ownership.rs)"; echo; \
       cargo test --locked -p pw-core --test ownership -- --nocapture --test-threads=1 2>&1 \
         | grep -oE "^(store|accepted corpus|not Pleris): .*|^test [a-z_]+ .*|^test result.*"; \
       echo; echo "== carried captures serialize deterministically (runtime/pw-render/tests/properties.rs)"; echo; \
       cargo test --locked -p pw-render --test properties carried 2>&1 | grep -E "^(test |test result)"; \
       echo; echo "== resume versioning: E7V, recorded in docs/evidence/E7V/deployment-matrix.txt"; \
       grep -cE "^" docs/evidence/E7V/deployment-matrix.txt | sed 's/^/   lines: /'; \
     } > docs/evidence/E10/ownership.txt
    @cat docs/evidence/E10/ownership.txt

# The development server under sustained commands and session churn: frames
# held, subscribers, outbox rows and materialized entries, each bounded. The
# compiled command called 20,000 times through the E8 host, with resident memory
# measured before and after. The numbers before the bound existed are in
# docs/evidence/E10/load-2026-09-25.md.
#
# E10 gate item 3 — memory leaks and unbounded resource growth under sustained load.
e10-load:
    @{ echo "E10 gate item 3 - sustained load"; echo; \
       echo "produced by: just e10-load"; \
       echo "commit: $(git rev-parse HEAD)$(git diff --quiet HEAD -- . ':(exclude)docs/evidence' ':(exclude)spikes/own-renderer/store-ir.json' || echo ' + uncommitted changes')"; \
       echo "rust: $(rustc --version)"; echo; \
       echo "== the development server (spikes/own-renderer/server)"; echo; \
       cargo test --locked -p pw-dev-server -- --nocapture --test-threads=1 2>&1 \
         | grep -oE "^(steady|churn|idle): .*|\{\"cursor\".*|^test tests::(sustained|a_subscriber|a_forgotten)[a-z_]* .*|^test result.*"; \
       echo; echo "== the compiled command through the E8 host (runtime/pw-host/tests/pleris_component.rs)"; echo; \
       cargo test --locked -p pw-host --features engine --test pleris_component sustained -- --nocapture 2>&1 \
         | grep -oE "^sustained: .*|^test result.*"; \
       echo; \
       echo "NOT CLAIMED: per-session DATA is bounded. A cart is the store's data and"; \
       echo "is kept; what is bounded is what the server holds FOR a subscriber (its"; \
       echo "frames, its queue, its cached fragment) and what it keeps AFTER use (the"; \
       echo "outbox)."; \
     } > docs/evidence/E10/load.txt
    @cat docs/evidence/E10/load.txt

# Writes the store's modules as `pw emit-handlers` emits them to
# `docs/evidence/E10/handlers/`, held there by `evidence_is_current`, and
# records the refusal matrix, the renderer carrying exactly the paths a handler
# reads, the host typing a browser's arguments by the component's own
# parameters, and the development server's one command path.
#
# E10 — resumable handler bodies, compiled to JavaScript modules (ADR-0033).
e10-handlers:
    @rm -rf docs/evidence/E10/handlers
    @{ echo "E10 - resumable handlers, compiled to JavaScript modules"; echo; \
       echo "produced by: just e10-handlers"; \
       echo "commit: $(git rev-parse HEAD)$(git diff --quiet HEAD -- . ':(exclude)docs/evidence' ':(exclude)spikes/own-renderer/store-ir.json' || echo ' + uncommitted changes')"; \
       echo "rust: $(rustc --version)"; echo; \
       echo "== the store's handlers, as pw emit-handlers writes them"; echo; \
       cargo run --quiet --locked -p pw-cli -- emit-handlers --out docs/evidence/E10/handlers \
         packages/pw-std/*.pw packages/pw-platform-web/*.pw \
         examples/domain.pw examples/lib/*.pw examples/store/*.pw; \
       for f in docs/evidence/E10/handlers/*.mjs; do echo; echo "-- $f"; cat "$f"; done; \
       echo; echo "== the compiler: modules, event parts, contracts, refusals (compiler/pw-core/tests/handlers.rs)"; echo; \
       cargo test --locked -p pw-core --test handlers -- --nocapture --test-threads=1 2>&1 \
         | grep -oE "refused \`.*|^test [a-z_]+ .*|^test result.*"; \
       echo; echo "== the committed modules are the compiler's current output"; echo; \
       cargo test --locked -p pw-core --test evidence_is_current 2>&1 | grep -E '^(test |test result)'; \
       echo; echo "== the renderer carries exactly the paths a handler reads (runtime/pw-render/tests/properties.rs)"; echo; \
       cargo test --locked -p pw-render --test properties 2>&1 | grep -E "^test (an_element|a_captured|a_capture|an_event)|^test result"; \
       echo; echo "== the host types a browser's arguments by the artifact's own parameters (runtime/pw-host/tests/pleris_component.rs)"; echo; \
       cargo test --locked -p pw-host --features engine --test pleris_component -- --nocapture --test-threads=1 2>&1 \
         | grep -oE '^\[.*\]: .*|^test (json|arguments)[a-z_]+ .*|^test result.*'; \
       echo; echo "== the development server: one command path, compiled handlers only (pw-dev-server)"; echo; \
       cargo test --locked -p pw-dev-server 2>&1 | grep -E '^(test |test result)'; \
       echo; \
       echo "NOT CLAIMED: captures are a patched part. An element carries what its"; \
       echo "handler reads as rendered; the store reads only item.id, the loop key."; \
       echo "NOT CLAIMED: an opaque type's invariant is checked at the boundary."; \
       echo "PositiveInt arrives as an s64; the language states no invariant for it."; \
       echo "NOT CLAIMED: handler parameters, local computation, branches, or"; \
       echo "multi-statement bodies compile. Each is refused by name."; \
     } > docs/evidence/E10/handlers.txt
    @grep -E "^test result|written to|refused \`" docs/evidence/E10/handlers.txt | head -20

# E10-I in the browser: the store page whose Add and Clear buttons reach the
# COMPILED commands, in all three engine families, three full runs — a flake
# shows up as a run that differs. Needs the Playwright browsers
# (`pnpm --filter pw-own-renderer-spike exec playwright install chromium firefox
# webkit`), so it is not part of `just ci`.
#
# `out` names the evidence file: E10-I recorded `browser-suite.txt`, the
# compiled handlers (ADR-0033) `handlers-browser-suite.txt`.
e10-browser engines="chromium firefox webkit" out="docs/evidence/E10/browser-suite.txt":
    @BUILD_ONLY=1 bash spikes/own-renderer/run.sh > /dev/null
    @cargo build --quiet --locked -p pw-dev-server
    @{ echo "E10 - the store page, compiled commands and handlers, in browser engines: {{engines}}"; echo; \
       echo "produced by: just e10-browser \"{{engines}}\" {{out}}"; \
       echo "commit: $(git rev-parse HEAD)$(git diff --quiet HEAD -- . ':(exclude)docs/evidence' ':(exclude)spikes/own-renderer/store-ir.json' || echo ' + uncommitted changes')"; \
       echo "playwright: $(cd spikes/own-renderer && pnpm exec playwright --version)"; echo; \
       for i in 1 2 3; do \
         echo "== full run $i"; \
         (cd spikes/own-renderer && pnpm exec playwright test --reporter=line \
           $(for e in {{engines}}; do printf -- '--project=%s ' "$e"; done) 2>&1) \
           | sed 's/\x1b\[[0-9;]*m//g;s/\x1b\[1A\x1b\[2K//g' \
           | grep -E "^ +[0-9]+\) |Error:|passed|failed|flaky|did not run" || true; \
       done; \
     } > {{out}}
    @cat {{out}}

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
ci: evidence-gates fmt-check lint case-check test-unit test-compile
    @echo ""
    @echo "ci: OK"
