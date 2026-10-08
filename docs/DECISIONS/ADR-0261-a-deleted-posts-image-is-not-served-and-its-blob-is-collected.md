# ADR-0261: a deleted post's image is not served, and its blob is collected

Status: accepted under the owner's delegation of 2026-10-02; ADR-0260's
first question at its merge. Date: 2026-10-07. Milestone: E14.

## Context

- **ADR-0260 served every blob it kept, and kept every one.** A committed
  image is served from the deployment's blob storage at its SHA-256, to
  anyone who asks, `immutable`. "Deleting a committed image" was not
  claimed, and ADR-0258's `delete` had merged first: a deleted post's image
  stayed served at an address its readers had been shown.
- **Delete promises its author the post is gone**, and its image is part of
  it.
- **A blob is the posts' together**: content-addressed, so two posts of the
  same bytes share one, and it goes when the last of them does.
- **A command in flight may have put the same bytes back**: `keep` puts a
  claimed lease's bytes in the deployment's storage before its post commits
  (ADR-0260), so a blob no committed post names may be one a post is about
  to.

## Decision

1. **A blob is served while a committed row names it.** The data layer says
   which (`DataLayer::names_blob`), handed to the uploads once, as the
   leases are handed to the layer, and held weakly, since the layer holds
   the leases. A layer that keeps no upload names none, so nothing a program
   never committed is served; one that cannot say is answered 503, and is
   never taken to say yes. In memory it is read under the feed's lock; on
   PostgreSQL it is one `EXISTS` on `posts.image_key`.
2. **A deleted post's blob is collected where no post names it**, once the
   deleting transaction has committed: the image of the post and of every
   reply under it. The memory layer reads whether a post names the key
   under the feed's lock, which it holds through the commit; PostgreSQL's
   reads it after `COMMIT`, while the command still holds the writes' lock.
   Either way no other command commits between the look and the delete.
3. **A claim in flight keeps its blob.** Under the leases' lock, a key that
   a claimed lease holds is not deleted, since its command's post will name
   it. A claim taken after the look is one whose `keep` puts the bytes back
   before its post commits. Deleting a blob already gone is no error.

## Found

- **ADR-0260's one local intermittent is found.**
  `on_postgres_an_image_is_kept_with_its_post` read every schema's
  `posts_image_whole` constraint, and PostgreSQL may evaluate
  `pg_get_constraintdef` before the condition that keeps its own schema's:
  on another test's constraint, as that test drops its schema, it fails
  "could not open relation with OID". This ADR's PostgreSQL test, one more
  beside it, made it fail here at once. The test finds its own constraint
  first now, in a `MATERIALIZED` CTE, and reads only that; five runs since
  have not failed.

## Acceptance

- **The server, in memory and on PostgreSQL** (`tests/uploads.rs`): two
  posts of the same bytes share one blob, which is served after the first
  is deleted and not after the second, and is then collected; the control,
  a third post of the bytes, served again.
- **The uploads module** (`uploads.rs`): serving asks the layer, none
  handed answered 404, a key named 200, a key no longer named 404 with its
  bytes still kept, a layer that cannot say 503; `collect` keeps a blob a
  post names and one a claimed lease holds, deletes it once given back, is
  no error twice, and refuses what is no key.
- **`scripts/uploads_mutations.py`, 8 more**: a blob served whether or not
  a post names it; a layer that cannot say taken to say yes; a claim in
  flight not keeping its blob; the memory layer collecting a blob a post
  still names, collecting none, and naming none; PostgreSQL's layer
  collecting none, and naming none. One of ADR-0260's is retired as
  equivalent: "a lease is served where committed images are, before its
  post". No post names a lease's bytes, and that is asked first; where a
  committed post's blob were lost and a lease held the same bytes, the
  mutant would serve the very bytes the post names. Recorded by `just
  e14-uploads`.
- **The whole workspace's tests**, and the chain on the push.

## Not claimed

- **A copy a browser or a CDN keeps** under `immutable` outlives the
  delete: a deployment's blob store purges its CDN as it deletes.
- **A blob put by a command whose commit then failed** is named by no post,
  so served to no one, and stays: nothing sweeps the store for blobs no row
  names.
- **Two hosts on one database**: each collects after its own commits, and
  knows its own claims alone (ADR-0246's second host is not claimed).
