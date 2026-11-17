#!/usr/bin/env python3
"""Build the Windows release payload and WiX MSI; never uses old alpha artifacts.
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
from release_common import SOURCES, VERSION, verify_media, copy_source, copy_licenses, checksums, license_sidecar
MSI_VERSION = VERSION.split('-')[0]
UPGRADE_CODE = '790fce8b-e564-43b1-9701-d427fdd80c6c'
NAME = f'M4B-Editor-{VERSION}-windows-x86_64'
NS = 'http://schemas.microsoft.com/wix/2006/wi'
ET.register_namespace('', NS)

def node(parent, tag, **attrs):
    return ET.SubElement(parent, '{'+NS+'}'+tag, attrs)

def wix(payload, output):
    root = ET.Element('{'+NS+'}Wix')
    product = node(root, 'Product', Id=str(uuid.uuid5(uuid.UUID(UPGRADE_CODE), VERSION)),
                   Name='M4B Editor', Language='1033', Version=MSI_VERSION,
                   Manufacturer='M4B Editor contributors', UpgradeCode=UPGRADE_CODE)
    node(product, 'Package', InstallerVersion='500', Compressed='yes', InstallScope='perUser',
         Description='M4B Editor audiobook editor')
    node(product, 'Media', Id='1', Cabinet='application.cab', EmbedCab='yes')
    node(product, 'MajorUpgrade', DowngradeErrorMessage='A newer M4B Editor is already installed.')
    sequence = node(product, 'InstallExecuteSequence')
    node(sequence, 'RemoveExistingProducts', After='InstallInitialize')
    directory = node(product, 'Directory', Id='TARGETDIR', Name='SourceDir')
    local = node(directory, 'Directory', Id='LocalAppDataFolder')
    programs = node(local, 'Directory', Id='Programs', Name='Programs')
    install = node(programs, 'Directory', Id='INSTALLDIR', Name='m4b-Craft')
    menu = node(directory, 'Directory', Id='ProgramMenuFolder')
    menu_dir = node(menu, 'Directory', Id='ApplicationProgramsFolder', Name='M4B Editor')
    legacy_menu = node(menu, 'Directory', Id='LegacyApplicationProgramsFolder', Name='m4b-Craft')
    feature = node(product, 'Feature', Id='Complete', Title='M4B Editor', Level='1')
    menu_component = node(menu_dir, 'Component', Id='UserRegistration',
        Guid=str(uuid.uuid5(uuid.UUID(UPGRADE_CODE), 'editor-user-registration')), Win64='yes')
    node(menu_component, 'RegistryValue', Root='HKCU', Key='Software\\M4B-Editor',
        Name='Installed', Type='integer', Value='1', KeyPath='yes')
    node(menu_component, 'RemoveFolder', Id='RemoveStartMenu', On='uninstall')
    node(feature, 'ComponentRef', Id='UserRegistration')
    legacy_registration = node(legacy_menu, 'Component', Id='LegacyUserRegistration',
        Guid=str(uuid.uuid5(uuid.UUID(UPGRADE_CODE), 'user-registration')), Win64='yes')
    node(legacy_registration, 'RegistryValue', Root='HKCU', Key='Software\\m4b-Craft',
        Name='Installed', Type='integer', Value='1', KeyPath='yes')
    node(legacy_registration, 'RemoveFolder', Id='RemoveLegacyStartMenu', On='uninstall')
    node(feature, 'ComponentRef', Id='LegacyUserRegistration')
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
        if relative.as_posix() == 'm4b-editor.exe':
            node(installed, 'Shortcut', Id='StartMenuShortcut', Directory='ApplicationProgramsFolder',
                 Name='M4B Editor', WorkingDirectory='INSTALLDIR', Advertise='yes')
        if relative.as_posix() == 'm4b-Craft.exe':
            node(installed, 'Shortcut', Id='LegacyStartMenuShortcut', Directory='LegacyApplicationProgramsFolder',
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
    parser.add_argument('--out', type=Path, default=ROOT/f'dist/release-{VERSION}')
    parser.add_argument('--scratch', type=Path, default=Path(os.environ.get('TMPDIR','/root/.hermes/cache/scratch')))
    args = parser.parse_args()
    version = subprocess.check_output(['cargo','metadata','--no-deps','--format-version=1'], cwd=ROOT)
    assert all(p['version'] == VERSION for p in json.loads(version)['packages'])
    binaries = ROOT/'target/x86_64-pc-windows-gnu/release'
    # Always build first. A failed build must not silently package stale alpha executables.
    subprocess.run(['cargo','build','--locked','--release','--target','x86_64-pc-windows-gnu',
                    '-p','m4b-editor-desktop','-p','m4b-editor-cli'], cwd=ROOT, check=True)
    args.out.mkdir(parents=True, exist_ok=True)
    output = args.out.resolve()
    args.scratch.mkdir(parents=True, exist_ok=True)
    with tempfile.TemporaryDirectory(prefix='m4b-release-', dir=args.scratch) as temporary:
        payload = Path(temporary)/'payload'
        payload.mkdir()
        for source, target in [('m4b-editor.exe','m4b-editor.exe'),('m4b-editor-cli.exe','m4b-editor-cli.exe')]:
            shutil.copy2(binaries/source, payload/target)
        # Exact old updater relaunch path, same bytes as canonical binary.
        shutil.copy2(payload/'m4b-editor.exe', payload/'m4b-Craft.exe')
        media = ROOT/'vendor/windows-media'
        verify_media(media, 'windows-x86_64')
        for name in ['ffmpeg.exe','ffprobe.exe','7z.exe','7z.dll']:
            shutil.copy2(media/name, payload/name)
        for executable in list(payload.glob('*.exe')) + list(payload.glob('*.dll')): verify_pe(executable)
        shutil.copy2(ROOT/'LICENSE', payload/'LICENSE')
        for name in ['Lancer-M4B-Editor.cmd','LISEZ-MOI-WINDOWS.txt']:
            shutil.copy2(ROOT/'packaging/windows'/name, payload/name)
        source = payload/'source'
        copy_source(source)
        third = source/'third-party/ffmpeg'
        third.mkdir(parents=True)
        for file in media.iterdir():
            if file.is_file() and file.name not in set(SOURCES)|{'ffmpeg.exe','ffprobe.exe','7z.exe','7z.dll'} and not file.name.endswith(('.tar.xz','.tar.gz')):
                shutil.copy2(file, third/file.name)
        # Upstream source sidecar ships alongside MSI; do not omit it from releases.
        upstream = Path(temporary)/'media-source'
        upstream.mkdir()
        for name in SOURCES:
            shutil.copy2(media/name, upstream/name)
        archive(upstream, output/f'{NAME}-media-sources.zip')
        (payload/'MEDIA-SOURCES.txt').write_text(f'Corresponding FFmpeg/zlib/LAME/7-Zip source and official archives: {NAME}-media-sources.zip, supplied alongside this MSI. Build recipe and configuration: source/.\n')
        copy_licenses(payload)
        license_sidecar(payload,media,output/f'{NAME}-licenses.zip')
        retained = output/'payload'
        if retained.exists(): shutil.rmtree(retained)
        shutil.copytree(payload, retained)
        wxs = output/'M4B-Editor.wxs'
        wix(retained,wxs)
        subprocess.run([args.wixl,'-a','x64','-o',str(output/f'{NAME}.msi'),str(wxs)], check=True)
        archive(payload,output/f'{NAME}.zip')
    # Retained payload and generated WiX source allow MSI inspection/rebuilding.
    checksums(output)
    print(output)

if __name__ == '__main__': main()
