"""Complete Rust/native host numeric-generation comparison; offline files only."""
import json
import struct
import subprocess
import tempfile
from pathlib import Path
from coefficient_oracle import ROOT,CoefficientOracle
from test_coefficient_generator import cases


def main():
    subprocess.run(['cargo','build'],cwd=ROOT,check=True)
    oracle=CoefficientOracle();models=oracle.load_models();count=0
    with tempfile.TemporaryDirectory(prefix='variax-host-engine-') as temp:
        temp=Path(temp);manifest=temp/'models.tsv'
        manifest.write_text('\n'.join(f'{r["kind"]}\t{r["id"]&0xffffffff:08x}\t{ROOT/r["path"]}' for r in models)+'\n')
        inputs=[];outputs=[];names=[]
        for name,settings in cases(models):
            expected=oracle.generate(settings)
            inputs.append(bytes(settings));outputs.append(expected);names.append(name)
        inp=temp/'input.bin';out=temp/'output.bin';inp.write_bytes(b''.join(inputs))
        result=subprocess.run([str(ROOT/'target/debug/variax-workbench'),'host-generate-batch',str(inp),str(manifest),str(out)],text=True,capture_output=True)
        assert result.returncode==0,result.stderr
        all_outputs=out.read_bytes();assert len(all_outputs)==len(outputs)*3276
        for count,(name,expected) in enumerate(zip(names,outputs),1):
            actual=all_outputs[(count-1)*3276:count*3276]
            if actual!=expected:
                words=lambda b:[int.from_bytes(b[i:i+3],'big') for i in range(0,len(b),3)]
                diff=[(i,f'{a:06x}',f'{e:06x}') for i,(a,e) in enumerate(zip(words(actual),words(expected))) if a!=e]
                raise AssertionError((name,len(diff),diff[:25]))
            print('PASS:',name,flush=True)
    print('PASS:',count,'complete Rust/native host-engine cases')


if __name__=='__main__':main()
