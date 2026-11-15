"""Original-Java multi-bank/collection and resized replacement compatibility.

Synthetic presets only, file readers/writers only, no native/device API.
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


def chunk(tag, data):
    return tag + struct.pack('>I', len(data)) + data + (b'\xa7' if len(data) % 2 else b'')


def augment(data, odd=False):
    """Add ignored chunks at root/bank/preset levels; preserve existing bytes."""
    tag = data[:4]
    length, = struct.unpack('>I', data[4:8])
    payload = data[8:8+length]
    if tag not in (b'FORM', b'LIST'):
        return data
    out = payload[:4]
    offset = 4
    while offset < len(payload):
        size, = struct.unpack('>I', payload[offset+4:offset+8])
        end = offset + 8 + size + size % 2
        out += augment(payload[offset:end], odd)
        offset = end
    if payload[:4] in (b'L6BA', b'L6CO', b'BANK', b'FLDR', b'L6PA'):
        out += chunk(b'XTRA', b'unknown-' + payload[:4] + (b'!' if odd else b'!!'))
    return chunk(tag, out)


def main():
    subprocess.run(['cargo', 'build', '--locked'], cwd=ROOT, check=True)
    jar = ROOT / 'analysis/extracted/mac-app/VariaxWorkbench.app/Contents/Resources/Java/VariaxWorkbench.jar'
    java = [ROOT / '.tools/jdk/bin/java', '-Djava.awt.headless=true', '-cp', jar]
    helper = ROOT / 'scripts/InspectBundle.java'
    preset_helper = ROOT / 'scripts/InspectPreset.java'
    rust = [ROOT / 'target/debug/variax-workbench']
    source = ROOT / 'tests/fixtures/synthetic-host-preset.vwm'
    checks = []
    with tempfile.TemporaryDirectory(prefix='variax-bank-matrix-') as directory:
        temporary = Path(directory)
        replacement = temporary / 'replacement.vwm'
        run([*rust, 'rename', source, replacement, 'Resized 🎸'])
        data = replacement.read_bytes() + chunk(b'XTRA', b'new preset extension!!')
        data = data[:4] + struct.pack('>I', len(data)-8) + data[8:]
        replacement.write_bytes(data)
        for fixed in ('true', 'false'):
            original = temporary / f'{fixed}.original'
            expected = run([*java, helper, 'create-matrix', source, original, fixed])
            enriched = temporary / f'{fixed}.enriched'
            enriched.write_bytes(augment(original.read_bytes()))
            odd = temporary / f'{fixed}.odd-extension'
            odd.write_bytes(augment(original.read_bytes(), odd=True))
            odd_result = subprocess.run([str(x) for x in [*java, helper, 'inspect', odd]], capture_output=True, text=True)
            assert odd_result.returncode != 0 and 'EOFException' in odd_result.stderr
            run([*rust, 'inspect-bundle', odd])
            before = enriched.read_bytes()
            assert run([*rust, 'inspect-bundle', enriched]) == expected
            assert run([*java, helper, 'inspect', enriched]) == expected
            output = temporary / f'{fixed}.edited'
            run([*rust, 'replace-preset', enriched, replacement, output, 2, 1])
            assert enriched.read_bytes() == before
            edited = run([*rust, 'inspect-bundle', output])
            assert edited == expected.replace('Preset 2:1', 'Resized 🎸')
            assert run([*java, helper, 'inspect', output]) == edited
            # Byte preservation: untouched banks/presets/extensions remain;
            # original Java may discard unknown chunks only in its own resave.
            assert output.read_bytes().count(b'unknown-') == before.count(b'unknown-')-1
            for bank, count in ((1,2),(2,3)):
                for preset in range(count):
                    extracted = temporary / f'{fixed}-{bank}-{preset}.vwm'
                    run([*rust, 'extract-preset', output, extracted, bank, preset])
                    summary = run([*rust, 'inspect', extracted])
                    assert summary == run([*java, preset_helper, extracted])
                    if (bank,preset) == (2,1):
                        assert extracted.read_bytes() == replacement.read_bytes()
            saved = temporary / f'{fixed}.java-resaved'
            assert run([*java, helper, 'resave', output, saved]) == edited
            assert run([*rust, 'inspect-bundle', saved]) == edited
            checks.append({'format': 'L6BA' if fixed == 'true' else 'L6CO',
                           'banks': 3, 'presets': 5, 'empty_bank': True,
                           'unknown_chunks': 'retained except replaced preset',
                           'odd_extension': 'Rust accepts/preserves; original Java rejects with EOFException',
                           'resized_replacement_and_all_original_readers': 'pass',
                           'java_reserialization': 'pass'})
    digest = lambda p: hashlib.sha256(p.read_bytes()).hexdigest()
    report = {'checks': checks, 'jar_sha256': digest(jar), 'source_sha256': digest(source),
              'binary_sha256': digest(Path(rust[0])), 'helper_sha256': digest(helper),
              'driver_sha256': digest(Path(__file__)),
              'scope': 'offline container/file compatibility; no hardware or audio claim'}
    path = ROOT / 'analysis/reports' / ('bundle-matrix-' + hashlib.sha256(json.dumps(report,sort_keys=True).encode()).hexdigest()[:16] + '.json')
    path.parent.mkdir(parents=True, exist_ok=True)
    try:
        with path.open('x') as output:
            output.write(json.dumps(report,indent=2)+'\n')
    except FileExistsError:
        assert json.loads(path.read_text()) == report
    print(json.dumps(report,indent=2))
    print(path)


if __name__ == '__main__':
    main()
