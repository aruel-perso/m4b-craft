"""Shared release contract; no network, build or platform commands at import time."""
import hashlib
import re
import shutil
import subprocess
import tomllib
from pathlib import Path

ROOT = Path(__file__).resolve().parents[1]
VERSION = tomllib.loads((ROOT / 'Cargo.toml').read_text())['workspace']['package']['version']
SEMVER = r'(0|[1-9][0-9]*)\.(0|[1-9][0-9]*)\.(0|[1-9][0-9]*)(?:-([0-9A-Za-z-]+(?:\.[0-9A-Za-z-]+)*))?'
def valid_version(version):
    match = re.fullmatch(SEMVER, version)
    return bool(match and not (match[4] and any(p.isdigit() and len(p)>1 and p[0]=='0' for p in match[4].split('.'))))
if not valid_version(VERSION):
    raise ValueError('Noncanonical workspace version')
SOURCES = {
    'ffmpeg-8.0.1.tar.xz': '05ee0b03119b45c0bdb4df654b96802e909e0a752f72e4fe3794f487229e5a41',
    'zlib-1.3.1.tar.gz': '9a93b2b7dfdac77ceba5a558a580e74667dd6fede4585b91eefb60f03b72df23',
}
def sha256(path):
    with Path(path).open('rb') as file:
        return hashlib.file_digest(file, 'sha256').hexdigest()
def verify_media(media, platform):
    executables = ['ffmpeg.exe','ffprobe.exe'] if platform == 'windows-x86_64' else ['ffmpeg','ffprobe']
    expected = set(executables) | set(SOURCES) | {'config.h','config_components.h','config.mak','COPYING.LGPLv2.1','LICENSE.md','LICENSE.zlib','zlib-config.txt'}
    if {p.name for p in media.iterdir()} != expected | {'SHA256SUMS'}:
        raise ValueError('Unexpected files in media cache')
    if (media/'SHA256SUMS').is_symlink():
        raise ValueError('Symlinked media checksum manifest')
    records = {}
    for line in (media/'SHA256SUMS').read_text().splitlines():
        digest, name = line.split(maxsplit=1)
        name = name.lstrip('*')
        if Path(name).name != name or name in records or not re.fullmatch('[a-f0-9]{64}', digest):
            raise ValueError('Invalid media checksum record')
        records[name] = digest
    if set(records) != expected:
        raise ValueError(f'Unexpected media cache contents: {set(records) ^ expected}')
    for name, digest in records.items():
        if (media/name).is_symlink() or sha256(media/name) != digest:
            raise ValueError(f'Media checksum mismatch: {name}')
    for name, digest in SOURCES.items():
        if records[name] != digest:
            raise ValueError(f'Unpinned source: {name}')
def copy_source(destination):
    destination.mkdir(parents=True, exist_ok=True)
    for name in ['crates','.cargo','.github','scripts','packaging']:
        for entry in (ROOT/name).rglob('*'):
            if entry.is_symlink() or entry.suffix.lower() in ('.pem','.key'):
                raise ValueError(f'Refusing private key or symlink: {entry}')
        shutil.copytree(ROOT/name, destination/name, ignore=shutil.ignore_patterns('__pycache__','*.pyc'))
    for name in ['Cargo.toml','Cargo.lock','.gitignore','.gitattributes','LICENSE','README.md','README.en.md','BRIEF.md','ROADMAP.md','VALIDATION.md','RELEASE.md']:
        shutil.copy2(ROOT/name, destination/name)
def copy_licenses(destination):
    import json
    metadata = json.loads(subprocess.check_output(['cargo','metadata','--locked','--offline','--format-version=1'],cwd=ROOT))
    notices = []
    for package in metadata['packages']:
        if not package.get('source'): continue
        notices.append(f"{package['name']} {package['version']}: {package.get('license')}")
        base = Path(package['manifest_path']).parent
        candidates = list(base.iterdir())
        license_file = base/package['license_file'] if package.get('license_file') else None
        if license_file is not None: candidates.append(license_file)
        for file in candidates:
            if file.is_file() and (file.name.upper().startswith(('LICENSE','LICENCE','COPYING','NOTICE','COPYRIGHT')) or file == license_file):
                dest = destination/'licenses'/f"{package['name']}-{package['version']}"/file.name
                dest.parent.mkdir(parents=True,exist_ok=True)
                shutil.copy2(file,dest)
    (destination/'THIRD-PARTY-NOTICES.txt').write_text('\n'.join(sorted(notices))+'\n',encoding='utf-8')
def checksums(output):
    for file in sorted(output.iterdir()):
        if file.suffix in ('.msi','.dmg','.zip'):
            file.with_name(file.name+'.sha256').write_text(sha256(file)+'  '+file.name+'\n')
if __name__ == '__main__':
    import argparse
    parser = argparse.ArgumentParser()
    parser.add_argument('--media',type=Path)
    parser.add_argument('--platform',choices=['windows-x86_64','macos-aarch64'])
    args = parser.parse_args()
    if args.media:
        if not args.platform: parser.error('--platform required with --media')
        verify_media(args.media,args.platform)
    else: print(VERSION)
