# ADR-0227: a value computed from a signal is the browser's

Status: accepted under the owner's delegation of 2026-10-02. It is ruling
0073-a's browser part (ADR-0210), after ADR-0226's host part, which the feed
reference app's draft needs (NEXT 24). Date: 2026-10-05. Milestone: E14.

## Context

- **ADR-0226 compiled a value a template computes from a query's value.**
  The host computes it when the page renders, and again when the value
  changes.
- **A value computed from a signal was refused at build**: "which the browser
  computes (ADR-0227)". A signal is the browser's (ADR-0130). It changes as a
  person types, and no host hears of it.
- **The feed needs two:**
  - what is left of a draft, `{280 - String.length(draft)} left`;
  - a post button `disabled` while there is no post to send.

  ADR-0225 left it out: "A count of what is left wants a computed hole."

## Research

- **Ruling 0073-a**: a computed hole is "computed by the host per render and
  by the browser's pure emitter (ADR-0044) when the inputs are signals or
  speculated values".
- **Qwik's `useComputed$`** is the closest model to a page that runs no
  script until something changes. "Signal reads in `useComputed$()` are
  tracked automatically", and it "will automatically reexecute when
  `name.value` changes" ([state](https://qwik.dev/docs/components/state/)).
  The server renders its first value; the browser loads its code when a
  tracked signal first changes.
- **One function, two compilations.** ADR-0044's pure backend (`js_pure`)
  and the component backend lower the same IR. A host's first value and the
  browser's later ones agree by being one function, as ADR-0130's renderer
  agrees with itself in the browser.

## Decision

1. **A text hole, and an attribute's whole value, may compute from one
   signal**, at the top of a page, a composed view's included.
   - The compiler lifts it as ADR-0226 does: `derived$3`, the component
     `feed.app.Home.derived_3`, read by the path `#feed.app.Home~3`.
   - The page's plan names it among the parts a signal decides (`live`): the
     signal, the path its value is read at (`draft`, or `panel.open` for a
     view given a field), the kind, an attribute's name, and the component.
     No part of the host's plan runs it again.
2. **The host renders its first value.** It runs the component with the
   signal's first value at that path, typed by the component's parameter as
   a command's argument is. So the page shows "280 left", and the button
   `disabled`, with scripts off.
3. **The browser computes it from then on.**
   - The build writes one module per page that computes from a signal,
     `computed/feed.app.Home.mjs`: each function as `js_pure` writes it, and
     `parts`, each part's by its number, decoding the signal's value from its
     wire form. An `Int`'s is `BigInt(j)`.
   - The document names it. The runtime loads it when one of those signals
     first changes, and sets the part's text, or its attribute in place, as
     it sets one that reads the signal as it is (ADR-0142).
   - The server serves only a module a page's plan names.
   - A module the browser's backend cannot write is refused at build: the
     value would stay at its first.
4. **The feed's draft says what is left, and its post button is disabled
   while there is no post to send**:
   `disabled={String.length(draft) == 0 | String.length(draft) > 280}`. A
   281-character draft cannot be sent from the button. A submit that comes
   another way still meets the page's own check, `post_text` (ADR-0225).
5. **Still refused at build, by name** (ADR-0228):
   - a value computed inside a block a signal decides, which the browser
     renders again whole;
   - one in a row, an arm or a view that contains itself;
   - one from a signal and a query's value together.

## Acceptance

- **`compiler/pw-core/tests/computed_signals.rs`, 3 tests:**
  - the feed's two parts: live, by component, read at `draft`, not the
    host's to run again; typed `string -> s64` and `string -> bool`; the
    module's functions by their parts; none for the thread page; and a
    module that did not compile refused;
  - an attribute, and a view given a field of a signal, read at
    `panel.open`; an `Int` decoded by `BigInt(j)`; and the same values read
    as they are computing nothing;
  - a value from a signal and a query, in a block a signal decides, and in
    one a query decides, refused by name, with a control.

  ADR-0226's `computed_holes.rs` says the home page's host computes nothing,
  where it said the page computes nothing, and its refusals name ADR-0228.
- **The development server's tests:**
  - `what_a_page_computes_from_a_signal_is_rendered_at_its_first_value`:
    "280 left" and the button disabled, the module named and served, none
    for the thread page, nothing outside the modules, and "275 left" from
    another first value;
  - `a_page_of_signals_alone_computes_at_their_first_values`: an `Int`'s
    first value doubled, and a view's given a field of a record's.
- **`e2e/feed.spec.mjs`, in three engines:**
  - `what is left of a draft is computed as it is typed`: with scripts off
    "280 left" and the button disabled; then "275 left" and enabled for
    "Hello", and back, without a reload;
  - ADR-0225's test: "-1 left" and the button disabled for 281 characters,
    the form's own submit sending nothing, and "0 left" for 280 emoji.
- **`scripts/computed_signals_mutations.py`: 16 mutants**, 13 against the
  compiler's and the server's tests and 3 against the feed in three engines.
  Recorded by `just e14-computed-signals`. `computed_holes_mutations.py`'s
  "a signal's value is not said to be the browser's" is retired: the browser
  computes it now, and "a signal's value is a query's to compute" stands in
  its place.
- **The workspace, 2,080 tests; the browser suite, 764 in three engines.**

## Not claimed

- **Where else, and from what else** (ADR-0228): a value in a block, a row,
  an arm or an instance; one from several values, a signal's and a query's
  among them; one from a value the page speculates on.
- **A value computed before its module loads.** The first change loads it,
  and the part shows its last value until it has: one request, once per
  page.
