"""Preserve the pinned math crate's license and embedded source notices.

Only permissive dependency notices are copied, never Line 6 or Apple artifacts.
Saves are non-overwriting, so a version update requires a reviewed new filename.
"""
import collections
import re
from build_rust_math_reference import verify_cached_source
from inspect_firmware import preserve
from native_oracle import ROOT


def main():
    source,_=verify_cached_source();directory=ROOT/'third_party/notices';directory.mkdir(parents=True,exist_ok=True)
    preserve(directory/'libm-0.2.16-LICENSE.txt',(source/'LICENSE.txt').read_bytes())
    notices=collections.defaultdict(list)
    for path in sorted(source.rglob('*.rs')):
        text=path.read_text()
        comments=re.findall(r'/\*[\s\S]*?\*/|(?:^[ \t]*//[^\n]*(?:\n|$))+',text,re.M)
        for block in comments:
            if re.search(r'copyright|permission|redistribution|SPDX-License',block,re.I):notices[block.strip()].append(str(path.relative_to(source)))
    data='Source notices preserved from the checksum-verified libm 0.2.16 crate.\nUpstream: https://github.com/rust-lang/compiler-builtins/tree/main/libm\n\n'
    for notice,files in notices.items():data+='Files: '+', '.join(files)+'\n\n'+notice+'\n\n'
    preserve(directory/'libm-0.2.16-SOURCE-NOTICES.txt',data.encode());print('Preserved license and',len(notices),'distinct embedded source notices')


if __name__=='__main__':main()
