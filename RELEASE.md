# m4b-Craft 0.2.0-alpha.1

Release foundation: optional native GitHub Releases updater and a per-user Windows x86_64 MSI (Windows Installer product version **0.2.0**). Existing editing, import, playback, project recovery, CLI/MCP and export features remain.

On Windows x86_64, in Settings, enable update actions for the current session, choose Stable or Include prereleases, then Check GitHub Releases. Nothing contacts GitHub before opt-in and an explicit check. Check and download run on worker threads; media/project actions are locked until completion. Playback must be stopped. There is no periodic polling or telemetry. Stable excludes prereleases; Include prereleases considers both stable and prerelease versions. The highest newer semantic version among the latest 100 non-draft releases is considered. A failed signature on that release is an error, not a fallback to an older release.

The updater trusts only the Ed25519 public key compiled from `packaging/update-public-key.hex`. Release assets must include `update-windows-x86_64.json`. Its envelope has exactly `payload` (a JSON string) and `signature` (hex Ed25519 signature of the exact UTF-8 payload bytes). The signed payload binds `version`, `platform`, `filename`, `sha256`, `size`, and `url`. Unknown fields, invalid versions/platforms, downgrades, mismatched release versions, oversized assets and untrusted URLs are rejected. Install is enabled only after signature and downloaded size/hash verification. Save the project first; installation requires a second explicit confirmation. Signature, size and hash are checked again immediately before launching system `msiexec /i`, with its normal UI and without quiet flags. The editor closes only after Windows Installer has been successfully launched, releasing its executable for replacement. Reopen it after completing or cancelling installation. The verified temporary MSI is retained across this handoff so it is not removed while Windows Installer uses it; it can be removed later through normal temporary-file cleanup. No automatic restart or project discard is performed.

**The update signature is not Authenticode.** It authenticates the release manifest and installer bytes to this application. No Windows publisher certificate is available; the MSI and executable do not have a verified Windows publisher identity. Windows may display an unknown publisher warning. HTTPS, GitHub accounts and asset names alone do not authorize an update.

Network policy: rustls HTTPS only; 10-second connection and 180-second total request timeout, at most five redirects, 1 MiB release listing, 16 KiB manifest, 512 MiB installer limit. Installer and manifest URLs must use the exact `https://github.com/aruel-perso/m4b-craft/releases/download/v<VERSION>/` location; redirect hosts are GitHub release origins (`github.com`, `release-assets.githubusercontent.com`, `objects.githubusercontent.com`). Downloads stream into exclusive temporary files. Files are removed on normal failure/completion; abrupt termination can leave a temporary installer. A local process with access to the user's files is outside this updater's threat model.

## Build and package

Use Rust **1.100.0**, Cargo.lock, one Cargo job, Python 3.11+, OpenSSL 3, MinGW-w64 x86_64 GNU tools and wixl/msitools. Linux GUI development dependencies are also needed for native tests. CI uses Ubuntu 24.04 and builds the Windows GNU target; no MSVC runtime installation is needed. System tools come from the runner distribution; the workflow is a rebuild recipe, not a claim of bit-for-bit reproducibility across distribution package updates. Rust dependencies and upstream media source are locked/pinned. Media tool configuration is in `scripts/build-ffmpeg-windows.sh` and `vendor/windows-media/config*`.

```sh
export CARGO_BUILD_JOBS=1
export TMPDIR=/root/.hermes/cache/scratch
cargo fetch --locked
cargo fmt --all -- --check
cargo test --workspace --locked
cargo clippy --workspace --all-targets --locked -- -D warnings
cargo build --workspace --locked
python3 scripts/package-release.py --wixl /path/to/wixl
python3 scripts/test-release.py --scratch "$TMPDIR" --msi dist/release-0.2.0-alpha.1/m4b-Craft-0.2.0-alpha.1-windows-x86_64.msi
```

The packager performs its own locked Windows GNU **release** build and stops on failure. It does not package the previous debug alpha. It emits the MSI, portable ZIP, FFmpeg/zlib source sidecar, SHA-256 files, generated full WiX source and retained payload in `dist/release-0.2.0-alpha.1/`. `packaging/windows/m4b-Craft.wxs` documents the base layout; the generator adds every application/source/license file. The MSI bundles app, CLI, ffmpeg, ffprobe, application source with Cargo.lock and build recipes, media build configurations and dependency license notices. Publish the media source ZIP alongside the MSI/portable ZIP. The exact FFmpeg 8.0.1 and zlib 1.3.1 archives are pinned by SHA-256 in the workflow. No media download occurs on user machines.

For the already extracted local tools, set `LD_LIBRARY_PATH=/root/.hermes/cache/scratch/m4b-msi-tools/usr/lib/x86_64-linux-gnu` and use `/root/.hermes/cache/scratch/m4b-msi-tools/usr/bin/wixl`. The MSI installs under the current user's LocalAppData/Programs/m4b-Craft, registers in Windows installed applications and creates a Start menu shortcut. The stable UpgradeCode is `790fce8b-e564-43b1-9701-d427fdd80c6c`; never change it. MajorUpgrade removes older managed program files and blocks newer-product downgrades. User projects, imported assets and recovery are outside the managed installation and are never removed by this MSI. Do not save personal files into the program/source directory. New release versions must update the packager, WiX template and tag gate. MSI compares three numeric fields only: allocate a strictly increasing MSI version for each installable future release (including prereleases); do not reuse 0.2.0 for another prerelease product.

## Sign and publish (reviewer only)

The private Ed25519 PEM stays outside the repository. Never copy it into a payload, artifact, source archive, log or git. The signing tool checks that its derived public key matches the pinned repository key and verifies its own signature before writing the envelope.

```sh
python3 scripts/sign-update.py \
  --key /external/private/update-signing.pem \
  --version 0.2.0-alpha.1 \
  --msi dist/release-0.2.0-alpha.1/m4b-Craft-0.2.0-alpha.1-windows-x86_64.msi \
  --out dist/release-0.2.0-alpha.1/update-windows-x86_64.json \
  --scratch /root/.hermes/cache/scratch
```

The tag workflow requires the protected `release` environment and `UPDATE_SIGNING_PEM` secret. Missing/incorrect secrets stop before publishing; there is no unsigned-update fallback. Build/PR jobs have read-only repository permissions and no signing secret. Only the tag release job has contents write permission. Current tag gate is `v0.2.0-alpha.1`, published as a prerelease. Configure the environment and protect release tags before enabling publishing. No repository upload, tag, commit, release or credential configuration is performed by the local scripts. The public repository may still be empty until the reviewer uploads it.

The empty remote cannot yet supply a live end-to-end update. Windows installation, upgrade, uninstall and interactive installer behavior must be smoke-tested on real Windows; Linux MSI table/extraction checks are not a substitute. Key rotation, delta updates, Linux installers and Authenticode signing are not included.
