# ADR-XXXX: a navigation keeps the layout, and shows the next page in it

Status: proposed by the integrator, 2026-10-09, under the owner's delegation
of 2026-10-02. The third of three steps to the parts two pages share kept in
place (NEXT, the DoorDash customer app's item 1): the build is named (the
build id's ADR), a page names its layout (the layouts' ADR), and a navigation
between two pages of one layout and one build keeps it. Builds on ADR-0280
(a navigation after a commit, read after it), ADR-0219 (a live document is
told), ADR-0122 (speculation), and the refusal ruling's announcer. Date:
2026-10-09. Milestone: E14.

## Context

- **The owner's Next.js bug** (ADR-0280's brief): save, navigate, and see
  the page from before the save, or the header mounted again. ADR-0280 keeps
  the page fresh by order: a navigation is a document load, after the
  commit, `no-store`. It keeps nothing.
- **What others do** (read 2026-10-09):
  - which clicks are a navigation's (Turbo 8, Astro 7, SvelteKit 3 agree):
    the primary button, no modifier key, not `defaultPrevented`, the nearest
    `a[href]`, no `download`, no `target` but `_self`, no `rel=external`, an
    opt-out attribute, the same origin, not a fragment of the same page;
  - the next page is a plain `GET`, its whole body read; a build or version
    mismatch is a full load (SvelteKit's `x-sveltekit-version`, Next's
    deployment id, Turbo's `data-turbo-track="reload"`);
  - Next.js keeps a shared layout and does not read it again (the bug);
    SvelteKit reruns a layout's `load` when what it read changed; Turbo
    keeps `data-turbo-permanent` elements; Astro `transition:persist`;
  - a navigation followed while another loads replaces it (Turbo,
    SvelteKit, React Router: "a new navigation cancels the pending one's
    fetches");
  - the History API (`pushState`, `popstate`, the scroll kept per entry,
    `scrollRestoration = "manual"`): the Navigation API is Baseline since
    2026-01-13, and the Firefox (146.0.1) and WebKit (26.0) that Playwright
    1.58 runs have none;
  - accessibility: focus moved to where the new content begins, a heading
    (Marcy Sutton's study for Gatsby, 2019), and the page named in a live
    region where it is not; never focus left on a removed node.
- **The runtime held one document**: every piece of its state read once,
  from the document's parts manifest, at module scope; its subscription had
  no handle to end it; and an index key carries the page's schema, so even
  the layout's parts are addressed anew on the next page. The host derives a
  document's next patches from what that document showed (its `Shown`), so
  the page must show what the next document renders.

## Decision

1. **Which navigations keep the layout**: a link's (the consensus above,
   `data-pw-reload` to opt out), a handler's `navigate Page(args)` after its
   commit (ADR-0280), and back and forward between the entries a navigation
   made. Each from a page that names a layout; any other is the browser's.
2. **The next page is read as a load reads it**: `GET` its address, `cache:
   no-store`, after each press made before the navigation is answered
   (ADR-0280's order), its whole body. A redirect's final address is the one
   pushed.
3. **Kept only where both say the same**: the response's `pw-build` is the
   build that served this runtime, and its manifest's layout (path and
   schema) is this page's. Otherwise, and on any failure before the swap, the
   address is loaded whole (`location.assign`; after `popstate`, a reload),
   with ADR-0280's rule for a whole load stopped.
4. **The swap**, one synchronous step:
   - the slot's content is the next document's;
   - each of the layout's regions the next document renders differently is
     taken from it, and each of the layout's elements carries its attributes
     (its captures among them), so the host's next patches meet what it
     derived them from;
   - its title, description, style and parts manifest are the next
     document's, and the regions its response filled after its body (ADR-0148)
     are added for the next runtime to settle;
   - its address pushed, the scroll recorded in the entry left; the scroll to
     the top, the fragment, or the entry's own on back and forward;
   - focus to the page's first heading, else its `main`, held focusable
     without a tab stop; where neither, its title said by the announcer.
5. **The page goes to the next document's runtime**: this one's life ends at
   once (one `AbortController`: every listener it added, its subscription,
   its keyed reads), and a runtime of the next document's is loaded
   (`/pw-runtime.mjs?document=N`), which reads the manifest now in the page:
   its signals at the next document's values, the layout's among them.
6. **A page that leaves applies nothing it is told**: a batch that arrives
   while a navigation is in flight, to keep the layout or to load the page
   whole, is not applied, so a reload or a missed patch cannot load this page
   again and stop the navigation.
7. **The reader's choice wins, the browser's first**: a link followed, or back
   and forward, while a navigation is in flight replaces it; a handler's
   navigation is not taken while another is (ADR-0280). And once the browser
   begins a load of its own (an address typed, a test's `goto`), a navigation
   in flight is abandoned, and nothing the page does loads another.

## Found

1. **A page shown again from the back-forward cache listened to nothing.**
   Hidden, its subscription ends as the browser ends its connections, and
   shown again, nothing asked again (`leaving` stayed set), so it was told
   nothing until reloaded. It asks again now, and a server that forgot the
   document answers with a reload. Found mapping the runtime for this ADR.
2. **A soft navigation's `pushState` or reload, after the browser began a
   load of its own, stopped that load** (Firefox's `NS_BINDING_ABORTED`):
   found by `e2e/accessibility.spec.mjs`, whose `goto` followed a soft
   navigation still loading its next runtime. The browser's load wins now
   (7), and a frame's reload waits for no one's load either.
3. **Three tests of ADR-0280's navigation raced the page they pressed.** A
   press made while the order's commit removed the cart's lines landed where
   the button had been, focused it, and clicked nothing (Firefox, under
   load): the order's answer, held by the test until that press, never came.
   The presses are made from the keyboard now. Found running the suite whole.

## Alternatives

- **One runtime, its state reset for the next document.** Every module-level
  binding the runtime reads from its manifest would have to be made
  rebindable and reset, and a missed one would hold the last page's state. A
  runtime per document, the old one's listeners ended by one signal, holds
  each document's state where it already is.
- **Morphing the whole document** (idiomorph's identity sets): the layout's
  identity here is the compiler's, the same part ids on every page that names
  it, and the slot's markers say where the page is; nothing need be guessed.
- **A cache of pages for back and forward**: a page kept from before a save
  is the owner's bug; every navigation is read.
- **The Navigation API**: not in two of the three engines the suite runs.

## Acceptance

`just e14-soft-navigation` records each (docs/evidence/E14/soft-navigation.txt):

- **`e2e/soft-navigation.spec.mjs`, in Chromium, Firefox and WebKit**:
  - a link to a page of the same layout keeps the header (a property set on
    its node survives) and the window, shows the page, its title, its
    heading focused, and the runtime is the next page's;
  - the owner's bug: the cart changed on the store's page, its link
    followed: the cart's line shown, the header kept and counting it;
  - back and forward each read the page again: a change made in another tab
    while away is shown;
  - the page shown is told what changes, and nothing of the page left is
    applied;
  - the next page's streamed regions are filled;
  - a handler's navigation after its commit keeps the layout;
  - a page with no layout, and a page of another build, are loaded whole;
  - a link opened with a key held is the browser's;
  - the layout shows what the next document read, though this page heard
    nothing;
  - what a leaving page is told is applied to nothing;
  - a page shown again from the back-forward cache listens again;
  - a reader's next choice abandons a navigation in flight;
  - the document left is asked for nothing more (its runtime's life ended:
    the first run of the mutation controls found that mutant alive, the
    leaving page's guard hiding it, and the old subscription asking the
    server for its document every few seconds).
- **`e2e/navigate.spec.mjs`** (ADR-0280), in three engines: its order placed
  goes to its page softly, read after the commit; its presses before the
  navigation answered first; a press while the page leaves not taken; a
  second navigation not taken; a whole load stopped, and one shown again from
  the cache, as before.
- **`scripts/soft_navigation_mutations.py`**: 12 mutants.

## Not claimed

- **A layout's loops and views that contain themselves** are taken from the
  next document whole: their instance tokens carry the page's path, so a
  region of them is never the same markup on two pages.
- **A view-transition animation**, **prefetching on hover**, and a progress
  indicator beyond `data-pw-navigating` on the root element, for a page's
  style to show.
- **The document left lingers on the server** until its subscriber is idle
  (120 s): there is no release yet.
