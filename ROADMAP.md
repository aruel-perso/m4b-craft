# Requirement map

“Implemented” below describes source code, not a production release. Core/CLI tests run on Linux and cross-compiled Windows GNU tests pass under Wine; Windows GUI launch under Wine passes. Native Windows 11 and interactive UI validation remain outstanding. See VALIDATION.md. Priority: P0 verify this slice; P1 complete desktop v1; P2 platform/distribution extensions.

| BRIEF.md requirement | Current state / next work |
|---|---|
| Rust native desktop, Windows 11 x86_64 first, Linux next | Implemented workspace and eframe UI; Windows MSVC build/test workflow and Linux core workflow. P0 run CI and manual Windows/Linux GUI checks. |
| Shared portable core, no UI coupling | Implemented `m4b-core`, independent of eframe. `MediaBackend` contract isolates desktop subprocess implementation. |
| Future Android without desktop FFmpeg spawning | Backend replaceable. P2 native/mobile media implementation, storage/content URI abstraction, frontend and Android build. Current `std::fs` project persistence is not Android-ready. |
| GPL-3.0-or-later | Implemented license text, package metadata and README. |
| M4B primary open/export | Implemented first audio stream open, tags/chapters/embedded cover retained; staged AAC M4B export. P1 richer stream selection and metadata compatibility matrix. |
| Import audio, natural ordering, manual reorder | Implemented transactional batch probe, natural sorting, up/down/remove UI. Import/reorder/remove resets chapters, explicitly labeled. P1 preserve edited chapters where unambiguous. |
| Title/author/narrator/description plus genre/date/copyright | Implemented editor, backward-compatible JSON defaults, MP4 import/export and real FFmpeg roundtrip validation. Publisher/language tags are not exposed without a verified MP4 roundtrip. |
| Chapters and default one per track | Implemented starts/titles/add/remove/reset; strict export validation, with decoded-duration alignment to avoid MP3 padding gaps. No independent chapter end/gap editor. |
| Embedded cover | Implemented existing source cover reference, external image selection, JPEG attachment, probe validation. P1 image preview and image dimension limits. |
| AAC default / codec and bitrate selection based on encoders | Native AAC with selectable supported bitrate list and actual runtime encoder detection. Other codecs deferred P1; no unavailable-codec UI promises. |
| Source properties, estimated size | Implemented rate/channels/codec/duration and bitrate-derived approximate audio size. P1 automatic suggestions, exact container overhead estimate if useful. |
| Lossless remux when possible | Deferred P1; current pipeline always normalizes PCM then encodes AAC. |
| ZIP/RAR/7z safe import, limits/password | ZIP stored/Deflate implemented with path/link/duplicate validation, CRC, quotas, persistent assets and transactional rollback tests. Optional external 7z/7zz RAR/7Z backend implemented: bounded structured preflight, exact stdout member streaming, kill/reap and rollback. Real 7Z tests pass; RAR lacks a local real fixture and remains runtime-unverified. ZIP64/encryption and other ZIP compression refused. P1 real RAR/Windows backend validation and hostile-input fuzzing. |
| Playback tracks/transitions/output/encoded samples | Existing track/chapter preview retained unchanged, including pause/resume/volume/seek. Transition/output/encoded-sample preview remains P1. |
| Project save | Implemented versioned serde JSON, atomic replacement, canonical imported paths, relative-path load. P1 migrations and portability/copy-media flow. |
| Autosave / recovery | Implemented 15-second dirty autosave and startup recovery banner; single global recovery slot. Save/discard/cancel protection for new/open/restore/close implemented. P1 multiple projects/instances and crash tests. |
| FR/EN selectable | Implemented project language and main controls. P1 complete all strings, logs/dialogs/plurals and localization tests. |
| System/light/dark | Implemented egui theme preference persisted per project. P0 check OS change behavior on Windows. |
| Asynchronous export, real progress, cancel/log/errors | Implemented worker/channel, FFmpeg timestamp progress, token-based child kill/reap, bounded UI logs. Import/open also off UI thread. Probe/encoder subprocesses lack timeout/cancel. P1 bounded process deadlines and job lifecycle stress tests. |
| No overwrite originals, temporary staging, validate then finalize | Implemented normalized staging, full decode + ffprobe validation, same-filesystem atomic hard-link publication. Unsupported hard-link filesystems fail closed. P1 disk preflight/large-file staging and power-loss tests. |
| Stdio MCP shared Rust API | Implemented read-only probe/inspect_project tools, initialize/ping, newline JSON-RPC. P1 protocol conformance suite; writes/export only with explicit design for cancellation/authorization. |
| Optional future MCP client | Deferred P2. |
| No DRM bypass / no advanced waveform v1 | Neither implemented; intentional exclusions. |
| Optional future GitHub Release checks, stable/prerelease | Implemented opt-in worker-thread GitHub checks with semantic-version stable/prerelease selection; see RELEASE.md. Live release test awaits reviewer upload. |
| Mandatory signatures before auto installation, no updates during export | Implemented Ed25519 manifest verification, bounded HTTPS download, SHA-256/size recheck at explicit interactive MSI launch, media/playback lock and saved-project requirement. Key rotation and automatic restart remain deferred. |
| Windows exe and platform installer goal | Windows GNU release MSI/ZIP packaging includes media tools, application source, licenses and media source sidecar. Stable UpgradeCode, per-user shortcut/uninstall and major upgrade tables implemented. Native Windows validation and Authenticode remain outstanding. |
| Linux AppImage later | Deferred P2 after desktop smoke tests. |
| Actual synthetic-audio integration tests | Rust integration test covers mixed rates, MP3 encoder-padding boundaries, natural/audible order, AAC/chapters/cover/tags, no-overwrite race, active/pre-cancellation and invalid-cover cleanup. Passed on Linux. Independent media command smoke test also passed; read-only MCP has a stdio protocol integration test. |
| Low resource builds / tests | One Cargo job, debug info off, FFmpeg filter thread limits, tiny fixtures. P1 long-duration storage strategy. |
| No publish, push, commits or system changes | No commits, pushes, remote setup, releases or local system installs performed. CI is an unexecuted workflow definition. |

P0 exit criteria: review the generated lockfile; compile all crates; run Rust tests on Windows; run desktop open/edit/save/recovery/export/cancel smoke tests on Windows; inspect an exported audiobook in at least two independent players. No claim of a production-ready Windows release before those checks.

Desktop evolution: Project menu and Tracks/Metadata/Chapters/Export/Settings navigation implemented, with explicit named creation and no-clobber project creation. Imported media is copied to persistent sibling assets; Save as retains existing absolute references. Asset relocation/garbage collection and interactive native GUI regression validation remain P1.
