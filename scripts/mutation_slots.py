"""**A mutation script's mutants, run in parallel slots** (ADR-XXXX).

Each mutation script applies one mutant at a time in place, rebuilds, runs
its tests and restores: `store_accounts_mutations.py` took 37 minutes on a
32-thread machine, and CI's verification shards reached GitHub's limit of
345 minutes a job. `PW_MUTANT_SLOTS=N`, N above 1, deals a script's mutants
to N slots and runs them at once. Unset, or 1, nothing changes.

- **The tree is never mutated.** Every script imports `mutation_baseline`
  first in its `main()`; the import calls `engage()` here, which runs the
  slots and ends the process with the run's code before the script's loop
  is reached. A run stopped from outside leaves no mutant in the tree.
- **A slot is a copy of the tree** (`target/mutant-slots/<k>`, or under
  `PW_MUTANT_SLOT_DIR`): each file of the tree as it is, tracked, untracked
  and ignored, but its `.git`, its `target/` and each `node_modules/`, which
  is linked. Kept from run to run, and brought to the tree's state at each
  by content: a file that differs is written, its time now, so that cargo
  builds it again; one that does not is left, so that cargo's last build of
  it stands. What a stopped run left in a slot is undone at the next.
  - Its own git, at the tree's commit and reading the tree's objects
    (`objects/info/alternates`): a test that asks git what is tracked is
    answered in the slot as in the tree.
  - Its own `CARGO_TARGET_DIR`, `<slot>/target`, where the repository's
    scripts look for what cargo built.
  - Its own ports, `PORT + k x 200` (`PORT` 3141 where unset), and its own
    `TMPDIR`: two slots running one test that names a fixed temporary
    directory do not share it.
- **A slot runs the script's own copy**, its `MUTANTS` narrowed to those
  dealt to it, baseline and all: each slot's copy is shown green before any
  mutant there counts. Slot 0 is dealt one mutant of each suite first, so
  its baseline is the whole script's.
- **The output is the script's**, on its standard output: the baseline as
  slot 0 said it, each mutant's lines in the script's order, as soon as
  every mutant before it has been said, the count summed, and the memory
  bound's last line as `mutation_bound.Watch` says it over those lines. A
  slot's standard error is passed on as it comes.
- **A script that cannot share the machine with itself** (`SEQUENTIAL`, each
  with why) runs in one slot: in order, and still not in the tree.
"""

import fcntl
import os
import pathlib
import re
import shutil
import signal
import subprocess
import sys
import threading

ROOT = pathlib.Path(__file__).resolve().parent.parent

# How many slots a run asks for; in a slot, which mutants it runs, as
# `<how many the script has>:<i>,<j>,..`, and which slot it is.
SLOTS = "PW_MUTANT_SLOTS"
PICK = "PW_MUTANT_PICK"
SLOT = "PW_MUTANT_SLOT"
# Where the slots are kept, where not under the tree's `target/`.
WHERE = "PW_MUTANT_SLOT_DIR"

# Each slot's ports: `PORT + k * STRIDE` and the offsets the browser suites
# add (the largest, PORT+182, in spikes/own-renderer/playwright.config.mjs).
STRIDE = 200
DEFAULT_PORT = 3141

# The scripts that run in one slot, whatever `PW_MUTANT_SLOTS` says.
SEQUENTIAL = {
    "messages_mutations.py": "its hosts listen on `PW_MESSAGES_PORT`, 7141 where unset, not on `PORT`",
    "notifications_mutations.py": "its hosts listen on `PW_NOTIFICATIONS_PORT`, 6100 where unset, not on `PORT`",
    "bench_contract_mutations.py": (
        "it builds the Next.js and SvelteKit baselines, whose tools may write caches into "
        "`node_modules`, which the slots share"
    ),
}

# A mutant's verdict, as the scripts print it after its description.
VERDICT = re.compile(
    r": (?:KILLED|SURVIVED|ANCHOR NOT FOUND EXACTLY ONCE in |NOT RUN|DID NOT BUILD)"
)
COUNT = re.compile(rb"^(\d+) of (\d+) mutants killed$")
# A process the memory bound stopped, or found ended (mutation_bound.py).
STOPPED = re.compile(r"^  memory bound: stopped `(.*)`, which held more than ")
ENDED = re.compile(r"^  memory bound: `(.*)` ended before it was stopped$")
SUMMARY = b"memory bound: "


def asked(environ=None):
    """How many slots the environment asks for: 1 where it names none."""
    environ = os.environ if environ is None else environ
    raw = environ.get(SLOTS, "").strip()
    if not raw:
        return 1
    if not raw.isdigit() or int(raw) < 1:
        raise SystemExit(f"{SLOTS}={raw!r}: give a whole number of slots, 1 or more")
    return int(raw)


def description(mutant):
    """What a script prints of a mutant before its verdict: each element
    before its file (`mutation_anchors.py` reads the same)."""
    return [str(m) for m in mutant[:-3]]


def begins(line, mutant):
    """Whether `line` begins with the mutant's first element, followed by
    `:`, ` [` or two spaces, as each script prints it."""
    words = description(mutant)
    if not words or not line.startswith(words[0]):
        return False
    after = line[len(words[0]) : len(words[0]) + 2]
    return after.startswith(":") or after in (" [", "  ")


def verdict_of(line, pending):
    """Which of `pending` (a slot's mutants not yet said, `(index, mutant)`
    in the script's order) `line` is the verdict of: the first whose every
    element before its file the line holds, beginning with its first; else,
    where the line holds the first alone (`{what}: NOT RUN ..` names no
    suite), the next. None where the line is no verdict. A script that
    chose not to run a mutant (`row_reads_mutations.py` takes a part of a
    description) says nothing of it, and the next verdict names its own."""
    if VERDICT.search(line) is None:
        return None
    for j, (_, m) in enumerate(pending):
        if begins(line, m) and all(w in line for w in description(m)[1:]):
            return j
    if pending and begins(line, pending[0][1]):
        return 0
    return None


def says(line, mutant):
    """Whether `line` is `mutant`'s verdict, the next to be said."""
    return verdict_of(line, [(0, mutant)]) == 0


def suite(mutant):
    """The suite a mutant names, the element before its file, or None."""
    return mutant[-4] if len(mutant) >= 5 else None


def deal(mutants, n):
    """Each slot's mutants, as indices in the script's order. Slot 0 first
    takes the first mutant of each suite, so that its baseline is the
    script's whole; then each mutant goes to the slot that holds fewest of
    its suite, then fewest in all, then the first."""
    n = max(1, min(n, len(mutants)))
    slots = [[] for _ in range(n)]
    of_suite = [dict() for _ in range(n)]
    seen = set()
    for i, m in enumerate(mutants):
        if suite(m) not in seen:
            seen.add(suite(m))
            slots[0].append(i)
            of_suite[0][suite(m)] = of_suite[0].get(suite(m), 0) + 1
    for i, m in enumerate(mutants):
        if i in slots[0]:
            continue
        k = min(range(n), key=lambda k: (of_suite[k].get(suite(m), 0), len(slots[k]), k))
        slots[k].append(i)
        of_suite[k][suite(m)] = of_suite[k].get(suite(m), 0) + 1
    # A slot dealt nothing is not run.
    return [sorted(s) for s in slots if s]


def narrow(mutants, pick):
    """In a slot: `mutants` (the script's own list) narrowed in place to
    those dealt to it."""
    total, _, chosen = pick.partition(":")
    if not total.isdigit() or int(total) != len(mutants):
        raise SystemExit(
            f"{PICK}={pick!r} was dealt from {total} mutants; this script has {len(mutants)}"
        )
    indices = [int(i) for i in chosen.split(",") if i]
    mutants[:] = [mutants[i] for i in indices]


def script_globals():
    """The globals of the mutation script whose `main()` imports
    `mutation_baseline`: the nearest frame up the stack that holds
    `MUTANTS`, loaded from this tree's `scripts/*_mutations.py`."""
    frame = sys._getframe(1)
    while frame is not None:
        g = frame.f_globals
        file = g.get("__file__")
        if "MUTANTS" in g and file and file.endswith("_mutations.py"):
            if pathlib.Path(file).resolve().parent == ROOT / "scripts":
                return g
        frame = frame.f_back
    return None


def engage():
    """Called by `mutation_baseline` as it is imported. In a slot: narrow
    the script's `MUTANTS` to those dealt to it. Asked for slots: run them
    and end this process with the run's code. Else nothing."""
    g = script_globals()
    pick = os.environ.pop(PICK, None)
    os.environ.pop(SLOT, None)
    if g is None:
        return
    if pick is not None:
        narrow(g["MUTANTS"], pick)
        # Each line reaches the coordinator as it is printed.
        try:
            sys.stdout.reconfigure(line_buffering=True)
        except AttributeError:
            pass
        return
    n = asked()
    if n <= 1:
        return
    script = pathlib.Path(g["__file__"]).resolve()
    if script.name in SEQUENTIAL or sys.argv[1:]:
        # Given arguments, a script chooses its own mutants among those it
        # holds (`row_reads_mutations.py`), and slot 0's baseline would not
        # be the run's whole: one slot.
        n = 1
    sys.stdout.flush()
    code = Run(script, list(g["MUTANTS"]), n, sys.argv[1:]).run()
    sys.stdout.flush()
    sys.exit(code)


# ---------------------------------------------------------------- the slots


def base_dir(root=ROOT, environ=None):
    """Where the slots are kept."""
    environ = os.environ if environ is None else environ
    where = environ.get(WHERE)
    return pathlib.Path(where).resolve() if where else root / "target" / "mutant-slots"


def same(a, b):
    """Whether two regular files hold the same bytes."""
    if a.stat().st_size != b.stat().st_size:
        return False
    with open(a, "rb") as fa, open(b, "rb") as fb:
        while True:
            x, y = fa.read(1 << 20), fb.read(1 << 20)
            if x != y:
                return False
            if not x:
                return True


def remove(path):
    """A file, a link or a directory, gone."""
    if path.is_symlink() or not path.is_dir():
        path.unlink()
    else:
        shutil.rmtree(path)


def mirror(src, dst, keep=(".git", "target"), skip=()):
    """Make `dst` hold what `src` holds, by content: a file that differs is
    written (its time now), one that does not is left as it is; each
    `node_modules` is a link to `src`'s; what `src` does not hold goes.
    `keep` names what at `dst`'s top is its own and left; `skip` names
    paths under `src` not copied. Returns the files written and removed."""
    written = removed = 0
    skip = {pathlib.Path(p).resolve() for p in skip}
    stack = [(src, dst, True)]
    while stack:
        s, d, top = stack.pop()
        d.mkdir(parents=True, exist_ok=True)
        wanted = set()
        with os.scandir(s) as entries:
            for e in entries:
                if top and e.name in keep:
                    continue
                sp, dp = pathlib.Path(e.path), d / e.name
                if sp.resolve() in skip:
                    continue
                wanted.add(e.name)
                if e.name == "node_modules" and e.is_dir(follow_symlinks=False):
                    # Linked, never copied: the slots share what is installed.
                    if not (dp.is_symlink() and os.readlink(dp) == str(sp)):
                        if dp.exists() or dp.is_symlink():
                            remove(dp)
                        dp.symlink_to(sp)
                    continue
                if e.is_symlink():
                    target = os.readlink(sp)
                    if not (dp.is_symlink() and os.readlink(dp) == target):
                        if dp.exists() or dp.is_symlink():
                            remove(dp)
                        dp.symlink_to(target)
                        written += 1
                elif e.is_dir(follow_symlinks=False):
                    if dp.is_symlink() or (dp.exists() and not dp.is_dir()):
                        remove(dp)
                    stack.append((sp, dp, False))
                elif e.is_file(follow_symlinks=False):
                    if dp.is_symlink() or dp.is_dir():
                        remove(dp)
                    if not dp.exists() or not same(sp, dp):
                        shutil.copyfile(sp, dp)
                        written += 1
                    mode = sp.stat().st_mode & 0o7777
                    if dp.stat().st_mode & 0o7777 != mode:
                        os.chmod(dp, mode)
        with os.scandir(d) as entries:
            for e in entries:
                if top and e.name in keep:
                    continue
                if e.name not in wanted:
                    remove(pathlib.Path(e.path))
                    removed += 1
    return written, removed


def git(*args, cwd):
    return subprocess.run(
        ["git", *args], cwd=cwd, check=True, capture_output=True, text=True
    ).stdout.strip()


def git_of(root, slot):
    """The slot's own git: at the tree's commit, its index the commit's
    files, its objects read from the tree's repository."""
    if not (slot / ".git" / "HEAD").exists():
        git("init", "-q", str(slot), cwd=root)
    common = git("rev-parse", "--path-format=absolute", "--git-common-dir", cwd=root)
    alternates = slot / ".git" / "objects" / "info" / "alternates"
    alternates.parent.mkdir(parents=True, exist_ok=True)
    alternates.write_text(str(pathlib.Path(common) / "objects") + "\n")
    head = git("rev-parse", "HEAD", cwd=root)
    git("update-ref", "--no-deref", "HEAD", head, cwd=slot)
    git("read-tree", head, cwd=slot)


def env_of(k, slot, tmp, picked, total, environ=None):
    """A slot's environment: the run's, with its mutants, its target, its
    ports and its temporary directory, and without the ask for slots, so
    that nothing the slot runs deals again."""
    env = dict(os.environ if environ is None else environ)
    env.pop(SLOTS, None)
    port = int(env.get("PORT") or DEFAULT_PORT)
    env.update(
        {
            PICK: f"{total}:" + ",".join(str(i) for i in picked),
            SLOT: str(k),
            "CARGO_TARGET_DIR": str(slot / "target"),
            "PORT": str(port + k * STRIDE),
            "TMPDIR": str(tmp),
        }
    )
    return env


# ------------------------------------------------------------------ a run


class Slot:
    """One slot's script, and what it has said, read into the baseline,
    each mutant's lines (those since the last verdict, and its verdict),
    and what follows its last."""

    def __init__(self, k, picked, mutants):
        self.k = k
        self.pending = [(i, mutants[i]) for i in picked]
        self.head = []  # what it said before its first verdict
        self.baseline = None  # set at its first verdict, or its end
        self.since = []  # what it said since its last verdict
        self.said = {}  # each mutant's lines, by index
        self.skipped = set()  # each mutant it ran no verdict for
        self.closing = None  # what it said after its last verdict
        self.all = []  # every line, as it came
        self.proc = None
        self.code = None

    @property
    def ended(self):
        return self.closing is not None

    def hear(self, raw):
        """One line it printed."""
        self.all.append(raw)
        line = raw.decode("utf-8", "replace").rstrip("\n")
        j = verdict_of(line, self.pending)
        if j is None:
            (self.head if self.baseline is None else self.since).append(raw)
            return
        if self.baseline is None:
            self.split_head(ended=False)
        for i, _ in self.pending[:j]:
            self.skipped.add(i)
        i = self.pending[j][0]
        self.said[i] = self.since + [raw]
        self.since = []
        self.pending = self.pending[j + 1 :]

    def split_head(self, ended):
        """Its baseline: what it said up to its last `baseline` line. What
        follows was said in its first mutant's run, or, where it said no
        verdict, after its last. Without a `baseline` line, what it said
        before a verdict is its baseline, and what it said with none is
        all after."""
        marks = [n for n, l in enumerate(self.head) if l.startswith(b"baseline")]
        cut = marks[-1] + 1 if marks else (0 if ended else len(self.head))
        self.baseline, self.since = self.head[:cut], self.head[cut:]

    def end(self, code):
        if self.baseline is None:
            self.split_head(ended=True)
        self.code = code
        counted = any(COUNT.match(l.rstrip(b"\n")) for l in self.since)
        if counted:
            # A script that chose not to run a mutant said nothing of it.
            self.skipped.update(i for i, _ in self.pending)
            self.pending = []
        self.closing = self.since


class Run:
    """A script's run in `n` slots."""

    def __init__(self, script, mutants, n, argv, root=ROOT, environ=None, out=None, err=None):
        self.script = script
        self.mutants = mutants
        self.n = n
        self.argv = argv
        self.root = root
        self.environ = dict(os.environ if environ is None else environ)
        self.out = out or sys.stdout.buffer
        self.err = err or sys.stderr
        self.lock = threading.Condition()
        self.slots = []
        self.stopping = False
        import mutation_bound

        # The memory bound's account of the run, read from what it says.
        self.watch = mutation_bound.Watch(platform=None)

    # -- the slots' trees

    def prepare(self):
        base = base_dir(self.root, self.environ)
        base.mkdir(parents=True, exist_ok=True)
        self.held = open(base / ".lock", "w")
        try:
            fcntl.flock(self.held, fcntl.LOCK_EX | fcntl.LOCK_NB)
        except BlockingIOError:
            print(f"mutant slots: waiting for {base}, held by another run", file=self.err, flush=True)
            fcntl.flock(self.held, fcntl.LOCK_EX)
        trees = []
        for k in range(len(self.dealt)):
            slot, tmp = base / str(k), base / f"tmp-{k}"
            mirror(self.root, slot, skip=[base])
            git_of(self.root, slot)
            if tmp.exists():
                shutil.rmtree(tmp)
            tmp.mkdir(parents=True)
            trees.append((slot, tmp))
        return trees

    # -- what each slot says

    def listen(self, slot):
        for raw in iter(slot.proc.stdout.readline, b""):
            with self.lock:
                slot.hear(raw)
                self.lock.notify_all()
        code = slot.proc.wait()
        with self.lock:
            slot.end(code)
            self.lock.notify_all()

    def say(self, lines):
        for raw in lines:
            self.out.write(raw)
            self.account(raw.decode("utf-8", "replace").rstrip("\n"))
        self.out.flush()

    def account(self, line):
        """What the memory bound counts of one line said: a stop, a verdict,
        or the baseline's."""
        import mutation_bound

        w = self.watch
        if STOPPED.match(line):
            w.stopped.append((STOPPED.match(line).group(1), 0))
            w.since += 1
        elif ENDED.match(line):
            if w.stopped:
                w.stopped.pop()
            w.since = max(0, w.since - 1)
        elif line.startswith("baseline"):
            w.verdict("the baseline", counted=False)
        else:
            m = mutation_bound.Lines.VERDICT.match(line)
            if m:
                w.verdict(m.group("what"))

    def stop(self, why=None):
        """Every slot still running, stopped: its script by SIGTERM (it
        restores its slot's source), then what it started and left."""
        self.stopping = True
        for s in self.slots:
            if s.proc and s.proc.poll() is None:
                left = descendants_of(s.proc.pid)
                try:
                    os.killpg(s.proc.pid, signal.SIGTERM)
                except ProcessLookupError:
                    pass
                try:
                    s.proc.wait(timeout=30)
                except subprocess.TimeoutExpired:
                    os.killpg(s.proc.pid, signal.SIGKILL)
                    s.proc.wait()
                for pid, started in left:
                    if started_at(pid) == started:
                        try:
                            os.kill(pid, signal.SIGKILL)
                        except ProcessLookupError:
                            pass

    # -- the run

    def run(self):
        # Stopped from outside, the slots are stopped too (`merge`).
        previous = {}
        for sig in (signal.SIGTERM, signal.SIGINT):
            previous[sig] = signal.signal(sig, self.stopped_from_outside)
        self.held = None
        try:
            self.dealt = deal(self.mutants, self.n)
            trees = self.prepare()
            return self.merge(trees)
        finally:
            for sig, handler in previous.items():
                signal.signal(sig, handler)
            if self.held is not None:
                self.held.close()

    def stopped_from_outside(self, *_):
        raise SystemExit(1)

    def start(self, trees):
        total = len(self.mutants)
        relative = self.script.relative_to(self.root)
        for k, (picked, (slot, tmp)) in enumerate(zip(self.dealt, trees)):
            s = Slot(k, picked, self.mutants)
            s.proc = subprocess.Popen(
                [sys.executable, str(slot / relative), *self.argv],
                cwd=slot,
                env=env_of(k, slot, tmp, picked, total, self.environ),
                stdout=subprocess.PIPE,
                start_new_session=True,
            )
            self.slots.append(s)
            threading.Thread(target=self.listen, args=(s,), daemon=True).start()

    def merge(self, trees):
        try:
            self.start(trees)
            with self.lock:
                # Every slot past its baseline, or ended.
                self.lock.wait_for(lambda: all(s.said or s.skipped or s.ended for s in self.slots))
                red = next((s for s in self.slots if s.ended and s.code and not s.said), None)
            if red is not None:
                # As the script says a red baseline: that slot's words.
                self.stop()
                self.say(red.all)
                return red.code
            self.say(self.baseline())
            for i in range(len(self.mutants)):
                s = next(s for s in self.slots if i in self.dealt[s.k])
                with self.lock:
                    self.lock.wait_for(lambda: i in s.said or i in s.skipped or s.ended)
                    lines = s.said.get(i)
                    known = i in s.said or i in s.skipped
                if lines is not None:
                    self.say(lines)
                elif not known:
                    # The slot ended before it said this mutant's verdict:
                    # what it said last, then the bound's line.
                    self.stop()
                    self.say([l for l in s.closing if not l.startswith(SUMMARY)])
                    self.say_summary()
                    return s.code or 1
            with self.lock:
                self.lock.wait_for(lambda: all(s.ended for s in self.slots))
            self.close()
            return next((s.code for s in self.slots if s.code), 0)
        except BaseException:
            self.stop()
            raise

    def baseline(self):
        """The run's baseline: slot 0's, which was dealt a mutant of each
        suite; and, should another slot have said one it did not (no
        script does), that one after it."""
        said = list(self.slots[0].baseline)
        labels = {l.split(b":")[0] for l in said}
        for s in self.slots[1:]:
            for raw in s.baseline:
                label = raw.split(b":")[0]
                if raw.startswith(b"baseline") and label not in labels:
                    labels.add(label)
                    said.append(raw)
        return said

    def close(self):
        """What follows the last verdict: what any slot said there beside
        the count and the bound's line, once; the count summed; the
        bound's line."""
        killed = total = 0
        extra = []
        for s in self.slots:
            for raw in s.closing:
                line = raw.rstrip(b"\n")
                m = COUNT.match(line)
                if m:
                    killed += int(m.group(1))
                    total += int(m.group(2))
                elif not line.startswith(SUMMARY) and raw not in extra:
                    extra.append(raw)
        self.say(extra)
        self.say([f"{killed} of {total} mutants killed\n".encode()])
        self.say_summary()

    def say_summary(self):
        self.out.write((self.watch.summary() + "\n").encode())
        self.out.flush()


def descendants_of(pid):
    """(each process `pid` started, and theirs, with when each started),
    on Linux; nothing elsewhere."""
    import mutation_bound

    if mutation_bound.PLATFORM is not mutation_bound.Linux:
        return []
    parents = mutation_bound.Linux.parents()
    return [(p, started_at(p)) for p in mutation_bound.descendants(parents, pid)]


def started_at(pid):
    """When a process started, in clock ticks since boot (Linux), or None."""
    try:
        with open(f"/proc/{pid}/stat", "rb") as f:
            stat = f.read()
        return int(stat[stat.rfind(b")") + 2 :].split()[19])
    except (OSError, IndexError, ValueError):
        return None


def main():
    """`python3 scripts/mutation_slots.py --remove`: the slots, removed."""
    if sys.argv[1:] == ["--remove"]:
        base = base_dir()
        if base.exists():
            shutil.rmtree(base)
        print(f"removed {base}")
        return 0
    print(__doc__)
    return 2


if __name__ == "__main__":
    sys.exit(main())
