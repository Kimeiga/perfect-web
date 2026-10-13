#!/usr/bin/env python3
"""Mutation controls for track `order-total` (ADR-XXXX, NEXT's item 6, step
6b, W9): an order's total, each store's fees and tax, a promo code and a
tip, priced by the program from the store's data and charged as the page
showed it.

Each mutant undoes one rule:
- the money (`examples/domain.pw`): `is_less` at or under, not under;
- the program (`examples/store/checkout.pw`): the tax rounded toward zero;
  delivery, or service and the small-order fee, never taxed where the
  schedule taxes them; the service fee not clamped at its least, or at its
  most; no small-order fee; a percent off rounded toward zero; a platform's
  discount lowering the tax, or a store's not; the tip left out of the
  total; an expired code, a used code, a code of a store not in the cart,
  or one under its least subtotal, taken; a negative tip, or a percent the
  page does not offer, taken;
- the command (`examples/store/app.pw`): the order placed whatever total
  the page showed;
- the layers: memory's uses of a code not read; memory's charges not
  recorded; PostgreSQL's use of a code recorded twice.

A mutant must fail the development server's order-total tests in memory
(`server`), or on PostgreSQL (`postgres`, which needs
PW_STORE_DATABASE_URL), narrowed by name to `tests::order_total::` and
`checkout::tests::`, the tests that can see their rules.

Run from the repository root; `just e14-order-total` records the output.
The source is restored after every mutant, whatever happens.
"""

import os
import pathlib
import re
import signal
import subprocess
import sys

ROOT = pathlib.Path(__file__).resolve().parent.parent
DOMAIN = ROOT / "examples/domain.pw"
CHECKOUT = ROOT / "examples/store/checkout.pw"
APP = ROOT / "examples/store/app.pw"
CHECKOUT_RS = ROOT / "spikes/own-renderer/server/src/checkout.rs"
STORE_DATA = ROOT / "spikes/own-renderer/server/src/store.rs"

# (what, suite, file, anchor, replacement)
MUTANTS = [
    (
        "is_less is at or under",
        "server",
        DOMAIN,
        "    a.value < b.value\n",
        "    a.value <= b.value\n",
    ),
    (
        "the tax is rounded toward zero",
        "server",
        CHECKOUT,
        "        tax: at_rate(taxable, fees.tax_rate, Rounding.HalfUp),\n",
        "        tax: at_rate(taxable, fees.tax_rate, Rounding.TowardZero),\n",
    ),
    (
        "delivery is never taxed",
        "server",
        CHECKOUT,
        "    let delivery_taxed = if fees.tax_delivery { fees.delivery_fee } else { nothing() }\n",
        "    let delivery_taxed = nothing()\n",
    ),
    (
        "service is never taxed",
        "server",
        CHECKOUT,
        "    let service_taxed = if fees.tax_service { plus(service, small) } else { nothing() }\n",
        "    let service_taxed = nothing()\n",
    ),
    (
        "the small-order fee is not taxed with service",
        "server",
        CHECKOUT,
        "    let service_taxed = if fees.tax_service { plus(service, small) } else { nothing() }\n",
        "    let service_taxed = if fees.tax_service { service } else { nothing() }\n",
    ),
    (
        "a fee is not clamped at its least",
        "server",
        CHECKOUT,
        "    if is_less(amount, least) { least } else if is_less(most, amount) { most } else { amount }\n",
        "    if false { least } else if is_less(most, amount) { most } else { amount }\n",
    ),
    (
        "a fee is not clamped at its most",
        "server",
        CHECKOUT,
        "    if is_less(amount, least) { least } else if is_less(most, amount) { most } else { amount }\n",
        "    if is_less(amount, least) { least } else if false { most } else { amount }\n",
    ),
    (
        "no small-order fee",
        "server",
        CHECKOUT,
        "    let small = if is_less(subtotal, fees.small_order_under) { fees.small_order_fee } else { nothing() }\n",
        "    let small = nothing()\n",
    ),
    (
        "a percent off is rounded toward zero",
        "server",
        CHECKOUT,
        "        PercentOff(rate) => at_rate(subtotal, rate, Rounding.HalfUp),\n",
        "        PercentOff(rate) => at_rate(subtotal, rate, Rounding.TowardZero),\n",
    ),
    (
        "a platform's discount lowers the tax",
        "server",
        CHECKOUT,
        "        Some(ByPlatform) => nothing(),\n",
        "        Some(ByPlatform) => discount,\n",
    ),
    (
        "a store's discount does not lower the tax",
        "server",
        CHECKOUT,
        "        Some(ByStore) => discount,\n",
        "        Some(ByStore) => nothing(),\n",
    ),
    (
        "the tip is left out of the total",
        "server",
        CHECKOUT,
        "        total: plus(sum(List.map(charged, part_total)), tip_charged),\n",
        "        total: sum(List.map(charged, part_total)),\n",
    ),
    (
        "an expired code is taken",
        "server",
        CHECKOUT,
        "        Some(f) => if clock.passed(f.promo.expires) {\n",
        "        Some(f) => if false {\n",
    ),
    (
        "a used code is taken",
        "server",
        CHECKOUT,
        "        } else if f.used {\n",
        "        } else if false {\n",
    ),
    (
        "a code of a store not in the cart is taken",
        "server",
        CHECKOUT,
        "                None => Err(PromoError.NotInCart),\n",
        "                None => Ok(f.promo),\n",
    ),
    (
        "a code under its least subtotal is taken",
        "server",
        CHECKOUT,
        "                Some(part) => if is_less(subtotal_of(part.lines), f.promo.min_subtotal) {\n",
        "                Some(part) => if false {\n",
    ),
    (
        "a negative tip is taken",
        "server",
        CHECKOUT,
        "        TipAmount(amount) => is_less(amount, nothing()),\n",
        "        TipAmount(amount) => false,\n",
    ),
    (
        "a percent the page does not offer is taken",
        "server",
        CHECKOUT,
        "        Some(Tip.Twenty)\n    } else {\n        None\n",
        "        Some(Tip.Twenty)\n    } else {\n        Some(Tip.NoTip)\n",
    ),
    (
        "the order is placed whatever total the page showed",
        "server",
        APP,
        "            Ok(p) => if is_less(p.charged.total, shown) | is_less(shown, p.charged.total) {\n",
        "            Ok(p) => if false {\n",
    ),
    (
        "memory's uses of a code are not read",
        "server",
        STORE_DATA,
        "            .contains(&(code.to_string(), owner.to_string()));\n",
        "            .contains(&(String::new(), String::new()));\n",
    ),
    (
        "memory's charges are not recorded",
        "server",
        STORE_DATA,
        "            .insert(owner.to_string(), charged.clone());\n",
        "            .remove(owner);\n",
    ),
    (
        "PostgreSQL's use of a code is recorded twice",
        "postgres",
        CHECKOUT_RS,
        "    Ok(n == 1)\n",
        "    Ok(n <= 1)\n",
    ),
]

TESTS = [
    "cargo", "test", "--quiet", "--locked", "-p", "pw-dev-server", "--",
    "tests::order_total::", "checkout::tests::",
]
CARGO = {
    "server": (TESTS, {"PW_STORE_TEST_LAYER": "memory", "PW_STORE_DATABASE_URL": ""}),
    "postgres": (TESTS, {"PW_STORE_TEST_LAYER": "postgres"}),
}

# How long one command may run. Past the bound it is killed with everything
# it started, and the run counts as failing.
BOUND = 1200


def bounded(cmd, env):
    """(output, returncode), or (output, None) when it ran past the bound."""
    p = subprocess.Popen(cmd, cwd=ROOT, env={**os.environ, **env}, start_new_session=True,
                         stdout=subprocess.PIPE, stderr=subprocess.STDOUT, text=True)
    try:
        out, _ = p.communicate(timeout=BOUND)
        return out, p.returncode
    except subprocess.TimeoutExpired:
        os.killpg(p.pid, signal.SIGKILL)
        out, _ = p.communicate()
        return out, None


def cargo_tests(suite):
    """(built, passed, failed) over the suite's tests."""
    cmd, env = CARGO[suite]
    out, code = bounded(cmd, env)
    if code is None:
        return True, 0, 1
    found = re.findall(r"test result: \w+\. (\d+) passed; (\d+) failed", out)
    if not found:
        return False, 0, 0
    return True, sum(int(p) for p, _ in found), sum(int(f) for _, f in found)


def main():
    import mutation_baseline  # keeps what each command says, for a red baseline
    # Stopped from outside, the source is still restored: SIGTERM raises
    # here, and the `finally` below runs.
    signal.signal(signal.SIGTERM, lambda *_: sys.exit(1))
    if not os.environ.get("PW_STORE_DATABASE_URL"):
        print("FAIL: the postgres suite needs PW_STORE_DATABASE_URL, a throwaway database")
        return 1
    for suite in CARGO:
        built, passed, failed = cargo_tests(suite)
        print(f"baseline ({suite}): {passed} passed, {failed} failed", flush=True)
        if not built or failed or not passed:
            print("FAIL: the unmutated baseline is not green; no mutant can mean anything")
            mutation_baseline.explain()
            return 1

    survivors = 0
    for what, suite, path, anchor, replacement in MUTANTS:
        original = path.read_text()
        if original.count(anchor) != 1:
            print(f"{what}: ANCHOR NOT FOUND EXACTLY ONCE in {path.name}")
            survivors += 1
            continue
        try:
            path.write_text(original.replace(anchor, replacement, 1))
            built, passed, failed = cargo_tests(suite)
        finally:
            path.write_text(original)
        if not built:
            verdict = "KILLED (does not build)"
        elif failed:
            verdict = f"KILLED ({failed} of {passed + failed} tests fail)"
        else:
            verdict = "SURVIVED"
            survivors += 1
        print(f"{what} [{suite}]: {verdict}", flush=True)

    print(f"{len(MUTANTS) - survivors} of {len(MUTANTS)} mutants killed")
    return 1 if survivors else 0


if __name__ == "__main__":
    sys.exit(main())
