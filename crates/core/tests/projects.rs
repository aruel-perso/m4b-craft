use m4b_editor_core::*;
use std::fs;
fn book(dir: &std::path::Path) -> Project {
    let mut p = Project::default();
    for i in 0..3 {
        let path = dir.join(format!("{i}.wav"));
        fs::write(&path, b"synthetic").unwrap();
        p.tracks.push(Track {
            path,
            duration_ms: 1000,
            codec: "pcm_s16le".into(),
            sample_rate: 48000,
            channels: 2,
            stream_index: None,
        });
    }
    p.reset_chapters();
    p.chapters.insert(
        1,
        Chapter {
            start_ms: 400,
            title: "Custom".into(),
        },
    );
    p
}
#[test]
fn reorder_remove_preserve_offsets_and_titles_transactionally() {
    let d = tempfile::tempdir().unwrap();
    let mut p = book(d.path());
    p.reorder_tracks(&[2, 0, 1]).unwrap();
    assert_eq!(p.chapters[2].start_ms, 1400);
    assert_eq!(p.chapters[2].title, "Custom");
    p.remove_track(0).unwrap();
    assert_eq!(p.chapters[1].start_ms, 400);
    let before = serde_json::to_string(&p).unwrap();
    assert!(p.reorder_tracks(&[0, 0]).is_err());
    assert_eq!(serde_json::to_string(&p).unwrap(), before);
    let path = d.path().join("book.m4bproj");
    p.save_new(&path).unwrap();
    assert_eq!(Project::load(&path).unwrap().chapters[1].title, "Custom");
}
#[test]
fn portable_copy_reopens_without_originals_and_refuses_overwrite() {
    let source = tempfile::tempdir().unwrap();
    let destination = tempfile::tempdir().unwrap();
    let mut p = book(source.path());
    let cover = source.path().join("cover.png");
    fs::write(&cover, b"cover").unwrap();
    p.cover = Some(cover);
    let target = destination.path().join("portable");
    let path = p.copy_to(&target).unwrap();
    assert!(p.copy_to(&target).is_err());
    drop(source);
    let loaded = Project::load(&path).unwrap();
    loaded.validate().unwrap();
    assert_eq!(loaded.chapters[1].title, "Custom");
    assert_eq!(fs::read(loaded.cover.unwrap()).unwrap(), b"cover");
    let json: serde_json::Value = serde_json::from_slice(&fs::read(path).unwrap()).unwrap();
    assert!(std::path::Path::new(json["tracks"][0]["path"].as_str().unwrap()).is_relative());
}
#[test]
fn newly_leading_track_gets_boundary_without_losing_custom_offset() {
    let d = tempfile::tempdir().unwrap();
    let mut p = book(d.path());
    p.chapters.retain(|c| c.start_ms != 1000);
    p.chapters.push(Chapter {
        start_ms: 1200,
        title: "Relative".into(),
    });
    p.chapters.sort_by_key(|c| c.start_ms);
    p.reorder_tracks(&[1, 0, 2]).unwrap();
    assert_eq!(p.chapters[0].start_ms, 0);
    assert_eq!(p.chapters[1].start_ms, 200);
    assert_eq!(p.chapters[1].title, "Relative");
    p.validate().unwrap();
}
#[test]
fn copy_cancellation_rolls_back_and_disk_requirement_is_conservative() {
    use std::sync::{atomic::AtomicBool, Arc};
    let d = tempfile::tempdir().unwrap();
    let p = book(d.path());
    let destination = d.path().join("copy");
    let job = Job::new(
        Arc::new(AtomicBool::new(true)),
        std::time::Duration::from_secs(1),
    );
    assert!(p.copy_to_with_job(&destination, &job).is_err());
    assert!(!destination.exists());
    assert!(required_export_bytes(&p) >= p.duration_ms() * 384);
    assert!(available_space(d.path()).unwrap() > 0);
    let mut huge = p;
    huge.tracks[0].duration_ms = u64::MAX;
    assert!(preflight(&huge, d.path()).is_err());
}
