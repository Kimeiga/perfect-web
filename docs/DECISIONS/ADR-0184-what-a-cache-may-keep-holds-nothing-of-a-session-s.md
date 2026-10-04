# ADR-0184: what a cache may keep holds nothing of a session's

Status: accepted under the owner's delegation of 2026-10-02. Date: 2026-10-04.
Milestone: E14, the audit's tenth gap: charter §15.6 tests 2 ("Static public
shell does not contain private cart data") and 13 ("Shared cache files
contain no session or secret fields"), read against the running store.

## Context

- **Both tests were partly met.**
  - **At build.** The compiler refuses a private value in a shared cache:
    PW5001, and ADR-0128's rulings.
  - **In `pw-resource`.** Its tests showed that its public cache holds no
    private value, but with synthetic values. The audit called that "partial".
  - **Test 2's "public shell" was taken to be none**, since the store's page is
    `cache private`.
- **What the running store keeps, and what it sends, had never been read.**
  Reading them found two defects, each one person's data going to the next
  person through a cache:
  1. **The store's page said nothing of how it may be kept.** It is one
     session's page, with that session's cart. Yet it went out with no
     `Cache-Control`.
     - A 200 response a cache is told nothing about may be stored and
       reused: "heuristic caching" (RFC 9111 §3, §4.2.2).
     - So a CDN set to cache pages would serve one person's cart to the
       next person.
  2. **A fresh session's cookie went on whatever it asked for first, a
     build's file included.** That covers the runtime, the renderer's
     WebAssembly, and a handler's module.
     - RFC 9111 §7.3: "the Set-Cookie response header field does not
       inhibit caching".
     - A cache that kept such a file would give that session to everyone it
       served the file to. They would all share one session, and one cart.
- **The usual answer.**
  - A personalized response says `Cache-Control: private`, which a shared
    cache "MUST NOT store" (RFC 9111 §5.2.2.7).
  - Neither a cookie nor `Vary: Cookie` makes a response private (MDN, HTTP
    caching).
  - What every reader shares is marked so: Next.js sends `public,
    max-age=31536000, immutable` for an asset whose name carries its hash
    (Next.js's `headers` documentation).

## Decision

1. **A response to a session's request is kept by no cache.** It says
   `Cache-Control: private, no-store`. That covers:
   - the store's page, whole or streamed, and the page that is not found;
   - a page of signals;
   - the stream of frames;
   - a command's answer, a read, and every control.

   What each means:
   - `private`: no shared cache stores it.
   - `no-store`: nor does the browser. A page gone back to is asked for
     again, and shows the cart as it is now, not as it was left.
2. **A file of the build names no session.** The runtime, the renderer, a
   handler's or a speculation's module, and the build's JSON carry no
   cookie. They are every reader's, and any cache may keep them. How long a
   cache keeps them is the deployment's to say.
3. **Tests 2 and 13 are read against the running store.**
   - **The setup.** Two sessions fill their carts and read their pages.
   - **The check.** Two caches are compared with what they hold when
     nobody pressed anything:
     - what the query runtime keeps for every reader;
     - each fragment the materializer keeps in its public partition.
   - **The result.** Each is the same in both cases. Neither names a
     session, holds a line of a cart, or holds the deployment's identity
     key.
   - **Test 2's public shell** is those: the store's values and its menu's
     fragment, which every reader's page is made from. The rest of the page
     is the session's, and decision 1 keeps it out of every cache.

## Alternatives

- **`private` alone.** A browser could keep the page and show it again on
  going back, with the cart as it was. That breaks read-your-writes, which
  the charter asks of the cart (§15.2).
- **`no-cache`.** A cache may still store the response; it only has to ask
  before using it. A stored copy of someone's page is still a stored copy.
- **`Vary: Cookie`.** A shared cache would keep one copy per session, which
  is still a shared cache holding each session's page (test 13).
- **A public shell, with the cart streamed in** (§15.2's "edge/shared
  materialization" for the store and the menu). This is a change of the
  store's placement, from `origin` and `cache private`. It is a later
  decision, for when a page is declared `cache shared`; such a page's
  response will follow its declaration.
- **No cookie until a session is needed.** That is already so for every
  file of the build. A document needs one: its cart is the session's.

## Acceptance

Recorded by `just e14-shared-output` in `docs/evidence/E14/shared-output.txt`:

- **The development server.**
  - A session's response is kept by no cache: the page whole, the page
    streamed, and the page that is not found.
  - A file of the build, asked for first by a fresh session, names no
    session.
  - What a shared cache keeps holds nothing a session put in it.
- **The browser, in Chromium, Firefox and WebKit**
  (`e2e/shared-output.spec.mjs`).
  - The store's page and a build's file, as a fresh visitor's request is
    answered.
  - Going back to the store after the same session's cart changed in
    another tab shows the cart as it is now, and a press works.
- **`scripts/shared_output_mutations.py`: 6 mutants.** They cover each
  header, a build file's cookie, a private query kept for every reader, and
  a cart materialized in the public partition.

## Not claimed

- **A page declared `cache shared`** is not served, since none is
  declared. A public response's caching on the wire waits for one.
- **A build's files** carry no caching headers.
- **The data layer's state** (each session's cart) is not a cache, and is
  not read here.
- **A browser that restores a page from its back-forward cache.** The
  engines here ask for the page again whatever its response says. `no-store`
  is what makes one that keeps pages ask too, and the header test holds it.
