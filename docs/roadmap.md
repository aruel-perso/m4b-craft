# Roadmap

0.4.0 implements stable M4B Editor naming, portable projects, preserved custom
chapters, bounded/cancellable media jobs, selected audio streams, metadata,
AAC/MP3/FLAC exports, encoded previews, isolated recovery and FR/EN UI.
Regression suites exercise the actual core and CLI paths, both UI languages,
stale-save protection, read-only project loading and large MP4 metadata.

Automated acceptance covers Linux, Windows tests under Wine and native Mac ARM64,
bundled media/archive tools, MSI structure and mounted DMG installed CLI operation.
The exact hosted MSI is also exercised through 0.3-to-0.4 upgrade, exports and
uninstall while preserving external data. Physical Windows/Mac GUI, audible
playback and live in-app replacement remain separate manual acceptance work.

Publication requires signed manifests with the original trust key. The old-slug
compatibility channel retains historical release assets/source tags and supports
old clients without weakening origin or signature checks.

Future work: publisher signing/notarization, other desktop architectures and
platforms, Android backend abstraction. No DRM circumvention or advanced waveform
editing is planned for this release.
