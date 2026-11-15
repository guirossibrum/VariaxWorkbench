"""Synthetic package integrity, non-overwrite, rollback and uninstall checks."""
import hashlib
import json
from pathlib import Path
import sys
import tempfile
import unittest
from unittest.mock import patch

ROOT = Path(__file__).resolve().parents[1]
sys.path.insert(0, str(ROOT/'packaging/linux'))
import install
from package_linux import archive


def package(root):
    root.mkdir()
    entries = []
    for name in ['bin/variax-check', 'bin/variax-workbench-gui', 'share/doc/variax-workbench/test.txt']:
        data = ('synthetic '+name).encode()
        path = root/'files'/name;path.parent.mkdir(parents=True, exist_ok=True);path.write_bytes(data);path.chmod(0o644)
        entries.append({'path':name, 'sha256':hashlib.sha256(data).hexdigest(), 'size':len(data), 'mode':0o644})
    manifest = {'application':install.APP, 'format':1, 'hardware_access_locked':True,
                'vendor_resources_included':False, 'files':entries}
    (root/'manifest.json').write_text(json.dumps(manifest))
    return manifest


class PackageGuards(unittest.TestCase):
    def test_launcher_uses_absolute_executable_and_receipt_tracks_its_transformed_bytes(self):
        with tempfile.TemporaryDirectory() as temporary:
            root=Path(temporary);payload=root/'package';manifest=package(payload);prefix=root/'a prefix'
            name=f'share/applications/{install.DESKTOP_ID}.desktop'
            data=b'[Desktop Entry]\nType=Application\nExec=variax-workbench-gui --display x11 %f\nTryExec=variax-workbench-gui\nIcon=original\n'
            path=payload/'files'/name;path.parent.mkdir(parents=True);path.write_bytes(data);path.chmod(0o644)
            manifest['files'].append({'path':name,'sha256':hashlib.sha256(data).hexdigest(),'size':len(data),'mode':0o644})
            (payload/'manifest.json').write_text(json.dumps(manifest));install.install(payload,prefix)
            text=(prefix/name).read_text()
            self.assertIn(f'Exec="{prefix}/bin/variax-workbench-gui" --display x11 %f\n',text)
            self.assertIn(f'TryExec={prefix}/bin/variax-workbench-gui\n',text)
            receipt=json.loads((prefix/install.RECEIPT).read_text())
            self.assertTrue(all(install.matches(prefix/e['path'],e) for e in receipt['files']))
            install.uninstall(prefix);self.assertFalse((prefix/name).exists())
        for bad in ['relative','/tmp/bad=prefix','/tmp/bad%prefix','/tmp/bad\nprefix']:
            with self.assertRaises(ValueError):install.check_prefix(Path(bad))

    def test_install_remove_preserves_artwork_and_unrelated_files_in_space_prefix(self):
        with tempfile.TemporaryDirectory() as temporary:
            root=Path(temporary);payload=root/'package';manifest=package(payload);prefix=root/'a prefix'
            install.install(payload,prefix)
            for entry in manifest['files']:
                self.assertTrue(install.matches(prefix/entry['path'],entry))
            artwork=prefix/'share/variax-workbench/artwork/user.png';artwork.parent.mkdir();artwork.write_bytes(b'synthetic user file')
            unrelated=prefix/'bin/other-tool';unrelated.write_bytes(b'other')
            install.uninstall(prefix)
            self.assertEqual(artwork.read_bytes(),b'synthetic user file')
            self.assertEqual(unrelated.read_bytes(),b'other')
            self.assertFalse((prefix/install.RECEIPT).exists())
            self.assertFalse(any((prefix/e['path']).exists() for e in manifest['files']))

    def test_collision_or_changed_payload_fails_before_installing_any_file(self):
        with tempfile.TemporaryDirectory() as temporary:
            root=Path(temporary);payload=root/'package';package(payload);prefix=root/'prefix'
            target=prefix/'bin/variax-workbench-gui';target.parent.mkdir(parents=True);target.write_bytes(b'owner')
            with self.assertRaisesRegex(ValueError,'replace existing'):install.install(payload,prefix)
            self.assertEqual(target.read_bytes(),b'owner');self.assertFalse((prefix/'bin/variax-check').exists())
            target.unlink();(payload/'files/bin/variax-check').write_bytes(b'changed')
            with self.assertRaisesRegex(ValueError,'digest mismatch'):install.install(payload,prefix)
            self.assertEqual(list((prefix/'bin').iterdir()),[])

    def test_changed_installed_file_prevents_all_uninstall_deletions(self):
        with tempfile.TemporaryDirectory() as temporary:
            root=Path(temporary);payload=root/'package';manifest=package(payload);prefix=root/'prefix';install.install(payload,prefix)
            changed=prefix/manifest['files'][-1]['path'];changed.write_bytes(b'owner edit')
            with self.assertRaisesRegex(ValueError,'nothing removed'):install.uninstall(prefix)
            self.assertTrue(all((prefix/e['path']).exists() for e in manifest['files']))
            self.assertTrue((prefix/install.RECEIPT).exists());self.assertEqual(changed.read_bytes(),b'owner edit')

    def test_late_collision_rolls_back_only_the_files_created_by_this_install(self):
        with tempfile.TemporaryDirectory() as temporary:
            root=Path(temporary);payload=root/'package';package(payload);prefix=root/'prefix'
            original=install.create_new;calls=[]
            def collide(path,data,mode):
                calls.append(path)
                if len(calls)==2:
                    path.write_bytes(b'competing owner');raise FileExistsError('synthetic late collision')
                return original(path,data,mode)
            with patch.object(install,'create_new',side_effect=collide):
                with self.assertRaises(FileExistsError):install.install(payload,prefix)
            self.assertFalse(calls[0].exists());self.assertEqual(calls[1].read_bytes(),b'competing owner')
            self.assertFalse((prefix/install.RECEIPT).exists())

    def test_symlink_redirect_is_rejected_without_touching_the_target(self):
        with tempfile.TemporaryDirectory() as temporary:
            root=Path(temporary);payload=root/'package';package(payload);prefix=root/'prefix';prefix.mkdir();outside=root/'outside';outside.mkdir()
            try:(prefix/'bin').symlink_to(outside,target_is_directory=True)
            except OSError as error:self.skipTest(f'symlink not available: {error}')
            with self.assertRaisesRegex(ValueError,'symlink'):install.install(payload,prefix)
            self.assertEqual(list(outside.iterdir()),[])

    def test_manifest_cannot_target_user_data_drivers_or_parent_paths(self):
        for name in ['../outside', '/outside', 'bin/../outside', 'bin\\outside',
                     'lib/modules/driver.ko', 'share/variax-workbench/artwork/user.png', 'bin/unknown']:
            with self.assertRaises(ValueError):install.relative_name(name)
        with tempfile.TemporaryDirectory() as temporary:
            manifest=package(Path(temporary)/'package')
            manifest['files'].append(manifest['files'][0])
            with self.assertRaisesRegex(ValueError,'duplicate'):install.entries(manifest)
            manifest['files'].pop();manifest['hardware_access_locked']=False
            with self.assertRaisesRegex(ValueError,'offline-only'):install.entries(manifest)

    def test_archive_is_repeatable_for_the_same_payload_and_cannot_replace_a_file(self):
        with tempfile.TemporaryDirectory() as temporary:
            root=Path(temporary);payload=root/'package';package(payload)
            a=root/'a.tar.gz';b=root/'b.tar.gz';archive(payload,a,1234567890);archive(payload,b,1234567890)
            self.assertEqual(a.read_bytes(),b.read_bytes());before=a.read_bytes()
            with self.assertRaisesRegex(ValueError,'replace archive'):archive(payload,a,1234567890)
            self.assertEqual(a.read_bytes(),before)


if __name__=='__main__':unittest.main()
