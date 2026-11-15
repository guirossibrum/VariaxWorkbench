"""Validate explicit Rust edits using domains and readers from the original JAR.

Uses synthetic presets only. No editor, native library or device is opened.
Original artifacts stay ignored/local; reports are non-overwriting.
"""
import hashlib
import json
from pathlib import Path
import struct
import subprocess
import tempfile

ROOT = Path(__file__).resolve().parents[1]


def run(command):
    return subprocess.check_output([str(x) for x in command], text=True)


def main():
    jar = ROOT / 'analysis/extracted/mac-app/VariaxWorkbench.app/Contents/Resources/Java/VariaxWorkbench.jar'
    java = [ROOT / '.tools/jdk/bin/java', '-Djava.awt.headless=true', '-cp', jar,
            ROOT / 'scripts/InspectEditDomains.java']
    subprocess.run(['cargo', 'build', '--locked'], cwd=ROOT, check=True)
    rust = [ROOT / 'target/debug/variax-workbench']
    original = (ROOT / 'tests/fixtures/synthetic-host-preset.vwm').read_bytes()
    # Select a twelve-string acoustic body for mix/spread, and enable saved tone.
    # Use separate electric/twelve-string seeds, preserving unrelated fields.
    def seed(body):
        data = original.replace(struct.pack('>II', 0x03100003, 0x02010003), struct.pack('>II', body, 0x02010003), 1)
        return data.replace(struct.pack('>III', 0x3f200005, 0, 0), struct.pack('>III', 0x3f200005, 0, 1), 1)

    with tempfile.TemporaryDirectory(prefix='variax-domains-') as directory:
        temporary = Path(directory)
        electric = temporary / 'electric.seed'
        twelve = temporary / 'twelve.seed'
        electric.write_bytes(seed(0x03100003))
        twelve.write_bytes(seed(0x0310000d))
        domains = run([*java, electric]).splitlines()
        assert len(domains) == 34
        cases = temporary / 'cases'
        cases.mkdir()
        count = 0
        for line in domains:
            slot, parameter, kind, *values = line.split('\t')
            for bits in dict.fromkeys(values):
                number = int(bits, 16)
                value = (str(struct.unpack('>f', struct.pack('>I', number))[0]) if kind == '1'
                         else str(struct.unpack('>i', struct.pack('>I', number))[0]))
                source = twelve if slot == '02010005' else electric
                output = cases / f'{slot}-{parameter}-{kind}-{bits}.vwm'
                run([*rust, 'set', source, output, slot, parameter, 'float' if kind == '1' else 'int', value])
                count += 1
        result = run([*java, 'verify', cases]).strip()
        for path in sorted(cases.glob('*.vwm')):
            assert run([*rust, 'inspect', path]) == run([*rust, 'inspect', str(path)+'.resaved'])
        report = {'jar_sha256': hashlib.sha256(jar.read_bytes()).hexdigest(),
                  'synthetic_preset_sha256': hashlib.sha256(original).hexdigest(),
                  'binary_sha256': hashlib.sha256(Path(rust[0]).read_bytes()).hexdigest(),
                  'helper_sha256': hashlib.sha256(Path(java[-1]).read_bytes()).hexdigest(),
                  'driver_sha256': hashlib.sha256(Path(__file__).read_bytes()).hexdigest(),
                  'parameters': len(domains), 'boundary_and_choice_cases': count,
                  'original_reader': result, 'java_reserialization_reread': 'pass',
                  'scope': 'UI domains/file compatibility; no coefficient/audio/hardware safety claim'}
    reports = ROOT / 'analysis/reports'
    reports.mkdir(parents=True, exist_ok=True)
    path = reports / ('edit-domains-' + hashlib.sha256(json.dumps(report,sort_keys=True).encode()).hexdigest()[:16] + '.json')
    try:
        with path.open('x') as output:
            output.write(json.dumps(report, indent=2)+'\n')
    except FileExistsError:
        assert json.loads(path.read_text()) == report
    print(json.dumps(report,indent=2))
    print(path)


if __name__ == '__main__':
    main()
