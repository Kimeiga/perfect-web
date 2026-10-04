# ADR-0185: ids, and the ARIA that names them, checked at build

Status: accepted under the owner's delegation of 2026-10-02. Date: 2026-10-04.
Milestone: E14. Charter §8.2 lists "duplicate IDs" and "invalid ARIA
relationships" among the checks the compiler should make. ADR-0182 logged
both as open.

## Context

- **What the compiler checked.** Only what names a form control (PW5014,
  ADR-0143): a `<label for>`, a wrapping `<label>`, or an
  `aria-labelledby` that reaches text.
- **What it did not check.** Every other id and reference was the browser's
  to find, and only on a page someone read:
  - **Two elements with one id.** A label, an ARIA relation and a fragment
    link each find the first. The second is named by nothing.
  - **A static id inside an `{#each}`.** Every row has it.
  - **An `aria-describedby`, `aria-controls` or `for` that names nothing.**
    A browser drops the reference without a word.
  - **A reference to an element shown only in one arm of a block** the
    referrer is outside of. The reference names nothing whenever that arm
    is not shown.
  - **An ARIA attribute WAI-ARIA does not define, or a value or a role it
    does not.** `aria-labeledby` is one letter short of `aria-labelledby`.
    A browser ignores it, and a section loses its name with no warning.
- **ADR-0182's audit reads all of these at run time**, in three engines. It
  found none on the store. A check at build finds them on every page, before
  it runs.

## Decision

1. **PW5031: an id names one element of its page.** Refused:
   - two elements with one id written as text;
   - an id written as text inside an `{#each}`, which every row has. The
     repair is to write the row's key into it: `id="line-{line.item_id}"`.

   Two elements in two arms of one block are one element, because only one
   arm is shown. The arms are those of `{#if}`/`{:else}`, of `{#match}`'s
   cases, and of a `<stream>`'s placeholder, ready and failed.
2. **PW5032: an id reference names an element its page shows whenever the
   referrer is shown.** That is, an element in an arm around the referrer,
   or in every arm of a block that shows one of its arms whatever it
   decides: an `{#if}` with an `{:else}`, a `{#match}` or a `<stream>`.
   - **The attributes read**: WAI-ARIA's id references
     (`aria-activedescendant`, `-controls`, `-describedby`, `-details`,
     `-errormessage`, `-flowto`, `-labelledby`, `-owns`), and HTML's `for`,
     `form`, `headers`, `list`, `popovertarget` and `commandfor`.
   - **Each token is read**: a reference list whose other ids name the
     element still has a stale one.
   - **An id the page computes may be any**, `id="line-{x}"` within its
     pattern: a reference that may name it is not refused.
3. **PW5033: an ARIA attribute, its value and a role are ones WAI-ARIA
   defines.**
   - **The attributes**: WAI-ARIA 1.2's states and properties, with 1.3's
     `aria-description`, `aria-braille*` and `*indextext`.
   - **The values**: each written value is checked against its type: a
     token, a list of tokens, an integer or a number.
   - **The roles**: 1.2's concrete roles, 1.3's `comment`, `image`, `mark`
     and `suggestion`, and the Graphics and DPUB modules' roles.
   - **The repair names the attribute meant**, by edit distance:
     `aria-labeledby` is offered `aria-labelledby`.
4. **One mistake is reported once, where it is.** A reference to nothing,
   or an id two elements have, is the defect. A field it leaves unnamed is
   not reported again by PW5014:
   - a field whose `aria-labelledby` names no element at all: PW5032;
   - a field a `<label for>` misses because the field's id is an earlier
     element's, or every row's: PW5031.
5. **Read in the declaration that renders them**, as PW5014 reads a label
   (ADR-0143). What a program computes is not read.
6. **Corpus C10.**
   - **New fixtures**: R-049, R-050 and R-051, one per rule; A-026, a page
     whose ids, references and roles are written right, an id in both arms
     of a block among them. Four categories are added.
   - **Generality witnesses**: a GENERAL and a NEIGHBOUR one per rule, so
     generality is 36 / 36.
   - **Two `control_without_label` witnesses named an id nothing had.**
     `labelledby-nothing.pw` moves to `reference_names_nothing`, since its
     defect is the reference. `label-for-another.pw`'s label now names
     another control's id, so it is an unnamed field alone. Their old texts
     are in `examples/history/C10/`.

## Alternatives

- **Leave these to the browser audit** (ADR-0182). It reads one page at a
  time, after it runs. The compiler reads every page, before any runs.
- **An id inside a loop allowed when the loop has one row.** A loop's row
  count is the data's, and a list that has one row today has two tomorrow.
- **A reference allowed when any of its ids names an element**, as the
  accessible name's algorithm skips the rest (accname 1.2, step 2B). The
  rest is a stale reference all the same, usually a leftover from a rename.
- **Report a field both as unnamed and for its reference.** That is two
  reports of one mistake, at one element, and the second says less than the
  first.
- **Check a reference across the page and the views it composes.** That is
  the next step. PW5014's own reading is within one declaration
  (ADR-0143), and a view composed into two pages would be read against
  each.

## Acceptance

Recorded by `just e14-ids` in `docs/evidence/E14/ids.txt`:

- **`pw-core`, `tests/ids_and_aria.rs`**: each rule, each case with
  controls; and one mistake reported once.
- **`pw-core`, `tests/labels.rs`**: ADR-0143's cases that now report their
  root cause.
- **The corpus at C10**: `corpus-check`; every rejected fixture emitting only
  its own defect; generality 36 / 36; the C10 history test.
- **Every program the repository checks is clean**: the store, the demos,
  the kiokun slice, the accepted corpus and the benchmark's store.
- **`scripts/ids_mutations.py`**: 12 mutants.

## Not claimed

- **A reference or an id across a page and the views it composes.**
- **Ids and references the program computes.**
- **Whether an ARIA attribute is allowed on its element's role**, axe's
  `aria-allowed-attr`, and the attributes a role requires. These are left
  to ADR-0182's audit.
- **A `<label for>` that names an element that is not a control.**
