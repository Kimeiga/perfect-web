# ADR-0203: a view that contains itself is an instance of its own template, made at run time

Status: accepted under the owner's delegation of 2026-10-02. It builds
ADR-0130's ruling 2 for a view that contains itself. Date: 2026-10-05.
Milestone: E14, the app layer the owner put before the AI benchmark. Its
first item's last part: views over types that contain themselves.

## Context

- **A view used in another is composed** (ADR-0136): its markup is written
  where it is used. A view that contains itself, a comment showing each
  reply as itself, would be written without end. ADR-0136 refused it by
  name (PW5020).
- **ADR-0130 ruled it an instance made at run time.** ADR-0194 and ADR-0202
  compiled the types such a view shows, so a reply thread's data existed and
  nothing could show it.
- **What other frameworks do.** React, Vue and Svelte render a component
  that uses itself at run time, as deep as its data, on the language's call
  stack.
- **A browser bounds how deep markup nests.** Read in each engine's source:
  - Blink: `kMaximumHTMLParserDOMTreeDepth = 512`
    (`html_construction_site.h`). Past it, `html_construction_site.cc` adds
    each element "as a sibling of the parent".
  - WebKit: `defaultMaximumHTMLParserDOMTreeDepth = 512`
    (`SettingsBase.h`). Past it, `HTMLConstructionSite.cpp` closes the last
    open tag and adds the element as a sibling of its parent.
  - Gecko: layout's `MAX_REFLOW_DEPTH` is 1026 on 64-bit desktop and 585
    on 32-bit Windows and Android (`nsIFrame.h`), and its tree builder stops
    appending elements past a threshold of its own
    (`nsHtml5TreeBuilderCppSupplement.h`).

  Past any of these, the DOM stops matching the server's markup, and every
  address in it with it.
- **A renderer that recurses with the data overflows its stack before
  then.** Measured in a debug build: rendering one instance inside another,
  a chain of 247 overflowed a 4 MiB thread, over 16 KiB a level. Rust's
  default for a spawned thread, and Tokio's for a worker, is 2 MiB.

## Decision

### 1. Which views are instances

- **A view that contains itself**, directly or through the views it uses, is
  an instance at every use. The others compose as before.
- **One with no block on the way back to it is refused** (PW5020): no
  `{#if}`, `{#match}` or `{#each}` encloses some cycle of its uses, so no
  data ends it.
- **One holds no signal yet**, and provides none (PW5020). An instance is a
  run-time scope, as a loop's row is (PW5307).
- **One shows no `<stream>` yet** (PW5020). A host runs the streams its
  page's plan names (ADR-0148), and an instance's are its view's: one would
  show its placeholder for ever.

### 2. Its template, and each use's part

- **The view is compiled once, into a template of its own**, as a page is.
- **Each use is an `instance` part**: the view's path, each parameter given
  the value path its prop is, how many elements enclose the use, and how
  many its view's template nests. The page holds none of the view's markup.
- **A template's schema closes over every template its instances reach.**
  An instance's parts are numbered by its view's template, so a change there
  changes what the page's addresses mean.

### 3. Rendering

- **An instance renders its view's template in a frame of its own**,
  `<!--pw:s{part}@{token}-->` .. `<!--pw:e..-->`, as a keyed loop's row
  does. Its token derives from the frames around it, so each instance's
  parts have addresses of their own.
- **It reads its parameters alone**, given the values its arguments read
  where it is used. It holds the page's capabilities and identity domain.
- **It is rendered after the markup around it, not on its stack.** The
  renderer writes a place for it and keeps what is open on a stack on the
  heap, depth first, so the document is written in order and an error is the
  first in it. A page as deep as its data takes the renderer no deeper.
- **A page nests at most 500 elements** (`NESTED_ELEMENTS`): 512, less the
  document's `<html>` and `<body>` and ten for a host's shell. An instance
  that would take it past is refused (`TooDeep`), not rendered for a
  browser to rearrange.

### 4. Where it is rendered again

- **An instance a query's value gives, at the top of a page**, is planned
  as a block: the host renders it, and again, whole, when what it renders
  changed (ADR-0146). Inside a block or a row, it is rendered with them.
- **One the page's signals alone give** is rendered again in the browser,
  whole, when one of them changes. One given a signal and anything else is
  refused: the browser holds the signals alone (ADR-0137).
- **A host sends each template the page's instances reach**: its parts, and
  the template, which the browser's renderer renders an instance in a block
  with.

### 5. The browser

- **Its index reads each frame in its template**: a loop's row in its
  loop's, an instance in its view's. An element's part is read in its own
  template's manifest, so a view's button numbered as one of the page's is
  not taken for it.
- **A range is paired by its path and its id**: a view inside itself opens
  a part of an id still open around it.
- **An instance's range is its frame**, at its part's address in the
  template around it. A change replaces it whole.
- **Each template's event parts are bound in its instances**, and each
  decided once, by its template.

## Alternatives

- **Compose to a fixed depth.** Every thread deeper than the guess is cut,
  and every shallower one carries the unrolled markup.
- **Recurse on the call stack, as React, Vue and Svelte do.** At over
  16 KiB a level, a 2 MiB stack holds fewer than 125, and a server would
  crash on a thread a browser shows.
- **Bound the depth by the data's, not the page's.** A level that nests no
  element nests nothing for a browser to rearrange. The page's nesting is
  what browsers bound.
- **A component per instance, at run time, in the browser.** Each would
  need the hydration E7 removed.

## Acceptance

Recorded by `just e14-view-instances` in
`docs/evidence/E14/view-instances.txt`:

- **`compiler/pw-core/tests/views_contain_themselves.rs`**, 10 tests:
  - a reply thread checks; one with no block on the way back, directly or
    through another view, one holding a signal and one showing a stream,
    are refused;
  - each use an instance, with its depth and its view's;
  - rendered to its data's depth, each instance in a frame of its own;
  - refused past what a browser nests, and the deepest chain allowed
    rendered on a 2 MiB thread;
  - a template's schema closing over the views its instances reach;
  - an instance a query gives planned as a block, one a signal gives as
    live, and one given both refused.
- **`runtime/pw-render/tests/properties.rs`**: an instance of an unknown
  template refused; an error inside an instance first in the document; one
  rendered alone the bytes the page has there; the templates a page reaches.
- **`runtime/pw-render-wasm`**: a block holding an instance rendered with
  the template its request carries, and refused without it.
- **`spikes/own-renderer/e2e/thread.spec.mjs`**, in Chromium, Firefox and
  WebKit, on `examples/demo/thread.pw`:
  - a query's thread rendered to its depth, every range, element and
    instance indexed;
  - a button three instances deep sending its own comment;
  - an outline a signal gives, expanded and shut, rendered again in the
    browser;
  - a block holding instances rendered again in the browser, and bound;
  - an instance replaced whole by a change, and bound.
- **`scripts/view_instances_mutations.py`**: 27 mutants.
- **Mutation scripts that moved** with the runtime's `bindEvent` and the
  plan, re-anchored: `command_answers`, `press_order`, `provide`,
  `runtime_recovery` and `query_blocks`.

## Not claimed

- **A signal in a view that contains itself**, held or provided, and a
  `<stream>` in one.
- **A change inside an instance kept in place.** A list inside one is
  rendered again with the instance, and what has focus in it loses it.
- **A value deeper than what carries it nests.**
  - A query's value is nested by its host from the component's nodes, at
    most 128 nodes deep (`NESTED_DEPTH`, ADR-0194): a thread 128 levels deep,
    under the 250 the page's bound allows two elements a level.
  - A signal's value reaches the browser's renderer as JSON, which
    serde_json parses at most 128 values deep: a thread about 60 levels
    deep, each level an object and a list.

  Holding such a value as its nodes outside a component, on the browser's
  wire and in the renderer, is ADR-0194's next step.
