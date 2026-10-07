# ADR-0239: every code the compiler writes is registered, once

Status: accepted under the owner's delegation of 2026-10-02. Date: 2026-10-07.
Milestone: E14. A correction, found with ADR-0237: the parser's codes for
what lowering parses were unregistered, and looking for others found three
more.

## Context

- **The registry is the codes' one source of truth** (`codes.rs`). A
  diagnostic's symbol, its semantic identity, is read from it, and a code it
  lacks has the symbol `<unregistered>`.
- **Its tests checked one direction.** A registered code must be one some
  checker writes (`every_registered_code_is_one_a_checker_can_emit`). Nothing
  checked that a code a checker writes is registered.
- **Three codes written were not**, found by reading the compiler for its
  literals:
  - **PW0101**, the declaration rules' "`read_your_writes` requires a
    session-scoped query";
  - **PW0102**, the declaration rules' "session query `Cart` declares a
    30.seconds staleness window";
  - **PW0102 again**, the parser's "expected `in` after the loop binding".
    One number, two meanings. The registry exists to prevent exactly that
    (`PW0323` once meant three things).
- **PW0100 was written too**: the declaration rules' "cannot materialize
  `Cart` in a shared public cache". It is an alias, the corpus's number for
  PW5001, and the label algebra writes PW5001 for the same clause. So
  `session query Cart(..) cache shared` was two errors under two numbers for
  one mistake.
- **The existing direction read the tests too**, so a code only a test names
  counted as one a checker can emit.

## Research

- **rustc's `tidy` checks both directions.** Its error-code check reads the
  registered codes, scans the compiler's source for the codes it uses, and
  flags a code used and not registered as well as one registered and not
  emitted: "We check that the error code is actually emitted by the
  compiler"
  ([tidy/src/error_codes.rs](https://github.com/rust-lang/rust/blob/master/src/tools/tidy/src/error_codes.rs)).

## Decision

1. **Every code a checker writes is registered.** The registry's tests read
   the compiler's own source, the parser, the checker and the CLI, without
   their tests:
   - `every_code_a_checker_writes_is_registered`, new;
   - `every_registered_code_is_one_a_checker_can_emit`, now without the
     tests.
2. **PW0101 and PW0102 are registered** as the declaration rules' codes:
   `read_your_writes_needs_a_session` and `session_state_is_fresh`.
3. **The parser's `for` without `in` is PW0018 `for_needs_in`**, in the syntax
   range.
4. **A reader's value in a shared cache is one error, PW5001.** The
   declaration rule that wrote PW0100 is retired. The label algebra's rule
   reads what a declaration observes as well as its keyword, so it catches
   everything the declaration rule did and more. Its diagnostic has charter
   §16.3's shape: the label's origin, why, and the repair. The corpus's
   PW0100 stays an alias for it.

## Acceptance

- **`codes.rs`'s tests**: both directions, over the compiler's own source.
  Before this ADR the new one listed PW0100, PW0101 and PW0102.
- **`compiler/pw-core/tests/registered_codes.rs`, 2 tests:**
  - a session's or a user's value in a shared cache is one error, PW5001,
    with its origin, explanation and repair; a public one is none;
  - a `for` with no `in` is PW0018, a stale session read PW0102, a public
    read-your-writes PW0101, each with its symbol.
- **`rules.rs`'s two tests of the retired rule are gone**; the charter §16.3
  shape they asserted is asserted of PW5001 above. The corpus's R-004, which
  declares PW0100, is caught by PW5001, its canonical code
  (`checking_source.rs`).
- **`scripts/registered_codes_mutations.py`: 6 mutants**, recorded by `just
  e14-registered-codes`.
- **Run whole, near what this changed or with tests it changed**:
  `read_whole_mutations.py` 18 of 18 killed, `query_retry_mutations.py` 4
  of 4, and `policy_value_mutations.py` 14 of 14 after one survivor.
- **The survivor, older than this ADR**: "a duration is not read" in
  `policy.rs`. Since ADR-0109 made `timeout` a budget, `freshness` is the
  duration domain's one head, and the test that killed the mutant wrote
  `timeout 30.secondz`. No test held a freshness that is no duration.
  `policy_values.rs` now refuses `freshness 30.secondz` and `freshness
  thirty`, beside a control.
- **ADR-0089's `policy_values.rs`** said of the shared cache "Two detectors
  report this one, as `pw check` prints them", PW0100 and PW5001. It expects
  PW5001 alone now.
- **The workspace, 2,133 tests; the browser suite, 782 in three engines.**

## Not claimed

- **A code written by another crate**, such as a host's. Only the compiler's
  are read.
