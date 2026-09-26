# ADR-0056: Unicode case mapping

Status: accepted under the owner's instruction of 2026-09-25 ("ok fix all the
known gaps"). Decisions marked **(ruling needed)** were made without a ruling
and are offered for reversal. Date: 2026-09-25. Milestone: E10 (charter §14
M10 task 2, the standard library).

## Context

`String.to_lower_ascii` maps `A`-`Z` and nothing else (ADR-0040). Its
ruling-needed mark said Unicode case mapping was not decided.
KNOWN_LIMITATIONS: "There is no ... Unicode case mapping". A program could
not compare two texts regardless of case outside ASCII.

The component and the JavaScript module must give the same answer
(ADR-0044). JavaScript's `toLowerCase` follows its engine's Unicode version,
and Node 22's is not the compiler's. The Unicode version is part of what a
mapping means:
- the workspace's pinned Rust, 1.97.1, knows Unicode 17.0.0;
- a Rust 1.89 on the same machine knows 16.0.0.

The first version of this ADR's pin test assumed 16.0.0 and failed.

## Decision

### 1. What the operations are

`String.to_lower(text)` and `String.to_upper(text)` map each code point to
its lower or upper case, as Unicode 17.0 maps it: its full mapping, which
may be more than one code point.
- `ß` uppers to `SS`, and `ΐ` to three code points.
- `İ` lowers to `i` and a combining dot above.

**(ruling needed)**: the mapping is per code point. No rule looks at a code
point's neighbours, so a word-final capital sigma lowers to `σ`. Unicode's
default conversion, Rust's `str::to_lowercase` and JavaScript's
`toLowerCase` give `ς` there. `to_upper` is Rust's `str::to_uppercase`
exactly.

### 2. One set of tables, generated

`backend::case` generates each direction's table from the compiler's own
`char::to_lowercase` and `char::to_uppercase`, once per process:
- **ranges**, each `lo..=hi` with a delta on a stride of 1 or 2: 186
  entries for lower case, 199 for upper case;
- **multi entries**, code points that map to two or three: 1 for lower
  case, 102 for upper case.

The component carries a table in its data segment, after the string
literals, only when a body maps that way. A helper decodes each code point
and binary-searches the table. The output is allocated at three times the
input's bytes, the most any mapping grows. The module carries the same
numbers as constants, and a function reads them the same way. Neither calls
a platform's mapping.

`tests/case_mapping.rs` pins `char::UNICODE_VERSION` at 17.0.0. A toolchain
that changes it changes the language only through a failing test.

## Acceptance

- `compiler/pw-core/tests/case_mapping.rs`:
  - the tables reproduce Rust's mapping for every code point;
  - no mapping grows past the bound;
  - the version pin.
- `compiler/pw-conformance/tests/case_mapping.rs`, through the E8 host:
  - every code point either direction changes, as Rust maps it;
  - mixed text;
  - the growing and special cases, and the final sigma;
  - two growing results allocated one after the other.
- `compiler/pw-conformance/tests/javascript.rs`:
  - the module maps every such code point as Rust does;
  - two queries join the differential, and the generated strings gain
    cased letters beyond ASCII. The component and the module agree on 61
    queries and 12,200 calls.
- Mutation controls: `scripts/case_mutations.py`, `just e10-case`,
  9 mutants.

A tenth control, a range growing over a code point mapped otherwise,
survived. It showed that the generator's check could never be false: code
points are read in order, so such a code point would already have begun a
range. The check is removed, and the control replaced.

## Not done

- **Case folding** for comparison, and **locale rules** such as Turkish's
  dotted `i`, are not in the library.
- **`to_lower_ascii` stays** for callers that want ASCII only.
