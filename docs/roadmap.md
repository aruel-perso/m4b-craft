# Roadmap

0.4.0 targets stable M4B Editor naming, portable projects, preserved custom
chapters, bounded/cancellable media jobs, selected audio streams, metadata,
AAC/MP3/FLAC exports, encoded previews, isolated recovery and FR/EN UI.
The core and GUI implementation and its acceptance results are reviewed separately
from the packaging changes. This list is release scope, not evidence of validation.

Release gates: merge the 0.4.0 code/version, verify official archiver artifacts
before bundling, run hosted Windows/Mac builds, inspect the exact installers,
complete native upgrade/GUI tests, create the legacy release-only repository and
publish signed manifests with the original trust key. Preserve corresponding
sources and historical releases.

Future work: publisher signing/notarization, other desktop architectures and
platforms, Android backend abstraction. No DRM circumvention or advanced waveform
editing is planned for this release.
