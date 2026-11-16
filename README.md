# m4b-Craft

GPL-3.0-or-later audiobook creator/editor, Windows 11 x86_64 first. A Rust workspace with a shared core, native egui/eframe desktop editor, and Linux-capable CLI. Version **0.2.0-alpha.1** adds a release foundation; Windows MSI version is **0.2.0**. This remains an alpha requiring real Windows validation. See [RELEASE.md](RELEASE.md) for updater trust, packaging, signing and CI instructions. Core/CLI tests run on Linux; native desktop and Windows validation are tracked separately. See [VALIDATION.md](VALIDATION.md) for exact checks and [ROADMAP.md](ROADMAP.md) for every brief requirement.

## Windows installation and updates

Download the `.msi` installer from [GitHub Releases](https://github.com/aruel-perso/m4b-craft/releases). It installs for the current user, includes FFmpeg/ffprobe, and creates a Start menu shortcut. This is an unsigned Windows publisher preview: Windows may show an unknown-publisher warning. The media-source ZIP is supplied for license/source availability and is not needed to run the app.

For future versions, open **Settings / Réglages**, enable the opt-in GitHub update actions and choose Stable or Include prereleases. Checks and downloads are explicit; installation requires a saved project, a verified Ed25519 signature/hash and confirmation. The editor closes after launching the normal Windows installer; reopen it afterward. Update-manifest signing is not a Windows Authenticode certificate.

## Build and run

Prerequisites: current stable Rust/Cargo, FFmpeg and ffprobe available on PATH, and an FFmpeg build exposing the native `aac` encoder (`ffmpeg -encoders`). The MP3-padding regression fixture also needs `libmp3lame` (included in typical full FFmpeg builds). Developer builds can use PATH; packaged builds prefer FFmpeg/ffprobe in their application media directory. The application does not download media tools. Windows: use the MSVC Rust toolchain with Visual Studio C++ build tools / Windows SDK. Linux desktop builds additionally require your distribution's native graphics, Wayland/X11 and font development libraries; CLI/core need no GUI libraries. A per-user Windows GNU MSI packager is implemented; see [RELEASE.md](RELEASE.md).

```sh
cargo run -p m4b-desktop
cargo run -p m4b-cli -- --help
cargo test -p m4b-core -p m4b-cli
cargo build --release -p m4b-desktop -p m4b-cli
```

Builds use one Cargo job and no development/test debug info. FFmpeg filters use one thread; native AAC is the only selectable codec in this slice. The [GitHub Actions workflow](.github/workflows/build.yml) tests push/PR builds, cross-builds Windows GNU binaries and MSI, and publishes only matching release tags after mandatory external-secret signing. Local scripts never upload or publish. `Cargo.lock` pins the resolved workspace dependencies. Use `--locked` for repeatable CI builds.

## Desktop workflow

Use **Project → New** to name and explicitly create a JSON project (existing files are refused), then **Import** M4B, MP3 or other supported audio / ZIP / RAR / 7Z files. Project also provides Open, Save, Save as and Export. The sidebar separates Tracks (including existing playback), Metadata (including cover), Chapters, Export and Settings. New/Open/Restore/Close prompt to save, discard or cancel when changes are unsaved; cancelling a save or a failed save prevents replacement. JSON projects open cleanly; importing or opening an audiobook marks the project modified.

Imports run on a background worker and commit the entire batch only after all files probe successfully. Desktop imports copy audio into unique persistent batch folders under `<project-stem>.assets` beside the JSON. Keep those folders: Save as retains absolute references to existing assets and does not relocate them. Removing tracks does not delete assets. Opening M4B directly still references the original source. Importing a single M4B/M4A into an empty project retains its metadata, chapters and embedded cover through the copied source. Adding/reordering/removing tracks regenerates one chapter per track, **discarding custom chapter edits**; set the final order before editing chapters.

ZIP import supports stored and Deflate entries with UTF-8 names. Every member is validated; traversal, absolute/Windows device paths, links/special files and duplicate paths (case insensitive) are rejected. Maximum 4,096 entries per batch, 2 GiB per file and 8 GiB expanded per batch; streaming reads enforce actual size and CRC. Extraction uses fresh private staging, generated batch roots and exclusive file creation; failed batches are removed. Unsupported non-audio members reject the whole batch with the member name, rather than silently disappearing. Extract mixed-content archives yourself and select the desired audio files. ZIP64, encrypted ZIP, multi-volume ZIP and other compression methods are refused. RAR/7Z use an optional external `7z` or `7zz` executable, discovered beside the running executable first, then on PATH (`7z.exe`/`7zz.exe` on Windows). Supply the archiver and any runtime components it requires yourself; it is never bundled or downloaded. Missing backends produce actionable guidance. A bounded `-slt` listing preflights every member (including encryption, paths, links, duplicates, size and count). Validated audio members are streamed individually through stdout with `-spd` and `--` into generated private destinations; no blind extraction is used. Stream sizes and exit status are checked, and errors kill/reap the child and roll back the batch. Only formats supported by the supplied archiver work; encrypted archives are refused. Real 7Z import is tested with installed 7-Zip 23.01; no local RAR fixture was found, so RAR runtime compatibility remains unverified. A crash may leave an unreferenced asset batch; there is no asset garbage collector or free-space preflight.

Edit title, author, narrator, description, genre, date and copyright (standard MP4 tags verified on export), pick a JPG/PNG cover, reorder tracks with arrows, edit chapter titles and millisecond start times, add/remove chapters, and select AAC bitrate. First chapter must start at zero; starts must strictly increase and remain inside the total audio duration. Invalid values are rejected before export. The UI shows source codec/rate/channels, approximate encoded audio size, FFmpeg progress, cancellation, and bounded logs/errors. The progress bar weights normalization (45%), encode (50%) and decode validation (4%); it reflects media timestamps, not a reliable wall-clock ETA.

Select English/French and system/light/dark appearance in project settings. Most controls are translated; technical status/error text and theme values remain partly English. Settings persist in the project. There is no separate global preferences store. Autosave writes every 15 seconds while dirty, and on normal exit, to `%LOCALAPPDATA%/m4b-Craft/recovery.autosave.json` on Windows or `$XDG_STATE_HOME/m4b-Craft/` (`~/.local/state/m4b-Craft/`) on Linux. A recovery banner offers restoration on startup. Saving manually clears recovery. Only one recovery slot exists: run one desktop instance. New/open/restore/close protect unsaved changes with a save/discard/cancel dialog. Close requests during a media job wait for it to finish/cancel; close again afterward.

## CLI verification

```sh
# Natural order: 2 before 10, independent of argument order.
cargo run -p m4b-cli -- new --project book.json --title 'My book' \
  --author 'Author' --narrator 'Narrator' --bitrate 96 track10.wav track2.wav
cargo run -p m4b-cli -- check book.json
cargo run -p m4b-cli -- export book.json book.m4b
cargo run -p m4b-cli -- probe book.m4b
cargo run -p m4b-cli -- open book.m4b --project reopened.json
# Optional standalone extraction (refuses existing destinations):
cargo run -p m4b-cli -- open book.m4b --project with-cover.json --extract-cover cover.jpg
bash scripts/verify-cli.sh
```

Project JSON is directly editable for chapter/settings changes. Imported paths are absolute; relative paths in hand-written projects resolve against the project directory. CLI `new` and `open` refuse existing project destinations. Desktop Save intentionally replaces its project atomically; it refuses source/cover paths. CLI global `--ffmpeg PATH --ffprobe PATH` options support explicit binaries; desktop uses bundled tools when present, otherwise PATH. Ctrl+C requests export cancellation. ffprobe preflight and encoder detection are synchronous subprocesses and cannot be interrupted by that token; FFmpeg stages are polled about every 40 ms.

`cargo run -p m4b-cli -- mcp` serves sequential newline-delimited JSON-RPC stdio MCP (2024-11-05). It supports initialize, ping, tools/list and tools/call with **read-only** `probe` and `inspect_project` tools, taking `{ "path": "..." }`. No HTTP transport, exports, writes, subscriptions, MCP client or concurrent cancellation protocol. Only protocol responses go to stdout. Use only with trusted local clients: tools can read local paths. Maximum accepted line length is 1 MiB after reading; this is not a hardened untrusted-input service.

## Export guarantees and costs

The exporter checks project/chapter validity, source existence and source durations, and actual AAC encoder availability. It decodes each ordered track into a generated-name 48 kHz stereo PCM WAV, then joins those compatible files and encodes one continuous AAC stream. The exporter probes each decoded WAV and aligns chapters to decoded track boundaries, preserving the offset within each track; this avoids accumulating MP3 encoder padding as chapter gaps. Header-based project durations may therefore differ slightly from the final duration. A chapter falling entirely in discarded padding is rejected. It writes escaped FFmetadata for tags/chapters and converts an optional cover to attached MJPEG. Filesystem paths are passed as subprocess arguments, never shell text; concat manifests contain only generated names. A same-directory temporary folder holds all intermediates. On completion, ffprobe verifies AAC, duration (500 ms tolerance), all edited tags, chapter names/times, and cover presence; a full audio decode with `-xerror` must also succeed. The staged file is synced, then a hard link publishes it atomically without replacement, including when another writer creates the destination during export. Existing destinations and originals are never overwritten by export. Cancellation/errors remove temporary staging on normal return; hard crashes may leave `.m4b-craft-*` folders for manual cleanup.

This requires a filesystem supporting hard links (normally local NTFS/ext4). Unsupported shares/FAT/exFAT fail safely; no unsafe fallback rename is used. Publication is atomic, but directory entries are not fsynced, so sudden power-loss durability is not guaranteed. Tracks must remain unchanged during export; a duration check is not a content hash or snapshot. The PCM strategy takes **about 11.5 MB per source minute** plus encoded output, and normalizes even already compatible sources. There is no free-space preflight, disk quota, concurrent-job management or lossless remux. PCM staging enables RF64 automatically for tracks exceeding ordinary WAV size limits; large-book behavior still needs field testing. Size estimation excludes cover/container overhead. Source properties are displayed, but bitrate recommendations are not automatic. Multi-audio inputs use the first audio stream. Video containers are accepted only insofar as FFmpeg can read the selected audio/cover stream. No DRM bypass is attempted.

Track/chapter playback and ZIP imports are implemented. Wider ZIP formats, codec choice beyond AAC, Linux AppImage and Android remain unavailable. Windows MSI packaging and opt-in signed GitHub Releases update checks/downloads/installations are implemented; see [RELEASE.md](RELEASE.md) for limits and validation requirements. The `MediaBackend` trait keeps process execution out of the UI and allows a future native Android media backend; the current project/store still uses `std::fs`/paths and needs a storage abstraction for Android content URIs. Update actions are disabled by default and cannot run during media work/playback; installation additionally requires a saved project and explicit confirmation.

## License

[GNU GPL version 3 or later](LICENSE). SPDX: `GPL-3.0-or-later`. Packaged Windows builds include the existing LGPL FFmpeg/zlib media tools, licenses, build configuration and corresponding source sidecar. Development builds can use user-provided FFmpeg. Publish the source sidecar with every binary release; see [RELEASE.md](RELEASE.md).
