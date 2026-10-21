#!/usr/bin/env python3
"""Mutation controls for the bound in memory on a mutation script's
processes (`mutation_bound.py`).

Each mutant undoes one thing the bound does: stopping a process past it,
watching what the script's processes started, saying each stop once, a
process at the bound within it, the script and another's processes left
unread, a second reading before a stop, a number used again watched again,
the bound itself, a watch that fails saying so, the macOS footprint's place
in its structure, a stop invalidating summary/fetch/preverification, and
`mutation_baseline` starting the bound. Canonical unittest receipts require
the intended actual assertion; errors, incomplete runs and stops earn no kill.

The tests that watch real processes stop only processes they marked, so a
mutant that watches every process stops none of the machine's. At least one test of
`scripts/tests/test_mutation_bound.py` or `test_mutation_baseline.py` must
then fail.

Run from the repository root; `just e14-mutation-bound` records the output.
The sources are restored after every mutant, whatever happens.
"""

import pathlib
import tempfile
import signal
import sys

ROOT = pathlib.Path(__file__).resolve().parent.parent
BOUND = ROOT / "scripts/mutation_bound.py"
BASELINE = ROOT / "scripts/mutation_baseline.py"
SUMMARY = ROOT / "scripts/ci_summary.py"
FETCH = ROOT / "scripts/evidence_fetch.py"
PREVERIFY = ROOT / "scripts/preverify.py"
SELF = ROOT / "scripts/mutation_bound_mutations.py"
sys.path.insert(0, str(ROOT / "scripts"))
import guide_python

# (full assertion witness, what is undone, file, anchor, replacement)
MUTANTS = [
    (
        'test_mutation_bound.Bound.test_a_process_past_the_bound_is_stopped_and_said',
        "a process past the bound is said stopped and runs on",
        BOUND,
        "                self.kill(pid, signal.SIGKILL)\n",
        "                pass\n",
    ),
    (
        'test_mutation_bound.Bound.test_the_processes_watched_are_those_the_root_started_and_theirs',
        "only the processes the script started itself are watched",
        BOUND,
        "        stack.extend(children.get(pid, []))\n",
        "        pass\n",
    ),
    (
        'test_mutation_bound.Bound.test_only_the_scripts_own_are_read_each_stop_said_once',
        "a process stopped is said again at each look",
        BOUND,
        "            self.seen.add(pid)\n",
        "",
    ),
    (
        'test_mutation_bound.Bound.test_only_the_scripts_own_are_read_each_stop_said_once',
        "a process at the bound is stopped",
        BOUND,
        "            if held is None or held <= self.bound:\n",
        "            if held is None or held < self.bound:\n",
    ),
    (
        'test_mutation_bound.Bound.test_only_the_scripts_own_are_read_each_stop_said_once',
        "the script itself is watched",
        BOUND,
        "        for pid in sorted(descendants(parents, self.root) - self.seen):\n",
        "        for pid in sorted((descendants(parents, self.root) | {self.root}) - self.seen):\n",
    ),
    (
        'test_mutation_bound.Bound.test_only_the_scripts_own_are_read_each_stop_said_once',
        "every process is watched",
        BOUND,
        "        for pid in sorted(descendants(parents, self.root) - self.seen):\n",
        "        for pid in sorted(set(parents) - self.seen):\n",
    ),
    (
        'test_mutation_bound.Bound.test_one_whose_parents_read_again_are_anothers_is_left_alone',
        "a process is stopped on the first reading alone",
        BOUND,
        "            if not descends(self.platform.parents(), pid, self.root):\n                continue\n",
        "",
    ),
    (
        'test_mutation_bound.Bound.test_a_number_used_again_is_watched_again',
        "a number used again is never watched again",
        BOUND,
        "        self.seen &= set(parents)\n",
        "",
    ),
    (
        'test_mutation_bound.Bound.test_the_bound_is_4_GiB_or_a_quarter_of_the_machine',
        "the bound is the machine's memory",
        BOUND,
        "BOUND = min(4 * GiB, total_memory() // 4)\n",
        "BOUND = total_memory()\n",
    ),
    (
        'test_mutation_bound.Bound.test_a_watch_that_cannot_read_the_processes_says_so',
        "a watch that cannot read the processes stops without a word",
        BOUND,
        '                self.say(f"  the memory bound stopped watching: {e!r}")\n',
        "                pass\n",
    ),
    (
        'test_mutation_bound.Bound.test_the_structures_read_on_macos_are_the_sdks',
        "the macOS footprint is read where the resident size is",
        BOUND,
        '                "resident_size",\n                "phys_footprint",\n',
        '                "phys_footprint",\n                "resident_size",\n',
    ),
    (
        'test_mutation_bound.Bound.test_a_stop_names_the_verdict_after_it_a_kind_of_its_own',
        "a stop is not named after the verdict it may have decided",
        BOUND,
        "                self.decided.append(what)\n",
        "                pass\n",
    ),
    (
        'test_mutation_bound.Bound.test_a_run_the_bound_did_not_touch_says_so',
        "a run the bound did not touch says nothing",
        BOUND,
        '                return "memory bound: no process was stopped"\n',
        '                return ""\n',
    ),
    (
        'test_mutation_bound.Bound.test_a_stop_names_the_verdict_after_it_a_kind_of_its_own',
        "an indented note is read as a verdict",
        BOUND,
        '    VERDICT = re.compile(r"^(?P<what>\\S.*?): (?:KILLED|SURVIVED)\\b")\n',
        '    VERDICT = re.compile(r"^(?P<what>.*?): (?:KILLED|SURVIVED)\\b")\n',
    ),
    (
        'test_ci_scripts.Summary.test_a_kill_perhaps_the_memory_bounds_alone_fails_the_recipe',
        "the run's summary leaves out what the bound stopped",
        SUMMARY,
        '            ok = r["status"] == 0 and not survived and not invalid\n',
        '            ok = r["status"] == 0 and not survived\n',
    ),
    (
        'test_mutation_baseline.Bounded.test_imported_it_bounds_every_process_the_script_starts',
        "a mutation script starts no bound",
        BASELINE,
        "mutation_bound.start()\n",
        "pass\n",
    ),
    (
        'test_ci_scripts.BoundEvidence.test_summary_refuses_each_stop_and_failed_watcher',
        'an indented immediate stop is ignored',
        SUMMARY,
        '        if (line.strip().startswith(BOUND) and line.strip() != UNTOUCHED)\n',
        '        if (line.startswith(BOUND) and line.strip() != UNTOUCHED)\n',
    ),
    (
        'test_ci_scripts.BoundEvidence.test_summary_refuses_each_stop_and_failed_watcher',
        'a final stop summary is ignored',
        SUMMARY,
        '        if (line.strip().startswith(BOUND) and line.strip() != UNTOUCHED)\n',
        '        if (line.strip().startswith("memory bound: stopped ") and line.strip() != UNTOUCHED)\n',
    ),
    (
        'test_ci_scripts.BoundEvidence.test_summary_refuses_each_stop_and_failed_watcher',
        'a failed memory watcher is ignored',
        SUMMARY,
        '        or line.strip().startswith("the memory bound stopped watching:")\n',
        '        or False\n',
    ),
    (
        'test_ci_scripts.BoundEvidence.test_summary_refuses_each_stop_and_failed_watcher',
        'a later no-stop line erases an earlier failure',
        SUMMARY,
        '    return list(dict.fromkeys(\n',
        '    if UNTOUCHED in text:\n        return []\n    return list(dict.fromkeys(\n',
    ),
    (
        'test_ci_scripts.BoundEvidence.test_recipe_log_alone_invalidates_summary_fetch_and_preverify',
        'the recipe log is not read for memory stops',
        SUMMARY,
        '    if log.exists():\n',
        '    if False:\n',
    ),
    (
        'test_ci_scripts.BoundEvidence.test_fetch_excludes_all_outputs_and_completed_timing_of_stopped_recipe',
        'fetch reads native status without guard validity',
        FETCH,
        '                row["invalid"] = ci_summary.recipe_memory_failures(shard, row)\n',
        '                row["invalid"] = []\n',
    ),
    (
        'test_ci_scripts.BoundEvidence.test_known_summary_cannot_copy_stopped_recipe_from_historical_green_run',
        'fetch copies a stopped recipes files',
        FETCH,
        'if r.get("status") != 0 or r.get("invalid") for w',
        'if r.get("status") != 0 for w',
    ),
    (
        'test_ci_scripts.BoundEvidence.test_fetch_excludes_all_outputs_and_completed_timing_of_stopped_recipe',
        'a stopped recipe retains completed timing',
        FETCH,
        'if r.get("status") == 0 and not r.get("invalid")}',
        'if r.get("status") == 0}',
    ),
    (
        'test_ci_scripts.BoundEvidence.test_recipe_log_alone_invalidates_summary_fetch_and_preverify',
        'local preverification ignores guard validity',
        PREVERIFY,
        '    ok = judge(row["status"], survived + invalid)\n',
        '    ok = judge(row["status"], survived)\n',
    ),
    (
        'test_ci_scripts.BoundEvidence.test_exact_no_stop_and_absent_guard_remain_valid',
        'an exact no-stop result is refused',
        SUMMARY,
        '        if (line.strip().startswith(BOUND) and line.strip() != UNTOUCHED)\n',
        '        if line.strip().startswith(BOUND)\n',
    ),
    (
        "test_ci_scripts.BoundEvidence.test_bound_controls_require_a_complete_actual_intended_assertion",
        'invalid control receipts earn a kill',
        SELF,
        '    if stopped or any(not run.result.valid or run.result.body_errors\n',
        '    if stopped or any(False or run.result.body_errors\n',
    ),
    (
        "test_ci_scripts.BoundEvidence.test_bound_controls_require_a_complete_actual_intended_assertion",
        'an erroneous control body earns a kill',
        SELF,
        '    if stopped or any(not run.result.valid or run.result.body_errors\n',
        '    if stopped or any(not run.result.valid or False\n',
    ),
    (
        "test_ci_scripts.BoundEvidence.test_bound_controls_require_a_complete_actual_intended_assertion",
        'an incomplete control inventory earns a kill',
        SELF,
        '                      or run.result.executed != len(ids)\n',
        '                      or False\n',
    ),
    (
        "test_ci_scripts.BoundEvidence.test_bound_controls_require_a_complete_actual_intended_assertion",
        'a stopped control producer earns a kill',
        SELF,
        '    if stopped or any(not run.result.valid or run.result.body_errors\n',
        '    if False or any(not run.result.valid or run.result.body_errors\n',
    ),
    (
        "test_ci_scripts.BoundEvidence.test_bound_controls_require_a_complete_actual_intended_assertion",
        'an unrelated body assertion earns a kill',
        SELF,
        '    return "KILLED" if any(witness in run.result.assertion_failed for run in runs) else "SURVIVED"\n',
        '    return "KILLED" if any(run.result.assertion_failed for run in runs) else "SURVIVED"\n',
    ),
]

SEARCH = ROOT / "scripts/tests"
SELECTORS = ("test_mutation_bound", "test_mutation_baseline", "test_ci_scripts")
TIMEOUT = 120


def credit(runs, expected, stopped, witness):
    """A complete canonical result and the intended actual assertion are required."""
    if stopped or any(not run.result.valid or run.result.body_errors
                      or run.result.executed != len(ids)
                      for run, ids in zip(runs, expected)) or len(runs) != len(expected):
        return "REFUSED"
    return "KILLED" if any(witness in run.result.assertion_failed for run in runs) else "SURVIVED"


def main():
    import mutation_baseline  # keeps each native output; the shared guard remains active
    signal.signal(signal.SIGTERM, lambda *_: sys.exit(1))
    watch = mutation_baseline.mutation_bound.start()
    evidence = pathlib.Path(tempfile.mkdtemp(prefix="pw-bound-controls-"))
    print(f"structured instrument receipts: {evidence}", flush=True)
    try:
        inventories = [guide_python.freeze(name, SEARCH, name + ".py", evidence_root=evidence,
                                           timeout=TIMEOUT) for name in SELECTORS]
    except guide_python.DiscoveryRefused as error:
        print(f"REFUSED: {error}")
        return 1
    witnesses = {witness for witness, *_ in MUTANTS}
    if not witnesses <= {name for ids in inventories for name in ids}:
        print("REFUSED: intended full IDs are absent from the frozen inventories")
        return 1
    def run_all():
        return [guide_python.run(name, ids, SEARCH, evidence, TIMEOUT)
                for name, ids in zip(SELECTORS, inventories)]
    baseline = run_all()
    count = sum(len(ids) for ids in inventories)
    valid = all(run.result.valid and not run.result.failed and run.result.executed == len(ids)
                for run, ids in zip(baseline, inventories)) and not watch.stopped
    print(f"baseline: {count} tests; valid={valid}", flush=True)
    if not valid:
        for run in baseline:
            print(run.output)
        print("FAIL: the unmutated baseline is not green; no mutant can mean anything")
        mutation_baseline.explain()
        return 1
    killed = refused = survived = 0
    originals = {path: path.read_bytes() for _, _, path, _, _ in MUTANTS}
    try:
        for witness, what, path, anchor, replacement in MUTANTS:
            original = originals[path]
            assert path.read_bytes() == original, "source changed between owned controls"
            text = original.decode()
            if text.count(anchor) != 1:
                print(f"{what}: ANCHOR NOT FOUND EXACTLY ONCE in {path.name}", flush=True)
                refused += 1
                continue
            try:
                path.write_bytes(text.replace(anchor, replacement, 1).encode())
                runs = run_all()
                verdict = credit(runs, inventories, bool(watch.stopped), witness)
                if verdict == "KILLED":
                    killed += 1
                    detail = f"KILLED by {witness} (actual body assertion)"
                else:
                    refused += verdict == "REFUSED"
                    survived += verdict == "SURVIVED"
                    detail = f"{verdict} (invalid producer or no intended assertion; no kill)"
                    for run in runs:
                        print(run.output)
            finally:
                path.write_bytes(original)
            print(f"{what}: {detail}", flush=True)
    finally:
        for path, original in originals.items():
            path.write_bytes(original)
        assert all(path.read_bytes() == original for path, original in originals.items())
    print(f"{killed} of {len(MUTANTS)} mutants killed; {refused} refused; {survived} survived", flush=True)
    return int(bool(refused or survived or watch.stopped))


if __name__ == "__main__":
    sys.exit(main())
