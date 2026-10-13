# ADR-0330: an arm's binding is read through a member, as a block's value is

Status: accepted under the owner's delegation of 2026-10-02 (the
integrator's). Date: 2026-10-13. Milestone: E14. Found by W9 in NEXT 6b (an
order's total charged). Builds on ADR-0169 (a member read is computed or
refused), ADR-0323 (a member read is computed wherever a page reads it),
ADR-0042 (a `{#match}` arm binds its case's payload) and ADR-0061 (a declared
case of several fields).

## Context

- **A member read of an arm's binding did not build.** `{#match
  order_total}{:Some(total)}{total.display}{/match}` was refused by `pw
  build`: "part N reads `total.display` through a member function in a text
  part, which no host computes there". ADR-0323 computes a member read by its
  path from a query's binding; an arm's name is no query's binding, and
  ADR-0169 computes only a loop's item's.
- **So a program worded its value where it read it.** W9's order page asked
  for "Total charged: $x" from its query: the sentence-instead-of-data
  pattern the integrator ruled against for a closed store (ADR-0328).
- **`pw check` said nothing**: the read is well typed; whether a host
  computes it is the build's, as every ADR-0169 refusal is.
- **A field of an arm's binding always worked**: the renderer binds the
  payload to the arm's name (`arm_shown`) and reads it by field.

## Decision

1. **The path gains a case's payload** (`Step::Payload { case, field }`), by
   the case's WIT name as the template names every case (`some`, `ok`,
   `err`, `circle`), and for a declared case of several fields the one at
   `field`. An arm's read through a member is planned as ADR-0323's are:
   from the query's value the match decides by, by field and through a
   member to its subject (`order.discount`, `total.discount`), the payload,
   then the reads after the arm's name; a host computes it once a change,
   into what the block is rendered with, by its path. In text, in an
   attribute, and in what a block inside the arm decides by.
2. **A value of another case sets nothing.** The host's step answers no
   value where the subject's case is not the arm's, and the document sets
   nothing for the part: its arm is not shown. A value that is no case is
   the plan's fault, said.
3. **Two arms that bind one name and read one path through it from
   different values are refused**, naming both parts: a block's values are
   set by path, whole, and one would overwrite the other. Named apart, both
   are computed; the same read twice is planned once. An arm's name that
   hides a binding of the page is renamed where the template lowers it, and
   is read by its new name.
4. **Refused, by name**: an arm whose match decides by no query's value (a
   loop's item's, another arm's binding), and an arm's member read inside a
   region the browser renders again from a speculation, which would have no
   such value ("a member read of an arm's binding in a speculated region").
5. **The template's `Arm` names its case by its WIT name**, as it did; its
   comment said `Some`, `None`, `Ok` and `Err`, which no lowering wrote.

## Alternatives

- **Lift the arm into a computed hole** (ADR-0226) over the subject, a
  function answering an `Option` of the member's result. One more component
  per read, and a function the program never wrote, where the member it
  calls is a component already.
- **Name every arm's binding apart in the template.** No collision could be
  written, and every page's template would change; a refusal names the two
  parts where they are written.
- **Keep the sentence.** A query answers data; the page words it.

## Acceptance

- `compiler/pw-core/tests/arm_member_reads.rs`: each kind of case (`some`,
  `ok`, `err`, a declared case of one field and one of several) read through
  a member, its subject read by field and through a member; in an attribute
  and a block's subject, planned once; a field of an arm's binding is the
  renderer's; a renamed binding read by its new name; two arms reading one
  path from different values refused, and named apart computed; an arm of a
  match on a loop's item refused by name.
- The server's `payload_tests`: each case's payload, and none for a value of
  another case; a field of a case of several.
- Every program and every other test as before (`just ci`).
- `scripts/arm_member_reads_mutations.py`, each mutant killed (`just
  e14-arm-member-reads`).

## Not claimed

- **An arm of a match on a loop's item, or another arm's binding**: refused
  by name; nothing asks for it yet.
- **In a region the browser renders again from a speculation**: refused
  until the speculation module computes it from the speculated value.
- **`pw check` does not report what the build refuses** here or in
  ADR-0169's other cases: a page that checks clean may not build. Unchanged.
