"""**The browser host map's own tests, in `just ci`** (ADR-0356).

The host map (`spikes/own-renderer/host-registry.json`, `host-metadata.mjs`,
`host-plan.mjs`) decides which development servers each browser run starts:
only the selected tests' hosts. Its tests that start no host and need no
browser are `host-plan.test.mjs`'s, Node's own test runner's; they run here,
so that every push holds the map to what each spec reaches (a spec whose
host changed and whose registry entry did not fails), the registry to its
ports, and the build policy, the declared hosts and the planner's receipts
to what they say. The planner against the real Playwright CLI is
`host-plan.native.test.mjs`'s, run by `just e14-browser-host-map`.
"""

import pathlib
import shutil
import subprocess
import unittest

ROOT = pathlib.Path(__file__).resolve().parents[2]
SUITE = ROOT / "spikes/own-renderer/host-plan.test.mjs"


class HostMap(unittest.TestCase):
    def test_the_host_maps_node_tests_pass(self) -> None:
        node = shutil.which("node")
        self.assertIsNotNone(node, "node is not on PATH: the host map's tests need it")
        out = subprocess.run(
            [node, "--test", "--test-reporter=spec", str(SUITE)],
            cwd=SUITE.parent,
            capture_output=True,
            text=True,
            timeout=300,
        )
        self.assertEqual(out.returncode, 0, out.stdout[-12000:] + out.stderr[-4000:])
        # A run that ran nothing is no evidence: the file names its tests.
        self.assertRegex(out.stdout, r"(?m)^ℹ pass (\d+)$")
        passed = int(next(line for line in out.stdout.splitlines() if line.startswith("ℹ pass ")).split()[-1])
        self.assertGreaterEqual(passed, 20, out.stdout[-4000:])


if __name__ == "__main__":
    unittest.main()
