# ADR-0166: the store and its items say what they are

Status: accepted under the owner's delegation of 2026-10-02. Date: 2026-10-03.
Milestone: E14. Part of the audit's eighth gap, charter §15.1's fields,
brought forward by ADR-0165. Also a correction to ADR-0165's browser-suite
claim.

## Context

- **WebKit did not paint the store until its slots were filled**
  (ADR-0165). WebKit's `LocalFrameView` holds a page's first paint, while
  the document is still parsing, until it has more than 200 non-whitespace
  characters of text, or more than 32 by 32 pixels of an image
  (`visualCharacterThreshold`, `visualPixelThreshold`, read on WebKit's
  `main` on 2026-10-03). The store said about 88 characters, so a Safari
  user saw nothing until the recommender answered, 1.2 seconds by §15.5.
- **The store said nothing about itself or its items.** Charter §15.1 gives
  `Store` a `description`, and `MenuItem` a `description` and a `price`;
  the domain had neither.
- **One data layer answers two programs.** The development server serves the
  canonical store and the benchmark's frozen copy (ADR-0156), whose `Store`
  and `MenuItem` keep their old fields. The engine checked a host's answer
  against the importing component's types field for field, in their order.
  So a row with a description would have been refused for the benchmark's
  store, and a row without one for the canonical store.

## Decision

1. **`Store` and `MenuItem` declare a `description`**, and the store's page
   shows each: the store's under its name, and an item's under its own. The
   development server's rows carry them, by the store's and the item's id.
2. **A host's answer is read through the type the importing component
   declares.**
   - A record's fields that the type does not name are not passed in.
   - A field it names that the answer lacks is refused by name: the
     host's record has no field `description`.
   - Fields are read by name, in any order.
   - Lists, options, results, variants, maps and tuples are read element by
     element.
   - Anything else is passed to the engine, which checks it as before.
   - It is the engine's (`pw_host::engine::project`), so every host gets
     it, not only the development server.

   A row may hold more than a program asks for, as a table holds more
   columns than one query selects, and the program is given exactly what
   its types declare.
3. **`price` waits** for a ruling on how a page shows money. The language
   has no way yet to turn `Money<USD>` into text such as "$4.50": no number
   is formatted except as a template's hole.

## Correction, found on the way

- **ADR-0165's browser suite ran on a build of an experiment.** To see
  whether the estimate's position changed when WebKit paints, the estimate
  was moved below the cart and the page rebuilt. The source was put back,
  but the page was not rebuilt before the full suite ran. ADR-0165's commit
  reported "518 passed" for a page the sources no longer described.
  - In that build the menu's loop was part 1.
  - In the committed page it is part 3, and two tests in `store.spec.mjs`,
    which named the loop's part by number, fail in every engine.
  - ADR-0165's own evidence (`just e14-slots`) builds first, so it is
    sound.
- **Now the suite refuses a stale build.**
  - `run.sh` records each source it built from, by its SHA-256, in
    `dist/sources.json`.
  - The Playwright config refuses a `dist` whose sources have changed since:
    "examples/store/app.pw changed since dist was built: run run.sh again".
    It does so as it already refuses one built with another runtime
    (ADR-0161). The keyed store's build is held to its sources the same way.
- **The two tests find the menu's tokens by where they are**, not by part
  number. A part's number moves whenever the page above it changes.

## Alternatives

- **A paragraph of fixed text**, as the streamed demo has: wrong for every
  store but one.
- **An image over 32 by 32 pixels**, the other way past WebKit's threshold.
  The store has no image yet, and text is what §15.1 asks for.
- **A record per program in the development server**, chosen by reading the
  build's WIT. That would move the projection into one host's code, where
  every other host would need it again.
- **Width subtyping in the component model.** The component model has no
  record subtyping; a host's answer must be lowered to the exact type. The
  projection is the host's adapter, before lowering.
- **Re-base the benchmark's store** to the new fields. ADR-0156 froze it so
  the canonical store could grow without moving the tasks' patches.

## Acceptance

- `runtime/pw-host/tests/host_answers.rs`, on the compiled `Store` and
  `Menu` queries:
  - an answer as declared is passed as it is;
  - an undeclared field, at the top or nested, is not passed in;
  - fields in another order are read by name;
  - a declared field the answer lacks is refused by name;
  - a list's rows, and a declared error, are read through their types.
- The development server's tests:
  - each store and each item says what it is, each store its own;
  - every test of the benchmark's store, served from the same rows.
- `e2e/slots.spec.mjs`, three engines. WebKit's two `fixme` tests from
  ADR-0165 are on, and pass:
  - the store is painted before its slots are filled;
  - an Add is answered before the recommendations come (test 17).

  Measured in WebKit 26 on this host: first contentful paint at 16-32 ms,
  with 299 non-whitespace characters, where it was 2527 ms.
- The browser suite, on a build its sources are checked against.
- `scripts/descriptions_mutations.py`: 8 mutants, recorded by
  `just e14-descriptions`. The last removes both descriptions and must fail
  in WebKit; removing either alone leaves the page above WebKit's threshold.

## Not claimed

- **`MenuItem.price`, `available` and `category`, and `Store.menu_version`**
  (§15.1). Price needs money shown as text (above). Availability on the page
  is the audit's first gap's remainder.
- **A page that says little, streamed, in WebKit.** Any page under WebKit's
  threshold is shown in Safari only when it has loaded. A compiler cannot
  know a page's text before its data. The store says enough.
- **A host's answer to a command's argument.** Projection reads what a host
  answers into a component, not what a component sends.
