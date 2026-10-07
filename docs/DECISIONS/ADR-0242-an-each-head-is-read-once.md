# ADR-0242: an `{#each}`'s head is read once, by the grammar

Status: accepted under the owner's delegation of 2026-10-02. Date: 2026-10-07.
Milestone: E14. A correction, found with ADR-0237.

## Context

- **`{#each menu as item (item.id)}` was kept as text.** Its head is the
  list, the name each row binds, and the key that tells the rows apart.
  Five places split the directive at ` as ` and `(`, each its own reading:
  - name resolution (`resolve.rs`);
  - the template IR and its key checks (`template_ir.rs`, `check.rs`,
    `values.rs`);
  - lexical scope (`lexical.rs`, through `infer.rs`);
  - the names check (`names.rs`);
  - Marko (`marko.rs`).
- **What that missed or misread, each probed before this ADR:**
  - `{#each xs ys as x (x)}` checked, its list `xs ys`;
  - an unclosed key, `{#each xs as x (x}`, checked;
  - `{#each xs}` was refused for a key and a name it lacks (PW5011, PW0021),
    never for the `as` it lacks;
  - `{#each xs as x y (x)}` was refused for a key "`x y`";
  - PW5011 took any `(` in the directive for a key. A keyless loop over a
    list computed by a call, `same(xs) as x`, needed none.
- **ADR-0237 made what lowering parses reported**, and left this one as text
  no grammar read.

## Research

- **Svelte reads an each block's head with its grammar**, part by part
  (`phases/1-parse/state/tag.js`):
  - the list by `read_expression`;
  - the `as` context by `read_pattern`;
  - the index by `read_identifier`;
  - the key, in parentheses, by `read_expression(parser, '(')`.

  ([tag.js](https://github.com/sveltejs/svelte/blob/main/packages/svelte/src/compiler/phases/1-parse/state/tag.js))

## Decision

1. **The grammar parses the head** (`parse_each_head`): the list, `as`, the
   name each row binds, and the key in parentheses where one is written.
   Lowering parses it padded to its place, so its errors are reported where
   it is written (ADR-0237).
2. **A head with no `as` is PW0019 `each_head`**: "expected `as` and a name
   for each row". It is one error over what stands before `as`, and the
   head is read on from `as`, so the rows' name is bound. What stands
   between the name and the key, an index or a second name, is one error
   (PW0016) and the key is read. An unclosed key is PW0001.
3. **The block carries its head** (`Node::Block::each`, an `EachHead`):
   each part as written and where. Each of the five readers reads it, and
   none splits the directive.
4. **A loop's key is refused at the key** (PW5021), not at the block.
5. **PW5011 reads the head's key**: a keyless loop over a computed list
   needs a key as any loop over a mutable one does.

## Acceptance

- **`compiler/pw-core/tests/each_heads.rs`, 4 tests**, each with controls:
  - two words for a list is PW0019 at the second, alone; an unclosed key is
    PW0001; an index is PW0016 at `, i`, alone, the key read;
  - a head with no name says it lacks `as`;
  - `same(xs) as x` is PW5011, and keyed is not;
  - a key read from another name is PW5021 at the key.
- **`pw-syntax`'s unit test**: a head's parts, and each recovery, lossless.
- **`scripts/each_heads_mutations.py`: 12 mutants**, recorded by `just
  e14-each-heads`, each against the readers' own tests:
  - `every_name_resolves.rs`, `lexical_scope.rs`, `each_typing.rs`;
  - `marko_adapter.rs`, `template_blocks.rs`, `keyed.rs` and
    `nested_lists.rs`.
- **A survivor, and its test**: "a row's name is not a local binding", the
  name set a call's path through a row is resolved against where no lexical
  scope says. `resolve.rs`'s unit test holds it, and the controls run it.
- **Run whole, near what this changed**: `names_mutations.py` 20 of 20
  killed, `nested_scope` 12 of 12, `read_whole` 18 of 18, `template` 16 of
  16, `template_text` 16 of 16, `template_value` 11 of 11.
  `lexical_mutations.py` 17 of 19: its two survivors, "a `for` loop's name
  carries no label" and "a lambda's parameters carry no label", are older
  than this ADR, which changes neither path. Both were killed at `d569e18`
  (2026-09-26); with either mutant, the programs their tests write are
  refused all the same now. Why, and what holds each, is accounted for on
  its own.
- **The workspace, 2,148 tests**: 2,147 at the full check, and the one
  written for the survivor. **The browser suite, 782 in three engines.**

## Not claimed

- **The list and the key as expressions in the body's arena.** They are
  read as written, as they were. Typing a computed list, or resolving a
  key's names as terms, changes what every walker of a template sees. It is
  a later step.
- **An index**, `{#each xs as x, i}`. Refused, as before, now by the grammar.
