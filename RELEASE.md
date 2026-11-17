# m4b-Craft 0.3.0-alpha.1 release contract

Approved target: Windows x86_64 MSI (MSI product version **0.3.0**) and native macOS
AppleSilicon ARM64 DMG. This is release tooling, not evidence of publication or of a
successful hosted build. Native Mac validation is pending the first actual CI run.
See [VALIDATION.md](VALIDATION.md) for historical Linux/Wine evidence and current limits.

The integrated Rust workspace version is `0.3.0-alpha.1`.
All current release scripts and CI discover `[workspace.package].version` from root
Cargo.toml through `scripts/release_common.py`. The updater
interface is `windows-x86_64` / `.msi` and `macos-aarch64` / `.dmg`; the GUI executable
inside the Mac bundle is `Contents/MacOS/m4b-Craft`. The document type is `.m4bproj`.
Declaring the type in Info.plist does not prove Finder open-event handling works.

## Build and artifacts

Use Rust 1.100.0, locked Cargo dependencies, Python 3.11+, Bash, curl and native build
tools. CI uses Ubuntu 24.04 with MinGW-w64/wixl/msitools for Windows and `macos-15`
for Mac. GitHub lists that standard runner as ARM64; CI additionally requires
`uname -m = arm64` and never cross-builds the Mac media tools. Runner reference:
[GitHub hosted runners](https://docs.github.com/en/actions/reference/runners/github-hosted-runners).
Xcode command-line clang/make, hdiutil, codesign, lipo and otool come from the Mac
runner; no Homebrew package installation is performed. Mac deployment target is
11.0; operation on that minimum OS has not been tested. Dependency/toolchain minimum
OS requirements still need testing on actual older machines.

Set `TMPDIR` to a writable scratch directory (locally use
`/root/.hermes/cache/scratch`, not `/tmp`). Do not reuse another agent's Cargo target.
The scripts do not install system packages; the disposable Linux CI job installs
its own prerequisites. Example after merging the Rust changes:

```sh
export TMPDIR=/root/.hermes/cache/scratch
export CARGO_BUILD_JOBS=1
mkdir -p "$TMPDIR"
VERSION=$(python3 scripts/release_common.py)
bash scripts/fetch-media-sources.sh "$TMPDIR"
# Linux with MinGW/wixl prerequisites:
bash scripts/build-ffmpeg-windows.sh "$TMPDIR/ffmpeg-8.0.1.tar.xz" "$TMPDIR/zlib-1.3.1.tar.gz"
python3 scripts/package-release.py
python3 scripts/test-release.py --scratch "$TMPDIR" --msi "dist/release-$VERSION/m4b-Craft-$VERSION-windows-x86_64.msi"
# On native ARM64 macOS instead:
bash scripts/build-ffmpeg-macos.sh "$TMPDIR/ffmpeg-8.0.1.tar.xz" "$TMPDIR/zlib-1.3.1.tar.gz"
export PATH="$PWD/vendor/macos-media:$PATH"
cargo test --workspace --locked
python3 scripts/package-macos.py
python3 scripts/test-macos-release.py --scratch "$TMPDIR" --dmg "dist/release-$VERSION/m4b-Craft-$VERSION-macos-aarch64.dmg"
```

`scripts/package-windows.py` forwards to the current Windows packager; it no longer
packages the legacy debug alpha. Both packagers perform fresh locked release builds and stop on failure. Artifacts
under `dist/release-VERSION/` use `m4b-Craft-VERSION-PLATFORM` names:

| Platform | Public binary assets | Corresponding media sources |
| --- | --- | --- |
| windows-x86_64 | `.msi`, `.zip` | `-media-sources.zip` |
| macos-aarch64 | `.dmg` | `-media-sources.zip` |

Every listed file has its own `.sha256` sidecar. Windows also retains generated WiX
and a review payload locally, excluded from public staging. Both bundles include
application sources/Cargo.lock, build recipes, locked dependency license notices,
FFmpeg configuration (`config.h`, `config_components.h`, `config.mak`), zlib generated
build configuration, LGPL/zlib licenses and the GPL application license. Publish
both source ZIPs alongside the installers. Source copying rejects symlinks and
`.pem`/`.key` files. Private material must never be stored anywhere in source trees.

FFmpeg 8.0.1 and zlib 1.3.1 come only from official HTTPS release sources, with pinned
SHA-256 checks before extraction/build. Fetch has a 64 MiB per-download cap, bounded connection/total time,
retries and HTTPS-only redirects; HTTP-success challenge pages fail the checksum.
Both platforms share codec options in `scripts/ffmpeg-options.sh`: built-in AAC
encoding; common audio decoders/containers, cover image support, resampling/scaling;
no network, external codec libraries, shared FFmpeg or ffplay. zlib is linked
statically. Each platform's tool cache key contains platform and script digests;
cache hits still validate the exact expected file records and both pinned archives.
Cargo uses a separate normal platform cache. This is a rebuild recipe, not a claim
of bit-for-bit reproducibility across runner/compiler updates.

Mac `Contents/MacOS` contains `m4b-Craft`, `m4b-cli`, `ffmpeg`, `ffprobe`. All four must
be Mach-O **arm64 only**; `otool -L` allows only `/usr/lib/` and `/System/Library/`
loads, rejecting Homebrew, local, relative and `@rpath` dylibs. Thus no private dylib
closure or Homebrew runtime is required. The DMG contains the `.app`, instructions
and an `/Applications` symlink. Inner binaries and bundle receive ad-hoc signatures;
codesign verification failures stop packaging. **No Developer ID, notarization,
Gatekeeper acceptance or interactive GUI test is claimed.**

The Windows MSI keeps UpgradeCode `790fce8b-e564-43b1-9701-d427fdd80c6c`, per-user
LocalAppData/Programs installation, Start menu shortcut, uninstall registration,
major-upgrade cleanup and downgrade blocking. User projects belong outside managed
installation paths. MSI compares only three numeric version fields: future
installable prereleases must receive a strictly increasing MSI numeric version;
do not reuse 0.3.0 for another prerelease without an explicit versioning decision.
The static WiX file is a layout reference; the generator derives the actual version.

## Update signing and CI publication

The pinned `packaging/update-public-key.hex` is unchanged. Only the external matching
Ed25519 PEM can produce accepted updates. Envelope schema is exactly `payload`
(JSON **string**) and `signature` (64-byte signature in hex). Signature input is the
exact UTF-8 bytes of that string, not reserialized JSON. Its object has exactly
`version`, `platform`, `filename`, `sha256`, `size`, `url`. URLs are bound to
`https://github.com/aruel-perso/m4b-craft/releases/download/vVERSION/FILENAME`.
Installer size must be 1–512 MiB. Canonical semver without build metadata is required;
platform/extension/filename mismatch is rejected. Existing Windows `--msi` calls
remain supported; `--installer` supports both platforms. Do not add payload fields
without coordinating the Rust parser.

```sh
VERSION=$(python3 scripts/release_common.py)
python3 scripts/sign-update.py --key /external/update-signing.pem \
  --version "$VERSION" --platform macos-aarch64 \
  --installer "dist/release-$VERSION/m4b-Craft-$VERSION-macos-aarch64.dmg" \
  --out "dist/release-$VERSION/update-macos-aarch64.json" --scratch "$TMPDIR"
# Windows: --platform windows-x86_64, --installer ...windows-x86_64.msi,
# --out .../update-windows-x86_64.json (or the legacy --msi alias).
```

The workflow runs independent Windows and native Mac jobs with read-only repository
permissions. Mac runs native workspace tests and Clippy with bundled media on PATH,
then mounts the DMG read-only, inspects bundle/plist/source/licenses, copies the app
to a writable simulated Applications directory and ejects the image. With a
system-only PATH, the **installed CLI** creates a PCM project, exports AAC/M4B,
probes/fully decodes it, imports M4B and exports again. GUI launch/interaction,
Finder document opening and actual `/Applications` replacement remain manual tests.
Windows checks MSI tables/extraction, portable ZIP contents/CRC and source digests.
Linux/Wine history is not native Windows or Mac proof.

The release job needs **both jobs successful**, a `vVERSION` tag matching Cargo,
and the `release` environment. Restrict that environment to release tags and protect
tags/reviewer access. `UPDATE_SIGNING_PEM` is read only in that tag job; no build/PR
job receives it. Only release has `contents: write`. Downloaded artifacts are merged
flat, then an exact file allowlist and every SHA-256 sidecar are checked before
signing. Both `update-windows-x86_64.json` and `update-macos-aarch64.json` must be
signed successfully before `gh release create`; prerelease tags use `--prerelease`.
The temporary PEM is permission-restricted and removed with an EXIT trap. Local
scripts never push, tag, publish, call release APIs or configure credentials.

The 0.3 updater contract includes an automatic startup version check and manual
checking, prerelease channel selection, pinned signature/download verification and
**explicit consent before installation**. Integrate and test that Rust work before
release. Save projects and stop playback/media work before handoff. Windows uses
its visible MSI wizard; Mac uses the DMG/app-copy flow. Automatic checking is not
consent to replace the application. Update signatures do not provide Authenticode
or Apple publisher identity: expect unknown-publisher/SmartScreen and Gatekeeper
warnings. Do not claim a live updater, native installation/upgrade or public release
has been validated until the corresponding real execution is recorded.
