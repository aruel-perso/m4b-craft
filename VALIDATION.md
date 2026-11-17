# Validation record — Windows alpha

## Integrated 0.3.0-alpha.1 verification

Fresh reviewer execution after the focused fixes passed: 43 Linux workspace tests,
40 Windows tests across six executables under Wine, strict workspace/all-target
Clippy, rustfmt and `git diff --check`. Real media tests include cover preview,
AAC packet-hash preservation during remux, metadata/chapters and no-overwrite export.
Release tooling tests passed 6 cases; source-fetch tests passed 4 cases.
Save-target overwrite consent and update channel selection have regression tests.
The duplicate central seek slider was removed in favor of the bottom transport.
These results are not native Windows certification or successful macOS execution.
The new two-platform hosted build and public release gates remain pending until
actual GitHub execution; historical sections below describe earlier snapshots.

## Environment and scope

Build host: Linux x86_64. Rust/Cargo 1.100.0, rustfmt and Clippy installed for this task. MinGW x86_64 cross-linker and Windows GNU Rust target installed. Native Linux media tests use system FFmpeg; Windows test executables use the Gyan FFmpeg 9.0.2 essentials build under Wine 9.0. Windows media binaries were downloaded only for verification, not included in the deliverable. Cargo.lock pins all dependencies. No commit, push, remote repository, GitHub publication or release was created.

## Passed checks

- `cargo test --workspace --locked --offline`: 6 tests passed, 0 failed (3 core unit tests, 2 real-media integration tests, 1 stdio MCP integration test). Desktop currently has no automated interaction tests.
- `cargo clippy --workspace --all-targets --locked --offline -- -D warnings`: passed.
- `cargo fmt --all -- --check`: passed.
- `cargo build -p m4b-desktop -p m4b-cli --target x86_64-pc-windows-gnu --locked --offline`: passed; produces real Windows x86_64 PE GUI and CLI executables. Alpha uses the development profile with debug information disabled, not an optimized release profile.
- Cross-compiled core and CLI Windows test executables run using Wine: 6 tests passed, 0 failed. These exercised Windows Rust file/process behavior and actual Windows FFmpeg/ffprobe binaries, including create/open/metadata/chapters/embedded cover, AAC export/full decode, audio order, MP3 padding timeline, cancellation, staging cleanup, no-overwrite publication, project persistence and stdio MCP.
- Independent FFmpeg command-strategy test (`scripts/verify-media.py`) passed on Linux. It does not execute Rust.
- Actual Rust CLI end-to-end smoke passed on Linux: new/check/export/probe/open, natural ordering, Unicode/space/apostrophe paths, and project/output overwrite refusal.
- Windows GUI executable started under Wine on an isolated Xvfb display; `xdotool` confirmed the window title `m4b-Craft`. This is a launch smoke test, not visual/interactive GUI validation.
- `git diff --check`: passed; sources are not committed.

## Bugs found by Windows execution and corrected

- Windows canonical paths can contain the extended `\\?\` prefix, which breaks FFmpeg concat-relative paths. Canonicalization now uses `dunce` to retain ordinary Windows path syntax where representable. The actual Windows export test verifies this path.
- `FlushFileBuffers` requires a write-capable file handle on Windows. Final export synchronization uses read/write OpenOptions instead of a read-only File::open.
- MP3 header duration behavior differs across FFmpeg versions. The regression test checks the final chapter timeline without requiring a specific header-padding representation.

## Not yet verified or delivered

No native Windows 11 machine was used. Windows MSVC build and the GitHub Actions workflow have not run. Native dialogs, detailed GUI interaction, appearance/theme changes, accessibility, recovery UX and independent audiobook-player compatibility need Windows testing. No screenshot/visual validation is claimed. Large books, disk-full/power-loss, source mutation during export, unsupported hard-link filesystems and long Windows paths need additional coverage.

The alpha is not the full agreed product: wider ZIP formats, advanced previews, codecs beyond AAC, lossless remux, installer/file association, signed updates, write/export MCP tools, full localization, Linux packaging and Android remain tracked in ROADMAP.md. Export requires a hard-link-capable destination (normally local NTFS); PCM staging needs substantial free space and has no preflight quota check. Do not use the alpha as the only copy of a valuable project.

## Project/menu/import evolution — Linux, 2026-11-16

The previous Windows record above is historical; these changes have not been revalidated on Windows or through interactive GUI automation. Existing playback source was preserved.

- Offline single-job workspace tests and build run for this change (see final verification below). Tests exercise named creation/legacy metadata defaults, persistent copied assets after source removal, natural ordering, failed-batch rollback, hostile ZIP paths/links/case-colliding duplicates/unsupported members, advertised size limits and corrupt CRC. ZIP fixtures include stored and Deflate entries.
- Real FFmpeg integration exercises genre/date/copyright alongside existing tags, chapters and cover, M4B import into an empty project, copied M4B assets independent of their original source, and re-export. Publisher/language are intentionally not added.
- UI compiles; menus, section navigation, native dialogs, unsaved-change prompts, close interception and recovery need manual native interaction checks. No visual or interactive GUI pass is claimed.
- Archive reader is restricted to stored/Deflate UTF-8 ZIP, without ZIP64/encryption/multi-volume. At that stage no external backend was invoked; the follow-up below supersedes the RAR/7Z refusal. crates.io DNS was unavailable; existing cached flate2/CRC dependencies were reused, with no OS installation.
- Initial source/docs/lockfile snapshot is in `/tmp/m4b-evolution-snapshot`. No commits or pushes were made. Successful asset batches are retained even if tracks are later removed; Save as does not relocate assets. Crash/power-loss cleanup, fuzzing and large-limit stress tests remain outstanding.

Final verification for this evolution: `cargo test --workspace --locked --offline -j 1` passed **19 tests, 0 failures**; `cargo build --workspace --locked --offline -j 1`, `cargo clippy --workspace --all-targets --locked --offline -j 1 -- -D warnings` and `cargo fmt --all -- --check` passed. The desktop test includes the save/discard/cancel decision, including unsuccessful/cancelled saves. Native dialog interaction remains untested.


## Archive hardening follow-up — 2026-11-16

Preserved the existing UI, metadata and ZIP evolution. No commits, system installs, archiver downloads/bundles or packaging were performed. All new probes and Cargo temporary files used `TMPDIR=/root/.hermes/cache/scratch` (manual archive probe directory under that root).

- Optional RAR/7Z backend discovers 7z/7zz beside current_exe before PATH. It preflights a bounded UTF-8 `-slt -ba` listing, rejects ambiguous fields, encryption, hostile paths, links/special files, case-insensitive duplicates, prefix conflicts and declared count/size violations. Members stream individually with `-so -spd --` into generated directories and exclusive destinations. Actual byte counts and successful exit statuses are required; RAII kills/reaps children on failure, and import staging rolls back.
- The installed `/usr/bin/7z` reports 7-Zip 23.01. Real generated `.7z` import tests pass, including a leading-dash member name, natural order, corrupt input, password refusal and failed-media rollback. Failure-injection process test checks that an overflowing stream child has been killed/reaped and its staging removed. Synthetic listings test traversal, Windows names, empty components, duplicate fields/paths, links, encryption, oversized files/batches, excess count and prefix conflicts. Synthetic listings are not RAR-format evidence.
- No `.rar` fixture was found in local `/root`, `/usr`, `/opt` or `/srv` searches. No RAR-format import success is claimed. Runtime RAR support depends on the supplied archiver build; native Windows archiver execution also remains unverified.
- ZIP central-directory records and variable fields are bounded by the declared directory; local headers/payloads cannot overlap that directory. Stored sizes must agree; unsupported flags, excessive local names, invalid indexes and malformed directory sizes are rejected. Existing CRC/stream limits remain. Path validation rejects empty components, trailing dots/spaces, Windows devices including superscript COM/LPT aliases and CONIN$/CONOUT$. A single trailing slash is accepted only as directory notation.
- Filename-first natural sorting already existed and is retained. A new separate-input-root regression proves track1/track2/track10 ordering despite reversed selection and generated input-index directories.

Final follow-up commands/results are recorded below. Cross compilation is build evidence only; no native Windows GUI/runtime verification or new Wine run is claimed. Fuzzing, large-limit stress, disk exhaustion, crash cleanup and concurrent source mutation remain unverified. The external process has no wall-clock timeout; a stalled archiver can stall import. ZIP64, encrypted ZIP and other ZIP compression methods remain unsupported.

Final follow-up verification (Linux host; all passed):

- `TMPDIR=/root/.hermes/cache/scratch cargo test --workspace --locked --offline`: **24 tests passed, 0 failed** (13 core unit, 2 tool-discovery integration, 2 real FFmpeg integration, 1 CLI MCP integration, 6 desktop unit). The installed-7z integration test ran successfully; it was not skipped.
- `TMPDIR=/root/.hermes/cache/scratch cargo clippy --workspace --all-targets --locked --offline -- -D warnings`: passed.
- `cargo fmt --all -- --check`: passed.
- `TMPDIR=/root/.hermes/cache/scratch cargo build --locked --offline --target x86_64-pc-windows-gnu -p m4b-desktop -p m4b-cli`: passed for both executables.
- `git diff --check`: passed; repository content is currently untracked, so this alone does not validate new-file whitespace. Rust formatting was checked independently.

## Independent reviewer verification

- Re-ran Linux workspace tests: **24 passed**, including the installed Linux 7z test; strict Clippy, formatting and native build passed.
- Cross-built both Windows executables and all tests. Ran six Windows test executables with bundled FFmpeg/ffprobe under Wine: **22 tests passed**. The Linux-specific `/usr/bin/7z` integration and Unix process-injection tests are excluded on Windows; they are not Windows archiver evidence. Fixed the Linux archiver test's platform guard after Wine exposed an invalid-handle failure when attempting to launch a Linux executable from the Windows test.
- Windows archiver paths now use `dunce::canonicalize`, avoiding extended verbatim path prefixes.
- Exercised the actual Windows GUI under Wine on an isolated Xvfb display: opened the Project menu, selected Metadata, and opened New project. Screenshot OCR confirmed Save/Import menu entries, Genre/Copyright editor labels and the project-name dialog. Does not prove native Windows dialog save/import or full visual/accessibility quality.
- Native Linux GUI launch was attempted but is blocked by missing `libxkbcommon-x11.so` on the host; no package was installed to work around it. Windows GUI launch and the interactions above succeeded instead.
- RAR with real media, native Windows 7-Zip, ZIP64/password support and stalled-process deadlines remain outstanding as stated above.

## 0.2.0-alpha.1 release foundation (2026-11-16)

This section supersedes earlier statements that no updater or installer exists. Before-edit copies are in `/root/.hermes/cache/scratch/m4b-release-before/`; release logs and reviewer patch are beside that directory. No commit, push, GitHub write, credential setup or local OS package installation was performed. The existing Ed25519 public key is unchanged; private signing material is external and excluded from all source/payload copying.

- Cargo initially failed DNS resolution in the sandbox. A later `cargo fetch --locked` succeeded; real ed25519-dalek 2.2.0 and rustls reqwest 0.12.28 dependencies were fetched and locked. No substitute crypto/HTTP implementation was used.
- `TMPDIR=/root/.hermes/cache/scratch cargo test --workspace --locked`: **31 passed**, including existing core/media/CLI/playback tests and updater signature tampering, wrong key/signature length, malformed/oversized signed manifests, platform/filename/URL/hash/size rules, semver/downgrade/channel rules, pre-network busy rejection, pending playback/decoder guard, and changed/truncated installer detection.
- Native workspace build, strict workspace/all-target clippy (`-D warnings`) and rustfmt checks pass. One Cargo build job is configured in `.cargo/config.toml` and CI.
- Optimized Windows GNU desktop and CLI cross-build succeeded with Rust 1.100.0. PE import inspection of app, CLI, ffmpeg and ffprobe passes the packager's Windows system-DLL allowlist. No external media/runtime installer or PATH change is required.
- WiX source compiled with the reviewer's extracted wixl/msitools, without installing those packages. MSI product version is 0.2.0; per-user scope, LocalAppData destination, Start menu shortcut, registry/uninstall cleanup, fixed UpgradeCode, downgrade detection and major-upgrade sequencing after InstallInitialize are inspected in MSI tables. The final packager retains the full generated source and payload for review. Application source/licenses are included; pinned FFmpeg/zlib source is supplied as a sidecar ZIP.
- Actual MSI table/export/extraction and portable ZIP CRC/content checks pass, including application source and private-key exclusion. The media-source sidecar archive hashes match the pinned upstream FFmpeg/zlib source. Release tooling security tests cover payload binding/tampering and refusal of a mismatched external PEM. The release workflow is defined but has not run on GitHub; it requires a matching tag, protected release environment and matching signing secret before creating a signed release.

A Wine MSI execution attempt was blocked before installation: wineserver cannot create its server directory under the sandbox's read-only `/run/user/0`. No Windows install/upgrade/uninstall execution or new GUI smoke-test is claimed. The remote is empty, so live discovery/download/install from GitHub is not verified. Real Windows validation remains necessary. Cryptographic update signing is implemented; Authenticode publisher signing is unavailable. See RELEASE.md for the release contract and rebuild/signing commands.

## Independent release review

- Re-ran the final Linux workspace suite: 31 tests passed; strict Clippy and formatting passed.
- Cross-built and executed all six Windows test executables under Wine with bundled media tools: 29 tests passed. The two Linux-only archiver/process tests remain excluded on Windows.
- Executed the actual generated MSI under Wine/Xvfb: per-user installation under LocalAppData, installed CLI version 0.2.0-alpha.1, bundled AAC encoder availability, installed application/source files and Start menu shortcut passed. Uninstallation removed the executable while leaving an external project and asset preservation fixture unchanged. This is actual Wine installation/uninstallation evidence, not native Windows certification or a real prior-version upgrade.
- Corrected installer handoff: after re-verification and explicit confirmation, spawn the system Windows Installer and close the saved/idle editor so its executable is not locked during replacement. The verified MSI remains available across process exit; launch failure preserves the editor and removes the retained MSI.
- The release signing secret is configured in a GitHub environment restricted to `v*` tags. Public source contains only the pinned public key; the private key stays outside the repository and in the encrypted GitHub environment secret. GitHub workflow/release execution status is authoritative on the repository Actions/Releases pages.

## 0.3.0-alpha.1 Windows + native Mac release tooling

This section records the scoped packaging work on `release/macos-0.3`, superseding
older release-status statements above where applicable. The branch is an isolated
worktree. Cargo.toml/Cargo.lock, crates, BRIEF.md and the pinned updater public key
were not changed by this work. Root Cargo version is still the earlier version in
this snapshot; the Rust owner's separate 0.3.0-alpha.1 bump and updater implementation
must be merged before tagging. Current packagers discover that version dynamically.

Local host is Linux x86_64. No macOS executable, hdiutil, codesign, lipo or otool was
run here. No build target/media bundle was duplicated and no full Rust/media rebuild
was performed for these script/document changes. No system installation, push,
tag, release or release API call was performed. The original linked worktree Git metadata is read-only under `/srv`, so staging/
commit there failed with `index.lock: Read-only file system`. A scoped commit is
prepared on the same-named branch in writable scratch Git metadata, with a Git
bundle for transfer; the original worktree branch reference remains unchanged. Historical Rust/Wine/MSI results above remain historical evidence,
not a new test of 0.3.0 or a native Windows certification.

Local checks actually executed successfully:

- `TMPDIR=/root/.hermes/cache/scratch python3 scripts/test-release.py --scratch /root/.hermes/cache/scratch`: **6 tests passed**. Real temporary Ed25519 keys/OpenSSL signatures exercise both platform payloads, exact envelope/payload field sets and UTF-8 signature bytes, signature tampering, wrong pinned key, filename/platform mismatch, canonical semver and the backwards-compatible Windows `--msi` call. The repository public key is not replaced; fixture repositories receive fixture keys only.
- The same suite checks generated WiX identity/per-user layout, strict media-cache records and source pinning, flat two-platform release staging/checksum refusal. Mocked lipo/otool output tests ARM64-only/system-dylib policy, rejection of x86_64/universal binaries, Homebrew/local libraries and `@rpath`. **Mocks are not real Mac execution evidence.**
- `TMPDIR=/root/.hermes/cache/scratch python3 scripts/test-source-fetch.py`: **4 tests passed**, covering HTTP-success challenge rejection/fallback, all-sources failure, verified cache reuse and corrupt-cache replacement. Transport fixtures do not download upstream bytes or prove current upstream availability.
- `python3 -m compileall -q scripts`: passed.
- `bash -n scripts/build-ffmpeg-macos.sh scripts/build-ffmpeg-windows.sh scripts/ffmpeg-options.sh scripts/fetch-media-sources.sh`: passed.
- Workflow YAML parsed successfully using the already available Prettier YAML parser at `/root/.hermes/lsp/node_modules/prettier/plugins/yaml.js`. This is syntax validation, not a hosted workflow run or actionlint validation.
- `git diff --check`: passed. Final scoped staged diff checked before commit.

**Pending, not tested:** the new native `macos-15` CI job, native Cargo workspace
suite on Mac, actual AppleSilicon FFmpeg/zlib build, real Mach-O/dylib inspection,
ad-hoc bundle signatures, creation/mounting of the DMG, writable simulated install
and installed CLI PCM→M4B export/import/probe/re-export. The workflow implements all
of these checks and will fail if any fails; only successful actual CI logs may
promote them to passed. The current public runner label was checked against GitHub's
[official hosted-runner reference](https://docs.github.com/en/actions/reference/runners/github-hosted-runners);
the job independently asserts `uname -m = arm64`.

Also pending: a new Windows packaging CI run after the shared-option/cache changes;
real Windows install/upgrade/uninstall; real Mac Applications installation/replacement;
GUI launch/interaction on Mac; Finder `.m4bproj` open events; Gatekeeper/quarantine
behavior; minimum macOS 11 testing; live two-platform signed update discovery,
download and consent/handoff. No GUI test, Developer ID, notarization, Authenticode,
public release success or native platform acceptance is claimed. The manifest secret
remains confined to the tag-only protected release environment; the release job
requires both platform jobs to pass before signing and publishing either platform.

## Focused 0.3 UI/updater fixes — 2026-11-17

Save As now asks explicit overwrite consent for the final normalized `.m4bproj`
path. Update-check messages carry the requested channel; mismatched results are
ignored, channel changes clear candidates and persist immediately, and download
requires a candidate matching the selected channel. Manual GitHub checks and the
channel selector are available in Settings both with and without a project; the
startup check remains mandatory. The central Tracks seek slider was removed;
track play initiation and volume remain, with seeking in the bottom transport.

macOS update staging errors explain that the installed `.app` parent must be
user-writable and direct users to copy the app into `~/Applications`, eject the
DMG and reopen the installed copy. No administrator elevation is requested.
This alpha has no Apple Developer ID signing or notarization.

Linux verification: `cargo test --workspace --offline` passed 43 tests;
`cargo clippy --workspace --all-targets --locked --offline -- -D warnings`,
`cargo fmt --all -- --check` and `git diff --check` passed. Added helper coverage
for normalized save targets/overwrite consent, candidate-channel matching and the
macOS recovery error; existing preference round-trip coverage includes the channel.
Native dialog consent, pointer dragging and actual macOS replacement remain
manual review items. No packaging, Wine tests, system installs, commits or pushes
were performed for these fixes.
