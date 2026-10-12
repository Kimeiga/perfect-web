"""The search oracle's D1 stand-in (spikes/own-renderer/kiokun-oracle/d1.mjs)
passes its own test, so an adapter bug cannot pass as the oracle agreeing
with the rewrite (the integrator's ruling of 2026-10-10, B4). Node 22's
`node:sqlite` runs it, the version `.nvmrc` names."""

import pathlib
import shutil
import subprocess
import unittest

ROOT = pathlib.Path(__file__).resolve().parent.parent.parent


class D1Adapter(unittest.TestCase):
    def test_the_adapter_passes_its_own_test(self) -> None:
        node = shutil.which("node")
        if node is None:
            self.skipTest("no node on this machine")
        r = subprocess.run(
            [node, "--test", "--no-warnings", "spikes/own-renderer/kiokun-oracle/d1.test.mjs"],
            cwd=ROOT,
            capture_output=True,
            text=True,
        )
        self.assertEqual(r.returncode, 0, r.stdout + r.stderr)
        self.assertIn("# pass 4", r.stdout)


if __name__ == "__main__":
    unittest.main()
