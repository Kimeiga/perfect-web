#!/usr/bin/env python3
"""Mutation controls for a mutation script's mutants run in parallel slots
(`mutation_slots.py`, ADR-0357).

Each mutant undoes one thing the slots do: the tree never mutated (the
script's loop run in the tree, or the tree's copy of the script run in a
slot); each slot its own (ports, temporary directory, target, git, a copy
brought to the tree by content, a run's leftovers removed, `node_modules`
linked, the tree's target not copied); the ask for slots and the mutants
dealt kept from what a slot runs; the mutants dealt (each once, slot 0 a
mutant of each suite, a run given arguments in one slot) and narrowed; the
output the script's (in its order, its baseline slot 0's, each mutant's
lines its own, a verdict naming no suite the next's, the count summed, the
memory bound's line, a red baseline said as the script says it); a run
stopped from outside stopping its slots and what they started; and two runs
not sharing the slots at once.

At least one test of `scripts/tests/test_mutation_slots.py` must then fail.
Its suite runs with `PYTHONDONTWRITEBYTECODE=1`: a mutant of the same size
written in the second its bytecode was would otherwise run as the bytecode
says.

Run from the repository root; `just e14-parallel-mutants` records the
output. The source is restored after every mutant, whatever happens.
"""

import hashlib
import importlib.util
import json
import os
import pathlib
import signal
import subprocess
import sys
import tempfile
import time
from types import SimpleNamespace

ROOT = pathlib.Path(__file__).resolve().parent.parent
SLOTS = ROOT / "scripts/mutation_slots.py"
HARNESS = ROOT / "scripts/mutation_slots_mutations.py"
PROTOCOL = ROOT / "scripts/unittest_protocol.py"
# The outer process retains this authority before mutation. Fresh child harnesses
# load their local bytes, rather than a retained producer's sys.modules cache.
_protocol_spec = importlib.util.spec_from_file_location("local_unittest_protocol", PROTOCOL)
_protocol = importlib.util.module_from_spec(_protocol_spec)
_protocol_spec.loader.exec_module(_protocol)
strict_json = _protocol.strict_json

# (what is undone, file, anchor, replacement)
MUTANTS = [
    (
        "a run asked for slots runs the script's loop in the tree",
        SLOTS,
        "    n = asked()\n    if n <= 1:\n        return\n",
        "    n = asked()\n    if n <= 1 or True:\n        return\n",
    ),
    (
        "a slot runs the tree's copy of the script",
        SLOTS,
        "                    [sys.executable, str(slot / relative), *self.argv],\n",
        "                    [sys.executable, str(self.script), *self.argv],\n",
    ),
    (
        "a slot runs every mutant, not those dealt to it",
        SLOTS,
        "    mutants[:] = [mutants[i] for i in indices]\n",
        "    mutants[:] = list(mutants)\n",
    ),
    (
        "a slot's mutants are not checked against the count they were dealt from",
        SLOTS,
        "    if not total.isdigit() or int(total) != len(mutants):\n",
        "    if not total.isdigit():\n",
    ),
    (
        "no slots is a number of slots",
        SLOTS,
        "    if not raw.isdigit() or int(raw) < 1:\n",
        "    if not raw.isdigit():\n",
    ),
    (
        "the ask for slots reaches what a slot runs",
        SLOTS,
        "    env.pop(SLOTS, None)\n",
        "    pass\n",
    ),
    (
        "the mutants dealt reach what a slot runs",
        SLOTS,
        "    pick = os.environ.pop(PICK, None)\n",
        "    pick = os.environ.get(PICK)\n",
    ),
    (
        "every slot listens on the same ports",
        SLOTS,
        '            "PORT": str(port),\n',
        '            "PORT": env.get("PORT", str(DEFAULT_PORT)),\n',
    ),
    (
        "every slot shares one temporary directory",
        SLOTS,
        '            "TMPDIR": str(tmp),\n',
        '            "TMPDIR": env.get("TMPDIR", "/tmp"),\n',
    ),
    (
        "a slot builds in the tree's target",
        SLOTS,
        '            "CARGO_TARGET_DIR": str(slot / "target"),\n',
        '            "CARGO_TARGET_DIR": str(ROOT / "target"),\n',
    ),
    (
        "slot 0 is not dealt a mutant of each suite first",
        SLOTS,
        "        if suite(m) not in seen:\n",
        "        if suite(m) not in seen and i == 0:\n",
    ),
    (
        "a run given arguments is dealt to slots",
        SLOTS,
        "    if script.name in SEQUENTIAL or sys.argv[1:]:\n",
        "    if script.name in SEQUENTIAL:\n",
    ),
    (
        "the mutants are said in the slots' order, not the script's",
        SLOTS,
        "            for i in range(len(self.mutants)):\n",
        "            for i in [i for d in self.dealt for i in d]:\n",
    ),
    (
        "the slots do not wait for every baseline",
        SLOTS,
        "                self.lock.wait_for(lambda: all(s.said or s.skipped or s.ended for s in self.slots))\n",
        "                pass\n",
    ),
    (
        "a red baseline in one slot is not said as the script says it",
        SLOTS,
        "                red = next((s for s in self.slots if s.ended and s.code and not s.said), None)\n",
        "                red = None\n",
    ),
    (
        "a mutant's lines before its verdict are another's",
        SLOTS,
        "        self.said[i] = self.since + [raw]\n",
        "        self.said[i] = [raw]\n",
    ),
    (
        "a slot's baseline is all it said before its first verdict",
        SLOTS,
        "        cut = marks[-1] + 1 if marks else (0 if ended else len(self.head))\n",
        "        cut = len(self.head)\n",
    ),
    (
        "a verdict that names no suite is no mutant's",
        SLOTS,
        "    if pending and begins(line, pending[0][1]):\n        return 0\n",
        "    if False:\n        return 0\n",
    ),
    (
        "a longer description is taken for a shorter one",
        SLOTS,
        '    return after.startswith(":") or after in (" [", "  ")\n',
        "    return True\n",
    ),
    (
        "the count is the first slot's",
        SLOTS,
        "                if m:\n                    killed += int(m.group(1))\n",
        "                if m and s.k == 0:\n                    killed += int(m.group(1))\n",
    ),
    (
        "the memory bound's line forgets a stop",
        SLOTS,
        "            w.since += 1\n",
        "            pass\n",
    ),
    (
        "the run ends well though a slot's mutant survived",
        SLOTS,
        "            return next((s.code for s in self.slots if s.code), 0)\n",
        "            return 0\n",
    ),
    (
        "a slot is brought to the tree by whether a file is there, not by its content",
        SLOTS,
        "                    if not dp.exists() or not same(sp, dp):\n",
        "                    if not dp.exists():\n",
    ),
    (
        "what a run left in a slot is kept",
        SLOTS,
        "                if e.name not in wanted:\n                    remove(pathlib.Path(e.path))\n",
        "                if e.name not in wanted and False:\n                    remove(pathlib.Path(e.path))\n",
    ),
    (
        "node_modules is copied, not linked",
        SLOTS,
        '                if e.name == "node_modules" and e.is_dir(follow_symlinks=False):\n',
        "                if False:\n",
    ),
    (
        "the tree's target is copied into a slot",
        SLOTS,
        'def mirror(src, dst, keep=(".git", "target"), skip=()):\n',
        'def mirror(src, dst, keep=(".git",), skip=()):\n',
    ),
    (
        "a slot's git tracks nothing",
        SLOTS,
        '    git("read-tree", head, cwd=slot)\n',
        "",
    ),
    (
        "two runs share the slots at once",
        SLOTS,
        "            fcntl.flock(self.held, fcntl.LOCK_EX | fcntl.LOCK_NB)\n",
        "            pass\n",
    ),
    (
        "a run stopped from outside leaves its slots running",
        SLOTS,
        "            previous[sig] = signal.signal(sig, self.stopped_from_outside)\n",
        "            previous[sig] = signal.getsignal(sig)\n",
    ),
    (
        "a stopped slot root is not terminated",
        SLOTS,
        "                for owned in [*reversed(list(frozen.values())), s.identity]:\n",
        "                for owned in [*reversed(list(frozen.values()))]:\n",
    ),
    (
        "what a stopped slot started in a session of its own is left running",
        SLOTS,
        "                for owned in [*reversed(list(frozen.values())), s.identity]:\n",
        "                for owned in [s.identity]:\n",
    ),
    (
        "a reused discovered identity is pinned",
        SLOTS,
        "    if observed is None or (expected is not None and observed != expected):\n",
        "    if observed is None:\n",
    ),
    (
        "a reuse while opening a pidfd is accepted",
        SLOTS,
        "        if started_at(pid) != observed:\n",
        "        if False and started_at(pid) != observed:\n",
    ),
    (
        "a pinned signal is redirected to a numeric PID",
        SLOTS,
        "            signal.pidfd_send_signal(self.fd, sig)\n",
        "            os.kill(self.pid, sig)\n",
    ),
    (
        "parallel launch omits its capability probe",
        SLOTS,
        "            require_pidfds()\n",
        "            pass\n",
    ),
    (
        "launch cancellation is raised before the child handoff",
        SLOTS,
        "        if self.launching:\n",
        "        if False and self.launching:\n",
    ),
    (
        "repeated cancellation interrupts frozen-child cleanup",
        SLOTS,
        "        if self.stopping:\n            return\n",
        "        if False and self.stopping:\n            return\n",
    ),
    (
        "a live unidentified descendant is treated as absent",
        SLOTS,
        "                if any(p == pid for p, _ in descendants_of(root.pid)):\n",
        "                if False and any(p == pid for p, _ in descendants_of(root.pid)):\n",
    ),
    (
        "a descriptor closes without confirming process exit",
        SLOTS,
        "                        if not owned.wait_exited(deadline):\n",
        "                        if False and not owned.wait_exited(deadline):\n",
    ),
    (
        "unreadable live root state counts as exited",
        SLOTS,
        "            if owned.wait_exited(time.monotonic()):\n",
        "            if True or owned.wait_exited(time.monotonic()):\n",
    ),

    (
        "an invalid unittest exit earns a kill",
        PROTOCOL,
        "        if type(status) is not int or status != (1 if failed else 0):\n",
        "        if False and (type(status) is not int or status != (1 if failed else 0)):\n",
    ),
    (
        "contradictory unittest inventories are accepted",
        PROTOCOL,
        '        if report["failed"] != sorted(failures) or report["passed"] != sorted(successes):\n',
        '        if False and (report["failed"] != sorted(failures) or report["passed"] != sorted(successes)):\n',
    ),

]

PRODUCER = ROOT / "scripts/unittest_producer.py"
PROFILES = {
    "full": ["scripts.tests.test_mutation_slots", "scripts.tests.test_mutation_slot_processes",
             "scripts.tests.test_unittest_producer"],
    "mocked": ["scripts.tests.test_mutation_slot_processes." + name
               for name in ("Identities", "Freeze", "Startup", "ReviewRegressions", "ProducerResults")],
}
# Never deliberately break real-child cleanup. The mocked profile guards both
# process creation and signals/closes unless a case explicitly mocks the API.
MOCKED_ONLY = {
    "an invalid unittest exit earns a kill",
    "contradictory unittest inventories are accepted",
    "a run stopped from outside leaves its slots running",
    "a stopped slot root is not terminated",
    "what a stopped slot started in a session of its own is left running",
    "a reused discovered identity is pinned",
    "a reuse while opening a pidfd is accepted",
    "a pinned signal is redirected to a numeric PID",
    "parallel launch omits its capability probe",
    "launch cancellation is raised before the child handoff",
    "repeated cancellation interrupts frozen-child cleanup",
    "a live unidentified descendant is treated as absent",
    "a descriptor closes without confirming process exit",
    "unreadable live root state counts as exited",
}
PROCESS_TESTS = ROOT / "scripts/tests/test_mutation_slot_processes.py"
MUTANTS += [
    ('a report with different expected methods is accepted', PROTOCOL, '                or expected != sorted(set(expected)) or report["expected"] != expected):\n', '                or expected != sorted(set(expected)) or False):\n'),
    ('incomplete start or finish inventories are accepted', PROTOCOL, '            if not isinstance(report[key], list) or sorted(report[key]) != expected:\n', '            if False and (not isinstance(report[key], list) or sorted(report[key]) != expected):\n'),
    ('unexecuted or repeated method bodies are accepted', PROTOCOL, '                    or body["entered"] != 1 or body["completed"] != 1):\n', '                    or False):\n'),
    ('fixture events may earn a kill', PROTOCOL, '            if event["origin"] != "body":\n', '            if False and event["origin"] != "body":\n'),
    ('a refused or skipped producer may earn a kill', PROTOCOL, '        if report["refusals"] != []:\n', '        if False and report["refusals"] != []:\n'),
    ('failed subtest events invent failed method counts', PROTOCOL, '        return tuple(sorted(set(self.assertion_failed) | set(self.body_errors)))\n', '        return tuple(sorted(self.assertion_failed + self.body_errors))\n'),
    ('duplicate protocol keys are accepted', PROTOCOL, '            if key in result:\n', '            if False and key in result:\n'),
    ('producer cleanup uses communicate instead of a bounded wait', HARNESS, '            status = record.proc.wait(timeout=bound)\n', '            status = record.proc.communicate(timeout=bound)\n'),
    ('an already exited producer root hides unproved orphans', HARNESS, '            if record.identity is not None and record.identity.wait_exited(time.monotonic()):\n', '            if False and record.identity is not None and record.identity.wait_exited(time.monotonic()):\n'),
    ('producer root is reaped before it is pinned', HARNESS, '            record.identity = slots.pin(record.proc.pid)\n', '            record.proc.wait(timeout=bound)\n            record.identity = slots.pin(record.proc.pid)\n'),
    ('mocked profile leaves process creation unguarded', PROCESS_TESTS, "        apis += [(subprocess, 'Popen'), (signal, 'pidfd_send_signal')]\n", "        apis += [(signal, 'pidfd_send_signal')]\n"),
    ('body exception provenance is discarded', PRODUCER, "                state['errors'].append((error, 'body'))\n", "                state['errors'].append((error, 'fixture'))\n"),
    ('fixture exception provenance becomes body provenance', PRODUCER, "                    state['errors'].append((error, 'fixture'))\n", "                    state['errors'].append((error, 'body'))\n"),
    ('executed subtest provenance is discarded', PRODUCER, "                        and item['origin'] == 'body'\n", "                        and item['origin'] == 'fixture'\n"),
    ('loader failures become method inventories', PRODUCER, '    if loader.errors:\n', '    if False and loader.errors:\n'),
    ('producer executes despite changed frozen method IDs', PRODUCER, '        if expected != ids:\n', '        if False and expected != ids:\n'),
]
MOCKED_ONLY.update({'mocked profile leaves process creation unguarded', 'a refused or skipped producer may earn a kill', 'producer cleanup uses communicate instead of a bounded wait', 'producer root is reaped before it is pinned', 'failed subtest events invent failed method counts', 'unexecuted or repeated method bodies are accepted', 'duplicate protocol keys are accepted', 'an already exited producer root hides unproved orphans', 'a report with different expected methods is accepted', 'fixture events may earn a kill', 'incomplete start or finish inventories are accepted'})
MUTANTS += [
    ("secondary cleanup errors swallow cancellation", HARNESS,
     "    if cancellation is not None:\n",
     "    if False and cancellation is not None:\n"),
]
MOCKED_ONLY.add("secondary cleanup errors swallow cancellation")
MUTANTS += [
    ("output close replaces primary cancellation", HARNESS,
     '                secondary("output close", error)\n',
     '                primary = error\n'),
]
MOCKED_ONLY.add("output close replaces primary cancellation")
MUTANTS += [
    ('normal cleanup signals are not recorded', HARNESS, '        if pending is None:\n            pending = SystemExit(1)\n', '        if False and pending is None:\n            pending = SystemExit(1)\n'),
    ('a repeated callback replaces the first pending cancellation', HARNESS, '        if pending is None:\n            pending = SystemExit(1)\n', '        if True:\n            pending = SystemExit(1)\n'),
    ('secondary cancellation is reduced to ordinary diagnostic text', HARNESS, '            if pending is None:\n                pending = error\n', '            if False and pending is None:\n                pending = error\n'),
    ('pending cancellation is not promoted after releases', HARNESS, '    cancellation = primary if isinstance(primary, (SystemExit, KeyboardInterrupt)) else pending\n', '    cancellation = primary if isinstance(primary, (SystemExit, KeyboardInterrupt)) else None\n'),
]
MOCKED_ONLY.update({'secondary cancellation is reduced to ordinary diagnostic text', 'a repeated callback replaces the first pending cancellation', 'normal cleanup signals are not recorded', 'pending cancellation is not promoted after releases'})
MUTANTS += [
    ('deferred invocation results are treated as synchronous', PRODUCER, '        if deferred(value):\n', '        if False and deferred(value):\n'),
    ('runtime async generators are ignored', PRODUCER, '    return inspect.isawaitable(value) or inspect.isgenerator(value) or inspect.isasyncgen(value)\n', '    return inspect.isawaitable(value) or inspect.isgenerator(value)\n'),
    ('instance cleanup checks only the discarding outer hook', PRODUCER, '                return original_cleanup(observed, *args, **kwargs)\n', '                return original_cleanup(callback, *args, **kwargs)\n'),
    ('class and module cleanup callbacks bypass return observation', PRODUCER, '        return observed, positional, keywords\n', '        return callback, positional, keywords\n'),
    ('class fixtures bypass return observation', PRODUCER, "                self.bind(cls, name, staticmethod(self.invocation(getattr(cls, name), cls.__qualname__ + '.' + name)))\n", '                self.bind(cls, name, staticmethod(getattr(cls, name)))\n'),
    ('module fixtures bypass return observation', PRODUCER, "                    self.bind(module, name, self.invocation(callback, module.__name__ + '.' + name))\n", '                    self.bind(module, name, callback)\n'),
    ('restoration requeues consumed cleanups', PRODUCER, "            original[:] = scope['proxy']\n", "            original[:] = scope['initial']\n"),
    ('original fixture bindings are not restored', PRODUCER, '                setattr(target, name, original)\n', '                pass\n'),
    ('deferred invocation results are never closed', PRODUCER, '                    closed = close()\n', '                    closed = None\n'),
]
MUTANTS += [
    ('canonical execution and registration changes are ignored', PRODUCER, 'def canonical_problems():\n    problems = []\n', 'def canonical_problems():\n    return []\n    problems = []\n'),
    ('custom suite execution may omit fixtures', PRODUCER, '            require_suite(node)\n', '            pass\n'),
    ('effective instance execution hooks are not checked', PRODUCER, '        check_effective(test, _CASE_HOOKS, test.id())\n', '        pass\n'),
]
MUTANTS += [
    ('suite global dispatch helpers are not frozen', PRODUCER, "    (unittest.suite, '_isnotsuite'), (unittest.suite, '_call_if_exists'),\n", ''),
    ('runner and Outcome execution descriptors are not frozen', PRODUCER, '                          (_STD_RUNNER, _RUNNER_HOOKS), (_STD_OUTCOME, _OUTCOME_HOOKS)):\n', '                          ):\n'),
    ('suppressed captured exceptions are accepted', PRODUCER, "        refusals = self.refusals + ['unmatched captured exception: ' + label for label in unmatched]\n", '        refusals = self.refusals\n'),
    # Train-11: a subtest's own, killed by the suppressed-subtest test on
    # every Python since its fixture keeps the failure from addSubTest (it
    # filtered 3.10's _Outcome.errors, and on CI's 3.12 was refused for an
    # AttributeError instead, killing nothing of this).
    ("a suppressed subtest's captured exception is not unmatched", PRODUCER,
     "            unmatched += [state['id'] + ' subtest' for item in state['subtests'].values()\n"
     "                          if item['error'] is not None and not item['matched']]\n",
     '            pass\n'),
    ('a cached protocol replaces actual local validation authority', HARNESS, 'strict_json = _protocol.strict_json\n', '_protocol = __import__("unittest_protocol")\nstrict_json = _protocol.strict_json\n'),
    ('body errors are relabelled as assertion failures', PROTOCOL, '                body_errors.add(identity)\n', '                assertion_failed.add(identity)\n'),
]
MOCKED_ONLY.update({'a cached protocol replaces actual local validation authority', 'body errors are relabelled as assertion failures'})
MUTANTS += [
    ('deep malformed JSON escapes validation', PROTOCOL, '    except (ValueError, TypeError, KeyError, AttributeError, RecursionError):\n', '    except (ValueError, TypeError, KeyError, AttributeError):\n'),
]
MOCKED_ONLY.add('deep malformed JSON escapes validation')
MUTANTS += [
    ("slot spacing ignores the registry", SLOTS,
     "    port = base + k * stride\n", "    port = base + k * 200\n"),
    ("a reserved port above TCP's maximum is allocated", SLOTS,
     "    if base < 1 or port + maximum > 65535:\n", "    if base < 1:\n"),
    ("invalid later slot ports reach tree preparation", SLOTS,
     "            if self.dealt:\n                port_for_slot(len(self.dealt) - 1, self.environ, self.root)\n",
     "            pass\n"),
]
BOUND = 900
# The reviewed source's SHA-256. Train-11's numbering (b18b704e) changed only
# its first line, the ADR's placeholder to ADR-0357; reviewed again so
# (8facb409... before).
CLEANUP_SHA256 = "f9b98537d0ece843c5e5cf19b8e4ab2e5b71016fe389583f2eb0642d22f5960b"
_CLEANUP = None
FROZEN = {}
EVIDENCE = None
FROZEN_PRODUCER = None


def python_result(status, text, expected):
    """Compatibility counts from the one shared strict protocol validator."""
    result = _protocol.validate_result(status, text, expected)
    return result.valid, len(result.passed), len(result.failed)


def cleanup_module():
    """Retain reviewed code in memory before mutations; never import a mutant reaper."""
    global _CLEANUP
    if _CLEANUP is None:
        path = pathlib.Path(os.environ.get("PW_UNITTEST_CLEANUP", str(SLOTS)))
        source = path.read_bytes()
        if hashlib.sha256(source).hexdigest() != CLEANUP_SHA256:
            raise RuntimeError("reviewed pidfd cleanup source differs")
        spec = importlib.util.spec_from_file_location("retained_unittest_cleanup", path)
        module = importlib.util.module_from_spec(spec)
        exec(compile(source, str(path), "exec"), module.__dict__)
        _CLEANUP = module
    return _CLEANUP


def execute(argv, output, env, bound=BOUND):
    """One sole-reaper child; fresh file output and retained pidfd cleanup.

    Previously orphaned descendants without a retained live root have unproved
    ownership. Refuse that boundary, never guess a PID or process group.
    """
    slots = cleanup_module()
    slots.require_pidfds()
    owner = slots.Run(SLOTS, [], 1, [], root=ROOT)
    record = SimpleNamespace(proc=None, identity=None)
    owner.slots.append(record)
    previous, errors = {}, []
    status, primary, pending = None, None, None
    def cancelled(*args):
        nonlocal pending
        if pending is None:
            pending = SystemExit(1)
        owner.stopped_from_outside(*args)
    def secondary(label, error):
        nonlocal pending
        if isinstance(error, (SystemExit, KeyboardInterrupt)):
            if pending is None:
                pending = error
        errors.append(label + ": " + str(error))
    log = None
    try:
        log = open(output, "xb")
        for sig in (signal.SIGTERM, signal.SIGINT):
            previous[sig] = signal.signal(sig, cancelled)
        owner.launching = True
        try:
            record.proc = subprocess.Popen(argv, cwd=ROOT, env=env,
                                           stdout=log, stderr=subprocess.STDOUT,
                                           start_new_session=True)
            # Canonical failed-pin cleanup closes stdout. This is a file,
            # whose closure cannot wait for a child's inherited pipe.
            record.proc.stdout = log
            record.identity = slots.pin(record.proc.pid)
            if record.identity is None:
                raise RuntimeError("cannot pin unreaped unittest producer")
        finally:
            owner.launching = False
        if owner.cancel_requested:
            raise SystemExit(1)
        try:
            status = record.proc.wait(timeout=bound)
        except subprocess.TimeoutExpired:
            raise RuntimeError("unittest producer timed out")
    except BaseException as error:
        primary = error
        owner.stopping = True
        if record.proc is not None:
            try:
                if record.identity is not None and record.identity.wait_exited(time.monotonic()):
                    errors.append("root already exited; previously orphaned descendants have unproved ownership")
            except BaseException as error:
                secondary("exit probe", error)
            try:
                owner.stop()
            except BaseException as error:
                secondary("stop", error)
    finally:
        owner.stopping = True
        if log is not None:
            try:
                log.close()
            except BaseException as error:
                secondary("output close", error)
        if record.identity is not None:
            try:
                record.identity.close()
            except BaseException as error:
                secondary("close", error)
        for sig, handler in previous.items():
            try:
                signal.signal(sig, handler)
            except BaseException as error:
                secondary("restore handler", error)
    cancellation = primary if isinstance(primary, (SystemExit, KeyboardInterrupt)) else pending
    if cancellation is not None:
        if primary is not None and primary is not cancellation:
            errors.insert(0, "primary: " + type(primary).__name__ + ": " + str(primary))
        try:
            if errors:
                print("producer cleanup refused during cancellation: " + "; ".join(errors), file=sys.stderr, flush=True)
        finally:
            raise cancellation
    if primary is not None:
        if errors:
            raise RuntimeError("producer cleanup refused: " + "; ".join(errors)) from primary
        raise primary
    if errors:
        raise RuntimeError("producer cleanup refused: " + "; ".join(errors))
    return status


def clean_environment():
    env = {key: value for key, value in os.environ.items() if not key.startswith("PW_MUTANT_")}
    env["PYTHONDONTWRITEBYTECODE"] = "1"
    env["PYTHONPATH"] = str(ROOT) + os.pathsep + env.get("PYTHONPATH", "")
    return env


def invocation(profile, inventory=False, expected=None):
    folder = pathlib.Path(tempfile.mkdtemp(prefix="producer-", dir=EVIDENCE))
    report, output = folder / "report.json", folder / "output.txt"
    argv = [sys.executable, "-B", str(FROZEN_PRODUCER or PRODUCER), "--report", str(report)]
    if inventory:
        argv.append("--inventory")
    else:
        frozen = folder / "expected.json"
        frozen.write_text(json.dumps(expected) + "\n")
        argv += ["--expected", str(frozen)]
    argv += PROFILES[profile]
    try:
        status = execute(argv, output, clean_environment())
        text = report.read_text(encoding="utf-8")
        return status, text
    except Exception as error:
        print(f"producer REFUSED: {error}; evidence {folder}", flush=True)
        return None, ""


def freeze_profiles():
    global EVIDENCE, FROZEN_PRODUCER
    EVIDENCE = pathlib.Path(tempfile.mkdtemp(prefix="parallel-instrument-"))
    # Children running private producer regressions share this exact retained code.
    source = SLOTS.read_bytes()
    if hashlib.sha256(source).hexdigest() != CLEANUP_SHA256:
        raise RuntimeError("unmutated pidfd module differs from reviewed source")
    retained = EVIDENCE / "reviewed_pidfd.py"
    retained.write_bytes(source)
    os.environ["PW_UNITTEST_CLEANUP"] = str(retained)
    cleanup_module()
    FROZEN_PRODUCER = EVIDENCE / "reviewed_unittest_producer.py"
    FROZEN_PRODUCER.write_bytes(PRODUCER.read_bytes())
    (EVIDENCE / "unittest_protocol.py").write_bytes(PROTOCOL.read_bytes())
    for profile in PROFILES:
        status, text = invocation(profile, inventory=True)
        inventory = strict_json(text)
        if (status != 0 or not isinstance(inventory, dict) or set(inventory) != {"schema", "ids"}
                or type(inventory["schema"]) is not int or inventory["schema"] != 1
                or not isinstance(inventory["ids"], list) or not inventory["ids"]
                or any(type(item) is not str for item in inventory["ids"])
                or inventory["ids"] != sorted(set(inventory["ids"]))):
            raise RuntimeError("cannot freeze complete " + profile + " method inventory")
        FROZEN[profile] = inventory["ids"]
    print("frozen profiles: " + ", ".join(f"{name}={len(ids)}" for name, ids in FROZEN.items())
          + f"; evidence {EVIDENCE}", flush=True)


def run_tests(mocked=False):
    profile = "mocked" if mocked else "full"
    expected = FROZEN[profile]
    status, text = invocation(profile, expected=expected)
    return python_result(status, text, expected)


def preflight(mutants):
    """Every control must actually change exactly one anchored source input."""
    proof = []
    for name, path, anchor, replacement in mutants:
        original = path.read_bytes()
        encoded = anchor.encode()
        if anchor == replacement:
            raise ValueError("identical mutation substitution: " + name)
        if original.count(encoded) != 1:
            raise ValueError("mutation anchor is not unique: " + name)
        changed = original.replace(encoded, replacement.encode(), 1)
        before = hashlib.sha256(original).hexdigest()
        after = hashlib.sha256(changed).hexdigest()
        if changed == original or before == after:
            raise ValueError("mutation did not alter source SHA: " + name)
        compile(changed, str(path), "exec")
        proof.append({"name": name, "source": str(path), "anchor_occurrences": 1,
                      "original_sha256": before, "altered_sha256": after,
                      "original_bytes": len(original), "altered_bytes": len(changed)})
    return proof


def main():
    import mutation_baseline  # keeps what each command says, for a red baseline
    signal.signal(signal.SIGTERM, lambda *_: sys.exit(1))
    proof = preflight(MUTANTS)
    freeze_profiles()
    (EVIDENCE / "preflight.json").write_text(json.dumps(proof, sort_keys=True, indent=2) + "\n")
    print(f"preflight: {len(proof)} actual unequal source/SHA substitutions recorded", flush=True)
    built, passed, failed = run_tests()
    print(f"baseline: {passed} passed, {failed} failed", flush=True)
    if not built or failed or not passed:
        print("FAIL: the unmutated baseline is not green; no mutant can mean anything")
        mutation_baseline.explain()
        return 1
    survivors = 0
    for what, path, anchor, replacement in MUTANTS:
        original = path.read_bytes()
        encoded = anchor.encode()
        if original.count(encoded) != 1:
            print(f"{what}: ANCHOR NOT FOUND EXACTLY ONCE in {path.name}")
            survivors += 1
            continue
        try:
            path.write_bytes(original.replace(encoded, replacement.encode(), 1))
            built, passed, failed = run_tests(what in MOCKED_ONLY)
        finally:
            path.write_bytes(original)
        if not built:
            verdict = "REFUSED (invalid test producer; earns no kill)"
            survivors += 1
        elif failed:
            verdict = f"KILLED ({failed} of {passed + failed} test methods fail)"
        else:
            verdict = "SURVIVED"
            survivors += 1
        print(f"{what}: {verdict}", flush=True)
    print(f"{len(MUTANTS) - survivors} of {len(MUTANTS)} mutants killed")
    return 1 if survivors else 0


if __name__ == "__main__":
    sys.exit(main())
