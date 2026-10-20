"""Pure validation of the synchronous unittest body-provenance protocol.

No discovery, file ownership, process launch, timeout or cleanup authority lives
here. Invalid outcomes contain no positive evidence IDs or executed-body count.
"""
import json
from typing import NamedTuple

SCHEMA = 1


class Result(NamedTuple):
    valid: bool
    reason: str
    passed: tuple = ()
    assertion_failed: tuple = ()
    body_errors: tuple = ()
    executed: int = 0

    @property
    def failed(self):
        """Distinct failed methods, even when one has several subtest events."""
        return tuple(sorted(set(self.assertion_failed) | set(self.body_errors)))


def strict_json(text):
    def unique(pairs):
        result = {}
        for key, value in pairs:
            if key in result:
                raise ValueError("duplicate JSON key")
            result[key] = value
        return result
    return json.loads(text, object_pairs_hook=unique)


def validate_result(status, text, expected):
    """Validate a complete fresh receipt against a consumer-frozen inventory."""
    try:
        report = strict_json(text)
        fields = {"schema", "expected", "started", "finished", "bodies", "events",
                  "failed", "passed", "refusals", "complete"}
        if not isinstance(report, dict) or set(report) != fields:
            return Result(False, "invalid report fields")
        if type(report["schema"]) is not int or report["schema"] != SCHEMA or report["complete"] is not True:
            return Result(False, "unsupported schema or incomplete report")
        if (not isinstance(expected, list) or not expected or any(type(x) is not str for x in expected)
                or expected != sorted(set(expected)) or report["expected"] != expected):
            return Result(False, "expected inventory mismatch")
        for key in ("started", "finished"):
            if not isinstance(report[key], list) or sorted(report[key]) != expected:
                return Result(False, "incomplete " + key + " inventory")
        if report["refusals"] != []:
            return Result(False, "producer refused execution")
        bodies = report["bodies"]
        if not isinstance(bodies, list) or len(bodies) != len(expected):
            return Result(False, "incomplete body inventory")
        if sorted(body["id"] for body in bodies) != expected:
            return Result(False, "body inventory mismatch")
        for body in bodies:
            if (set(body) != {"id", "entered", "completed"}
                    or type(body["entered"]) is not int or type(body["completed"]) is not int
                    or body["entered"] != 1 or body["completed"] != 1):
                return Result(False, "unexecuted, unfinished or repeated body")
        events = report["events"]
        if not isinstance(events, list):
            return Result(False, "invalid events")
        failures, successes, terminal, subtests = set(), set(), {}, {}
        assertion_failed, body_errors = set(), set()
        kinds = {"success", "failure", "error", "subtest_success", "subtest_failure", "subtest_error"}
        for event in events:
            if (not isinstance(event, dict) or set(event) != {"id", "kind", "origin"}
                    or event["id"] not in expected or event["kind"] not in kinds):
                return Result(False, "unknown result event")
            if event["origin"] != "body":
                return Result(False, "non-body result event")
            identity, kind = event["id"], event["kind"]
            if kind == "success":
                successes.add(identity)
            if kind in ("failure", "error", "subtest_failure", "subtest_error"):
                failures.add(identity)
            if kind in ("failure", "subtest_failure"):
                assertion_failed.add(identity)
            if kind in ("error", "subtest_error"):
                body_errors.add(identity)
            if kind.startswith("subtest_"):
                subtests[identity] = subtests.get(identity, 0) + (kind != "subtest_success")
            else:
                terminal[identity] = terminal.get(identity, 0) + 1
        # A method either has exactly one terminal callback, or failed subtests.
        for identity in expected:
            if terminal.get(identity, 0) > 1 or (not terminal.get(identity, 0) and not subtests.get(identity, 0)):
                return Result(False, "incomplete or repeated terminal callbacks")
        if successes & failures or successes | failures != set(expected):
            return Result(False, "contradictory method partition")
        if report["failed"] != sorted(failures) or report["passed"] != sorted(successes):
            return Result(False, "contradictory declared partition")
        failed = len(failures)
        if type(status) is not int or status != (1 if failed else 0):
            return Result(False, "child exit contradicts result")
        return Result(True, "complete", tuple(sorted(successes)), tuple(sorted(assertion_failed)),
                      tuple(sorted(body_errors)), len(bodies))
    except (ValueError, TypeError, KeyError, AttributeError, RecursionError):
        return Result(False, "malformed report")
