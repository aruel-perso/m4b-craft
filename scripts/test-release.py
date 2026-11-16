#!/usr/bin/env python3
"""Exercise release packaging/signing and optionally inspect/extract an actual MSI."""
import argparse
import importlib.util
import json
import os
import hashlib
import shutil
from pathlib import Path
import subprocess
import tempfile
import unittest
import zipfile

ROOT = Path(__file__).resolve().parents[1]
SPEC = importlib.util.spec_from_file_location('release', ROOT/'scripts/package-release.py')
release = importlib.util.module_from_spec(SPEC)
SPEC.loader.exec_module(release)

class ReleaseTests(unittest.TestCase):
    def test_layout_identity(self):
        with tempfile.TemporaryDirectory(dir=SCRATCH) as d:
            path = Path(d)
            payload = path/'payload'; payload.mkdir()
            (payload/'m4b-Craft.exe').write_bytes(b'fixture')
            source = payload/'source'; source.mkdir(); (source/'Cargo.lock').write_text('fixture')
            wxs = path/'app.wxs'; release.wix(payload,wxs)
            text = wxs.read_text()
            self.assertIn(release.UPGRADE_CODE,text)
            self.assertIn('Version="0.2.0"',text)
            self.assertIn('InstallScope="perUser"',text)
            self.assertIn('MajorUpgrade',text)
            self.assertNotIn('RemoveFile',text)
            self.assertIn('Cargo.lock',text)
    def test_signing_binds_payload_and_rejects_tampering(self):
        with tempfile.TemporaryDirectory(dir=SCRATCH) as d:
            path = Path(d)
            repo = path/'repo'
            (repo/'scripts').mkdir(parents=True)
            (repo/'packaging').mkdir()
            shutil.copy2(ROOT/'scripts/sign-update.py',repo/'scripts/sign-update.py')
            key = path/'key.pem'
            subprocess.run(['openssl','genpkey','-algorithm','ED25519','-out',str(key)],check=True)
            public = subprocess.check_output(['openssl','pkey','-in',str(key),'-pubout','-outform','DER'])
            (repo/'packaging/update-public-key.hex').write_text(public[-32:].hex())
            msi = path/'m4b-Craft-0.2.0-alpha.1-windows-x86_64.msi'; msi.write_bytes(b'test fixture')
            output = path/'update.json'
            subprocess.run(['python3',str(repo/'scripts/sign-update.py'),'--key',str(key),
                '--msi',str(msi),'--version','0.2.0-alpha.1','--out',str(output),'--scratch',str(path)],check=True)
            envelope = json.loads(output.read_text())
            manifest = json.loads(envelope['payload'])
            self.assertEqual(manifest['size'],msi.stat().st_size)
            self.assertEqual(manifest['sha256'],hashlib.sha256(msi.read_bytes()).hexdigest())
            self.assertEqual(manifest['filename'],msi.name)
            payload = path/'payload'; payload.write_text(envelope['payload']+' ')
            signature = path/'signature'; signature.write_bytes(bytes.fromhex(envelope['signature']))
            pub = path/'pub.der'; pub.write_bytes(public)
            result = subprocess.run(['openssl','pkeyutl','-verify','-rawin','-pubin','-keyform','DER',
                '-inkey',str(pub),'-in',str(payload),'-sigfile',str(signature)],capture_output=True)
            self.assertNotEqual(result.returncode,0)
    def test_signing_refuses_wrong_external_key(self):
        with tempfile.TemporaryDirectory(dir=SCRATCH) as d:
            path = Path(d)
            key = path/'key.pem'
            subprocess.run(['openssl','genpkey','-algorithm','ED25519','-out',str(key)],check=True)
            msi = path/'m4b-Craft-0.2.0-alpha.1-windows-x86_64.msi'; msi.write_bytes(b'test fixture')
            output = path/'update.json'
            result = subprocess.run(['python3',str(ROOT/'scripts/sign-update.py'),'--key',str(key),
                '--msi',str(msi),'--version','0.2.0-alpha.1','--out',str(output),'--scratch',str(path)],capture_output=True)
            self.assertNotEqual(result.returncode,0)
            self.assertIn(b'does not match pinned public key',result.stderr)
            self.assertFalse(output.exists())

def inspect(msi, scratch):
    tool = os.environ.get('MSIINFO','msiinfo')
    properties = subprocess.check_output([tool,'export',str(msi),'Property']).decode()
    for value in [release.UPGRADE_CODE.upper(), '0.2.0','m4b-Craft']:
        assert value.lower() in properties.lower(),value
    files = subprocess.check_output([tool,'export',str(msi),'File']).decode()
    for name in ['m4b-Craft.exe','m4b-cli.exe','ffmpeg.exe','ffprobe.exe','Cargo.lock','LICENSE']:
        assert name in files,name
    assert '\nALLUSERS\t1' not in properties
    directories = subprocess.check_output([tool,'export',str(msi),'Directory']).decode()
    assert 'LocalAppDataFolder' in directories
    sequence = subprocess.check_output([tool,'export',str(msi),'InstallExecuteSequence']).decode()
    actions = {row.split('\t')[0]: row.split('\t')[-1].strip() for row in sequence.splitlines()[3:]}
    assert int(actions['RemoveExistingProducts']) > int(actions['InstallInitialize'])
    upgrade = subprocess.check_output([tool,'export',str(msi),'Upgrade']).decode()
    assert release.UPGRADE_CODE.lower() in upgrade.lower()
    with zipfile.ZipFile(msi.with_suffix('.zip')) as archive:
        assert archive.testzip() is None
        for name in ['m4b-Craft.exe','m4b-cli.exe','ffmpeg.exe','ffprobe.exe','source/crates/desktop/src/updater.rs','source/Cargo.lock']:
            assert name in archive.namelist(),name
        assert not any(name.lower().endswith(('.pem','.key')) for name in archive.namelist())
    sidecar = msi.with_name(msi.stem+'-media-sources.zip')
    with zipfile.ZipFile(sidecar) as archive:
        assert archive.testzip() is None
        for name, digest in [('ffmpeg-8.0.1.tar.xz','05ee0b03119b45c0bdb4df654b96802e909e0a752f72e4fe3794f487229e5a41'),('zlib-1.3.1.tar.gz','9a93b2b7dfdac77ceba5a558a580e74667dd6fede4585b91eefb60f03b72df23')]:
            assert hashlib.sha256(archive.read(name)).hexdigest() == digest
    with tempfile.TemporaryDirectory(dir=scratch) as d:
        subprocess.run([os.environ.get('MSIEXTRACT','msiextract'),'-C',d,str(msi)],check=True,stdout=subprocess.DEVNULL)
        for path in Path(d).rglob('m4b-Craft.exe'):
            assert path.read_bytes().startswith(b'MZ')
            assert (path.parent/'source/crates/desktop/src/updater.rs').exists()
            assert (path.parent/'ffprobe.exe').exists()
            break
        else: raise AssertionError('Missing extracted application')

if __name__ == '__main__':
    parser = argparse.ArgumentParser(); parser.add_argument('--scratch',required=True,type=Path)
    parser.add_argument('--msi',type=Path); args = parser.parse_args()
    SCRATCH = args.scratch; SCRATCH.mkdir(parents=True,exist_ok=True)
    result = unittest.TextTestRunner(verbosity=2).run(unittest.defaultTestLoader.loadTestsFromTestCase(ReleaseTests))
    if not result.wasSuccessful(): raise SystemExit(1)
    if args.msi:
        inspect(args.msi.resolve(),SCRATCH)
        print('PASS MSI tables/extraction, portable ZIP contents/CRC, source sidecar hashes, key exclusion')
