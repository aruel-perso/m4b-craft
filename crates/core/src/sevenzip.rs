//! Optional external archiver. Only validated, exact members are streamed to our own files.
use crate::assets::{audio, copy_limited, safe_name, MAX_BYTES, MAX_FILE, MAX_FILES};
use anyhow::{bail, ensure, Context, Result};
use std::{
    collections::{HashMap, HashSet},
    fs,
    io::Read,
    path::{Path, PathBuf},
    process::{Child, Command, Stdio},
};

struct Process(Child);
impl Drop for Process {
    fn drop(&mut self) {
        let _ = self.0.kill();
        let _ = self.0.wait();
    }
}
fn discover() -> Result<PathBuf> {
    let mut dirs = Vec::new();
    if let Some(parent) = std::env::current_exe()?.parent() {
        dirs.push(parent.to_path_buf());
    }
    if let Some(path) = std::env::var_os("PATH") {
        dirs.extend(std::env::split_paths(&path));
    }
    discover_in(dirs)
}
fn discover_in(dirs: impl IntoIterator<Item = PathBuf>) -> Result<PathBuf> {
    for dir in dirs {
        for name in if cfg!(windows) {
            ["7z.exe", "7zz.exe"]
        } else {
            ["7z", "7zz"]
        } {
            let path = dir.join(name);
            if path.is_file() {
                return Ok(path);
            }
        }
    }
    bail!("RAR/7Z import requires optional 7z or 7zz. Place the archiver beside the application or on PATH; no archiver is bundled.")
}
fn command(exe: &Path, action: &str, source: &Path) -> Command {
    let mut c = Command::new(exe);
    c.args([action, "-bd", "-y", "-p", "-spd", "-sccUTF-8"]);
    if action == "l" {
        c.args(["-slt", "-ba"]);
    } else {
        c.arg("-so");
    }
    c.arg("--")
        .arg(source)
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::null());
    c
}
#[derive(Debug)]
struct Member {
    name: String,
    size: u64,
    directory: bool,
}
fn listing(text: &str) -> Result<Vec<Member>> {
    let mut members = Vec::new();
    let mut seen = HashSet::new();
    let mut total = 0u64;
    for block in text.split("\n\n").filter(|b| !b.trim().is_empty()) {
        let mut fields = HashMap::new();
        for line in block.lines() {
            let (key, value) = line
                .split_once(" = ")
                .context("Malformed archiver listing")?;
            ensure!(
                fields.insert(key, value).is_none(),
                "Ambiguous archiver listing field"
            );
        }
        let name = *fields.get("Path").context("Missing archive path")?;
        let path = safe_name(name)?;
        ensure!(
            seen.insert(name.trim_end_matches('/').to_lowercase()),
            "Duplicate archive path: {name}"
        );
        ensure!(
            !fields
                .keys()
                .any(|k| k.to_ascii_lowercase().contains("link")
                    || k.to_ascii_lowercase().contains("reparse")),
            "Archive links forbidden"
        );
        let attributes = fields.get("Attributes").copied().unwrap_or_default();
        ensure!(
            !attributes
                .split_whitespace()
                .any(|a| a.starts_with(['l', 'b', 'c', 'p', 's'])),
            "Archive special file forbidden"
        );
        ensure!(
            fields.get("Encrypted") == Some(&"-"),
            "Encrypted or unknown encryption state unsupported"
        );
        let directory = fields.get("Folder") == Some(&"+") || attributes.starts_with('D');
        ensure!(
            directory || !name.ends_with('/'),
            "File path ends with directory separator"
        );
        let size: u64 = fields.get("Size").context("Missing member size")?.parse()?;
        ensure!(
            size <= MAX_FILE && (!directory || size == 0),
            "Invalid archive member size"
        );
        total = total.checked_add(size).context("Archive size overflow")?;
        ensure!(
            total <= MAX_BYTES && members.len() < MAX_FILES,
            "Archive exceeds import limits"
        );
        ensure!(
            directory || audio(&path),
            "Unsupported archive member: {name}"
        );
        members.push(Member {
            name: name.into(),
            size,
            directory,
        });
    }
    ensure!(!members.is_empty(), "Empty archive listing");
    // Ancestor lookups avoid quadratic work for large listings.
    let files: HashSet<_> = members
        .iter()
        .filter(|m| !m.directory)
        .map(|m| m.name.to_lowercase())
        .collect();
    for member in &members {
        let name = member.name.trim_end_matches('/').to_lowercase();
        for (offset, _) in name.match_indices('/') {
            ensure!(
                !files.contains(&name[..offset]),
                "Conflicting archive paths"
            );
        }
    }
    Ok(members)
}
pub(crate) fn extract(
    source: &Path,
    root: &Path,
    total: &mut u64,
    count: &mut usize,
) -> Result<Vec<PathBuf>> {
    let exe = discover()?;
    extract_with(&exe, source, root, total, count)
}
fn extract_with(
    exe: &Path,
    source: &Path,
    root: &Path,
    total: &mut u64,
    count: &mut usize,
) -> Result<Vec<PathBuf>> {
    let source = dunce::canonicalize(source).context("Open archive")?;
    let mut child = Process(
        command(exe, "l", &source)
            .spawn()
            .context("Start optional 7z/7zz archiver")?,
    );
    let mut bytes = Vec::new();
    child
        .0
        .stdout
        .take()
        .context("Archiver stdout")?
        .take(16 * 1024 * 1024 + 1)
        .read_to_end(&mut bytes)?;
    ensure!(
        bytes.len() <= 16 * 1024 * 1024,
        "Archive listing exceeds limit"
    );
    ensure!(
        child.0.wait()?.success(),
        "Archiver listing failed (corrupt, encrypted or unsupported archive)"
    );
    let text = String::from_utf8(bytes)?.replace("\r\n", "\n");
    let members = listing(&text)?;
    *count = count
        .checked_add(members.len())
        .context("Entry count overflow")?;
    ensure!(*count <= MAX_FILES, "Import exceeds 4096-entry limit");
    let declared: u64 = members.iter().map(|m| m.size).sum();
    ensure!(
        declared <= MAX_BYTES.saturating_sub(*total),
        "Batch exceeds expanded limit"
    );
    let mut paths = Vec::new();
    for (i, member) in members.iter().enumerate().filter(|(_, m)| !m.directory) {
        // Generated directories prevent archived names from controlling destination traversal.
        let dir = root.join(i.to_string());
        fs::create_dir(&dir)?;
        let dest = dir.join(
            Path::new(&member.name)
                .file_name()
                .context("Member filename")?,
        );
        let mut c = command(exe, "x", &source);
        c.arg(&member.name);
        let mut child = Process(c.spawn().context("Start member stream")?);
        let before = *total;
        copy_limited(
            child
                .0
                .stdout
                .take()
                .context("Member stdout")?
                .take(member.size + 1),
            &dest,
            total,
        )?;
        ensure!(
            *total - before == member.size,
            "Archive member size mismatch"
        );
        ensure!(
            child.0.wait()?.success(),
            "Archive member extraction failed"
        );
        paths.push(dest);
    }
    Ok(paths)
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn discovery_precedence_and_missing_guidance() {
        let dir = tempfile::tempdir().unwrap();
        let beside = dir.path().join("beside");
        let path = dir.path().join("path");
        fs::create_dir(&beside).unwrap();
        fs::create_dir(&path).unwrap();
        assert!(discover_in([beside.clone(), path.clone()])
            .unwrap_err()
            .to_string()
            .contains("beside the application or on PATH"));
        let first = beside.join(if cfg!(windows) { "7zz.exe" } else { "7zz" });
        let second = path.join(if cfg!(windows) { "7z.exe" } else { "7z" });
        fs::write(&first, b"test").unwrap();
        fs::write(&second, b"test").unwrap();
        assert_eq!(discover_in([beside, path]).unwrap(), first);
    }
    #[cfg(unix)]
    #[test]
    fn stream_failure_kills_and_reaps_child() {
        use std::os::unix::fs::PermissionsExt;
        let dir = tempfile::tempdir().unwrap();
        let exe = dir.path().join("fake7z");
        let pid = dir.path().join("pid");
        // A fake command is used only for failure injection, never as format evidence.
        fs::write(&exe, format!("#!/bin/sh\nif [ \"$1\" = l ]; then\nprintf 'Path = a.mp3\\nSize = 1\\nEncrypted = -\\n\\n'\nelse\necho $$ > '{}'\nprintf overflow\nexec sleep 30\nfi\n", pid.display())).unwrap();
        fs::set_permissions(&exe, fs::Permissions::from_mode(0o700)).unwrap();
        let source = dir.path().join("archive");
        fs::write(&source, b"fixture").unwrap();
        let stage = tempfile::tempdir_in(dir.path()).unwrap();
        let stage_path = stage.path().to_path_buf();
        assert!(extract_with(&exe, &source, stage.path(), &mut 0, &mut 0).is_err());
        let process = fs::read_to_string(pid).unwrap();
        assert!(!Path::new("/proc").join(process.trim()).exists());
        drop(stage);
        assert!(!stage_path.exists());
    }
    #[test]
    fn hostile_listings() {
        for name in [
            "../a.mp3",
            "a//b.mp3",
            "CON.mp3",
            "a. /b.mp3",
            "a\nSize = 1",
        ] {
            assert!(listing(&format!("Path = {name}\nSize = 5\nEncrypted = -\n")).is_err());
        }
        for extra in [
            "Symbolic Link = x\n",
            "Hard Link = x\n",
            "Encrypted = +\n",
            "Size = 6\n",
        ] {
            assert!(listing(&format!("Path = a.mp3\nSize = 5\nEncrypted = -\n{extra}")).is_err());
        }
        for text in [
            "Path = a.mp3\nSize = 2147483649\nEncrypted = -\n".to_owned(),
            (0..4097).map(|i| format!("Path = {i}.mp3\nSize = 0\nEncrypted = -\n\n")).collect(),
            (0..5).map(|i| format!("Path = {i}.mp3\nSize = 2147483648\nEncrypted = -\n\n")).collect(),
            "Path = a.mp3\nSize = 1\nEncrypted = -\n\nPath = a.mp3/b.mp3\nSize = 1\nEncrypted = -\n".to_owned(),
            "Path = a.mp3\nSize = 1\nEncrypted = -\nAttributes = A lrwxrwxrwx\n".to_owned(),
        ] { assert!(listing(&text).is_err()); }
        assert!(listing(
            "Path = a.mp3\nSize = 5\nEncrypted = -\n\nPath = A.mp3\nSize = 5\nEncrypted = -\n"
        )
        .is_err());
    }
}
