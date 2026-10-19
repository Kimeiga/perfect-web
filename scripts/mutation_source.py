"""Release owned source/report resources without losing cancellation identity.

Process execution, signals, bounds and reaping remain with each caller.
"""
from contextlib import contextmanager
import sys


def cancellation(error):
    return isinstance(error, (SystemExit, KeyboardInterrupt))


def release(cleanup, primary):
    try:
        cleanup()
    except BaseException as secondary:
        if primary is None or (not cancellation(primary) and not isinstance(secondary, Exception)):
            raise
        try:
            print(f"cleanup also failed: {secondary!r}", file=sys.stderr)
        except BaseException as diagnostic:
            if not cancellation(primary) and not isinstance(diagnostic, Exception):
                raise


@contextmanager
def releasing(cleanup):
    primary = None
    try:
        yield
    except BaseException as error:
        primary = error
        raise
    finally:
        release(cleanup, primary)


@contextmanager
def changed(path, original, replacement):
    with releasing(lambda: path.write_text(original)):
        path.write_text(replacement)
        yield
