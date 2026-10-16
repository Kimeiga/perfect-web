# ADR-0345: an `{#each}`'s list and key are expressions every walker of a view reads

Status: accepted under the owner's delegation of 2026-10-02 (the
integrator's ruling of 2026-10-15 on the each-head finding: "a soundness
hole in a guarantee the project claims; close it at the root, not at the
one position"). Date: 2026-10-15. Milestone: E14. The each-head-effects
track. Builds on ADR-0242 (an `{#each}`'s head is read once, by the
grammar; its "not claimed" is this ADR), ADR-0048 (a member read is a
call), ADR-0073 and ADR-0074 (what a template writes), ADR-0129 (labels
follow values), ADR-0130 and ADR-0141 (signals, dialogs), ADR-0017 (Marko).

## Context

- **The finding** (the track guide, verified 2026-10-15 01:35Z). In
  `view Menu() !{} { <ul>{#each items() as item (item.name)}..{/each}</ul> }`,
  `items` reading the database, `pw check` reported nothing; the same call
  in a `let`, a text hole, an `{#if}` or an attribute is PW0401. Charter
  §7.4's pure rendering had a hole at the loop's head.
- **What else checked clean at 5da9c055**, each probed with that commit's
  `pw` and refused now (`tests/each_head_effects.rs` holds each):
  - the list read through a member, `{#each store.items as item}`, `items`
    an effectful function of `Store`: a member read is a call (ADR-0048).
    `pw build` compiled it too, so the hole reached a built program;
  - the same in a loop's key, `(item.label)`, and in a nested loop;
  - a secret rendered through a computed list, `{#each [key] as k}<li>{k}</li>`
    or `List.reverse(keys)`, where the same list in a `let` is PW5003;
  - an assignment to a signal in a loop's head, unseen by PW5300;
  - `{item.label}`, a text hole every walker read, in a loop over a
    computed list, `all(given)`: the rows had no type, so no member, and no
    effect, was found;
  - and two wrong readings the other way: a dialog in a loop over a
    module's signal was refused as shown by nothing (PW5303), and Marko
    wrote a list read from a parameter, `store.items`, without `input.`.
  `pw check` and `pw build` also disagreed on the first program: the build
  refuses `items()` as "computed", and the check accepted it.
- **Why**: one cause, read three ways.
  1. **The head was text.** `EachHead` held the list and the key as
     strings, "read as written" (ADR-0242's "not claimed": "Typing a
     computed list, or resolving a key's names as terms, changes what every
     walker of a template sees. It is a later step").
  2. **The walk could not reach it.** Every analysis that walks a body's
     expressions reaches markup through `Expr::Template`'s `parts`, which
     lowering collected with a `match` on node kinds whose `_ => {}` arm
     dropped what it did not name. The loop's head was not an expression,
     so it was none of them.
  3. **Each reader of the text read its own narrower thing**: the labels
     (`collection_label`: public unless the list's first name was a
     local's, and without a member body's label that `label()` joins), the
     row's type (a path from a local only), the value relations (a path,
     field by field), the names check (a path's first name), lexical scope
     (the first name), the dialog rule (a local signal only), PW5021 (the
     key's first segment), the template IR and Marko.
  This is the failure seen about eight times before: a consumer names the
  kinds it knows and silently drops the rest.

## Decision

1. **The list and the key are expressions in the body's arena**, lowered
   by the real grammar over the head padded to its place, so each span is
   the file's (as a block marker's condition, ADR-0237). The text as
   written stays, as `EachHead::written`, for what a message quotes and
   nothing else.
2. **One reading of where a view holds an expression**: `hir::Position`
   (a hole, an attribute, a handler, a block's subject, a branch's
   condition, a loop's list, a loop's key), declared with `Position::ALL`
   by one macro, and `Node::exprs`, which destructures every node variant
   and every field, `EachHead`, `Attr` and `TemplateArm` included, with no
   wildcard: a field that holds an expression cannot be added without a
   compile error there. `Expr::Template`'s parts are built from it, so
   every walk of a body's expressions reads every position.
3. **Each reader reads the expression**:
   - lexical scope and the names check read the list in the scope around
     the block and the key in the row's, where `item` in `(item.id)` is the
     row; a name in a key that nothing binds is PW0021 (it was PW5021
     alone);
   - a row's label is its list's `label()`; its type, its list's: a path's
     as before, and a computed list's where its type names no parameter
     (`infer.rs`), or as the value relations type any call (`values.rs`);
   - the list's and the key's fields are related where they are read, by
     the general walk (`read_through`, the text's own reading, is gone);
   - the dialog rule reads the list as it reads a block's subject
     (`signal_of`), so a loop over a module's signal shows its dialog;
   - PW5021, the template IR and Marko read the key's and the list's path
     from the expression; Marko writes the list with `expr_text`.
4. **What a template part means to each rule is decided per position.**
   PW5040 (a session's handle printed) reads `Node::exprs` with an
   exhaustive match: a hole and an attribute are printed, and a handler, a
   subject, a condition, a list and a key are not (the key is written as a
   keyed digest, never as itself; `pw-document`'s `instance_token`). What a
   handler captures is the resume manifest's: a handle captured is PW5007,
   or PW5018 in a page private to no one (probed 2026-10-15).
   PW5003 refuses a secret at every part, as it did an `{#if}`'s subject:
   a secret list or key now too.

## The audit: every analysis by position

Positions: hole (H), attribute (A), handler (Hd), subject (S), condition
(C), list (L), key (K). "Walk" means `body.walk()` through
`Template.parts`, which reads every position since decision 2.

| analysis | reads | before | after |
|---|---|---|---|
| effect inference (`effects.rs`), and what derives from it: contracts (`contract.rs`), placement (`placement.rs`) | walk | H A Hd S C | all seven |
| render contexts (`contexts.rs`) | Hd is an interaction, a stream's query a streamed task, the rest render | L K not expressions | L K render |
| privacy: a row's label (`labels.rs`) | the list | text: a local's path, declared member labels | `label(list)` |
| privacy: PW5003 (`check.rs`) | walk | H A Hd S C | all seven |
| privacy: PW5007 (`resume.rs`) | a row through its list's head | text's first name | the expression's path root |
| PW5040, a session's handle printed (`check.rs`) | a node `match` with `_` | H A | `Node::exprs`, each position an arm: H A printed |
| signals: where an expression runs (`signals.rs`, PW5300, PW5301) | walk | H A Hd S C | all seven |
| signals: what shows a dialog | S, L | L by a local signal only | L by `signal_of`, as S |
| names (`names.rs`) | node walk | H A Hd S C, L's first name | all seven, L and K as expressions |
| lexical scope (`lexical.rs`) | node walk | H A Hd S C, L's first name | all seven, K in the row's scope |
| a row's type (`infer.rs`) | L | a path from a local | the expression: a path, or a computed list's closed type |
| value relations (`values.rs`) | node walk with `_`, and walk | L K as text | L K typed as expressions, their fields by the walk |
| keys: PW5021, PW5011 (`check.rs`) | K | text | the expression |
| template IR (`template_ir.rs`) | L K by path | text | `value_path` of the expression |
| speculation (`backend/speculation.rs`), page values (`page_values.rs`) | the template IR's reads | unchanged | unchanged |
| Marko (`marko.rs`) | L K | text | `expr_text`, `value_path` |
| predicates a markup holds (`holds.rs`) | S, C, `hidden`, handlers | a loop asserts nothing | unchanged |

Rules about some positions by design keep their own per-kind matches, and
are not claimed to read every position: the value relations' (a hole is
text, a subject a truth, an attribute text, a truth or a prop), the render
contexts', the predicates', the dialog walk's, and the markup rules that
read elements by tag and attribute (ids, titles, links, metadata, nesting,
comments, routes, uploads, streams, origins, layout, annotations).

`tests/view_positions.rs` holds five analyses at every position: effects
(PW0401 while rendering; PW5005 in a handler), privacy (PW5003), signals
(PW5300; PW5302 in a handler), names (PW0021), and PW5040 (refused at H
and A, accepted elsewhere). Each probe and each verdict is an exhaustive
`match` over `Position`, so a new position is a compile error there until
every verdict at it is written; it fails if `Position::ALL` is empty, if a
probe is not held at its position (read back through `Node::exprs`), or if
a refusal is missing.

## What it changes for programs

- **Nothing in the repository is newly refused.** `pw check` over the
  accepted corpus, the store, the store with the demo, the feed, kiokun,
  kiokun-site, hello-static, render, koka, the benchmark baseline and the
  library: clean, as before. counter (PW5302), streamed (unresolved types)
  and robustness's regression fixture (PW0603) say what they said at
  master, byte for byte (each checked with master's `pw` at bb2847f6);
  none has a loop over anything but a path.
- **Newly refused**: the shapes above; the corpus's R-080 and the
  generality witnesses below.
- **Newly accepted**: a dialog in a loop over a module's signal, whose
  closing it hears.
- **A diagnostic moved**: a field a loop's list or key reads that its value
  lacks (PW0610) is reported at the field, by the general walk, rather than
  at the block.

## Not claimed

- **A member read through a generic call's result** (found by this audit,
  not closed by it). The effect inference's typer (`infer.rs`) keeps a
  generic call's declared result with its parameter unsolved, so a member
  read from a value `List.reverse(given)` computes performs nothing it can
  see: `{#each List.reverse(given) as item}..{item.label}..`, the same
  through `let all = List.reverse(given)`, and `fn first_label(given)
  !{} { for x in List.reverse(given) { return x.label } .. }` (an
  undeclared effect) each check clean, at master and now. The value
  relations' typer solves such a call. Closing it changes the type of every
  generic call the effect, label and resume analyses read, and a NARROW
  witness under `forbidden_effect` would move the published 54 / 54 figure:
  the integrator's question, below.
- **A computed list builds.** The build still reads a loop's list by path
  ("`{#each}` reads its list by path, and `items()` is computed", ADR-0073);
  the check accepts a pure computed list, as it accepts a computed hole the
  build refuses (ADR-0074).
- **A secret key's digest**: a row's token is a keyed PRF of its key, so
  no key is recovered from the markup; that two rows share a key is visible.
  PW5003 refuses a secret key, so no program shows one.
- The rules listed above as per-kind are not claimed to read every
  position.

## Alternatives

- **Add the loop's head to the parts' `match`** at the one position: it
  closes the effect hole and leaves the text readers (labels, types,
  names, the dialog rule) each reading its own narrower thing, and the
  next position as easy to drop as this one was.
- **A visitor every analysis calls with a closure per position**: the
  analyses that need a position's scope (lexical, names) would still walk
  the nodes, and the rest read the walk; `Node::exprs` with the parts built
  from it is that visitor where it matters, and the position test holds
  the readers that walk on their own.
- **Keep the text and add the expressions beside it**: two readings of one
  head, which is how the labels' and the types' disagreed with `label()`.

## Questions for the integrator

1. The generic-call gap above: a track of its own (instantiate a generic
   call's result in `infer.rs`, or have the effect inference read the value
   relations' typer), and whether a NARROW witness should record it now,
   which moves 54 / 54.
2. `compiler/pw-core/tests/statements_separated.rs:176` on master still
   says `ADR-0345` (another track's); numbering this ADR by a global
   replace would rename that one too.

## Acceptance

`just e14-each-head-effects`:

- `compiler/pw-core/tests/each_head_effects.rs`, 9 tests, each with its
  control: the call, the member, the key, the nested loop, the computed
  list's secret and its row's member, the dialog over a module's signal,
  Marko's list, and the neighbours that stay accepted (a loop over a
  query's value, over a pure function's result, over a pure member);
- `compiler/pw-core/tests/view_positions.rs`, 2 tests: five analyses at
  seven positions, 35 verdicts, with the floor above; and `Node::exprs`
  finding each position where it is written;
- the readers whose tests the change reached: `each_heads` (a key naming
  nothing is PW0021 too), `template_values` (PW5021 keyed on a name that
  resolves), `template_text`, `dialogs`, `marko_adapter`, `lexical_scope`,
  `every_name_resolves`, `session_to_browser`;
- the corpus at C26: R-080 (PW0401 at a loop's list), A-038 (a loop over a
  query's value); `forbidden_effect`'s `each-head-member` and
  `each-key-nested` (GENERAL) and `each-pure-member` (NEIGHBOUR);
  `secret_to_browser`'s `each-computed-list` (GENERAL). Each GENERAL one
  checked clean with master's `pw`;
- mutation controls, `scripts/each_head_effects_mutations.py`: 20 mutants,
  each a position taken from `Node::exprs` or one reader's visit of it,
  20 of 20 killed (2026-10-15, run whole). A 21st, "a handler is printed"
  (PW5040), survived and is left out as equivalent: a handler's value is a
  function (PW0614 refuses any other), whose type never holds a handle.
- Sixteen mutants of six scripts re-anchored where the readers moved
  (`each_heads`, `lexical`, `names`, `row_reads`, `template_text`,
  `template_value`) and two of `session`, each undoing the same piece.
- **Run whole, near what this changed** (2026-10-15): `each_heads` 12 of
  12, `template_text` 16 of 16, `template_value` 11 of 11, `row_reads` 21
  of 21, `session` 8 of 8, `dialogs` 5 of 5; and, since their tests write
  a loop over a labelled value, `ids` 13 of 13, `labels` 16 of 16,
  `nested_scope` 12 of 12, `provide` 29 of 29 (its browser half in
  Chromium). Two survived the first run, each a test this change had made
  too weak, and each test is now precise: `lexical` ("an `{#each}`
  block's name carries no label": a secret list is a part now, refused
  itself, so `lexical_scope` asks for the row's own refusal) and `names`
  ("an `{#each}` source is not read": the names check reads a part no
  node holds in the template's scope, which is the block's at the top of
  a view; `every_name_resolves` adds a nested loop's list and a loop in an
  arm). Run again: `lexical` 19 of 19, `names` 20 of 20.
- The workspace suite, on a copy of the tree: one failure, `pw-resource`'s
  count of the accepted corpus's resources, which A-038 moves to 14.

## Integration on train-10 (2026-10-15)

Corpus C25 and A-037 belong to bounded reads on train-9. This change is C26,
with A-038 and R-080: 38 accepted and 79 rejected fixtures on the combined
tree. The generic-result hole is assigned to the generic-effects track,
with explicit refusal where a receiver cannot be typed; it remains open
until that track's own controls and integrated verification pass.
The stale statements_separated ADR reference was repaired on train-9 as
ADR-0303. Integrated CI and verification remain pending.
