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
from release_common import verify_media, SOURCES, ARCHIVER_MEMBERS, sha256

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

    def test_source_copy_excludes_private_notes_and_includes_tests(self):
        from release_common import copy_source
        with tempfile.TemporaryDirectory(dir=SCRATCH) as d:
            out = Path(d)/'source'
            copy_source(out)
            for name in ['BRIEF.md','VALIDATION.md','ALPHA-0.1.1.md','implementation-plan.md']:
                self.assertFalse((out/name).exists(), name)
            self.assertTrue((out/'docs/development.md').is_file())
            self.assertTrue((out/'crates').is_dir())
            if (ROOT/'tests').exists(): self.assertTrue((out/'tests').is_dir())

    def test_layout_identity(self):
        with tempfile.TemporaryDirectory(dir=SCRATCH) as d:
            path = Path(d)
            payload = path/'payload'; payload.mkdir()
            (payload/'m4b-editor.exe').write_bytes(b'fixture')
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
            self.assertIn('LegacyApplicationProgramsFolder',text)
            self.assertIn('Name="M4B Editor"',text)
            self.assertIn('Name="m4b-Craft"',text)
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
                    installer = path/f'M4B-Editor-0.4.0-{platform}.{extension}'
                    installer.write_bytes(b'test fixture')
                    output = path/f'update-v2-{platform}.json'
                    command = ['python3',str(repo/'scripts/sign-update.py'),'--key',str(key),
                        '--installer',str(installer),'--platform',platform,'--version','0.4.0',
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
                    self.assertEqual(manifest['url'],f'https://github.com/aruel-perso/m4b-editor/releases/download/v0.4.0/{installer.name}')
                    payload = path/'payload'; payload.write_bytes(envelope['payload'].encode('utf-8'))
                    signature = path/'signature'; signature.write_bytes(bytes.fromhex(envelope['signature']))
                    pub = path/'pub.der'; pub.write_bytes(public)
                    verify = ['openssl','pkeyutl','-verify','-rawin','-pubin','-keyform','DER',
                        '-inkey',str(pub),'-in',str(payload),'-sigfile',str(signature)]
                    self.assertEqual(subprocess.run(verify,capture_output=True).returncode,0)
                    legacy_installer = installer.with_name(installer.name.replace('M4B-Editor','m4b-Craft'))
                    shutil.copy2(installer, legacy_installer)
                    canonical_alias = command.copy()
                    canonical_alias[canonical_alias.index('--installer')+1] = str(legacy_installer)
                    self.assertNotEqual(subprocess.run(canonical_alias,capture_output=True).returncode,0)
                    legacy_command = command.copy() + ['--legacy']
                    legacy_command[legacy_command.index('--installer')+1] = str(legacy_installer)
                    legacy_output = path/f'update-{platform}.json'
                    legacy_command[legacy_command.index('--out')+1] = str(legacy_output)
                    subprocess.run(legacy_command,check=True,capture_output=True)
                    legacy_envelope = json.loads(legacy_output.read_text())
                    legacy_manifest = json.loads(legacy_envelope['payload'])
                    self.assertEqual(legacy_manifest['url'],f'https://github.com/aruel-perso/m4b-craft/releases/download/v0.4.0/{legacy_installer.name}')
                    payload.write_bytes(legacy_envelope['payload'].encode())
                    signature.write_bytes(bytes.fromhex(legacy_envelope['signature']))
                    self.assertEqual(subprocess.run(verify,capture_output=True).returncode,0)
                    payload.write_bytes(payload.read_bytes()+b' ')
                    self.assertNotEqual(subprocess.run(verify,capture_output=True).returncode,0)
                    wrong = command.copy(); wrong[wrong.index('--platform')+1] = 'macos-aarch64' if platform == 'windows-x86_64' else 'windows-x86_64'
                    result = subprocess.run(wrong,capture_output=True)
                    self.assertNotEqual(result.returncode,0)
                    self.assertIn(b'Expected',result.stderr)
                    if platform == 'windows-x86_64':
                        legacy = command.copy(); legacy[legacy.index('--installer')] = '--msi'
                        self.assertEqual(subprocess.run(legacy,capture_output=True).returncode,0)
                    for invalid in ['0.3.0-alpha.01','v0.3.0','0.3.0+build']:
                        bad = command.copy(); bad[bad.index('--version')+1] = invalid
                        self.assertNotEqual(subprocess.run(bad,capture_output=True).returncode,0)
    def test_signed_exact_allowlists_for_both_channels(self):
        spec = importlib.util.spec_from_file_location('assets',ROOT/'scripts/verify-release-assets.py')
        assets = importlib.util.module_from_spec(spec); spec.loader.exec_module(assets)
        with tempfile.TemporaryDirectory(dir=SCRATCH) as d:
            root = Path(d); repo = root/'repo'
            (repo/'scripts').mkdir(parents=True); (repo/'packaging').mkdir()
            shutil.copy2(ROOT/'scripts/sign-update.py',repo/'scripts/sign-update.py')
            key = root/'key.pem'
            subprocess.run(['openssl','genpkey','-algorithm','ED25519','-out',str(key)],check=True)
            public = subprocess.check_output(['openssl','pkey','-in',str(key),'-pubout','-outform','DER'])
            (repo/'packaging/update-public-key.hex').write_text(public[-32:].hex())
            for legacy in [False,True]:
                out = root/('legacy' if legacy else 'canonical'); out.mkdir()
                prefix = 'm4b-Craft' if legacy else 'M4B-Editor'
                for platform, ext in [('windows-x86_64','msi'),('macos-aarch64','dmg')]:
                    base = f'{prefix}-0.4.0-{platform}'
                    for suffix in ['.'+ext,'-media-sources.zip','-licenses.zip'] + (['.zip'] if platform=='windows-x86_64' and not legacy else []):
                        file = out/(base+suffix); file.write_bytes(b'offline fixture')
                        file.with_name(file.name+'.sha256').write_text(sha256(file)+'  '+file.name+'\n')
                    manifest = out/(f'update-{platform}.json' if legacy else f'update-v2-{platform}.json')
                    command = ['python3',str(repo/'scripts/sign-update.py'),'--key',str(key),'--version','0.4.0',
                               '--platform',platform,'--installer',str(out/(base+'.'+ext)),
                               '--out',str(manifest),'--scratch',str(root)] + (['--legacy'] if legacy else [])
                    subprocess.run(command,check=True,capture_output=True)
                with patch.object(assets,'ROOT',repo):
                    assets.verify(out,'0.4.0',legacy=legacy,signed=True)
                    wrong = next(out.glob('update-*.json'))
                    envelope = json.loads(wrong.read_text()); payload = json.loads(envelope['payload'])
                    payload['url'] = payload['url'].replace('/releases/', '/redirect/releases/')
                    envelope['payload'] = json.dumps(payload); wrong.write_text(json.dumps(envelope))
                    with self.assertRaisesRegex(ValueError,'origin/content'):
                        assets.verify(out,'0.4.0',legacy=legacy,signed=True)

    def test_prepare_legacy_preserves_windows_bytes_and_requires_legacy_dmg(self):
        spec = importlib.util.spec_from_file_location('legacy',ROOT/'scripts/prepare-legacy-release.py')
        legacy = importlib.util.module_from_spec(spec); spec.loader.exec_module(legacy)
        with tempfile.TemporaryDirectory(dir=SCRATCH) as d:
            root = Path(d); canonical = root/'canonical'; canonical.mkdir()
            windows = canonical/'M4B-Editor-0.4.0-windows-x86_64.msi'
            windows.write_bytes(b'exact offline Windows installation fixture')
            for platform in ['windows-x86_64','macos-aarch64']:
                for suffix in ['media-sources','licenses']:
                    (canonical/f'M4B-Editor-0.4.0-{platform}-{suffix}.zip').write_bytes(b'sidecar fixture')
            dmg = root/'m4b-Craft-0.4.0-macos-aarch64.dmg'; dmg.write_bytes(b'legacy image fixture')
            out = root/'output'
            legacy.prepare(canonical,dmg,out,'0.4.0')
            self.assertEqual((out/'m4b-Craft-0.4.0-windows-x86_64.msi').read_bytes(),windows.read_bytes())
            self.assertEqual((out/dmg.name).read_bytes(),dmg.read_bytes())
            with self.assertRaisesRegex(ValueError,'empty'):
                legacy.prepare(canonical,dmg,out,'0.4.0')
            canonical_dmg = root/'M4B-Editor-0.4.0-macos-aarch64.dmg'
            canonical_dmg.write_bytes(b'canonical layout is incompatible')
            with self.assertRaisesRegex(ValueError,'separately built'):
                legacy.prepare(canonical,canonical_dmg,root/'bad','0.4.0')

    def test_source_pins_match_fetch_and_platform_recipes(self):
        for name, digest in SOURCES.items():
            if name.startswith('7z'): continue
            for script in ['fetch-media-sources.sh','build-ffmpeg-windows.sh','build-ffmpeg-macos.sh']:
                text = (ROOT/'scripts'/script).read_text()
                self.assertIn(name,text)
                self.assertIn(digest,text)

    def test_license_sidecar_contains_notices_and_all_media_licenses(self):
        from release_common import license_sidecar
        with tempfile.TemporaryDirectory(dir=SCRATCH) as d:
            root = Path(d); payload = root/'payload'; payload.mkdir()
            media = root/'media'; media.mkdir()
            (payload/'licenses/crate').mkdir(parents=True)
            (payload/'licenses/crate/LICENSE').write_text('crate license fixture')
            for name in ['LICENSE','THIRD-PARTY-NOTICES.txt']:
                (payload/name).write_text(name)
            for name in ['COPYING.LGPLv2.1','LICENSE.md','LICENSE.zlib','COPYING.LAME','LICENSE.7zip','README.7zip']:
                (media/name).write_text(name)
            output = root/'licenses.zip'; license_sidecar(payload,media,output)
            with zipfile.ZipFile(output) as archive:
                self.assertIsNone(archive.testzip())
                self.assertEqual(archive.read('licenses/crate/LICENSE'),b'crate license fixture')
                self.assertEqual(archive.read('media/COPYING.LAME'),b'COPYING.LAME')
                self.assertEqual(len(archive.namelist()),9)

    def test_archive_contains_real_source_bytes(self):
        with tempfile.TemporaryDirectory(dir=SCRATCH) as d:
            root = Path(d); source = root/'source'
            release.copy_source(source)
            output = root/'source.zip'; release.archive(source,output)
            with zipfile.ZipFile(output) as archive:
                self.assertIsNone(archive.testzip())
                self.assertEqual(archive.read('Cargo.lock'),(ROOT/'Cargo.lock').read_bytes())
                self.assertEqual(archive.read('scripts/ffmpeg-options.sh'),(ROOT/'scripts/ffmpeg-options.sh').read_bytes())
                self.assertNotIn('BRIEF.md',archive.namelist())

    def test_signing_refuses_wrong_external_key(self):
        with tempfile.TemporaryDirectory(dir=SCRATCH) as d:
            path = Path(d)
            key = path/'key.pem'
            subprocess.run(['openssl','genpkey','-algorithm','ED25519','-out',str(key)],check=True)
            msi = path/'M4B-Editor-0.4.0-windows-x86_64.msi'; msi.write_bytes(b'test fixture')
            output = path/'update-v2-windows-x86_64.json'
            result = subprocess.run(['python3',str(ROOT/'scripts/sign-update.py'),'--key',str(key),
                '--msi',str(msi),'--version','0.4.0','--out',str(output),'--scratch',str(path)],capture_output=True)
            self.assertNotEqual(result.returncode,0)
            self.assertIn(b'does not match pinned public key',result.stderr)
            self.assertFalse(output.exists())

    def test_media_cache_requires_exact_records_and_pinned_sources(self):
        with tempfile.TemporaryDirectory(dir=SCRATCH) as d:
            media = Path(d)
            names = ['7zz','LICENSE.7zip','README.7zip','ffmpeg','ffprobe','config.h','config_components.h','config.mak',
                     'COPYING.LGPLv2.1','LICENSE.md','LICENSE.zlib','zlib-config.txt','COPYING.LAME','lame-config.txt'] + list(SOURCES)
            for name in names: (media/name).write_bytes(b'fixture')
            def manifest():
                (media/'SHA256SUMS').write_text(''.join(sha256(media/name)+'  '+name+'\n' for name in names))
            manifest()
            with self.assertRaisesRegex(ValueError,'Unpinned source'): verify_media(media,'macos-aarch64')
            with patch.dict(SOURCES,{name:sha256(media/name) for name in SOURCES}), patch.dict(ARCHIVER_MEMBERS['macos-aarch64'],{name:sha256(media/name) for name in ARCHIVER_MEMBERS['macos-aarch64']}):
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
                for suffix in ['.'+ext for ext in extensions]+['-media-sources.zip','-licenses.zip']:
                    file = directory/f'M4B-Editor-0.4.0-{platform}{suffix}'
                    file.write_bytes(b'fixture')
                    file.with_name(file.name+'.sha256').write_text(sha256(file)+'  '+file.name+'\n')
            assets.verify(directory,'0.4.0')
            extra = directory/'nested'; extra.mkdir()
            with self.assertRaises(ValueError): assets.verify(directory,'0.4.0')
            extra.rmdir()
            next(directory.glob('*.dmg')).write_bytes(b'changed')
            with self.assertRaisesRegex(ValueError,'Checksum mismatch'): assets.verify(directory,'0.4.0')

def inspect(msi, scratch):
    tool = os.environ.get('MSIINFO','msiinfo')
    properties = subprocess.check_output([tool,'export',str(msi),'Property']).decode()
    for value in [release.UPGRADE_CODE.upper(), release.MSI_VERSION]:
        assert value.lower() in properties.lower(),value
    property_rows = dict(row.split('\t', 1) for row in properties.splitlines()[3:] if '\t' in row)
    assert property_rows.get('ProductName') == 'M4B Editor', property_rows.get('ProductName')
    files = subprocess.check_output([tool,'export',str(msi),'File']).decode()
    for name in ['m4b-editor.exe','m4b-editor-cli.exe','ffmpeg.exe','ffprobe.exe','7z.exe','7z.dll','Cargo.lock','LICENSE']:
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
        for name in ['m4b-editor.exe','m4b-editor-cli.exe','ffmpeg.exe','ffprobe.exe','7z.exe','7z.dll','source/crates/desktop/src/updater.rs','source/Cargo.lock']:
            assert name in archive.namelist(),name
        assert not any(name.lower().endswith(('.pem','.key')) for name in archive.namelist())
    sidecar = msi.with_name(msi.stem+'-media-sources.zip')
    with zipfile.ZipFile(sidecar) as archive:
        assert archive.testzip() is None
        for name, digest in SOURCES.items():
            assert hashlib.sha256(archive.read(name)).hexdigest() == digest
    with tempfile.TemporaryDirectory(dir=scratch) as d:
        subprocess.run([os.environ.get('MSIEXTRACT','msiextract'),'-C',d,str(msi)],check=True,stdout=subprocess.DEVNULL)
        for path in Path(d).rglob('m4b-editor.exe'):
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
