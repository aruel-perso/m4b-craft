#!/usr/bin/env python3
"""Run cross-built Rust tests under Wine with the bundled Windows media tools."""
import argparse
import json
import os
from pathlib import Path
import shutil
import subprocess
import tempfile

parser = argparse.ArgumentParser()
parser.add_argument('cargo_json', type=Path)
args = parser.parse_args()
root = Path(__file__).resolve().parents[1]
media = root / 'vendor/windows-media'
scratch = Path(os.environ.get('TMPDIR', str(Path.home() / '.hermes/cache/scratch')))
env = dict(os.environ, WINEDEBUG='-all', WINEPREFIX=str(scratch / 'm4b-bundle-verification-wine'))
executables = []
for line in args.cargo_json.read_text().splitlines():
    message = json.loads(line)
    if message.get('reason') == 'compiler-artifact' and message.get('profile', {}).get('test') and message.get('executable'):
        executables.append(Path(message['executable']))
assert executables, 'No test artifacts found'
with tempfile.TemporaryDirectory(prefix='m4b-windows-tests-', dir=scratch) as temp:
    temp = Path(temp)
    for name in ['ffmpeg.exe', 'ffprobe.exe']:
        shutil.copy2(media / name, temp / name)
    env['WINEPATH'] = 'Z:' + str(temp).replace('/', '\\')
    for executable in executables:
        copied = temp / executable.name
        shutil.copy2(executable, copied)
        print('RUN Windows:', executable.name, flush=True)
        subprocess.run(['/usr/lib/wine/wine64', str(copied), '--nocapture', '--test-threads=1'], cwd=temp, env=env, timeout=100, check=True)
print(f'PASS {len(executables)} Windows Rust test executables using bundled FFmpeg/ffprobe under Wine')
subprocess.run(['/usr/lib/wine/wineserver64', '-k'], env=env, timeout=20, check=False)
