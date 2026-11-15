"""Stage a package built from this checkout and test its own Rust binaries.

Uses synthetic inputs only. No device command, driver action or installer other
than the package's file-copy manager is run. --screenshots opens brief X11 GUI
windows, including on XWayland; it never changes desktop configuration.
"""
import argparse
import hashlib
import json
import os
from pathlib import Path
import subprocess
import sys
import tarfile
import tempfile

ROOT = Path(__file__).resolve().parents[1]


def sha(path):
    with path.open('rb') as stream: return hashlib.file_digest(stream,'sha256').hexdigest()


def verify(archive, screenshots):
    (ROOT/'.tools').mkdir(exist_ok=True)
    evidence = Path(tempfile.mkdtemp(prefix='package-verification-',dir=ROOT/'.tools'))
    report = {'archive_sha256':sha(archive), 'verifier_sha256':sha(Path(__file__)),
              'device_access':False, 'scope':'Own Rust package; synthetic file inputs; no device commands'}
    try:
        with tarfile.open(archive) as tar:
            members = tar.getmembers()
            if len(members)>20_000 or sum(m.size for m in members)>512*1024*1024:
                raise ValueError('package exceeds verification bounds')
            if any(not (m.isfile() or m.isdir()) for m in members):
                raise ValueError('non-file/directory member in package')
            tar.extractall(evidence,filter='data')
        directories = list(evidence.iterdir())
        if len(directories)!=1 or not directories[0].is_dir():
            raise ValueError('expected one package directory')
        package = directories[0]
        manifest = json.loads((package/'manifest.json').read_text())
        current = subprocess.check_output(['git','rev-parse','HEAD'],cwd=ROOT,text=True).strip()
        if manifest['revision']!=current:
            raise ValueError('package revision differs from this checkout; verify at its source revision')
        report.update(revision=current, math_profile=manifest['math_profile'],
                      desktop_backend=manifest['desktop_backend'], payload_files=len(manifest['files']))
        prefix = evidence/'prefix with spaces'
        manager = package/'install.py'
        subprocess.run([sys.executable,str(manager),'install','--prefix',str(prefix)],check=True)
        desktop = prefix/'share/applications/io.github.guirossibrum.VariaxWorkbench.desktop'
        subprocess.run(['desktop-file-validate',str(desktop)],check=True)
        if f'--display {manifest["desktop_backend"]} %f' not in desktop.read_text():
            raise ValueError('installed launcher lost backend/file arguments')
        cli, gui = prefix/'bin/variax-workbench', prefix/'bin/variax-workbench-gui'
        profile = subprocess.check_output([str(cli),'math-profile'],text=True).strip()
        if profile!=manifest['math_profile']: raise ValueError('compiled profile differs')
        subprocess.run([str(gui),'--help'],check=True,stdout=subprocess.DEVNULL)
        subprocess.run([str(prefix/'bin/variax-check'),'plan'],check=True,stdout=subprocess.DEVNULL)
        fixture = evidence/'synthetic preset.vwm'
        fixture.write_bytes((ROOT/'tests/fixtures/synthetic-host-preset.vwm').read_bytes())
        subprocess.run([str(cli),'inspect',str(fixture)],check=True,stdout=subprocess.DEVNULL)
        subprocess.run([str(prefix/'bin/variax-firmware-inspect'),
                        str(ROOT/'tests/fixtures/synthetic-l6ff-python.bin')],
                       check=True,stdout=subprocess.DEVNULL)
        if screenshots:
            environment = dict(os.environ, VARIAX_WORKBENCH_DATA_DIR=str(evidence/'empty-data'),
                               LIBGL_ALWAYS_SOFTWARE='1')
            captures = []
            for case,arguments in [('welcome',[]),('synthetic-preset',[str(fixture),'--tab','pickups'])]:
                screenshot = evidence/(case+'.png')
                with (evidence/(case+'.log')).open('w') as output:
                    subprocess.run([str(gui),'--display','x11',*arguments,'--screenshot',str(screenshot)],
                                   cwd=evidence,env=environment,stdout=output,stderr=subprocess.STDOUT,
                                   check=True,timeout=30)
                if not screenshot.read_bytes().startswith(b'\x89PNG\r\n\x1a\n'):
                    raise ValueError('screenshot missing/invalid')
                captures.append({'case':case,'png_sha256':sha(screenshot)})
            report['x11_captures'] = captures
            if (evidence/'empty-data').exists():
                raise ValueError('launch unexpectedly created the optional artwork directory')
        marker = prefix/'share/variax-workbench/artwork/retained-user-file'
        marker.parent.mkdir();marker.write_bytes(b'synthetic preservation marker')
        subprocess.run([sys.executable,str(manager),'uninstall','--prefix',str(prefix)],check=True)
        if marker.read_bytes()!=b'synthetic preservation marker': raise ValueError('user data changed')
        if (prefix/'bin/variax-workbench-gui').exists(): raise ValueError('GUI not removed')
        report.update(status='pass', user_data_preserved=True)
    except Exception as error:
        report.update(status='fail', error=type(error).__name__+': '+str(error))
        raise
    finally:
        (evidence/'validation.json').write_text(json.dumps(report,indent=2)+'\n')
        print(evidence,flush=True)


if __name__=='__main__':
    parser=argparse.ArgumentParser(description=__doc__)
    parser.add_argument('archive',type=Path)
    parser.add_argument('--screenshots',action='store_true')
    args=parser.parse_args()
    verify(args.archive.resolve(),args.screenshots)
