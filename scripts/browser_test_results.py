"""Validate the nonce-bound complete assertion reporter receipt, never prose counts.

The reporter owns lifecycle and assertion provenance. This transport validator
owns native exit agreement, freshness and the campaign's frozen test inventory.
"""
from typing import NamedTuple


class Result(NamedTuple):
    valid: bool
    passed: int = 0
    failed: int = 0
    inventory: tuple = ()
    reason: str = ""


def parse(status, receipt, nonce, expected=None):
    if type(status) is not int or status not in (0, 1):
        return Result(False, reason="unexpected browser exit")
    if (not isinstance(receipt, dict) or type(receipt.get("schema")) is not int
            or receipt.get("schema") != 1 or receipt.get("complete") is not True):
        return Result(False, reason="missing complete browser receipt")
    if receipt.get("nonce") != nonce:
        return Result(False, reason="stale browser receipt")
    inventory, tests = receipt.get("inventory"), receipt.get("tests")
    if (not isinstance(inventory, list) or not inventory
            or any(not isinstance(i, str) or not i for i in inventory)
            or len(set(inventory)) != len(inventory)):
        return Result(False, reason="invalid browser inventory")
    if (not isinstance(tests, list) or any(not isinstance(t, list) or len(t) != 2
            or t[0] not in inventory or t[1] not in ("passed", "failed") for t in tests)
            or len(tests) != len(inventory) or len({t[0] for t in tests}) != len(inventory)):
        return Result(False, reason="incomplete browser results")
    if expected is not None and set(inventory) != set(expected):
        return Result(False, reason="browser inventory changed")
    failed = sum(t[1] == "failed" for t in tests)
    if status != (1 if failed else 0):
        return Result(False, reason="browser exit contradicts receipt")
    return Result(True, len(tests) - failed, failed, tuple(sorted(inventory)))
