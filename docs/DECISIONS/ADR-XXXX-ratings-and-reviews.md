# ADR-XXXX: a store is rated through its delivered orders, and its reviewers go unnamed

Status: proposed by track `reviews` (NEXT, "W9, ratings and reviews of orders
and stores, after accounts"), under the owner's delegation of 2026-10-02 (the
rulings delegated: research and decide). Date: 2026-10-14. Milestone: E14, the
DoorDash store's customer side. Based on `track/bounded-statements` (NEXT 4b),
so a store's rating in a list is one set read from the start. Builds on
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
     told in the program's words where the press was.
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
   customers said, each from a delivered order". The rate page tells the
   reader before they send: "Your comment is shown on the store's page,
   without your name."
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
       reviews by who wrote them, into what every reader is served:
       `examples/rejected/R-070`, refused with PW5001's cause chain.
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
     moves (`UserOrderChanged`) or is rated (`OrderRated`).
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

To be filled with the recorded counts at the end of the track:

- `tests/reviews.rs`, in memory and on PostgreSQL;
- `tests/bounded_statements.rs`, the four operations, and each page and
  command, pinned at the seed and with fifty stores more;
- `e2e/reviews.spec.mjs`, three engines;
- `scripts/reviews_mutations.py`, every rule's mutant killed;
- `examples/rejected/R-070`, the checker's refusal;
- `just e14-reviews`, writing `docs/evidence/E14/reviews.txt`.

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
