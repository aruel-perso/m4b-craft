#!/usr/bin/env python3
"""Build the 0.2 release payload and WiX MSI; never uses old alpha artifacts.
Requires freshly built Windows GNU release binaries, wixl and locked Cargo sources.
"""
import argparse
import hashlib
import json
import os
import re
from pathlib import Path
import shutil
import subprocess
import tempfile
import uuid
import zipfile
import xml.etree.ElementTree as ET

ROOT = Path(__file__).resolve().parents[1]
VERSION = '0.2.0-alpha.1'
MSI_VERSION = '0.2.0'
UPGRADE_CODE = '790fce8b-e564-43b1-9701-d427fdd80c6c'
NAME = f'm4b-Craft-{VERSION}-windows-x86_64'
NS = 'http://schemas.microsoft.com/wix/2006/wi'
ET.register_namespace('', NS)

def node(parent, tag, **attrs):
    return ET.SubElement(parent, '{'+NS+'}'+tag, attrs)

def wix(payload, output):
    root = ET.Element('{'+NS+'}Wix')
    product = node(root, 'Product', Id=str(uuid.uuid5(uuid.UUID(UPGRADE_CODE), VERSION)),
                   Name='m4b-Craft', Language='1033', Version=MSI_VERSION,
                   Manufacturer='m4b-Craft contributors', UpgradeCode=UPGRADE_CODE)
    node(product, 'Package', InstallerVersion='500', Compressed='yes', InstallScope='perUser',
         Description='m4b-Craft audiobook editor')
    node(product, 'Media', Id='1', Cabinet='application.cab', EmbedCab='yes')
    node(product, 'MajorUpgrade', DowngradeErrorMessage='A newer m4b-Craft is already installed.')
    sequence = node(product, 'InstallExecuteSequence')
    node(sequence, 'RemoveExistingProducts', After='InstallInitialize')
    directory = node(product, 'Directory', Id='TARGETDIR', Name='SourceDir')
    local = node(directory, 'Directory', Id='LocalAppDataFolder')
    programs = node(local, 'Directory', Id='Programs', Name='Programs')
    install = node(programs, 'Directory', Id='INSTALLDIR', Name='m4b-Craft')
    menu = node(directory, 'Directory', Id='ProgramMenuFolder')
    menu_dir = node(menu, 'Directory', Id='ApplicationProgramsFolder', Name='m4b-Craft')
    feature = node(product, 'Feature', Id='Complete', Title='m4b-Craft', Level='1')
    menu_component = node(menu_dir, 'Component', Id='UserRegistration',
        Guid=str(uuid.uuid5(uuid.UUID(UPGRADE_CODE), 'user-registration')), Win64='yes')
    node(menu_component, 'RegistryValue', Root='HKCU', Key='Software\\m4b-Craft',
        Name='Installed', Type='integer', Value='1', KeyPath='yes')
    node(menu_component, 'RemoveFolder', Id='RemoveStartMenu', On='uninstall')
    node(feature, 'ComponentRef', Id='UserRegistration')
    directories = {Path('.'): install}
    for index, file in enumerate(sorted(payload.rglob('*'))):
        if not file.is_file():
            continue
        relative = file.relative_to(payload)
        path = Path('.')
        for part in relative.parent.parts:
            parent = path
            path = path / part
            if path not in directories:
                directories[path] = node(directories[parent], 'Directory',
                    Id='D'+hashlib.sha256(str(path).encode()).hexdigest()[:24], Name=part)
        component_id = f'C{index}'
        component = node(directories[relative.parent], 'Component', Id=component_id,
                         Guid=str(uuid.uuid5(uuid.UUID(UPGRADE_CODE), relative.as_posix())), Win64='yes')
        installed = node(component, 'File', Id=f'F{index}', Source=str(file), Name=file.name, KeyPath='yes')
        if relative.as_posix() == 'm4b-Craft.exe':
            node(installed, 'Shortcut', Id='StartMenuShortcut', Directory='ApplicationProgramsFolder',
                 Name='m4b-Craft', WorkingDirectory='INSTALLDIR', Advertise='yes')
        node(feature, 'ComponentRef', Id=component_id)
    ET.indent(root)
    ET.ElementTree(root).write(output, encoding='utf-8', xml_declaration=True)

def verify_pe(file):
    system = set(('advapi32 avrt bcrypt bcryptprimitives crypt32 d3d11 d3d12 d3dcompiler_47 '
        'dcomp dwmapi dxgi gdi32 imm32 iphlpapi kernel32 msvcrt ntdll ole32 oleaut32 '
        'opengl32 propsys psapi rpcrt4 secur32 setupapi shell32 shlwapi user32 userenv '
        'uuid version winhttp winmm ws2_32 wtsapi32 ucrtbase uiautomationcore uxtheme '
        'comdlg32 comctl32 msimg32 wintrust normaliz dnsapi netapi32').split())
    imports = subprocess.check_output(['x86_64-w64-mingw32-objdump','-p',str(file)],text=True)
    names = re.findall(r'DLL Name: (\S+)',imports)
    if not names: raise SystemExit(f'No PE imports found: {file}')
    for name in names:
        lower = name.lower()
        if not (lower.startswith(('api-ms-win-','ext-ms-win-')) or lower.removesuffix('.dll') in system):
            raise SystemExit(f'Unbundled non-system DLL in {file.name}: {name}')


def archive(directory, output):
    with zipfile.ZipFile(output, 'w', zipfile.ZIP_DEFLATED, strict_timestamps=False) as z:
        for file in sorted(directory.rglob('*')):
            if file.is_file(): z.write(file, file.relative_to(directory))

def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--wixl', default='wixl')
    parser.add_argument('--out', type=Path, default=ROOT/'dist/release-0.2.0-alpha.1')
    parser.add_argument('--scratch', type=Path, default=Path(os.environ.get('TMPDIR','/root/.hermes/cache/scratch')))
    args = parser.parse_args()
    version = subprocess.check_output(['cargo','metadata','--no-deps','--format-version=1'], cwd=ROOT)
    assert all(p['version'] == VERSION for p in json.loads(version)['packages'])
    binaries = ROOT/'target/x86_64-pc-windows-gnu/release'
    # Always build first. A failed build must not silently package stale alpha executables.
    subprocess.run(['cargo','build','--locked','--release','--target','x86_64-pc-windows-gnu',
                    '-p','m4b-desktop','-p','m4b-cli'], cwd=ROOT, check=True)
    args.out.mkdir(parents=True, exist_ok=True)
    output = args.out.resolve()
    args.scratch.mkdir(parents=True, exist_ok=True)
    with tempfile.TemporaryDirectory(prefix='m4b-release-', dir=args.scratch) as temporary:
        payload = Path(temporary)/'payload'
        payload.mkdir()
        for source, target in [('m4b-desktop.exe','m4b-Craft.exe'),('m4b-cli.exe','m4b-cli.exe')]:
            shutil.copy2(binaries/source, payload/target)
        media = ROOT/'vendor/windows-media'
        for line in (media/'SHA256SUMS').read_text().splitlines():
            digest, name = line.split(maxsplit=1)
            file = media/Path(name.strip()).name
            if hashlib.sha256(file.read_bytes()).hexdigest() != digest:
                raise SystemExit(f'Media checksum mismatch: {file.name}')
        if hashlib.sha256((media/'zlib-1.3.1.tar.gz').read_bytes()).hexdigest() != '9a93b2b7dfdac77ceba5a558a580e74667dd6fede4585b91eefb60f03b72df23':
            raise SystemExit('zlib source checksum mismatch')
        for name in ['ffmpeg.exe','ffprobe.exe']:
            shutil.copy2(media/name, payload/name)
        for executable in payload.glob('*.exe'): verify_pe(executable)
        shutil.copy2(ROOT/'LICENSE', payload/'LICENSE')
        source = payload/'source'
        source.mkdir()
        for name in ['crates','.cargo','.github','scripts','packaging']:
            for entry in (ROOT/name).rglob('*'):
                if entry.is_symlink() or entry.suffix.lower() in ('.pem','.key'):
                    raise SystemExit(f'Refusing private key or symlink in release source: {entry}')
            shutil.copytree(ROOT/name, source/name, ignore=shutil.ignore_patterns('__pycache__','*.pyc'))
        for name in ['Cargo.toml','Cargo.lock','.gitignore','.gitattributes','LICENSE','README.md','BRIEF.md','ROADMAP.md','VALIDATION.md','RELEASE.md']:
            shutil.copy2(ROOT/name, source/name)
        third = source/'third-party/ffmpeg'
        third.mkdir(parents=True)
        for file in media.iterdir():
            if file.is_file() and file.suffix != '.exe' and not file.name.endswith(('.tar.xz','.tar.gz')):
                shutil.copy2(file, third/file.name)
        # Upstream source sidecar ships alongside MSI; do not omit it from releases.
        upstream = Path(temporary)/'media-source'
        upstream.mkdir()
        for name in ['ffmpeg-8.0.1.tar.xz','zlib-1.3.1.tar.gz']:
            shutil.copy2(media/name, upstream/name)
        archive(upstream, output/f'{NAME}-media-sources.zip')
        (payload/'MEDIA-SOURCES.txt').write_text(f'Corresponding FFmpeg/zlib source: {NAME}-media-sources.zip, supplied alongside this MSI. Build recipe and configuration: source/.\n')
        metadata = json.loads(subprocess.check_output(['cargo','metadata','--locked','--offline','--format-version=1'],cwd=ROOT))
        notices = []
        for package in metadata['packages']:
            if not package.get('source'): continue
            notices.append(f"{package['name']} {package['version']}: {package.get('license')}")
            base = Path(package['manifest_path']).parent
            for file in base.iterdir():
                if file.is_file() and file.name.upper().startswith(('LICENSE','LICENCE','COPYING','NOTICE','COPYRIGHT')):
                    dest = payload/'licenses'/f"{package['name']}-{package['version']}"/file.name
                    dest.parent.mkdir(parents=True,exist_ok=True)
                    shutil.copy2(file,dest)
        (payload/'THIRD-PARTY-NOTICES.txt').write_text('\n'.join(sorted(notices))+'\n')
        retained = output/'payload'
        if retained.exists(): shutil.rmtree(retained)
        shutil.copytree(payload, retained)
        wxs = output/'m4b-Craft.wxs'
        wix(retained,wxs)
        subprocess.run([args.wixl,'-a','x64','-o',str(output/f'{NAME}.msi'),str(wxs)], check=True)
        archive(payload,output/f'{NAME}.zip')
    # Retained payload and generated WiX source allow MSI inspection/rebuilding.
    for file in sorted(output.iterdir()):
        if file.suffix in ('.msi','.zip'):
            file.with_name(file.name+'.sha256').write_text(hashlib.sha256(file.read_bytes()).hexdigest()+'  '+file.name+'\n')
    print(output)

if __name__ == '__main__': main()
