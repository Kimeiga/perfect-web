# ADR-0341: a store is rated through its delivered orders, and its reviewers go unnamed

Status: accepted under the owner's delegation of 2026-10-02 (the rulings
delegated: research and decide), with the integrator's rulings and answers of
2026-10-15; merged on train-9. Proposed by track `reviews` (NEXT, "W9,
ratings and reviews of orders and stores, after accounts"). Date: 2026-10-14.
Milestone: E14, the DoorDash store's customer side. Based on
`track/bounded-statements` (NEXT 4b, ADR-0336), so a store's rating in a list
is one set read from the start. Builds on
ADR-0318 (an order is a user's), ADR-0270 (the reader's user is the host's
handle), ADR-0324 (`user` says the data is the user's), ADR-0302 (a refusal
told where the press was), ADR-0328 (the hours' set read, its model) and
ADR-0329 (an order's parts, one per store).

## Context

- **The store had no ratings.** An order is placed, moved along by the
  kitchen (`/bench/order`) to `Delivered`, and forgotten; a store's page and
  the lists say nothing of what its customers thought.
- **What the store has**: an order is its user's, read by their handle alone,
  and the program sees only the latest (`UserOrders.current`). Its statuses
  are `Placed`, `Preparing`, `OnTheWay`, `Delivered`, `AwaitingPayment` and
  `Canceled`; there is no pickup. **An order may hold several stores'
  items**: its charges are recorded one part per store (ADR-0329), where a
  DoorDash or Uber Eats order is one store's.
- **The merchant, courier and support apps are out of scope** (NEXT), so
  nothing here replies to a review, rates a courier, or moderates.

## Research

The customer side, from each company's own help pages, read 2026-10-14
(DoorDash's consumer FAQ "User-Generated Content", its item-ratings FAQ, and
its merchant article on customer reviews; Uber Eats' "How to rate the items
in my order?", its merchant article on viewing reviews, its Ratings FAQ, its
Irish "Ratings and Reviews on Uber Eats" terms, and its Top Eats FAQ).

- **Who rates, and when.**
  - DoorDash: "only one verified review can be associated with each order";
    "You'll only be able to submit ratings for your most recent order",
    prompted "after your order is delivered or picked up". Content needs a
    signed-in account.
  - Uber Eats: "After receiving their order, customers get a prompt"; a past
    order is rated from Orders, "Rate the order".
  - **A window**: neither states one for a rating. DoorDash notes only that
    most customers rate within a day.
- **What is rated, and on what scale.**
  - Uber Eats: the store, "a star rating ranging from 1 to 5"; each dish a
    thumbs up or down, and "These ratings are not shown to customers"; the
    courier apart.
  - DoorDash: "Didn't Like", "Liked" and "Loved", which "replaces the
    previous 5-star ratings system for restaurant orders", aggregated into a
    lifetime rating that "appears as a number on a 1–5 scale"; how that
    rating is calculated "isn't shown and may change over time". Items
    thumbs up or down; the Dasher apart.
  - **Text**: optional at both ("a rating or review (which is not
    required)"; Uber's comment "can" be left). DoorDash publishes a review
    only at 25 characters or more.
- **Once, and editing.** DoorDash: one verified review per order, and
  reviews "cannot be edited". Uber Eats states neither.
- **How a store's rating shows.**
  - Uber Eats (Ireland): the average "is calculated from the ratings
    received over the last 91 days, from a minimum of 10 ratings received";
    its US FAQ describes a weighted average over four years and counts
    "rounded to the nearest 100 or 1,000".
  - DoorDash: the lifetime rating, on a 1–5 scale; a review "may not be
    publicly visible yet if the store has a low number of reviews".
  - **"New"**: neither page read defines a "New" label or when it shows.
- **The reviewer.** DoorDash shows a review "with the reviewer's first name
  and last initial", and "there is not the option to submit a review
  anonymously"; a reviewer may choose "Store feedback", seen only by DoorDash
  and the store, whose rating "will still be included in the average".
  Uber Eats shows customers no comments at all: "No, customers cannot see
  comments."
- **A rating filter or sort in search.** Neither help centre describes one
  for customers. Each has a badge instead: DoorDash's Most Loved (a banner
  and icon on the store page; stores "appear in search and discovery
  regardless of status") and Uber's Top Eats ("Must have a customer rating
  of 4.6 stars or higher"; shown "as Best Overall or Highest Rated").

## Decision

1. **A rating is a customer's, of a store, through a delivered order.** The
   reader may rate each store of their latest order once its status is
   `Delivered`: one rating per order and store, since an order here may hold
   several stores' (one per order at both companies, whose orders are one
   store's). The latest order only, DoorDash's rule, which is also all the
   program sees of a user's orders; no time limit, since neither company
   states one. Not `Canceled`, `AwaitingPayment` or any status before
   delivery.
   - *Alternatives*: one rating per order (which of its stores would it
     rate?); any past order (the program reads only the latest; a list of
     past orders is its own feature); a window of N days (neither company
     states one, and it needs the clock to close a form, a later step).
2. **What is given: 1 to 5 stars, and a comment if the reader wants.** The
   stars an `opaque type Stars = Int where value >= 1 & value <= 5`; the
   comment trimmed, at most 500 code points (`Comment`), none where empty.
   Courier and item ratings are not taken.
   - *Alternatives*: DoorDash's three choices (how they become a number is
     unpublished, so a store's number would be ours, presented as theirs);
     DoorDash's 25-character minimum (a moderation rule, out of scope); item
     thumbs (both keep them from customers or behind a badge; a step of its
     own).
3. **Once, and final.** A rating is not edited or withdrawn (DoorDash's
   rule). Held twice, as the store holds its other rules:
   - **`requires SignedIn, MayRate(store)`**, the deployment's predicate
     (ADR-0115, ADR-0302): the session's user's latest order is delivered,
     holds `store`, and has no rating of it, read through the command's own
     operations, so in its transaction, as `OwnsPost(post)` is. A refusal is
     told in the program's words where the press was. `SignedIn`'s words,
     "Sign in to place an order.", are the store's for a guest, a predicate
     being declared once per program, so `rate_order` reuses them: a guest can
     barely reach the rate page, and its press is refused all the same
     (*ruled by the integrator, 2026-10-15*).
   - **The data's own uniqueness**: `(order, store)` is the key on
     PostgreSQL, and memory refuses a second; the layer's `rate` checks the
     order again, delivered and holding the store, in the transaction.

   *Only the order's owner*: the order is read by the reader's handle, which
   the host makes from the session (ADR-0270), so no program or browser
   names another user's order.
   - *Alternatives*: editing (more state, a history the store would have to
     show honestly, and DoorDash declines it); the predicate alone (a race
     between two presses is the key's to refuse).
4. **How a store's rating shows**, the same words on its page, the home page
   and a search's results:
   - **No ratings**: "No ratings yet".
   - **Fewer than 10**: the count alone, "1 rating", "3 ratings". Uber Eats
     holds its average back below 10; an average of three is noise.
   - **10 or more**: "4.6 out of 5 · 12 ratings": the mean of every rating
     the store has, to one decimal, rounded half up, by the program
     (ADR-0327's rule that rounding is named where it happens), and the
     count exact. "out of 5" is said, not a star glyph, which a screen
     reader names oddly (ADR-0180's reason).
   - *Alternatives*: Uber's 91 days or a recency weighting (a value that
     changes as ratings age needs the clock and a re-read when one ages out;
     a later step); counts rounded to "500+" (the store's counts are small,
     and exact is honest); a "New" label (neither company defines it, and a
     store rarely rated is not new; the count says what is true).
5. **Reviews show on the store's page, without their reviewer.** The five
   newest ratings with a comment, each its stars and its words, under "What
   customers said"; each is of a delivered order, which decision 3 holds. The
   rate page tells the reader before they send: "A comment, if you like. It is
   shown on the store's page, without your name." Public, unnamed and
   unmoderated comments are accepted for the reference app (*ruled by the
   integrator, 2026-10-15*): there is no moderation, flagging or removal, and
   no merchant side (Not claimed).
   - **Why no name.** A user's data is private by default here: their
     identity reaches a program only as a handle the host makes (ADR-0270),
     and what a program reads with it is labelled theirs (ADR-0318,
     ADR-0324). The store has never shown a user to another: its account's
     name is the identity provider's claim, given to sign in (ADR-0258), not
     a profile the user published, unlike the feed's, whose handle and name
     are the point of posting. DoorDash's first name and last initial,
     with no way to stay anonymous, publishes personal data for trust; Uber
     Eats shows no comment at all. Pleris takes the middle: the words are
     public, by the reader's choice to write them; who wrote them is not.
   - **What holds it.**
     - **The data**: no operation of the store answers a user's name, and a
       rating's row holds no user: it names its order and store, and the
       order names its owner. The reviews' operation answers stars and
       words alone (a test, and a mutant adding the order's owner).
     - **The checker**: the store's ratings are a `public` query cached
       `shared`. Anything read through the reader's handle is labelled
       `User<UserId>`, and a shared cache refuses it (PW5001). So no program
       can mark "your review" among a store's reviews, or read a store's
       reviews by who wrote them, into what every reader is served. It is
       held by a test of the store itself, not by a corpus file:
       `the_checker_keeps_the_reader_out_of_a_stores_shared_ratings`
       (`tests/reviews.rs`) builds the real store, then the store with
       `StoreRatings` made to read `current_user()`, and the second is
       refused with PW5001's cause chain (`StoreRatings` is `User<UserId>`
       and declares a shared cache). The rule itself is the compiler's, tested
       in `compiler/pw-core/tests/shared_cache_holds_no_readers_value.rs`
       (`User<UserId>` among its cases); what this test adds is the store's
       own program, held to it. A corpus file is for a new invariant, and
       the ids R-070 to R-074 are train-7's (*ruled by the integrator,
       2026-10-15*).
   - *Alternatives*: DoorDash's name (personal data published by default);
     Uber's private comments (the store's page could not say what customers
     think, and a comment would go to a merchant app that is out of scope);
     a choice per review, DoorDash's "Everyone" or "Store feedback" (a
     second visibility to hold, for a store team that has no app here).
6. **The data, on both layers, read by set** (NEXT 4b; 4c refuses N+1 by
   construction soon):
   - in memory, `State`'s `ratings`, newest last; on PostgreSQL the table
     `order_ratings` (migration `0017_ratings.sql`), keyed `(order_id,
     store)`, its stars checked 1 to 5 and its comment at most 500;
   - **an order has an id on both layers**: PostgreSQL's `orders.id`, and in
     memory one each order is made, so "this order is rated" is a fact of
     the order and not of its owner;
   - **the operations** (`store:data/ratings`): `#for-stores`, each listed
     store's count and stars summed, **one statement for any number of
     stores**, which the home page, a search and a store's page read;
     `#reviews`, a store's newest five with a comment; `#for-order`, the
     reader's latest order's status and stores, each with its name and the
     reader's rating, one statement; `#rate`, the order read again and the
     row written, two;
   - each pinned in `tests/bounded_statements.rs`, at the seed and with
     fifty stores more, and each page's and command's count with them.
7. **The pages.**
   - The home page and a search: each store's words (decision 4), read for
     the listed stores at once beside their hours (`StoresOpen`,
     `Search`), dropped by a rating's `StoreRated(store)`.
   - A store's page: its words and its reviews (`StoreRatings(id)`, public,
     shared, dropped by `StoreRated(id)`).
   - The order page, once the order is delivered: for each store of it,
     "Rate Blue Bottle", or what the reader gave, read again when the order
     moves (`UserOrderChanged`) and when the reader rates (`rate_order`
     invalidates `OrderRatings` and `StoreToRate`, and emits
     `UserCartChanged(current_user())`, through which the store tells a
     command's own session: Findings).
   - **The rate page**, `/order/rate/{store}`: five buttons, "1 out of 5" to
     "5 out of 5", a comment, and "Send rating"; or, where the store cannot
     be rated now, why (not delivered, not of the order, or rated already,
     with what was given).
   - Reviews in new files (`examples/store/reviews.pw`, the data and
     queries; `examples/store/rate.pw`, the predicate, the command and the
     page, apart so the modules do not import each other in a cycle), and
     `spikes/own-renderer/server/src/reviews.rs`; each shared file touched at
     lines marked `TRACK SEAM (reviews)`.
8. **No rating filter or sort in search.** Neither company documents one
   for customers; each badges its best-rated stores instead. W8's `Reach`
   stays the search's one filter.
   - *Alternatives*: "4.5 and over" as a filter (a feature no source
     describes); a badge (DoorDash's and Uber's thresholds sit on more than a
     rating: cancellations, accuracy; a later step that can read this one's
     numbers).

## Acceptance

Recorded by `just e14-reviews` (`docs/evidence/E14/reviews.txt`, at 0d76d896
on the development machine; the verification run records it again on CI) and
by `just ci` (`ci: OK` at the branch's last code commit):

- **`tests/reviews.rs`: 14 tests**, each green in memory and on PostgreSQL. Each
  decision's rule has one: a delivered order's store is rated once and shown;
  not before delivery; not a store of another order; once for each store of an
  order; only the order's owner rates it; the layer holds its rules without the
  predicate; the latest order alone is rated; the mean said from ten ratings,
  half up; reviews show without their reviewer; the ratings of a list of
  stores are read at once; a rating is told to the pages that show it and to
  its rater's open ones; a comment is at most 500 characters; and the checker
  keeps the reader out of a store's shared ratings (decision 5).
- **`tests/bounded_statements.rs`**, its four tests green on PostgreSQL, the
  ratings' counts pinned equal at the seed and with fifty stores more:
  `#for-stores` 1, `#reviews` 1, `#for-order` 1 and `#rate` 2 statements; the
  home page 6, a store's page 18, a search 9, the order page 6 and the rate
  page 2; `rate_order` 8 in its transaction and 42 to read its reader's seven
  open pages again, which its cart's event regenerates (Findings).
- **`e2e/reviews.spec.mjs`: 6 passed**, two tests in each of chromium, firefox
  and webkit, in 3.1 minutes, on hosts of their own (`REVIEWS_PORTS`, PORT +
  200 to 202): the order page links a delivered order's store to rate, told
  unreloaded; the rate page refuses to send with no star chosen, takes four
  and a comment, and says what was given with its form gone; the store's page
  shows the review without its reviewer; another reader's open store page is
  told of the rating, unreloaded. The integrator accepted PORT + 190 to 192
  (2026-10-15); they are the payments hosts' providers' ports (Findings).
- **`scripts/reviews_mutations.py`: 30 of 30 mutants killed**, each by a test
  green on the unmutated program (baselines 14, 14 and 4 tests): 11 mutate the
  store's programs (all well-typed), 17 the layer (`reviews.rs`) and 2 the
  store's memory (`store.rs`); 25 are killed by the memory suite, 4 by the
  PostgreSQL suite and 1 by the statements' count.
- **The whole development server**: 579 passed, 0 failed, 9 ignored, in memory
  and on PostgreSQL alike. `just ci` is `ci: OK`: 2,901 tests passed in the
  workspace run, clippy with warnings denied, formatting, the anchors of all
  2,924 mutants, the evidence gates and the corpus.
- **The committed evidence the store's pages and commands move**, regenerated by
  its recipes and additions only: `just e10-handlers` (seven handlers, the rate
  page's five stars, its comment and its send), `just e10-component` (the
  store page's plan, whose ids shift by one from the new line; one component,
  `StoreRatings`, and `StoresOpen`'s, which reads the ratings),
  `just e8-contracts` and `just e8-wit` (eleven contracts and worlds),
  `just materialize` (six nodes and thirteen edges), `store-ir.json` and
  `store-values.json`.
- **Not yet run when this was written**: the three mutation scripts whose
  anchors this track's seams re-anchored (`descriptions`, `store_hours`,
  `store_search`) whole, here; the verification run plans them, since a changed
  line is within 30 lines of their mutants.

## Not claimed

- **Moderation**: a comment is shown as written (escaped, as all text is),
  with no review, flagging, removal or merchant reply (the merchant and
  support apps are out of scope).
- **Item and courier ratings**, and a list of past orders to rate from.
- **A window, recency weighting, rounded counts, a "New" label, a badge, a
  rating filter or sort** (decisions 1, 4 and 8).
- **What the checker cannot see**: a host operation's string is not a
  handle, so the checker holds what is read *through* a handle, not a user's
  id a data layer might answer. The store's has none to answer; a test and
  a mutant hold that.

## Findings

Found while building; corrections first.

- **Corrected: PW5001's test is the compiler's, not the corpus's.** The first
  plan named a corpus file for the checker's refusal on the claim that PW5001
  has one (R-004). R-004 is PW0100 and R-045 is PW5101; the corpus has no
  PW5001 file, and `compiler/pw-core/tests/shared_cache_holds_no_readers_value.rs`
  holds the rule, `User<UserId>` among its cases. The store's own test
  (decision 5) holds the store to it instead.
- **An opaque type's invariant is not held on a command's arguments**
  (*taken by the integrator to its own track, 2026-10-15*).
  `rate_order(store, stars, comment)` takes `Stars` and `Comment`, opaque
  types with `where` clauses; the component decoded 6, 0 and a 501-character
  comment without refusing them, and they reached the data layer
  (`store:data/ratings#rate`), which refused them by its own check (and
  PostgreSQL's `CHECK`). Whether a value a browser sends for an opaque type is
  checked at the boundary is ADR-0179's and ADR-0331's question. This layer
  keeps both checks, their tests (`a_comment_is_at_most_five_hundred_characters`,
  `the_layer_holds_its_rules_without_the_predicate`) and mutants, so it is
  right whichever way that is ruled.
- **A command's own session is told only through its cart's entry.** The store
  re-sends its author's open documents when the session's cart entry
  regenerates (`drain_held`, ADR-0218's `session_entry`); events and
  `invalidates` drop entries, and `others_reading` excludes the author. A
  command that changes no cart therefore emits `UserCartChanged(current_user())`
  to have its reader's pages read again, as `choose_tip`, `apply_promo` and
  `remove_promo` do, and now `rate_order` (a test and a mutant hold it).
  The program cannot say "and my own open pages"; it names an event that
  means something else. This bears on NEXT's one telling path for every page
  a change reaches.
- **`pw check` accepts what `pw build` refuses** (ADR-0137): a signal read
  inside `{#if server_value}`. The first build of the rate page failed 131
  of the server's tests, each serving the store; the form is `hidden={..}`
  instead of in a block.
- **The static render needs a value for each new page expression, and the
  recipes hide its refusal.** `spikes/own-renderer/store-values.json` holds
  the canonical store's values; `run.sh` stopped at step 3c on the first
  missing one (`store.rating`), then the next: fourteen keys and a field of
  each listed store, across the home, store, order and rate pages (the rate
  page's `aria-pressed` and `chosen_said(stars)` parts among them). The
  recipes' pattern, `BUILD_ONLY=1 bash spikes/own-renderer/run.sh
  > /dev/null 2>&1;`, goes on to run a browser suite on the half-built
  `dist`. `just e14-reviews` says when the build failed.
- **Two port groups can share a port, and nothing names it.** Each payments
  host's provider is `port + 10`, set in an env value, so a group at
  `PORT + 190` (this track's first choice, which the integrator accepted)
  shared three ports with it. Playwright starts every group's server for any
  spec, so every run failed at start with "already used", whatever it ran,
  and with `PW_REUSE` it would have driven the wrong server silently. Moved to
  `PORT + 200`.
- **PW5106 shaped the model.** A command that writes a resource must
  invalidate every query reading it (or emit an event it listens to); ratings
  written under `database.write<Orders>` would have obliged `Order`,
  `Checkout` and `OrderTotal` to be dropped. So ratings are a resource of
  their own, `Ratings`, a module as `Orders` is.
- A capability argument names a type, an opaque type or a MODULE, and must be
  imported (`import Orders`).
- **In memory an order had no id** (one per owner, replaced), where
  PostgreSQL's `orders.id` is its key. Added (`State.orders_made`), so "this
  order is rated" is a fact of the order on both layers.
- **Pre-existing, not fixed**: memory's `place` without charges (the
  benchmark's `orders#place`) keeps the previous order's `charged` row where
  PostgreSQL's new order has no parts. No store program reaches it.
- A command's `Ok` answer reaches the page without its value (ADR-0157); a
  `requires` refusal returns `Err("authorization predicate `X` denied the
  invocation")` from `command_answered`, and the route's 403 carries the
  words.

## Next

- A list of a reader's past orders, from which to rate; item and courier
  ratings; a store badge reading these numbers; a window or recency
  weighting with the clock's help; moderation with the merchant and support
  apps (each in Not claimed).
- A boundary check of opaque arguments (the integrator's track), after which
  the layer's own range checks become a second line.
- One telling path for a command's own session, so that `UserCartChanged` is
  not named by commands that change no cart.
- A check, when the Playwright config loads, that no two hosts share a port.

## The integrator's answers

On train-9, 2026-10-15, to the track's questions, and what the merge did:

- **Numbered ADR-0341**, merged on train-9 from `2190221c`. `ADR-XXXX` reads
  `ADR-0341` in the store's programs (`Ratings.pw`, `StoreData.pw`,
  `app.pw`, `rate.pw`, `reviews.pw`), the layer (`reviews.rs`, `store.rs`)
  and its tests, the migration, the recipe, the mutation script and the
  spec.
- **The reviews' browser hosts are PORT + 200 to 202**, accepted. The
  acceptance of PORT + 190 to 192 (Acceptance above) is withdrawn: those are
  the payments hosts' providers' ports (Findings).
- **The three mutation scripts the seams re-anchored** (`descriptions`,
  `store_hours`, `store_search`) are run whole by the train's verification
  run, which plans each of them; that run is their record. The track's own
  verify (38781487357) ran their recipes green already, with its 55 others.
- **Reviews are public and unmoderated, with no merchant side**:
  KNOWN_LIMITATIONS says so, with the rest of Not claimed.
- **The track's own verify (38781487357) failed its three browser jobs**,
  each on the same two tests: two pinned shapes of the store's page, the
  accessibility tree `accessibility.spec.mjs` compares and the count of
  identity anchors `store.spec.mjs` holds, still described the page before
  its rating line and its reviews. Neither suite is among those the track
  ran. Both are brought to the page as it is on the train, in a commit of
  their own, and run there in the three engines.
