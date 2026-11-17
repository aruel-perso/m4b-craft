//! Crash recovery is scoped to a project and a unique running instance.
use m4b_editor_core::Project;
use sha2::{Digest, Sha256};
use std::path::{Path, PathBuf};
pub struct Recovery {
    directory: PathBuf,
}
impl Recovery {
    pub fn new(root: &Path) -> std::io::Result<Self> {
        std::fs::create_dir_all(root)?;
        let directory = tempfile::Builder::new()
            .prefix("instance-")
            .tempdir_in(root)?
            .keep();
        std::fs::write(directory.join("owner"), std::process::id().to_string())?;
        Ok(Self { directory })
    }
    pub fn slot(&self, path: Option<&Path>, project: &Project) -> PathBuf {
        let identity = path
            .map(|p| p.to_string_lossy().into_owned())
            .unwrap_or_else(|| format!("unsaved:{}", project.name));
        self.directory
            .join(format!("{:x}.m4bproj", Sha256::digest(identity.as_bytes())))
    }
    pub fn discover(root: &Path) -> Vec<PathBuf> {
        let mut found = Vec::new();
        if let Ok(entries) = std::fs::read_dir(root) {
            for entry in entries.flatten() {
                let directory = entry.path();
                if !directory.is_dir() {
                    continue;
                }
                let pid = std::fs::read_to_string(directory.join("owner"))
                    .ok()
                    .and_then(|p| p.parse().ok());
                if pid.is_some_and(alive) {
                    continue;
                }
                if let Ok(slots) = std::fs::read_dir(directory) {
                    found.extend(slots.flatten().map(|s| s.path()).filter(|p| {
                        p.extension().is_some_and(|e| e == "m4bproj")
                            && !p.with_extension("resolved").exists()
                    }));
                }
            }
        }
        found.sort();
        found
    }
}
impl Drop for Recovery {
    fn drop(&mut self) {
        let _ = std::fs::remove_file(self.directory.join("owner"));
        let _ = std::fs::remove_dir(&self.directory);
    }
}
#[cfg(unix)]
fn alive(pid: u32) -> bool {
    unsafe {
        libc::kill(pid as i32, 0) == 0
            || std::io::Error::last_os_error().raw_os_error() == Some(libc::EPERM)
    }
}
#[cfg(windows)]
fn alive(pid: u32) -> bool {
    #[link(name = "kernel32")]
    extern "system" {
        fn OpenProcess(access: u32, inherit: i32, pid: u32) -> *mut std::ffi::c_void;
        fn CloseHandle(handle: *mut std::ffi::c_void) -> i32;
        fn GetExitCodeProcess(handle: *mut std::ffi::c_void, code: *mut u32) -> i32;
    }
    unsafe {
        let handle = OpenProcess(0x1000, 0, pid);
        if handle.is_null() {
            return false;
        }
        let mut code = 0;
        let result = GetExitCodeProcess(handle, &mut code);
        CloseHandle(handle);
        result != 0 && code == 259
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn independent_slots_live_conflicts_and_crash_discovery() {
        let root = tempfile::tempdir().unwrap();
        let a = Recovery::new(root.path()).unwrap();
        let b = Recovery::new(root.path()).unwrap();
        let p = Project::named("Book").unwrap();
        let slot = a.slot(Some(Path::new("a.m4bproj")), &p);
        assert_ne!(slot, a.slot(Some(Path::new("b.m4bproj")), &p));
        assert_ne!(slot, b.slot(Some(Path::new("a.m4bproj")), &p));
        p.save_new(&slot).unwrap();
        assert!(Recovery::discover(root.path()).is_empty());
        drop(a);
        assert_eq!(Recovery::discover(root.path()), vec![slot.clone()]);
        assert_eq!(Project::load(&slot).unwrap().name, "Book");
    }
}
