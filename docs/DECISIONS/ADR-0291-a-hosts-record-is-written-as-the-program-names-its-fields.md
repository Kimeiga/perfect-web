# ADR-0291: a host's record is written as the program names its fields

Status: accepted under the owner's delegation of 2026-10-02, on W6's
finding of 2026-10-09 (kiokun's labels and character header). Date:
2026-10-09. Milestone: E14. Extends ADR-0172's rule from the browser to the
host.

## Context

- **W6's finding.** kiokun's data layer wrote an entry's field
  `chinese_char`, as the program's `Entry` names it, and the query trapped:
  "the host's record has no field `chinese-char`". A component's world names
  a record's fields in WIT's kebab case, and `pw-host`'s `project`, which
  holds a host's answer to the component's types, looked a field up by that
  name alone. A field of one word never showed it.
- **The browser already writes the program's names.** ADR-0172 reads a
  record a browser sends by its Pleris names (`from_json`: `minor_units` for
  `minor-units`). A data layer is written by people who read the program,
  not its world, and two rules for one record is how a layer and a browser
  come to disagree.

## Decision

- **A host's record is read field by field by the WIT name, or else by the
  program's** (`-` read as `_`, as ADR-0172 reads a browser's). A field under
  neither is refused, naming both: "the host's record has no field
  `opens-minute`, nor `opens_minute`".
- A field written under both names is read by its WIT name, and the other is
  a field the type does not declare, which is not passed in, as before.

## Alternatives

- **The layer's own record builder maps the names**: refused, since a rule
  every layer must remember is one some layer forgets.
- **Refuse at start, checking each layer's records against the contract's
  types**: a layer builds its records as it answers, so there is nothing to
  check before an answer; the refusal names both spellings instead.

## Acceptance

- **`runtime/pw-host/tests/host_answers.rs`**, two tests: a field written as
  the program names it (`opens_minute`) is read, and one under neither name
  is refused by both.
- **`scripts/descriptions_mutations.py`**: "a field is read by its WIT name
  alone" is killed; "a field the answer lacks is filled with nothing" is
  re-anchored and killed.

## Not claimed

- **A field named in camel case** (`opensMinute`) does not come back from its
  WIT name (`opens-minute`) as `_`-for-`-` does, as ADR-0172 says of the
  browser: such a field is written by its WIT name.
