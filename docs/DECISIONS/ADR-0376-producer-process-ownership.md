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

Cancellation and timeout kill the owned process group and reap its leader
before returning or raising. Repeated cancellation during cleanup is also
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
