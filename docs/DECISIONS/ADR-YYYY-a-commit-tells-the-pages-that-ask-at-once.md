# ADR-YYYY: a commit tells the pages that ask, and tells them at once

Status: accepted under the owner's delegation of 2026-10-02, on CI's
finding (the feed's "a follow reaches another reader", red in mutation
baselines on CI, 2026-10-09). Date: 2026-10-09. Milestone: E14.

## Context

- **"a follow reaches another reader of the page without a reload" failed
  on CI and never here**: in five recipes' baselines of run 37918808029 and
  three of 37893301747, in Chromium and Firefox, and in no browser job; on
  branches with ADR-XXXX's telling and without it, and with the earlier
  test's unfollow awaited and not.
- **Its records** (ADR-XXXX's trail, printed by the failing test, run
  37925722922): the reader who followed was told at version 607; the
  second reader's document had subscribed, and its streams opened and
  ended, and no telling reached it in five seconds. In another failure it
  was told at version 721, five seconds after the follower at 674.
- **A commit's telling went one session after another** (ADR-0219,
  ADR-0271): every other session with an open document reading what the
  commit dropped, in the order of their ids, each read and derived before
  the next. And **a document stays open for [`IDLE`], two minutes, after
  its page last asked**: the runtime says nothing when a page goes, and a
  closed page's document was derived at every commit, like a live one. By
  the follow test, late in the feed's file, every earlier test's pages were
  such documents, and the 47 versions between the two tellings were 47 of
  them told first.
- **This is not a test's problem.** A host whose page many read told the
  last reader of a change after every other reader, the closed pages of the
  last two minutes among them.

## Decision

1. **A commit derives only the documents whose pages ask.** A page asks
   without pause (a stream holds two seconds and is opened again at once, a
   long poll one): a document no page of which has asked for [`LIVE`],
   three seconds, is passed by, and marked so.
2. **A document passed by is told when its page asks**: before it is given
   its frames, it is derived against what it shows, in its session's hold,
   as a telling would have, and sent what changed. A page that comes back
   misses nothing; a page that never does costs nothing, and is forgotten
   at [`IDLE`] as before.
3. **A commit tells the other sessions at once**, up to [`TELLERS`] (8) at
   a time, each in its own hold as before (ADR-0219), one telling of a
   session at a time as before (ADR-0271).

## Acceptance

- **The development server's tests**: a reader is told while another's
  telling takes its time (the first in the order sleeps 400 ms in its
  derivation; the second is told first); a document no page asks for is
  passed by, and told what changed when its page asks over `/stream`, and
  once.
- **The browser**: the feed's tests of a change reaching another reader
  ("a follow", "a reply"), in CI's three-engine baselines.
- **`scripts/tell_at_once_mutations.py`**, recorded by `just
  e14-tell-at-once`: each part undone fails a test.

## Not claimed

- **A page that is open and does not ask**, frozen in a background tab or
  the back-forward cache, is passed by like a closed one, and told when it
  asks again: it is shown what changed then, not while it was frozen.
- **The order of the sessions told** is unchanged: at once, the order
  matters only past eight.
- **A beacon from a page that leaves** would forget its document at once;
  [`LIVE`] makes it cost nothing first. Not built.

## Alternatives

- **Forget a document at once when its page leaves**, by a beacon on
  `pagehide`: precise for a page that closes, and nothing for one that
  crashes, freezes or loses its network; a document passed by while it does
  not ask covers all of them.
- **A shorter [`IDLE`]**: a page that comes back after it is forgotten is
  told to reload; passing by costs it nothing.
- **A thread per session**: a host with many readers would start as many
  threads per commit; eight at once bounds it.
