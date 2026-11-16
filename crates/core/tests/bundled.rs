use m4b_core::DesktopFfmpeg;

#[test]
fn bundled_tools_are_preferred_by_absolute_executable_directory() {
    let directory = tempfile::tempdir().unwrap();
    let suffix = if cfg!(windows) { ".exe" } else { "" };
    let ffmpeg = directory.path().join(format!("ffmpeg{suffix}"));
    let ffprobe = directory.path().join(format!("ffprobe{suffix}"));
    std::fs::write(&ffmpeg, b"test fixture, never executed").unwrap();
    std::fs::write(&ffprobe, b"test fixture, never executed").unwrap();
    let backend = DesktopFfmpeg::discover_in(Some(directory.path()));
    assert_eq!(backend.ffmpeg, ffmpeg);
    assert_eq!(backend.ffprobe, ffprobe);
    assert!(backend.ffmpeg.is_absolute());
}

#[test]
fn developer_build_without_bundle_can_use_path() {
    let backend = DesktopFfmpeg::discover_in(None);
    assert_eq!(
        backend.ffmpeg,
        std::path::PathBuf::from(if cfg!(windows) {
            "ffmpeg.exe"
        } else {
            "ffmpeg"
        })
    );
    assert_eq!(
        backend.ffprobe,
        std::path::PathBuf::from(if cfg!(windows) {
            "ffprobe.exe"
        } else {
            "ffprobe"
        })
    );
}
