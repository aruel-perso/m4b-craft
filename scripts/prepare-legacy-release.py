#!/usr/bin/env python3
"""Prepare local release-only old-slug assets. Never uploads or signs implicitly."""
import argparse
from pathlib import Path
import shutil
from release_common import VERSION, checksums, valid_version

def prepare(canonical, legacy_dmg, output, version):
    if not valid_version(version): raise ValueError('Invalid semver')
    if output.is_symlink(): raise ValueError('Symlinked output')
    if output.exists() and any(output.iterdir()):
        raise ValueError('Compatibility output must be empty')
    output.mkdir(parents=True, exist_ok=True)
    # Windows installation bytes are identical, including canonical and legacy exe.
    windows = f'M4B-Editor-{version}-windows-x86_64.msi'
    mac = f'm4b-Craft-{version}-macos-aarch64.dmg'
    if legacy_dmg.name != mac or legacy_dmg.is_symlink():
        raise ValueError('Expected separately built legacy DMG, never rename canonical DMG')
    for source, name in [(canonical/windows, windows.replace('M4B-Editor','m4b-Craft')),
                         (legacy_dmg, mac)]:
        if source.is_symlink() or not source.is_file() or not source.stat().st_size:
            raise ValueError(f'Invalid installer: {source}')
        shutil.copy2(source, output/name)
    # Same corresponding sources cover both installation payloads.
    for platform in ['windows-x86_64','macos-aarch64']:
        for suffix in ['media-sources','licenses']:
            name = f'M4B-Editor-{version}-{platform}-{suffix}.zip'
            shutil.copy2(canonical/name, output/name.replace('M4B-Editor','m4b-Craft'))
    checksums(output)

if __name__ == '__main__':
    p = argparse.ArgumentParser(description=__doc__)
    p.add_argument('--canonical', type=Path, required=True)
    p.add_argument('--legacy-dmg', type=Path, required=True)
    p.add_argument('--out', type=Path, required=True)
    p.add_argument('--version', default=VERSION)
    a = p.parse_args(); prepare(a.canonical,a.legacy_dmg,a.out,a.version)
