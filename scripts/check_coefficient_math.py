"""Reject coefficient math that bypasses the versioned Rust profile boundary."""
import re
from pathlib import Path

ROOT=Path(__file__).resolve().parents[1]
FUNCTIONS='powf|sin|cos|tan|asin|acos|atan|atan2|sinh|cosh|tanh|exp|exp2|exp_m1|ln|ln_1p|log|log2|log10|sqrt|cbrt'
CALL=re.compile(r'\.(?:'+FUNCTIONS+r')\s*\(|(?:f32|f64|libm)::(?:'+FUNCTIONS+r')\s*\(')


def main():
    paths=[]
    for name in ['body','dsp','pickup','host','models']:
        paths.append(ROOT/'src'/f'{name}.rs');paths.extend(sorted((ROOT/'src'/name).rglob('*.rs')))
    failures=[]
    for path in paths:
        for number,line in enumerate(path.read_text(encoding='utf-8').splitlines(),1):
            if CALL.search(line):failures.append(f'{path.relative_to(ROOT)}:{number}: direct math call bypasses src/math.rs')
    if failures:raise SystemExit('\n'.join(failures))
    print('PASS: coefficient transcendental/square-root calls use the versioned math boundary')


if __name__=='__main__':main()
