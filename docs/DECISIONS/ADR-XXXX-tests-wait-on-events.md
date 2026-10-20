# ADR-XXXX: a test waits on an event, never on a timer

Status: proposed by track `flake-hunt` (2026-10-20) for the integrator; the
local gates and the track's own runs are recorded below; not yet in a train.
Date: 2026-10-20. Milestone: E14, the evidence's own harness (ADR-0281,
ADR-0344, ADR-0352, ADR-0297).
Track `flake-hunt`, on `e4847f6f` (`track/train-11`'s head when the track was
finished, merged 2026-10-20; the track began at `af5c1758`), not on `master`.

## The rule

**A test waits on an event, never on a timer.** What a test needs to have
happened before its next step is a fact the code under test can state: a
read is held at a gate, a response carries a marker, a subscriber's trail has
a note, a lock found itself held, a counter moved. The test waits for that
(with a cap on a hang, and only on a hang), and it asserts on what it heard,
not on how long it took. A wall-clock window decides no verdict.

Every wall-clock wait in a test is one of three things. Each has a fix, and
only the third is left as it is:

| | what it is | the fix |
|---|---|---|
| (a) | **an event awaited by a timer**: `sleep(300)` then "it has happened"; `recv_timeout(500)` then "it has not"; "give the write 100 ms to return early" | wait for the event itself: an explicit signal the code exposes, a test-only hook where it needs one, and no more than one. A *negative* window ("it has not returned yet") becomes a positive report ("it found the guard held", `Order::waits`, `Gate::held`). |
| (b) | **time is the property measured**: "the shell arrives within 150 ms", "eight pages take under 1.2 s", "a delay slows what it names" | never raise the bound. Measure the cause instead (the delay was taken: `delays_taken`; the reads were in the kitchen together: `Gate::held(8)`), or hold the source at a gate so that "before" and "after" are the test's own steps. Where the duration is the property (a CPU ceiling), bound the fastest of several checks (`check_latency`). |
| (c) | **a safety cap**: a 10 s `recv_timeout` on an event that must come, a `Gate`'s 10 s, a 30 s test timeout | leave it. It caps a hang and asserts nothing about speed. A cap sized as a multiple of the thing it waits for (`SHORT * 5`) is a (b) in (c)'s clothes: it is sized as a hang bound. |

A wall-clock window a **product** needs (`LIVE`, `order::WAIT`, a budget) is
tested at the instant the test means (`seen` set to `now - LIVE - 1 ms`; a
window of an hour that the test ends itself), and is a fixture parameter
everywhere else.

**A fix keeps what the timer distinguished.** The event a test waits for must
tell the code from its mutants as the timer's outcome did, so a test changed
under this rule is checked by its mutation script, run whole (three of this
track's own rewrites let a mutant live: see Corrections).

## Corrections first

- **The top flake in the brief's evidence is already fixed on the base.**
  `check_latency` was the most frequent flake of the week (at least nine reds:
  GitHub's ARM runner once, the PC's `just ci` eight times), and
  `track/ci-diagnostics` (merged into `train-11`, `526ccf66`) bounds the
  fastest of seven or more checks. Measured here, on the unchanged test
  binary of `af5c1758`: **8 of 8 passed** at load averages of 106 to 137
  on 4 CPUs beside 12 busy loops (the check took 7.8 s of wall clock, 1.95 s
  of CPU in the median and 1.85 s in the fastest: 282 ms a thousand lines
  against the 800 ms ceiling). It is ranked 1 below as fixed, not as mine.
- **Most of the week's failed CI runs were not flakes.** Of 30 failed runs
  from 2026-10-13 (`ci-failures-2026-10-13-to-20.md` in the track's evidence,
  every failed job's log read): 14 `ci` runs were red on `licenses and
  advisories` (cargo-deny, repaired by train-8's Wasmtime), one on a runner
  that was lost (`order-total`, step 9 never concluded, no log), one on
  `check_latency` (ARM), and one `Fallow` on a setup step. Of 13 `verify`
  reds, **6 were deterministic** (a surviving mutant or a red baseline that
  repeats: `e14-cart-lines`, `-orders`, `-store-accounts`, `-page-policy`,
  `-evidence-integrity`, `-layouts`, `-descriptions` ×3, `-accessibility`;
  repaired by later commits), **6 were the week's flakes**, and train-6's
  had both (`store_scheduled::an_order_swept_is_told_to_its_readers_open_page`
  failed in two recipes of one run: one of the 41 tests of section 1). The
  flakes:
  `e14-bounded-statements` ×3 (10-16, 10-17, 10-18; the 10-17 nightly also
  WebKit's `controls.spec.mjs:31`), WebKit's crash on a navigation (10-18),
  Chromium's `scheduled-orders.spec.mjs:74` (10-18) and WebKit's
  `feed.spec.mjs:176` `newPage` (10-19).
- **The wall-clock window that decided most was none of the tests' own.**
  `LIVE`, three seconds (ADR-0297), is how long a document's page counts as
  asking. A test that serves a page and, some steps later, commits, assumes
  the page is still asking; a slow machine ran the three seconds out, the
  document was "passed by", nothing was derived for it, and the test failed
  for the speed of its machine. With `LIVE` at 30 ms, **41 of the 662 server
  tests that run fail** (the list is in the track's evidence); with this
  change and `LIVE` at 30 ms, **none**. See 1.
- **The flake of `a_flight_another_reader_waits_for_is_not_stopped` was not
  the flight.** The test asserted that no read of the category had been
  stopped, at its end. The runtime's trace of two reds (bounded-race's, in
  `just ci`, and one that this track's stalls produced) shows the first
  flight, Hot's, ending as it should for `b`, and then **a second flight of
  Hot begun with no new `Subscribed` event and let go of at once** ("no
  subscribers remain"): the read `b`'s page makes again once the command has
  committed, most likely, begun after the first flight was over. Whether its
  source sees the cancel before it returns is the machine's timing, so a gate
  on the first flight did not make the end assertion deterministic (1 of 34
  seeds failed with it). The test now asks the held read twice whether it is
  stopped, with its gate closed (`Gate::polled`), and asserts there: the
  flight asked about is the first. That a second read of a slow source is
  begun and let go of at once after a command is a finding for the
  integrator (the trace is in the progress file); it was not changed.
- **A test that builds two hosts minutes apart and compares a value that
  holds the clock fails when the pair straddles the stores' local midnight.**
  `what_a_shared_cache_keeps_holds_nothing_a_session_put_in_it` failed in this
  track's own run at 07:00Z, 2026-10-20, midnight in America/Los_Angeles: the
  store's status ("open until ..") is the next local midnight's, and the two
  hosts had taken the wall clock when each was built. Both hosts keep one
  instant now (`Server::epoch`). Other tests that compare two hosts' values
  would fail the same way once a day for a few minutes; none was seen.
- **A `waitForFunction` that reads `window.__pw` met a page between two
  documents.** `lazy-handler.spec.mjs`'s two refusal tests press a handler
  the build refuses, and the page is read again once; a pause of 400 ms and a
  read of `window.__pw.log` landed in the gap on a loaded machine
  (`window.__pw is undefined`: the Firefox flake the host map's track
  recorded at line 191). Found by the first version of the fix, which met it
  3 times in about 200 runs.
- **A mutation control caught this track's own first rewrite.**
  `scripts/command_order_mutations.py`, run whole on the tree where
  `order::tests::a_command_still_here_is_kept_past_the_bound` waited for its
  follower to begin to wait (where it had slept two windows and asked whether
  the follower had ended), killed 19 of 20: "a command in flight is let go
  past the bound" lived. A command let go of while it ran is waited for as one
  *missing* (a wait with a window), and answered all the same when it ends,
  so "the follower has begun to wait" is true of the code and of the mutant.
  The test now asserts what the wait is for (`Order::waits_for_arrival`, a
  test-only counter told before `waits`): the mutant fails it in 6 runs of 6,
  the code in none (and the order's 8 tests passed 240 of 240 under stalls).
  **A fix keeps what the timer distinguished**: the event waited for must tell
  the code from its mutant as the timer's outcome did, and the test's
  mutation script, run whole, is how one finds out.
- **It caught a second.** `scripts/streams_mutations.py`, run whole, killed 25
  of 26: "the response is not ended with its last region" (the server's
  `shutdown(Write)` removed) lived. The test that killed it,
  `a_region_past_its_budget_is_given_the_host_s_failure_when_its_budget_is_spent`,
  had measured `ended < 2500 ms` on a source of 3 s; the rewrite held the
  source at a gate and measured nothing, and without the shutdown the
  connection closed only when the gate's cap ended the source's thread (the
  handler waits for its sources): the test passed, in eleven seconds. It now
  lets the source go when the close is heard, and asks the gate whether the
  source was still held then (`Gate::waiting_now`): 2.3 s on the code; the
  mutant fails it 3 runs of 3. (`fetched_stepwise` tells its callback once
  more, last, when the connection closes.) A cap that ends a test on its
  normal path is a smell: the first rewrite took ten seconds, and said so.
- **And a third.** `scripts/stream_ack_mutations.py`, run whole, killed 1 of 2:
  "the stream acknowledges what it wrote" lived.
  `a_frame_a_stream_wrote_to_a_dropped_connection_still_reaches_its_page` had
  slept 300 ms after the change before it looked at the subscriber, a sleep
  that stood for "the stream has gone round its loop again" (the pass that
  acknowledges what it wrote, when it is wrong); the rewrite read the bytes
  and looked at once, before that pass. Each pass marks the subscriber seen as
  it begins, in the one hold of the table that goes on to look at what the
  subscriber holds, so the test now waits for `seen` to have moved since the
  bytes came: the mutant fails it 6 runs of 6, the code passes 5 of 5, and the
  script, whole: 2 of 2 killed.

## 1. A test's page keeps asking (`LIVE`)

- **Cause.** `Subscriber::live` is `now - seen < LIVE`, `LIVE` three seconds
  of the wall clock; `installed` and `subscribed` set `seen` to now. In
  production a page asks without pause (a stream holds two seconds and is
  opened again at once). In a test nothing asks again.
- **Change.** `Subscriber::asked_at()`: now; in a test's build, an hour on.
  `installed` and `subscribed` call it. `Subscriber::at` is unchanged
  (`a_session_is_forgotten_with_its_last_document` builds subscribers at the
  instants it means), and so are the tests that age `seen` on purpose
  (`a_document_no_page_asks_for_is_told_when_one_asks`, the idle limit's).
- **Test and control.** `a_page_served_is_asking_for_as_long_as_a_test_takes`
  (a served and a subscribed page are live a hundred windows on; a page aged
  to the instant meant is not). Mutant 7 of `scripts/tell_at_once_mutations.py`
  (the test build's offset removed) fails it.

## 2. What the tests wait for now

A held source, as the recommender was (ADR-0223): `Gate`, which counts the
reads waiting at it (`held(n)` blocks until n do) and lets a stopped read win
over an opened gate. The menu's slow category, a session's estimator and the
kitchen can be held at one.

| test | waited on a timer for | waits for |
|---|---|---|
| `the_store_sends_its_slots_after_its_own_content` | delays of 200 and 600 ms; the document within 150 ms | the document in the response with both sources held; then the estimate's arm with the recommender still held; then the recommendations |
| `a_streamed_region_follows_its_document_in_the_same_response` | a delay of 400 ms; the document within 300 ms, the arm after 350 | the document and the runtime heard with the source held; the arm after |
| `a_region_past_its_budget_is_given_the_host_s_failure_when_its_budget_is_spent` | the response within 2.5 s of a 3 s source | the failure arm of a source that never answered, and the close heard while that source is still held (`Gate::waiting_now`) |
| `under_cancel_..`, `under_supersede_..`, `under_keep_..` (and `hot_then_cold`) | Hot's second, Cold asked 150 ms after it, reads within 500 and 700 ms | Hot held at a gate, `held(1)` before Cold is asked; stopped reads counted; a `keep` read telling that it found the turn held (`held_turn`) |
| `a_flight_another_reader_waits_for_is_not_stopped` | Hot's second, the steps of the scenario inside it; "nothing was stopped" at the end | Hot held for as long as the steps take; `b`'s read joined (the trace) before `a` moves on; the held read asked twice whether it is stopped, the gate closed, and asserted there |
| `a_browser_that_leaves_lets_go_of_its_read` | the browser gone at 150 ms, the read within 700 ms | the browser gone when the read is held; the read ended with the gate closed |
| `a_reader_is_told_while_another_readers_telling_takes_its_time` | a derive that sleeps 400 ms | a derive held until the other reader's trail says it was told |
| `a_forced_reconnect_ends_a_subscription_..` | a window of 800 ms the steps inside it outran; sleeps of 100 and 800; "ended within 300 ms" ×3 | a window of an hour the test ends; the stream open when its trail says so and cut when its trail says that |
| `a_frame_a_stream_wrote_to_a_dropped_connection_..` | a sleep of 300 and a read of 500 ms | the bytes |
| `a_stream_drops_what_its_page_says_it_applied` | a sleep of 100 ms | the stream's trail |
| `a_pages_command_..` (three) and `order::tests` (five) | a window of 2 s; sleeps of 100 and 300 ms; an answer within 150 ms and one within 1 s; a hang cap of 1 s on a 200 ms wait | a window of a minute; `Order::waits` (a command found its predecessor not yet run); `Order::waits_for_arrival` (and what the wait is for: one in flight, or one missing) |
| `pages_read_at_once_..` (two) | eight pages within 1.2 s | the kitchen held: eight reads in it together; or seven joined to the first (the trace) |
| `a_delay_slows_what_it_names_and_nothing_else` | three renders within a second | the delays taken (`delays_taken`), with the lower bounds kept |
| `on_postgres_a_render_past_the_bound_waits_for_a_seat` | 500 ms of silence | the seats say a render waits (`Renders::waiting`) |
| `what_a_shared_cache_keeps_holds_nothing_a_session_put_in_it` | the wall clock at two builds minutes apart | one pinned instant (`Server::epoch`) |
| `test_a_heartbeat_says_what_a_recipe_is_doing..` (Python) | a sleep of 0.3 s | the first beat |
| `lazy-handler.spec.mjs` (two), `recovery.spec.mjs`, `keyed.spec.mjs` (two) | pauses of 300 to 400 ms | the runtime's log line; the host's count of reads begun |
| `navigate.spec.mjs` "a press made while the page leaves is not taken" | a hold of 500 ms on the order's page that the press had to land in | the order's page held until the press has been made |

## 3. A browser host ends with the process that started it

The check-is-build worker's finding, put to this track by the integrator
(2026-10-20): a run's timeout leaves the run's servers behind.

- **Cause.** A mutation script's bound kills the Playwright run's process
  group with `SIGKILL` (`os.killpg(p.pid, SIGKILL)`; the same `bounded()` is
  copied into 107 scripts, `BOUND` 900 to 1,800 s; `e14-refusal`'s is 1,200).
  Playwright stops the hosts it started (`webServer`) in its own teardown,
  which a `SIGKILL` does not run, and each host is the leader of a process
  group of its own (`detached: true`, so that Playwright can kill a host's
  tree): the group kill did not reach them. At load 90-100, 52 `pw-dev-server`
  hosts kept their ports after one bound, and every later run on them failed
  `http://localhost:11141 is already used`: three mutants REFUSED (no result)
  and the restored suite ran no test. The validity rules held (no false
  kill), and the cost was a campaign to rerun. A start timeout is not the
  same: Playwright's plugin tears itself down when its wait fails
  (`webServerPlugin.js`, `setup`: `await this.teardown(); throw error`), and
  the task runner tears down the plugins set up before it. What leaks is
  a runner that is killed, by anything: the bound, the out-of-memory killer,
  a cancelled job.
  `run_suite.sh` sets no timeout; the only bounds are the scripts'.
- **Change.** Every host is run by `spikes/own-renderer/serve-while-owned.sh`,
  which ends its host when its standard input ends. Playwright starts each
  `webServer` with a pipe for its standard input and holds the pipe's other
  end for as long as it runs (`stdio: ["pipe", "pipe", "pipe"]`); the kernel
  closes it when the runner is gone, however it went. No polling for the
  owner's process (a killed owner that is not yet reaped still answers
  `kill -0`), no helper binary, no Linux-only call. `host-plan.mjs`'s
  `ownedCommand` builds the command, every word quoted, and
  `serversForSelection` uses it for every host: the configuration, and the
  hosts' environment, are unchanged. The launcher exits with its host's
  status, as soon as its host does, so a host that cannot start still fails
  the run at once.
- **Not changed: the 107 reapers.** A `SIGINT`-first reaper (Playwright's
  runner handles it, and stops its hosts) would be one mechanical edit in 107
  scripts with no test of its own, and would not cover the out-of-memory
  killer or a cancelled job. A host that ends with its owner covers every way
  the owner can end, and a reaper that is later improved loses nothing.
- **Tests and controls.** `host-plan.test.mjs` (in `just ci`), four tests that
  run the launcher as Playwright does (`sh -c` of the host's command, a pipe
  held for its input): each word of a command reaches its host as it is; the
  host lives while its input is open and ends, its port refusing, when the
  input ends; the launcher answers with its host's status, its input still
  open; an owner killed outright leaves its host listening nowhere.
  `host-plan.native.test.mjs`, one on the real Playwright 1.58: a runner
  whose group is killed with `SIGKILL`, while its one test hangs, leaves its
  host's port closed (with the host command bare, it stays open and the test
  fails at its cap of 30 s). Six mutants in
  `scripts/browser_host_map_mutations.py` (a selection that starts a host
  bare; a command that is not the launcher's; a word unquoted; a launcher that
  does not end its host; one that ends it as it starts; one that answers
  another status), each killed by the assertion of its own test; the script,
  whole: **54 of 54** (the host map's own 48 and these six; baselines 25 fast,
  17 native; 1,131 s).
- **Measured.** A bash "runner" holding the pipe and killed with `SIGKILL`:
  its host gone 17 ms later. The fast suite, 15 runs of 15 with 8 busy loops
  on its 2 CPUs; the whole native suite, 17 of 17.

## 4. Measured

Every number is from a recorded run on the PC (32 threads, load averages of
60 to 145, a dozen workers beside); the scripts that produced them and
their summaries are in the track's evidence folder next to the progress file
(`tracks/evidence/flake-hunt/`). "Base" is `af5c1758`'s test binary, "this"
the track's tree.

- **The wall-clock window, amplified.** `LIVE` is a constant: with it at 30
  ms, every test that depends on the machine's speed between a serve and a
  commit fails on a machine that is ordinary. The command, run whole:

  ```
  sed -i 's/^const LIVE: .*/const LIVE: std::time::Duration = std::time::Duration::from_millis(30);/' \
      spikes/own-renderer/server/src/main.rs
  cargo test -p pw-dev-server          # then: git checkout -- spikes/own-renderer/server/src/main.rs
  ```

  Base: **41 of 662 tests failed** (621 passed, 9 ignored; 2,869 s). This
  tree: **663 passed, 0 failed**, 9 ignored (the new test is the 663rd;
  1,539 s). The 41 are in `live30-base-failures-41.txt`. An earlier run
  of this track's tree (before the shared-cache test pinned its hosts'
  clock) failed exactly one test, which the base's run had not: that was
  midnight in the stores' zone (see Corrections), not the window.
- **Stalls.** `stalled.sh 900 400` stops the test process (`SIGSTOP`) for
  about 900 ms and lets it run for about 400, each interval jittered from a
  seed, which is how a runner starved of its CPUs behaves: a window a test
  guesses at is outrun, an event is not. 23 tests, the same list on both
  binaries, per seed one invocation:

  | test | base, 6 seeds | this, 30 seeds |
  |---|---|---|
  | `pages_read_at_once_share_an_ask_..` | 4 | 0 |
  | `a_streamed_region_follows_its_document_..` | 3 | 0 |
  | `pages_read_at_once_each_ask_..` | 3 | 0 |
  | `under_cancel_..`, `under_keep_..`, `a_browser_that_leaves_..`, `a_forced_reconnect_..` | 2 each | 0 |
  | `the_store_sends_its_slots_..` | 1 | 0 |
  | 12 others (the flight, the delay test, the streams, the told order, the command order, `order::tests` ×5, ...) | 0 | 0 |

  Base: 5 of 6 seeds had a failing test. This tree: **0 of 30 seeds, all 23
  tests**, measured again on the final tree after the last three repairs
  (seeds 501-530: 690 runs of 690 passed). Two first drafts of this track's
  own failed under the stalls before that, and are why the final tree is the
  one measured: the flight test (1 of 34 seeds: the second flight, in
  Corrections) and the dropped-connection test (2 of 34: a `read` with a
  timeout fails with `EINTR` when its process is stopped and continued, which
  the injector does and a loaded machine does not; it retries now). Those
  two tests and the cancel and supersede tests, 36 seeds more: 0 failures.
- **The whole server suite under stalls**, so that the 23 are not the only
  ones looked at: `stalled.sh 900 400` around the whole test binary, two
  seeds. Seed 301, base: **3 of 662 failed**
  (`a_flight_another_reader_waits_for_is_not_stopped`,
  `a_burst_of_posts_tells_another_reader_once`,
  `under_keep_the_new_key_is_read_after_the_old_one_ends`; 1,898 s); this
  tree: **663 passed, 0 failed** (2,160 s). Seed 302, base: **4 of 662
  failed** (`a_forced_reconnect_ends_a_subscription_and_refuses_new_ones_for_its_window`,
  `render_merge::a_commits_documents_are_read_in_one_render_each_identical_call_once`,
  `under_keep_the_new_key_is_read_after_the_old_one_ends`,
  `the_store_sends_its_slots_after_its_own_content`; 1,679 s); this tree:
  **663 passed, 0 failed** (1,811 s). All six distinct tests the base failed
  are in this ADR's lists (four in section 2, two of the `LIVE` family), and
  the sweep found no test of the base that is stall-sensitive and was left
  unlisted. These two runs are of the tree before three later repairs (the
  command order's check, the budget test's close, the stream test's next
  pass, all found by mutation scripts: Corrections); the tests they touch
  were run again under the same stalls, the order's eight over 30 seeds
  (240 of 240 passed) and the 23 on the final tree (690 of 690).
- **The machine as it is** (8 busy loops on 8 CPUs beside the workers, load
  90 to 145). Nine of the tests, base and the first round alternately, 8
  iterations: the slots test failed once in the base (`161 ms` against 150),
  nothing else did, nothing failed on this tree. The base's whole binary
  (662 tests) at the same load: 0 failures in 2,482 s. These tests are
  *rare* failures on this machine, which is why they cost a re-verify and
  hide in the week's noise; the amplifications above are how one reproduces
  them on demand.
- **`check_latency`** (unchanged by this track): 8 of 8 at load 106 to 137.
- **Browser specs** (`lazy-handler`, `recovery`, `keyed`, 330 tests, 5
  repeats in the three engines, 12 busy loops on 8 CPUs): base 330 passed,
  new 330 passed. The base did not fail either: the `window.__pw` gap is
  rare (the first draft of the fix, whose predicate threw on it, met it 3
  times in about 200 runs). Not reproduced, and so not claimed fixed:
  `controls.spec.mjs` in WebKit (12 repeats, 60 passed), `scheduled-orders`
  in Chromium (12 repeats, 24 passed).
- **A host's 60 s start, and what the host map changes** (the integrator's
  question: Playwright's hosts missed their start at load 95-150, and the
  timeout is not to be raised). The host map changes how many hosts a run
  starts, not how long one takes. Playwright sets its `webServer`s up one
  after another, each with 60 s from its own launch (one `plugin setup` task
  per plugin, `tasks.js`), so a run's wait before its first test is the sum
  of its hosts' starts. Before the host map a run declared every host, about
  68 (the registry now has 77 in 27 roles), some 50 of them the store's
  build: four minutes of start-up on a quiet machine before any test, and 50
  chances to miss. Now a run starts the hosts of the tests it selected: the
  registry maps each of the 61 specs to one role at most, so one spec in one
  engine starts one host, in three engines three. One host's start is as it
  was. Measured with `pw-dev-server` on the store's build (`dist`, the dearest:
  `dist-feed` 0.9 s, `dist-keyed` 0.15 s), time to the port accepting a
  connection (how `webServer`'s `port` option waits), pinned to 2 CPUs beside
  busy loops: as the machine is (load 12-25) 4.7-5.2 s in 5 starts; 8 loops
  (about 5 runnable per CPU, the load average of 100-150 on 32 threads) 21-31
  s in 4, median 23; 16 loops (about 10 per CPU) 47 and 52 s; 28 loops (about
  16 per CPU) 69 and 75 s. One store host keeps its 60 s to about 12 runnable
  per CPU (a load average near 400 here), and at 95-150 takes a third to a half
  of it. So a failure at that load is the sum over the old plan's hosts, which
  the host map removes, not a host that misses alone; the old 68-host plan was
  not run under load (twenty minutes an attempt), and a run of many specs at
  once still starts many hosts. The lever is fewer hosts per run, never a
  longer budget.
- **The mutation scripts** whose tests this track changed, run whole, each
  in a snapshot of the tree of its own target directory (a script that names
  a test by a filter was found by `changed_tests.py` in the track's
  evidence and `changed_tests2.py`, which reads every script's literals as
  possible filters; each script's log is in the evidence folder). All the
  scripts below ended with every mutant killed, **293 of 293**, and three of
  them only on the second run, after a repair that Corrections tells of:

  | script | killed | baselines | seconds |
  |---|---|---|---|
  | `tell_at_once` | 7 of 7 | host 5 | 890 |
  | `keyed_race` | 1 of 1 | 4 | 130 |
  | `keyed_reads` | 10 of 10 | 10 | 500 |
  | `document_reads` | 4 of 4 | 4 | 325 |
  | `shared_output` | 6 of 6 | server 4, browser 2 | 812 |
  | `derived_outside` | 10 of 10 | host 6, scripts 26 | 253 |
  | `connection_faults` | 10 of 10 | server 3, browser 3 | 579 |
  | `test_controls` | 10 of 10 | server 5, browser 17 | 529 |
  | `estimate_range` | 6 of 6, twice | server 4, browser 7 | 295, 375 |
  | `descriptions` | 9 of 9, twice | engine 9, server 2, WebKit 1 | 347, 561 |
  | `invariants` | 25 of 25 | core 12, host 79, server 3 | 403 |
  | `store_signals` | 2 of 2 | 17 | 138 |
  | `navigate` | 19 of 19 | compiler 6, browser 10 | 797 |
  | `render_merge` | 49 of 49 | memory 32, PostgreSQL 42, resource 4 | 3,827 |
  | `command_order` | 20 of 20 (first run 19 of 20) | server 12, browser 4 | 1,071 (1,929) |
  | `streams` | 26 of 26 (first run 25 of 26) | rules 12, plan 8, render 4, server 4, ... | 595 (523) |
  | `stream_ack` | 2 of 2 (first run 1 of 2) | 6 | 84 (59) |
  | `browser_host_map` | 54 of 54 (six new) | fast 25, native 17 | 1,131 |
  | `notifications` | 23 of 23 | compiler 3, server 20, browser 9, PostgreSQL | 713 |

  Not run here: `refusal`, `refusal_2`, `runtime_recovery`,
  `build_id` and `evidence_integrity_2`, whose browser mutants run the
  `lazy-handler` and `recovery` specs of the pauses this track replaced (each
  recipe costs hours of a runner: 15,730 s for `e14-refusal`). The assertions
  of those tests are unchanged, and a mutant that stops the log line they
  wait for fails them by their timeout as it failed them by their read; the
  verify this track's push dispatches runs them.

## 5. Ranked, with what was left

Ranked by how often it failed, the week of 2026-10-13 to 20 on GitHub's
runners and the PC's (the handoff's findings), then what the sweep found.
"Fixed" says by what; "left" says why.

| # | test | engine | seen | symptom | state |
|---|---|---|---|---|---|
| 1 | `pw-core` `check_latency::the_whole_program_and_one_edit_are_both_measured` | x86, ARM; CI, PC | at least 9: CI's ARM 10-17, the PC's `just ci` 8 times (guide, each-head-effects, generic-finish ×2, bounded-race ×3, bounded-checker) | the cold check's CPU past the ceiling (2 s; 800 ms a thousand lines) at load | **fixed on the base** (ci-diagnostics: the fastest of seven or more checks); 8 of 8 here |
| 2 | the LIVE-window family: 41 server tests (`a_burst_of_posts_tells_another_reader_once`, `messages::a_message_reaches_both_users_sessions_and_no_third_users`, `notifications::*`, `store_scheduled::an_order_swept_is_told_to_its_readers_open_page`, `render_merge::*`, `bounded_statements::each_page_and_command_is_a_bounded_number_of_statements` (also fixed by bounded-race's `open_pages_ask`), ...) | Rust, memory and PostgreSQL | at least 8 incidents: the page counts in 6 of 16 and 15 of 30 runs at load 40-76 and 9 of 16 on train-10's head; train-6's verify in two recipes; the burst and the message in `just ci` | a document "passed by": counts short, frames missing | **fixed** (section 1) |
| 3 | `the_store_sends_its_slots_after_its_own_content` | Rust | at least 5: render-merge 3 of 24, second-host 1 of 1, check-is-build 1, here 1 of 8 and 1 of 6 under stalls | "shell 154 to 215 ms" against 150 | **fixed** (gates) |
| 4 | `a_flight_another_reader_waits_for_is_not_stopped` | Rust | 3 (train-3's `just ci` at load 20, bounded-race's, check-is-build's) | "the flight was stopped": the command's read began a flight of its own after Hot's second ran out | **fixed** (Hot held) |
| 5 | WebKit's network process crashes on a navigation right after an in-page one (`store-accounts.spec.mjs:173`, `accessibility.spec.mjs:184`) | WebKit | 3 of about 9 runs on train-10's CI, 0 of about 30 elsewhere | `page.goto: WebKit encountered an internal error` | **left**: not a timer (libsoup, a use-after-free in the network process); train-10's specs wait for the page to settle in WebKit only; the upstream report is prepared |
| 6 | `bounded_statements` write-guard controls (a 100 ms window) | Rust, PostgreSQL | the nightly 10-18 (3 of 12 mutants alive), 19 of 120 runs | mutants alive | **fixed by track `bounded-race`** (merged into train-11) |
| 7 | `lazy-handler.spec.mjs`: the two refusal tests | Firefox, WebKit | 1 in a full-suite run at load 30-90 (host map); 3 of about 200 runs of this track's first draft | `window.__pw is undefined` in the gap of the page read again | **fixed** (the runtime's log line, through an optional chain) |
| 8 | `scheduled-orders.spec.mjs:74` | Chromium | 1 of about 12 on CI (train-10's verify) | `page.waitForURL`, 30 s | **left**: not reproduced (0 of 40 on train-10, 0 of 12 here under load); named steps and kept traces landed in train-11 |
| 9 | `controls.spec.mjs:31` | WebKit | 1 (the nightly 10-17) | `[data-pw-handler-error]` count 0 after 5 s | **left**: not reproduced (0 of 12 here under load); its trace is kept now |
| 10 | `feed.spec.mjs:176` | WebKit | 1 (the host map's verify) | `browserContext.newPage`, 30 s | **left**: the browser did not open a page; no wait of the test's is involved |
| 11 | `messages.spec.mjs:123`/`:232`, `notifications.spec.mjs:95`, `feed.spec.mjs:506` | Firefox, WebKit | 1 full-suite run at load 30-90 | 5 s `expect` timeouts | **left**, (c): a cap on an event that must come |
| 12 | `keyed.spec.mjs` tests 7 and 8, `recovery.spec.mjs:46` | all | none seen | a pause of 300 ms stood for Hot's read "under way" | **fixed** (the host's count of reads begun; the runtime's log line) |
| 12b | `navigate.spec.mjs:170` "a press made while the page leaves is not taken" | Firefox | 1 in about 42 runs of the spec phase of `e14-command-order` here, at load 100 | `locator.click` timeout: the page was swapped under the click (a hold of 500 ms) | **fixed** (the page held until the press is made); 36 of 36 under load, three engines |
| 13 | the forced reconnect, the dropped and the applied stream, the command order (`main.rs` ×3, `order.rs` ×5), `pages_read_at_once_*`, the delay test, the told order, the keyed reads (5), the streamed region (2) | Rust | none seen natively; stalls of about 900 ms every 400 ms fail 8 of them in 1 to 4 of 6 seeds | windows of 100 to 2500 ms outrun | **fixed** (section 2) |
| 14 | the heartbeat test | Python | none seen | `time.sleep(0.3)` | **fixed** (the first beat) |
| 15 | a browser run killed at its bound leaves its hosts listening (`e14-refusal`'s 1,200 s; 107 scripts' `bounded()`) | all | the check-is-build worker's campaign at load 90-100: 52 hosts left, 3 mutants REFUSED, the restored suite ran no test | `http://localhost:11141 is already used` | **fixed** (section 3: a host ends with the process that started it) |

The four dev-server tests that failed at load 50-67 in the check-is-build
worker's `cargo test --workspace` (and passed on a rerun) are ranks 2 (twice:
`a_burst_of_posts_tells_another_reader_once` and
`messages::a_message_reaches_both_users_sessions_and_no_third_users`), 3
(`the_store_sends_its_slots_after_its_own_content`) and 4
(`a_flight_another_reader_waits_for_is_not_stopped`): all four fixed. Under
the stalls of section 4 (900 ms stopped every 400), 20 seeds of those four
tests: the base failed in **11 of 20 seeds** (the slots test 7 times, the
flight 5, the burst 1; the message test 0: it fails the `LIVE` amplification
of section 1 and is a rare failure at natural load); this tree, **0 of 80
runs**.

Left in the sweep and not changed, by class:

- (a), **silences**: a wait followed by "nothing happened" can only
  false-pass, so such a test weakens a mutation control under load, never a
  green run. `render_merge.rs`: the commands' bound's `early =
  recv_timeout(500 ms)` (the seats' one is `Renders::waiting` now) and the
  300 ms `waiting_at_most` limit; `a_sessions_changes_wait_for_the_one_in_progress`
  (sleep 500 ms, "not finished"); `pw-materialize/tests/gate.rs`
  (`wait_timeout` 500 ms); Playwright `waitForTimeout` before "nothing
  moved": `cart.spec.mjs:210`, `payments.spec.mjs:101`,
  `signals.spec.mjs:137`, `scheduled-orders.spec.mjs:215`,
  `store.spec.mjs:295`, `lazy-handler.spec.mjs:60,97,312`,
  `recovery.spec.mjs:162`, `keyed.spec.mjs:74,93`,
  `compiled-handler.spec.mjs:122,158`, `resource-path.spec.mjs:87-141`,
  `navigate.spec.mjs:254,257`, `keyed-list.spec.mjs:355`,
  `no-replay.spec.mjs:141,172`, `store-accounts.spec.mjs:176`,
  `events.spec.mjs:23`, `stores.spec.mjs:156`, `soft-navigation.spec.mjs:308`,
  `public-fragment.spec.mjs:104`. Each wants a sentinel: an event that
  provably follows what the test says did not happen (a second command whose
  effect is seen, a request that completes after the forged one).
- (b), **time is the property**: `performance.spec.mjs` (long frames; run
  alone by `just e7-performance`, not in the parallel suite),
  `soft-navigation.spec.mjs:330` (4 s, "longer than a stream is held, about
  2.5 s"), `keyed.spec.mjs:74` (1.5 s, "once Hot would have answered"),
  `payments.rs` `an_answer_is_read_by_its_length_not_its_close` and
  `a_provider_that_does_not_answer_in_time_is_not_reached` (`took < 5 s`
  against a hold of 30 s: a margin of six), `kiokun.rs`'s `#[ignore]`d
  measurements (no assertion on time), `host-plan.native.test.mjs` (a reap
  within 25 s of an 8 s timeout).
- (c), **caps**, all left: `recv_timeout(Duration::from_secs(5 | 10 | 20 |
  30))` in `pw-resource`'s tests and `render_merge.rs`, `bounded_statements.rs`'s
  `MUST_HAPPEN`, `Gate`'s ten seconds, Playwright's 5 s `expect` and 30 s test
  timeouts. One was a (b) in (c)'s clothes and changed: `order.rs`'s
  hang cap of `SHORT * 5` (a second) on a wait of 200 ms is now `MUST_HAPPEN`.

## 6. What this does not claim

- That the suite is free of flakes. It names and removes the ones that waited
  on a timer; the browser engines' own (WebKit's network process, a page that
  will not open in 30 s on a runner) are not timers of the tests' making.
- That a test of a **silence** is deterministic where it still sleeps
  (`waitForTimeout(300)` before "nothing moved"): a silence can only
  false-pass, so such a wait weakens a mutation control, never a green run.
  Each is listed in section 5 with the sentinel that would replace it.
- That the mutation scripts' mutants are killed *by the same tests for the
  same reasons*: each script that names a test this track changed was run
  whole (section 4), and the planner's verify of this track's push runs
  the rest.
- That a run's bound is gentle. The 107 scripts' `bounded()` still kills a
  run's group with `SIGKILL` and gives Playwright no chance to say what it
  was doing; only its hosts no longer outlive it. What else a killed runner
  started (its workers, their browsers) was not examined: the leak that was
  reported was the hosts'.
- That the old 68-host plan's start misses were measured: they are read from
  the check-is-build worker's report and explained by the sum of the starts
  (section 4).

## 7. What to do next

1. **The silences** (the (a) list above): one sentinel each, with the
   mutation script that depends on it run whole. The pattern is
   bounded-race's (a report from the code: `Writes::tell_on_wait`, here
   `Order::waits`, `Renders::waiting`, `held_turn`).
2. **A test that serves a page asks for an hour; a test of a window sets the
   window it means.** A new test that fails with `LIVE` at 30 ms depends on
   the machine's speed. The command that finds one is in the track's
   progress file (the server's tests, built with `LIVE` at 30 ms and run
   whole: 41 failures on the base, none on this tree); a recipe for it is
   the integrator's to weigh against the planner's budget (the run is about
   45 minutes).
3. **The browser engines' own** (ranks 5, 8, 9, 10): the traces CI keeps now
   are what is needed.
4. **If a test must measure time** (the CPU ceiling, a long frame), bound
   the fastest of several, as `check_latency` does; never raise the bound.
5. **A bound that stops a run should ask it to stop first**: `SIGINT` to
   the group (Playwright's runner stops its hosts and writes what it ran),
   a grace of seconds, then `SIGKILL`. One shared function in place of the
   107 copies; the hosts no longer need it to be right, but a campaign's
   evidence would read better for it.
