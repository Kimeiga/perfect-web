"""Own a native producer through launch, cancellation and reaping (ADR-0376)."""
import os
import signal
import subprocess
import time

POLL_SECONDS = 0.1


def run(cmd, *, timeout, **kwargs):
    """CompletedProcess, or TimeoutExpired after cleanup; cancellation unwinds.

    This main-thread API defers SIGINT/SIGTERM without changing the child's
    signal mask. Saved callable handlers retain their behavior. A default
    cancellation raises SystemExit so the caller's finally blocks still run.
    """
    previous = {}
    pending = []
    child = None
    complete = timed_out = False
    out = err = None

    def defer(number, frame):
        if not pending:
            pending.append((number, frame))

    try:
        for number in (signal.SIGINT, signal.SIGTERM):
            previous[number] = signal.getsignal(number)
            if previous[number] != signal.SIG_IGN:
                signal.signal(number, defer)
        child = subprocess.Popen(cmd, start_new_session=True, stdout=subprocess.PIPE,
                                 stderr=subprocess.STDOUT, text=True, **kwargs)
        deadline = time.monotonic() + timeout
        while not pending:
            remaining = deadline - time.monotonic()
            if remaining <= 0:
                timed_out = True
                break
            try:
                out, err = child.communicate(timeout=min(POLL_SECONDS, remaining))
                complete = True
                break
            except subprocess.TimeoutExpired:
                continue
    finally:
        try:
            if child is not None and not complete:
                try:
                    os.killpg(child.pid, signal.SIGKILL)
                except ProcessLookupError:
                    pass
                out, err = child.communicate()
        finally:
            for number, handler in previous.items():
                signal.signal(number, handler)
    if pending:
        number, frame = pending[0]
        handler = previous[number]
        if callable(handler):
            handler(number, frame)
        # A callable that returns must not turn cancellation into success.
        raise SystemExit(128 + number)
    if timed_out:
        raise subprocess.TimeoutExpired(cmd, timeout, output=out, stderr=err)
    return subprocess.CompletedProcess(cmd, child.returncode, out, err)
