"""Bounded static firmware layout research; no device or firmware execution.

Only derived metadata is saved. Source firmware, disassembly and extracted
programs stay private. Counts/addresses below are file-derived loader candidates,
not approved flash offsets or evidence of recovery/physical compatibility.
"""
import argparse
import collections
import hashlib
import json
import math
from pathlib import Path

from inspect_firmware import ROOT, inspect, preserve


def u24(data, at):
    if at < 0 or at+3 > len(data):
        raise ValueError('truncated 24-bit loader field')
    return int.from_bytes(data[at:at+3], 'little')


def loader_records(data, exact=True):
    """Read the three-u24 header structure found in the 3.10 main region.

    A zero count ends with a second word, the nominal entry. Memory tags stay
    numeric: this parser does not execute the loader or remap flash addresses.
    The partial-prefix view of the boot region retains its unknown remainder.
    """
    cursor, records = 0, []
    while True:
        count, address = u24(data,cursor), u24(data,cursor+3)
        if count == 0:
            cursor += 6
            if not records: raise ValueError('loader has no load records')
            if exact and cursor != len(data): raise ValueError('data after loader terminator')
            ranges = collections.defaultdict(list)
            for r in records:
                ranges[r['memory_tag_raw']].append((r['destination_word_raw'], r['destination_word_raw']+r['word_count']))
            for intervals in ranges.values():
                intervals.sort()
                if any(a[1]>b[0] for a,b in zip(intervals,intervals[1:])):
                    raise ValueError('overlapping loader destination ranges')
            return {'records':records, 'entry_word_raw':address, 'consumed_bytes':cursor,
                    'trailing_unknown_bytes':len(data)-cursor,
                    'entry_covered_by_tag_zero':any(a<=address<b for a,b in ranges[0])}
        tag = u24(data,cursor+6)
        end = cursor+9+count*3
        if tag not in (0,1,2): raise ValueError('unrecognized memory tag in this loader profile')
        if address+count>0x1000000: raise ValueError('24-bit destination range overflows')
        if end>len(data): raise ValueError('truncated loader payload')
        if len(records)>=65536: raise ValueError('loader exceeds record limit')
        records.append({'header_byte_offset':cursor, 'word_count':count,
                        'destination_word_raw':address, 'memory_tag_raw':tag,
                        'payload_sha256':hashlib.sha256(data[cursor+9:end]).hexdigest()})
        cursor=end


def names_and_slots(templates, slots):
    # Only the observed 3.10 shapes, not a universal Line 6 format.
    if len(templates)!=50*2619 or len(slots)!=60*4096:
        raise ValueError('unexpected 3.10 template/slot lengths')
    def name(block):
        raw=block[2553:2571]
        text=b''.join(raw[i:i+3][::-1] for i in range(0,18,3))
        if any(c<32 or c>126 for c in text): raise ValueError('non-ASCII template name')
        return text.decode('ascii').rstrip(' ')
    blocks=[templates[i:i+2619] for i in range(0,len(templates),2619)]
    lookup=collections.defaultdict(list)
    for i,b in enumerate(blocks): lookup[b].append(i)
    return {'template_stride_bytes':2619, 'slot_stride_bytes':4096, 'name_byte_offset':2553,
            'template_candidates':[{'index':i,'name':name(b),'sha256':hashlib.sha256(b).hexdigest()} for i,b in enumerate(blocks)],
            'slot_candidates':[{'index':i,'name':name(slots[i*4096:i*4096+2619]),
                                'matching_template_indices':lookup[slots[i*4096:i*4096+2619]],
                                'prefix_sha256':hashlib.sha256(slots[i*4096:i*4096+2619]).hexdigest(),
                                'slot_sha256':hashlib.sha256(slots[i*4096:(i+1)*4096]).hexdigest()}
                               for i in range(60)]}


def analyze(data):
    envelope,image=inspect(data)
    results=[]
    for i,r in enumerate(envelope['regions']):
        raw=image[r['offset']:r['offset']+r['bytes']]
        counts=collections.Counter(raw)
        results.append(dict(r,index=i,all_ff=bool(raw) and counts[255]==len(raw),
                            entropy_bits_per_byte=-sum(n/len(raw)*math.log2(n/len(raw)) for n in counts.values())))
    report={'source_sha256':hashlib.sha256(data).hexdigest(),'envelope':envelope,
            'regions':results,'device_access':False,'firmware_executed':False,
            'scope':'File-derived structures; no flash destinations, authentication or recovery claim'}
    if envelope['product_id']=='00070004' and envelope['version_hex']=='03100000':
        if [r['type_raw'] for r in results]!=[5,0,2,1]:
            raise ValueError('unrecognized 3.10 region order')
        raw=[image[r['offset']:r['offset']+r['bytes']] for r in results]
        report['variax_310_candidates']={
            'main_loader':loader_records(raw[1]),
            'boot_region_prefix':loader_records(raw[0],exact=False),
            'named_records':names_and_slots(raw[2],raw[3]),
            'architecture_status':'Not validated by this parser; consult source-bound disassembly evidence; exact IC unproven',
            'record_roles_status':'Template/slot relationship observed; restoration/calibration semantics unverified'}
    return report


def main():
    parser=argparse.ArgumentParser(description=__doc__)
    parser.add_argument('files',nargs='+',type=Path)
    args=parser.parse_args()
    directory=ROOT/'analysis/reports';directory.mkdir(parents=True,exist_ok=True)
    for file in args.files:
        report=analyze(file.read_bytes())
        encoded=(json.dumps(report,indent=2,sort_keys=True)+'\n').encode()
        output=directory/('firmware-map-'+hashlib.sha256(encoded).hexdigest()[:16]+'.json')
        preserve(output,encoded);print(output)


if __name__=='__main__':
    main()
