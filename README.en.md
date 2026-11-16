# m4b-Craft

[![Latest release including prereleases](https://img.shields.io/github/v/release/aruel-perso/m4b-craft?include_prereleases&label=release%20%28including%20alpha%29)](https://github.com/aruel-perso/m4b-craft/releases)

[Français](README.md) · **English**

Create and edit M4B audiobooks locally: track import, natural ordering, chapters,
metadata, cover art, playback and AAC export. GUI, CLI and a read-only MCP server.
No remote audio service required.

**0.3.0-alpha.1 is a prerelease in preparation.** The badge reflects actual published
releases; this repository does not establish that publication has occurred.
Visit [GitHub releases, including prereleases](https://github.com/aruel-perso/m4b-craft/releases)
and select files from the same version:

| Platform | Planned 0.3.0-alpha.1 download | Installation |
| --- | --- | --- |
| Windows x86_64 | [MSI](https://github.com/aruel-perso/m4b-craft/releases/download/v0.3.0-alpha.1/m4b-Craft-0.3.0-alpha.1-windows-x86_64.msi) | Open MSI; per-user installation and Start menu shortcut. |
| macOS AppleSilicon ARM64 | [DMG](https://github.com/aruel-perso/m4b-craft/releases/download/v0.3.0-alpha.1/m4b-Craft-0.3.0-alpha.1-macos-aarch64.dmg) | Mount DMG, drag `m4b-Craft.app` into Applications, eject disk. |

These links become available only after successful publication. No Intel Mac,
installable Linux or Android edition is included. FFmpeg and ffprobe are bundled:
no media installation or Homebrew runtime dependency is required. The Windows ZIP
is a portable alternative; the `-media-sources` ZIP provides corresponding sources
and is not required to launch the application.

**Signing warnings:** Ed25519 update signing is neither Windows publisher signing
nor Apple notarization. Windows may show “unknown publisher” or SmartScreen.
The Mac application has ad-hoc signatures, without Developer ID or notarization;
Gatekeeper may block it. Use Privacy & Security / Open Anyway only for a download
whose origin you trust and when permitted by your security policy.

Historical validation includes Linux and Wine tests; the new macOS CI work remains
**untested until an actual run succeeds**. No interactive Mac GUI test is claimed.
See [VALIDATION.md](VALIDATION.md) and [CI runs](https://github.com/aruel-perso/m4b-craft/actions).

## Projects and transport

Create a `.m4bproj` project, import files, reorder tracks and edit chapters,
metadata and cover art, then export a new `.m4b`. Imported files copied into the
project's asset folder must stay with it. To transport an editable project, save,
close and copy **the entire project folder with its assets**, preserving relative
paths. Keep backups; never store projects inside the installation directory or
`.app`. Older projects may reference absolute paths: verify sources on the receiving
machine. For an audio player, transport the exported M4B. “Save as” does not guarantee
relocation of every asset.

M4B import reads available audio, chapters and metadata. ZIP accepts unencrypted
stored/Deflate entries; RAR/7Z requires a compatible separately supplied `7z`/`7zz`
(not bundled). No DRM bypass. Export uses PCM staging, approximately **11.5 MB per
source minute**, plus AAC output. There is no free-space preflight; allow sufficient
space and use a destination supporting hard links. Export refuses to overwrite an
existing file. Long books, power loss and changing sources still need validation.

## Integrated updates

The 0.3 contract provides an automatic startup check through GitHub Releases and a
manual check in settings. The check contacts GitHub for version information; it does
not authorize installation. The stable channel excludes prereleases; enable their
inclusion to receive alphas. Manifest signatures, download size and SHA-256 are
verified against the embedded public key, unchanged for this release.

**No installation without your explicit consent.** Save the project and stop
playback/export before confirming. Windows opens the MSI wizard; macOS uses the DMG
and application copying into Applications. Follow the application's instructions
for closing and replacement. Neither the startup check nor a download alone replaces
your application. The Rust behavior for this version must be integrated and validated
with this pipeline before publication.

## Development

Rust 1.100.0, Python 3.11+ and Cargo.lock. Native development:

```sh
cargo test --workspace --locked
cargo run -p m4b-desktop
cargo run -p m4b-cli -- --help
```

FFmpeg/ffprobe must be beside the executables or on PATH during development.
Linux requires GUI/audio development system libraries. Windows GNU and Mac ARM64
recipes, pinned sources, licenses, update manifests and limits are documented in
[RELEASE.md](RELEASE.md).

License: [GPL-3.0-or-later](LICENSE). Bundled FFmpeg/zlib tools retain their licenses;
each binary release must ship notices, configurations and its corresponding
`-media-sources.zip` archive.
