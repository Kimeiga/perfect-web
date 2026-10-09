"""**A mutation script's processes, bounded in memory** (2026-10-09).

A mutant can make a test hold memory without end. On 2026-10-09 one took
CI's runner down, as it had the night before: `graphs_on_the_wire_mutations.py`'s
"the renderer takes a node twice" (`take` made `clone`) has the 100,000-node
chain of `a_value_as_deep_as_its_data_is_held_without_recursion` copy each
node's whole rest and keep it, five billion values. The test binary went
from 2.9 to 14.5 GiB in thirty seconds, the runner was left 0.3 GiB, and
the job was killed (exit 143) with nothing said of why. On macOS the binary
ended without a result too, and how was not recorded.

From when a script imports `mutation_baseline`, first in its `main()`, a
thread watches every process the script started and every process they
started, twice a second. One holding more than `BOUND` is killed, and the
script's output says so where it happened, before the mutant's verdict:

      memory bound: stopped `graphs`, which held more than 4 GiB

Its command fails as a test that cannot finish fails, and the mutant counts
as killed, as PIT counts a mutant's memory error and Stryker its timeout.

**A kill by the bound is its own kind** (cargo-mutants reports a timeout
apart from a test's kill, and a suite whose kills were the bound's would
look stronger than it is). The script's output is read as it is written:
each mutant's verdict line (`<what>: KILLED …` or `SURVIVED`) after a stop
names that mutant as one the bound may have killed alone. The script's last
line is the bound's, every run:

    memory bound: no process was stopped
    memory bound: a process was stopped in 1 of 19 mutants' runs, each perhaps killed by the bound alone: the renderer takes a node twice

and `ci_summary.py` lists the second kind in the run's summary.

Only the script's own processes are read, and one is stopped only where two
readings agree that it is the script's: the list of processes, walked down
from the script, and the process's own line of parents, read again and
walked up. A process another started is never stopped, though its number
were given again between the two.

What a process holds:
- on Linux, its resident and its swapped memory, `VmRSS` and `VmSwap` in
  `/proc/<pid>/status`;
- on macOS, its physical footprint, `ri_phys_footprint` from
  `proc_pid_rusage`, which counts what the kernel compressed (XNU's
  `task.c`: `internal_compressed` is a part of it), as its resident size
  does not. Under the bound, a Mac stops the binary above six seconds in.

`BOUND` is 4 GiB, or a quarter of the machine's memory where that is less.
Nothing else these scripts run comes near it: in CI's heartbeat the largest
other process was a `rustc` at 0.8 GiB.
"""

import atexit
import ctypes
import os
import re
import signal
import sys
import threading

# How often the processes are looked at, in seconds.
EVERY = 0.5

GiB = 1 << 30


def total_memory():
    """The machine's memory, in bytes."""
    return os.sysconf("SC_PAGE_SIZE") * os.sysconf("SC_PHYS_PAGES")


BOUND = min(4 * GiB, total_memory() // 4)


def shown(size):
    """A size as the line says it: `4 GiB`, `256 MiB`."""
    if size >= GiB and size % GiB == 0:
        return f"{size // GiB} GiB"
    if size >= GiB:
        return f"{size / GiB:.1f} GiB"
    return f"{size >> 20} MiB"


def plain(name):
    """A program's name without the hash cargo gives a test binary."""
    return re.sub(r"-[0-9a-f]{16}$", "", name)


def descends(parents, pid, root):
    """Whether `root` started `pid`, or a process it started did: `pid`'s
    line of parents, walked up."""
    seen = set()
    while pid in parents and pid not in seen:
        seen.add(pid)
        pid = parents[pid]
        if pid == root:
            return True
    return False


def descendants(parents, root):
    """The processes `root` started, and theirs, from each process's parent."""
    children = {}
    for pid, parent in parents.items():
        children.setdefault(parent, []).append(pid)
    out, stack = set(), list(children.get(root, []))
    while stack:
        pid = stack.pop()
        if pid in out or pid == root:
            continue
        out.add(pid)
        stack.extend(children.get(pid, []))
    return out


class Linux:
    """Processes read from `/proc`."""

    @staticmethod
    def parents():
        out = {}
        for entry in os.scandir("/proc"):
            if not entry.name.isdigit():
                continue
            try:
                with open(f"/proc/{entry.name}/stat", "rb") as f:
                    stat = f.read()
            except OSError:
                continue
            # The name, in parentheses, may hold spaces and parentheses; the
            # state and the parent follow the last `)`.
            out[int(entry.name)] = int(stat[stat.rfind(b")") + 2 :].split()[1])
        return out

    @staticmethod
    def held(pid):
        try:
            with open(f"/proc/{pid}/status", "rb") as f:
                status = f.read()
        except OSError:
            return None
        held = 0
        for line in status.splitlines():
            if line.startswith((b"VmRSS:", b"VmSwap:")):
                held += int(line.split()[1]) * 1024
        return held

    @staticmethod
    def name(pid):
        try:
            with open(f"/proc/{pid}/cmdline", "rb") as f:
                program = f.read().split(b"\0")[0]
            if program:
                return os.path.basename(program.decode("utf-8", "replace"))
            with open(f"/proc/{pid}/comm", "rb") as f:
                return f.read().decode("utf-8", "replace").strip()
        except OSError:
            return f"process {pid}"


class MacOS:
    """Processes read through `libproc`; each structure as the SDK's
    `<sys/proc_info.h>` and `<sys/resource.h>` declare it."""

    PROC_PIDT_SHORTBSDINFO = 13
    RUSAGE_INFO_V0 = 0
    SZOMB = 5

    class Short(ctypes.Structure):
        # struct proc_bsdshortinfo
        _fields_ = [
            ("pid", ctypes.c_uint32),
            ("ppid", ctypes.c_uint32),
            ("pgid", ctypes.c_uint32),
            ("status", ctypes.c_uint32),
            ("comm", ctypes.c_char * 16),
            ("flags", ctypes.c_uint32),
            ("uid", ctypes.c_uint32),
            ("gid", ctypes.c_uint32),
            ("ruid", ctypes.c_uint32),
            ("rgid", ctypes.c_uint32),
            ("svuid", ctypes.c_uint32),
            ("svgid", ctypes.c_uint32),
            ("rfu", ctypes.c_uint32),
        ]

    class Usage(ctypes.Structure):
        # struct rusage_info_v0
        _fields_ = [("uuid", ctypes.c_uint8 * 16)] + [
            (n, ctypes.c_uint64)
            for n in (
                "user_time",
                "system_time",
                "pkg_idle_wkups",
                "interrupt_wkups",
                "pageins",
                "wired_size",
                "resident_size",
                "phys_footprint",
                "proc_start_abstime",
                "proc_exit_abstime",
            )
        ]

    lib = None

    @classmethod
    def proc(cls):
        if cls.lib is None:
            lib = ctypes.CDLL("/usr/lib/libproc.dylib", use_errno=True)
            lib.proc_listallpids.argtypes = [ctypes.c_void_p, ctypes.c_int]
            lib.proc_listallpids.restype = ctypes.c_int
            lib.proc_pidinfo.argtypes = [
                ctypes.c_int,
                ctypes.c_int,
                ctypes.c_uint64,
                ctypes.c_void_p,
                ctypes.c_int,
            ]
            lib.proc_pidinfo.restype = ctypes.c_int
            lib.proc_pid_rusage.argtypes = [ctypes.c_int, ctypes.c_int, ctypes.c_void_p]
            lib.proc_pid_rusage.restype = ctypes.c_int
            lib.proc_pidpath.argtypes = [ctypes.c_int, ctypes.c_void_p, ctypes.c_uint32]
            lib.proc_pidpath.restype = ctypes.c_int
            cls.lib = lib
        return cls.lib

    @classmethod
    def parents(cls):
        lib = cls.proc()
        count = lib.proc_listallpids(None, 0)
        pids = (ctypes.c_int * (max(count, 0) + 256))()
        count = lib.proc_listallpids(pids, ctypes.sizeof(pids))
        out = {}
        info = cls.Short()
        for pid in pids[: max(count, 0)]:
            if pid <= 0:
                continue
            got = lib.proc_pidinfo(
                pid, cls.PROC_PIDT_SHORTBSDINFO, 0, ctypes.byref(info), ctypes.sizeof(info)
            )
            # A process that has ended and is not yet waited for holds
            # nothing more.
            if got == ctypes.sizeof(info) and info.status != cls.SZOMB:
                out[pid] = info.ppid
        return out

    @classmethod
    def held(cls, pid):
        usage = cls.Usage()
        if cls.proc().proc_pid_rusage(pid, cls.RUSAGE_INFO_V0, ctypes.byref(usage)) != 0:
            return None
        return usage.phys_footprint

    @classmethod
    def name(cls, pid):
        path = ctypes.create_string_buffer(4096)
        if cls.proc().proc_pidpath(pid, path, ctypes.sizeof(path)) > 0:
            return os.path.basename(path.value.decode("utf-8", "replace"))
        return f"process {pid}"


PLATFORM = {"linux": Linux, "darwin": MacOS}.get(sys.platform)


class Watch:
    """Watches the processes `root` started; stops each holding more than
    `bound`, and says so on `out` (the script's output where none is
    given)."""

    def __init__(self, bound=BOUND, every=EVERY, out=None, root=None, platform=PLATFORM, kill=os.kill):
        self.bound = bound
        self.every = every
        self.out = out
        self.root = root if root is not None else os.getpid()
        self.platform = platform
        self.kill = kill
        # Each process stopped: its name, and what it held.
        self.stopped = []
        self.done = threading.Event()
        self.seen = set()
        # The mutants' verdicts printed, those printed after a stop, and the
        # stops since the last verdict.
        self.verdicts = 0
        self.decided = []
        self.since = 0
        self.lock = threading.Lock()

    def say(self, line):
        out = self.out or sys.stdout
        # One write, so no other line splits it.
        out.write(line + "\n")
        out.flush()

    def look(self):
        """Stop each process past the bound, once."""
        parents = self.platform.parents()
        # A process waited for is gone; its number may come again.
        self.seen &= set(parents)
        for pid in sorted(descendants(parents, self.root) - self.seen):
            held = self.platform.held(pid)
            if held is None or held <= self.bound:
                continue
            # Its line of parents, read again: a number given since to a
            # process another started is not the script's.
            if not descends(self.platform.parents(), pid, self.root):
                continue
            name = plain(self.platform.name(pid))
            # Counted and said before it is stopped: the mutant's verdict,
            # which waits for it, comes after both.
            with self.lock:
                self.stopped.append((name, held))
                self.since += 1
            self.say(f"  memory bound: stopped `{name}`, which held more than {shown(self.bound)}")
            try:
                self.kill(pid, signal.SIGKILL)
            except ProcessLookupError:
                # It ended in the instant between: not stopped. A verdict
                # already read stays named, on the side of the bound.
                with self.lock:
                    self.stopped.pop()
                    self.since = max(0, self.since - 1)
                self.say(f"  memory bound: `{name}` ended before it was stopped")
                continue
            self.seen.add(pid)

    def verdict(self, what, counted=True):
        """A verdict the script printed, a mutant's or its baseline's: one
        the bound may have decided where a process was stopped since the
        last."""
        with self.lock:
            self.verdicts += counted
            if self.since:
                self.decided.append(what)
                self.since = 0

    def summary(self):
        """What the bound did in the script's run, its last line."""
        with self.lock:
            if not self.stopped:
                return "memory bound: no process was stopped"
            if not self.verdicts:
                names = ", ".join(f"`{n}`" for n, _ in self.stopped)
                return f"memory bound: {len(self.stopped)} stopped, {names}"
            mutants = [d for d in self.decided if d != "the baseline"]
            line = (
                f"memory bound: a process was stopped in {len(mutants)} of {self.verdicts} "
                f"mutants' runs, each perhaps killed by the bound alone: {'; '.join(mutants) or 'none'}"
            )
            if "the baseline" in self.decided:
                line += "; and in the baseline's"
            if self.since:
                line += f"; and {self.since} after the last verdict"
            return line

    def run(self):
        while not self.done.is_set():
            try:
                self.look()
            except Exception as e:  # said, never silent: the bound is gone
                self.say(f"  the memory bound stopped watching: {e!r}")
                return
            self.done.wait(self.every)

    def stop(self):
        self.done.set()


class Lines:
    """The script's output, passed on as it is written, each line read as
    it ends: a mutant's verdict, or the baseline's, told to `watch`."""

    VERDICT = re.compile(r"^(?P<what>\S.*?): (?:KILLED|SURVIVED)\b")

    def __init__(self, out, watch):
        self._out = out
        self._watch = watch
        self._line = ""

    def write(self, text):
        written = self._out.write(text)
        self._line += text
        *ended, self._line = self._line.split("\n")
        for line in ended:
            if line.startswith("baseline"):
                self._watch.verdict("the baseline", counted=False)
                continue
            m = self.VERDICT.match(line)
            if m:
                self._watch.verdict(m.group("what"))
        return written

    def flush(self):
        return self._out.flush()

    def __getattr__(self, name):
        return getattr(self._out, name)


def start(**kw):
    """Watch this process's processes from now on: once, however often
    this is called. Reads the script's output for its verdicts, and says
    what the bound did as the script's last line. None where the platform
    cannot be read, said."""
    for thread in threading.enumerate():
        if thread.name == "mutation-bound" and thread.is_alive():
            return thread.watch
    if PLATFORM is None and "platform" not in kw:
        print(f"  memory is not bounded on {sys.platform}", flush=True)
        return None
    watch = Watch(**kw)
    thread = threading.Thread(target=watch.run, name="mutation-bound", daemon=True)
    thread.watch = watch
    sys.stdout = Lines(sys.stdout, watch)
    atexit.register(lambda: watch.say(watch.summary()))
    thread.start()
    return watch
