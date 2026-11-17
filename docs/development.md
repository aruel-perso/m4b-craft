# Development

M4B Editor uses Rust workspace packages `m4b-editor-core`, `m4b-editor-desktop`
and `m4b-editor-cli`. The binaries are `m4b-editor` and `m4b-editor-cli`.
Use the toolchain declared by CI and Python 3.11 or newer. The workspace version
is read from Cargo.toml; release tooling never substitutes a hardcoded version.
The 0.4.0 release requires the code changes and workspace version to be merged first.

```sh
cargo fetch --locked
cargo fmt --all -- --check
cargo test --workspace --locked
cargo clippy --workspace --all-targets --locked -- -D warnings
```

Use `CARGO_BUILD_JOBS=1` on constrained hosts. One owner builds Rust/media at a
time. Packaging scripts build fresh binaries and fail on a failed build.
Do not package stale executables. See [testing](testing.md) and [release](release.md).

## Media backend

`scripts/fetch-media-sources.sh DIRECTORY` fetches pinned FFmpeg 8.0.1,
zlib 1.3.1 and LAME 3.100 sources from upstream release locations. Every download
must match its committed SHA-256. An HTTP-success challenge page is rejected;
never update a digest to make a failing download pass. Verified cached archives
work offline. The source fetch regression suite uses local transport fixtures.

Pass all three archives, in that order, to `build-ffmpeg-windows.sh` or
`build-ffmpeg-macos.sh`. Windows requires the GNU cross toolchain; Mac requires
native ARM64 clang. LAME is built statically without its frontend, then linked
into FFmpeg as libmp3lame. The build enables AAC, MP3 and FLAC encoders and the
matching MP4, MP3 and FLAC muxers. Capability claims require actual packaged-tool
encoding tests. FFmpeg networking and automatic library discovery are disabled.

Installers contain application source, build recipes, media configuration,
Rust license notices and media licenses. Each platform's media-sources ZIP
contains the exact pinned upstream archives. Retain these sidecars with each
release so static-library modifications can be rebuilt with the supplied recipe.

## Archiver assessment

[Official downloads](https://www.7-zip.org/download.html) provide Windows x64
and native Mac ARM64 console distributions, plus corresponding source.
[Licensing](https://www.7-zip.org/license.txt) permits redistribution with license
notices; some decompression code has the unRAR restriction. Treat 7-Zip as a
separate executable, preserve all notices, distribute matching source and inspect
its dependencies/architecture before bundling. The recipes pin official 26.06
Windows SFX, native Mac archive and matching source archive. `bundle-archiver.py`
extracts Windows `7z.exe` and `7z.dll` as archive data, never runs the installer,
and uses lipo on the Mac runner when thinning is necessary. Actual 26.06 Mac
bytes are already thin ARM64. Member hashes, licenses, upstream archive hashes
and exact cache records are verified before packaging. Both source sidecars
include the original official archives and corresponding source. No first-launch
download or proprietary RAR creation is provided. Native archive/password/offline
acceptance remains required; archive/password behavior belongs to the core owner.
