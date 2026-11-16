"""Synthetic loader-layout rejection checks; no vendor files or hardware."""
import unittest
from map_firmware import loader_records, pod_resource_directory, pod_word_section, pod_load_records


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


class PodDirectoryChecks(unittest.TestCase):
    @staticmethod
    def image():
        image=bytearray(b'\xff'*512)
        def pointer(offset):return (0x1000000+offset//2).to_bytes(4,'big')
        image[64:88]=(0x1000000).to_bytes(4,'big')+pointer(128)+(0x1010000).to_bytes(4,'big')+pointer(224)+b'\xff'*4+pointer(304)
        for offset,first,second in [(128,'Synthetic Amp','Example'),(224,'Synthetic Cab','')]:
            for at,text in [(offset,first),(offset+32,second)]:
                image[at:at+32]=text.ljust(16,'\0').encode('utf-16-be')
        return image

    def test_word_offsets_bound_two_distinct_records(self):
        image=self.image()
        result=pod_resource_directory(image,64,512)
        self.assertEqual(result['data_start'],128)
        self.assertEqual(result['data_end'],304)
        self.assertEqual([r['bytes'] for r in result['records']],[96,80])
        self.assertEqual(result['records'][1]['name_fields'],['Synthetic Cab',''])
        for end in range(88):
            with self.assertRaises(ValueError):pod_resource_directory(image[:end],64,end)

    def test_invalid_pointers_padding_and_name_fields_are_rejected(self):
        corruptions=[(68,(0xffffff).to_bytes(4,'big')), # nominal underflow
                     (68,(0x1001000).to_bytes(4,'big')), # beyond data
                     (76,(0x1000040).to_bytes(4,'big')), # duplicate pointer
                     (76,(0x1000041).to_bytes(4,'big')), # short record
                     (84,(0x1000070).to_bytes(4,'big')), # end not greater
                     (88,b'X'), # padding
                     (128,b'\xd8\x00'), # invalid UTF-16
                     (128,b'\x00\x01'), # nonprintable
                     (130,b'\x00\x00')] # embedded zero
        for at,value in corruptions:
            image=self.image();image[at:at+len(value)]=value
            with self.assertRaises(ValueError):pod_resource_directory(image,64,512)


class PodLoadChecks(unittest.TestCase):
    @staticmethod
    def record(address,count,width):
        return address.to_bytes(4,'big')+count.to_bytes(4,'big')+width.to_bytes(2,'big')+bytes(range(count*width*2))

    def test_load_records_consume_all_widths_and_reject_truncation(self):
        blocks=[self.record(0xc000,3,2),self.record(0x10000,2,1),self.record(0x8000,2,3)]
        data=b''.join(blocks)
        parsed=pod_load_records(data)
        self.assertEqual([r['payload_bytes'] for r in parsed],[12,4,12])
        boundaries={len(blocks[0]),len(blocks[0])+len(blocks[1])}
        for n in range(len(data)):
            if n not in boundaries:
                with self.assertRaises(ValueError):pod_load_records(data[:n])
        for bad in [self.record(0xffffffff,2,1),self.record(0,0,1),self.record(0,2,4),
                    self.record(0x8000,3,3)+self.record(0x8002,2,3)]:
            with self.assertRaises(ValueError):pod_load_records(bad)

    def test_section_checksum_integrity_and_padding_are_independent(self):
        payload=self.record(0x8000,2,3)
        checksum=sum(int.from_bytes(payload[i:i+2],'big') for i in range(0,len(payload),2))
        data=(len(payload)//2).to_bytes(4,'big')+checksum.to_bytes(4,'big')+payload
        report,recovered=pod_word_section(data+b'\xff'*6)
        self.assertEqual(recovered,payload)
        self.assertEqual(report['padding_ff_bytes'],6)
        for n in range(len(data)):
            with self.assertRaises(ValueError):pod_word_section(data[:n])
        for at in [0,4,10,20]:
            bad=bytearray(data);bad[at]^=0x80
            with self.assertRaises(ValueError):pod_word_section(bad)
        with self.assertRaises(ValueError):pod_word_section(data+b'other!')


if __name__=='__main__':unittest.main()
