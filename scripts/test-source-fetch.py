#!/usr/bin/env python3
"""Offline regression tests: transport fixtures, never substitute release artifacts."""
import hashlib
import os
from pathlib import Path
import subprocess
import tempfile
import unittest

SCRIPT = Path(__file__).resolve().with_name('fetch-media-sources.sh')


class SourceFetchTests(unittest.TestCase):
    def setUp(self):
        self.directory = tempfile.TemporaryDirectory(prefix='m4b-source-fetch-')
        self.addCleanup(self.directory.cleanup)
        self.root = Path(self.directory.name)
        self.good = b'unit-test upstream archive bytes'
        self.digest = hashlib.sha256(self.good).hexdigest()
        self.destination = self.root / 'archive.tar.gz'
        self.log = self.root / 'calls'
        curl = self.root / 'curl'
        curl.write_text('''#!/usr/bin/env python3
import os,sys
from pathlib import Path
args=sys.argv[1:]; url=args[-1]
with open(os.environ['FETCH_CALLS'],'a') as f:f.write(url+'\\n')
if url.endswith('/offline'):sys.exit(22)
Path(args[args.index('--output')+1]).write_bytes(
 b'unit-test upstream archive bytes' if url.endswith('/good') else b'<html>challenge</html>')
''')
        curl.chmod(0o700)
        self.env = dict(os.environ, PATH=str(self.root) + os.pathsep + os.environ['PATH'],
                        FETCH_CALLS=str(self.log))

    def fetch(self, *urls):
        result = subprocess.run(['bash', '-c', 'source "$1"; shift; fetch_verified "$@"',
                                 'fetch-test', str(SCRIPT), str(self.destination), self.digest, *urls],
                                env=self.env, capture_output=True, text=True)
        self.assertEqual(list(self.root.glob('*.partial.*')), [])
        return result

    def test_http_success_challenge_falls_back_to_verified_source(self):
        result = self.fetch('https://official/challenge', 'https://official/good')
        self.assertEqual(result.returncode, 0, result.stderr)
        self.assertEqual(self.destination.read_bytes(), self.good)
        self.assertEqual(len(self.log.read_text().splitlines()), 2)

    def test_all_invalid_sources_fail_closed(self):
        result = self.fetch('https://official/challenge', 'https://official/offline')
        self.assertNotEqual(result.returncode, 0)
        self.assertFalse(self.destination.exists())

    def test_valid_cached_source_needs_no_network(self):
        self.destination.write_bytes(self.good)
        self.assertEqual(self.fetch('https://official/offline').returncode, 0)
        self.assertFalse(self.log.exists())

    def test_corrupt_cached_source_is_replaced_only_with_verified_bytes(self):
        self.destination.write_bytes(b'corrupt')
        result = self.fetch('https://official/good')
        self.assertEqual(result.returncode, 0, result.stderr)
        self.assertEqual(self.destination.read_bytes(), self.good)


if __name__ == '__main__':
    unittest.main(verbosity=2)
