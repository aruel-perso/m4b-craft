#!/usr/bin/env python3
"""Reject non-ARM64 executables and every non-system dylib (including @rpath)."""
import subprocess
import sys
from pathlib import Path

def verify(path):
    arch = subprocess.check_output(['/usr/bin/lipo','-archs',str(path)],text=True).strip()
    if arch != 'arm64': raise ValueError(f'Expected Mach-O arm64 only: {path}: {arch}')
    output = subprocess.check_output(['/usr/bin/otool','-L',str(path)],text=True)
    lines = output.splitlines()[1:]
    if not lines: raise ValueError(f'No dylib load commands: {path}')
    for line in lines:
        dependency = line.strip().split(' (compatibility version',1)[0]
        if not dependency.startswith(('/usr/lib/','/System/Library/')) or '..' in Path(dependency).parts:
            raise ValueError(f'Non-system dylib: {path}: {dependency}')
    print(f'PASS arm64/system dylib closure: {path}')
if __name__ == '__main__':
    for argument in sys.argv[1:]: verify(Path(argument))
