"""Original Java host mapping/overlay with no native or device services."""
import hashlib
import json
import random
import struct
import subprocess
import tempfile
from pathlib import Path
from verify_coefficient_jni import FIELD_LAYOUT
from native_oracle import ROOT
from inspect_firmware import preserve


def cases():
    rng = random.Random(600175)
    def bits(f): return struct.unpack('<I',struct.pack('<f',f))[0]
    pairs=[(0x3110001,0x3110000),(0x3110003,51445762),(51445764,0x3110003),
           (51478537,51478536),(51478529,51478528),(51445766,51445765),
           (51445771,51445770),(51445775,51445774),(51478531,51478530),
           (51478533,51478532),(0x3110012,0x3110013),(0,0xffffffff)]
    for n in range(240):
        body=[0x3100003,0x310000d,0x3100012,0x3100013,0x3100015,0x3100018,0x310001b,0xffffffff][n%8]
        a,b=pairs[(n//8)%len(pairs)]
        if n%2: a,b=b,a
        p={0x1010000:body,0x1020000:a,0x1030000:b,
           0x2020001:int(n%5!=0),0x2030001:int(n%7!=0),
           131074:bits(rng.choice([.1,2.,6.,9.5])),196610:bits(rng.choice([.1,2.,6.,9.5])),
           131075:bits(rng.choice([-45.,-15.,0.,15.,45.])),196611:bits(rng.choice([-45.,0.,45.])),
           131076:bits(rng.choice([-18.,0.,6.])),196612:bits(rng.choice([-18.,0.,6.])),
           196613:n%2,196614:(n//2)%2,262144:bits(rng.choice([25.,250.,500.,1000.])),
           262145:n%3,262146:bits(rng.choice([25.,250.,500.,1000.])),262147:bits(rng.choice([.001,.022,.047])),
           262148:bits(rng.choice([0.,.5,1.])),262149:n%2,0xff0000:bits(rng.choice([-20.,0.,6.])),0x2050000:(n//4)%2}
        for i in range(6):
            p[327681+i]=rng.choice([-12,-2,0,7,12])&0xffffffff
            p[327687+i]=bits(rng.choice([-50.,0.,25.,50.]))
            p[327693+i]=bits(rng.choice([0.,.25,.5,1.]))
        p[327699]=bits(rng.choice([0.,.5,1.]))
        # Explicit combinations independently of the rotating body matrix.
        if n>=160:
            p[0x1010000]=0x3100003;p[0x2020001]=p[0x2030001]=1
            p[196614]=n%2
        name=['Synthetic','Synthetic longer than eighteen','Guitár 🎸','日本語','A\x00B'][n%5]
        yield ' '.join([name.encode().hex(),str(n%4),f'{rng.getrandbits(32):08x}',f'{rng.getrandbits(32):08x}',*[f'{k:08x}:{v:08x}' for k,v in sorted(p.items())]])


def main():
    original=ROOT/'analysis/extracted/windows-app'
    with tempfile.TemporaryDirectory(prefix='variax-host-java-') as temp:
        temp=Path(temp);casefile=temp/'cases.txt';casefile.write_text('\n'.join(cases())+'\n')
        layout=temp/'layout.txt';layout.write_text('\n'.join(f'{name} {offset} {kind}' for name,(offset,kind) in FIELD_LAYOUT.items())+'\n')
        javac=ROOT/'.tools/jdk/bin/javac';java=ROOT/'.tools/jdk/bin/java'
        subprocess.run([str(javac),'-cp',str(original),'-d',str(temp),str(ROOT/'scripts/java-host-stubs/cmn/l6java/utils/L6StaticUtils.java'),str(ROOT/'scripts/VerifyHostPipeline.java')],check=True)
        output=subprocess.check_output([str(java),'-Djava.awt.headless=true','-cp',f'{temp}:{original}','VerifyHostPipeline',str(casefile),str(layout)],text=True)
        lines=output.splitlines();assert len(lines)==240,len(lines)
        assert all(' => ' in line for line in lines)
        preserve(ROOT/'tests/fixtures/original-java-host-pipeline.txt',output.encode())
        hashes={str(p.relative_to(original)):hashlib.sha256(p.read_bytes()).hexdigest() for p in [original/'modules/variax/device/L6VariaxMIDITranscoder.class',original/'modules/variax/device/coefeng/L6VariaxCoefEngine.class']}
        preserve(ROOT/'analysis/reports/java-host-pipeline-v1.json',(json.dumps({'cases':len(lines),'original_class_sha256':hashes,'scope':'Original Java GeneratePatch, ApplyHacks and metadata/checksum overlay; fake coefficient engine returns seeded synthetic bytes; no native generation or device construction'},indent=2)+'\n').encode())
    check=subprocess.run(['cargo','test','--lib','host::'],cwd=ROOT,check=True,text=True,capture_output=True)
    assert 'original_java_mapping_hacks_metadata_and_checksum_match_240_cases ... ok' in check.stdout,check.stdout
    print(check.stdout,end='')
    print('PASS: 240 original Java host mapping/overlay cases')


if __name__=='__main__':main()
