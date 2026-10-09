"""`mutation_bound.py`: every process a mutation script starts, bounded in
memory.

Each test states one thing the bound does: a process past it is stopped
and said, wherever under the script it was started; one within it runs to
its end; only the script's own are read, each stop said once; one whose
line of parents, read again, is another's is left alone; what a process
holds counts what it wrote; the bound itself; and a watch that cannot read
the processes says so.

A test that watches real processes stops only those that carry its mark:
under a mutant that watches every process, it records the others, and
fails, where it would otherwise stop them.
"""

import ctypes
import importlib.util
import io
import os
import pathlib
import subprocess
import sys
import threading
import unittest

SCRIPTS = pathlib.Path(__file__).resolve().parent.parent


def load(name: str):
    spec = importlib.util.spec_from_file_location(name, SCRIPTS / f"{name}.py")
    module = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(module)
    return module


bound = load("mutation_bound")

MiB = 1 << 20

# Carried by every process these tests start, in its arguments.
MARK = f"mutation-bound-test-{os.getpid()}"


def holding(size, then="import time; time.sleep(15)"):
    """Python that writes `size` bytes, keeps them, and then runs `then`."""
    return f"held = b'x' * {size}; {then}"


def python(code):
    return [sys.executable, "-c", code, MARK]


def marked_only(refused):
    """A kill that stops a process carrying `MARK`, and records any other."""

    def kill(pid, sig):
        line = subprocess.run(
            ["ps", "-o", "command=", "-p", str(pid)], capture_output=True, text=True
        ).stdout
        if MARK in line:
            os.kill(pid, sig)
        else:
            refused.append((pid, line.strip()))

    return kill


class Fake:
    """Processes as a test states them: each one's parent, what it holds,
    and its name. Each reading of the parents takes the next table, the last
    again once they run out; each process read is recorded."""

    def __init__(self, parents, held):
        self.tables = parents if isinstance(parents, list) else [parents]
        self.holding = held
        self.read = []

    @property
    def table(self):
        return self.tables[0]

    @table.setter
    def table(self, parents):
        self.tables = [parents]

    def parents(self):
        table = self.tables[0]
        if len(self.tables) > 1:
            self.tables.pop(0)
        return dict(table)

    def held(self, pid):
        self.read.append(pid)
        return self.holding.get(pid)

    def name(self, pid):
        return f"p{pid}-0123456789abcdef"


class Bound(unittest.TestCase):
    def watch(self, limit):
        """A watch of this process's processes, stopping only those marked."""
        out = io.StringIO()
        self.refused = []
        watch = bound.Watch(bound=limit, every=0.05, out=out, kill=marked_only(self.refused))
        thread = threading.Thread(target=watch.run, daemon=True)
        thread.start()

        def ended():
            watch.stop()
            thread.join(5)

        self.addCleanup(ended)
        return watch, out

    def test_a_process_past_the_bound_is_stopped_and_said(self):
        watch, out = self.watch(128 * MiB)
        # Started by a shell the test started, as cargo starts a test
        # binary: watched as the script's own.
        r = subprocess.run(
            ["sh", "-c", '"$0" -c "$1" "$2"; exit $?', sys.executable, holding(512 * MiB), MARK],
            capture_output=True,
            timeout=60,
        )
        self.assertEqual(self.refused, [])
        self.assertEqual(r.returncode, 128 + 9)
        self.assertEqual(len(watch.stopped), 1)
        self.assertGreater(watch.stopped[0][1], 128 * MiB)
        self.assertRegex(
            out.getvalue(),
            r"\A  stopped `[^`]+`: it held more than 128 MiB, "
            r"the most a process may hold here\n\Z",
        )

    def test_a_process_within_the_bound_runs_to_its_end(self):
        watch, out = self.watch(512 * MiB)
        r = subprocess.run(python(holding(16 * MiB, "import time; time.sleep(1)")), timeout=60)
        self.assertEqual(self.refused, [])
        self.assertEqual(r.returncode, 0)
        self.assertEqual(watch.stopped, [])
        self.assertEqual(out.getvalue(), "")

    def test_only_the_scripts_own_are_read_each_stop_said_once(self):
        killed = []
        processes = Fake(
            # 10 is the script; 11 and 12 its; 20 another's.
            {1: 0, 10: 1, 11: 10, 12: 11, 20: 1},
            {1: 10 * MiB, 10: 900 * MiB, 11: 100 * MiB, 12: 300 * MiB, 20: 900 * MiB},
        )
        out = io.StringIO()
        watch = bound.Watch(
            bound=100 * MiB, out=out, root=10, platform=processes,
            kill=lambda pid, sig: killed.append(pid),
        )
        # Looked at again while it is still listed, as a process stopped and
        # not yet waited for is: stopped and said once.
        for _ in range(3):
            watch.look()
        self.assertEqual(killed, [12])
        self.assertEqual(set(processes.read), {11, 12})
        self.assertEqual(watch.stopped, [("p12", 300 * MiB)])
        self.assertEqual(
            out.getvalue(),
            "  stopped `p12`: it held more than 100 MiB, the most a process may hold here\n",
        )
        # One at the bound is within it.
        self.assertNotIn(11, killed)

    def test_one_whose_parents_read_again_are_anothers_is_left_alone(self):
        killed = []
        # Listed as the script's; read again, given since to another's.
        processes = Fake(
            [{1: 0, 10: 1, 12: 10}, {1: 0, 10: 1, 20: 1, 12: 20}],
            {12: 300 * MiB},
        )
        out = io.StringIO()
        watch = bound.Watch(
            bound=100 * MiB, out=out, root=10, platform=processes,
            kill=lambda pid, sig: killed.append(pid),
        )
        watch.look()
        self.assertEqual(killed, [])
        self.assertEqual(out.getvalue(), "")
        self.assertEqual(processes.read, [12])

    def test_a_number_used_again_is_watched_again(self):
        killed = []
        processes = Fake({1: 0, 10: 1, 12: 10}, {12: 300 * MiB})
        watch = bound.Watch(
            bound=100 * MiB, out=io.StringIO(), root=10, platform=processes,
            kill=lambda pid, sig: killed.append(pid),
        )
        watch.look()
        # Waited for, it is gone; a later process given its number is new.
        processes.table = {1: 0, 10: 1}
        watch.look()
        processes.table = {1: 0, 10: 1, 12: 10}
        watch.look()
        self.assertEqual(killed, [12, 12])

    def test_what_a_process_holds_counts_what_it_wrote(self):
        p = subprocess.Popen(
            python(holding(256 * MiB, "print('held', flush=True); import time; time.sleep(30)")),
            stdout=subprocess.PIPE,
            text=True,
        )
        try:
            self.assertEqual(p.stdout.readline(), "held\n")
            self.assertGreaterEqual(bound.PLATFORM.held(p.pid), 256 * MiB)
            self.assertIn(p.pid, bound.descendants(bound.PLATFORM.parents(), os.getpid()))
            self.assertIn("python", bound.PLATFORM.name(p.pid).lower())
        finally:
            p.kill()
            p.communicate()

    def test_the_processes_watched_are_those_the_root_started_and_theirs(self):
        parents = {1: 0, 10: 1, 11: 10, 12: 11, 13: 10, 20: 1, 21: 20}
        self.assertEqual(bound.descendants(parents, 10), {11, 12, 13})
        self.assertEqual(bound.descendants(parents, 13), set())

    def test_the_bound_is_4_GiB_or_a_quarter_of_the_machine(self):
        total = bound.total_memory()
        self.assertGreater(total, bound.GiB)
        self.assertEqual(bound.BOUND, min(4 * bound.GiB, total // 4))
        self.assertEqual(bound.shown(4 * bound.GiB), "4 GiB")
        self.assertEqual(bound.shown(128 * MiB), "128 MiB")

    def test_a_test_binary_is_named_without_its_hash(self):
        self.assertEqual(bound.plain("graphs-34050469c0ffee12"), "graphs")
        self.assertEqual(bound.plain("pw-dev-server"), "pw-dev-server")

    def test_the_structures_read_on_macos_are_the_sdks(self):
        # `struct proc_bsdshortinfo` and `struct rusage_info_v0`, as
        # <sys/proc_info.h> and <sys/resource.h> lay them out.
        self.assertEqual(ctypes.sizeof(bound.MacOS.Short), 64)
        self.assertEqual(ctypes.sizeof(bound.MacOS.Usage), 96)
        self.assertEqual(bound.MacOS.Usage.phys_footprint.offset, 72)

    def test_a_watch_that_cannot_read_the_processes_says_so(self):
        class Broken:
            def parents(self):
                raise PermissionError("no /proc")

        out = io.StringIO()
        watch = bound.Watch(out=out, platform=Broken())
        watch.run()
        self.assertEqual(
            out.getvalue(), "  the memory bound stopped watching: PermissionError('no /proc')\n"
        )

    def test_started_once_however_often_asked(self):
        first = bound.start()
        self.assertIs(bound.start(), first)
        self.assertEqual(
            [t for t in threading.enumerate() if t.name == "mutation-bound"][0].watch, first
        )


if __name__ == "__main__":
    unittest.main()
