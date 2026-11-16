use crate::zip;
use crate::*;
use std::{
    collections::HashSet,
    io::{Read, Write},
};

pub(crate) const MAX_FILES: usize = 4096;
pub(crate) const MAX_BYTES: u64 = 8 * 1024 * 1024 * 1024;
pub(crate) const MAX_FILE: u64 = 2 * 1024 * 1024 * 1024;

pub(crate) fn safe_name(name: &str) -> Result<PathBuf> {
    ensure!(
        !name.is_empty()
            && name.len() <= 4096
            && !name.contains('\\')
            && !name.contains(':')
            && !name.contains('\0'),
        "Unsafe archive path: {name:?}"
    );
    ensure!(
        !name.starts_with('/') && name.split('/').all(|c| !matches!(c, "." | "..")),
        "Unsafe archive path: {name:?}"
    );
    let trimmed = name.strip_suffix('/').unwrap_or(name);
    ensure!(
        !trimmed.is_empty() && trimmed.split('/').all(|p| !p.is_empty()),
        "Empty archive path component"
    );
    for part in trimmed.split('/') {
        ensure!(
            !part.ends_with(['.', ' ']) && !part.chars().any(|c| c < ' ' || "<>\"|?*".contains(c)),
            "Unsafe Windows archive path: {name:?}"
        );
        let stem = part
            .split('.')
            .next()
            .unwrap_or_default()
            .to_ascii_uppercase();
        ensure!(
            ![
                "CON", "PRN", "AUX", "NUL", "COM1", "COM2", "COM3", "COM4", "COM5", "COM6", "COM7",
                "COM8", "COM9", "LPT1", "LPT2", "LPT3", "LPT4", "LPT5", "LPT6", "LPT7", "LPT8",
                "LPT9", "CONIN$", "CONOUT$", "COM¹", "COM²", "COM³", "LPT¹", "LPT²", "LPT³"
            ]
            .contains(&stem.as_str()),
            "Reserved Windows path: {name:?}"
        );
    }
    Ok(Path::new(name).components().collect())
}
pub(crate) fn audio(path: &Path) -> bool {
    path.extension().and_then(|e| e.to_str()).is_some_and(|e| {
        ["m4b", "m4a", "mp3", "wav", "flac", "ogg", "opus", "aac"]
            .contains(&e.to_ascii_lowercase().as_str())
    })
}
pub(crate) fn copy_limited(mut input: impl Read, path: &Path, total: &mut u64) -> Result<()> {
    let mut out = fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(path)?;
    let mut size = 0;
    let mut buf = [0u8; 65536];
    loop {
        let n = input.read(&mut buf)?;
        if n == 0 {
            break;
        }
        size += n as u64;
        *total += n as u64;
        ensure!(
            size <= MAX_FILE && *total <= MAX_BYTES,
            "Import exceeds limits: 2 GiB/file, 8 GiB/batch"
        );
        out.write_all(&buf[..n])?;
    }
    out.sync_all()?;
    Ok(())
}
impl Project {
    /// Copies a whole batch into persistent assets; failures leave the project unchanged.
    /// The caller owns the asset directory and must retain it alongside the project.
    pub fn import_assets(
        &mut self,
        backend: &dyn MediaBackend,
        paths: Vec<PathBuf>,
        assets: &Path,
    ) -> Result<()> {
        ensure!(!paths.is_empty(), "Select at least one file");
        fs::create_dir_all(assets).context("Create project asset directory")?;
        let assets = dunce::canonicalize(assets)?;
        let stage = tempfile::Builder::new()
            .prefix(".import-")
            .tempdir_in(&assets)?;
        let mut media = Vec::new();
        let mut total = 0;
        let mut count: usize = 0;
        for (index, source) in paths.iter().enumerate() {
            let ext = source
                .extension()
                .unwrap_or_default()
                .to_string_lossy()
                .to_ascii_lowercase();
            let root = stage.path().join(index.to_string());
            fs::create_dir(&root)?;
            if ["rar", "7z"].contains(&ext.as_str()) {
                media.extend(crate::sevenzip::extract(
                    source, &root, &mut total, &mut count,
                )?);
            } else if ext == "zip" {
                let mut zip = zip::ZipArchive::new(fs::File::open(source)?)
                    .context("Read ZIP (encrypted/unsupported archives cannot be imported)")?;
                count = count
                    .checked_add(zip.len())
                    .context("Entry count overflow")?;
                ensure!(count <= MAX_FILES, "Import exceeds 4096-entry limit");
                let mut seen = HashSet::new();
                let mut declared = 0u64;
                // Validate every entry before writing anything from this archive.
                for i in 0..zip.len() {
                    let entry = zip
                        .by_index(i)
                        .context("Read ZIP entry: encrypted/unsupported entry")?;
                    let name = safe_name(entry.name())?;
                    ensure!(
                        seen.insert(name.to_string_lossy().trim_end_matches('/').to_lowercase()),
                        "Duplicate archive path: {}",
                        entry.name()
                    );
                    if let Some(mode) = entry.unix_mode() {
                        let kind = mode & 0o170000;
                        ensure!(
                            kind == 0 || kind == 0o100000 || (kind == 0o040000 && entry.is_dir()),
                            "Archive links/special files forbidden: {}",
                            entry.name()
                        );
                    }
                    ensure!(
                        entry.size() <= MAX_FILE,
                        "ZIP entry exceeds 2 GiB: {}",
                        entry.name()
                    );
                    declared = declared
                        .checked_add(entry.size())
                        .context("ZIP size overflow")?;
                    ensure!(
                        declared <= MAX_BYTES.saturating_sub(total),
                        "ZIP exceeds 8 GiB batch expanded limit"
                    );
                    ensure!(
                        !entry.is_dir() || entry.size() == 0,
                        "Directory entry has unexpected data: {}",
                        entry.name()
                    );
                    if !entry.is_dir() {
                        ensure!(audio(&name), "Unsupported ZIP member {}; batch rejected. Extract and select audio explicitly.", entry.name());
                    }
                }
                for i in 0..zip.len() {
                    let mut entry = zip.by_index(i)?;
                    let dest = root.join(safe_name(entry.name())?);
                    if entry.is_dir() {
                        fs::create_dir_all(&dest)?;
                        continue;
                    }
                    fs::create_dir_all(dest.parent().context("ZIP parent")?)?;
                    copy_limited(&mut entry, &dest, &mut total)
                        .with_context(|| format!("Extract {}", entry.name()))?;
                    media.push(dest);
                }
            } else {
                ensure!(audio(source), "Unsupported import: {}", source.display());
                count += 1;
                ensure!(count <= MAX_FILES, "Import exceeds 4096 files");
                let dest = root.join(source.file_name().context("Missing filename")?);
                copy_limited(fs::File::open(source)?, &dest, &mut total)?;
                media.push(dest);
            }
        }
        ensure!(!media.is_empty(), "Archive contains no audio files");
        let mut next = self.clone();
        next.import(backend, media)?;
        // Persist the successful batch with a unique name, preserving prior batches.
        let old = stage.path().to_path_buf();
        let committed = stage.keep();
        for track in &mut next.tracks {
            if let Ok(relative) = track.path.strip_prefix(&old) {
                track.path = committed.join(relative);
            }
        }
        if let Some(cover) = &mut next.cover {
            if let Ok(relative) = cover.strip_prefix(&old) {
                *cover = committed.join(relative);
            }
        }
        *self = next;
        Ok(())
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn hostile_paths() {
        for name in [
            "../a.mp3",
            "/a.mp3",
            "C:/a.mp3",
            "a\\b.mp3",
            "a/../b",
            "./x",
            "a//b.mp3",
            "a///",
            "NUL.mp3",
            "LPT¹.mp3",
            "CONIN$.mp3",
            "a./b.mp3",
            "a /b.mp3",
        ] {
            assert!(safe_name(name).is_err(), "{name}");
        }
        assert!(safe_name("book/02.mp3").is_ok());
    }
    #[test]
    fn legacy_metadata_defaults() {
        let m: Metadata = serde_json::from_str(
            r#"{"title":"Old","author":"A","narrator":"N","description":"D"}"#,
        )
        .unwrap();
        assert!(m.genre.is_empty());
    }
}

#[cfg(test)]
mod archive_tests {
    use super::*;
    struct Backend;
    impl MediaBackend for Backend {
        fn probe(&self, path: &Path) -> Result<Probe> {
            ensure!(fs::read(path)? == b"audio", "Invalid test audio");
            Ok(Probe {
                track: Track {
                    path: dunce::canonicalize(path)?,
                    duration_ms: 1000,
                    codec: "test".into(),
                    sample_rate: 48000,
                    channels: 2,
                },
                metadata: Metadata::default(),
                chapters: vec![],
                has_cover: false,
            })
        }
        fn encoders(&self) -> Result<Vec<String>> {
            Ok(vec![])
        }
        fn export(
            &self,
            _: &Project,
            _: &Path,
            _: Cancel,
            _: &mut dyn FnMut(ExportEvent),
        ) -> Result<()> {
            bail!("unused")
        }
    }
    fn archive(path: &Path, entries: &[(&str, &[u8], u32)]) {
        let mut data = Vec::new();
        let mut central = Vec::new();
        for (name, bytes, mode) in entries {
            let compressed = name.contains("deflate");
            let payload = if compressed {
                let mut encoder =
                    flate2::write::DeflateEncoder::new(Vec::new(), flate2::Compression::default());
                encoder.write_all(bytes).unwrap();
                encoder.finish().unwrap()
            } else {
                bytes.to_vec()
            };
            let method = if compressed { 8u16 } else { 0u16 };
            let offset = data.len() as u32;
            let crc = crc32fast::hash(bytes);
            data.extend(b"PK\x03\x04");
            data.extend(20u16.to_le_bytes());
            data.extend([0; 2]);
            data.extend(method.to_le_bytes());
            data.extend([0; 4]);
            data.extend(crc.to_le_bytes());
            data.extend((payload.len() as u32).to_le_bytes());
            data.extend((bytes.len() as u32).to_le_bytes());
            data.extend((name.len() as u16).to_le_bytes());
            data.extend(0u16.to_le_bytes());
            data.extend(name.as_bytes());
            data.extend(&payload);
            central.extend(b"PK\x01\x02");
            central.extend([20, 3]);
            central.extend(20u16.to_le_bytes());
            central.extend([0; 2]);
            central.extend(method.to_le_bytes());
            central.extend([0; 4]);
            central.extend(crc.to_le_bytes());
            central.extend((payload.len() as u32).to_le_bytes());
            central.extend((bytes.len() as u32).to_le_bytes());
            central.extend((name.len() as u16).to_le_bytes());
            central.extend([0; 8]);
            central.extend((mode << 16).to_le_bytes());
            central.extend(offset.to_le_bytes());
            central.extend(name.as_bytes());
        }
        let offset = data.len() as u32;
        let size = central.len() as u32;
        data.extend(central);
        data.extend(b"PK\x05\x06");
        data.extend([0; 4]);
        data.extend((entries.len() as u16).to_le_bytes());
        data.extend((entries.len() as u16).to_le_bytes());
        data.extend(size.to_le_bytes());
        data.extend(offset.to_le_bytes());
        data.extend([0; 2]);
        fs::write(path, data).unwrap();
    }
    #[test]
    fn persistent_transactional_import_and_named_project() {
        assert!(Project::named("  ").is_err());
        let dir = tempfile::tempdir().unwrap();
        let zip = dir.path().join("book.zip");
        let assets = dir.path().join("book.assets");
        archive(
            &zip,
            &[
                ("part10-deflate.mp3", b"audio", 0o100644),
                ("part2.mp3", b"audio", 0o100644),
            ],
        );
        let mut p = Project::named(" Book ").unwrap();
        p.import_assets(&Backend, vec![zip.clone()], &assets)
            .unwrap();
        assert_eq!(p.name, "Book");
        assert!(p.tracks[0].path.ends_with("part2.mp3"));
        fs::remove_file(&zip).unwrap();
        let project = dir.path().join("book.json");
        p.save_new(&project).unwrap();
        let loaded = Project::load(&project).unwrap();
        assert!(loaded.tracks.iter().all(|t| t.path.is_file()));
        let before = format!("{p:?}");
        let batches = fs::read_dir(&assets).unwrap().count();
        archive(
            &zip,
            &[
                ("valid.mp3", b"audio", 0o100644),
                ("bad.mp3", b"invalid", 0o100644),
            ],
        );
        assert!(p.import_assets(&Backend, vec![zip], &assets).is_err());
        assert_eq!(format!("{p:?}"), before);
        assert_eq!(fs::read_dir(&assets).unwrap().count(), batches);
    }
    #[test]
    fn hostile_archives_rollback() {
        let dir = tempfile::tempdir().unwrap();
        let zip = dir.path().join("hostile.zip");
        let assets = dir.path().join("assets");
        let mut p = Project::default();
        for entries in [
            vec![("../escape.mp3", b"audio".as_slice(), 0o100644)],
            vec![("/escape.mp3", b"audio".as_slice(), 0o100644)],
            vec![("link.mp3", b"audio".as_slice(), 0o120777)],
            vec![
                ("a.mp3", b"audio".as_slice(), 0o100644),
                ("A.mp3", b"audio".as_slice(), 0o100644),
            ],
            vec![("notes.txt", b"audio".as_slice(), 0o100644)],
            vec![("CON.mp3", b"audio".as_slice(), 0o100644)],
        ] {
            archive(&zip, &entries);
            assert!(p
                .import_assets(&Backend, vec![zip.clone()], &assets)
                .is_err());
            assert!(p.tracks.is_empty());
            assert_eq!(fs::read_dir(&assets).unwrap().count(), 0);
        }
    }
    #[cfg(unix)]
    #[test]
    fn real_sevenzip_import_order_and_rollback() {
        // This is an integration test of the installed executable, not a synthetic 7z fixture.
        let exe = Path::new("/usr/bin/7z");
        if !exe.is_file() {
            eprintln!("SKIP: /usr/bin/7z unavailable");
            return;
        }
        let dir = tempfile::tempdir().unwrap();
        for name in ["track10.mp3", "track2.mp3", "-switch.mp3"] {
            fs::write(dir.path().join(name), b"audio").unwrap();
        }
        let zip = dir.path().join("book.7z");
        assert!(std::process::Command::new(exe)
            .current_dir(dir.path())
            .args(["a", "-bd", "-spd", "--"])
            .arg(&zip)
            .args(["track10.mp3", "track2.mp3", "-switch.mp3"])
            .output()
            .unwrap()
            .status
            .success());
        let assets = dir.path().join("assets");
        let mut project = Project::default();
        project
            .import_assets(&Backend, vec![zip.clone()], &assets)
            .unwrap();
        assert!(project.tracks[1].path.ends_with("track2.mp3"));
        assert!(project.tracks[2].path.ends_with("track10.mp3"));
        let before = format!("{project:?}");
        let batches = fs::read_dir(&assets).unwrap().count();
        // Corrupt archive, real password-protected archive, and a valid archive with bad audio.
        fs::write(&zip, b"broken archive").unwrap();
        assert!(project
            .import_assets(&Backend, vec![zip.clone()], &assets)
            .is_err());
        fs::remove_file(&zip).unwrap();
        assert!(std::process::Command::new(exe)
            .current_dir(dir.path())
            .args(["a", "-bd", "-psecret", "--"])
            .arg(&zip)
            .arg("track2.mp3")
            .output()
            .unwrap()
            .status
            .success());
        assert!(project
            .import_assets(&Backend, vec![zip.clone()], &assets)
            .is_err());
        fs::remove_file(&zip).unwrap();
        fs::write(dir.path().join("bad.mp3"), b"invalid").unwrap();
        assert!(std::process::Command::new(exe)
            .current_dir(dir.path())
            .args(["a", "-bd", "--"])
            .arg(&zip)
            .arg("bad.mp3")
            .output()
            .unwrap()
            .status
            .success());
        assert!(project.import_assets(&Backend, vec![zip], &assets).is_err());
        assert_eq!(format!("{project:?}"), before);
        assert_eq!(fs::read_dir(&assets).unwrap().count(), batches);
    }
    #[test]
    fn separate_input_roots_keep_natural_track_order() {
        let dir = tempfile::tempdir().unwrap();
        let mut paths = Vec::new();
        for name in ["track10.mp3", "track2.mp3", "track1.mp3"] {
            let path = dir.path().join(name);
            fs::write(&path, b"audio").unwrap();
            paths.push(path);
        }
        let mut project = Project::default();
        project
            .import_assets(&Backend, paths, &dir.path().join("assets"))
            .unwrap();
        for (track, expected) in
            project
                .tracks
                .iter()
                .zip(["track1.mp3", "track2.mp3", "track10.mp3"])
        {
            assert!(track.path.ends_with(expected));
        }
    }
    #[test]
    fn quotas_and_corrupt_zip() {
        let dir = tempfile::tempdir().unwrap();
        let zip = dir.path().join("bad.zip");
        let assets = dir.path().join("assets");
        let mut p = Project::default();
        archive(&zip, &[("a.mp3", b"audio", 0o100644)]);
        let original = fs::read(&zip).unwrap();
        let central = original
            .windows(4)
            .position(|b| b == b"PK\x01\x02")
            .unwrap();
        let mut too_many = original.clone();
        let end = too_many.len() - 22;
        too_many[end + 8..end + 10].copy_from_slice(&4097u16.to_le_bytes());
        too_many[end + 10..end + 12].copy_from_slice(&4097u16.to_le_bytes());
        fs::write(&zip, too_many).unwrap();
        assert!(p
            .import_assets(&Backend, vec![zip.clone()], &assets)
            .is_err());
        let mut oversized = original.clone();
        oversized[central + 24..central + 28]
            .copy_from_slice(&((MAX_FILE + 1) as u32).to_le_bytes());
        fs::write(&zip, oversized).unwrap();
        assert!(p
            .import_assets(&Backend, vec![zip.clone()], &assets)
            .is_err());
        for (position, value) in [
            (end + 12, 1u32),
            (central + 42, central as u32),
            (central + 20, u32::MAX - 1),
        ] {
            let mut malformed = original.clone();
            malformed[position..position + 4].copy_from_slice(&value.to_le_bytes());
            fs::write(&zip, malformed).unwrap();
            assert!(p
                .import_assets(&Backend, vec![zip.clone()], &assets)
                .is_err());
            assert_eq!(fs::read_dir(&assets).unwrap().count(), 0);
        }
        let mut corrupt = original;
        corrupt[35] ^= 1;
        fs::write(&zip, corrupt).unwrap();
        assert!(p.import_assets(&Backend, vec![zip], &assets).is_err());
        assert_eq!(fs::read_dir(&assets).unwrap().count(), 0);
    }
}
