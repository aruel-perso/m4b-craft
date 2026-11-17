#!/usr/bin/env python3
"""Optional real-archive checks; no media/Rust build or installer execution."""
import argparse
import importlib.util
import struct
import shutil
import subprocess
import tempfile
from pathlib import Path
from release_common import SOURCES, sha256


def main():
    p = argparse.ArgumentParser()
    p.add_argument('--archives',type=Path,required=True)
    a = p.parse_args()
    spec = importlib.util.spec_from_file_location('bundle',Path(__file__).with_name('bundle-archiver.py'))
    bundle = importlib.util.module_from_spec(spec); spec.loader.exec_module(bundle)
    lame = a.archives/'lame-3.100.tar.gz'
    assert sha256(lame) == SOURCES[lame.name]
    subprocess.run(['bash','-c','source "$1"; fetch_verified "$2" "$3" https://invalid.invalid/no-network',
                    'cache-check',str(Path(__file__).with_name('fetch-media-sources.sh')),str(lame),SOURCES[lame.name]],check=True)
    with tempfile.TemporaryDirectory(prefix='m4b-official-') as d:
        root = Path(d)
        bundle.extract(a.archives,root/'windows','windows-x86_64')
        release_spec = importlib.util.spec_from_file_location('release',Path(__file__).with_name('package-release.py'))
        release = importlib.util.module_from_spec(release_spec); release_spec.loader.exec_module(release)
        for name in ['7z.exe','7z.dll']:
            path = root/'windows'/name
            data = path.read_bytes(); assert data[:2] == b'MZ'
            offset = struct.unpack_from('<I',data,60)[0]
            assert data[offset:offset+6] == b'PE\0\0\x64\x86'
            release.verify_pe(path)
        bundle.extract(a.archives,root/'mac','macos-aarch64',thin=False)
        data = (root/'mac/7zz').read_bytes()
        magic,count = struct.unpack_from('>II',data)
        if magic == 0xcafebabe:
            cpus = [struct.unpack_from('>I',data,8+20*i)[0] for i in range(count)]
        else:
            assert data[:4] == bytes.fromhex('cffaedfe')
            cpus = [struct.unpack_from('<I',data,4)[0]]
        assert 0x100000c in cpus, cpus
        import tarfile
        with tarfile.open(a.archives/'7z2606-src.tar.xz') as source:
            assert source.getmember('DOC/License.txt').isfile()
            assert any(n.endswith('/Console/Main.cpp') for n in source.getnames())
        damaged = root/'damaged'; damaged.mkdir()
        for name in bundle.ARCHIVER_SOURCES: shutil.copy2(a.archives/name,damaged/name)
        (damaged/'7z2606-x64.exe').write_bytes(b'corrupt installer')
        try:
            bundle.extract(damaged,root/'rejected','windows-x86_64')
        except ValueError as error:
            assert 'Unpinned' in str(error)
        else: raise AssertionError('Corrupt official archive accepted')
        assert not (root/'rejected').exists()
        assert b'GNU LGPL' in (root/'windows/LICENSE.7zip').read_bytes()
    print('PASS pinned LAME/cache reuse; official Windows PE extraction; Mac native ARM64 executable; matching source/license contents')

if __name__ == '__main__': main()
