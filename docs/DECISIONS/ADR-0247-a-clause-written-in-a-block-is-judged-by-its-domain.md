# ADR-0247: a clause written in a block is judged by its domain

Status: accepted under the owner's delegation of 2026-10-02. Date: 2026-10-07.
Milestone: E14. A correction, found with ADR-0243: a value the checker
accepted and nothing judged, close to a soundness hole, so taken before the
owner's next item.

## Context

- **The names check reads a clause a block writes as one**: a head the
  policy table knows and what follows it on its line (ADR-0047), to a `;`
  (ADR-0243). PW0335 judged a policy's value against that table only where
  the policy heads a declaration (ADR-0089): it read `decl.policies`, and a
  block's clauses are body statements. So, each probed before this ADR:
  - `scope bogus` and `scope component page` in A-019's resource block;
  - `captures bogus`, `load bogus`, `on_version_mismatch bogus` and
    `identity content_address(bogus)` in A-014's `handler_policy`;
  - `respects bogus` in A-020's `animate` block;
  - `intrinsic_height bogus` in A-023's `subtree` block

  each checked. KNOWN_LIMITATIONS said such clauses were read by the scope
  graph and the handler rules. The scope graph reads `scope` and refuses no
  word it lacks; nothing else reads them.
- **A length was judged nowhere.** The table gives `intrinsic_height` a
  length, "a CSS length: `intrinsic_height 24.px`", and the judge had no
  case for it: `intrinsic_height bogus` checked as a header too.

## Research

- **Ruling 0047-a** (ADR-0210): "A clause is syntax only in a block that
  admits it … The parser makes a clause node there and checks its words
  against the closed set." Its interim reads clauses in the names check
  (ADR-0216). This checks the interim's clauses' words, as the ruling's full
  form will.
- **CSS Values and Units Level 4 defines the length units**
  ([css-values-4](https://www.w3.org/TR/css-values-4/)):
  - absolute: `cm`, `mm`, `Q`, `in`, `pt`, `pc`, `px`;
  - font-relative: `em`, `ex`, `cap`, `ch`, `ic`, `rem`, `lh`, and their
    root forms `rex`, `rcap`, `rch`, `ric`, `rlh`;
  - viewport-percentage: `vw`, `vh`, `vi`, `vb`, `vmin`, `vmax`, each also
    in its small, large and dynamic forms (`svw`, `lvw`, `dvw`, ..).

  Container query units are another module's.

## Decision

1. **The names check returns each clause it reads in a block**: its head,
   its value as written with its layout removed, as a header's is, and
   where, head to value.
2. **One judge**: PW0335's, for a policy heading a declaration and a clause
   a block writes, against the same table. Its related note says which: "a
   policy of `X`" or "a clause in `X`".
3. **A clause's value is all its line holds**, to a `;`: `scope component
   page` is a value of two words, which a word's domain refuses.
4. **A length is a count, whole or with a fraction, then `.` and a unit CSS
   Values 4 defines**: `24.px`, `1.5.rem`, `10.dvh`. A unit is spelt as the
   spec spells it. CSS reads one in any case; a program writes it one way,
   as it writes every word of a closed set (`cache Shared` is refused).

## Acceptance

- **`compiler/pw-core/tests/block_clauses.rs`, 5 tests**, each editing a
  program of the accepted corpus, which as the corpus writes it is the
  control:
  - A-019: `scope bogus` and `scope component page` are PW0335, said of a
    clause in `LazyMap`, its value without its layout; a placement's list
    is judged whole;
  - A-014: `captures`, `load`, `on_version_mismatch` and `identity`'s
    operator;
  - A-020: `respects bogus`;
  - A-023: `intrinsic_height bogus`, `24.pz` and `24` are not lengths, and
    `1.5.rem` and `10.dvh` are;
  - `policy::length`, each case.
- **`compiler/pw-core/tests/policy_values.rs`**: a header's values, judged
  as before.
- **Every `.pw` file in the repository** reports the same PW0335 as before
  (32): none writes a value a block's clause lacks.
- **`scripts/block_clauses_mutations.py`: 13 mutants**, recorded by `just
  e14-block-clauses`.
- **The chain runs on the push** (ADR-0245).

## Not claimed

- **What reads these clauses.** Judged now, they are read by nothing:
  - a `handler_policy`'s `identity`, `captures`, `on_version_mismatch` and
    `load`: no rule, generator or runtime reads the block (the rolling
    deployment it is for is RISK_REGISTER's R5, open);
  - `respects prefers_reduced_motion`: no animation is generated that
    yields to the preference;
  - `intrinsic_height`: no `contain-intrinsic-size` is written.
- **Where a block's clause may be written**: `respects` in a resource's
  block is judged by its domain, and not refused for its place. That is
  0047-a's "only in a block that admits it", the full split.
