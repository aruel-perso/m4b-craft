# Testing

Fast release-tool tests (no Rust/media compilation or network required):

```sh
python3 scripts/test-source-fetch.py
python3 scripts/test-release.py --scratch /tmp
python3 -m compileall -q scripts
for script in scripts/*.sh; do bash -n "$script"; done
```

The suites cover nullable Cargo license_file, private-note exclusion from source,
MSI identity, media-cache exact checksums and source pins, canonical and legacy
Ed25519 envelopes, wrong keys/platforms/names, payload tampering and a flat exact
asset allowlist. Test signatures use temporary keys, never the release private key.
Transport fixtures prove rejection behavior, not live upstream availability.

CI builds Windows GNU MSI/ZIP and native Mac ARM64 DMG with fresh binaries and
verified media caches. `test-release.py --msi FILE --scratch DIR` inspects MSI
tables, extracts files, checks ZIP CRC and corresponding source hashes.
`verify-windows-bundle.py` runs installed-layout media operations under Wine;
Wine is not a native Windows GUI/upgrade test. `test-macos-release.py` mounts the
DMG, copies the app into a writable directory and exercises the installed CLI
with a system-only PATH. It does not test GUI behavior or live app replacement.

Before publication, exercise the exact signed release payload: install over 0.3,
retain projects/preferences, confirm both Windows shortcut identities and legacy
exe relaunch, export/decode AAC/MP3/FLAC offline, uninstall without deleting user
data. On physical Mac ARM64, test Gatekeeper, GUI playback, canonical and legacy
update helpers and replacement/relaunch from both app roots. No live native Mac
upgrade is claimed by the automated tests. Authenticode and Apple Developer ID /
notarization are outside this release scope; ad hoc sealing is not publisher signing.

Official host-byte verification (no Rust/media build, installer execution or download):

```sh
python3 scripts/test-archiver.py --archives /path/to/verified-archives
```

The directory must contain pinned LAME and all three official 7-Zip 26.06
archives. This checks actual PE x64 members, native ARM64 Mach-O bytes, matching
source/license members and LAME fetch-cache reuse. The Windows CI job repeats it
against its verified cache. Both installed-layout platform tests round-trip a
synthetic ZIP with the bundled archiver. RAR/7Z/password/core integration and
native GUI/upgrade acceptance still require platform execution. Official archive
hashes and extracted member pins are in `scripts/release_common.py`; cache keys
include the extraction recipe. No network blocker remains for supplied bytes.
