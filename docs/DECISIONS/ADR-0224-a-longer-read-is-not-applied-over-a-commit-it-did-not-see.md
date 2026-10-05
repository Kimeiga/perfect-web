# ADR-0224: a longer read is not applied over a commit it did not see

Status: accepted under the owner's delegation of 2026-10-02. Found by
ADR-0222. Date: 2026-10-05. Milestone: E14.

## Context

- **A keyed read** (ADR-0152) reads a page's binding again for the key a
  signal gives it: the feed's "Load more" reads the timeline for a longer
  page.
  - It reads outside the session's hold, so a slow read keeps none of the
    session's commands waiting.
  - It then derives what the page shows from the value it read, and sends
    that.
- **A commit holds the session from its commit to what it sends**
  (ADR-0172).
- **So a commit could come between the read and its apply.**
  - The commit's change was sent first, the post included.
  - The read's value, from before the commit, was applied after it.
  - The page then showed the longer list without the post, until the next
    change.
- **ADR-0222 made the value a page speculates on follow what it shows**, so
  the race reached that too.

## Decision

- **A keyed read applies its value in the session's hold**, so no commit
  comes between what it checks and what it sends.
- **It reads again when a change reached its document while it read**, by
  the number of frames the document has been sent, before and after.
  - Up to three reads, the first two outside the hold. The last is read
    inside it, so nothing can reach the session between that read and its
    apply.
  - This is the serve path's rule for a document (ADR-0151).
- **A slow read still keeps no command waiting**, but for a third attempt,
  which only changes arriving through both reads before it bring about.

## Acceptance

- **`a_longer_read_is_not_applied_over_a_commit_it_did_not_see`**:
  - 21 posts by another session, the page open at twenty;
  - "Load more" asked;
  - the session's own post committed between the read and its apply (a
    test's hook, there);
  - the page then shows 23 posts, its own first.

  ADR-0152's keyed reads' tests pass as they did.
- **`scripts/keyed_race_mutations.py`: 1 mutant**, a read applied whatever
  reached its document, recorded by `just e14-keyed-race`.
- **The workspace, 2,056 tests; the browser suite, 752 in three engines.**

## Not claimed

- **Another session's commit between a read and its apply.**
  - It reaches this session through the telling after its answer
    (ADR-0219), which takes this session's hold and reads its documents at
    the key they show.
  - So the page is told of it then, and not by the read again.
