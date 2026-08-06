# Risk-retirement queue

Experiments that test a load-bearing assumption **early**, using temporary
dependencies if that is what it takes.

> Passing an `RQ` retires a risk. It does **not** close the corresponding
> engineering milestone. `docs/MILESTONES.md` holds that distinction.

Each entry records its **decision rule before it runs**. That is not ceremony:
three separate measurements in this project produced plausible, favourable
numbers that turned out to be measuring nothing (see "the recurring bug" below).

Order was fixed by the project architect after reviewing E0's evidence.

| id | question | status | ruling |
|---|---|---|---|
| **RQ-1** | Is Marko's resumption real, in Chrome **and** Safari? | **done** | 11/12 in both engines → oracle for **E7-R** (resumption, DOM preservation) and partially **E7-P**. **Not** the oracle for **E7-L**: check 4 failed. |
| **RQ-2** | Does Koka propagate effects through higher-order abstraction? | **done** | Outcome 1, clean pass → Koka remains the effects oracle; `pw` effect checker not pulled forward |
| **RQ-3** | `pw`-owned exhaustiveness and canonical typed ABI decoding | **partial** | exhaustiveness + type-directed ABI land in `compiler/pw-core`; 30 tests. **Exhaustiveness now runs on `.pw` source** and rejects R-007 with named witnesses. Remaining: the ABI half has no source path |
| **RQ-4** | Minimal static task-scope checker (E2A-S) + runtime structured concurrency (E2A-R) | **done** | Both halves. E2A-S: `compiler/pw-core/src/scope.rs`, PW2001-PW2004, fed from real `.pw` bodies — R-013 and R-039 are compile failures. E2A-R: `runtime/pw-tasks` (ADR-0016), 12 behaviour tests, 0 failures in 25 consecutive runs |
| **RQ-5** | Final-artifact declared-vs-actual component import verification | **partial** | rule + `PW4007` in `compiler/pw-core/src/capability.rs`, tested against E0's real 15-vs-1 import lists. Remaining: call it from the build, not just from tests |
| **RQ-6** | Backfill direct / helper-hidden / generic-callback rejection cases for **every** effect family | **partial** | layout/DOM families done: 10 accepted + 13 rejected, including both hidden cases. Remaining: backfill the other families |
| **RQ-7** | Generate and adopt the E→P claim/evidence table before publishing P0 | **done** (first draft) | `docs/EVIDENCE_LEDGER.md` |

---

## RQ-1 — Marko resumption (done)

**Pre-registered rule.** All core properties pass in both engines → behavioural
oracle for E7. Passes Chrome, fails Safari → architectural donor only. Component
replay, eager handler, DOM replacement, or lost browser state → stop calling
Marko resumable for our purposes. Only payload scaling passes → report payload
scaling and nothing more.

**Result.** Core properties (no replay, first click works, node identity, focus,
second interaction) passed in **Chrome 150 and Safari 26.5.2**.

**Check 4 failed in both engines** — the interaction artifact loads eagerly at
40–55 ms. That was a *pre-registered property*, so it is not a footnote: it
falsifies charter §1.11's "interaction-lazy code" for this configuration, and
E7 is subdivided accordingly (`docs/MILESTONES.md`).

> Marko is the behavioural oracle for resumption, DOM preservation and
> patch-placement semantics. **It is not the oracle for interaction-lazy code
> delivery.**

Safari's autorun path is acceptable for what it can observe and **must not** be
described as equivalent to the Chrome stream-timing harness.

Evidence: `docs/evidence/E0/spike-browser-resumption.txt` ·
`spikes/browser-resumption/README.md`

## RQ-2 — Koka higher-order effects (done)

**Pre-registered rule.** Koka may overstate an effect but must never silently
understate or erase one. Four enumerated outcomes with a fixed decision table.

**Result.** Outcome 1 — automatic inference of effect polymorphism on
*unannotated* generic helpers, propagation through two layers, correct selective
discharge, and no capability hiding. E1 clean pass on effects.

Evidence: `docs/evidence/E0/spike-koka-row-polymorphism.txt` ·
`spikes/koka-row-polymorphism/README.md`

## RQ-3 — `pw` exhaustiveness and typed ABI (partial)

Delivers the core of E1A. Pre-registered acceptance, with what is done marked:

- **done** — an incomplete match is rejected **regardless of the function's
  effect row**, the case Koka provably does not catch
  (`tests/differential_vs_koka.rs`);
- **done** — `Option<T>` and `List<T>` cannot be confused at the boundary, even
  though both collapse to `null` in Koka's JS output;
- **done** — a raw primitive cannot enter a nominal position undetected: debug
  mode catches a `Money<EUR>` payload arriving in a `Money<USD>` slot;
- **done** — decoding is type-directed: `decode(path, ExpectedType, payload)`,
  and there is deliberately no `decode_unknown(payload)` entry point;
- **done** — rejections carry a stable `PW1004` code, a primary span and an
  origin span.

**Open:** there is no parser, so the checker runs on a constructed pattern
matrix rather than on `.pw` files. The original acceptance line — *"fails if any
accepted-corpus program is rejected, or any rejected-corpus program compiles"* —
cannot be evaluated until **E2** puts a front end in front of it. RQ-3 is
therefore `partial`, not `done`.

## RQ-4 — structured concurrency

Split, because a runtime task tree cannot make a compile-fail fixture fail at
compile time:

- **E2A-R (runtime):** owner scopes, cancellation propagation, cleanup, leak
  detection, results not committing into dead scopes. These are *behaviour*
  tests and must never be presented as static guarantees.
- **E2A-S (static):** `task.spawn` owned by the lexical scope; handles that
  cannot be returned, stored, or captured into a longer-lived scope; no
  detachment without a durable capability; effects propagating through helpers.

**E2A closes only when both halves pass.** Koka representing `task.spawn` in an
effect row would be useful but insufficient — it does not prove scope
non-escape, and `pw` owns that property.

**E2A-S status: implemented** in `compiler/pw-core/src/scope.rs`. A scope tree
plus handle-lifetime rules, with four diagnostics:

| code | rule |
|---|---|
| `PW2001` | a handle escapes into a scope that outlives its owner |
| `PW2002` | an ordinary task is detached without the `durable_job.enqueue` capability |
| `PW2003` | a handle is used outside its owning scope — the late-result-into-dead-component case |
| `PW2004` | a subscription declares a scope longer than the component that created it |

Each carries an **origin span** as well as a violation span, per charter §16.3.
Escaping into a *descendant* scope is allowed, because a descendant dies no
later than the handle does. `durable.spawn` is the legal escape hatch and is
exempt by capability, not by convention.

**E2A-R status: done** — `runtime/pw-tasks`, ADR-0016. A thread-scoped runtime
built on `std::thread::scope`, adding cancellation propagation, ordered cleanup,
and the dead-scope commit rule. Twelve behaviour tests, each mapped to one of
RQ-4's five properties in `docs/evidence/E2A/e2a-r-runtime.txt`, with **0
failures across 25 consecutive runs** — a concurrency suite that passes once has
measured almost nothing.

The separation still holds in both directions: these are **behaviour** results
and do not make any misuse a compile error, and E2A-S's static rules do not
cover runtime cancellation. Two cases genuinely need both — a task that finishes
*after* its scope was torn down has no escaping handle for the static checker to
see, and `Scope::commit` refusing it is the only thing that catches it.

Deliberately not proven: anything async. E8 chooses the host execution model and
these semantics must be re-proved there.

## RQ-5 — artifact import verification (partial)

Every build emits declared capabilities, actual component imports, and the
difference. Any undeclared import fails the build (`PW4007`).

**Implemented** in `compiler/pw-core/src/capability.rs`, deliberately free of any
Wasm dependency so the rule is unit-testable without a runtime. Its tests use the
**real import lists E0 measured** — 1 for the `no_std` guest, 15 for the `std`
guest — rather than invented fixtures.

Two runtime profiles, because charter §14 M8 task 4 requires denying ambient
authority by default while still allowing a richer runtime *explicitly*:

| profile | allowance |
|---|---|
| `minimal` | declared interfaces only; `no_std`-equivalent |
| `wasi-cli` | additionally tolerates `wasi:cli/`, `clocks/`, `io/`, `random/`, `filesystem/` |

Choosing `wasi-cli` does not hide the surface: the extra imports are still
listed under `allowed_by_profile`. And an import outside the allowance —
`wasi:sockets/tcp` — still fails.

The `PW4007` text carries the architect's terminology correction, because it is
the difference between a true and a false claim: these imports are a *requested
authority surface*, not authority already held. The host still decides what to
link.

**Open:** the audit is called from tests, not from a build step. Wiring it into
`spikes/wasmtime-component/host` (which already computes the actual list) and
then into the E8 build is the remaining work.

## Corpus harness rules

Architect ruling: rejection is insufficient; a program must be rejected **for
the specified invariant**.

```text
accepted program
  - parses
  - produces no error diagnostics

rejected program
  - parses
  - produces its DECLARED diagnostic code (aliases resolved to canonical)
  - produces it at the declared or a compatible span
  - is not counted as passing merely because another rule rejected it
```

**Accepted programs are checked before rejected ones**, so a false positive
stops the suite immediately. A rule that fires on a program the corpus calls
correct is worse than one that misses a violation.

Multi-defect fixtures must list every expected code explicitly. Otherwise each
rejected fixture should isolate one primary defect.

## RQ-6 — effect-family rejection coverage

For **every** effect family, three cases are required before the family counts as
implemented:

1. one accepted use;
2. one **direct** rejected use;
3. one rejected use **hidden behind a helper or generic callback**.

The third is the one that matters. An effect checker that catches
`element.offsetWidth` in a component body and misses it two calls deep inside
`measure_tooltip` is worth approximately nothing, and it is exactly the failure
that survives a green test suite. RQ-2 showed Koka handles this for its own
effects; the `pw` checker must be shown to do the same.

---

## The recurring bug

Twenty-one measurements in this project produced plausible, favourable results
while measuring nothing. The count is kept accurate deliberately: it is the
argument for the admissibility rule above.

| where | what it reported | what was actually happening |
|---|---|---|
| layout spike F-2 | 79.7 ms → 0.3 ms, looking like a fix | the write stopped invalidating layout, so no work was done |
| RQ-1 F-1 | 72 "component replacements" | the browser's own initial HTML parse |
| RQ-1 F-4 | "silent failure, no error signal" | resource-load errors do not bubble; only a capture-phase listener sees them |
| RQ-2 F-2 | a generic helper reported as **pure** | an effect-polymorphic row variable fell through to the "total" branch |
| E2 | `pw check` green on 68 files | 13 of them were malformed and no tool had ever read their bodies |
| E2 grammar | 56/68 files parsed; the other 12 were called "exotic forms" | a missing closer was consumed by a bare `eat` whose failure was discarded, so five files' lambdas never parsed at all. Making it an error found four real defects underneath |
| E2 grammar | `a >= b` parsed without complaint | it lexed as `>` then `=` and parsed as `a > (= b)` |
| E2 grammar | an error on `</main>` | the real fault was two lines earlier: `expr` crossed a newline into `<` and parsed markup as a comparison |
| E2 grammar | a statement parsed, the next one failed | a modifier loop crossed the newline and ate the next statement's first token |
| E3 adapter | the generated template looked correct | **Marko strips whitespace between elements**, so `<span>a</span> <span>b</span>` rendered as `ab`. The template-level test asserted the template, and the template was fine — only a browser saw the DOM |
| E5 placement | corpus coverage rose from 7 to 9 files | an effect family absent from the capability table was granted by **no** world, so two files were reported as unplaceable for a reason that had nothing to do with their actual defect. Right file, wrong rule, and the number went up while nothing was detected |
| E5 markup | corpus coverage read 20 files | `<form>` was missing from the list of interactive elements, so `<form on:submit={..}>` — the normal way to submit a form — was reported as an accessibility defect. It made R-022, a handler *type* mismatch, look caught. The real number was 19 |
| E2D | `R-012` was caught, and its code matched | it was caught for a **second** defect — its `setup` declared neither `dom.mutate` nor `layout.measure` — which masked the affine-resource leak the fixture was written for. A fixture that fails for the wrong reason is not enforced, so the fixture was repaired rather than the count banked |
| E2D | corpus coverage rose from 25 to 27 | giving an effect's type argument its place in the effect's *identity* was correct, but `covered()` split the family on `.` before stripping arguments — and `secret<Payments>` has no dot. Two fixtures were reported for an effect their own row declared. The 27 was worse than the 25 |
| registry | `PW0323` looked owned and consistent | it meant three things at once: the registry called it "a resource must declare how it is released" (nothing enforced that), `rules.rs` emitted it for a placement failure, and the corpus fixture declaring it is about placement. Had the rule and the fixture ever met, the ratchet would have counted a correct catch under a description of a different rule — and `declared_code == errored` **cannot see that**, because the code genuinely matches |
| registry | four gap entries read as open work | `PW3001`, `PW3002`, `PW3004` and `PW3008` were aliased onto `PW0401`/`PW0402` when the layout family landed, so nothing could ever resolve to the gap entries. They described work that was already done as unowned |
| E9C fuzzer | 69 covered regions for the whole compiler | the profile parser looked for `Function name: `, which `llvm-profdata` does not emit, so it matched nothing in the dependencies and measured only the harness. The real figure is 1,200-2,300 per target. Caught by the number being implausible for a parser plus fourteen analyses — not by a test |
| E6 corpus | A-009 passed as the "edge materialized menu" fixture | a `materialize` block writes its policies INSIDE its braces, the grammar parsed that block as an expression body, and `decl.policies` was empty for every materialization in the corpus. All four of A-009's graph edges pointed at nothing. The fixture demonstrated the SYNTAX of a dependency graph and none of its semantics, and it passed because no analysis had ever read those clauses — the C0→C1 repair arriving in a second place, unnoticed for the same reason |
| E6 graph | every page's dependency on its own query resolved | it resolved to the TYPE of the same name. `store.page` declares `query Store` and imports `domain.Store`, and the general `resolve` tries the type namespace first, so every page edge pointed at a record definition. The graph was full, the paths were plausible, and no edge named a thing that could be invalidated |
| E7/E9 inference | every checker resolved sibling declarations, because the API offered a way to say so | the module was a **builder step** — `Types::of_body(..).in_module(m)` — and four of the seven callers never called it. Those four silently inferred with no module, so a bare name never resolved to a sibling and rules that depend on a type went quiet rather than wrong. Found only because a new rule needed to resolve `query Menu(..)` **during** construction, where no builder step had run yet. The fix is not a seventh call site: the module is now a parameter of `of_body`, so a caller cannot forget it and there is no order to get wrong |
| E2 grammar | an audited `unsafe` was reported as unjustified | the newline rule that ends a statement also ended `unsafe capability … because "…"` before its `because` clause, so the justification became a separate statement and never reached the declaration. **`pw fmt` then baked the misparse into the source**, which is the part worth remembering: a formatter faithfully renders a wrong parse |

The two E6 rows and the inference row share a shape with the by-name member
fallback deleted in E2C, arriving through three different doors. The E6 graph
row is the sharpest: a wrong answer that is *shaped like* a right one — a real
path, to a real declaration, of the wrong kind — is harder to see than no answer
at all, because every summary count looks correct.

The inference row: **whether a correctness analysis ran at
all depended on something other than the program.** There it was a global
accident of spelling; here it was whether a caller remembered a second method
call. The countermeasure is the same one, and it is not a test — it is removing
the way to express the mistake. A test asserting "every caller calls
`in_module`" would be a lint that a new call site can be added without.

Four of the grammar rows share one shape: **the diagnostic pointed at the line
after the defect.** A parser that recovers silently moves the blame downstream,
which is why the errors read as "exotic corpus syntax" rather than as parser
bugs. The countermeasure is in the E2 tests — every construct asserts its tree
*shape*, so a wrong parse fails where it happens instead of somewhere plausible.

Five share a different and more dangerous shape — the two `E5` rows, `R-012`,
the 25-to-27 rise, and `PW0323`: **the number went up.** Coverage rising is the
signal everyone is watching for, and in each case it rose, or would have risen,
while detection got worse or stayed the same. Two countermeasures now exist:

- **three numbers, not one** — produces an error / emits the declared code /
  fully enforces the declared invariant — with `declared_code == errored`
  asserted as an *equality* rather than a floor, so a catch that is merely red
  is a regression even when the count rises;
- **registry tests that fail closed** —
  `every_registered_code_is_one_a_checker_can_emit` rejects a code the registry
  claims and no checker emits, and the known-gap test now rejects an entry the
  alias table has made unreachable.

Neither countermeasure would have caught the `PW0323` case, which is worth
stating: the code matched, the fixture was uncaught, and only reading the
registry against the corpus by hand found it. That is the residual risk.

Almost all were caught by looking at a raw number and asking whether it was
plausible — never by a test going red.

**Rule adopted**, promoted by the project architect into a general
admissibility standard:

> **A measurement or checker result is not admissible evidence until its
> instrument has a negative control proving it can detect the corresponding
> failure.**

Every gate needs four things:

| | |
|---|---|
| **positive control** | the expected-good case passes |
| **negative control** | an intentionally broken case makes *that exact detector* go red |
| **plausibility check** | raw values fall in a physically or semantically credible range |
| **raw evidence** | the unprocessed trace or output is archived |

For parsers and metadata readers, add **mutation tests**: remove an effect row,
change a constructor tag, omit a source span, misclassify an open row as total,
change an expected import. The reader must fail closed or make the test red.

This applies to benchmarking, compiler gates, browser instrumentation,
capability auditing, and the E→P evidence ledger — not only to unit tests.

The five instances so far are why. Two flattered the system, one damned it, and
one (`.kki` reporting polymorphic rows as total) would have let a *gate* pass
while hiding the case the architect had flagged as most important.
