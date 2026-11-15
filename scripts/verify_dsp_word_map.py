"""Observe word ownership during the initialized original host generator sweep."""
import bisect
import collections
import hashlib
import json
from unicorn import UC_HOOK_MEM_WRITE
from unicorn.x86_const import UC_X86_REG_EIP
from coefficient_oracle import CoefficientOracle,ROOT
from test_coefficient_generator import cases
from inspect_firmware import preserve


def main():
    oracle=CoefficientOracle();models=oracle.load_models();owners=[collections.Counter() for _ in range(1091)];active=False
    ranges=[]
    for f in oracle.index['functions']:
        if f['status']!='written':continue
        for r in f.get('body',{}).get('ranges',[]):ranges.append((int(r['start'],16),int(r['end'],16),f['name']))
    ranges.sort();starts=[r[0] for r in ranges];cache={}
    def owner(pc):
        if pc not in cache:
            i=bisect.bisect_right(starts,pc)-1
            cache[pc]=ranges[i][2] if i>=0 and pc<=ranges[i][1] else f'unresolved pc {pc:#x}'
        return cache[pc]
    def wrote(cpu,access,address,size,value,context):
        if not active:return
        name=owner(cpu.reg_read(UC_X86_REG_EIP))
        for i in range(max(0,(address-oracle.OUTPUT)//4),min(1090,(address+size-1-oracle.OUTPUT)//4)+1):owners[i][name]+=1
    oracle.cpu.hook_add(UC_HOOK_MEM_WRITE,wrote,begin=oracle.OUTPUT,end=oracle.OUTPUT+1091*4-1)
    records=[]
    for name,settings in cases(models):
        oracle.write(oracle.OUTPUT+0x2000,bytes(settings));oracle.call('0x180a0',oracle.OUTPUT)
        active=True
        try:oracle.call('0x2c40',oracle.OBJECT,oracle.OUTPUT,oracle.OUTPUT+0x2000,instruction_limit=200_000_000,timeout_us=10_000_000)
        finally:active=False
        oracle.call('0x1bd10',oracle.OUTPUT,oracle.OUTPUT+0x4000)
        output=oracle.read(oracle.OUTPUT+0x4000,3276)
        stem=hashlib.sha256(name.encode()+b'\0'+bytes(settings)).hexdigest()[:16]
        expected=(ROOT/'analysis/extracted/coefficient-reference'/oracle.sha256/oracle.RUNTIME_VERSION/f'{stem}.packed.bin').read_bytes()
        assert output==expected,name
        records.append(name);print('PASS:',len(records),name,flush=True)
    report={'source_sha256':oracle.sha256,'runtime_version':oracle.RUNTIME_VERSION,'cases':len(records),
            'scope':'Observed native instruction writes during initialized whole-generator calls, excluding constructor zeroing and checksum. Emulation API writes from adapters may not trigger CPU hooks; unobserved is not proof of unused/reserved hardware words.',
            'observed_words':sum(bool(v) for v in owners),'unobserved_words':sum(not bool(v) for v in owners),
            'words':[{'index':i,'byte_offset':hex(i*4),'writers':dict(v),'state':'observed instruction write' if v else 'no CPU instruction write observed; semantics unknown'} for i,v in enumerate(owners)]}
    preserve(ROOT/f'analysis/reports/dsp-word-map-{oracle.RUNTIME_VERSION}.json',(json.dumps(report,indent=2)+'\n').encode())
    print('PASS:',report['cases'],'unchanged reference outputs;',report['observed_words'],'words with instruction writes;',report['unobserved_words'],'unobserved')


if __name__=='__main__':main()
