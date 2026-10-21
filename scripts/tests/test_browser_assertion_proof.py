"""Native proof, rather than failed expect-step categories, owns browser kills."""
import json
import os
from pathlib import Path
import sys
import tempfile
import unittest

ROOT = Path(__file__).resolve().parents[2]
sys.path.insert(0, str(ROOT / 'scripts'))
import device_browser_protocol as protocol
import producer_process


class BrowserAssertions(unittest.TestCase):
    def case(self, mode, valid, failed=0):
        code, result, output = protocol.run_case(mode)
        self.assertEqual(code, 0 if mode in ('green', 'pollGreen', 'pollNaN') else 1, output)
        self.assertEqual((result.valid, result.failed), (valid, failed), output)

    def test_native_comparisons_messages_and_negation_produce_proof(self):
        for mode in ('assertion', 'customMessage', 'not'):
            self.case(mode, True, 1)

    @unittest.skipUnless(os.environ.get('PW_BROWSER_PROOF') == '1', 'explicit browser proof recipe')
    def test_native_locator_mismatch_produces_proof(self):
        self.case('locator', True, 1)

    @unittest.skipUnless(os.environ.get('PW_BROWSER_PROOF') == '1', 'explicit browser proof recipe')
    def test_closed_browser_is_not_a_comparison(self):
        self.case('closed', False)

    @unittest.skipUnless(os.environ.get('PW_BROWSER_PROOF') == '1', 'explicit browser proof recipe')
    def test_invalid_matcher_api_is_not_a_comparison(self):
        self.case('invalidMatcher', False)

    def test_poll_preserves_object_is_and_completed_mismatches(self):
        for mode, failed in (('poll', 1), ('pollGreen', 0), ('pollNaN', 0), ('pollSignedZero', 1)):
            self.case(mode, True, failed)

    def test_poll_callback_errors_are_not_assertion_results(self):
        for mode in ('pollCallback', 'pollAssertCallback'):
            self.case(mode, False)

    def test_pending_poll_callback_is_not_a_mismatch(self):
        self.case('pollPending', False)

    def test_native_cause_is_preserved_under_proof(self):
        self.case('cause', True, 1)

    def fixture(self, mode):
        with tempfile.TemporaryDirectory() as directory:
            receipt = Path(directory) / 'result.json'
            run = producer_process.run(['node', str(ROOT / 'spikes/own-renderer/assertion-result-fixture.mjs'), mode],
                env={**os.environ, 'PW_ASSERTION_REPORT': str(receipt), 'PW_ASSERTION_NONCE': 'a' * 64},
                timeout=10)
            self.assertEqual(run.returncode, 0, run.stdout)
            return json.loads(receipt.read_text()) if receipt.exists() else None

    def test_missing_native_proof_is_refused(self):
        self.assertIsNone(self.fixture('unproven'))
        self.assertIsNotNone(self.fixture('assertion'))

    def test_stale_native_proof_is_refused(self):
        self.assertIsNone(self.fixture('staleProof'))
        self.assertIsNotNone(self.fixture('preservedCause'))
