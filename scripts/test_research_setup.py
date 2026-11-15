"""Synthetic archive/setup guard tests; no original artifacts or external tools."""
import hashlib
import io
import json
from pathlib import Path
import tempfile
import unittest
from unittest.mock import patch
import zipfile

import research_setup as setup


def odc(name, data=b'', mode=0o100755):
    encoded=name.encode()+b'\0'
    header=('070707'+''.join(f'{x:06o}' for x in [0,1,mode,0,0,1,0])+f'{0:011o}{len(encoded):06o}{len(data):011o}').encode()
    assert len(header)==76
    return header+encoded+data


class SetupGuards(unittest.TestCase):
    def test_old_cpio_preserves_data_without_creating_executable_or_symbolic_files(self):
        data=odc('.',mode=0o40755)+odc('./app/file',b'archive data')+odc('./link',b'../../outside',0o120777)+odc('TRAILER!!!')
        with tempfile.TemporaryDirectory() as d:
            root=Path(d);report=setup.unpack_odc(data,root)
            self.assertEqual((root/'app/file').read_bytes(),b'archive data')
            self.assertFalse((root/'app/file').stat().st_mode & 0o111)
            self.assertFalse((root/'link').exists())
            self.assertEqual(report['regular_files'],1)
            self.assertEqual(report['unmaterialized_links'],[{'path':'./link','target':'../../outside'}])

    def test_every_truncated_cpio_prefix_and_unsafe_name_is_rejected(self):
        data=odc('file',b'payload')+odc('TRAILER!!!')
        with tempfile.TemporaryDirectory() as d:
            for length in range(len(data)):
                with self.assertRaises(ValueError):setup.unpack_odc(data[:length],Path(d))
            for name in ['../outside','/outside','C:/outside','a/../../outside','a\\..\\outside']:
                with self.assertRaises(ValueError):setup.unpack_odc(odc(name)+odc('TRAILER!!!'),Path(d))

    def test_zip_paths_and_symbolic_links_cannot_escape_output(self):
        with tempfile.TemporaryDirectory() as d:
            root=Path(d)
            for name in ['../outside','/outside','C:/outside','a\\outside','a\0hidden']:
                data=io.BytesIO()
                # ZipInfo's constructor normalizes OS separators. Set raw names
                # afterward so the malicious corpus is identical on Windows.
                member=zipfile.ZipInfo('placeholder');member.filename=name;member.orig_filename=name
                with zipfile.ZipFile(data,'w') as z:z.writestr(member,b'data')
                data.seek(0)
                with zipfile.ZipFile(data) as z:self.assertEqual(z.infolist()[0].orig_filename,name)
                data.seek(0)
                with self.assertRaises(ValueError):setup.unzip(data,root)
            data=io.BytesIO();member=zipfile.ZipInfo('link');member.create_system=3;member.external_attr=0o120777<<16
            with zipfile.ZipFile(data,'w') as z:z.writestr(member,'../outside')
            data.seek(0)
            with self.assertRaises(ValueError):setup.unzip(data,root)

    def test_extracted_existing_files_are_never_replaced_with_different_data(self):
        with tempfile.TemporaryDirectory() as d:
            path=Path(d)/'existing';setup.write_new(path,b'original');before=path.stat()
            setup.write_new(path,b'original')
            self.assertEqual(path.stat().st_mtime_ns,before.st_mtime_ns)
            with self.assertRaises(ValueError):setup.write_new(path,b'other')
            self.assertEqual(path.read_bytes(),b'original')

    def test_preexisting_directory_link_cannot_redirect_extraction(self):
        with tempfile.TemporaryDirectory() as d:
            root=Path(d)/'root';outside=Path(d)/'outside';root.mkdir();outside.mkdir()
            try:(root/'link').symlink_to(outside,target_is_directory=True)
            except OSError as error:self.skipTest(f'OS does not permit test symlink: {error}')
            with self.assertRaises(ValueError):setup.destination(root,'link/file')
            self.assertEqual(list(outside.iterdir()),[])

    def test_artifact_identity_mismatch_fails_before_archive_tools_can_run(self):
        with tempfile.TemporaryDirectory() as d:
            root=Path(d);(root/'research').mkdir();payload=b'original bytes';path=root/'app.exe';path.write_bytes(payload)
            (root/'research/artifacts.json').write_text(json.dumps({'artifacts':[{'name':'app.exe','size':len(payload),'sha256':hashlib.sha256(payload).hexdigest()}]}))
            with patch.object(setup,'ROOT',root):
                self.assertEqual(setup.checked_artifact(root,'app.exe'),path)
                path.write_bytes(b'changed bytes!')
                with self.assertRaisesRegex(ValueError,'identity mismatch'):setup.checked_artifact(root,'app.exe')
                path.unlink()
                with self.assertRaisesRegex(ValueError,'missing original artifact'):setup.checked_artifact(root,'app.exe')

    def test_conflicting_derived_index_is_refused_without_overwriting_it(self):
        with tempfile.TemporaryDirectory() as d:
            root=Path(d);(root/'research').mkdir()
            metadata={'target_sha256':'synthetic target','functions':[{'address':'0x10','name':'synthetic','status':'written','body':{'ranges':[{'start':'0x10','end':'0x20'}]}}]}
            (root/'research/native-functions.json').write_text(json.dumps(metadata))
            with patch.object(setup,'ROOT',root):
                setup.index()
                path=root/'analysis/decompiled/native-coef/decompile-index.json'
                self.assertEqual(json.loads(path.read_text()),metadata)
                changed=dict(metadata,target_sha256='different target');path.write_text(json.dumps(changed));before=path.read_bytes()
                with self.assertRaisesRegex(ValueError,'differs'):setup.index()
                self.assertEqual(path.read_bytes(),before)

    def test_device_probe_or_unlisted_script_is_rejected_before_any_process(self):
        with patch.object(setup.subprocess,'run',side_effect=AssertionError('must not run')):
            with self.assertRaisesRegex(ValueError,'not an approved'):
                setup.references(['read_variax_usb'])

    def test_atomic_save_race_preserves_competing_file_and_removes_temporary_copy(self):
        with tempfile.TemporaryDirectory() as d:
            root=Path(d);target=root/'output'
            def raced_link(source,destination):
                Path(destination).write_bytes(b'competing owner')
                raise FileExistsError('simulated race')
            with patch.object(setup.os,'link',side_effect=raced_link):
                with self.assertRaisesRegex(ValueError,'raced replacement'):
                    setup.write_new(target,b'new bytes')
            self.assertEqual(target.read_bytes(),b'competing owner')
            self.assertEqual(list(root.iterdir()),[target])


if __name__=='__main__':unittest.main()
