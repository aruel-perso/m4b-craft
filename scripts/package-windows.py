#!/usr/bin/env python3
"""Package the already-built Windows GNU alpha, with corresponding source.
Run after cargo build -p m4b-desktop -p m4b-cli --target x86_64-pc-windows-gnu.
No downloads, uploads or builds are performed by this script.
"""
import hashlib
import json
import os
from pathlib import Path
import shutil
import subprocess
import tempfile
import zipfile

ROOT = Path(__file__).resolve().parents[1]
DIST = ROOT / 'dist'
NAME = 'm4b-Craft-0.1.1-alpha-windows-x86_64'
SOURCE_DIRS = ['crates', '.cargo', '.github', 'scripts', 'packaging']
SOURCE_FILES = ['Cargo.toml', 'Cargo.lock', '.gitignore', 'LICENSE', 'README.md',
                'BRIEF.md', 'ROADMAP.md', 'VALIDATION.md', 'ALPHA-0.1.1.md']

def main():
    DIST.mkdir(exist_ok=True)
    cargo = shutil.which('cargo') or str(Path.home() / '.cargo/bin/cargo')
    metadata = json.loads(subprocess.check_output(
        [cargo, 'metadata', '--format-version=1', '--locked', '--offline'], cwd=ROOT))
    with tempfile.TemporaryDirectory(prefix='m4b-package-', dir=DIST) as temporary:
        folder = Path(temporary) / NAME
        folder.mkdir()
        for source, target in [('m4b-desktop.exe', 'm4b-Craft.exe'),
                               ('m4b-cli.exe', 'm4b-cli.exe')]:
            binary = ROOT / 'target/x86_64-pc-windows-gnu/debug' / source
            if not binary.is_file():
                raise SystemExit(f'Missing binary: {binary}')
            shutil.copy2(binary, folder / target)
            strip = shutil.which('x86_64-w64-mingw32-strip')
            if strip:
                subprocess.run([strip, str(folder / target)], check=True)
        for path in (ROOT / 'packaging/windows').iterdir():
            if path.is_file():
                shutil.copy2(path, folder / path.name)
        shutil.copy2(ROOT / 'LICENSE', folder / 'LICENSE')
        media = ROOT / 'vendor/windows-media'
        for name in ['ffmpeg.exe', 'ffprobe.exe']:
            if not (media / name).is_file():
                raise SystemExit(f'Missing bundled tool: {media / name}')
            shutil.copy2(media / name, folder / name)
        media_source = folder / 'source/third-party/ffmpeg'
        media_source.mkdir(parents=True)
        for name in ['ffmpeg-8.0.1.tar.xz', 'COPYING.LGPLv2.1', 'LICENSE.md',
                     'config.h', 'config_components.h', 'config.mak', 'SHA256SUMS',
                     'zlib-1.3.1.tar.gz', 'LICENSE.zlib']:
            shutil.copy2(media / name, media_source / name)
        # Separate large, unmodified upstream sources to fit messaging attachment
        # limits. This sidecar is delivered alongside the runnable app; end users
        # do not need to extract it to run the application.
        sources_output = DIST / f'{NAME}-media-sources.zip'
        with zipfile.ZipFile(sources_output, 'w', compression=zipfile.ZIP_DEFLATED,
                             strict_timestamps=False) as upstream:
            for name in ['ffmpeg-8.0.1.tar.xz', 'zlib-1.3.1.tar.gz']:
                upstream.write(media_source / name, f'{NAME}/source/third-party/ffmpeg/{name}')
                (media_source / name).unlink()
        (folder / 'MEDIA-SOURCES.txt').write_text(
            f'Complete corresponding FFmpeg/zlib source is supplied alongside this package in {sources_output.name}.\n'
            'You do NOT need to install or extract that source archive to run the app.\n'
            'Exact build configuration, licenses and rebuild recipe are included in source/.\n', encoding='utf-8')
        for name in SOURCE_FILES:
            destination = folder / 'source' / name
            destination.parent.mkdir(parents=True, exist_ok=True)
            shutil.copy2(ROOT / name, destination)
        for name in SOURCE_DIRS:
            shutil.copytree(ROOT / name, folder / 'source' / name,
                            ignore=shutil.ignore_patterns('__pycache__', '*.pyc'))
        # Preserve notices shipped by the exact registry dependencies in Cargo.lock.
        notices = ['Rust dependency licenses/notices from the locked Cargo packages.',
                   'Bundled FFmpeg 8.0.1 is built without external codec libraries under LGPL-2.1-or-later.',
                   'FFmpeg/zlib source is in the accompanying media-sources ZIP; exact configuration/licenses are in source/third-party/ffmpeg/.',
                   'Rebuild recipe: source/scripts/build-ffmpeg-windows.sh. Each dependency retains its license.', '']
        for package in sorted(metadata['packages'], key=lambda p: (p['name'], p['version'])):
            if not package.get('source'):
                continue
            base = Path(package['manifest_path']).parent
            notices.append(f"{package['name']} {package['version']}: {package.get('license', 'unspecified')}")
            candidates = [p for p in base.iterdir() if p.is_file() and
                          p.name.upper().startswith(('LICENSE', 'LICENCE', 'COPYING', 'NOTICE', 'AUTHORS', 'COPYRIGHT'))]
            if package.get('license_file'):
                candidates.append(base / package['license_file'])
            for path in set(candidates):
                if path.is_file():
                    destination = folder / 'licenses' / f"{package['name']}-{package['version']}" / path.name
                    destination.parent.mkdir(parents=True, exist_ok=True)
                    shutil.copy2(path, destination)
        (folder / 'THIRD-PARTY-NOTICES.txt').write_text('\n'.join(notices) + '\n', encoding='utf-8')
        output = DIST / f'{NAME}.zip'
        with zipfile.ZipFile(output, 'w', compression=zipfile.ZIP_DEFLATED, compresslevel=9, strict_timestamps=False) as archive:
            for path in sorted(folder.rglob('*')):
                if path.is_file():
                    archive.write(path, path.relative_to(folder.parent))
    with zipfile.ZipFile(output) as archive:
        assert archive.testzip() is None, 'ZIP CRC verification failed'
        names = archive.namelist()
        for required in ['m4b-Craft.exe', 'm4b-cli.exe', 'Lancer-m4b-Craft.cmd',
                         'LISEZ-MOI-WINDOWS.txt', 'source/Cargo.lock', 'source/crates/core/src/lib.rs']:
            assert f'{NAME}/{required}' in names, required
        for required in ['ffmpeg.exe', 'ffprobe.exe', 'MEDIA-SOURCES.txt']:
            assert f'{NAME}/{required}' in names, required
    with zipfile.ZipFile(sources_output) as upstream:
        assert upstream.testzip() is None
        assert len(upstream.namelist()) == 2
    sources_digest = hashlib.sha256(sources_output.read_bytes()).hexdigest()
    sources_output.with_suffix('.zip.sha256').write_text(f'{sources_digest}  {sources_output.name}\n', encoding='ascii')
    digest = hashlib.sha256(output.read_bytes()).hexdigest()
    output.with_suffix('.zip.sha256').write_text(f'{digest}  {output.name}\n', encoding='ascii')
    print(f'PASS ZIP CRC and required content: {len(names)} entries')
    print(f'Artifact: {output}')
    print(f'Bytes: {output.stat().st_size}')
    print(f'SHA256: {digest}')

if __name__ == '__main__':
    main()
