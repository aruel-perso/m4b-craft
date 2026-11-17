#!/usr/bin/env python3
"""Extract pinned official 7-Zip assets as data; never execute the installer."""
import argparse
import shutil
import subprocess
import tarfile
import tempfile
from pathlib import Path
from release_common import ARCHIVER_SOURCES, ARCHIVER_MEMBERS, archiver_files, sha256


def extract(archives, destination, platform, thin=True):
    for name, digest in ARCHIVER_SOURCES.items():
        path = archives/name
        if path.is_symlink() or sha256(path) != digest:
            raise ValueError(f'Unpinned 7-Zip archive: {name}')
    destination.mkdir(parents=True, exist_ok=True)
    if platform == 'windows-x86_64':
        archiver = shutil.which('7z') or shutil.which('7zz')
        if not archiver: raise ValueError('Build prerequisite missing: 7z or 7zz')
        for member in ['7z.exe', '7z.dll', 'License.txt', 'readme.txt']:
            data = subprocess.check_output([archiver, 'e', '-so', str(archives/'7z2606-x64.exe'), member])
            if not data: raise ValueError(f'Empty official member: {member}')
            target = {'License.txt':'LICENSE.7zip','readme.txt':'README.7zip'}.get(member, member)
            (destination/target).write_bytes(data)
    else:
        with tarfile.open(archives/'7z2606-mac.tar.xz') as archive:
            for member, target in [('7zz','7zz'),('License.txt','LICENSE.7zip'),('readme.txt','README.7zip')]:
                info = archive.getmember(member)
                if not info.isfile(): raise ValueError(f'Non-regular member: {member}')
                (destination/target).write_bytes(archive.extractfile(info).read())
        if thin:
            binary = destination/'7zz'
            archs = subprocess.check_output(['/usr/bin/lipo','-archs',str(binary)],text=True).split()
            if 'arm64' not in archs: raise ValueError('Official 7zz lacks ARM64')
            if archs != ['arm64']:
                temporary = destination/'7zz.arm64'
                subprocess.run(['/usr/bin/lipo',str(binary),'-thin','arm64','-output',str(temporary)],check=True)
                temporary.replace(binary)
            subprocess.run(['python3',str(Path(__file__).with_name('verify-macos.py')),str(binary)],check=True)
    for name, digest in ARCHIVER_MEMBERS[platform].items():
        if sha256(destination/name) != digest: raise ValueError(f'Unexpected official member: {name}')
    for name in archiver_files(platform): (destination/name).chmod(0o755)
    for name in ARCHIVER_SOURCES:
        if (archives/name).resolve() != (destination/name).resolve(): shutil.copy2(archives/name,destination/name)


def main():
    p = argparse.ArgumentParser(description=__doc__)
    p.add_argument('--archives', type=Path, required=True)
    p.add_argument('--media', type=Path, required=True)
    p.add_argument('--platform', choices=['windows-x86_64','macos-aarch64'], required=True)
    a = p.parse_args()
    with tempfile.TemporaryDirectory(prefix='m4b-7zip-') as temporary:
        stage = Path(temporary)
        extract(a.archives,stage,a.platform)
        for file in stage.iterdir(): shutil.copy2(file,a.media/file.name)
    files = sorted(f for f in a.media.iterdir() if f.name != 'SHA256SUMS')
    (a.media/'SHA256SUMS').write_text(''.join(f'{sha256(f)}  {f.name}\n' for f in files))

if __name__ == '__main__': main()
