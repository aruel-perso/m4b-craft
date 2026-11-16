# Alpha 0.1.1 — preview and self-contained Windows distribution

This record supersedes earlier alpha documentation where it differs. Historical README/ROADMAP/VALIDATION files may describe 0.1.0.

## Implemented

- Native in-app rodio preview: Play per source track or chapter, pause/resume, stop, volume, sample-based elapsed position, seek by restarting at the selected position. Opening a M4B allows the same controls for its audio and chapters.
- Bounded streaming: FFmpeg produces stereo 48 kHz signed PCM through an OS pipe and a bounded queue; the whole audiobook is not decoded into RAM or a temporary preview file. A worker owns playback and device initialization. Stop, track change, seek and shutdown kill/reap the decoder and reader threads. Missing audio devices are reported instead of crashing the UI. Preview stops before export.
- Packaged FFmpeg/ffprobe discovered relative to the application executable, not CWD. No user package installation, PATH change, ffplay/other player, first-launch download or VC runtime installer is required.
- Windows media binaries are built from unmodified FFmpeg 8.0.1 and static zlib 1.3.1. FFmpeg is LGPL-2.1-or-later; zlib has its permissive license. Exact config and build recipe accompany the app. Full upstream source archives are supplied in the separate `-media-sources.zip` sidecar to keep each attachment below messaging limits. That sidecar is not needed to run the app.

## Real verification

- `cargo test --workspace --locked --offline`: all tests passed on Linux, including the existing media/MCP suite, bundled-tool discovery and five playback tests.
- Playback tests render actual non-silent FFmpeg PCM through the same rodio sink used by the app, without requiring a physical device. They verify rendering, sample counting, pause/resume, mute/volume controls, seek offset, stream framing, chapter-to-track mapping and cleanup. A separate decoder test verifies half-second seek/PCM length.
- `cargo clippy --workspace --all-targets --locked --offline -- -D warnings`: passed.
- `cargo fmt --all`: completed; Windows binaries and all test executables cross-compiled successfully with MinGW.
- All six Windows Rust test executables ran under Wine with the newly built bundled FFmpeg/ffprobe. All tests passed, including the five playback tests and real M4B exports.
- ZIP end-to-end smoke: extracted the package, launched the packaged Windows CLI from an unrelated Unicode directory containing deliberately invalid `ffmpeg.exe`/`ffprobe.exe` names, and left WINEPATH empty. Actual WAV import, AAC M4B export, embedded PNG cover, chapters, metadata, reopening and sought non-silent preview PCM all passed. Source files and an existing output were preserved.
- PE import inspection: FFmpeg/ffprobe import only Windows system DLLs (bcrypt, kernel32, msvcrt, shell32); no separately installed codec, zlib or compiler-runtime DLL is needed. Desktop imports only Windows system DLLs, including OS audio interfaces.
- Package script verifies ZIP CRCs, required executables/source entries, source-sidecar CRCs and writes SHA256 files. Both matching archives must be delivered together for complete source availability.

## Limits

No native Windows 11 machine or physical loudspeaker/headphone test was available. Windows audio-device behavior, GUI interaction and audible quality remain acceptance checks for the user; Wine tests are not native Windows certification. No screenshot/visual validation is claimed. The executable is unsigned.

Preview currently plays one source at a time; automatic inter-track transitions and encoded comparison samples remain future work. Additional output codecs, remux without encoding, installer/file association, signed updates, full localization and write/export MCP remain unimplemented. Export still uses substantial PCM disk staging and requires a hard-link-capable filesystem such as local NTFS; it has no disk-space preflight.

## Project/menu evolution

The current source adds a Project menu, named project creation with save/discard/cancel protection, and separate Tracks, Metadata, Chapters, Export and Settings sections. Imports copy audio into a persistent sibling `.assets` directory. Keep this directory with the project; Save As retains references rather than relocating previous assets. A single M4B imported into an empty project retains chapters, supported tags and embedded cover. Genre, date and copyright now join title, author, narrator and description in the editor and tested FFmpeg roundtrip.

Stored/Deflate ZIP import is built in, with strict path, duplicate, link, CRC and expanded-size checks. ZIP64, encrypted archives and non-audio members are rejected. RAR/7Z uses optional external `7z`/`7zz`, discovered beside the app or on PATH; it is **not bundled**, so these formats do not have the zero-install guarantee of ordinary audio/ZIP. Real 7Z import was verified on Linux, but no real RAR fixture or Windows archiver was available. External archiver subprocesses currently have no timeout.

The updated Windows binary was cross-built and its applicable Rust tests executed under Wine. Menu opening, the metadata screen and the New project dialog were also exercised under Wine/Xvfb with screenshot OCR. This remains an unsigned preview, not native Windows 11 acceptance.
