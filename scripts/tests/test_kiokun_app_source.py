"""`kiokun_app_source.py`: an oracle's copy of kiokun.com's files is the
commit at HEAD, never the working tree, and the owner's uncommitted work
is named as not read."""

import contextlib
import hashlib
import importlib.util
import io
import os
import pathlib
import subprocess
import tempfile
import unittest

SCRIPTS = pathlib.Path(__file__).resolve().parent.parent
spec = importlib.util.spec_from_file_location("kiokun_app_source", SCRIPTS / "kiokun_app_source.py")
source = importlib.util.module_from_spec(spec)
spec.loader.exec_module(source)


def git(repo: pathlib.Path, *args: str) -> None:
    env = {**os.environ, "GIT_CONFIG_GLOBAL": os.devnull, "GIT_CONFIG_NOSYSTEM": "1"}
    subprocess.run(
        ["git", "-C", str(repo), "-c", "user.name=t", "-c", "user.email=t@t", *args],
        check=True,
        capture_output=True,
        env=env,
    )


class Source(unittest.TestCase):
    def setUp(self) -> None:
        self.dir = tempfile.TemporaryDirectory()
        self.app = pathlib.Path(self.dir.name) / "app"
        (self.app / "src" / "lib").mkdir(parents=True)
        (self.app / "src" / "routes" / "[word]").mkdir(parents=True)
        (self.app / "src" / "lib" / "order.ts").write_text("committed\n")
        (self.app / "src" / "routes" / "[word]" / "+page.svelte").write_text("page\n")
        git(self.app, "init", "-q")
        git(self.app, "add", "-A")
        git(self.app, "commit", "-q", "-m", "the app")
        self.out = pathlib.Path(self.dir.name) / "out"

    def tearDown(self) -> None:
        self.dir.cleanup()

    def run_main(self, *paths: str) -> tuple[int, str]:
        printed = io.StringIO()
        with contextlib.redirect_stdout(printed):
            code = source.main(["x", str(self.app), str(self.out), *paths])
        return code, printed.getvalue()

    def test_the_commit_is_copied_and_the_working_tree_is_not(self) -> None:
        (self.app / "src" / "lib" / "order.ts").write_text("the owner's work\n")
        code, printed = self.run_main("src/lib/order.ts", "src/routes/[word]/+page.svelte=page.svelte")
        self.assertEqual(code, 0)
        self.assertEqual((self.out / "order.ts").read_text(), "committed\n")
        self.assertEqual((self.out / "page.svelte").read_text(), "page\n")
        digest = hashlib.sha256(b"committed\n").hexdigest()[:12]
        self.assertIn(f"src/lib/order.ts {digest}", printed)
        self.assertIn("the checkout has uncommitted work, not read: src/lib/order.ts", printed)

    def test_a_clean_checkout_names_no_work(self) -> None:
        code, printed = self.run_main("src/lib/order.ts")
        self.assertEqual(code, 0)
        self.assertNotIn("not read", printed)

    def test_a_file_not_committed_is_refused(self) -> None:
        (self.app / "src" / "lib" / "new.ts").write_text("new\n")
        code, _ = self.run_main("src/lib/new.ts")
        self.assertEqual(code, 1)
        self.assertFalse((self.out / "new.ts").exists())


if __name__ == "__main__":
    unittest.main()
