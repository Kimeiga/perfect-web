# ADR-0042: `{:else}`, `{#match}`, and interpolated attributes in templates

Status: accepted under the owner's instruction of 2026-09-25 ("ok do all of
that"). Decisions marked **(ruling needed)** were made without a ruling and are
offered for reversal. Date: 2026-09-25. Milestone: E10.

## Context

`docs/NEXT.md` named three template gaps after kiokun's logic moved into
Pleris (ADR-0041): `else`, `Option`, and an interpolated attribute. ADR-0037 §4
worked around the second by splitting kiokun's page in two, one page for
`Some` and one for `None`.

The first of the three was not a gap. It was a miscompile:

```
{#if signed_in}<p>Welcome back</p>{:else}<p>Please sign in</p>{/if}
```

passed `pw check` and `pw build` with no diagnostic. It rendered both
paragraphs when `signed_in` held and neither when it did not. The HIR
lowering dropped the `{:else}` marker and kept both halves as the block's
children. The template IR's `Conditional` has had an `otherwise` branch since
E7, and nothing ever filled it.

Nothing else checked a block's markers either. `{#if a} .. {/each}` closed
the `if`, and an unknown directive such as `{#await}` passed `pw check` and
was refused only when rendered.

## Decision

### 1. A block's markers are kept, and checked

The HIR keeps each `{:..}` marker in its block's children as a
`Node::Branch`, at its place. Nothing that walks the markup can skip a branch,
because the branches are the same children as before, now with separators.
The subject of `{#if c}`, `{:else if c}` and `{#match e}` is parsed as an
expression, as a string's `{..}` holes are. The value relations, the effect
walk and the privacy rules therefore see it.

A malformed block is refused with PW5019, a new code owned by the markup
rules. A block is malformed when:
- it closes with another block's name (`{#if} .. {/each}`);
- its directive is unknown;
- a marker is not one its block takes:
  - `{:else}` and `{:else if}` belong to `{#if}`;
  - `{:else}` comes once, and last;
  - constructor arms belong to `{#match}`, with nothing before the first;
- a marker stands outside any block.

`{:else}` in `{#each}`, Svelte's branch for an empty list, is refused;
`{#if xs}` around the list says the same thing.

### 2. `{:else}` and `{:else if}`

`{#if a}A{:else if b}B{:else}C{/if}` is a `Conditional` on `a`. Its
`otherwise` is a second `Conditional` on `b`, whose `otherwise` is `C`. An
`if` with no `{:else}` renders nothing when the condition is false, as before.

### 3. `{#match}` takes an `Option` or a `Result` apart

```
{#match entry}
    {:Some(e)} <h1>{e.key}</h1>
    {:None} <p>There is no entry.</p>
{/match}
```

- The subject is a value path, as every template value is.
- The arms are `{:Some(x)}` and `{:None}`, or `{:Ok(v)}` and `{:Err(e)}`.
- An arm binds at most one name, typed as the payload for the value
  relations.
- A template match is exhaustive, as ADR-0011 requires of every match: both
  constructors, each once. A missing arm is PW0305, the language's own code.
  A constructor the subject's type lacks is PW0608.
- `{#if}` over an `Option` or a `Result` is PW0600, a value that may be
  absent used as a value: `{#match}` takes it apart.
- A declared sum type's constructors are refused, as the component backend
  refuses them (ADR-0039).

The syntax is **(ruling needed)**. It follows Svelte's `{#await}`, whose
`{:then value}` markers bind names, and Pleris's `match`. The alternatives
were `{#if let Some(x) = e}` and `{#some e as x}`; each serves `Option` alone.

The IR gains `Part::Match { id, value, arms }`. Each arm is a case name, an
optional binding and a body. The renderer gains `Value::Variant { case,
payload }`, and a host passes `Some(v)` as the case `Some` with the payload
`v`. A match region is a range part, as a `Conditional` is, so the browser
runtime replaces it with the server's markup as it replaces a conditional.

### 4. An attribute can interpolate: `href="/{hit.target}"`

A quoted attribute value with `{..}` holes lowers as a string with holes. Each
hole is a real expression, so everything that sees a string's holes sees
these. The IR gains `Part::InterpolatedAttribute { id, owner, name, segments,
context }`, whose segments are static text and value paths. A hole that is not
a path is refused, as a `{..}` between tags is.

Each value is escaped for its attribute's context:
- **An ordinary attribute**: attribute-escaped.
- **A URL attribute** (`href`, `src`, `action`, ..) **(ruling needed)**:
  - the value must begin with static text, which fixes its scheme and the
    start of its path;
  - each value is percent-encoded as a URI component: every UTF-8 byte but
    RFC 3986's unreserved `A-Z a-z 0-9 - . _ ~`. That is stricter than
    `encodeURIComponent`, which also leaves `! ' ( ) *`.

  So a value cannot add a path segment, a query, a fragment or a scheme. That
  is what the route checker already assumes: PW5009 matches `/stores/{id}`
  against a route as one segment, and an unencoded `7/reviews` would have
  rendered a link the checker had accepted. A URL that is wholly a value stays
  `href={u}`, with its existing scheme check.
- **A `style` attribute**: refused. Escaping inside a CSS declaration is not
  decided.

### 5. kiokun's pages use them

- **`WordPage(word, entry: Option<Entry>)` replaces `EntryPage` and
  `NotFound`.** It is one page with `{#match entry}`. The host still answers
  404 for `None`, and the markup of each arm is the old page's.
- **`SearchPage` says when a query matches nothing**, with `{:else if q}`,
  and says nothing on the empty page.
- **Links are `href="/{hit.target}"`**, so `길거리로/에 나앉다` links to
  `/길거리로%2F에%20나앉다`, which the host's route reads (ADR-0041 §2).
  `href={hit.target}` was relative, and correct only because the search page
  sits at `/search`.

## Found while building it

1. **`{:else}` was a silent miscompile**, as the Context says.
2. **A secret in an attribute's hole checked clean.** `<a href="/pay/{key}">`,
   with `key: Secret<Payments>`, passed `pw check`. The hole was static text,
   and no privacy rule reads static text. Nothing leaked, because neither
   renderer interpolated attribute strings. But the check had judged a
   program it could not see into. It is PW5003 now, as `href={key}` always
   was.
3. **A name that resolves to nothing is refused only in a call.**
   `{nothing.here}` passes `pw check`, between tags or in an attribute, as
   `let x = nothing` does. The resolver checks calls and policy terms by
   design (`check.rs`, "the subject is narrow on purpose"). This is recorded
   in KNOWN_LIMITATIONS, not changed.

## Acceptance

- `compiler/pw-core/tests/template_blocks.rs`:
  - `{:else}` is the other branch;
  - `{:else if}` nests;
  - `{#match}` has one arm per case, with the payload bound and typed;
  - an attribute's holes are segments in their context;
  - a secret in a hole is PW5003;
  - each malformed block is refused with its code;
  - a well-formed page is clean.
- `runtime/pw-render/tests/branches.rs`:
  - one branch and one arm render;
  - a variant is not a condition and has no text;
  - a URL value is one component whatever it holds, and hostile values
    cannot add a scheme, a segment, a query or a fragment.
- kiokun's `WordPage` and `SearchPage`, through the host tests and the browser
  spec in three engines.
- Mutation controls: `scripts/template_mutations.py`, `just e10-templates`,
  sixteen mutants, each undoing one piece. The first is the lowering that
  dropped `{:else}`.

## Consequences

- The dev server's patch generator does not emit patches for the new parts.
  The store uses none of them, and kiokun's pages carry no script.
- The Marko adapter refuses `{:else}` and `{#match}`, as it refuses `{#if}`.
- The `pw-render` binary's JSON values have no variant form. A host that
  renders an `Option` passes a `Value::Variant`.
