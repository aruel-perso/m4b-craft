#!/usr/bin/env python3
"""Compatibility entry point for the current locked Windows release packager.
Produces MSI, portable ZIP and corresponding-source sidecars (not legacy debug ZIP).
Accepts the same arguments as package-release.py.
"""
import runpy
from pathlib import Path

if __name__ == '__main__':
    runpy.run_path(str(Path(__file__).with_name('package-release.py')),run_name='__main__')
