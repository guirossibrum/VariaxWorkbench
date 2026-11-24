"""Synthetic checks for the offline factory measurement."""

import math
import unittest

from verify_factory_response import (
    Q23, eq_section, frequencies, magnitude, pickup_bandpass, reverb,
    resonator, section, vx, word_counts,
)


class FactoryResponseTests(unittest.TestCase):
    def test_packed_eq_and_resonator_order(self):
        # Transfer numerator 1 + z/2 + z²/4, denominator 1 - z/2 + z²/4.
        words = [Q23 // 16, Q23 // 8, Q23 // 4, Q23 // 16, 2 * Q23 - Q23 // 8]
        eq = ((eq_section(words, 0),),)
        self.assertAlmostEqual(magnitude(eq, 1., 1.), 7 / 3)
        self.assertAlmostEqual(magnitude(eq, -1., 1.), 3 / 7)
        # Half-scaled packed [b2, a2, a1], with b0 = -b2.
        words = [2 * Q23 - Q23 // 4, Q23 // 8, 0]
        path = ((resonator(words, 0),),)
        self.assertEqual(magnitude(path, 1., 1.), 0)
        self.assertAlmostEqual(magnitude(path, -1j, -1.), 4 / 3)

    def test_vx_exponent_and_pickup_bandpass(self):
        # A unity center-frequency inverse corresponds to fs/4.
        self.assertEqual(vx([128, Q23 // 2], 0), 1.)
        self.assertEqual(vx([127, Q23 // 2], 0), 0.5)
        path = ((pickup_bandpass(1., 2., 0.25),),)
        self.assertAlmostEqual(magnitude(path, -1j, -1.), 0.25)
        self.assertEqual(pickup_bandpass(1., 0., 0.), section(0.))

    def test_word_distribution_takes_shortest_24bit_distance(self):
        counts = word_counts([0, 5, 10], [2 * Q23 - 1, 3, 10], [(0, 2, 'test')])
        self.assertEqual(counts, {'test': {1: 1, 2: 1}})

    def test_reverb_toward_unity_and_at_unity_are_independent(self):
        firmware = [0] * 1092
        firmware[1] = 0x400
        firmware[643] = Q23 // 2
        rust = firmware.copy()
        rust[643] += 1
        windows = firmware.copy()
        windows[644] = Q23  # The signed minimum encodes -1.
        result = reverb({'firmware': firmware, 'host_std': rust, 'windows_53': windows})
        tank = result['host_std']['tank_feedback']
        self.assertEqual(tank['toward_unity_words'], [643])
        self.assertEqual(tank['toward_unity_max_counts'], 1)
        self.assertEqual(tank['at_or_past_unity_words'], [])
        self.assertEqual(result['windows_53']['tank_feedback']['at_or_past_unity_words'], [644])

    def test_frequency_grid_includes_endpoints_and_nyquist(self):
        grid = frequencies()
        self.assertEqual((grid[0][0], grid[-1][0]), (20., 20000.))
        z = next(z for f, z, _ in grid if f == 39062.5 / 2)
        self.assertTrue(math.isclose(z.real, -1.))


if __name__ == '__main__':
    unittest.main()
