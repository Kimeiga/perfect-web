# Stop the kitchen's request storm

The store page shows how long the kitchen takes right now ("Ready in 12
min"). A prep time is a promise to the customer, so the store never keeps
it: every page asks the kitchen, and asking the kitchen is slow. When a
promotion goes out and many customers open the page at once, the kitchen is
asked once for each of them, and it cannot keep up.

Change it so that:
- customers who open the page while the kitchen is being asked share that
  ask, rather than each asking again;
- a page still shows the prep time as the kitchen says it now: once an ask
  has answered, the next page asks again.

A test changes the prep time with `POST /bench/prep?minutes=…`, and reads
how many times the kitchen was asked at `GET /bench/calls`; those hooks are
already there.

Keep everything else working.
