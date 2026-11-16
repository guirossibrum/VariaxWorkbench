"""Synthetic loader-layout rejection checks; no vendor files or hardware."""
import unittest
from map_firmware import loader_records


def word(n):return n.to_bytes(3,'little')
def record(address,tag,words):return word(len(words))+word(address)+word(tag)+b''.join(word(n) for n in words)


class LoaderChecks(unittest.TestCase):
    def test_every_truncated_prefix_of_two_space_loader_is_rejected(self):
        data=record(0x200,0,[0,0x0c0200])+record(0x200,1,[123])+word(0)+word(0x200)
        result=loader_records(data)
        self.assertEqual(len(result['records']),2)
        self.assertEqual(result['entry_word_raw'],0x200)
        self.assertTrue(result['entry_covered_by_tag_zero'])
        self.assertEqual(result['trailing_unknown_bytes'],0)
        for length in range(len(data)):
            with self.assertRaises(ValueError):loader_records(data[:length])

    def test_unknown_tags_overflow_overlap_and_extra_data_are_rejected(self):
        tail=word(0)+word(0x200)
        for data in [record(0x200,3,[0])+tail,
                     record(0xffffff,0,[0,0])+tail,
                     record(0x200,0,[0,1])+record(0x201,0,[0])+tail,
                     record(0x200,0,[0])+tail+b'other',tail]:
            with self.assertRaises(ValueError):loader_records(data)

    def test_partial_prefix_preserves_unknown_tail_and_entry_is_only_metadata(self):
        data=record(0x200,0,[0])+word(0)+word(0x123456)
        result=loader_records(data+b'unknown tail',exact=False)
        self.assertEqual(result['trailing_unknown_bytes'],12)
        self.assertEqual(result['consumed_bytes'],len(data))
        self.assertFalse(result['entry_covered_by_tag_zero'])


if __name__=='__main__':unittest.main()
