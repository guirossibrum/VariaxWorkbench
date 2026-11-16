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



def variax_boot_stages(raw):
    """Two file-derived loader candidates separated by FF padding.

    This identifies the archived region's layout, not a physical boot-ROM
    protocol, boot-mode entry sequence or permission to program that region.
    """
    if len(raw)!=16384:
        raise ValueError('unexpected boot-region profile length')
    first=loader_records(raw,exact=False)
    split=0x400
    if first['consumed_bytes']>split:
        raise ValueError('first boot prefix overlaps second stage')
    if raw[first['consumed_bytes']:split]!=b'\xff'*(split-first['consumed_bytes']):
        raise ValueError('unexpected interstage padding')
    second=loader_records(raw[split:],exact=False)
    end=split+second['consumed_bytes']
    if raw[end:]!=b'\xff'*(len(raw)-end):
        raise ValueError('unexpected padding after second stage')
    return {'first_loader_candidate':first,'second_loader_candidate':second,
            'second_stage_byte_offset':split,
            'interstage_ff_padding_bytes':split-first['consumed_bytes'],
            'end_ff_padding_bytes':len(raw)-end,
            'scope':'File-derived stages only; chip ROM protocol and physical boot/recovery behavior unverified'}


def names_and_slots(templates, slots):
    # Compare immutable snapshots even when an analysis caller supplies bytearray.
    templates, slots = bytes(templates), bytes(slots)
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




def pod_word_section(data):
    """Observed POD 3.01 section length and additive checksum, not a signature."""
    if len(data)<8:
        raise ValueError('truncated word-section header')
    count=int.from_bytes(data[:4],'big')
    declared=int.from_bytes(data[4:8],'big')
    end=8+count*2
    if not count or end>len(data):
        raise ValueError('word-section length outside region')
    payload=data[8:end]
    computed=sum(int.from_bytes(payload[i:i+2],'big') for i in range(0,len(payload),2))&0xffffffff
    if computed!=declared:
        raise ValueError('word-section additive checksum mismatch')
    if data[end:]!=b'\xff'*(len(data)-end):
        raise ValueError('non-ff bytes beyond word section')
    return {'payload_bytes':len(payload),'word_count':count,'checksum_u32':declared,
            'checksum_matches':True,'padding_ff_bytes':len(data)-end,
            'payload_sha256':hashlib.sha256(payload).hexdigest()},payload


def pod_load_records(data):
    """Observed address-u32/count-u32/width-u16 records, all big endian.

    Width tags 1/2/3 consume 2/4/6 bytes per destination word. Static decoding
    supports program interpretation for width 3; this is not the generic ADI
    boot stream or an approved mapping to physical flash.
    """
    records=[];cursor=0;ranges=collections.defaultdict(list)
    while cursor<len(data):
        if cursor+10>len(data):raise ValueError('truncated POD load header')
        address=int.from_bytes(data[cursor:cursor+4],'big')
        count=int.from_bytes(data[cursor+4:cursor+8],'big')
        width=int.from_bytes(data[cursor+8:cursor+10],'big')
        if width not in (1,2,3) or not count:
            raise ValueError('unsupported POD word width or empty record')
        stop=cursor+10+count*width*2
        if stop>len(data) or address+count>0x100000000:
            raise ValueError('POD load payload or destination overflows')
        if len(records)>=65536:raise ValueError('POD load record limit')
        records.append({'header_byte_offset':cursor,'destination_word_raw':address,
                        'word_count':count,'width_tag_raw':width,
                        'payload_bytes':count*width*2,
                        'payload_sha256':hashlib.sha256(data[cursor+10:stop]).hexdigest()})
        ranges[width].append((address,address+count));cursor=stop
    if not records:raise ValueError('no POD load records')
    for intervals in ranges.values():
        intervals.sort()
        if any(a[1]>b[0] for a,b in zip(intervals,intervals[1:])):
            raise ValueError('overlapping POD destinations within width tag')
    return records


def pod_resource_directory(image, start, end):
    """Observed POD 3.01 big-endian key/word-offset pairs; not a flash map.

    A final ffffffff key supplies the end pointer. The following eight bytes
    are ff padding. Pointer units are two bytes relative to nominal 01000000.
    Validate containment and names rather than trusting plausible pointers.
    """
    if not 0 <= start < end <= len(image):
        raise ValueError('resource region outside image')
    pairs, cursor = [], start
    while cursor + 8 <= end:
        key = int.from_bytes(image[cursor:cursor+4], 'big')
        pointer = int.from_bytes(image[cursor+4:cursor+8], 'big')
        if pointer < 0x1000000:
            raise ValueError('resource pointer below observed nominal base')
        offset = 2 * (pointer - 0x1000000)
        if not start <= offset <= end:
            raise ValueError('resource pointer outside data regions')
        if pairs and offset <= pairs[-1][2]:
            raise ValueError('resource offsets are not strictly increasing')
        pairs.append((key, pointer, offset))
        cursor += 8
        if key == 0xffffffff:
            break
        if len(pairs) > 8192:
            raise ValueError('resource directory exceeds profile limit')
    else:
        raise ValueError('missing resource directory end entry')
    if len(pairs) < 2 or cursor+8 > pairs[0][2]:
        raise ValueError('empty or overlapping resource directory')
    if image[cursor:pairs[0][2]] != b'\xff' * (pairs[0][2]-cursor):
        raise ValueError('unexpected directory padding')
    records = []
    for (key, pointer, offset), following in zip(pairs, pairs[1:]):
        stop = following[2]
        if stop-offset < 64:
            raise ValueError('resource record too short for observed name fields')
        names = []
        for at in (offset, offset+32):
            try:
                text = image[at:at+32].decode('utf-16-be')
            except UnicodeDecodeError as error:
                raise ValueError('invalid resource name encoding') from error
            if any(c != '\0' and not 32 <= ord(c) < 127 for c in text):
                raise ValueError('unexpected characters in resource name')
            stripped = text.rstrip('\0')
            if '\0' in stripped:
                raise ValueError('non-padding zero within resource name')
            names.append(stripped)
        records.append({'key_raw':f'{key:08x}', 'pointer_raw':pointer,
                        'file_offset':offset, 'bytes':stop-offset,
                        'name_fields':names,
                        'sha256':hashlib.sha256(image[offset:stop]).hexdigest()})
    return {'nominal_pointer_base':0x1000000, 'bytes_per_pointer_unit':2,
            'directory_start':start, 'directory_end':cursor,
            'data_start':pairs[0][2], 'data_end':pairs[-1][2],
            'records':records,
            'status':'File-derived relationship; payload ISA, licensing, authentication and physical mapping unverified'}


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
            'boot_region_layout':variax_boot_stages(raw[0]),
            'named_records':names_and_slots(raw[2],raw[3]),
            'architecture_status':'Not validated by this parser; consult source-bound disassembly evidence; exact IC unproven',
            'record_roles_status':'Template/slot relationship observed; restoration/calibration semantics unverified'}
    if envelope['product_id']=='0003000a' and envelope['version_hex']=='03010000':
        if len(results)!=19 or any(r['type_raw']!=3 for r in results[6:12]):
            raise ValueError('unrecognized POD 3.01 data-region profile')
        sections=[]
        for index in (4,5):
            r=results[index]
            metadata,payload=pod_word_section(image[r['offset']:r['offset']+r['bytes']])
            metadata['region_index']=index
            if index==4:metadata['load_records']=pod_load_records(payload)
            sections.append(metadata)
        report['pod_301_candidates']={'word_sections':sections,
            'resource_directory':pod_resource_directory(
                image, results[6]['offset'], results[11]['offset']+results[11]['bytes'])}
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
