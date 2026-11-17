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
from unittest.mock import patch
from release_common import verify_media, SOURCES, sha256

ROOT = Path(__file__).resolve().parents[1]
SPEC = importlib.util.spec_from_file_location('release', ROOT/'scripts/package-release.py')
release = importlib.util.module_from_spec(SPEC)
SPEC.loader.exec_module(release)

class ReleaseTests(unittest.TestCase):
    def test_copy_licenses_accepts_null_license_file(self):
        with tempfile.TemporaryDirectory(dir=SCRATCH) as d:
            root = Path(d)
            package = root/'crate'; package.mkdir()
            (package/'Cargo.toml').write_text('[package]')
            (package/'README.md').write_text('not a license')
            (package/'LICENSE-MIT').write_text('fixture MIT license')
            out = root/'payload'; out.mkdir()
            metadata = {'packages': [{'name': 'fixture', 'version': '1.0.0',
                'source': 'registry+fixture', 'license': 'MIT', 'license_file': None,
                'manifest_path': str(package/'Cargo.toml')}]}
            with patch('release_common.subprocess.check_output', return_value=json.dumps(metadata).encode()):
                release.copy_licenses(out)
            self.assertEqual((out/'licenses/fixture-1.0.0/LICENSE-MIT').read_text(), 'fixture MIT license')
            self.assertFalse((out/'licenses/fixture-1.0.0/README.md').exists())
            self.assertIn('fixture 1.0.0: MIT', (out/'THIRD-PARTY-NOTICES.txt').read_text())

    def test_layout_identity(self):
        with tempfile.TemporaryDirectory(dir=SCRATCH) as d:
            path = Path(d)
            payload = path/'payload'; payload.mkdir()
            (payload/'m4b-Craft.exe').write_bytes(b'fixture')
            source = payload/'source'; source.mkdir(); (source/'Cargo.lock').write_text('fixture')
            wxs = path/'app.wxs'; release.wix(payload,wxs)
            text = wxs.read_text()
            self.assertIn(release.UPGRADE_CODE,text)
            self.assertIn(f'Version="{release.MSI_VERSION}"',text)
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
            for platform, extension in [('windows-x86_64','msi'),('macos-aarch64','dmg')]:
                with self.subTest(platform=platform):
                    installer = path/f'm4b-Craft-0.3.0-alpha.1-{platform}.{extension}'
                    installer.write_bytes(b'test fixture')
                    output = path/'update.json'
                    command = ['python3',str(repo/'scripts/sign-update.py'),'--key',str(key),
                        '--installer',str(installer),'--platform',platform,'--version','0.3.0-alpha.1',
                        '--out',str(output),'--scratch',str(path)]
                    subprocess.run(command,check=True,capture_output=True)
                    envelope = json.loads(output.read_text())
                    self.assertEqual(set(envelope),{'payload','signature'})
                    manifest = json.loads(envelope['payload'])
                    self.assertEqual(set(manifest),{'version','platform','filename','sha256','size','url'})
                    self.assertEqual(manifest['platform'],platform)
                    self.assertEqual(manifest['size'],installer.stat().st_size)
                    self.assertEqual(manifest['sha256'],sha256(installer))
                    self.assertEqual(manifest['filename'],installer.name)
                    self.assertEqual(manifest['url'],f'https://github.com/aruel-perso/m4b-craft/releases/download/v0.3.0-alpha.1/{installer.name}')
                    payload = path/'payload'; payload.write_bytes(envelope['payload'].encode('utf-8'))
                    signature = path/'signature'; signature.write_bytes(bytes.fromhex(envelope['signature']))
                    pub = path/'pub.der'; pub.write_bytes(public)
                    verify = ['openssl','pkeyutl','-verify','-rawin','-pubin','-keyform','DER',
                        '-inkey',str(pub),'-in',str(payload),'-sigfile',str(signature)]
                    self.assertEqual(subprocess.run(verify,capture_output=True).returncode,0)
                    payload.write_bytes(payload.read_bytes()+b' ')
                    self.assertNotEqual(subprocess.run(verify,capture_output=True).returncode,0)
                    wrong = command.copy(); wrong[wrong.index('--platform')+1] = 'macos-aarch64' if platform == 'windows-x86_64' else 'windows-x86_64'
                    result = subprocess.run(wrong,capture_output=True)
                    self.assertNotEqual(result.returncode,0)
                    self.assertIn(b'Expected filename',result.stderr)
                    if platform == 'windows-x86_64':
                        legacy = command.copy(); legacy[legacy.index('--installer')] = '--msi'
                        self.assertEqual(subprocess.run(legacy,capture_output=True).returncode,0)
                    for invalid in ['0.3.0-alpha.01','v0.3.0','0.3.0+build']:
                        bad = command.copy(); bad[bad.index('--version')+1] = invalid
                        self.assertNotEqual(subprocess.run(bad,capture_output=True).returncode,0)
    def test_signing_refuses_wrong_external_key(self):
        with tempfile.TemporaryDirectory(dir=SCRATCH) as d:
            path = Path(d)
            key = path/'key.pem'
            subprocess.run(['openssl','genpkey','-algorithm','ED25519','-out',str(key)],check=True)
            msi = path/'m4b-Craft-0.3.0-alpha.1-windows-x86_64.msi'; msi.write_bytes(b'test fixture')
            output = path/'update.json'
            result = subprocess.run(['python3',str(ROOT/'scripts/sign-update.py'),'--key',str(key),
                '--msi',str(msi),'--version','0.3.0-alpha.1','--out',str(output),'--scratch',str(path)],capture_output=True)
            self.assertNotEqual(result.returncode,0)
            self.assertIn(b'does not match pinned public key',result.stderr)
            self.assertFalse(output.exists())

    def test_media_cache_requires_exact_records_and_pinned_sources(self):
        with tempfile.TemporaryDirectory(dir=SCRATCH) as d:
            media = Path(d)
            names = ['ffmpeg','ffprobe','config.h','config_components.h','config.mak',
                     'COPYING.LGPLv2.1','LICENSE.md','LICENSE.zlib','zlib-config.txt'] + list(SOURCES)
            for name in names: (media/name).write_bytes(b'fixture')
            def manifest():
                (media/'SHA256SUMS').write_text(''.join(sha256(media/name)+'  '+name+'\n' for name in names))
            manifest()
            with self.assertRaisesRegex(ValueError,'Unpinned source'): verify_media(media,'macos-aarch64')
            with patch.dict(SOURCES,{name:sha256(media/name) for name in SOURCES}):
                verify_media(media,'macos-aarch64')
                (media/'ffmpeg').write_bytes(b'corrupted')
                with self.assertRaisesRegex(ValueError,'checksum mismatch'): verify_media(media,'macos-aarch64')
                manifest()
                names.remove('ffprobe'); manifest()
                with self.assertRaisesRegex(ValueError,'cache contents'): verify_media(media,'macos-aarch64')

    def test_macho_rejects_external_dylibs_and_wrong_architecture(self):
        spec = importlib.util.spec_from_file_location('macho',ROOT/'scripts/verify-macos.py')
        macho = importlib.util.module_from_spec(spec); spec.loader.exec_module(macho)
        for arch, dependency, accepted in [
            ('arm64','/usr/lib/libSystem.B.dylib',True),
            ('arm64','/System/Library/Frameworks/AppKit.framework/Versions/C/AppKit',True),
            ('x86_64','/usr/lib/libSystem.B.dylib',False),
            ('arm64 x86_64','/usr/lib/libSystem.B.dylib',False),
            ('arm64','/opt/homebrew/lib/libz.1.dylib',False),
            ('arm64','@rpath/libz.1.dylib',False),
            ('arm64','/usr/local/lib/libz.1.dylib',False)]:
            with self.subTest(arch=arch,dependency=dependency):
                with patch.object(macho.subprocess,'check_output',side_effect=[arch+'\n',f'fixture:\n\t{dependency} (compatibility version 1.0.0, current version 1.0.0)\n']):
                    if accepted: macho.verify(Path('fixture'))
                    else:
                        with self.assertRaises(ValueError): macho.verify(Path('fixture'))

    def test_flat_assets_fail_on_missing_nested_or_changed_files(self):
        spec = importlib.util.spec_from_file_location('assets',ROOT/'scripts/verify-release-assets.py')
        assets = importlib.util.module_from_spec(spec); spec.loader.exec_module(assets)
        with tempfile.TemporaryDirectory(dir=SCRATCH) as d:
            directory = Path(d)
            for platform, extensions in [('windows-x86_64',['msi','zip']),('macos-aarch64',['dmg'])]:
                for suffix in ['.'+ext for ext in extensions]+['-media-sources.zip']:
                    file = directory/f'm4b-Craft-0.3.0-alpha.1-{platform}{suffix}'
                    file.write_bytes(b'fixture')
                    file.with_name(file.name+'.sha256').write_text(sha256(file)+'  '+file.name+'\n')
            assets.verify(directory,'0.3.0-alpha.1')
            extra = directory/'nested'; extra.mkdir()
            with self.assertRaises(ValueError): assets.verify(directory,'0.3.0-alpha.1')
            extra.rmdir()
            next(directory.glob('*.dmg')).write_bytes(b'changed')
            with self.assertRaisesRegex(ValueError,'Checksum mismatch'): assets.verify(directory,'0.3.0-alpha.1')

def inspect(msi, scratch):
    tool = os.environ.get('MSIINFO','msiinfo')
    properties = subprocess.check_output([tool,'export',str(msi),'Property']).decode()
    for value in [release.UPGRADE_CODE.upper(), release.MSI_VERSION,'m4b-Craft']:
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
