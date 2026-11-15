"""Cross-check manual native input layout against original JNI field mapping.

JNI is a private memory adapter, not a JVM or device interface. It supplies named
scalar fields and a private byte-array buffer. Original JNI extraction, native
object construction, coefficient generation and packing instructions execute.
Java ApplyHacks and final metadata overlay are not covered by this harness.
"""
import hashlib
import json
import struct
from unicorn import UC_HOOK_CODE
from unicorn.x86_const import UC_X86_REG_ESP, UC_X86_REG_EAX
from coefficient_oracle import CoefficientOracle, default_settings, ROOT

FIELD_LAYOUT = {
    'm_is_acoustic':(0,'Z'), 'm_use_existing_12str':(1,'Z'),
    'm_body_modelID':(4,'I'), 'm_pickup1_enabled':(8,'Z'), 'm_pickup2_enabled':(9,'Z'),
    'm_pickup1_modelID':(0xc,'I'), 'm_pickup2_modelID':(0x10,'I'), 'm_alt_pickup_sweepID':(0x14,'I'),
    'm_pickup1_position':(0x18,'F'), 'm_pickup2_position':(0x1c,'F'),
    'm_pickup1_angle':(0x20,'F'), 'm_pickup2_angle':(0x24,'F'),
    'm_pickup1_level':(0x28,'F'), 'm_pickup2_level':(0x2c,'F'),
    'm_pickup1_phasereverse':(0x30,'Z'), 'm_pickup2_phasereverse':(0x31,'Z'),
    'm_series_pickups':(0x32,'Z'), 'm_vol_knob_taper':(0x34,'I'),
    'm_vol_knob_resistance':(0x38,'F'), 'm_tone_knob_resistance':(0x3c,'F'),
    'm_tone_knob_capacitance':(0x40,'F'), 'm_load_r':(0x44,'F'), 'm_load_c':(0x48,'F'),
    'm_alt_tune_enable':(0x4c,'Z'), 'm_tone_knob_position':(0x98,'F'),
    'm_save_tone_knob':(0x9c,'Z'), 'm_tone_knob_function':(0xa0,'I'),
    'm_force_sitar':(0xa4,'Z'), 'm_trim':(0xbc,'F'),
}
for stem,start,count in [('tune',0x50,6),('mix',0x68,6),('delay',0x80,6)]:
    for i in range(count): FIELD_LAYOUT[f'm_string{i+1}_{stem}']=(start+i*4,'F')
for i in range(5): FIELD_LAYOUT[f'm_drone{i+1}_tune']=(0xa8+i*4,'F')


def via_jni(oracle, settings):
    fields={}
    for name,(offset,kind) in FIELD_LAYOUT.items():
        fields[name]=(kind, settings[offset] if kind=='Z' else struct.unpack_from('<I',settings,offset)[0])
    fields['m_block_mask']=('I',0xffffffff)
    handles={}
    # Copy an initialized engine object into the library's original JNI global.
    # Internal model/table references still point only to the private heap.
    oracle.write(0x3c660,oracle.read(oracle.OBJECT,0x1134))
    oracle.write(0x3c560,bytes(0xd4))  # original cached field handles
    # ghasFieldIDs is observed separately in the Mach-O symbol table below.
    import subprocess
    output=subprocess.check_output(['llvm-nm',str(ROOT/'analysis/extracted/macho-slices/L6VariaxCoefEngine.jnilib.i386.macho')],text=True)
    lines=[line.split() for line in output.splitlines() if line.endswith(' _ghasFieldIDs')]
    assert len(lines)==1
    oracle.write(int(lines[0][0],16),b'\0')
    table=oracle.allocate(0x400)
    env=oracle.allocate(4)
    oracle.write(env,struct.pack('<I',table))
    array=oracle.allocate(3276)
    operations={0x7c:'class',0x178:'field',0x180:'bool',0x190:'int',0x198:'float',
                0x2ac:'length',0x2e0:'elements',0x300:'release'}
    hooks=[]
    for i,(offset,operation) in enumerate(operations.items()):
        shim=oracle.SHIMS+0xe000+i*32
        oracle.write(table+offset,struct.pack('<I',shim))
        oracle.write(shim,b'\xdd\x05'+struct.pack('<I',shim+16)+b'\xc3' if operation=='float' else b'\xc3')
        def handler(cpu,address,size,context,operation=operation):
            stack=cpu.reg_read(UC_X86_REG_ESP)
            args=struct.unpack('<5I',oracle.read(stack+4,20))
            assert args[0]==env
            result=0
            if operation=='class': result=0x1234
            elif operation=='field':
                name=oracle.cstring(args[2]).decode('ascii'); kind=oracle.cstring(args[3]).decode('ascii')
                assert name in fields and fields[name][0]==kind,(name,kind)
                result=len(handles)+1; handles[result]=name
            elif operation in ('bool','int','float'):
                kind,bits=fields[handles[args[2]]]
                assert kind=={'bool':'Z','int':'I','float':'F'}[operation]
                if operation=='float': oracle.write(address+16,struct.pack('<d',struct.unpack('<f',struct.pack('<I',bits))[0]))
                else: result=bits
            elif operation=='length': result=3276
            elif operation=='elements': result=array
            elif operation=='release': assert args[2]==array and args[3]==0
            cpu.reg_write(UC_X86_REG_EAX,result)
        hooks.append(oracle.cpu.hook_add(UC_HOOK_CODE,handler,begin=shim,end=shim))
    try:
        oracle.call('0x4790',env,0,1,2,instruction_limit=20_000_000,timeout_us=10_000_000)
        assert len(handles)==len(fields)
        return oracle.read(array,3276)
    finally:
        for hook in hooks: oracle.cpu.hook_del(hook)


def main():
    records=[]
    oracle=CoefficientOracle();oracle.load_models()
    for name in ['baseline','saved-tone','series-phase','alternate-tuning']:
        settings=default_settings()
        if name=='saved-tone': settings[0x9c]=1;struct.pack_into('<f',settings,0x98,0.25)
        if name=='series-phase': settings[9]=1;settings[0x32]=1;settings[0x31]=1
        if name=='alternate-tuning':
            settings[0x4c]=1
            for i in range(6):
                struct.pack_into('<f',settings,0x50+i*4,(-2.0 if i%2 else -12.0))
                struct.pack_into('<f',settings,0x68+i*4,1.0)
        direct=oracle.generate(settings)
        actual=via_jni(oracle,settings)
        assert direct==actual,f'original JNI and manual input path differ: {name}'
        records.append({'case':name,'status':'pass','output_sha256':hashlib.sha256(actual).hexdigest()})
        print(f'PASS: original JNI mapping matches direct generation: {name}',flush=True)
    (ROOT/'analysis/reports/coefficient-jni-agreement.json').write_text(json.dumps({'source_sha256':oracle.sha256,'fields_checked':len(FIELD_LAYOUT)+1,
        'scope':'Original JNI/native instructions with private named-field/byte-array adapters and host libm; no JVM, Java ApplyHacks, final metadata overlay or device',
        'records':records},indent=2)+'\n')

if __name__=='__main__': main()
