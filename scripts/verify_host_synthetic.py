"""Original full host numeric outputs from exclusively synthetic resources."""
import hashlib
import struct
import subprocess
import tempfile
from pathlib import Path
from coefficient_oracle import CoefficientOracle,ROOT,default_settings
from inspect_firmware import preserve
from verify_host_complete import make_preset,run_java
from verify_host_pipeline import cases,FIELD_LAYOUT

BODY_IDS=[0x3100003,0x310000d,0x3100012,0x3100013,0x3100015,0x3100018,0x310001b,0x3100011]


def main():
    subprocess.run(['cargo','build'],cwd=ROOT,check=True)
    bodies=(ROOT/'tests/fixtures/native-dsp-body-inputs.bin').read_bytes();pickups=[]
    for i in range(4):
        pickups.append(struct.pack('>4dBf12f',6200.+i*1300,95000.+i*12000,2.1+i,110e-12+i*30e-12,int(i%2==0),-3.+i,*[n*.03125 for n in range(6)],*([.1875]*6)))
    preserve(ROOT/'tests/fixtures/native-host-pickups.bin',b''.join(pickups))
    with tempfile.TemporaryDirectory(prefix='variax-synthetic-host-') as temp:
        temp=Path(temp);records=[]
        for i,id in enumerate(BODY_IDS):
            path=temp/f'body-{i}.bin';payload=bodies[i*17483:(i+1)*17483];path.write_bytes(payload)
            records.append({'kind':'body','id':id,'path':str(path),'sha256':hashlib.sha256(payload).hexdigest()})
        for i,id in enumerate([0x3110001,0x3110000,-2,-3]):
            path=temp/f'pickup-{i}.bin';path.write_bytes(pickups[i]);records.append({'kind':'pickup','id':id,'path':str(path),'sha256':hashlib.sha256(pickups[i]).hexdigest()})
        manifest=temp/'models.tsv';manifest.write_text('\n'.join(f'{r["kind"]}\t{r["id"]&0xffffffff:08x}\t{r["path"]}' for r in records)+'\n')
        oracle=CoefficientOracle();oracle.load_models(records);inputs=[];outputs=[];native_trim=[]
        for n in range(48):
            body=BODY_IDS[n//6];s=default_settings();struct.pack_into('<I',s,4,body)
            s[0]=int(body in [0x3100012,0x3100013,0x3100015,0x310001b]);s[1]=int(n%3==0);s[9]=int(n%4!=0);s[8]=int(n%5!=0)
            s[0x4c]=int(n%2==0);s[0xa4]=int(body==0x3100018);s[0x31]=n%2;s[0x32]=int(n%3==0)
            struct.pack_into('<I',s,0xa0,3 if s[0] else (2 if s[0xa4] else 1));struct.pack_into('<I',s,0x34,n%2)
            struct.pack_into('<I',s,0x14,(-3 if s[0x32] else -2)&0xffffffff if s[8] and s[9] else 0xffffffff)
            for offset,value in {0x18:[1.,2.,3.,5.,7.,9.][n%6],0x1c:[9.,7.,5.,3.,2.,1.][n%6],0x20:(n%3-1)*.5,0x24:((n+1)%3-1)*.5,0x28:(n%3-1)*6.,0x2c:((n+1)%3-1)*6.,0x98:[0.,.5,1.][n%3],0xbc:[-20.,0.,6.][n%3]}.items():struct.pack_into('<f',s,offset,value)
            s[0x9c]=n%2
            for i in range(6):
                struct.pack_into('<f',s,0x50+i*4,[-12.,-2.,0.,7.][(n+i)%4]);struct.pack_into('<f',s,0x68+i*4,[0.,.5,1.][(n+i)%3]);struct.pack_into('<f',s,0x80+i*4,-1. if n%3==0 else (i+1)*.25)
            inputs.append(bytes(s));outputs.append(oracle.generate(s));native_trim.append(oracle.read(oracle.OBJECT+0xbbc+0x38,4).hex())
        inp=temp/'input.bin';out=temp/'output.bin';inp.write_bytes(b''.join(inputs))
        p=subprocess.run([str(ROOT/'target/debug/variax-workbench'),'host-generate-batch',str(inp),str(manifest),str(out)],text=True,capture_output=True)
        assert p.returncode==0,p.stderr
        actual=out.read_bytes();expected=b''.join(outputs)
        if actual!=expected:
            diffs=[(i//3276,(i%3276)//3,actual[i:i+3].hex(),expected[i:i+3].hex()) for i in range(0,len(expected),3) if actual[i:i+3]!=expected[i:i+3]]
            print('native trim:',native_trim,flush=True);print(p.stderr,flush=True)
            raise AssertionError((len(diffs),diffs[:30]))
        preserve(ROOT/'tests/fixtures/native-host-inputs.bin',b''.join(inputs));preserve(ROOT/'tests/fixtures/native-host-outputs.bin',expected)
        # A complete synthetic file-to-final-byte fixture for ordinary CLI tests.
        f=list(cases())[192].split();f[0]='Synthetic host'.encode().hex();f[1]='0';f[2]='ffffffff';line=' '.join(f)
        casefile=temp/'cases.txt';casefile.write_text(line+'\n');layout=temp/'layout.txt';layout.write_text('\n'.join(f'{name} {offset} {kind}' for name,(offset,kind) in FIELD_LAYOUT.items())+'\n')
        subprocess.run([str(ROOT/'.tools/jdk/bin/javac'),'-cp',str(ROOT/'analysis/extracted/windows-app'),'-d',str(temp),str(ROOT/'scripts/java-host-stubs/cmn/l6java/utils/L6StaticUtils.java'),str(ROOT/'scripts/VerifyHostPipeline.java')],check=True)
        mapped=run_java(temp,casefile,layout);assert len(mapped)==1
        fresh=CoefficientOracle();fresh.load_models(records);packed=fresh.generate(bytes.fromhex(mapped[0][0]))
        native=temp/'native.bin';native.write_bytes(packed);preset=make_preset(line);(temp/'0.vwm').write_bytes(preset)
        final=run_java(temp,casefile,layout,native,temp);expected=bytes.fromhex(final[0][2])
        output=temp/'final.bin';subprocess.run([str(ROOT/'target/debug/variax-workbench'),'render-preset',str(temp/'0.vwm'),str(manifest),str(output)],check=True)
        assert output.read_bytes()==expected
        preserve(ROOT/'tests/fixtures/synthetic-host-preset.vwm',preset);preserve(ROOT/'tests/fixtures/synthetic-host-preset.packed',expected)

    print('PASS: 48 complete synthetic host outputs with original constructors and original generation')


if __name__=='__main__':main()
