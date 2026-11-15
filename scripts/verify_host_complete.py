"""Full preset-file -> Java/native and Rust final-byte comparison, offline."""
import hashlib
import json
import struct
import subprocess
import tempfile
from pathlib import Path
from coefficient_oracle import CoefficientOracle,ROOT,default_settings
from inspect_firmware import preserve
from verify_host_pipeline import cases,FIELD_LAYOUT


def chunk(tag,data):return tag+struct.pack('>I',len(data))+data+(b'\0' if len(data)%2 else b'')


def make_preset(line):
    f=line.split();name=bytes.fromhex(f[0]).decode();p={int(k,16):int(v,16) for k,v in (pair.split(':') for pair in f[4:])}
    groups=[(0x2010000,0,None,[(0x200000,0xff0000,1)]),
        (0x2010001,p[0x1020000],p[0x2020001],[(0x200000,131074,1),(0x200001,131075,1),(0x200003,131076,1)]),
        (0x2010002,p[0x1030000],p[0x2030001],[(0x200000,196610,1),(0x200001,196611,1),(0x200003,196612,1),(0x200002,196613,0),(0x200004,196614,0)]),
        (0x2010003,p[0x1010000],None,[]),
        (0x2010004,0,None,[(0x200000+i,262144+i,int(i not in [1,5])) for i in range(6)]),
        (0x2010005,0,p[0x2050000],[(0x200000+i,327681+i,int(i>=6)) for i in range(19)])]
    body=b'PATC'
    for slot,id,enabled,fields in groups:
        model=b'MODL'+chunk(b'MINF',struct.pack('>III',id,slot,int(enabled is None or enabled!=0)))
        for local,logical,kind in fields:model+=chunk(b'PARM',struct.pack('>III',0x3f000000|local,kind,p[logical]))
        body+=chunk(b'LIST',model)
    encoded=name.encode('utf-16be');assert len(encoded)<=64
    return chunk(b'FORM',b'L6PA'+chunk(b'PINF',struct.pack('>II',1,0x70004)+encoded.ljust(64,b'\0')+struct.pack('>I',0x3100000))+chunk(b'LIST',body))


def run_java(temp,casefile,layout,native=None,presets=None):
    cp=f'{temp}:{ROOT/"analysis/extracted/windows-app"}'
    args=[str(ROOT/'.tools/jdk/bin/java'),'-Djava.awt.headless=true','-cp',cp,'VerifyHostPipeline',str(casefile),str(layout)]
    if native is not None:args.extend([str(native),str(presets)])
    output=subprocess.check_output(args,text=True)
    return [line.split(' => ')[1].split() for line in output.splitlines()]


def main():
    subprocess.run(['cargo','build'],cwd=ROOT,check=True)
    oracle=CoefficientOracle();models=oracle.load_models();initial=oracle.read(oracle.OBJECT,0x1134)
    with tempfile.TemporaryDirectory(prefix='variax-host-complete-') as temp:
        temp=Path(temp);lines=[]
        for line in cases():
            f=line.split();
            if int(f[1])&1:continue
            f[2]='ffffffff';lines.append(' '.join(f))
        for first,second,series,resistance,capacitance in [(0x3110001,0x3110000,1,1000.,.022),(51478529,51478528,0,500.,.047)]:
            for reverse in [False,True]:
                for delta in [0.,.05,.15]:
                    for phase in [0,1]:
                        f=lines[0].split();p={int(k,16):int(v,16) for k,v in (v.split(':') for v in f[4:])}
                        bits=lambda value:struct.unpack('<I',struct.pack('<f',value))[0]
                        p.update({0x1010000:0x3100003,0x2020001:1,0x2030001:1,0x1020000:second if reverse else first,0x1030000:first if reverse else second,196613:phase,196614:series,262144:bits(resistance+delta),262146:bits(resistance),262147:bits(capacitance)})
                        lines.append(' '.join(['Synthetic combo'.encode().hex(),'0','ffffffff','00000000',*[f'{k:08x}:{v:08x}' for k,v in sorted(p.items())]]))
        casefile=temp/'cases.txt';casefile.write_text('\n'.join(lines)+'\n')
        layout=temp/'layout.txt';layout.write_text('\n'.join(f'{name} {offset} {kind}' for name,(offset,kind) in FIELD_LAYOUT.items())+'\n')
        subprocess.run([str(ROOT/'.tools/jdk/bin/javac'),'-cp',str(ROOT/'analysis/extracted/windows-app'),'-d',str(temp),str(ROOT/'scripts/java-host-stubs/cmn/l6java/utils/L6StaticUtils.java'),str(ROOT/'scripts/VerifyHostPipeline.java')],check=True)
        mapped=run_java(temp,casefile,layout);assert len(mapped)==len(lines)==144
        manifest=temp/'models.tsv';entries=[f'{r["kind"]}\t{r["id"]&0xffffffff:08x}\t{ROOT/r["path"]}' for r in models]
        # Keep these original special-table snapshots local, never in fixtures.
        for id,resistance,cap in [(-3,1e6,2.2e-8),(-7,500000.,4.7e-8)]:
            oracle.write(oracle.OBJECT,initial);s=default_settings();struct.pack_into('<I',s,0x14,id&0xffffffff)
            for off,value in [(0x38,resistance),(0x3c,resistance),(0x40,cap)]:struct.pack_into('<f',s,off,value)
            oracle.generate(s);path=temp/f'special-{id}.bin';path.write_bytes(oracle.read(oracle.OBJECT+0xdd0,360));entries.append(f'special\t{id&0xffffffff:08x}\t{path}')
        manifest.write_text('\n'.join(entries)+'\n')
        outputs=[]
        for i,(line,fields) in enumerate(zip(lines,mapped)):
            oracle.write(oracle.OBJECT,initial)
            outputs.append(oracle.generate(bytes.fromhex(fields[0])))
            (temp/f'{i}.vwm').write_bytes(make_preset(line))
        native=temp/'native.bin';native.write_bytes(b''.join(outputs))
        final=run_java(temp,casefile,layout,native,temp);assert len(final)==144
        results=[]
        for i,(line,expected_fields) in enumerate(zip(lines,final)):
            # Original file parser/ParamMap must agree with direct logical-map input.
            assert expected_fields[:2]==mapped[i][:2],('original file mapping differs',i)
            output=temp/f'{i}.packed';command=[str(ROOT/'target/debug/variax-workbench'),'render-preset',str(temp/f'{i}.vwm'),str(manifest),str(output)]
            if int(line.split()[1])&2:command.append('muted')
            result=subprocess.run(command,text=True,capture_output=True)
            assert result.returncode==0,(i,result.stderr)
            actual=output.read_bytes();expected=bytes.fromhex(expected_fields[2])
            if actual!=expected:
                diff=[(n,actual[n],expected[n]) for n in range(3276) if actual[n]!=expected[n]]
                raise AssertionError((i,len(diff),diff[:20]))
            results.append({'case':i,'sha256':hashlib.sha256(actual).hexdigest(),'bytes':len(actual)})
            private=ROOT/'analysis/extracted/host-complete-v1';private.mkdir(parents=True,exist_ok=True)
            preserve(private/f'{i}.vwm',(temp/f'{i}.vwm').read_bytes());preserve(private/f'{i}.packed',actual)
            print('PASS: complete preset',i,hashlib.sha256(actual).hexdigest(),flush=True)
        preserve(ROOT/'analysis/reports/host-complete-v1.json',(json.dumps({'source_sha256':oracle.sha256,'runtime_version':oracle.RUNTIME_VERSION,'cases':len(results),'scope':'Original Java preset reader and ParamMap, mapping/ApplyHacks, original native instructions, final original Java metadata overlay versus Rust render-preset. Fresh engine per preset; external vendor resources stay private. No audio/device equivalence.', 'special_combinations':24,'results':results},indent=2)+'\n').encode())
    print('PASS: 144 complete original Java/native vs Rust final preset byte arrays')


if __name__=='__main__':main()
