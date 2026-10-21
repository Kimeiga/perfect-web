# ADR-0376: A producer owns its process before cancellation can restore source

Status: accepted by the train integrator, 2026-10-20.

A deterministic probe sent SIGTERM immediately after native Popen.__init__
returned, before the caller assigned its result. The case and template
producers exited and restored mutated source while their child still ran.
The device and qualified-device producers had the same launch window.
A try around communicate alone cannot own a process whose assignment was
interrupted.

One shared `producer_process.run` owns launch, bounded communication and
cleanup for these four producers. SIGINT and SIGTERM are recorded while
ownership is active; ignored dispositions remain ignored. Public Popen and
communicate APIs launch an isolated process group and check pending cancellation
at most every 100ms while communicating. Signals are never masked, so the
child does not inherit a blocked cancellation signal. No preexec_fn, private
subprocess internals or launch thread is needed.

Before native communicate completes, cancellation and timeout kill the owned
process group and reap its leader before returning or raising. Repeated cancellation during cleanup is also
recorded. Saved callable handlers run only after ownership ends; default
cancellation becomes SystemExit(128 + signal), not a signal sent to the
parent, so the caller's source-restoration finally block executes. Native
normal status, output, assertion classification and per-producer timeout
policy remain the producers' authority.

Native tests inject cancellation in the post-constructor window, during
communication and during cleanup. Every probe verifies that source restoration
ran only after its child was reaped. They also cover default, callable and
ignored dispositions, child signal behavior, timeout, output and status.
Assertion-backed controls remove lifecycle guarantees one at a time; a stopped
or invalid producer earns no kill.

## The canonical Python guide launcher

The follow-up native probe demonstrated the same post-constructor cancellation
gap in `guide_python._launch`: the caller restored source while its child and
a grandchild still ran. Its timeout reaped only its immediate child, leaving
the grandchild running. This launcher now uses the same owner with separate
stdout/stderr file handles and a receipt callback after reaping. Normal native
status, fresh import environment and log files remain unchanged; timeout and
cancellation retain the actual child status but cannot validate a result.

Cleanup covers the owned process group, including a native grandchild proved
in the same group. The direct child is reaped. This does not claim cleanup of
descendants that deliberately create a separate session or process group.

The nested native probe then showed why an outer owner cannot immediately
KILL a cooperative owner: the latter cannot reap its separate child group or
restore its source. Guide launches request at most two seconds of TERM grace;
direct producers retain zero grace. The shared owner caps any requested grace
by the inherited `PW_PRODUCER_CLEANUP_MS` allowance, then reserves 500ms before
passing the remaining allowance to the child. This authority comes from the
parent process environment, overriding a copy of any caller-provided child
environment. Global and caller environments remain unchanged. Malformed,
out-of-range or exhausted inherited authority refuses before Popen; normal
native children do not consume it unless they invoke the shared owner.

The maximum explicit allowance is 2000 whole milliseconds. Thus cooperative
owner launches descend through 2000, 1500, 1000 and 500ms, with a fifth shared
launch refused. A direct zero-grace launch forwards zero. This bounds the
actual recursive guide campaigns and the outer browser campaign; callers
must request cooperative grace at an outer boundary. Tests prove recursive
guide-to-guide-to-owner timeout and cancellation, including an ignored-TERM
leaf, with every source restoration following leaf reaping. They also assert
the allowance, environment isolation and refusal before a native launch.

Pre-completion abort cleanup always sends final KILL to the owned group, even if its leader
already exited during TERM grace. A native parent that exits on TERM while a
same-group grandchild ignores TERM exposed the need for this separate abort
state. The leader is retained until that final group signal to prevent its
PID being reused. Where available, public `os.waitid(..., WNOWAIT)` observes
exit without reaping; otherwise the owner retains it for the full bounded
grace. The final `communicate()` drains captured output and reaps the leader.
See the [Python waitid documentation](https://docs.python.org/3.12/library/os.html#os.waitid)
for the waitable-state behavior and macOS availability limit. Grace does not
drain pipes: cooperative cleanup must fit its reserved time and output buffer.

The guarantee covers cooperative shared owners inside their descending
allowance and members of each owned group. It does not promise arbitrary
process-tree containment for noncooperative descendants that create another
session/group. The direct child is reaped; orphan grandchildren can remain
zombies for their parent/init to reap and are distinguished from running
processes in native group tests. Native nested owners reap their own children,
so those stricter witnesses require every recorded PID to be gone.

Receipt storage errors cannot replace pending cancellation: after cleanup,
the saved cancellation handler still runs, retaining a callback failure as
its exception cause. Timeout and cancellation remain invalid evidence
regardless of cooperative exit status. Native tests and named assertion-backed
controls cover the launch window, ordinary/repeated cancellation, callback
failure, recursive budget, final group cleanup, leader retention and fallback.

Native `communicate()` completion is the boundary of owned cancellation cleanup.
A deterministic review probe reaped a successful leader, then delivered SIGTERM
before its communicate wrapper returned; a background grandchild remained.
This is a retained limit, not a repaired case. A normally completed native
producer is responsible for its descendants. Late cancellation still preserves
its handler identity, invalidates evidence and restores source, but the shared
owner must not signal a process-group ID whose leader has already been reaped.
A focused boundary regression and control assert exactly that behavior. This
owner does not replace native pipe I/O or provide process-tree containment.

The guide ownership witness class is explicitly Linux-native: it requires
`/proc/self/stat`, waitid/WNOWAIT observation and pidfd signalling APIs. Ordinary discovery
reports skips on unsupported hosts instead of treating a missing `/proc` tree
as evidence that a grandchild stopped. The strict guide campaign refuses a
skipped baseline and cannot claim mutation kills there. Linux tests simulate
the production helper's missing-waitid fallback; they do not establish native
macOS cancellation behavior.

### Qualified control witnesses (2026-10-21)

The original nine process-owner controls now store their full unittest witness
IDs beside their descriptions and pass those IDs directly to the native
producer. The descriptions, mutations, frozen ten-test baseline, assertion
authority and output are unchanged. Bare method names were longer than eight
descriptions, so the canonical evidence-count reader treated that metadata as
the description when the completed bulk output was fetched. Its refusal was
correct for the ambiguous catalog; its parser is unchanged.

A regression retains the nine actual verdict lines from run 39526960864 and
checks their exact canonical owner and catalog. It fails against the original
metadata and passes with qualified IDs. The bounded local whole campaign
killed all nine controls with native assertion receipts and no memory stop.
[Raw diagnostic proof](../diagnostics/train-12/producer-witness-metadata.txt)
preserves those results. The original bulk run remains failed; the newly
changed campaign requires fresh hosted evidence before final integration is
declared verified.

### Native fixture readiness and guide witness metadata (2026-10-21)

Run 39538607183 refused the unmutated guide campaign because the 300ms timeout
expired before its fixture published both child PIDs. The recursive fixture
had the same startup race before its 500ms timeout. Fixture setup now waits
for an atomic readiness receipt inside the injected Popen constructor. The
production owner starts its unchanged execution deadline only after that
constructor returns. Deterministic delayed starts reproduce both old failures
and pass with the readiness gate; ignored-TERM behavior and the launch-window
cancellation injection remain intact.

Readiness has its own five-second bound. A constructor that fails before
returning still owns its child: native TERM/grace/KILL cleanup retains the
leader with WNOWAIT, then communicates and reaps it before rethrowing the
startup error. This safety harness does not call the production cleanup
functions being mutated. Early atomic PID/start-time receipts cover failures
before normal fixture readiness; emergency cleanup opens a pidfd and verifies
that identity before signalling. Startup failure stays invalid even when the
ready marker appears during cleanup. Native witnesses require source
restoration and stopped owned work, without crediting that setup failure as a
semantic assertion or weakening the original 15-method/13-control inventory.

The guide's thirteen control records also now contain full unittest witness
IDs, passed directly to the producer. Twelve bare long method names had been
misread as descriptions by the unchanged canonical count reader. A separate
regression retains all thirteen actual historical verdicts and requires their
exact catalog owner. Both repairs require fresh whole-campaign evidence; the
failed hosted baseline is retained as a failure. Raw fixture diagnostics are
in [guide readiness](../diagnostics/train-12/guide-readiness.txt), and the
independent metadata causality is in [guide witness metadata](../diagnostics/train-12/guide-witness-metadata.txt).


## Landing (2026-10-21)

Landed on `master` with train-12 at `e7664d2eb01aa8c6e91df594bd7270a84a8bf8de`
after exact-commit CI ([39584219053](https://github.com/studiokimeiga/perfect-web/actions/runs/39584219053))
and automatic track verification ([39575516650](https://github.com/studiokimeiga/perfect-web/actions/runs/39575516650)).
Dated statements above that call landing or combined gates pending describe
the candidate before it landed and are kept as written. The
[acceptance record](../diagnostics/train-12/acceptance-record-2026-10-21.md)
states what the landing's evidence does and does not prove.
