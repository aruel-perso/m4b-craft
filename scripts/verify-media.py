#!/usr/bin/env python3
"""Exercise the FFmpeg command strategy without Rust dependencies.
This is NOT a substitute for cargo test: it does not execute m4b-core.
"""
import hashlib
import json
import math
import os
from pathlib import Path
import struct
import subprocess
import tempfile


def run(*args):
    return subprocess.run(args, check=True, stdout=subprocess.PIPE, stderr=subprocess.PIPE).stdout


def ff(*args):
    return run('ffmpeg', '-hide_banner', '-nostdin', '-v', 'warning',
               '-threads', '1', '-filter_threads', '1', '-filter_complex_threads', '1', *map(str, args))


def probe(path):
    return json.loads(run('ffprobe', '-v', 'error', '-show_format', '-show_streams',
                          '-show_chapters', '-of', 'json', str(path)))


with tempfile.TemporaryDirectory(prefix='m4b-media-') as root:
    root = Path(root)
    hashes = []
    for index, (hz, rate) in enumerate([(440, 44100), (880, 32000)]):
        source = root / f"original{index}.wav"
        ff('-n', '-f', 'lavfi', '-i', f'sine=frequency={hz}:duration=1:sample_rate={rate}', source)
        hashes.append(hashlib.sha256(source.read_bytes()).digest())
        ff('-n', '-i', source, '-map', '0:a:0', '-vn', '-ar', '48000', '-ac', '2',
           '-c:a', 'pcm_s16le', '-rf64', 'auto', root / f'track{index}.wav')
    (root / 'concat.txt').write_text("file 'track0.wav'\nfile 'track1.wav'\n")
    (root / 'cover.ppm').write_bytes(b'P6\n2 2\n255\n' + b'\xff\x00\x00' * 4)
    (root / 'metadata.txt').write_text(
        ';FFMETADATA1\ntitle=Livre \\= \\# \\; é\nartist=Author\n'
        'album_artist=Author\ncomposer=Narrator\ndescription=First line\\\nSecond line\n'
        '[CHAPTER]\nTIMEBASE=1/1000\nSTART=0\nEND=1000\ntitle=track2\n'
        '[CHAPTER]\nTIMEBASE=1/1000\nSTART=1000\nEND=2000\ntitle=track10\n', encoding='utf-8')
    staged = root / 'staged.m4b'
    ff('-n', '-f', 'concat', '-safe', '1', '-i', root / 'concat.txt',
       '-f', 'ffmetadata', '-i', root / 'metadata.txt', '-i', root / 'cover.ppm',
       '-map', '0:a:0', '-map_metadata', '1', '-map_chapters', '1', '-c:a', 'aac',
       '-b:a', '96k', '-map', '2:v:0', '-c:v', 'mjpeg', '-disposition:v:0', 'attached_pic',
       '-movflags', '+faststart', '-f', 'ipod', staged)
    data = probe(staged)
    audio = next(s for s in data['streams'] if s['codec_type'] == 'audio')
    assert audio['codec_name'] == 'aac'
    assert abs(float(audio['duration']) - 2) < .1
    assert data['format']['tags']['title'] == 'Livre = # ; é'
    assert data['format']['tags']['artist'] == 'Author'
    assert data['format']['tags']['composer'] == 'Narrator'
    assert data['format']['tags']['description'] == 'First line\nSecond line'
    assert [c['tags']['title'] for c in data['chapters']] == ['track2', 'track10']
    assert abs(float(data['chapters'][1]['start_time']) - 1) < .002
    assert any(s.get('disposition', {}).get('attached_pic') == 1 for s in data['streams'])
    ff('-xerror', '-i', staged, '-map', '0:a:0', '-f', 'null', '-')
    for offset, expected in [('0.2', 440), ('1.2', 880)]:
        raw = ff('-ss', offset, '-i', staged, '-t', '0.5', '-map', '0:a:0',
                 '-ac', '1', '-ar', '48000', '-f', 's16le', '-')
        samples = [s[0] for s in struct.iter_unpack('<h', raw)]
        hz = sum(a <= 0 < b for a, b in zip(samples, samples[1:])) * 48000 / len(samples)
        assert math.isclose(hz, expected, abs_tol=12), (hz, expected)
    published = root / 'published.m4b'
    os.link(staged, published)
    before = published.read_bytes()
    try:
        os.link(staged, published)
    except FileExistsError:
        pass
    else:
        raise AssertionError('Publication overwrote an existing destination')
    assert published.read_bytes() == before
    for index, digest in enumerate(hashes):
        assert hashlib.sha256((root / f'original{index}.wav').read_bytes()).digest() == digest
    print('PASS: synthetic mixed-rate audio, ordered AAC, escaped Unicode/newline tags, chapters, cover, full decode, hard-link no-overwrite, source hashes')
