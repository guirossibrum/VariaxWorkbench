"""Original Java creates offline bundles; Rust inspects and extracts their presets."""
import json
from pathlib import Path
import subprocess
import tempfile

ROOT = Path(__file__).resolve().parents[1]
JAVA = [str(ROOT / '.tools/jdk/bin/java'), '-Djava.awt.headless=true', '-cp',
        str(ROOT / 'analysis/extracted/mac-app/VariaxWorkbench.app/Contents/Resources/Java/VariaxWorkbench.jar'),
        str(ROOT / 'scripts/InspectBundle.java')]
RUST = [str(ROOT / 'target/debug/variax-workbench')]

def run(command):
    return subprocess.run([str(x) for x in command], check=True, capture_output=True, text=True).stdout

def main():
    subprocess.run(['cargo', 'build'], cwd=ROOT, check=True)
    source = ROOT / 'analysis/extracted/windows-app/modules/variax/device/Default Patch.vwm'
    checks = []
    with tempfile.TemporaryDirectory(prefix='variax-bundle-') as directory:
        for fixed in ('true', 'false'):
            bundle = Path(directory) / f'{fixed}.vwb'
            expected = run([*JAVA, 'create', source, bundle, fixed])
            assert run([*RUST, 'inspect-bundle', bundle]) == expected
            assert run([*JAVA, 'inspect', bundle]) == expected
            extracted = Path(directory) / f'{fixed}.vwm'
            run([*RUST, 'extract-preset', bundle, extracted, '0', '0'])
            assert run([*RUST, 'inspect', extracted]) == run([*RUST, 'inspect', source])
            checks.append({'check': 'original Java creation, reader agreement and Rust preset extraction',
                           'format': 'L6BA' if fixed == 'true' else 'L6CO', 'status': 'pass'})
    report = ROOT / 'analysis/reports/bundle-compatibility.json'
    report.write_text(json.dumps(checks, indent=2) + '\n')
    print(f'PASS: {len(checks)} bundle compatibility checks; report: {report}')

if __name__ == '__main__':
    main()
