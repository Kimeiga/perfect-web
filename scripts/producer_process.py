"""Own a native producer through launch, cancellation and reaping (ADR-0376)."""
import math
import os
import signal
import subprocess
import time

POLL_SECONDS = 0.1
CLEANUP_ENV = "PW_PRODUCER_CLEANUP_MS"
MAX_CLEANUP_MS = 2000
PARENT_MARGIN_MS = 500


def cleanup_budget(grace, environment):
    """Cap cooperative cleanup by inherited authority, reserving parent time."""
    if not isinstance(grace, (int, float)) or isinstance(grace, bool) or not math.isfinite(grace):
        raise ValueError("producer cleanup grace must be finite seconds")
    requested = grace * 1000
    if requested != int(requested) or not 0 <= requested <= MAX_CLEANUP_MS:
        raise ValueError("producer cleanup grace must be 0..2000 whole milliseconds")
    effective = int(requested)
    inherited = os.environ.get(CLEANUP_ENV)
    if inherited is not None:
        if not inherited.isascii() or not inherited.isdigit() or len(inherited) > 4:
            raise ValueError("malformed inherited producer cleanup allowance")
        allowance = int(inherited)
        if not 0 < allowance <= MAX_CLEANUP_MS:
            raise ValueError("exhausted or invalid inherited producer cleanup allowance")
        effective = min(effective, allowance)
    child_environment = dict(os.environ if environment is None else environment)
    child_environment[CLEANUP_ENV] = str(max(0, effective - PARENT_MARGIN_MS))
    return effective / 1000, child_environment


def wait_unreaped(child, grace):
    """Keep the owned leader's PID reserved until the final group signal.

    WNOWAIT observes exit without reaping. On platforms without waitid, retain
    ownership for the full bounded grace instead. Pipes are drained by the
    final communicate; cooperative cleanup must fit its time/output bounds.
    """
    can_observe = all(hasattr(os, name) for name in ("waitid", "P_PID", "WEXITED", "WNOHANG", "WNOWAIT"))
    deadline = time.monotonic() + grace
    while time.monotonic() < deadline:
        if can_observe and os.waitid(os.P_PID, child.pid, os.WEXITED | os.WNOHANG | os.WNOWAIT) is not None:
            break
        time.sleep(min(POLL_SECONDS, max(0, deadline - time.monotonic())))


def run(cmd, *, timeout, on_reaped=None, grace=0, **kwargs):
    """CompletedProcess, or TimeoutExpired after cleanup; cancellation unwinds.

    This main-thread API defers SIGINT/SIGTERM without changing the child's
    signal mask. Saved callable handlers retain their behavior. A default
    cancellation raises SystemExit so the caller's finally blocks still run.
    An optional TERM grace lets cooperative nested owners finish cleanup, with
    a descending inherited budget. Direct producers default to immediate KILL.
    Optional file streams are passed to Popen; on_reaped receives the native
    pid/status and timeout/cancellation flags before cancellation is raised.
    """
    kwargs.setdefault("stdout", subprocess.PIPE)
    kwargs.setdefault("stderr", subprocess.STDOUT)
    kwargs.setdefault("text", True)
    grace, kwargs["env"] = cleanup_budget(grace, kwargs.get("env"))
    previous = {}
    pending = []
    child = None
    complete = timed_out = False
    out = err = None
    receipt_error = None

    def defer(number, frame):
        if not pending:
            pending.append((number, frame))

    try:
        for number in (signal.SIGINT, signal.SIGTERM):
            previous[number] = signal.getsignal(number)
            if previous[number] != signal.SIG_IGN:
                signal.signal(number, defer)
        child = subprocess.Popen(cmd, start_new_session=True, **kwargs)
        deadline = time.monotonic() + timeout
        while not pending:
            remaining = deadline - time.monotonic()
            if remaining <= 0:
                timed_out = True
                break
            try:
                out, err = child.communicate(timeout=min(POLL_SECONDS, remaining))
                # Native communicate completion is the ownership boundary.
                # Its reaped group identity must not be signalled afterwards.
                complete = True
                break
            except subprocess.TimeoutExpired:
                continue
    finally:
        try:
            if child is not None and not complete:
                if grace:
                    stop_group(child, signal.SIGTERM)
                    wait_unreaped(child, grace)
                # Abort remains an abort even when TERM made the leader exit.
                # Signal the group before reaping releases its leader's PID.
                stop_group(child, signal.SIGKILL)
                out, err = child.communicate()
            if child is not None and child.returncode is not None and on_reaped is not None:
                try:
                    on_reaped(child.pid, child.returncode, timed_out, bool(pending))
                except Exception as error:
                    if not pending:
                        raise
                    receipt_error = error
        finally:
            for number, handler in previous.items():
                signal.signal(number, handler)
    if pending:
        number, frame = pending[0]
        handler = previous[number]
        try:
            if callable(handler):
                handler(number, frame)
            # A callable that returns must not turn cancellation into success.
            raise SystemExit(128 + number)
        except BaseException as cancelled:
            if receipt_error is not None:
                raise cancelled from receipt_error
            raise
    if timed_out:
        raise subprocess.TimeoutExpired(cmd, timeout, output=out, stderr=err)
    return subprocess.CompletedProcess(cmd, child.returncode, out, err)


def stop_group(child, number):
    """Signal only the process group this launch created."""
    try:
        os.killpg(child.pid, number)
    except ProcessLookupError:
        pass
