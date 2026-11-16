#!/usr/bin/env python3
"""Enforce a flat, exact two-platform artifact set and every sidecar checksum."""
import argparse
from pathlib import Path
from release_common import VERSION, sha256, valid_version

def verify(directory, version):
    if not valid_version(version): raise ValueError('Invalid semver')
    names = set()
    for platform, extensions in [('windows-x86_64',['msi','zip']),('macos-aarch64',['dmg'])]:
        base = f'm4b-Craft-{version}-{platform}'
        names |= {base+'.'+ext for ext in extensions}
        names.add(base+'-media-sources.zip')
    expected = names | {name+'.sha256' for name in names}
    actual = {p.name for p in directory.iterdir()}
    if actual != expected: raise ValueError(f'Asset set mismatch: missing={expected-actual}, extra={actual-expected}')
    for name in expected:
        file = directory/name
        if file.is_symlink() or not file.is_file(): raise ValueError(f'Non-flat asset: {file}')
    for name in names:
        file = directory/name
        record = (directory/(name+'.sha256')).read_text()
        if record != sha256(file)+'  '+name+'\n': raise ValueError(f'Checksum mismatch: {name}')
        if file.stat().st_size == 0: raise ValueError(f'Empty asset: {name}')
    print('PASS exact flat assets and SHA-256 sidecars')
if __name__ == '__main__':
    p = argparse.ArgumentParser(); p.add_argument('directory',type=Path); p.add_argument('--version',default=VERSION)
    a = p.parse_args(); verify(a.directory,a.version)
