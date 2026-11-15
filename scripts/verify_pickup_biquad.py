"""Compare Rust filter transform/float32 response with original instructions."""
import json
import struct
import subprocess
from coefficient_oracle import CoefficientOracle, ROOT
from inspect_firmware import preserve


def words(value): return struct.unpack('<2I', struct.pack('<d', value))
def bits(value): return f'{struct.unpack("<Q", struct.pack("<d", value))[0]:016x}'


def main():
    subprocess.run(['cargo', 'build'], cwd=ROOT, check=True)
    oracle = CoefficientOracle()
    vectors = []
    count = {'bandpass': 0, 'response': 0}
    for rate in (31250., 44100., 48000.):
        oracle.write(oracle.OBJECT + 8, struct.pack('<f', rate))
        ratebits = f'{struct.unpack("<I", struct.pack("<f", rate))[0]:08x}'
        for frequency in (40., 440., 1500., 5000., 10000.):
            for q in (0.1, 0.707, 2., 10.):
                for gain in (0.125, 1., 3.):
                    oracle.call('0x24960', oracle.OBJECT, oracle.OUTPUT, *words(frequency), *words(q), *words(gain))
                    coefficients = struct.unpack('<5d', oracle.read(oracle.OUTPUT, 40))
                    fields = [ratebits, bits(frequency), bits(q), bits(gain)]
                    expected = ' '.join(bits(v) for v in coefficients)
                    actual = subprocess.check_output([str(ROOT/'target/debug/variax-workbench'), 'pickup-bandpass', *fields], text=True).strip()
                    assert actual == expected, ('bandpass', fields, actual, expected)
                    vectors.append('bandpass '+' '.join(fields)+' '+expected)
                    count['bandpass'] += 1
                    oracle.write(oracle.OBJECT + 0x1000, struct.pack('<5d', *coefficients))
                    for f in (20., frequency, 1000., 12000.):
                        oracle.call('0x23d30', oracle.OUTPUT, oracle.OBJECT, oracle.OBJECT + 0x1000, *words(f))
                        response = struct.unpack('<2d', oracle.read(oracle.OUTPUT, 16))
                        fields = [ratebits, bits(f), *map(bits, coefficients)]
                        expected = ' '.join(bits(v) for v in response)
                        actual = subprocess.check_output([str(ROOT/'target/debug/variax-workbench'), 'pickup-biquad-response', *fields], text=True).strip()
                        assert actual == expected, ('response', fields, actual, expected)
                        vectors.append('response '+' '.join(fields)+' '+expected)
                        count['response'] += 1
    preserve(ROOT/'tests/fixtures/native-pickup-biquad.txt', ('\n'.join(vectors)+'\n').encode())
    preserve(ROOT/f'analysis/reports/pickup-biquad-{oracle.RUNTIME_VERSION}.json', (json.dumps({'source_sha256': oracle.sha256, 'runtime_version': oracle.RUNTIME_VERSION, 'counts':count, 'scope':'Bandpass coefficients and original float32 response, synthetic inputs only; not device/audio validation'}, indent=2)+'\n').encode())
    print('PASS:', count)


if __name__ == '__main__':
    main()
