"""Bounded counterexamples. These are NOT tests of the generated Pleris runtime.

Each model isolates one census invariant with intentionally explicit assumptions.
Passing these tests makes the counterexample reproducible, not Pleris correct.
"""
import itertools
import unittest


class FailureCounterexamples(unittest.TestCase):
    def test_G07_blind_restore_erases_later_confirmed_success(self):
        # A and B overlap. B commits +10, then A is rejected.
        before_a = 0
        confirmed_after_b = 10
        blindly_restored = before_a
        self.assertNotEqual(blindly_restored, confirmed_after_b)
        # A rejected intent must be removed from the speculative overlay,
        # not used to replace the latest confirmed base.
        pending = {"A": 1}
        del pending["A"]
        self.assertEqual(confirmed_after_b + sum(pending.values()), 10)

    def test_G07_commutative_counter_overlay_survives_all_completion_orders(self):
        # Restricted model: integer increments, no normalization or snapshots.
        # Not a proof for arbitrary Cart -> Cart transforms or server ordering.
        increments = {"A": 1, "B": 10, "C": 100}
        successful = {"B", "C"}
        for order in itertools.permutations(increments):
            confirmed = 0
            pending = dict(increments)
            for identity in order:
                amount = pending.pop(identity)
                if identity in successful:
                    confirmed += amount
                self.assertEqual(confirmed + sum(pending.values()),
                    sum(increments[i] for i in successful if i not in pending) + sum(pending.values()))
            self.assertEqual(confirmed, 110)

    def test_G06_timeout_does_not_distinguish_commit_from_no_commit(self):
        traces = [(False, "timeout"), (True, "timeout")]
        self.assertEqual(traces[0][1], traces[1][1])
        self.assertNotEqual(traces[0][0], traces[1][0])

    def test_C04_one_subscriber_cannot_abort_shared_work(self):
        owners = {"first", "second"}
        owners.remove("first")
        self.assertFalse(not owners)
        owners.remove("second")
        self.assertTrue(not owners)

    def test_I08_reading_a_frame_is_not_acknowledging_application(self):
        frames = [(1, "patch")]
        sent_to_dead_socket = list(frames)
        acknowledged = 0
        self.assertEqual([f for f in frames if f[0] > acknowledged], sent_to_dead_socket)
        acknowledged = 1  # Only after client application, under this model.
        self.assertEqual([f for f in frames if f[0] > acknowledged], [])

    def test_J01_equal_url_does_not_establish_equal_audience(self):
        first = ("/cart", "alice")
        second = ("/cart", "bob")
        self.assertEqual(first[0], second[0])
        self.assertNotEqual(first, second)

    def test_U01_missing_metric_is_not_a_measured_zero(self):
        absent = None
        measured_zero = 0.0
        self.assertNotEqual(absent, measured_zero)
        # A truthiness fallback incorrectly collapses the distinction.
        self.assertEqual(absent or 0, measured_zero or 0)


if __name__ == "__main__":
    unittest.main()
