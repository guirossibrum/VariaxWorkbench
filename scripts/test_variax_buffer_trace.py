"""Editor-versus-firmware checks for Variax 0x43 and 0x3B. No device and no image."""

import unittest

from map_variax_buffer import decode_do_immediate

# A typed `do a1` program on the local DSP56300 emulator left its marker
# unchanged for a count of 0 and updated it for a count of 2. The test locks
# that result. It does not launch the emulator.
ZERO_COUNT_SKIPS = True
LIMIT = 1091


def editor_header(start, last):
    """Header words for one changed run, matching family/variax/write.rs."""
    count = last - start + 1
    if last + 1 == LIMIT:
        return (start, count)
    return (count, start)


def firmware_run(first, second, data_words):
    """What P:0x14433 does with one header: first is the count, second the start."""
    count, start = first, second
    if count == 0 and ZERO_COUNT_SKIPS:
        consumed = 0
    else:
        consumed = min(count, len(data_words))
    return {
        "start": start,
        "consumed": consumed,
        "short": max(0, count - len(data_words)),
        "last": None if consumed == 0 else start + consumed - 1,
    }


class BufferHandlerTests(unittest.TestCase):
    def test_do_immediate_count_matches_the_known_forms(self):
        self.assertEqual(decode_do_immediate(0x060680), 6)
        self.assertEqual(decode_do_immediate(0x064384), 0x443)
        self.assertIsNone(decode_do_immediate(0x06CC00))

    def test_a_normal_run_writes_the_editor_span(self):
        header = editor_header(10, 13)
        self.assertEqual(header, (4, 10))
        applied = firmware_run(*header, [1, 2, 3, 4])
        self.assertEqual(applied["consumed"], 4)
        self.assertEqual(applied["start"], 10)
        self.assertEqual(applied["last"], 13)
        self.assertEqual(applied["short"], 0)

    def test_a_run_that_stops_before_word_1090_stays_in_order(self):
        header = editor_header(1000, 1089)
        self.assertEqual(header, (90, 1000))
        applied = firmware_run(*header, [0] * 90)
        self.assertEqual(applied["last"], 1089)
        self.assertEqual(applied["short"], 0)

    def test_changing_only_word_1090_is_read_as_a_long_run_at_word_1(self):
        header = editor_header(1090, 1090)
        self.assertEqual(header, (1090, 1))
        applied = firmware_run(*header, [0x55])
        self.assertEqual(applied["start"], 1)
        self.assertEqual(applied["consumed"], 1)
        self.assertEqual(applied["last"], 1)
        self.assertEqual(applied["short"], 1089)

    def test_an_unknown_baseline_sends_a_zero_count_at_offset_1091(self):
        header = editor_header(0, 1090)
        self.assertEqual(header, (0, 1091))
        applied = firmware_run(*header, [0] * 1091)
        self.assertEqual(applied["consumed"], 0)
        self.assertEqual(applied["short"], 0)

    def test_host_and_firmware_checksums_differ_when_words_1089_or_1090_are_set(self):
        words = [0] * 1092
        words[1089] = 1
        host = sum(words[:1089]) & 0xFFFFFF
        firmware = sum(words[:1091]) & 0xFFFFFF
        words[1091] = host
        self.assertNotEqual(firmware, words[1091])
        self.assertEqual((firmware - host) & 0xFFFFFF, 1)


if __name__ == "__main__":
    unittest.main()
