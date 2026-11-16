"""Bounded i386 code/preset/erase oracle in private Unicorn memory; no device API.

Five transfer/erase methods, two predicate helpers and three response callbacks execute in an
emulated CPU. Every import and helper is trapped; recognized synchronization/transport calls are memory-only stubs. No
Windows loader, Wine, DLL entry point, driver, OS call or live transport runs.
This does not execute device firmware or establish update/recovery safety.
"""
import argparse
import hashlib
import json
from pathlib import Path
import struct

from unicorn import Uc, UC_ARCH_X86, UC_MODE_32, UC_HOOK_CODE, UC_HOOK_MEM_INVALID
from unicorn.x86_const import UC_X86_REG_EAX, UC_X86_REG_ECX, UC_X86_REG_ESP, UC_X86_REG_EIP
from inspect_firmware import preserve

SHA256='6786a97458a5a6e5d225e5c2d2804281a8e0a082a0d992f558ce041c144ae078'
ROOT=Path(__file__).resolve().parents[1]
if not __debug__:raise RuntimeError('oracle assertions must not be disabled')
STACK,RETURN,TRAPS,HEAP=0x21000000,0x22000000,0x23000000,0x24000000
SEND,ERASE=TRAPS+0xF000,TRAPS+0xF010
SENDER_ENTRY,ERASE_ENTRY,PATCH_ENTRY=0x100D46F0,0x100D4B10,0x100D4BF0
POD_SEGMENT_ENTRY=0x100D7330
BOOT_ENTRY=0x100D56A0
APPLICABILITY_ENTRY,FALLBACK_ENTRY=0x100D3C80,0x100D5C60
PORT_WAIT=TRAPS+0xF020
RESPONSE_CALLBACKS={0x100D5E40:0,0x100D5E70:-3022,0x100D5EA0:-3038}


class PrivateUpdaterOracle:
    def __init__(self,binary):
        if hashlib.sha256(binary).hexdigest()!=SHA256:
            raise ValueError('requires exact source fingerprint; not a generic DLL runner')
        pe=struct.unpack_from('<I',binary,0x3c)[0]
        if binary[pe:pe+4]!=b'PE\0\0' or struct.unpack_from('<H',binary,pe+4)[0]!=0x14c:
            raise ValueError('expected known PE/i386')
        optional=pe+24
        if struct.unpack_from('<H',binary,optional)[0]!=0x10b:raise ValueError('expected PE32')
        base=struct.unpack_from('<I',binary,optional+28)[0]
        size,headers=struct.unpack_from('<II',binary,optional+56)
        if base!=0x10000000 or size>16*1024*1024:raise ValueError('unexpected image map')
        self.cpu=Uc(UC_ARCH_X86,UC_MODE_32)
        self.cpu.mem_map(base,(size+4095)&~4095);self.cpu.mem_write(base,binary[:headers])
        table=optional+struct.unpack_from('<H',binary,pe+20)[0]
        for i in range(struct.unpack_from('<H',binary,pe+6)[0]):
            _,rva,length,offset=struct.unpack_from('<4I',binary,table+i*40+8)
            if offset+length>len(binary) or rva+length>size:raise ValueError('invalid section')
            self.cpu.mem_write(base+rva,binary[offset:offset+length])
        for address,length in [(0,4096),(STACK,0x20000),(RETURN,4096),(TRAPS,0x10000),(HEAP,0x40000)]:
            self.cpu.mem_map(address,length)
        self.cpu.mem_write(0,struct.pack('<I',0xffffffff))
        self.imports={};rva,limit=struct.unpack_from('<II',binary,optional+104)
        offset=0
        while True:
            if offset+20>limit:raise ValueError('unterminated import directory')
            lookup,_,_,dll_name,iat=struct.unpack('<5I',self.read(base+rva+offset,20))
            if (lookup,dll_name,iat)==(0,0,0):break
            dll=self.cstring(base+dll_name)
            for i in range(4096):
                symbol=self.u32(base+(lookup or iat)+4*i)
                if not symbol:break
                name=f'ordinal-{symbol&65535}' if symbol&0x80000000 else self.cstring(base+symbol+2)
                trap=TRAPS+16*len(self.imports)
                if trap>=SEND:raise ValueError('import trap limit')
                self.imports[trap]=dll+'!'+name;self.write32(base+iat+4*i,trap)
            else:raise ValueError('unterminated import table')
            offset+=20
        self.cpu.hook_add(UC_HOOK_CODE,self.hook)
        self.cpu.hook_add(UC_HOOK_MEM_INVALID,self.invalid)
        self.frames=[];self.events=[];self.wait_result=0;self.erase_result=0
        self.normalized_status=0

    def read(self,address,count):return bytes(self.cpu.mem_read(address,count))
    def u32(self,address):return struct.unpack('<I',self.read(address,4))[0]
    def write32(self,address,value):self.cpu.mem_write(address,struct.pack('<I',value&0xffffffff))
    def cstring(self,address):
        out=bytearray()
        for i in range(256):
            value=self.read(address+i,1)[0]
            if not value:return out.decode('ascii')
            out.append(value)
        raise ValueError('unterminated import name')
    def ret(self,pop=0,result=0):
        stack=self.cpu.reg_read(UC_X86_REG_ESP);target=self.u32(stack)
        self.cpu.reg_write(UC_X86_REG_EAX,result&0xffffffff)
        self.cpu.reg_write(UC_X86_REG_ESP,stack+4+pop)
        self.cpu.reg_write(UC_X86_REG_EIP,target)
    def invalid(self,cpu,access,address,size,value,context):
        raise RuntimeError(f'unmapped guest access at {address:#x}; pc={cpu.reg_read(UC_X86_REG_EIP):#x}')
    def hook(self,cpu,address,size,context):
        stack=cpu.reg_read(UC_X86_REG_ESP)
        if address==SEND:
            pointer,length,timeout=[self.u32(stack+4*i) for i in (1,2,3)]
            if length>4096 or sum(map(len,self.frames))+length>512*1024:raise RuntimeError('send buffer limit')
            self.frames.append(self.read(pointer,length));self.events.append(['send',length,timeout]);self.ret(12,HEAP+0xF000);return
        if address==PORT_WAIT:
            self.events.append(['port_wait_stub_ms',self.u32(stack+4)])
            self.ret(4,0);return
        if address==ERASE:
            self.events.append(['delegated_erase',self.u32(stack+4)]);self.ret(4,self.erase_result);return
        if address in self.imports:
            name=self.imports[address].split('!')[-1]
            if name=='SetEvent':self.events.append(['set_event_stub']);self.ret(4,1);return
            if name=='ResetEvent':self.events.append(['reset_event']);self.ret(4,1);return
            if name=='Sleep':self.events.append(['sleep_stub_ms',self.u32(stack+4)]);self.ret(4);return
            if name=='WaitForSingleObject':
                self.events.append(['wait_stub_ms',self.u32(stack+8),self.wait_result])
                if self.wait_result==0:self.write32(HEAP+0x14c,0)
                self.ret(8,self.wait_result);return
            raise RuntimeError('unimplemented OS import blocked: '+self.imports[address])
        if address==0x10128430:
            # MSVC stack probe: reserve guest stack only; never touch host stack.
            allocation=cpu.reg_read(UC_X86_REG_EAX)
            if allocation!=0x19e4:raise RuntimeError('unexpected guest stack probe')
            target=self.u32(stack)
            cpu.reg_write(UC_X86_REG_ESP,stack+4-allocation)
            cpu.reg_write(UC_X86_REG_EIP,target)
            self.events.append(['guest_stack_probe',allocation]);return
        if address in (0x100D3A20,0x100D6A20):self.events.append(['debug_log_stub']);self.ret();return
        if address==0x100C9350:self.events.append(['status_adapter',self.normalized_status]);self.ret(0,self.normalized_status);return
        if address in (0x1003BB60,0x10126BBE):
            self.events.append(['pump_or_cookie_adapter',hex(address)])
            # Check the same guest cookie value and preserve the return register.
            if address==0x10126BBE and cpu.reg_read(UC_X86_REG_ECX)!=self.u32(0x1028D330):
                raise RuntimeError('guest stack cookie mismatch')
            self.ret(result=cpu.reg_read(UC_X86_REG_EAX) if address==0x10126BBE else 0);return
        if not (SENDER_ENTRY<=address<0x100D4AA5 or ERASE_ENTRY<=address<0x100D4BE0 or PATCH_ENTRY<=address<0x100D5001 or any(start<=address<start+0x27 for start in RESPONSE_CALLBACKS) or POD_SEGMENT_ENTRY<=address<0x100D758E or BOOT_ENTRY<=address<0x100D59A5 or APPLICABILITY_ENTRY<=address<0x100D3D38 or FALLBACK_ENTRY<=address<0x100D5C94):
            raise RuntimeError(f'guest code outside audited method ranges: {address:#x}')

    def call(self,entry,args,keep=True,boot=False,product=0x70004,wait=0,erase_result=0,version=0x03100000,cached_version=0x03100000):
        self.frames=[];self.events=[];self.wait_result=wait;self.erase_result=erase_result
        self.cpu.mem_write(HEAP,bytes(0x40000));self.cpu.mem_write(STACK,bytes(0x20000))
        self.cpu.mem_write(0,struct.pack('<I',0xffffffff))
        self.write32(HEAP,HEAP+0x1000);self.write32(HEAP+0x1024,ERASE)
        self.write32(HEAP+0x64,HEAP+0x1100);self.write32(HEAP+0x1140,SEND);self.write32(HEAP+0x112c,PORT_WAIT)
        self.write32(HEAP+0x4c,cached_version);self.write32(HEAP+0xc,version);self.write32(HEAP+0x44,int(boot));self.write32(HEAP+0x48,product)
        self.write32(HEAP+0x54,int(keep));self.write32(HEAP+0x148,HEAP+0x3100);self.write32(HEAP+0x3100,1)
        if entry in (SENDER_ENTRY,PATCH_ENTRY):
            payload=args[0]
            if len(payload)>(245760 if entry==PATCH_ENTRY else 4096):raise ValueError('synthetic payload cap')
            if entry==PATCH_ENTRY:
                expected_length=245760 if product==0x70005 or (product==0x70004 and version>0x0303ffff) else 196608
                if len(payload)!=expected_length:raise ValueError('synthetic patch layout length')
            self.cpu.mem_write(HEAP+0x4000,payload);self.write32(HEAP+0x2008,HEAP+0x4000);self.write32(HEAP+0x200c,len(payload))
            arguments=[0,HEAP+0x2000,max(1,len(payload)//3),HEAP+0x3000]
        elif entry in (POD_SEGMENT_ENTRY,BOOT_ENTRY):
            payload=args[0]
            if entry==POD_SEGMENT_ENTRY and (not 2<=len(payload)<=4096 or len(payload)%2):raise ValueError('synthetic POD word span')
            if entry==BOOT_ENTRY and not 1<=len(payload)<=16384:raise ValueError('synthetic boot byte span')
            self.cpu.mem_write(HEAP+0x4000,payload)
            self.write32(HEAP+0x2008,0);self.write32(HEAP+0x200c,len(payload))
            arguments=[HEAP+0x4000,HEAP+0x2000,max(1,len(payload)//(3 if entry==BOOT_ENTRY else 2)),HEAP+0x3000]
        elif entry==ERASE_ENTRY:arguments=args
        elif entry==APPLICABILITY_ENTRY:arguments=[product,HEAP+0x4c]
        elif entry==FALLBACK_ENTRY:arguments=[]
        elif entry in RESPONSE_CALLBACKS:
            self.write32(HEAP+0x14c,-5);arguments=[0,HEAP]
        else:raise ValueError('only audited methods/callbacks may be entered')
        stack=STACK+0x1F000;self.cpu.mem_write(stack,struct.pack('<'+'I'*(len(arguments)+1),RETURN,*arguments))
        self.cpu.reg_write(UC_X86_REG_ESP,stack);self.cpu.reg_write(UC_X86_REG_ECX,HEAP)
        self.cpu.emu_start(entry,RETURN,count=5_000_000 if entry==PATCH_ENTRY else 2_000_000,timeout=10_000_000 if entry==PATCH_ENTRY else 2_000_000)
        if self.cpu.reg_read(UC_X86_REG_EIP)!=RETURN:raise RuntimeError('bounded guest call did not return')
        return {'frames':list(self.frames),'events':list(self.events),'eax':self.cpu.reg_read(UC_X86_REG_EAX)}


def nibbles(value):return bytes((value>>(4*i))&15 for i in range(6))
def expected_frames(payload):
    words=[int.from_bytes(payload[i:i+3],'little') for i in range(0,len(payload)//3*3,3)]
    header=bytes([240,0,1,12,7,0,96])+nibbles(len(words)+1)
    chunks=[b''.join(nibbles(w) for w in words[i:i+32]) for i in range(0,len(words),32)]
    return [header]+chunks+[nibbles(sum(words)&0xffffff),bytes([247])]


def main():
    parser=argparse.ArgumentParser(description=__doc__);parser.add_argument('dll',type=Path);args=parser.parse_args()
    if not args.dll.is_file() or args.dll.stat().st_size>16*1024*1024:raise ValueError('expected bounded regular DLL file')
    oracle=PrivateUpdaterOracle(args.dll.read_bytes());results=[]
    for count in [0,1,2,31,32,33,64,65]:
        payload=b''.join(((i*0x12345+0x654321)&0xffffff).to_bytes(3,'little') for i in range(count))
        outcome=oracle.call(SENDER_ENTRY,[payload]);assert outcome['frames']==expected_frames(payload)
        results.append({'case':'code_sender','words':count,'frame_lengths':list(map(len,outcome['frames'])),'events':outcome['events'],'outbound_sha256':hashlib.sha256(b''.join(outcome['frames'])).hexdigest()})
    # These are adversarial host-method inputs, not approved receiver packets.
    for length in [1,2,4,7]:
        payload=bytes(range(length));outcome=oracle.call(SENDER_ENTRY,[payload])
        assert outcome['frames']==expected_frames(payload)
        results.append({'case':'nonword_sender_input','bytes':length,'whole_words_sent':length//3,
                        'ignored_trailing_bytes':length%3,'frame_lengths':list(map(len,outcome['frames']))})
    payload=(0x123456).to_bytes(3,'little')
    for result in [0,-1]:
        outcome=oracle.call(SENDER_ENTRY,[payload],keep=False,erase_result=result)
        assert ['delegated_erase',0] in outcome['events']
        assert outcome['frames']==(expected_frames(payload) if result==0 else [])
        results.append({'case':'preserve_flag_clear','erase_delegate_result':result,
                        'frame_lengths':list(map(len,outcome['frames']))})
    oracle.normalized_status=-1
    try:
        oracle.call(SENDER_ENTRY,[payload])
    except RuntimeError as error:
        assert str(error)=='guest code outside audited method ranges: 0x100d4aa5'
        results.append({'case':'send_error_handler','result':'stopped at audited normal-body boundary before unmodeled exception handling; no host forwarding'})
    else:raise AssertionError('guest error-handler boundary did not stop')
    # Discard the stopped CPU rather than resuming its incomplete error path.
    oracle=PrivateUpdaterOracle(args.dll.read_bytes())
    for factory,boot,wait in [(False,False,0),(True,False,0),(False,True,0),(False,False,0x102)]:
        outcome=oracle.call(ERASE_ENTRY,[int(factory)],boot=boot,wait=wait)
        expected=[] if boot else [bytes([240,0,1,12,7,0,104 if factory else 99,247])]
        assert outcome['frames']==expected and outcome['eax']==0
        results.append({'case':'erase_method','factory':factory,'boot_flag':boot,'wait_result_stub':wait,'native_return':outcome['eax'],'frame_lengths':list(map(len,outcome['frames'])),'events':outcome['events']})
    for version,stride in [(0x03100000,4096),(0x03030000,3276)]:
        # No vendor presets: each synthetic slot has a distinct deterministic pattern.
        # Legacy layout has an extra 16 source bytes after each group of 20.
        payload=bytearray()
        expected=bytearray()
        for group in range(3):
            for slot in range(group*20,(group+1)*20):
                block=bytes((slot*17+i*13)&255 for i in range(stride))
                payload.extend(block)
                for value in block[:3276]:expected.extend((value&15,value>>4))
            trailer=bytearray([255]*16);trailer[13]=0x45;trailer[15]=group
            for value in trailer:expected.extend((value&15,value>>4))
            if stride==3276:payload.extend(bytes([0xa5]*16))
        outcome=oracle.call(PATCH_ENTRY,[bytes(payload)],version=version)
        assert ['delegated_erase',0] in outcome['events']
        assert len(outcome['frames'][0])==7 and outcome['frames'][-1]==bytes([247])
        assert b''.join(outcome['frames'][1:-1])==expected
        assert all(len(frame)<=192 for frame in outcome['frames'])
        results.append({'case':'user_patch_sender','version':hex(version),'source_stride':stride,
                        'slots':60,'bytes_per_slot_sent':3276,'group_trailers':3,
                        'send_calls':len(outcome['frames']),
                        'wire_command':outcome['frames'][0][6],
                        'outbound_sha256':hashlib.sha256(b''.join(outcome['frames'])).hexdigest()})
    for entry,status in RESPONSE_CALLBACKS.items():
        outcome=oracle.call(entry,[])
        assert oracle.u32(HEAP+0x14c)==status&0xffffffff
        assert outcome['frames']==[] and ['set_event_stub'] in outcome['events']
        results.append({'case':'response_callback','entry':hex(entry),'stored_status':status,
                        'event_signaled_in_fake_adapter':True,'send_calls':0})
    for count in [1,2,63,64,65,128,129]:
        words=[(i*0x4321+0x1234)&65535 for i in range(count)]
        payload=b''.join(word.to_bytes(2,'big') for word in words)
        outcome=oracle.call(POD_SEGMENT_ENTRY,[payload])
        expected=[]
        for offset in range(0,count,64):
            chunk=words[offset:offset+64]
            encoded=b''.join(bytes([word>>10,(word>>4)&63,word&15]) for word in chunk)
            frame=outcome['frames'][len(expected)]
            assert frame[6]==len(chunk) and frame[7:-1]==encoded and frame[-1]==247
            expected.append(frame)
        assert len(expected)==len(outcome['frames'])
        assert sum(event[0]=='port_wait_stub_ms' for event in outcome['events'])==len(expected)
        results.append({'case':'pod_segment_words','words':count,'send_calls':len(expected),
                        'frame_lengths':list(map(len,expected)),
                        'outbound_sha256':hashlib.sha256(b''.join(expected)).hexdigest()})
    outcome=oracle.call(POD_SEGMENT_ENTRY,[bytes([255]*128)])
    assert not outcome['frames'] and not any(event[0]=='port_wait_stub_ms' for event in outcome['events'])
    results.append({'case':'pod_segment_all_ff','words':64,'send_calls':0})
    for length in [1,95,96,97,16384]:
        payload=bytes((i*17+0xa3)&255 for i in range(length))
        outcome=oracle.call(BOOT_ENTRY,[payload])
        chunks=[]
        for offset in range(0,length,96):
            chunks.append(bytes(nibble for value in payload[offset:offset+96] for nibble in (value>>4,value&15)))
        checksum=sum(payload)&255
        expected=[bytes([240,0,1,12,7,0,111,2])]+chunks+[bytes([checksum>>4,checksum&15]),bytes([247])]
        assert outcome['frames']==expected and outcome['eax']==0
        results.append({'case':'boot_sender','synthetic_bytes':length,'send_calls':len(expected),
                        'max_payload_bytes_per_chunk':96,
                        'outbound_sha256':hashlib.sha256(b''.join(expected)).hexdigest()})
    outcome=oracle.call(BOOT_ENTRY,[bytes([0x31]*96)],wait=0x102)
    assert outcome['eax']==(-3312)&0xffffffff
    results.append({'case':'boot_sender_timeout','synthetic_bytes':96,
                    'wait_stub_ms':15000,'native_return':-3312})
    for version,cached in [(0x02ffffff,0x02ffffff),(0x03000000,0x02ffffff),
                           (0x02ffffff,0x03000000),(0x03000000,0x03000000),
                           (0x03100000,0x03100000)]:
        transition=oracle.call(APPLICABILITY_ENTRY,[],version=version,cached_version=cached)['eax']&255
        fallback=oracle.call(FALLBACK_ENTRY,[],version=version,cached_version=cached)['eax']&255
        assert bool(transition)==((version>=0x03000000)!=(cached>=0x03000000))
        assert bool(fallback)==(cached>=0x03000000)
        results.append({'case':'variax_70004_factory_predicates','object_version':hex(version),
                        'cached_version':hex(cached),'transition':bool(transition),
                        'fallback':bool(fallback),'factory_stage_selected':bool(transition or fallback)})
    report={'status':'pass','source_sha256':SHA256,'scope':'Five host updater methods, two predicate helpers and three response callbacks in emulated i386 CPU with synthetic payloads and private fake transport/event/helper adapters. No DLL entry point, native vendor execution, device firmware or hardware.','cases':results,'limits':['OS calls never forwarded; unknown calls/addresses stop','event/status/erase delegate/pump/security-cookie behavior is stubbed','Zero native erase return is not proof of acknowledgment: boot guard and timeout also return zero','Zero-length sender shape is observed only; not approved as a receiver input','No receiver flash, recovery, power-loss or physical behavior is modeled']}
    encoded=(json.dumps(report,indent=2)+'\n').encode();folder=ROOT/'analysis/reports';folder.mkdir(parents=True,exist_ok=True);path=folder/('native-updater-oracle-'+hashlib.sha256(encoded).hexdigest()[:16]+'.json');preserve(path,encoded);print(path)

if __name__=='__main__':main()
