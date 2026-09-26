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

# Every mutation control's anchor matches its file exactly once. Reads the
# anchors and builds nothing, so a refactor that moves an anchored line fails
# here rather than when the control is next run (scripts/mutation_anchors.py).
mutation-anchors:
    @python3 scripts/mutation_anchors.py

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
       echo "NOT CLAIMED here: member existence, which is not one of V1-V6. It is a"; \
       echo "relation since 2026-09-25 (PW0610, ADR-0048): just e10-members."; \
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
       echo "NOT CLAIMED here: recursion and generic callees, compiled beside the"; \
       echo "export since 2026-09-25 (ADR-0050): just e10-recursion. A call that"; \
       echo "does not recurse is still inlined, so code size grows with each call site."; \
       echo "NOT CLAIMED: declared variants built or matched, Float remainder and"; \
       echo "formatting. Each is refused by name. An early return compiles since"; \
       echo "2026-09-25 (ADR-0051): just e10-control-flow."; \
     } > docs/evidence/E10/pure.txt
    @grep -E "^test result|mutants killed|^oracle:" docs/evidence/E10/pure.txt

# ADR-0040's standard library, compiled: each list and string operation
# against Rust's own over generated inputs, and the mutation controls.
#
# E10 task 2 — the standard library the kiokun slice's logic needs.
e10-stdlib:
    @{ echo "ADR-0040 - the standard library's lists and strings, compiled"; echo; \
       echo "produced by: just e10-stdlib"; \
       echo "commit: $(git rev-parse HEAD)$(git diff --quiet HEAD -- . ':(exclude)docs/evidence' || echo ' + uncommitted changes')"; \
       echo "rust: $(rustc --version)"; echo; \
       echo "== against Vec and str (compiler/pw-conformance/tests/stdlib.rs)"; echo; \
       cargo test --locked -p pw-conformance --test stdlib -- --nocapture --test-threads=1 2>&1 \
         | grep -E '^(test |test result)|^r\.Q: '; \
       echo; echo "== mutation controls (scripts/stdlib_mutations.py)"; echo; \
       python3 scripts/stdlib_mutations.py; \
       echo; \
       echo "NOT CLAIMED here: Unicode case mapping, which is ADR-0056's"; \
       echo "(just e10-case). to_lower_ascii maps A-Z only."; \
       echo "NOT CLAIMED here: a function stored, returned or passed to a program's"; \
       echo "own declaration. It is a value since 2026-09-25 (ADR-0052):"; \
       echo "just e10-function-values."; \
     } > docs/evidence/E10/stdlib.txt
    @grep -E "^test result|mutants killed" docs/evidence/E10/stdlib.txt

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
         | grep -oE "(sample|person|ren|人|谚|ひと|zzzz-no-such-word|place|search): .*|^test [a-z_:]+ .*|^test result.*"; \
       echo; echo "== the whole shard, and every word kiokun has"; echo; \
       if [ -n "${KIOKUN_DATA:-}" ]; then \
         cargo test --locked --release -p kiokun-server -- --ignored --nocapture --test-threads=1 2>&1 \
           | grep -oE "(whole shard|every word): .*|^test result.*"; \
       else echo "  (skipped: KIOKUN_DATA does not name a kiokun-data output_dictionary)"; fi; \
       echo; echo "== in browser engines (spikes/own-renderer/e2e/kiokun.spec.mjs)"; echo; \
       (cd spikes/own-renderer && pnpm exec playwright test e2e/kiokun.spec.mjs \
          --project=chromium --project=firefox --project=webkit --reporter=line 2>&1) \
         | sed 's/\x1b\[[0-9;]*m//g;s/\x1b\[1A\x1b\[2K//g' \
         | grep -E "^ +[0-9]+\) |Error:|passed|failed|flaky" || true; \
     } > docs/evidence/E10/kiokun.txt
    @cat docs/evidence/E10/kiokun.txt

# ADR-0046: E10 task 4 — what each compiled kiokun query costs per call, in
# release: instructions, peak linear memory, and instantiation beside the call.
# With KIOKUN_DATA naming a kiokun-data checkout, over the whole shard.
e10-memory:
    @{ echo "ADR-0046 - memory strategies: what each call costs"; echo; \
       echo "produced by: just e10-memory"; \
       echo "commit: $(git rev-parse HEAD)$(git diff --quiet HEAD -- . ':(exclude)docs/evidence' || echo ' + uncommitted changes')"; \
       echo "rust: $(rustc --version)"; \
       echo "machine: $(uname -m), $(sysctl -n machdep.cpu.brand_string 2>/dev/null || uname -s)"; echo; \
       cargo test --locked --release -p kiokun-server memory_and_work -- --ignored --nocapture 2>&1 \
         | grep -oE "memory: .*|^test result.*"; \
       echo; \
       echo "NOT CLAIMED: reference counting, Wasm GC and reuse analysis were not built and"; \
       echo "measured against the region. ADR-0046 argues from these numbers and the"; \
       echo "invocation model."; \
     } > docs/evidence/E10/memory.txt
    @cat docs/evidence/E10/memory.txt

# ADR-0045: an affine value consumed exactly once on every path; the witnesses
# and the mutation controls.
#
# E10 task 7 — affine annotations for scarce resources, expressing a real
# invariant.
e10-affine:
    @{ echo "ADR-0045 - affine values consumed exactly once, on every path"; echo; \
       echo "produced by: just e10-affine"; \
       echo "commit: $(git rev-parse HEAD)$(git diff --quiet HEAD -- . ':(exclude)docs/evidence' || echo ' + uncommitted changes')"; \
       echo "rust: $(rustc --version)"; echo; \
       echo "== each witness (examples/generality/affine_not_consumed_once)"; echo; \
       for f in examples/generality/affine_not_consumed_once/*.pw; do \
         printf '%-40s %-10s %s\n' "$(basename $f)" "$(grep -m1 '@status:' $f | sed 's#// @status: ##')" \
           "$(./target/debug/pw check packages/pw-std/*.pw packages/pw-platform-web/*.pw examples/domain.pw examples/lib/*.pw $f 2>&1 \
              | sed 's/\x1b\[[0-9;]*m//g' | grep -oE '\[PW2005\][^[]*' | head -1)"; \
       done; \
       echo; echo "== the generality suite"; echo; \
       cargo test --locked -p pw-core --test generality 2>&1 | grep -E '^test result'; \
       echo; echo "== mutation controls (scripts/affine_mutations.py)"; echo; \
       python3 scripts/affine_mutations.py; \
       echo; \
       echo "NOT CLAIMED: a borrow. Passing a value to a function whose row does not"; \
       echo "release it is a use; Pleris has no borrow syntax (gate item 5)."; \
     } > docs/evidence/E10/affine.txt
    @grep -E "^test result|mutants killed" docs/evidence/E10/affine.txt

# ADR-0052: a function is a value. The closures through the host, the
# component against the JavaScript module, and the mutation controls.
e10-function-values:
    @{ echo "ADR-0052 - a function is a value"; echo; \
       echo "produced by: just e10-function-values"; \
       echo "commit: $(git rev-parse HEAD)$(git diff --quiet HEAD -- . ':(exclude)docs/evidence' || echo ' + uncommitted changes')"; \
       echo "rust: $(rustc --version)"; echo "node: $(node --version)"; echo; \
       echo "== closures through the host (pw-conformance/tests/function_values.rs)"; echo; \
       cargo test --locked -p pw-conformance --test function_values 2>&1 | grep -E '^(test |test result)'; \
       echo; echo "== the component and the JavaScript module, on the same arguments"; echo; \
       cargo test --locked -p pw-conformance --test javascript -- --nocapture --test-threads=1 2>&1 \
         | grep -oE "javascript: [0-9]+ queries.*"; \
       echo; echo "== mutation controls (scripts/function_value_mutations.py)"; echo; \
       python3 scripts/function_value_mutations.py; \
       echo; \
       echo "NOT CLAIMED: a function read from a record field and called, or a"; \
       echo "function crossing the component boundary."; \
     } > docs/evidence/E10/function-values.txt
    @grep -E "^javascript:|mutants killed" docs/evidence/E10/function-values.txt

# ADR-0058: handlers that compute. Each handler's module run under Node against
# a context that records what it sends, and the mutation controls. Needs
# `node`.
e10-handlers-compute:
    @{ echo "ADR-0058 - handlers that compute"; echo; \
       echo "produced by: just e10-handlers-compute"; \
       echo "commit: $(git rev-parse HEAD)$(git diff --quiet HEAD -- . ':(exclude)docs/evidence' || echo ' + uncommitted changes')"; \
       echo "rust: $(rustc --version)"; echo "node: $(node --version)"; echo; \
       echo "== what each handler sends, run under Node (compiler/pw-core/tests/handlers.rs)"; echo; \
       cargo test --locked -p pw-core --test handlers 2>&1 | grep -E '^(test |test result)'; \
       echo; echo "== mutation controls (scripts/handler_mutations.py)"; echo; \
       python3 scripts/handler_mutations.py; \
       echo; \
       echo "NOT CLAIMED: the event as a parameter, which no syntax binds. NOT"; \
       echo "CLAIMED: a command's answer read by the handler; its value is the unit"; \
       echo "value. NOT CLAIMED: a command in a function value, or a function value"; \
       echo "that reads what the handler captured; each is refused by name."; \
     } > docs/evidence/E10/handlers-compute.txt
    @grep -E "^test result|mutants killed" docs/evidence/E10/handlers-compute.txt

# ADR-0062: generic records, sum types and opaque types in the backend, and
# a type beside a query of its name in the world. Each run through the E8
# host, the component against the JavaScript module, and the mutation
# controls. Needs `node`.
e10-generics:
    @{ echo "ADR-0062 - generic types in the backend"; echo; \
       echo "produced by: just e10-generics"; \
       echo "commit: $(git rev-parse HEAD)$(git diff --quiet HEAD -- . ':(exclude)docs/evidence' || echo ' + uncommitted changes')"; \
       echo "rust: $(rustc --version)"; echo "node: $(node --version)"; echo; \
       echo "== through the E8 host (compiler/pw-conformance/tests/generics.rs)"; echo; \
       cargo test --locked -p pw-conformance --test generics 2>&1 | grep -E '^(test |test result)'; \
       echo; echo "== a type and a query of one name (compiler/pw-conformance/tests/wit_names.rs)"; echo; \
       cargo test --locked -p pw-conformance --test wit_names 2>&1 | grep -E '^(test |test result)'; \
       echo; echo "== the component and the JavaScript module"; echo; \
       cargo test --locked -p pw-conformance --test javascript -- --nocapture --test-threads=1 2>&1 \
         | grep -oE "javascript: [0-9]+ queries.*|^test result.*"; \
       echo; echo "== mutation controls (scripts/generic_mutations.py)"; echo; \
       python3 scripts/generic_mutations.py; \
       echo; \
       echo "NOT CLAIMED: a generic type at the component boundary, which has no WIT"; \
       echo "form. NOT CLAIMED: an instance only a lambda's branches name."; \
     } > docs/evidence/E10/generics.txt
    @grep -E "^test result|^javascript:|mutants killed" docs/evidence/E10/generics.txt

# ADR-0063: every name means one binding. The resolver's scopes, each read
# by the value relations, the declared-type environment, a handler's capture
# and the privacy labels; what the value relations now decide over the store,
# kiokun and the accepted corpus; and the mutation controls.
e10-lexical:
    @cargo build --quiet --locked -p pw-cli
    @{ echo "ADR-0063 - every name means one binding"; echo; \
       echo "produced by: just e10-lexical"; \
       echo "commit: $(git rev-parse HEAD)$(git diff --quiet HEAD -- . ':(exclude)docs/evidence' || echo ' + uncommitted changes')"; \
       echo "rust: $(rustc --version)"; echo; \
       echo "== the scopes, and what each reader makes of them (compiler/pw-core/tests/lexical_scope.rs)"; echo; \
       cargo test --locked -p pw-core --test lexical_scope 2>&1 | grep -E '^(test |test result)'; \
       echo; echo "== a match over a name an arm binds again (compiler/pw-core/tests/match_exhaustiveness.rs)"; echo; \
       cargo test --locked -p pw-core --test match_exhaustiveness 2>&1 | grep -E '^(test |test result)'; \
       echo; echo "== what the value relations decide: the store program"; echo; \
       ./target/debug/pw audit-values packages/pw-std/*.pw packages/pw-platform-web/*.pw \
         examples/domain.pw examples/lib/*.pw examples/store/*.pw; \
       echo; echo "== what the value relations decide: kiokun"; echo; \
       ./target/debug/pw audit-values packages/pw-std/*.pw packages/pw-platform-web/*.pw examples/kiokun/*.pw; \
       echo; echo "== what the value relations decide: the accepted corpus as one program"; echo; \
       ./target/debug/pw audit-values packages/pw-std/*.pw packages/pw-platform-web/*.pw \
         examples/domain.pw examples/lib/*.pw examples/accepted/*.pw; \
       echo; echo "== mutation controls (scripts/lexical_mutations.py)"; echo; \
       python3 scripts/lexical_mutations.py; \
       echo; \
       echo "NOT CLAIMED: a label carried through a declared function, such as"; \
       echo "List.get over a list of secrets (next: ADR-0064). NOT CLAIMED: a record"; \
       echo "literal whose first field is shorthand, P { x }, which parses as P and a"; \
       echo "block. Each undecided relation above is counted, never agreement."; \
     } > docs/evidence/E10/lexical.txt
    @grep -E "^test result|mutants killed" docs/evidence/E10/lexical.txt

# ADR-0064: a label carried through a call. The privacy tests of a result
# made of its arguments, and the mutation controls.
e10-labels:
    @{ echo "ADR-0064 - a label carried through a call"; echo; \
       echo "produced by: just e10-labels"; \
       echo "commit: $(git rev-parse HEAD)$(git diff --quiet HEAD -- . ':(exclude)docs/evidence' || echo ' + uncommitted changes')"; \
       echo "rust: $(rustc --version)"; echo; \
       echo "== labels through calls (compiler/pw-core/tests/labels_through_calls.rs)"; echo; \
       cargo test --locked -p pw-core --test labels_through_calls 2>&1 | grep -E '^(test |test result)'; \
       echo; echo "== mutation controls (scripts/label_call_mutations.py)"; echo; \
       python3 scripts/label_call_mutations.py; \
       echo; \
       echo "NOT CLAIMED: an implicit flow, such as an element chosen by a secret"; \
       echo "index. NOT CLAIMED: a declaration that states how its result's label is"; \
       echo "made; the rule stands in for one."; \
     } > docs/evidence/E10/labels.txt
    @grep -E "^test result|mutants killed" docs/evidence/E10/labels.txt

# ADR-0065: a value that holds at every type. Its tests, what the value
# relations decide over the store, kiokun and the accepted corpus, and the
# mutation controls.
e10-any:
    @cargo build --quiet --locked -p pw-cli
    @{ echo "ADR-0065 - a value that holds at every type"; echo; \
       echo "produced by: just e10-any"; \
       echo "commit: $(git rev-parse HEAD)$(git diff --quiet HEAD -- . ':(exclude)docs/evidence' || echo ' + uncommitted changes')"; \
       echo "rust: $(rustc --version)"; echo; \
       echo "== None, [], Err, todo, a free parameter, a let mut (compiler/pw-core/tests/any_type.rs)"; echo; \
       cargo test --locked -p pw-core --test any_type 2>&1 | grep -E '^(test |test result)'; \
       echo; echo "== what the value relations decide: the store program"; echo; \
       ./target/debug/pw audit-values packages/pw-std/*.pw packages/pw-platform-web/*.pw \
         examples/domain.pw examples/lib/*.pw examples/store/*.pw; \
       echo; echo "== what the value relations decide: kiokun"; echo; \
       ./target/debug/pw audit-values packages/pw-std/*.pw packages/pw-platform-web/*.pw examples/kiokun/*.pw; \
       echo; echo "== what the value relations decide: the accepted corpus as one program"; echo; \
       ./target/debug/pw audit-values packages/pw-std/*.pw packages/pw-platform-web/*.pw \
         examples/domain.pw examples/lib/*.pw examples/accepted/*.pw; \
       echo; echo "== mutation controls (scripts/any_type_mutations.py)"; echo; \
       python3 scripts/any_type_mutations.py; \
       echo; \
       echo "NOT CLAIMED: a generic function's result its arguments do not fix, which"; \
       echo "stays unknown. Each undecided relation above is counted, never agreement."; \
     } > docs/evidence/E10/any.txt
    @grep -E "^test result|mutants killed" docs/evidence/E10/any.txt

# ADR-0066: a nested declaration sees the bindings around it. Its tests, and
# the mutation controls.
e10-nested:
    @{ echo "ADR-0066 - a nested declaration sees the bindings around it"; echo; \
       echo "produced by: just e10-nested"; \
       echo "commit: $(git rev-parse HEAD)$(git diff --quiet HEAD -- . ':(exclude)docs/evidence' || echo ' + uncommitted changes')"; \
       echo "rust: $(rustc --version)"; echo; \
       echo "== nested declarations, streams, clauses, calls (compiler/pw-core/tests/nested_scope.rs)"; echo; \
       cargo test --locked -p pw-core --test nested_scope 2>&1 | grep -E '^(test |test result)'; \
       echo; echo "== mutation controls (scripts/nested_scope_mutations.py)"; echo; \
       python3 scripts/nested_scope_mutations.py; \
       echo; \
       echo "NOT CLAIMED: one scope walk for the name check. names.rs keeps its own,"; \
       echo "which binds a keyword statement's first word where the resolver does not."; \
     } > docs/evidence/E10/nested.txt
    @grep -E "^test result|mutants killed" docs/evidence/E10/nested.txt

# ADR-0067: a record is built with each of its fields, once, and an `if`
# without `else` is no value. Its tests, and the mutation controls.
e10-record-fields:
    @{ echo "ADR-0067 - a record is built with each of its fields, once"; echo; \
       echo "produced by: just e10-record-fields"; \
       echo "commit: $(git rev-parse HEAD)$(git diff --quiet HEAD -- . ':(exclude)docs/evidence' || echo ' + uncommitted changes')"; \
       echo "rust: $(rustc --version)"; echo; \
       echo "== a record's fields, and an if without else (compiler/pw-core/tests/record_fields.rs)"; echo; \
       cargo test --locked -p pw-core --test record_fields 2>&1 | grep -E '^(test |test result)'; \
       echo; echo "== mutation controls (scripts/record_field_mutations.py)"; echo; \
       python3 scripts/record_field_mutations.py; \
     } > docs/evidence/E10/record-fields.txt
    @grep -E "^test result|mutants killed" docs/evidence/E10/record-fields.txt

# ADR-0068: what each construct takes, checked, and `elif` chains lowered as
# nested ifs. Its tests, the chains through the E8 host, and the mutation
# controls.
e10-calls:
    @{ echo "ADR-0068 - what each construct takes, checked"; echo; \
       echo "produced by: just e10-calls"; \
       echo "commit: $(git rev-parse HEAD)$(git diff --quiet HEAD -- . ':(exclude)docs/evidence' || echo ' + uncommitted changes')"; \
       echo "rust: $(rustc --version)"; echo; \
       echo "== calls, loops, tries and branches (compiler/pw-core/tests/calls_and_branches.rs)"; echo; \
       cargo test --locked -p pw-core --test calls_and_branches 2>&1 | grep -E '^(test |test result)'; \
       echo; echo "== elif chains through the E8 host (compiler/pw-conformance/tests/if_chains.rs)"; echo; \
       cargo test --locked -p pw-conformance --test if_chains 2>&1 | grep -E '^(test |test result)'; \
       echo; echo "== mutation controls (scripts/call_branch_mutations.py)"; echo; \
       python3 scripts/call_branch_mutations.py; \
       echo; \
       echo "NOT CLAIMED: a lambda with no annotation, called directly, whose"; \
       echo "parameter's type no use gives."; \
     } > docs/evidence/E10/calls.txt
    @grep -E "^test result|mutants killed" docs/evidence/E10/calls.txt

# ADR-0069: a list's items share one type, and an `Int` literal fits an
# `Int`. Its tests, and the mutation controls.
e10-lists:
    @{ echo "ADR-0069 - a list's items share one type, and an Int literal fits an Int"; echo; \
       echo "produced by: just e10-lists"; \
       echo "commit: $(git rev-parse HEAD)$(git diff --quiet HEAD -- . ':(exclude)docs/evidence' || echo ' + uncommitted changes')"; \
       echo "rust: $(rustc --version)"; echo; \
       echo "== lists and literals (compiler/pw-core/tests/lists_and_literals.rs)"; echo; \
       cargo test --locked -p pw-core --test lists_and_literals 2>&1 | grep -E '^(test |test result)'; \
       echo; echo "== mutation controls (scripts/list_literal_mutations.py)"; echo; \
       python3 scripts/list_literal_mutations.py; \
     } > docs/evidence/E10/lists.txt
    @grep -E "^test result|mutants killed" docs/evidence/E10/lists.txt

# ADR-0070: an assignment to a field has the field's type. Its test, and the
# mutation control.
e10-field-assignment:
    @{ echo "ADR-0070 - an assignment to a field has the field's type"; echo; \
       echo "produced by: just e10-field-assignment"; \
       echo "commit: $(git rev-parse HEAD)$(git diff --quiet HEAD -- . ':(exclude)docs/evidence' || echo ' + uncommitted changes')"; \
       echo "rust: $(rustc --version)"; echo; \
       echo "== a field's assignment (compiler/pw-core/tests/field_assignment.rs)"; echo; \
       cargo test --locked -p pw-core --test field_assignment 2>&1 | grep -E '^(test |test result)'; \
       echo; echo "== mutation controls (scripts/field_assignment_mutations.py)"; echo; \
       python3 scripts/field_assignment_mutations.py; \
     } > docs/evidence/E10/field-assignment.txt
    @grep -E "^test result|mutants killed" docs/evidence/E10/field-assignment.txt

# ADR-0071: what a template's blocks and events take. Its tests, the store's
# test that every relation is decided, and the mutation controls.
e10-template-operands:
    @{ echo "ADR-0071 - what a template's blocks and events take"; echo; \
       echo "produced by: just e10-template-operands"; \
       echo "commit: $(git rev-parse HEAD)$(git diff --quiet HEAD -- . ':(exclude)docs/evidence' || echo ' + uncommitted changes')"; \
       echo "rust: $(rustc --version)"; echo; \
       echo "== a template's operands (compiler/pw-core/tests/template_operands.rs)"; echo; \
       cargo test --locked -p pw-core --test template_operands 2>&1 | grep -E '^(test |test result)'; \
       echo; echo "== the store's relations, each decided (compiler/pw-core/tests/value_relations.rs)"; echo; \
       cargo test --locked -p pw-core --test value_relations the_store_program_is_decided_not_merely_silent 2>&1 | grep -E '^(test |test result)'; \
       echo; echo "== mutation controls (scripts/template_operand_mutations.py)"; echo; \
       python3 scripts/template_operand_mutations.py; \
     } > docs/evidence/E10/template-operands.txt
    @grep -E "^test result|mutants killed" docs/evidence/E10/template-operands.txt

# ADR-0072: an element named with a capital letter is a view. Its tests, and
# the mutation controls.
e10-view-elements:
    @{ echo "ADR-0072 - an element named with a capital letter is a view"; echo; \
       echo "produced by: just e10-view-elements"; \
       echo "commit: $(git rev-parse HEAD)$(git diff --quiet HEAD -- . ':(exclude)docs/evidence' || echo ' + uncommitted changes')"; \
       echo "rust: $(rustc --version)"; echo; \
       echo "== view elements (compiler/pw-core/tests/view_elements.rs)"; echo; \
       cargo test --locked -p pw-core --test view_elements 2>&1 | grep -E '^(test |test result)'; \
       echo; echo "== mutation controls (scripts/view_element_mutations.py)"; echo; \
       python3 scripts/view_element_mutations.py; \
     } > docs/evidence/E10/view-elements.txt
    @grep -E "^test result|mutants killed" docs/evidence/E10/view-elements.txt

# ADR-0073: a template reads each value by path. The checker's and the build's
# tests, the renderer's key tests, and the mutation controls.
e10-template-values:
    @{ echo "ADR-0073 - a template reads each value by path"; echo; \
       echo "produced by: just e10-template-values"; \
       echo "commit: $(git rev-parse HEAD)$(git diff --quiet HEAD -- . ':(exclude)docs/evidence' || echo ' + uncommitted changes')"; \
       echo "rust: $(rustc --version)"; echo; \
       echo "== the checker, the template IR and the build (compiler/pw-core/tests/template_values.rs)"; echo; \
       cargo test --locked -p pw-core --test template_values 2>&1 | grep -E '^(test |test result)'; \
       echo; echo "== the renderer's keys (runtime/pw-render/tests/keys.rs)"; echo; \
       cargo test --locked -p pw-render --test keys 2>&1 | grep -E '^(test |test result)'; \
       echo; echo "== mutation controls (scripts/template_value_mutations.py)"; echo; \
       python3 scripts/template_value_mutations.py; \
     } > docs/evidence/E10/template-values.txt
    @grep -E "^test result|mutants killed" docs/evidence/E10/template-values.txt

# ADR-0074: what a template writes has a text form. Its tests, ADR-0073's
# (a computed list now checks, and does not build), and the mutation controls.
e10-template-text:
    @{ echo "ADR-0074 - what a template writes has a text form"; echo; \
       echo "produced by: just e10-template-text"; \
       echo "commit: $(git rev-parse HEAD)$(git diff --quiet HEAD -- . ':(exclude)docs/evidence' || echo ' + uncommitted changes')"; \
       echo "rust: $(rustc --version)"; echo; \
       echo "== what a template writes (compiler/pw-core/tests/template_text.rs)"; echo; \
       cargo test --locked -p pw-core --test template_text 2>&1 | grep -E '^(test |test result)'; \
       echo; echo "== what a template reads by path (compiler/pw-core/tests/template_values.rs)"; echo; \
       cargo test --locked -p pw-core --test template_values 2>&1 | grep -E '^(test |test result)'; \
       echo; echo "== mutation controls (scripts/template_text_mutations.py)"; echo; \
       python3 scripts/template_text_mutations.py; \
     } > docs/evidence/E10/template-text.txt
    @grep -E "^test result|mutants killed" docs/evidence/E10/template-text.txt

# ADR-0075: a stream and a mounted resource do not build. Its tests, and the
# mutation controls.
e10-streams:
    @{ echo "ADR-0075 - a stream and a mounted resource do not build"; echo; \
       echo "produced by: just e10-streams"; \
       echo "commit: $(git rev-parse HEAD)$(git diff --quiet HEAD -- . ':(exclude)docs/evidence' || echo ' + uncommitted changes')"; \
       echo "rust: $(rustc --version)"; echo; \
       echo "== streams and resources (compiler/pw-core/tests/streams_and_resources.rs)"; echo; \
       cargo test --locked -p pw-core --test streams_and_resources 2>&1 | grep -E '^(test |test result)'; \
       echo; echo "== mutation controls (scripts/stream_mutations.py)"; echo; \
       python3 scripts/stream_mutations.py; \
     } > docs/evidence/E10/streams.txt
    @grep -E "^test result|mutants killed" docs/evidence/E10/streams.txt

# ADR-0076: an arm no value reaches is refused. Its tests, and the mutation
# controls.
e10-unreachable-arms:
    @{ echo "ADR-0076 - an arm no value reaches is refused"; echo; \
       echo "produced by: just e10-unreachable-arms"; \
       echo "commit: $(git rev-parse HEAD)$(git diff --quiet HEAD -- . ':(exclude)docs/evidence' || echo ' + uncommitted changes')"; \
       echo "rust: $(rustc --version)"; echo; \
       echo "== unreachable arms (compiler/pw-core/tests/unreachable_arms.rs)"; echo; \
       cargo test --locked -p pw-core --test unreachable_arms 2>&1 | grep -E '^(test |test result)'; \
       echo; echo "== mutation controls (scripts/unreachable_arm_mutations.py)"; echo; \
       python3 scripts/unreachable_arm_mutations.py; \
     } > docs/evidence/E10/unreachable-arms.txt
    @grep -E "^test result|mutants killed" docs/evidence/E10/unreachable-arms.txt

# ADR-0077: a call through a field holding a function is checked. Its tests,
# and the mutation controls.
e10-field-calls:
    @{ echo "ADR-0077 - a call through a field holding a function is checked"; echo; \
       echo "produced by: just e10-field-calls"; \
       echo "commit: $(git rev-parse HEAD)$(git diff --quiet HEAD -- . ':(exclude)docs/evidence' || echo ' + uncommitted changes')"; \
       echo "rust: $(rustc --version)"; echo; \
       echo "== field calls (compiler/pw-core/tests/field_calls.rs)"; echo; \
       cargo test --locked -p pw-core --test field_calls 2>&1 | grep -E '^(test |test result)'; \
       echo; echo "== mutation controls (scripts/field_call_mutations.py)"; echo; \
       python3 scripts/field_call_mutations.py; \
     } > docs/evidence/E10/field-calls.txt
    @grep -E "^test result|mutants killed" docs/evidence/E10/field-calls.txt

# ADR-0078: an effect is performed where its function is named. Its tests,
# the generality witnesses, and the mutation controls.
e10-effects-through-values:
    @{ echo "ADR-0078 - an effect is performed where its function is named"; echo; \
       echo "produced by: just e10-effects-through-values"; \
       echo "commit: $(git rev-parse HEAD)$(git diff --quiet HEAD -- . ':(exclude)docs/evidence' || echo ' + uncommitted changes')"; \
       echo "rust: $(rustc --version)"; echo; \
       echo "== effects through values (compiler/pw-core/tests/effects_through_values.rs)"; echo; \
       cargo test --locked -p pw-core --test effects_through_values 2>&1 | grep -E '^(test |test result)'; \
       echo; echo "== the generality witnesses (compiler/pw-core/tests/generality.rs)"; echo; \
       cargo test --locked -p pw-core --test generality 2>&1 | grep -E '^(test |test result)'; \
       echo; echo "== mutation controls (scripts/effect_value_mutations.py)"; echo; \
       python3 scripts/effect_value_mutations.py; \
     } > docs/evidence/E10/effects-through-values.txt
    @grep -E "^test result|mutants killed" docs/evidence/E10/effects-through-values.txt

# ADR-0079: a function value carries the label of what it makes. Its tests,
# and the mutation controls.
e10-labels-through-values:
    @{ echo "ADR-0079 - a function value carries the label of what it makes"; echo; \
       echo "produced by: just e10-labels-through-values"; \
       echo "commit: $(git rev-parse HEAD)$(git diff --quiet HEAD -- . ':(exclude)docs/evidence' || echo ' + uncommitted changes')"; \
       echo "rust: $(rustc --version)"; echo; \
       echo "== labels through values (compiler/pw-core/tests/labels_through_values.rs)"; echo; \
       cargo test --locked -p pw-core --test labels_through_values 2>&1 | grep -E '^(test |test result)'; \
       echo; echo "== mutation controls (scripts/label_value_mutations.py)"; echo; \
       python3 scripts/label_value_mutations.py; \
     } > docs/evidence/E10/labels-through-values.txt
    @grep -E "^test result|mutants killed" docs/evidence/E10/labels-through-values.txt

# ADR-0080: the affine rule follows bindings, not names. Its tests, and the
# mutation controls.
e10-affine-bindings:
    @{ echo "ADR-0080 - the affine rule follows bindings, not names"; echo; \
       echo "produced by: just e10-affine-bindings"; \
       echo "commit: $(git rev-parse HEAD)$(git diff --quiet HEAD -- . ':(exclude)docs/evidence' || echo ' + uncommitted changes')"; \
       echo "rust: $(rustc --version)"; echo; \
       echo "== affine bindings (compiler/pw-core/tests/affine_bindings.rs)"; echo; \
       cargo test --locked -p pw-core --test affine_bindings 2>&1 | grep -E '^(test |test result)'; \
       echo; echo "== mutation controls (scripts/affine_binding_mutations.py)"; echo; \
       python3 scripts/affine_binding_mutations.py; \
     } > docs/evidence/E10/affine-bindings.txt
    @grep -E "^test result|mutants killed" docs/evidence/E10/affine-bindings.txt

# ADR-0081: a named argument is given to the parameter of its name. The
# checker's tests, the call run through the E8 host, and the mutation
# controls.
e10-named-arguments:
    @{ echo "ADR-0081 - a named argument is given to the parameter of its name"; echo; \
       echo "produced by: just e10-named-arguments"; \
       echo "commit: $(git rev-parse HEAD)$(git diff --quiet HEAD -- . ':(exclude)docs/evidence' || echo ' + uncommitted changes')"; \
       echo "rust: $(rustc --version)"; echo; \
       echo "== the checker and the labels (compiler/pw-core/tests/named_arguments.rs)"; echo; \
       cargo test --locked -p pw-core --test named_arguments 2>&1 | grep -E '^(test |test result)'; \
       echo; echo "== the call, through the E8 host (compiler/pw-conformance/tests/named_arguments.rs)"; echo; \
       cargo test --locked -p pw-conformance --test named_arguments 2>&1 | grep -E '^(test |test result)'; \
       echo; echo "== mutation controls (scripts/named_argument_mutations.py)"; echo; \
       python3 scripts/named_argument_mutations.py; \
     } > docs/evidence/E10/named-arguments.txt
    @grep -E "^test result|mutants killed" docs/evidence/E10/named-arguments.txt

# ADR-0082: a `derived` value performs no effect. Its tests, and the mutation
# controls.
e10-derived:
    @{ echo "ADR-0082 - a derived value performs no effect"; echo; \
       echo "produced by: just e10-derived"; \
       echo "commit: $(git rev-parse HEAD)$(git diff --quiet HEAD -- . ':(exclude)docs/evidence' || echo ' + uncommitted changes')"; \
       echo "rust: $(rustc --version)"; echo; \
       echo "== derived purity (compiler/pw-core/tests/derived_purity.rs)"; echo; \
       cargo test --locked -p pw-core --test derived_purity 2>&1 | grep -E '^(test |test result)'; \
       echo; echo "== mutation controls (scripts/derived_mutations.py)"; echo; \
       python3 scripts/derived_mutations.py; \
     } > docs/evidence/E10/derived.txt
    @grep -E "^test result|mutants killed" docs/evidence/E10/derived.txt

# ADR-0083: a call's arguments open on the callee's line. Its tests, the
# grammar's, and the mutation control.
e10-call-lines:
    @{ echo "ADR-0083 - a call's arguments open on the callee's line"; echo; \
       echo "produced by: just e10-call-lines"; \
       echo "commit: $(git rev-parse HEAD)$(git diff --quiet HEAD -- . ':(exclude)docs/evidence' || echo ' + uncommitted changes')"; \
       echo "rust: $(rustc --version)"; echo; \
       echo "== call lines (compiler/pw-core/tests/call_lines.rs)"; echo; \
       cargo test --locked -p pw-core --test call_lines 2>&1 | grep -E '^(test |test result)'; \
       echo; echo "== the grammar (compiler/pw-syntax)"; echo; \
       cargo test --locked -p pw-syntax 2>&1 | grep -E '^test result'; \
       echo; echo "== mutation control (scripts/call_line_mutations.py)"; echo; \
       python3 scripts/call_line_mutations.py; \
     } > docs/evidence/E10/call-lines.txt
    @grep -E "^test result|mutants killed" docs/evidence/E10/call-lines.txt

# ADR-0084: what a string interpolates has a text form. Its tests, and the
# mutation controls.
e10-string-holes:
    @{ echo "ADR-0084 - what a string interpolates has a text form"; echo; \
       echo "produced by: just e10-string-holes"; \
       echo "commit: $(git rev-parse HEAD)$(git diff --quiet HEAD -- . ':(exclude)docs/evidence' || echo ' + uncommitted changes')"; \
       echo "rust: $(rustc --version)"; echo; \
       echo "== string holes (compiler/pw-core/tests/string_holes.rs)"; echo; \
       cargo test --locked -p pw-core --test string_holes 2>&1 | grep -E '^(test |test result)'; \
       echo; echo "== mutation controls (scripts/string_hole_mutations.py)"; echo; \
       python3 scripts/string_hole_mutations.py; \
     } > docs/evidence/E10/string-holes.txt
    @grep -E "^test result|mutants killed" docs/evidence/E10/string-holes.txt

# ADR-0085: a call carries what it is given. Its tests, and the mutation
# controls.
e10-labels-through-plain-values:
    @{ echo "ADR-0085 - a call carries what it is given"; echo; \
       echo "produced by: just e10-labels-through-plain-values"; \
       echo "commit: $(git rev-parse HEAD)$(git diff --quiet HEAD -- . ':(exclude)docs/evidence' || echo ' + uncommitted changes')"; \
       echo "rust: $(rustc --version)"; echo; \
       echo "== labels through plain values (compiler/pw-core/tests/labels_through_plain_values.rs)"; echo; \
       cargo test --locked -p pw-core --test labels_through_plain_values 2>&1 | grep -E '^(test |test result)'; \
       echo; echo "== mutation controls (scripts/label_plain_mutations.py)"; echo; \
       python3 scripts/label_plain_mutations.py; \
     } > docs/evidence/E10/labels-through-plain-values.txt
    @grep -E "^test result|mutants killed" docs/evidence/E10/labels-through-plain-values.txt

# ADR-0086: a function crosses no boundary. Its test, and the mutation
# controls.
e10-function-captures:
    @{ echo "ADR-0086 - a function crosses no boundary"; echo; \
       echo "produced by: just e10-function-captures"; \
       echo "commit: $(git rev-parse HEAD)$(git diff --quiet HEAD -- . ':(exclude)docs/evidence' || echo ' + uncommitted changes')"; \
       echo "rust: $(rustc --version)"; echo; \
       echo "== function captures (compiler/pw-core/tests/function_captures.rs)"; echo; \
       cargo test --locked -p pw-core --test function_captures 2>&1 | grep -E '^(test |test result)'; \
       echo; echo "== mutation controls (scripts/function_capture_mutations.py)"; echo; \
       python3 scripts/function_capture_mutations.py; \
     } > docs/evidence/E10/function-captures.txt
    @grep -E "^test result|mutants killed" docs/evidence/E10/function-captures.txt

# ADR-0087: a call names a term. Its tests, and the mutation controls.
e10-call-names:
    @{ echo "ADR-0087 - a call names a term"; echo; \
       echo "produced by: just e10-call-names"; \
       echo "commit: $(git rev-parse HEAD)$(git diff --quiet HEAD -- . ':(exclude)docs/evidence' || echo ' + uncommitted changes')"; \
       echo "rust: $(rustc --version)"; echo; \
       echo "== call names (compiler/pw-core/tests/calls_name_terms.rs)"; echo; \
       cargo test --locked -p pw-core --test calls_name_terms 2>&1 | grep -E '^(test |test result)'; \
       echo; echo "== mutation controls (scripts/call_name_mutations.py)"; echo; \
       python3 scripts/call_name_mutations.py; \
     } > docs/evidence/E10/call-names.txt
    @grep -E "^test result|mutants killed" docs/evidence/E10/call-names.txt

# ADR-0088: a clause names a declaration of its kind, and gives it its key.
# Its tests, and the mutation controls.
e10-clause-keys:
    @{ echo "ADR-0088 - a clause names a declaration of its kind, and gives it its key"; echo; \
       echo "produced by: just e10-clause-keys"; \
       echo "commit: $(git rev-parse HEAD)$(git diff --quiet HEAD -- . ':(exclude)docs/evidence' || echo ' + uncommitted changes')"; \
       echo "rust: $(rustc --version)"; echo; \
       echo "== clause keys (compiler/pw-core/tests/clause_keys.rs)"; echo; \
       cargo test --locked -p pw-core --test clause_keys 2>&1 | grep -E '^(test |test result)'; \
       echo; echo "== mutation controls (scripts/clause_key_mutations.py)"; echo; \
       python3 scripts/clause_key_mutations.py; \
     } > docs/evidence/E10/clause-keys.txt
    @grep -E "^test result|mutants killed" docs/evidence/E10/clause-keys.txt

# ADR-0089: a policy's value is one its domain has. Its tests, and the
# mutation controls.
e10-policy-values:
    @{ echo "ADR-0089 - a policy's value is one its domain has"; echo; \
       echo "produced by: just e10-policy-values"; \
       echo "commit: $(git rev-parse HEAD)$(git diff --quiet HEAD -- . ':(exclude)docs/evidence' || echo ' + uncommitted changes')"; \
       echo "rust: $(rustc --version)"; echo; \
       echo "== policy values (compiler/pw-core/tests/policy_values.rs)"; echo; \
       cargo test --locked -p pw-core --test policy_values 2>&1 | grep -E '^(test |test result)'; \
       echo; echo "== mutation controls (scripts/policy_value_mutations.py)"; echo; \
       python3 scripts/policy_value_mutations.py; \
     } > docs/evidence/E10/policy-values.txt
    @grep -E "^test result|mutants killed" docs/evidence/E10/policy-values.txt

# ADR-0090: `pw build` checks what `pw check` checks. Its tests, the corpus
# standard the declaration rules are held to, and the mutation controls.
e10-one-checker:
    @{ echo "ADR-0090 - pw build checks what pw check checks"; echo; \
       echo "produced by: just e10-one-checker"; \
       echo "commit: $(git rev-parse HEAD)$(git diff --quiet HEAD -- . ':(exclude)docs/evidence' || echo ' + uncommitted changes')"; \
       echo "rust: $(rustc --version)"; echo; \
       echo "== one checker (compiler/pw-core/tests/one_checker.rs)"; echo; \
       cargo test --locked -p pw-core --test one_checker 2>&1 | grep -E '^(test |test result)'; \
       echo; echo "== the corpus standard (compiler/pw-core/tests/checking_source.rs)"; echo; \
       cargo test --locked -p pw-core --test checking_source 2>&1 | grep -E '^(test |test result)'; \
       echo; echo "== mutation controls (scripts/one_checker_mutations.py)"; echo; \
       python3 scripts/one_checker_mutations.py; \
     } > docs/evidence/E10/one-checker.txt
    @grep -E "^test result|mutants killed" docs/evidence/E10/one-checker.txt

# ADR-0091: a listener binds its entry's key. The checker's tests, the
# materializer's, and the mutation controls.
e10-listeners:
    @{ echo "ADR-0091 - a listener binds its entry's key"; echo; \
       echo "produced by: just e10-listeners"; \
       echo "commit: $(git rev-parse HEAD)$(git diff --quiet HEAD -- . ':(exclude)docs/evidence' || echo ' + uncommitted changes')"; \
       echo "rust: $(rustc --version)"; echo; \
       echo "== listener keys (compiler/pw-core/tests/listener_keys.rs)"; echo; \
       cargo test --locked -p pw-core --test listener_keys 2>&1 | grep -E '^(test |test result)'; \
       echo; echo "== the materializer (runtime/pw-materialize/tests/listeners.rs)"; echo; \
       cargo test --locked -p pw-materialize --test listeners 2>&1 | grep -E '^(test |test result)'; \
       echo; echo "== mutation controls (scripts/listener_mutations.py)"; echo; \
       python3 scripts/listener_mutations.py; \
     } > docs/evidence/E10/listeners.txt
    @grep -E "^test result|mutants killed" docs/evidence/E10/listeners.txt

# ADR-0092: a dependency-graph clause belongs to a declaration that can mean
# it. Its tests, and the mutation controls.
e10-clause-places:
    @{ echo "ADR-0092 - a dependency-graph clause belongs to a declaration that can mean it"; echo; \
       echo "produced by: just e10-clause-places"; \
       echo "commit: $(git rev-parse HEAD)$(git diff --quiet HEAD -- . ':(exclude)docs/evidence' || echo ' + uncommitted changes')"; \
       echo "rust: $(rustc --version)"; echo; \
       echo "== clause places (compiler/pw-core/tests/clause_places.rs)"; echo; \
       cargo test --locked -p pw-core --test clause_places 2>&1 | grep -E '^(test |test result)'; \
       echo; echo "== mutation controls (scripts/clause_place_mutations.py)"; echo; \
       python3 scripts/clause_place_mutations.py; \
     } > docs/evidence/E10/clause-places.txt
    @grep -E "^test result|mutants killed" docs/evidence/E10/clause-places.txt

# ADR-0093: an element handles an event the platform declares. Its tests,
# and the mutation controls.
e10-declared-events:
    @{ echo "ADR-0093 - an element handles an event the platform declares"; echo; \
       echo "produced by: just e10-declared-events"; \
       echo "commit: $(git rev-parse HEAD)$(git diff --quiet HEAD -- . ':(exclude)docs/evidence' || echo ' + uncommitted changes')"; \
       echo "rust: $(rustc --version)"; echo; \
       echo "== declared events (compiler/pw-core/tests/declared_events.rs)"; echo; \
       cargo test --locked -p pw-core --test declared_events 2>&1 | grep -E '^(test |test result)'; \
       echo; echo "== mutation controls (scripts/declared_event_mutations.py)"; echo; \
       python3 scripts/declared_event_mutations.py; \
     } > docs/evidence/E10/declared-events.txt
    @grep -E "^test result|mutants killed" docs/evidence/E10/declared-events.txt

# ADR-0094: a template writes no code. Its tests, and the mutation controls.
e10-code-in-markup:
    @{ echo "ADR-0094 - a template writes no code"; echo; \
       echo "produced by: just e10-code-in-markup"; \
       echo "commit: $(git rev-parse HEAD)$(git diff --quiet HEAD -- . ':(exclude)docs/evidence' || echo ' + uncommitted changes')"; \
       echo "rust: $(rustc --version)"; echo; \
       echo "== code in markup (compiler/pw-core/tests/code_in_markup.rs)"; echo; \
       cargo test --locked -p pw-core --test code_in_markup 2>&1 | grep -E '^(test |test result)'; \
       echo; echo "== mutation controls (scripts/code_in_markup_mutations.py)"; echo; \
       python3 scripts/code_in_markup_mutations.py; \
     } > docs/evidence/E10/code-in-markup.txt
    @grep -E "^test result|mutants killed" docs/evidence/E10/code-in-markup.txt

# ADR-0095: an attribute's context is read as HTML reads its name. Its
# tests, and the mutation controls.
e10-attribute-case:
    @{ echo "ADR-0095 - an attribute's context is read as HTML reads its name"; echo; \
       echo "produced by: just e10-attribute-case"; \
       echo "commit: $(git rev-parse HEAD)$(git diff --quiet HEAD -- . ':(exclude)docs/evidence' || echo ' + uncommitted changes')"; \
       echo "rust: $(rustc --version)"; echo; \
       echo "== attribute case (compiler/pw-core/tests/attribute_case.rs)"; echo; \
       cargo test --locked -p pw-core --test attribute_case 2>&1 | grep -E '^(test |test result)'; \
       echo; echo "== mutation controls (scripts/attribute_case_mutations.py)"; echo; \
       python3 scripts/attribute_case_mutations.py; \
     } > docs/evidence/E10/attribute-case.txt
    @grep -E "^test result|mutants killed" docs/evidence/E10/attribute-case.txt

# ADR-0096: a template moves no URL. Its tests, and the mutation controls.
e10-moved-urls:
    @{ echo "ADR-0096 - a template moves no URL"; echo; \
       echo "produced by: just e10-moved-urls"; \
       echo "commit: $(git rev-parse HEAD)$(git diff --quiet HEAD -- . ':(exclude)docs/evidence' || echo ' + uncommitted changes')"; \
       echo "rust: $(rustc --version)"; echo; \
       echo "== moved URLs (compiler/pw-core/tests/platform_markup.rs)"; echo; \
       cargo test --locked -p pw-core --test platform_markup 2>&1 | grep -E '^(test |test result)'; \
       echo; echo "== mutation controls (scripts/moved_url_mutations.py)"; echo; \
       python3 scripts/moved_url_mutations.py; \
     } > docs/evidence/E10/moved-urls.txt
    @grep -E "^test result|mutants killed" docs/evidence/E10/moved-urls.txt

# ADR-0097: data embedded in a page cannot end its script element. The
# renderer's security matrix, and the mutation controls.
e10-embedded-json:
    @{ echo "ADR-0097 - data embedded in a page cannot end its script element"; echo; \
       echo "produced by: just e10-embedded-json"; \
       echo "commit: $(git rev-parse HEAD)$(git diff --quiet HEAD -- . ':(exclude)docs/evidence' || echo ' + uncommitted changes')"; \
       echo "rust: $(rustc --version)"; echo; \
       echo "== the escaping matrix (runtime/pw-render/tests/security.rs)"; echo; \
       cargo test --locked -p pw-render --test security 2>&1 | grep -E '^(test |test result)'; \
       echo; echo "== mutation controls (scripts/embedded_json_mutations.py)"; echo; \
       python3 scripts/embedded_json_mutations.py; \
     } > docs/evidence/E10/embedded-json.txt
    @grep -E "^test result|mutants killed" docs/evidence/E10/embedded-json.txt

# ADR-0098: a name is written once where it is declared. Its tests, and the
# mutation controls.
e10-declared-once:
    @{ echo "ADR-0098 - a name is written once where it is declared"; echo; \
       echo "produced by: just e10-declared-once"; \
       echo "commit: $(git rev-parse HEAD)$(git diff --quiet HEAD -- . ':(exclude)docs/evidence' || echo ' + uncommitted changes')"; \
       echo "rust: $(rustc --version)"; echo; \
       echo "== declared once (compiler/pw-core/tests/declared_once.rs)"; echo; \
       cargo test --locked -p pw-core --test declared_once 2>&1 | grep -E '^(test |test result)'; \
       echo; echo "== mutation controls (scripts/declared_once_mutations.py)"; echo; \
       python3 scripts/declared_once_mutations.py; \
     } > docs/evidence/E10/declared-once.txt
    @grep -E "^test result|mutants killed" docs/evidence/E10/declared-once.txt

# ADR-0099: a failure is handled. Its tests, and the mutation controls.
e10-results-handled:
    @{ echo "ADR-0099 - a failure is handled"; echo; \
       echo "produced by: just e10-results-handled"; \
       echo "commit: $(git rev-parse HEAD)$(git diff --quiet HEAD -- . ':(exclude)docs/evidence' || echo ' + uncommitted changes')"; \
       echo "rust: $(rustc --version)"; echo; \
       echo "== results handled (compiler/pw-core/tests/results_handled.rs)"; echo; \
       cargo test --locked -p pw-core --test results_handled 2>&1 | grep -E '^(test |test result)'; \
       echo; echo "== mutation controls (scripts/results_handled_mutations.py)"; echo; \
       python3 scripts/results_handled_mutations.py; \
     } > docs/evidence/E10/results-handled.txt
    @grep -E "^test result|mutants killed" docs/evidence/E10/results-handled.txt

# ADR-0100: a query reads. Its tests, and the mutation controls.
e10-query-reads:
    @{ echo "ADR-0100 - a query reads"; echo; \
       echo "produced by: just e10-query-reads"; \
       echo "commit: $(git rev-parse HEAD)$(git diff --quiet HEAD -- . ':(exclude)docs/evidence' || echo ' + uncommitted changes')"; \
       echo "rust: $(rustc --version)"; echo; \
       echo "== query reads (compiler/pw-core/tests/query_reads.rs, effects.rs)"; echo; \
       cargo test --locked -p pw-core --test query_reads 2>&1 | grep -E '^(test |test result)'; \
       cargo test --locked -p pw-core --lib a_query_may_read_and_not_write 2>&1 | grep -E '^(test |test result)'; \
       echo; echo "== mutation controls (scripts/query_read_mutations.py)"; echo; \
       python3 scripts/query_read_mutations.py; \
     } > docs/evidence/E10/query-reads.txt
    @grep -E "^test result|mutants killed" docs/evidence/E10/query-reads.txt

# ADR-0101: a command invalidates what it writes. Its tests, and the
# mutation controls.
e10-writes-invalidated:
    @{ echo "ADR-0101 - a command invalidates what it writes"; echo; \
       echo "produced by: just e10-writes-invalidated"; \
       echo "commit: $(git rev-parse HEAD)$(git diff --quiet HEAD -- . ':(exclude)docs/evidence' || echo ' + uncommitted changes')"; \
       echo "rust: $(rustc --version)"; echo; \
       echo "== writes invalidated (compiler/pw-core/tests/writes_invalidated.rs, effects.rs)"; echo; \
       cargo test --locked -p pw-core --test writes_invalidated 2>&1 | grep -E '^(test |test result)'; \
       cargo test --locked -p pw-core --lib a_database_effect_names_its_domain 2>&1 | grep -E '^(test |test result)'; \
       echo; echo "== mutation controls (scripts/write_invalidation_mutations.py)"; echo; \
       python3 scripts/write_invalidation_mutations.py; \
     } > docs/evidence/E10/writes-invalidated.txt
    @grep -E "^test result|mutants killed" docs/evidence/E10/writes-invalidated.txt

# ADR-0102: an event reaches what reads what it invalidates. The
# materializer's tests, ADR-0091's listener tests it must keep passing, and
# the mutation controls.
e10-read-through:
    @{ echo "ADR-0102 - an event reaches what reads what it invalidates"; echo; \
       echo "produced by: just e10-read-through"; \
       echo "commit: $(git rev-parse HEAD)$(git diff --quiet HEAD -- . ':(exclude)docs/evidence' || echo ' + uncommitted changes')"; \
       echo "rust: $(rustc --version)"; echo; \
       echo "== reads followed (runtime/pw-materialize/tests/reads.rs)"; echo; \
       cargo test --locked -p pw-materialize --test reads 2>&1 | grep -E '^(test |test result)'; \
       echo; echo "== the M6 gate and the listeners, unchanged"; echo; \
       cargo test --locked -p pw-materialize --test gate --test listeners 2>&1 | grep -E '^test result'; \
       echo; echo "== mutation controls (scripts/read_through_mutations.py)"; echo; \
       python3 scripts/read_through_mutations.py; \
     } > docs/evidence/E10/read-through.txt
    @grep -E "^test result|mutants killed" docs/evidence/E10/read-through.txt

# ADR-0103: a write reaches the fragments built on it. Its tests, ADR-0101's
# it must keep passing, and the mutation controls.
e10-fragments-reached:
    @{ echo "ADR-0103 - a write reaches the fragments built on it"; echo; \
       echo "produced by: just e10-fragments-reached"; \
       echo "commit: $(git rev-parse HEAD)$(git diff --quiet HEAD -- . ':(exclude)docs/evidence' || echo ' + uncommitted changes')"; \
       echo "rust: $(rustc --version)"; echo; \
       echo "== fragments reached (compiler/pw-core/tests/writes_reach_fragments.rs)"; echo; \
       cargo test --locked -p pw-core --test writes_reach_fragments 2>&1 | grep -E '^(test |test result)'; \
       echo; echo "== ADR-0101's readers, unchanged"; echo; \
       cargo test --locked -p pw-core --test writes_invalidated 2>&1 | grep -E '^test result'; \
       echo; echo "== mutation controls (scripts/fragment_reach_mutations.py)"; echo; \
       python3 scripts/fragment_reach_mutations.py; \
     } > docs/evidence/E10/fragments-reached.txt
    @grep -E "^test result|mutants killed" docs/evidence/E10/fragments-reached.txt

# ADR-0104: the dev server commits the events a command declares. The
# server's tests, and the mutation controls.
e10-committed-events:
    @{ echo "ADR-0104 - the dev server commits the events a command declares"; echo; \
       echo "produced by: just e10-committed-events"; \
       echo "commit: $(git rev-parse HEAD)$(git diff --quiet HEAD -- . ':(exclude)docs/evidence' || echo ' + uncommitted changes')"; \
       echo "rust: $(rustc --version)"; echo; \
       echo "== the dev server (spikes/own-renderer/server)"; echo; \
       cargo test --locked -p pw-dev-server 2>&1 | grep -E '^(test |test result)'; \
       echo; echo "== mutation controls (scripts/committed_events_mutations.py)"; echo; \
       python3 scripts/committed_events_mutations.py; \
     } > docs/evidence/E10/committed-events.txt
    @grep -E "^test result|mutants killed" docs/evidence/E10/committed-events.txt

# ADR-0105: a command invalidates the entry it speculates on. Its tests,
# ADR-0101's and ADR-0103's it must keep passing, and the mutation controls.
e10-speculation-reconciled:
    @{ echo "ADR-0105 - a command invalidates the entry it speculates on"; echo; \
       echo "produced by: just e10-speculation-reconciled"; \
       echo "commit: $(git rev-parse HEAD)$(git diff --quiet HEAD -- . ':(exclude)docs/evidence' || echo ' + uncommitted changes')"; \
       echo "rust: $(rustc --version)"; echo; \
       echo "== speculations reconciled (compiler/pw-core/tests/speculation_reconciled.rs)"; echo; \
       cargo test --locked -p pw-core --test speculation_reconciled 2>&1 | grep -E '^(test |test result)'; \
       echo; echo "== ADR-0101's readers and ADR-0103's fragments, unchanged"; echo; \
       cargo test --locked -p pw-core --test writes_invalidated --test writes_reach_fragments 2>&1 | grep -E '^test result'; \
       echo; echo "== mutation controls (scripts/speculation_mutations.py)"; echo; \
       python3 scripts/speculation_mutations.py; \
     } > docs/evidence/E10/speculation-reconciled.txt
    @grep -E "^test result|mutants killed" docs/evidence/E10/speculation-reconciled.txt

# ADR-0106: `pw check` reports each file by its place. Its tests, run
# against the binary, and the mutation controls.
e10-same-name:
    @{ echo "ADR-0106 - pw check reports each file by its place"; echo; \
       echo "produced by: just e10-same-name"; \
       echo "commit: $(git rev-parse HEAD)$(git diff --quiet HEAD -- . ':(exclude)docs/evidence' || echo ' + uncommitted changes')"; \
       echo "rust: $(rustc --version)"; echo; \
       echo "== two files of one name (compiler/pw-cli/tests/same_name.rs)"; echo; \
       cargo test --locked -p pw-cli --test same_name 2>&1 | grep -E '^(test |test result)'; \
       echo; echo "== mutation controls (scripts/same_name_mutations.py)"; echo; \
       python3 scripts/same_name_mutations.py; \
     } > docs/evidence/E10/same-name.txt
    @grep -E "^test result|mutants killed" docs/evidence/E10/same-name.txt

# ADR-0107: a cache key names each parameter its entry depends on. Its
# tests, and the mutation controls.
e10-keys-cover-reads:
    @{ echo "ADR-0107 - a cache key names each parameter its entry depends on"; echo; \
       echo "produced by: just e10-keys-cover-reads"; \
       echo "commit: $(git rev-parse HEAD)$(git diff --quiet HEAD -- . ':(exclude)docs/evidence' || echo ' + uncommitted changes')"; \
       echo "rust: $(rustc --version)"; echo; \
       echo "== keys cover reads (compiler/pw-core/tests/keys_cover_reads.rs)"; echo; \
       cargo test --locked -p pw-core --test keys_cover_reads 2>&1 | grep -E '^(test |test result)'; \
       echo; echo "== mutation controls (scripts/key_read_mutations.py)"; echo; \
       python3 scripts/key_read_mutations.py; \
     } > docs/evidence/E10/keys-cover-reads.txt
    @grep -E "^test result|mutants killed" docs/evidence/E10/keys-cover-reads.txt

# ADR-0108: a query names a resource that exists. Its tests, and the
# mutation controls.
e10-queries-name-resources:
    @{ echo "ADR-0108 - a query names a resource that exists"; echo; \
       echo "produced by: just e10-queries-name-resources"; \
       echo "commit: $(git rev-parse HEAD)$(git diff --quiet HEAD -- . ':(exclude)docs/evidence' || echo ' + uncommitted changes')"; \
       echo "rust: $(rustc --version)"; echo; \
       echo "== queries name resources (compiler/pw-core/tests/queries_name_resources.rs)"; echo; \
       cargo test --locked -p pw-core --test queries_name_resources 2>&1 | grep -E '^(test |test result)'; \
       echo; echo "== mutation controls (scripts/query_name_mutations.py)"; echo; \
       python3 scripts/query_name_mutations.py; \
     } > docs/evidence/E10/queries-name-resources.txt
    @grep -E "^test result|mutants killed" docs/evidence/E10/queries-name-resources.txt

# ADR-0061: a declared sum type in a template's `{#match}`. The checker's
# reading of each arm, the template IR's names for the cases, the renderer
# binding a case's fields, kiokun's server carrying a case, and the mutation
# controls.
e10-template-sum-types:
    @{ echo "ADR-0061 - a declared sum type in a template's match"; echo; \
       echo "produced by: just e10-template-sum-types"; \
       echo "commit: $(git rev-parse HEAD)$(git diff --quiet HEAD -- . ':(exclude)docs/evidence' || echo ' + uncommitted changes')"; \
       echo "rust: $(rustc --version)"; echo; \
       echo "== the checker and the template IR (compiler/pw-core/tests/template_blocks.rs)"; echo; \
       cargo test --locked -p pw-core --test template_blocks 2>&1 | grep -E '^(test |test result)'; \
       echo; echo "== the renderer (runtime/pw-render/tests/branches.rs)"; echo; \
       cargo test --locked -p pw-render --test branches 2>&1 | grep -E '^(test |test result)'; \
       echo; echo "== kiokun's server"; echo; \
       cargo test --locked -p kiokun-server a_declared_case 2>&1 | grep -E '^(test |test result: ok. [1-9])'; \
       echo; echo "== mutation controls (scripts/template_match_mutations.py)"; echo; \
       python3 scripts/template_match_mutations.py; \
       echo; \
       echo "NOT CLAIMED: a nested or literal pattern in a template arm. NOT CLAIMED:"; \
       echo "patches for a {#match} region, which renders on the server."; \
     } > docs/evidence/E10/template-sum-types.txt
    @grep -E "^test result|mutants killed" docs/evidence/E10/template-sum-types.txt

# ADR-0060: nested and literal patterns. The checker's analysis and the
# typing of what a nested pattern binds, each match compiled to a decision
# tree and run through the E8 host against a Rust model, the component
# against the JavaScript module, and the mutation controls. Needs `node`.
e10-patterns:
    @{ echo "ADR-0060 - nested and literal patterns"; echo; \
       echo "produced by: just e10-patterns"; \
       echo "commit: $(git rev-parse HEAD)$(git diff --quiet HEAD -- . ':(exclude)docs/evidence' || echo ' + uncommitted changes')"; \
       echo "rust: $(rustc --version)"; echo "node: $(node --version)"; echo; \
       echo "== the checker (compiler/pw-core/tests/patterns.rs)"; echo; \
       cargo test --locked -p pw-core --test patterns 2>&1 | grep -E '^(test |test result)'; \
       echo; echo "== through the E8 host, against a model (compiler/pw-conformance/tests/patterns.rs)"; echo; \
       cargo test --locked -p pw-conformance --test patterns 2>&1 | grep -E '^(test |test result)'; \
       echo; echo "== the component and the JavaScript module"; echo; \
       cargo test --locked -p pw-conformance --test javascript -- --nocapture --test-threads=1 2>&1 \
         | grep -oE "javascript: [0-9]+ queries.*|^test result.*"; \
       echo; echo "== mutation controls (scripts/pattern_mutations.py)"; echo; \
       python3 scripts/pattern_mutations.py; \
       echo; \
       echo "NOT CLAIMED: a Float literal pattern, refused by name. NOT CLAIMED: an"; \
       echo "or-pattern that binds a name, refused by name. NOT CLAIMED: an arm no"; \
       echo "value reaches reported by the checker; the backend refuses one."; \
     } > docs/evidence/E10/patterns.txt
    @grep -E "^test result|^javascript:|mutants killed" docs/evidence/E10/patterns.txt

# The 2026-09-26 correction: a Pleris name WIT reserves (`List`, `own`) is
# escaped in the generated WIT text. Each query run through the E8 host, and
# the mutation controls.
e10-wit-names:
    @{ echo "WIT's reserved names, escaped"; echo; \
       echo "produced by: just e10-wit-names"; \
       echo "commit: $(git rev-parse HEAD)$(git diff --quiet HEAD -- . ':(exclude)docs/evidence' || echo ' + uncommitted changes')"; \
       echo "rust: $(rustc --version)"; echo; \
       echo "== through the E8 host (compiler/pw-conformance/tests/wit_names.rs)"; echo; \
       cargo test --locked -p pw-conformance --test wit_names 2>&1 | grep -E '^(test |test result)'; \
       echo; echo "== mutation controls (scripts/wit_name_mutations.py)"; echo; \
       python3 scripts/wit_name_mutations.py; \
       echo; \
       echo "NOT CLAIMED: a host interface or operation named by a keyword. Its name"; \
       echo "is the author's WIT, written verbatim from its host clause."; \
     } > docs/evidence/E10/wit-names.txt
    @grep -E "^test result|mutants killed" docs/evidence/E10/wit-names.txt

# ADR-0059: declared sum types. The checker's typing of a case where it is
# written, each case built and matched through the E8 host against a Rust
# model, the component against the JavaScript module, and the mutation
# controls. Needs `node`.
e10-sum-types:
    @{ echo "ADR-0059 - declared sum types"; echo; \
       echo "produced by: just e10-sum-types"; \
       echo "commit: $(git rev-parse HEAD)$(git diff --quiet HEAD -- . ':(exclude)docs/evidence' || echo ' + uncommitted changes')"; \
       echo "rust: $(rustc --version)"; echo "node: $(node --version)"; echo; \
       echo "== the checker (compiler/pw-core/tests/sum_types.rs)"; echo; \
       cargo test --locked -p pw-core --test sum_types 2>&1 | grep -E '^(test |test result)'; \
       echo; echo "== through the E8 host, against a model (compiler/pw-conformance/tests/sum_types.rs)"; echo; \
       cargo test --locked -p pw-conformance --test sum_types 2>&1 | grep -E '^(test |test result)'; \
       echo; echo "== the component and the JavaScript module"; echo; \
       cargo test --locked -p pw-conformance --test javascript -- --nocapture --test-threads=1 2>&1 \
         | grep -oE "javascript: [0-9]+ queries.*|^test result.*"; \
       echo; echo "== mutation controls (scripts/sum_type_mutations.py)"; echo; \
       python3 scripts/sum_type_mutations.py; \
       echo; \
       echo "NOT CLAIMED: a type that contains itself in the backend, refused by"; \
       echo "name (a generic sum type is ADR-0062's, a nested or literal pattern"; \
       echo "ADR-0060's, a sum type in a template's {#match} ADR-0061's). NOT CLAIMED:"; \
       echo "a captured sum type in a handler; == between two cases."; \
     } > docs/evidence/E10/sum-types.txt
    @grep -E "^test result|^javascript:|mutants killed" docs/evidence/E10/sum-types.txt

# ADR-0057: maps and sets. Each operation against BTreeMap and BTreeSet, the
# checks on what arrives from outside, the component against the JavaScript
# module, and the mutation controls. Needs `node`.
e10-maps:
    @{ echo "ADR-0057 - maps and sets"; echo; \
       echo "produced by: just e10-maps"; \
       echo "commit: $(git rev-parse HEAD)$(git diff --quiet HEAD -- . ':(exclude)docs/evidence' || echo ' + uncommitted changes')"; \
       echo "rust: $(rustc --version)"; echo "node: $(node --version)"; echo; \
       echo "== against BTreeMap and BTreeSet (compiler/pw-conformance/tests/maps.rs)"; echo; \
       cargo test --locked -p pw-conformance --test maps 2>&1 | grep -E '^(test |test result)'; \
       echo; echo "== the component and the JavaScript module"; echo; \
       cargo test --locked -p pw-conformance --test javascript -- --nocapture --test-threads=1 2>&1 \
         | grep -oE "javascript: [0-9]+ queries.*|test the_modules_entry_check.*|^test result.*"; \
       echo; echo "== mutation controls (scripts/map_mutations.py)"; echo; \
       python3 scripts/map_mutations.py; \
       echo; \
       echo "NOT CLAIMED: a key of a type other than Int or String, which is refused"; \
       echo "by name. NOT CLAIMED: a map or set inside a parameter's value or a"; \
       echo "host's answer, which is refused because nothing would check it; or a"; \
       echo "for loop over a map or set, which reads it through keys or to_list."; \
     } > docs/evidence/E10/maps.txt
    @grep -E "^test result|^javascript:|mutants killed" docs/evidence/E10/maps.txt

# ADR-0056: Unicode case mapping. The tables against Rust for every code
# point, the component and the module against Rust and each other, and the
# mutation controls. Needs `node`.
e10-case:
    @{ echo "ADR-0056 - Unicode case mapping"; echo; \
       echo "produced by: just e10-case"; \
       echo "commit: $(git rev-parse HEAD)$(git diff --quiet HEAD -- . ':(exclude)docs/evidence' || echo ' + uncommitted changes')"; \
       echo "rust: $(rustc --version)"; echo "node: $(node --version)"; echo; \
       echo "== the tables (compiler/pw-core/tests/case_mapping.rs)"; echo; \
       cargo test --locked -p pw-core --test case_mapping 2>&1 | grep -E '^(test |test result)'; \
       echo; echo "== the component through the host (pw-conformance/tests/case_mapping.rs)"; echo; \
       cargo test --locked -p pw-conformance --test case_mapping 2>&1 | grep -E '^(test |test result)'; \
       echo; echo "== the module under Node, and the component against it"; echo; \
       cargo test --locked -p pw-conformance --test javascript -- --nocapture --test-threads=1 2>&1 \
         | grep -oE "javascript: [0-9]+ queries.*|test the_modules_case_mapping.*|^test result.*"; \
       echo; echo "== mutation controls (scripts/case_mutations.py)"; echo; \
       python3 scripts/case_mutations.py; \
       echo; \
       echo "NOT CLAIMED: Unicode's Final_Sigma rule. A word-final capital sigma"; \
       echo "lowers to sigma, not final sigma. NOT CLAIMED: case folding, or a"; \
       echo "locale's rules."; \
     } > docs/evidence/E10/case.txt
    @grep -E "^test result|^javascript:|mutants killed" docs/evidence/E10/case.txt

# ADR-0055: slicing, and the standard library's placeholders computed. The
# operations against Vec and str, the component against the JavaScript
# module, and the mutation controls.
e10-slices:
    @{ echo "ADR-0055 - slicing, and the standard library's placeholders computed"; echo; \
       echo "produced by: just e10-slices"; \
       echo "commit: $(git rev-parse HEAD)$(git diff --quiet HEAD -- . ':(exclude)docs/evidence' || echo ' + uncommitted changes')"; \
       echo "rust: $(rustc --version)"; echo "node: $(node --version)"; echo; \
       echo "== against Vec and str (compiler/pw-conformance/tests/stdlib.rs)"; echo; \
       cargo test --locked -p pw-conformance --test stdlib 2>&1 | grep -E '^(test |test result)'; \
       echo; echo "== the component and the JavaScript module, on the same arguments"; echo; \
       cargo test --locked -p pw-conformance --test javascript -- --nocapture --test-threads=1 2>&1 \
         | grep -oE "javascript: [0-9]+ queries.*"; \
       echo; echo "== mutation controls (scripts/slice_mutations.py)"; echo; \
       python3 scripts/slice_mutations.py; \
       echo; \
       echo "NOT CLAIMED: String.split, List.range, or an Int sum. Maps and sets"; \
       echo "are not claimed here, nor Unicode case mapping (ADR-0056)."; \
     } > docs/evidence/E10/slices.txt
    @grep -E "^test result|^javascript:|mutants killed" docs/evidence/E10/slices.txt

# ADR-0054: an opaque value is built and read inside a component. The opaque
# values through the host, the component against the JavaScript module, the
# member rule over a generic representation, and the mutation controls.
e10-opaque:
    @{ echo "ADR-0054 - an opaque value is built and read inside a component"; echo; \
       echo "produced by: just e10-opaque"; \
       echo "commit: $(git rev-parse HEAD)$(git diff --quiet HEAD -- . ':(exclude)docs/evidence' || echo ' + uncommitted changes')"; \
       echo "rust: $(rustc --version)"; echo "node: $(node --version)"; echo; \
       echo "== opaque values through the host (pw-conformance/tests/opaque.rs)"; echo; \
       cargo test --locked -p pw-conformance --test opaque 2>&1 | grep -E '^(test |test result)'; \
       echo; echo "== the component and the JavaScript module, on the same arguments"; echo; \
       cargo test --locked -p pw-conformance --test javascript -- --nocapture --test-threads=1 2>&1 \
         | grep -oE "javascript: [0-9]+ queries.*"; \
       echo; echo "== a generic representation's .value (pw-core/tests/members.rs)"; echo; \
       cargo test --locked -p pw-core --test members 2>&1 | grep -E '^(test |test result)'; \
       echo; echo "== mutation controls (scripts/opaque_mutations.py)"; echo; \
       python3 scripts/opaque_mutations.py; \
       echo; \
       echo "NOT CLAIMED: a generic opaque type inside a component, which is refused"; \
       echo "as a generic record is. NOT CLAIMED: an opaque type's invariant; none is"; \
       echo "stated."; \
     } > docs/evidence/E10/opaque.txt
    @grep -E "^test result|^javascript:|mutants killed" docs/evidence/E10/opaque.txt

# ADR-0053: a lambda's parameters take the types its use declares. The
# callback tests, the effect chain through a callback, what must stay clean,
# and the mutation controls.
e10-callbacks:
    @cargo build --quiet --locked -p pw-cli
    @{ echo "ADR-0053 - a lambda's parameters take the types its use declares"; echo; \
       echo "produced by: just e10-callbacks"; \
       echo "commit: $(git rev-parse HEAD)$(git diff --quiet HEAD -- . ':(exclude)docs/evidence' || echo ' + uncommitted changes')"; \
       echo "rust: $(rustc --version)"; echo; \
       echo "== the callback tests (compiler/pw-core/tests/callback_parameters.rs)"; echo; \
       cargo test --locked -p pw-core --test callback_parameters 2>&1 | grep -E '^(test |test result)'; \
       echo; echo "== an effect through a callback (compiler/pw-core/tests/causal_evidence.rs)"; echo; \
       cargo test --locked -p pw-core --test causal_evidence 2>&1 | grep -E '^(test |test result)'; \
       echo; echo "== what must stay clean: errors reported"; echo; \
       printf 'accepted corpus: %s\n' "$(./target/debug/pw check packages/pw-std/*.pw packages/pw-platform-web/*.pw examples/domain.pw examples/lib/*.pw examples/accepted/*.pw 2>&1 | sed 's/\x1b\[[0-9;]*m//g' | grep -cE '^error' || true)"; \
       printf 'store: %s\n' "$(./target/debug/pw check packages/pw-std/*.pw packages/pw-platform-web/*.pw examples/domain.pw examples/lib/*.pw examples/store/*.pw 2>&1 | sed 's/\x1b\[[0-9;]*m//g' | grep -cE '^error' || true)"; \
       printf 'kiokun: %s\n' "$(./target/debug/pw check packages/pw-std/*.pw packages/pw-platform-web/*.pw examples/kiokun/*.pw 2>&1 | sed 's/\x1b\[[0-9;]*m//g' | grep -cE '^error' || true)"; \
       echo; echo "== mutation controls (scripts/callback_mutations.py)"; echo; \
       python3 scripts/callback_mutations.py; \
       echo; \
       echo "NOT CLAIMED: a parameter whose name is bound at two sites in one body."; \
       echo "The value relations' environment is flat, and such a name is unknown."; \
       echo "NOT CLAIMED: a lambda in a branch of an annotated binding's initialiser,"; \
       echo "typed by the annotation. A returned value is seen through its branches;"; \
       echo "an initialiser is typed only when it is the lambda itself."; \
     } > docs/evidence/E10/callbacks.txt
    @grep -E "^test result|^(accepted corpus|store|kiokun):|mutants killed" docs/evidence/E10/callbacks.txt

# ADR-0051: an early `return`, `?`, and `for` loops compile, and the checker
# says what may be assigned. The compiled bodies through the host, the
# component against the JavaScript module, and the mutation controls.
e10-control-flow:
    @{ echo "ADR-0051 - an early return, ?, and for loops compile"; echo; \
       echo "produced by: just e10-control-flow"; \
       echo "commit: $(git rev-parse HEAD)$(git diff --quiet HEAD -- . ':(exclude)docs/evidence' || echo ' + uncommitted changes')"; \
       echo "rust: $(rustc --version)"; echo "node: $(node --version)"; echo; \
       echo "== compiled bodies through the host (pw-conformance/tests/control_flow.rs)"; echo; \
       cargo test --locked -p pw-conformance --test control_flow 2>&1 | grep -E '^(test |test result)'; \
       echo; echo "== what may be assigned (compiler/pw-core/tests/assignments.rs)"; echo; \
       cargo test --locked -p pw-core --test assignments 2>&1 | grep -E '^(test |test result)'; \
       echo; echo "== the component and the JavaScript module, on the same arguments"; echo; \
       cargo test --locked -p pw-conformance --test javascript -- --nocapture --test-threads=1 2>&1 \
         | grep -oE "javascript: [0-9]+ queries.*"; \
       echo; echo "== mutation controls (scripts/control_flow_mutations.py)"; echo; \
       python3 scripts/control_flow_mutations.py; \
       echo; \
       echo "NOT CLAIMED: a return or assignment inside a lambda a list operation"; \
       echo "runs, or an assignment to a field. A function value is ADR-0052's:"; \
       echo "just e10-function-values."; \
     } > docs/evidence/E10/control-flow.txt
    @grep -E "^javascript:|mutants killed" docs/evidence/E10/control-flow.txt

# ADR-0050: recursion and generic callees compile. The compiled recursions
# through the host, the component against the JavaScript module, and the
# mutation controls.
e10-recursion:
    @{ echo "ADR-0050 - recursion and generic callees compile"; echo; \
       echo "produced by: just e10-recursion"; \
       echo "commit: $(git rev-parse HEAD)$(git diff --quiet HEAD -- . ':(exclude)docs/evidence' || echo ' + uncommitted changes')"; \
       echo "rust: $(rustc --version)"; echo "node: $(node --version)"; echo; \
       echo "== recursions through the host (pw-conformance/tests/recursion.rs)"; echo; \
       cargo test --locked -p pw-conformance --test recursion 2>&1 | grep -E '^(test |test result)'; \
       echo; echo "== the component and the JavaScript module, on the same arguments"; echo; \
       cargo test --locked -p pw-conformance --test javascript -- --nocapture --test-threads=1 2>&1 \
         | grep -oE "javascript: [0-9]+ queries.*"; \
       echo; echo "== mutation controls (scripts/recursion_mutations.py)"; echo; \
       python3 scripts/recursion_mutations.py; \
       echo; \
       echo "NOT CLAIMED: a generic record. An early return and a loop are ADR-0051's"; \
       echo "(just e10-control-flow), a function value ADR-0052's (just e10-function-values)."; \
     } > docs/evidence/E10/recursion.txt
    @grep -E "^javascript:|mutants killed" docs/evidence/E10/recursion.txt

# ADR-0049: a string's escapes are the language's. The decoder's tests, the
# checker's, the compiled values, and the mutation controls.
e10-strings:
    @{ echo "ADR-0049 - a string's escapes are the language's"; echo; \
       echo "produced by: just e10-strings"; \
       echo "commit: $(git rev-parse HEAD)$(git diff --quiet HEAD -- . ':(exclude)docs/evidence' || echo ' + uncommitted changes')"; \
       echo "rust: $(rustc --version)"; echo; \
       echo "== the decoder (compiler/pw-syntax/src/strings.rs)"; echo; \
       cargo test --locked -p pw-syntax --lib strings 2>&1 | grep -E '^test result'; \
       echo; echo "== the checker, Koka and Marko (compiler/pw-core/tests/string_escapes.rs)"; echo; \
       cargo test --locked -p pw-core --test string_escapes 2>&1 | grep -E '^test result'; \
       echo; echo "== compiled components through the host (pw-conformance/tests/strings.rs)"; echo; \
       cargo test --locked -p pw-conformance --test strings 2>&1 | grep -E '^test result'; \
       echo; echo "== the component and the JavaScript module, on the same arguments"; echo; \
       cargo test --locked -p pw-conformance --test javascript -- --nocapture --test-threads=1 2>&1 \
         | grep -oE "javascript: [0-9]+ queries.*"; \
       echo; echo "== mutation controls (scripts/string_mutations.py)"; echo; \
       python3 scripts/string_mutations.py; \
       echo; \
       echo "NOT CLAIMED: a string inside a hole, or decoded policy strings."; \
     } > docs/evidence/E10/strings.txt
    @grep -E "^javascript:|mutants killed" docs/evidence/E10/strings.txt

# ADR-0048: a read names a member its value's type has (PW0610). The member
# tests, the corpus that must stay clean, and the mutation controls.
e10-members:
    @cargo build --quiet --locked -p pw-cli
    @{ echo "ADR-0048 - a read names a member its value's type has"; echo; \
       echo "produced by: just e10-members"; \
       echo "commit: $(git rev-parse HEAD)$(git diff --quiet HEAD -- . ':(exclude)docs/evidence' || echo ' + uncommitted changes')"; \
       echo "rust: $(rustc --version)"; echo; \
       echo "== the member tests (compiler/pw-core/tests/members.rs)"; echo; \
       cargo test --locked -p pw-core --test members 2>&1 | grep -E '^test result'; \
       echo; echo "== what must stay clean: errors reported"; echo; \
       printf 'accepted corpus: %s\n' "$(./target/debug/pw check packages/pw-std/*.pw packages/pw-platform-web/*.pw examples/domain.pw examples/lib/*.pw examples/accepted/*.pw 2>&1 | sed 's/\x1b\[[0-9;]*m//g' | grep -cE '^error' || true)"; \
       printf 'store: %s\n' "$(./target/debug/pw check packages/pw-std/*.pw packages/pw-platform-web/*.pw examples/domain.pw examples/lib/*.pw examples/store/*.pw 2>&1 | sed 's/\x1b\[[0-9;]*m//g' | grep -cE '^error' || true)"; \
       printf 'kiokun: %s\n' "$(./target/debug/pw check packages/pw-std/*.pw packages/pw-platform-web/*.pw examples/kiokun/*.pw 2>&1 | sed 's/\x1b\[[0-9;]*m//g' | grep -cE '^error' || true)"; \
       echo; echo "== what the audit decided (member relations, the accepted corpus)"; echo; \
       ./target/debug/pw audit-values packages/pw-std/*.pw packages/pw-platform-web/*.pw examples/domain.pw examples/lib/*.pw examples/accepted/*.pw 2>&1 | grep -E '^  Member' || true; \
       echo; echo "== mutation controls (scripts/member_mutations.py)"; echo; \
       python3 scripts/member_mutations.py; \
       echo; \
       echo "NOT CLAIMED here: an opaque value built or read inside a component. It"; \
       echo "compiles since 2026-09-25 (ADR-0054): just e10-opaque."; \
     } > docs/evidence/E10/members.txt
    @grep -E "^(accepted corpus|store|kiokun):|mutants killed" docs/evidence/E10/members.txt

# ADR-0047: every name resolves, in lexical scope (PW0021). The name tests, the
# corpus that must stay clean, and the mutation controls.
e10-names:
    @cargo build --quiet --locked -p pw-cli
    @{ echo "ADR-0047 - every name resolves, in lexical scope"; echo; \
       echo "produced by: just e10-names"; \
       echo "commit: $(git rev-parse HEAD)$(git diff --quiet HEAD -- . ':(exclude)docs/evidence' || echo ' + uncommitted changes')"; \
       echo "rust: $(rustc --version)"; echo; \
       echo "== the name tests (compiler/pw-core/tests/every_name_resolves.rs)"; echo; \
       cargo test --locked -p pw-core --test every_name_resolves 2>&1 | grep -E '^test result'; \
       echo; echo "== what must stay clean: errors reported"; echo; \
       printf 'accepted corpus: %s\n' "$(./target/debug/pw check packages/pw-std/*.pw packages/pw-platform-web/*.pw examples/domain.pw examples/lib/*.pw examples/accepted/*.pw 2>&1 | sed 's/\x1b\[[0-9;]*m//g' | grep -cE '^error' || true)"; \
       printf 'store: %s\n' "$(./target/debug/pw check packages/pw-std/*.pw packages/pw-platform-web/*.pw examples/domain.pw examples/lib/*.pw examples/store/*.pw 2>&1 | sed 's/\x1b\[[0-9;]*m//g' | grep -cE '^error' || true)"; \
       printf 'kiokun: %s\n' "$(./target/debug/pw check packages/pw-std/*.pw packages/pw-platform-web/*.pw examples/kiokun/*.pw 2>&1 | sed 's/\x1b\[[0-9;]*m//g' | grep -cE '^error' || true)"; \
       echo; echo "== mutation controls (scripts/names_mutations.py)"; echo; \
       python3 scripts/names_mutations.py; \
       echo; \
       echo "NOT CLAIMED: that derived is pure, or that a clause's words are in its"; \
       echo "domain. Both are left to the analyses that read them."; \
     } > docs/evidence/E10/names.txt
    @grep -E "^(accepted corpus|store|kiokun):|mutants killed" docs/evidence/E10/names.txt

# ADR-0044: pure computation compiled to JavaScript modules, held to its Wasm
# component under Node, with the mutation controls. Needs `node`.
#
# E10 task 2 — "generated modern JavaScript modules for ... pure computation".
e10-javascript:
    @{ echo "ADR-0044 - pure computation as JavaScript modules, against the components"; echo; \
       echo "produced by: just e10-javascript"; \
       echo "commit: $(git rev-parse HEAD)$(git diff --quiet HEAD -- . ':(exclude)docs/evidence' || echo ' + uncommitted changes')"; \
       echo "rust: $(rustc --version)"; echo "node: $(node --version)"; echo; \
       echo "== the component and the module, on the same arguments"; echo; \
       cargo test --locked -p pw-conformance --test javascript -- --nocapture --test-threads=1 2>&1 \
         | grep -oE "javascript: .*|^test result.*"; \
       echo; echo "== mutation controls (scripts/javascript_mutations.py)"; echo; \
       python3 scripts/javascript_mutations.py; \
       echo; \
       echo "NOT CLAIMED: a handler that computes, Wasm in the browser, or the modules"; \
       echo "in a browser engine. They run under Node."; \
     } > docs/evidence/E10/javascript.txt
    @grep -E "^javascript:|mutants killed" docs/evidence/E10/javascript.txt

# ADR-0043: an operator's operands and an `if`'s condition typed (PW0609), and
# `Float.from_int`, with the mutation controls.
#
# E10 — the checker gap KNOWN_LIMITATIONS named: `1 == "a"` checked.
e10-operands:
    @{ echo "ADR-0043 - operands are typed, and Float.from_int"; echo; \
       echo "produced by: just e10-operands"; \
       echo "commit: $(git rev-parse HEAD)$(git diff --quiet HEAD -- . ':(exclude)docs/evidence' || echo ' + uncommitted changes')"; \
       echo "rust: $(rustc --version)"; echo; \
       echo "== the value relations (compiler/pw-core/tests/value_relations.rs)"; echo; \
       cargo test --locked -p pw-core --test value_relations -- --test-threads=1 2>&1 \
         | grep -E '^test (an_operator|an_untyped|the_accepted)|^test result'; \
       echo; echo "== Float.from_int (compiler/pw-conformance/tests/stdlib.rs)"; echo; \
       cargo test --locked -p pw-conformance --test stdlib an_int_becomes -- --test-threads=1 2>&1 \
         | grep -E '^(test |test result)'; \
       echo; echo "== mutation controls (scripts/operand_mutations.py)"; echo; \
       python3 scripts/operand_mutations.py; \
       echo; \
       echo "NOT CLAIMED: an operand the checker cannot type. It is undecided, and"; \
       echo "the component backend refuses what remains."; \
     } > docs/evidence/E10/operands.txt
    @grep -E "^test result|mutants killed" docs/evidence/E10/operands.txt

# ADR-0042's template branches and attributes: the checker's refusals and the
# IR (pw-core), what renders (pw-render), and the mutation controls.
#
# E10 — `{:else}`, `{#match}` and interpolated attributes.
e10-templates:
    @{ echo "ADR-0042 - template branches and interpolated attributes"; echo; \
       echo "produced by: just e10-templates"; \
       echo "commit: $(git rev-parse HEAD)$(git diff --quiet HEAD -- . ':(exclude)docs/evidence' || echo ' + uncommitted changes')"; \
       echo "rust: $(rustc --version)"; echo; \
       echo "== the checker and the IR (compiler/pw-core/tests/template_blocks.rs)"; echo; \
       cargo test --locked -p pw-core --test template_blocks -- --test-threads=1 2>&1 \
         | grep -E '^(test |test result)'; \
       echo; echo "== the renderer (runtime/pw-render/tests/branches.rs)"; echo; \
       cargo test --locked -p pw-render --test branches -- --test-threads=1 2>&1 \
         | grep -E '^(test |test result)'; \
       echo; echo "== mutation controls (scripts/template_mutations.py)"; echo; \
       python3 scripts/template_mutations.py; \
       echo; \
       echo "NOT CLAIMED: patches for a match region or an interpolated attribute."; \
       echo "They render on the server; no patch generator emits them yet."; \
     } > docs/evidence/E10/templates.txt
    @grep -E "^test result|mutants killed" docs/evidence/E10/templates.txt

# ADR-0041's mutation controls: each piece of kiokun's logic in Pleris, and of
# the defects found building it, undone in turn, must fail a test.
#
# E10 — the kiokun slice's shard rule and ranking, in Pleris.
e10-kiokun-mutants:
    @{ echo "ADR-0041 - kiokun's shard rule and ranking in Pleris: mutation controls"; echo; \
       echo "produced by: just e10-kiokun-mutants"; \
       echo "commit: $(git rev-parse HEAD)$(git diff --quiet HEAD -- . ':(exclude)docs/evidence' || echo ' + uncommitted changes')"; \
       echo "rust: $(rustc --version)"; echo; \
       python3 scripts/kiokun_mutations.py; \
       echo; \
       echo "NOT CLAIMED: search over every shard. Search is over the loaded shard;"; \
       echo "a lookup reaches every shard."; \
     } > docs/evidence/E10/kiokun-mutants.txt
    @grep -E "mutants killed" docs/evidence/E10/kiokun-mutants.txt

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
       echo "NOT CLAIMED: the event as a handler parameter. No syntax binds one, the"; \
       echo "runtime listens for a click only, and a handler with parameters is refused."; \
       echo "Computation, branches, loops and several commands compile since"; \
       echo "2026-09-25 (ADR-0058): just e10-handlers-compute."; \
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
ci: evidence-gates fmt-check lint case-check mutation-anchors test-unit test-compile
    @echo ""
    @echo "ci: OK"
