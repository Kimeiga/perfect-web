# Keep the store's notice fresh

The store page shows the notice the store has posted, under its name ("Open
until 7 pm"). Asking the notice board is expensive, so the store keeps its
answer for five minutes. When the staff post a new notice, customers see the
old one for up to five minutes, and that is too long.

Change it so that:
- a new notice shows on the store page within ten seconds of being posted;
- the notice board is still asked at most once in any ten seconds, however
  many customers open the page.

A test posts a notice with `POST /bench/notice?text=…`, and reads how many
times the board was asked at `GET /bench/calls`; those hooks are already
there.

Keep everything else working.
