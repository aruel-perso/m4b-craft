#![cfg_attr(target_os = "windows", windows_subsystem = "windows")]
mod catalog;
mod playback;
mod preferences;
mod recovery;
mod updater;
use m4b_editor_core::{Cancel, Chapter, DesktopFfmpeg, ExportEvent, MediaBackend, Project};
use playback::{CommandMessage as AudioCommand, Playback};
use std::{
    path::PathBuf,
    sync::{
        atomic::{AtomicBool, Ordering},
        mpsc, Arc,
    },
    time::{Duration, Instant},
};
fn main() -> eframe::Result<()> {
    eframe::run_native(
        "M4B Editor",
        eframe::NativeOptions {
            viewport: egui::ViewportBuilder::default().with_inner_size([1000., 800.]),
            ..Default::default()
        },
        Box::new(|_| Ok(Box::new(App::default()))),
    )
}
use eframe::egui;
enum Message {
    Copied(Result<PathBuf, String>),
    Streams(usize, Result<Vec<m4b_editor_core::AudioStream>, String>),
    Preview(tempfile::TempDir, Result<PathBuf, String>),
    Cover(PathBuf, Result<egui::ColorImage, String>),
    UpdateProgress(f32),
    Encoders(Result<bool, String>),
    UpdateChecked(updater::Channel, Result<updater::Verified, String>),
    UpdateDownloaded(Result<updater::Download, String>),
    UpdateInstalled(Result<(), String>),
    Loaded(Box<Result<Project, String>>, Option<Option<PathBuf>>),
    LoadedProject(
        PathBuf,
        Box<Result<(Project, m4b_editor_core::ProjectBaseline), String>>,
    ),
    Event(ExportEvent),
    Done(Result<(), String>),
}
#[derive(Clone, Copy, PartialEq)]
enum Section {
    Tracks,
    Metadata,
    Chapters,
    Export,
    Settings,
}
struct App {
    pending_recoveries: Vec<PathBuf>,
    restored_from: Option<PathBuf>,
    recovery_store: recovery::Recovery,
    encoded_preview: Option<tempfile::TempDir>,
    archive_password: String,
    selected_stream: u32,
    stream_track: Option<usize>,
    stream_choices: Vec<m4b_editor_core::AudioStream>,
    prefs: preferences::Preferences,
    selected: bool,
    startup_checked: bool,
    checking: bool,
    cover_path: Option<PathBuf>,
    cover_texture: Option<egui::TextureHandle>,
    cover_error: String,
    update_channel: updater::Channel,
    update_candidate: Option<updater::Verified>,
    section: Section,
    creating: bool,
    new_name: String,
    allow_close: bool,
    project: Project,
    path: Option<PathBuf>,
    baseline: Option<m4b_editor_core::ProjectBaseline>,
    backend: DesktopFfmpeg,
    tx: mpsc::Sender<Message>,
    rx: mpsc::Receiver<Message>,
    busy: bool,
    exporting: bool,
    cancel: Cancel,
    progress: f32,
    logs: Vec<String>,
    status: String,
    last_save: Instant,
    dirty: bool,
    recovery: Option<PathBuf>,
    bitrate_available: Option<bool>,
    playback: Playback,
    preview: playback::State,
    preview_track: Option<usize>,
    volume: f32,
    seek_ms: u64,
}
impl Default for App {
    fn default() -> Self {
        let (tx, rx) = mpsc::channel();
        let recovery_store = recovery::Recovery::new(&state_directory().join("recovery"))
            .expect("Create recovery directory");
        let mut pending_recoveries =
            recovery::Recovery::discover(&state_directory().join("recovery"));
        if !cfg!(test) {
            if let Some(legacy) = legacy_state_directory()
                .map(|d| d.join("recovery.autosave.json"))
                .filter(|p| p.is_file() && !p.with_extension("resolved").exists())
            {
                pending_recoveries.push(legacy);
            }
        }
        let recovery = pending_recoveries.pop();
        let installer_error = std::fs::File::open(updater::error_log())
            .ok()
            .and_then(|file| {
                use std::io::Read;
                let mut text = String::new();
                file.take(16384).read_to_string(&mut text).ok()?;
                Some(text)
            })
            .filter(|s| !s.trim().is_empty());
        let legacy_preferences = if cfg!(test) {
            None
        } else {
            legacy_state_directory().map(|p| p.join("preferences.json"))
        };
        let prefs = preferences::load_migrated(&preferences_path(), legacy_preferences.as_deref());
        let mut project = Project::default();
        project.settings.language = prefs.language.clone();
        let channel = if prefs.prerelease {
            updater::Channel::Prerelease
        } else {
            updater::Channel::Stable
        };
        Self {
            pending_recoveries,
            restored_from: None,
            recovery_store,
            encoded_preview: None,
            archive_password: String::new(),
            selected_stream: 0,
            stream_track: None,
            stream_choices: Vec::new(),
            prefs,
            selected: false,
            startup_checked: false,
            checking: false,
            cover_path: None,
            cover_texture: None,
            cover_error: String::new(),
            update_channel: channel,
            update_candidate: None,
            section: Section::Tracks,
            creating: false,
            new_name: String::new(),
            allow_close: false,
            project,
            path: None,
            baseline: None,
            backend: DesktopFfmpeg::default(),
            tx,
            rx,
            busy: false,
            exporting: false,
            cancel: Arc::new(AtomicBool::new(false)),
            progress: 0.,
            logs: installer_error.clone().into_iter().collect(),
            status: installer_error.unwrap_or_default(),
            last_save: Instant::now(),
            dirty: false,
            recovery: recovery.filter(|p| p.exists()),
            bitrate_available: None,
            playback: Playback::new(),
            preview: playback::State::default(),
            preview_track: None,
            volume: 1.,
            seek_ms: 0,
        }
    }
}
fn legacy_state_directory() -> Option<PathBuf> {
    let root = std::env::var_os("LOCALAPPDATA")
        .or_else(|| std::env::var_os("XDG_STATE_HOME"))
        .map(PathBuf::from)
        .or_else(|| std::env::var_os("HOME").map(|h| PathBuf::from(h).join(".local/state")))?;
    Some(root.join("m4b-Craft")) // 0.3 compatibility: preserve originals.
}
#[cfg(test)]
fn state_directory() -> PathBuf {
    static ROOT: std::sync::OnceLock<PathBuf> = std::sync::OnceLock::new();
    ROOT.get_or_init(|| tempfile::tempdir().unwrap().keep())
        .clone()
}
#[cfg(not(test))]
fn state_directory() -> PathBuf {
    let directory = legacy_state_directory()
        .unwrap_or_else(std::env::temp_dir)
        .with_file_name("m4b-editor");
    let _ = std::fs::create_dir_all(&directory);
    directory
}
fn preferences_path() -> PathBuf {
    state_directory().join("preferences.json")
}

impl App {
    fn check_updates(&mut self) {
        self.checking = true;
        let tx = self.tx.clone();
        let channel = self.update_channel;
        std::thread::spawn(move || {
            let _ = tx.send(Message::UpdateChecked(
                channel,
                updater::check(channel, false).map_err(|e| format!("Update check: {e:#}")),
            ));
        });
    }
    fn update_settings(&mut self, ui: &mut egui::Ui) {
        ui.label(self.text("GitHub updates are checked automatically at startup."));
        let previous = self.update_channel;
        ui.horizontal(|ui| {
            ui.selectable_value(&mut self.update_channel, updater::Channel::Stable, "Stable");
            ui.selectable_value(
                &mut self.update_channel,
                updater::Channel::Prerelease,
                catalog::text(
                    self.project.settings.language == "fr",
                    "Include prereleases",
                ),
            );
        });
        if previous != self.update_channel {
            self.update_candidate = None;
            self.prefs.prerelease = self.update_channel == updater::Channel::Prerelease;
            let _ = self.prefs.save(&preferences_path());
        }
        if ui
            .add_enabled(
                !self.checking && !self.busy && !self.playback.is_busy(),
                egui::Button::new(self.text("Check GitHub Releases")),
            )
            .clicked()
        {
            self.check_updates();
        }
    }
    fn remember(&mut self) {
        if let Some(path) = self.path.clone() {
            self.prefs.remember(path);
        }
        let _ = self.prefs.save(&preferences_path());
    }
    fn load_path(&mut self, path: PathBuf) {
        let is_project = path
            .extension()
            .is_some_and(|s| s.eq_ignore_ascii_case("json") || s.eq_ignore_ascii_case("m4bproj"));
        let project_path = if is_project { Some(path.clone()) } else { None };
        self.cancel = Arc::new(AtomicBool::new(false));
        let mut b = self.backend.clone();
        b.job = m4b_editor_core::Job::new(self.cancel.clone(), Duration::from_secs(120));
        let tx = self.tx.clone();
        self.busy = true;
        std::thread::spawn(move || {
            if is_project {
                let result = Project::load_with_baseline(&path).map_err(|e| format!("{e:#}"));
                let _ = tx.send(Message::LoadedProject(path, Box::new(result)));
            } else {
                let result = b.open(&path).map_err(|e| format!("{e:#}"));
                let _ = tx.send(Message::Loaded(Box::new(result), Some(project_path)));
            }
        });
    }
    fn confirm_replace(&mut self) -> bool {
        if !self.dirty {
            return true;
        }
        let choice = rfd::MessageDialog::new()
            .set_title(self.text("Unsaved project"))
            .set_description(self.text("Save changes before continuing?"))
            .set_buttons(rfd::MessageButtons::YesNoCancel)
            .show();
        if choice == rfd::MessageDialogResult::Yes {
            self.save(false);
        }
        replacement_allowed(choice, !self.dirty)
    }

    fn navigate_preview(&mut self, next: bool) {
        let Some(index) = self.preview_track else {
            return;
        };
        if self.project.tracks.len() == 1 && !self.project.chapters.is_empty() {
            let position = self.preview.position_ms;
            let target = if next {
                self.project
                    .chapters
                    .iter()
                    .find(|c| c.start_ms > position)
                    .map(|c| c.start_ms)
            } else {
                self.project
                    .chapters
                    .iter()
                    .rev()
                    .find(|c| c.start_ms < position.saturating_sub(1000))
                    .map(|c| c.start_ms)
                    .or(Some(0))
            };
            if let Some(ms) = target {
                self.play_preview(0, ms);
            }
        } else {
            let target = if next {
                index
                    .checked_add(1)
                    .filter(|i| *i < self.project.tracks.len())
            } else {
                index.checked_sub(1)
            };
            if let Some(i) = target {
                self.play_preview(i, 0);
            }
        }
    }
    fn stop_preview(&mut self) {
        self.playback.send(AudioCommand::Stop);
        self.preview_track = None;
        self.preview = playback::State::default();
    }
    fn play_preview(&mut self, index: usize, position: u64) {
        if let Some(track) = self.project.tracks.get(index) {
            self.playback.send(AudioCommand::Play(
                track.path.clone(),
                position.min(track.duration_ms),
                track.stream_index,
            ));
            self.preview_track = Some(index);
            self.seek_ms = position.min(track.duration_ms);
        }
    }
    fn text(&self, key: &str) -> &'static str {
        catalog::text(self.project.settings.language == "fr", key)
    }
    fn log(&mut self, s: String) {
        let translated = catalog::message(self.project.settings.language == "fr", &s);
        self.status =
            if translated != s || catalog::CATALOG.iter().any(|(en, fr)| *en == s || *fr == s) {
                translated.clone()
            } else {
                self.text("Operation details").into()
            };
        self.logs.push(translated);
        if self.logs.len() > 500 {
            self.logs.remove(0);
        }
    }
    fn log_error(&mut self, diagnostic: String) {
        self.log(diagnostic.clone());
        self.status = catalog::error(self.project.settings.language == "fr", &diagnostic).into();
    }
    fn save(&mut self, choose: bool) {
        if !self.selected {
            return;
        }
        let path = if choose || self.path.is_none() {
            rfd::FileDialog::new()
                .add_filter(self.text("M4B Editor project"), &["m4bproj"])
                .set_file_name("book.m4bproj")
                .save_file()
                .and_then(|p| {
                    let target = project_save_target(&p);
                    let exists = target.exists();
                    let consent = !exists
                        || rfd::MessageDialog::new()
                            .set_title(self.text("Overwrite project"))
                            .set_description(format!(
                                "{} {} ?",
                                self.text("Replace"),
                                target.display()
                            ))
                            .set_buttons(rfd::MessageButtons::YesNo)
                            .show()
                            == rfd::MessageDialogResult::Yes;
                    overwrite_allowed(exists, consent).then_some(target)
                })
        } else {
            self.path.clone()
        };
        if let Some(path) = path {
            let previous_slot = self
                .recovery_store
                .slot(self.path.as_deref(), &self.project);
            let expected = if choose || self.path.is_none() {
                None
            } else {
                self.baseline.as_ref()
            };
            match self.project.save_with_baseline(&path, expected) {
                Ok(baseline) => {
                    self.baseline = Some(baseline);
                    let _ = std::fs::remove_file(previous_slot);
                    if let Some(restored) = self.restored_from.take() {
                        let _ = std::fs::write(
                            restored.with_extension("resolved"),
                            b"Recovered project saved successfully; original retained",
                        );
                    }
                    self.path = Some(path);
                    self.remember();
                    self.dirty = false;
                    {
                        let recovery = self
                            .recovery_store
                            .slot(self.path.as_deref(), &self.project);
                        let _ = std::fs::remove_file(recovery);
                    }
                    self.log(self.text("Project saved").into());
                }
                Err(e) => self.log_error(format!("{}: {e:#}", self.text("Save error"))),
            }
        }
    }
    fn import(&mut self) {
        if !self.selected {
            return;
        }
        if self.path.is_none() {
            if !self.confirm_replace() {
                return;
            }
            self.creating = true;
            self.new_name = self.project.metadata.title.clone();
            self.log(
                self.text("Create a named project first, then import")
                    .into(),
            );
            return;
        }
        let Some(project_path) = self.path.clone() else {
            return;
        };
        let assets = project_path.with_extension("assets");
        if let Some(paths) = rfd::FileDialog::new()
            .add_filter(
                self.text("Audio"),
                &[
                    "mka", "mkv", "mp4", "m4b", "m4a", "mp3", "flac", "ogg", "opus", "wav", "aac",
                    "zip", "rar", "7z",
                ],
            )
            .pick_files()
        {
            let mut p = self.project.clone();
            self.cancel = Arc::new(AtomicBool::new(false));
            let mut b = self.backend.clone();
            b.job = m4b_editor_core::Job::new(self.cancel.clone(), Duration::from_secs(600));
            let password = std::mem::take(&mut self.archive_password);
            let tx = self.tx.clone();
            self.busy = true;
            std::thread::spawn(move || {
                let result = p
                    .import_assets_with(
                        &b,
                        paths,
                        &assets,
                        &b.job,
                        (!password.is_empty()).then_some(password.as_str()),
                    )
                    .map(|_| p)
                    .map_err(|e| format!("{e:#}"));
                let _ = tx.send(Message::Loaded(Box::new(result), None));
            });
        }
    }
    fn open(&mut self) {
        if !self.confirm_replace() {
            return;
        }
        if let Some(path) = rfd::FileDialog::new()
            .add_filter(
                self.text("Project"),
                &[
                    "m4bproj", "json", "m4b", "m4a", "mp3", "flac", "mka", "wav", "ogg", "opus",
                ],
            )
            .pick_file()
        {
            self.load_path(path);
        }
    }
    fn encode_preview(&mut self, transition: bool) {
        let track_index = self
            .preview_track
            .unwrap_or(0)
            .min(self.project.tracks.len().saturating_sub(1));
        let book_position = self
            .project
            .tracks
            .iter()
            .take(track_index)
            .map(|t| t.duration_ms)
            .sum::<u64>()
            .saturating_add(self.seek_ms);
        let boundary = if self.project.tracks.len() > 1 {
            self.project
                .tracks
                .iter()
                .take(track_index.max(1))
                .map(|t| t.duration_ms)
                .sum::<u64>()
        } else {
            self.project
                .chapters
                .iter()
                .find(|c| c.start_ms > book_position)
                .map(|c| c.start_ms)
                .unwrap_or(book_position)
        };
        let start = if transition {
            boundary.saturating_sub(3000)
        } else {
            book_position.min(self.project.duration_ms().saturating_sub(1))
        };
        self.stop_preview();
        let directory = match tempfile::tempdir() {
            Ok(d) => d,
            Err(e) => {
                self.log(e.to_string());
                return;
            }
        };
        let output = directory.path().join(format!(
            "preview.{}",
            self.project.settings.format.extension()
        ));
        let p = self.project.clone();
        let mut b = self.backend.clone();
        let tx = self.tx.clone();
        self.cancel = Arc::new(AtomicBool::new(false));
        let cancel = self.cancel.clone();
        b.job = m4b_editor_core::Job::new(cancel.clone(), Duration::from_secs(600));
        self.busy = true;
        self.exporting = true;
        std::thread::spawn(move || {
            let result = b
                .encoded_preview(
                    &p,
                    &output,
                    start,
                    if transition { 6000 } else { 10000 },
                    cancel,
                    &mut |e| {
                        let _ = tx.send(Message::Event(e));
                    },
                )
                .map(|_| output)
                .map_err(|e| format!("{e:#}"));
            let _ = tx.send(Message::Preview(directory, result));
        });
    }
    fn export(&mut self) {
        if !self.selected {
            return;
        }
        if let Err(e) = self.project.validate() {
            self.log_error(format!("{}: {e:#}", self.text("Validation")));
            return;
        }
        if let Some(output) = rfd::FileDialog::new()
            .add_filter(
                self.text("Audio"),
                &[self.project.settings.format.extension()],
            )
            .set_file_name(format!(
                "audiobook.{}",
                self.project.settings.format.extension()
            ))
            .save_file()
        {
            self.stop_preview();
            self.busy = true;
            self.exporting = true;
            self.progress = 0.;
            self.cancel = Arc::new(AtomicBool::new(false));
            let cancel = self.cancel.clone();
            let p = self.project.clone();
            let mut b = self.backend.clone();
            b.job = m4b_editor_core::Job::new(cancel.clone(), Duration::from_secs(86400));
            let tx = self.tx.clone();
            std::thread::spawn(move || {
                let result = b
                    .export(&p, &output, cancel, &mut |e| {
                        let _ = tx.send(Message::Event(e));
                    })
                    .map_err(|e| format!("{e:#}"));
                let _ = tx.send(Message::Done(result));
            });
        }
    }
}
impl App {
    fn render(&mut self, ctx: &egui::Context) {
        while let Ok(state) = self.playback.rx.try_recv() {
            self.preview = state;
        }
        while let Ok(m) = self.rx.try_recv() {
            match m {
                Message::Copied(result) => {
                    self.busy = false;
                    match result {
                        Ok(path) => self.log(format!(
                            "{}: {}",
                            self.text("Project copied"),
                            path.display()
                        )),
                        Err(e) => self.log_error(e),
                    }
                }
                Message::Streams(index, result) => {
                    self.busy = false;
                    match result {
                        Ok(streams) => {
                            self.stream_track = Some(index);
                            self.stream_choices = streams;
                        }
                        Err(e) => self.log_error(e),
                    }
                }
                Message::Preview(directory, result) => {
                    self.busy = false;
                    self.exporting = false;
                    match result {
                        Ok(path) => {
                            self.encoded_preview = Some(directory);
                            self.preview_track = None;
                            self.playback.send(AudioCommand::Play(path, 0, None));
                        }
                        Err(e) => self.log_error(e),
                    }
                }
                Message::Cover(path, result) => {
                    if self.cover_path.as_ref() == Some(&path) {
                        match result {
                            Ok(image) => {
                                self.cover_texture =
                                    Some(ctx.load_texture("cover", image, Default::default()))
                            }
                            Err(e) => self.cover_error = e,
                        }
                    }
                }
                Message::Encoders(result) => {
                    self.busy = false;
                    match result {
                        Ok(v) => self.bitrate_available = Some(v),
                        Err(e) => self.log_error(e),
                    }
                }
                Message::UpdateProgress(p) => self.progress = p,
                Message::UpdateChecked(channel, result) => {
                    self.checking = false;
                    if channel != self.update_channel {
                        continue;
                    }
                    match result {
                        Ok(update) => {
                            self.status = format!(
                                "{} {}",
                                self.text("Verified update"),
                                update.manifest.version
                            );
                            self.update_candidate = Some(update);
                        }
                        Err(error) => self.log_error(error),
                    }
                }
                Message::UpdateDownloaded(result) => {
                    self.busy = false;
                    match result {
                        Ok(download) => {
                            self.status = self.text("Preparing verified installer").into();
                            let tx = self.tx.clone();
                            self.busy = true;
                            std::thread::spawn(move || {
                                let _ = tx.send(Message::UpdateInstalled(
                                    download.install(false, false).map_err(|e| format!("{e:#}")),
                                ));
                            });
                        }
                        Err(error) => self.log_error(error),
                    }
                }
                Message::UpdateInstalled(result) => {
                    self.busy = false;
                    match result {
                        Ok(()) => {
                            self.stop_preview();
                            self.allow_close = true;
                            ctx.send_viewport_cmd(egui::ViewportCommand::Close);
                        }
                        Err(error) => self.log_error(error),
                    }
                }

                Message::LoadedProject(path, result) => match *result {
                    Ok((project, baseline)) => {
                        self.baseline = Some(baseline);
                        let _ = self
                            .tx
                            .send(Message::Loaded(Box::new(Ok(project)), Some(Some(path))));
                    }
                    Err(error) => {
                        self.busy = false;
                        self.log_error(error);
                    }
                },
                Message::Loaded(r, path) => {
                    self.busy = false;
                    match *r {
                        Ok(p) => {
                            self.stop_preview();
                            self.project = p;
                            self.stream_track = None;
                            self.stream_choices.clear();
                            self.cover_path = None;
                            self.cover_texture = None;
                            self.project.settings.language = self.prefs.language.clone();
                            self.selected = true;
                            let clean = matches!(&path, Some(Some(_)));
                            if let Some(path) = path {
                                if path.is_none() {
                                    self.baseline = None;
                                }
                                self.path = path;
                            }
                            self.remember();
                            self.dirty = !clean;
                            self.log(self.text("Loaded").into());
                        }
                        Err(e) => self.log_error(e),
                    }
                }
                Message::Event(ExportEvent::Log(s)) => self.log(s),
                Message::Event(ExportEvent::Progress(p)) => self.progress = p,
                Message::Done(r) => {
                    self.busy = false;
                    self.exporting = false;
                    match r {
                        Ok(()) => self.log(self.text("Export complete").into()),
                        Err(e) => self.log_error(e),
                    }
                }
            }
        }
        if self.busy && ctx.input(|i| i.viewport().close_requested()) {
            ctx.send_viewport_cmd(egui::ViewportCommand::CancelClose);
            self.cancel.store(true, Ordering::Relaxed);
            self.status = if self.exporting {
                self.text("Waiting for media job; export cancellation requested. Close again when finished.")
            } else {
                self.text("Waiting for background operation. Close again when finished.")
            }
            .into();
        }
        if !self.busy && !self.allow_close && ctx.input(|i| i.viewport().close_requested()) {
            ctx.send_viewport_cmd(egui::ViewportCommand::CancelClose);
            if self.confirm_replace() {
                self.allow_close = true;
                self.dirty = false;
                ctx.send_viewport_cmd(egui::ViewportCommand::Close);
            }
        }
        if !self.startup_checked && !self.busy && !self.playback.is_busy() {
            self.startup_checked = true;
            self.check_updates();
        }
        if self.update_candidate.is_some() && !self.busy && !self.playback.is_busy() {
            let version = self
                .update_candidate
                .as_ref()
                .unwrap()
                .manifest
                .version
                .clone();
            egui::Window::new(self.text("Update available"))
                .collapsible(false)
                .show(ctx, |ui| {
                    ui.label(format!("{} → {}", env!("CARGO_PKG_VERSION"), version));
                    if ui.button(self.text("Later")).clicked() {
                        self.update_candidate = None;
                    }
                    if ui.button(self.text("Update")).clicked() {
                        if self.dirty {
                            self.save(false);
                        }
                        if updater::guard(self.busy || self.playback.is_busy(), self.dirty, true)
                            .is_ok()
                        {
                            if let Some(update) = self
                                .update_candidate
                                .take()
                                .filter(|u| u.matches_channel(self.update_channel))
                            {
                                self.busy = true;
                                self.progress = 0.;
                                self.status = self.text("Downloading").into();
                                let tx = self.tx.clone();
                                std::thread::spawn(move || {
                                    let progress_tx = tx.clone();
                                    let result = update
                                        .download_progress(false, |p| {
                                            let _ = progress_tx.send(Message::UpdateProgress(p));
                                        })
                                        .map_err(|e| format!("Update download: {e:#}"));
                                    let _ = tx.send(Message::UpdateDownloaded(result));
                                });
                            }
                        }
                    }
                });
        }
        if self.cover_path != self.project.cover {
            self.cover_path = self.project.cover.clone();
            self.cover_texture = None;
            self.cover_error.clear();
            if let Some(path) = self.cover_path.clone() {
                let tx = self.tx.clone();
                let backend = self.backend.clone();
                std::thread::spawn(move || {
                    let result = load_cover(&backend, &path)
                        .map_err(|e| format!("Unreadable cover / Couverture illisible: {e:#}"));
                    let _ = tx.send(Message::Cover(path, result));
                });
            }
        }
        // Follow OS theme when system is selected; egui polls native theme changes.
        match self.project.settings.theme.as_str() {
            "dark" => ctx.set_theme(egui::ThemePreference::Dark),
            "light" => ctx.set_theme(egui::ThemePreference::Light),
            _ => ctx.set_theme(egui::ThemePreference::System),
        }
        if self.dirty && self.last_save.elapsed() > Duration::from_secs(15) {
            {
                let path = self
                    .recovery_store
                    .slot(self.path.as_deref(), &self.project);
                if let Err(e) = self.project.save(&path) {
                    self.log(format!("{}: {e:#}", self.text("Autosave")));
                }
            }
            self.last_save = Instant::now();
        }
        egui::TopBottomPanel::top("menu").show(ctx, |ui| {
            egui::menu::bar(ui, |ui| {
                ui.add_enabled_ui(!self.busy && !self.creating, |ui| {
                    ui.menu_button(self.text("Project"), |ui| {
                        if ui.button(self.text("New…")).clicked() {
                            if self.confirm_replace() {
                                self.creating = true;
                                self.new_name.clear();
                            }
                            ui.close_menu();
                        }
                        if ui.button(self.text("Open…")).clicked() {
                            self.open();
                            ui.close_menu();
                        }
                        if ui.button(self.text("Save")).clicked() {
                            self.save(false);
                            ui.close_menu();
                        }
                        if ui.button(self.text("Save as…")).clicked() {
                            self.save(true);
                            ui.close_menu();
                        }
                        if ui.button(self.text("Copy portable project…")).clicked() {
                            if let Some(parent) = rfd::FileDialog::new().pick_folder() {
                                let safe_name: String = self
                                    .project
                                    .name
                                    .chars()
                                    .map(|c| if c.is_alphanumeric() { c } else { '-' })
                                    .collect();
                                let destination = parent.join(format!("portable-{safe_name}"));
                                let project = self.project.clone();
                                let tx = self.tx.clone();
                                self.cancel = Arc::new(AtomicBool::new(false));
                                let job = m4b_editor_core::Job::new(
                                    self.cancel.clone(),
                                    Duration::from_secs(3600),
                                );
                                self.busy = true;
                                std::thread::spawn(move || {
                                    let _ = tx.send(Message::Copied(
                                        project
                                            .copy_to_with_job(&destination, &job)
                                            .map_err(|e| e.to_string()),
                                    ));
                                });
                            }
                            ui.close_menu();
                        }
                        ui.separator();
                        if ui.button(self.text("Import…")).clicked() {
                            self.import();
                            ui.close_menu();
                        }
                        if ui.button(self.text("Export audio…")).clicked() {
                            self.section = Section::Export;
                            self.export();
                            ui.close_menu();
                        }
                    });
                });
                if ui.button(self.text("Settings")).clicked() {
                    self.section = Section::Settings;
                }
                if self.selected
                    && ui.button(self.text("Close project")).clicked()
                    && !self.busy
                    && self.confirm_replace()
                {
                    self.stop_preview();
                    self.selected = false;
                    self.project = Project::default();
                    self.project.settings.language = self.prefs.language.clone();
                    self.path = None;
                    self.dirty = false;
                }
            });
            ui.label(format!(
                "{}{}",
                if self.project.name.is_empty() {
                    "M4B Editor"
                } else {
                    &self.project.name
                },
                if self.dirty { " *" } else { "" }
            ));
            if self.busy && ui.button(self.text("Cancel operation")).clicked() {
                self.cancel.store(true, Ordering::Relaxed);
            }
            if let Some(recovery) = self.recovery.clone() {
                ui.horizontal(|ui| {
                    ui.label(self.text("Recovery available"));
                    if ui
                        .add_enabled(
                            !self.busy && !self.creating,
                            egui::Button::new(self.text("Restore")),
                        )
                        .clicked()
                    {
                        let recovered = Project::load(&recovery);
                        if !self.confirm_replace() {
                            return;
                        }
                        match recovered {
                            Ok(p) => {
                                self.stop_preview();
                                self.project = p;
                                self.selected = true;
                                self.path = None;
                                self.baseline = None;
                                self.dirty = true;
                                self.restored_from = Some(recovery.clone());
                                self.recovery = self.pending_recoveries.pop();
                            }
                            Err(e) => self.log_error(e.to_string()),
                        }
                    }
                    if ui.button(self.text("Dismiss")).clicked() {
                        self.recovery = self.pending_recoveries.pop();
                    }
                });
            }
        });
        egui::TopBottomPanel::bottom("status").show(ctx, |ui| {
            ui.add(egui::ProgressBar::new(self.progress).show_percentage());
            ui.label(&self.status);
            egui::CollapsingHeader::new(self.text("Logs / errors")).show(ui, |ui| {
                egui::ScrollArea::vertical()
                    .max_height(140.)
                    .stick_to_bottom(true)
                    .show(ui, |ui| {
                        for log in &self.logs {
                            ui.label(log);
                        }
                    });
            });
        });
        if !self.busy && (self.preview.active || self.preview.paused) {
            egui::TopBottomPanel::bottom("transport").show(ctx, |ui| {
                ui.horizontal(|ui| {
                    if ui.button("⏮").clicked() {
                        self.navigate_preview(false);
                    }
                    if ui
                        .button(if self.preview.paused { "▶" } else { "⏸" })
                        .clicked()
                    {
                        self.playback
                            .send(AudioCommand::Pause(!self.preview.paused));
                    }
                    if ui.button("⏹").clicked() {
                        self.stop_preview();
                    }
                    if ui.button("⏭").clicked() {
                        self.navigate_preview(true);
                    }
                    if let Some(index) = self.preview_track {
                        if let Some(track) = self.project.tracks.get(index) {
                            let title = if self.project.tracks.len() == 1 {
                                self.project
                                    .chapters
                                    .iter()
                                    .rev()
                                    .find(|c| c.start_ms <= self.preview.position_ms)
                                    .map(|c| c.title.clone())
                            } else {
                                None
                            }
                            .unwrap_or_else(|| {
                                track
                                    .path
                                    .file_name()
                                    .unwrap_or_default()
                                    .to_string_lossy()
                                    .into_owned()
                            });
                            ui.label(title);
                            ui.label(format!(
                                "{} / {}",
                                playback::clock(self.preview.position_ms),
                                playback::clock(track.duration_ms)
                            ));
                            let response = ui.add(
                                egui::Slider::new(&mut self.seek_ms, 0..=track.duration_ms)
                                    .show_value(false),
                            );
                            if response.drag_stopped()
                                || (response.changed() && !response.dragged())
                            {
                                self.play_preview(index, self.seek_ms);
                            } else if !response.dragged() {
                                self.seek_ms = self.preview.position_ms;
                            }
                        }
                    }
                });
            });
        }
        if self.creating {
            egui::Window::new(self.text("New project"))
                .collapsible(false)
                .show(ctx, |ui| {
                    ui.label(self.text("Project name"));
                    ui.text_edit_singleline(&mut self.new_name);
                    if ui
                        .add_enabled(
                            !self.new_name.trim().is_empty(),
                            egui::Button::new(self.text("Create…")),
                        )
                        .clicked()
                    {
                        if let Some(path) = rfd::FileDialog::new()
                            .add_filter("Project", &["m4bproj"])
                            .set_file_name("book.m4bproj")
                            .save_file()
                            .and_then(|p| {
                                let target = project_save_target(&p);
                                let exists = target.exists();
                                let consent = !exists
                                    || rfd::MessageDialog::new()
                                        .set_title(self.text("Overwrite project"))
                                        .set_description(format!(
                                            "{} {} ?",
                                            self.text("Replace"),
                                            target.display()
                                        ))
                                        .set_buttons(rfd::MessageButtons::YesNo)
                                        .show()
                                        == rfd::MessageDialogResult::Yes;
                                overwrite_allowed(exists, consent).then_some(target)
                            })
                        {
                            let mut project =
                                Project::named(&self.new_name).expect("validated project name");
                            project.settings = self.project.settings.clone();
                            project.settings.bitrate_kbps = 160;
                            match project.save_new(&path) {
                                Ok(()) => {
                                    self.stop_preview();
                                    self.project = project;
                                    self.stream_track = None;
                                    self.stream_choices.clear();
                                    self.selected = true;
                                    self.path = Some(path);
                                    self.remember();
                                    self.dirty = false;
                                    self.creating = false;
                                    self.section = Section::Tracks;
                                }
                                Err(e) => {
                                    self.log(format!("{}: {e:#}", self.text("Create project")))
                                }
                            }
                        }
                    }
                    if ui.button(self.text("Cancel")).clicked() {
                        self.creating = false;
                    }
                });
        }
        if !self.selected {
            egui::CentralPanel::default().show(ctx, |ui| {
                ui.heading(self.text("Welcome to M4B Editor"));
                ui.add_enabled_ui(!self.busy && !self.creating, |ui| {
                    if ui.button(self.text("Create project…")).clicked() {
                        self.creating = true;
                    }
                    if ui.button(self.text("Open project…")).clicked() {
                        self.open();
                    }
                    if self.section == Section::Settings {
                        self.update_settings(ui);
                        ui.horizontal(|ui| {
                            ui.label(catalog::text(
                                self.project.settings.language == "fr",
                                "Language",
                            ));
                            ui.selectable_value(&mut self.prefs.language, "en".into(), "English");
                            ui.selectable_value(&mut self.prefs.language, "fr".into(), "Français");
                        });
                        self.project.settings.language = self.prefs.language.clone();
                        let _ = self.prefs.save(&preferences_path());
                    }
                    ui.heading(self.text("Recent projects"));
                    for path in self.prefs.recent.clone() {
                        ui.horizontal(|ui| {
                            if ui.small_button("×").clicked() {
                                self.prefs.remove(&path);
                                let _ = self.prefs.save(&preferences_path());
                            }
                            if ui
                                .add_enabled(
                                    path.is_file(),
                                    egui::Button::new(path.display().to_string()),
                                )
                                .clicked()
                            {
                                self.load_path(path);
                            }
                        });
                    }
                });
            });
            ctx.request_repaint_after(Duration::from_millis(100));
            return;
        }
        let sidebar = egui::SidePanel::left("navigation")
            .resizable(true)
            .default_width(self.prefs.sidebar)
            .width_range(120.0..=400.0)
            .show(ctx, |ui| {
                for (section, en) in [
                    (Section::Tracks, "Tracks"),
                    (Section::Metadata, "Metadata"),
                    (Section::Chapters, "Chapters"),
                    (Section::Export, "Export"),
                    (Section::Settings, "Settings"),
                ] {
                    let label = self.text(en);
                    ui.selectable_value(&mut self.section, section, label);
                }
            });
        self.prefs.sidebar = sidebar.response.rect.width();
        egui::CentralPanel::default().show(ctx, |ui| {
            egui::ScrollArea::vertical().show(ui, |ui| {
                ui.add_enabled_ui(!self.busy && !self.creating, |ui| {
                    let before = format!("{:?}", self.project);
                    let fr = self.project.settings.language == "fr";
                    if self.section == Section::Settings {
                        ui.heading(format!("M4B Editor {}", env!("CARGO_PKG_VERSION")));
                        self.update_settings(ui);
                        ui.horizontal(|ui| {
                            ui.label(catalog::text(
                                self.project.settings.language == "fr",
                                "Language",
                            ));
                            egui::ComboBox::from_id_salt("language")
                                .selected_text(&self.project.settings.language)
                                .show_ui(ui, |ui| {
                                    ui.selectable_value(
                                        &mut self.project.settings.language,
                                        "en".into(),
                                        "English",
                                    );
                                    ui.selectable_value(
                                        &mut self.project.settings.language,
                                        "fr".into(),
                                        "Français",
                                    );
                                });
                            ui.label(self.text("Theme"));
                            egui::ComboBox::from_id_salt("theme")
                                .selected_text(catalog::text(
                                    fr,
                                    match self.project.settings.theme.as_str() {
                                        "dark" => "Dark",
                                        "light" => "Light",
                                        _ => "System",
                                    },
                                ))
                                .show_ui(ui, |ui| {
                                    for t in ["system", "light", "dark"] {
                                        ui.selectable_value(
                                            &mut self.project.settings.theme,
                                            t.into(),
                                            catalog::text(
                                                fr,
                                                match t {
                                                    "system" => "System",
                                                    "light" => "Light",
                                                    _ => "Dark",
                                                },
                                            ),
                                        );
                                    }
                                });
                        });
                        ui.separator();
                    }
                    if self.section == Section::Metadata {
                        ui.heading(self.text("Book metadata"));
                        egui::Grid::new("metadata").num_columns(2).show(ui, |ui| {
                            for (label, value) in [
                                (catalog::text(fr, "Title"), &mut self.project.metadata.title),
                                (
                                    catalog::text(fr, "Author"),
                                    &mut self.project.metadata.author,
                                ),
                                (
                                    catalog::text(fr, "Narrator"),
                                    &mut self.project.metadata.narrator,
                                ),
                            ] {
                                ui.label(label);
                                ui.text_edit_singleline(value);
                                ui.end_row();
                            }
                            for (label, value) in [
                                (
                                    catalog::text(fr, "Publisher"),
                                    &mut self.project.metadata.publisher,
                                ),
                                (
                                    catalog::text(fr, "Book language (ISO 639-2)"),
                                    &mut self.project.metadata.book_language,
                                ),
                                ("Genre", &mut self.project.metadata.genre),
                                ("Date", &mut self.project.metadata.date),
                                (
                                    catalog::text(fr, "Copyright"),
                                    &mut self.project.metadata.copyright,
                                ),
                            ] {
                                ui.label(label);
                                ui.text_edit_singleline(value);
                                ui.end_row();
                            }
                            ui.label(self.text("Description"));
                            ui.text_edit_multiline(&mut self.project.metadata.description);
                            ui.end_row();
                        });
                        ui.horizontal(|ui| {
                            if ui.button(self.text("Choose cover…")).clicked() {
                                if let Some(p) = rfd::FileDialog::new()
                                    .add_filter(self.text("Image"), &["jpg", "jpeg", "png"])
                                    .pick_file()
                                {
                                    self.project.cover = Some(p);
                                    self.cover_path = None;
                                    self.cover_texture = None;
                                }
                            }
                            if ui.button(self.text("Remove cover")).clicked() {
                                self.project.cover = None;
                            }
                            ui.label(
                                self.project
                                    .cover
                                    .as_ref()
                                    .map(|p| p.display().to_string())
                                    .unwrap_or_else(|| self.text("No cover").into()),
                            );
                        });
                        if let Some(texture) = &self.cover_texture {
                            ui.add(
                                egui::Image::new(texture)
                                    .max_size(egui::vec2(280., 280.))
                                    .maintain_aspect_ratio(true),
                            );
                        } else if self.project.cover.is_none() {
                            ui.label(self.text("No cover"));
                        } else if !self.cover_error.is_empty() {
                            ui.label(self.text("Cover unavailable"))
                                .on_hover_text(&self.cover_error);
                        } else {
                            ui.spinner();
                        }
                        ui.separator();
                    }
                    if self.section == Section::Tracks {
                        ui.horizontal(|ui| {
                            ui.label(catalog::text(fr, "Archive password (not saved)"));
                            ui.add(
                                egui::TextEdit::singleline(&mut self.archive_password)
                                    .password(true),
                            );
                        });
                        ui.horizontal(|ui| {
                            ui.label(catalog::text(fr, "Absolute audio stream index"));
                            if ui
                                .button(catalog::text(fr, "Inspect audio streams"))
                                .clicked()
                            {
                                if let Some(index) = self.preview_track {
                                    let path = self.project.tracks[index].path.clone();
                                    let mut backend = self.backend.clone();
                                    let tx = self.tx.clone();
                                    self.cancel = Arc::new(AtomicBool::new(false));
                                    backend.job = m4b_editor_core::Job::new(
                                        self.cancel.clone(),
                                        Duration::from_secs(120),
                                    );
                                    self.busy = true;
                                    std::thread::spawn(move || {
                                        let _ = tx.send(Message::Streams(
                                            index,
                                            backend.audio_streams(&path).map_err(|e| e.to_string()),
                                        ));
                                    });
                                }
                            }
                            egui::ComboBox::from_id_salt("audio-stream")
                                .selected_text(self.selected_stream.to_string())
                                .show_ui(ui, |ui| {
                                    for stream in &self.stream_choices {
                                        ui.selectable_value(
                                            &mut self.selected_stream,
                                            stream.index,
                                            format!(
                                                "{}: {} · {} Hz · {} ch · {}",
                                                stream.index,
                                                stream.codec,
                                                stream.sample_rate,
                                                stream.channels,
                                                stream.language
                                            ),
                                        );
                                    }
                                });
                            if ui
                                .button(catalog::text(fr, "Apply selected stream"))
                                .clicked()
                            {
                                if let Some(index) = self.stream_track {
                                    let mut project = self.project.clone();
                                    let mut backend = self.backend.clone();
                                    self.cancel = Arc::new(AtomicBool::new(false));
                                    backend.job = m4b_editor_core::Job::new(
                                        self.cancel.clone(),
                                        Duration::from_secs(120),
                                    );
                                    let selected = self.selected_stream;
                                    let tx = self.tx.clone();
                                    self.busy = true;
                                    self.stop_preview();
                                    std::thread::spawn(move || {
                                        let result = project
                                            .select_stream(&backend, index, selected)
                                            .map(|_| project)
                                            .map_err(|e| e.to_string());
                                        let _ = tx.send(Message::Loaded(Box::new(result), None));
                                    });
                                }
                            }
                        });
                        if ui.button(self.text("Import audio / archives…")).clicked() {
                            self.import();
                        }
                        ui.heading(self.text("Tracks (custom chapters preserved)"));
                        ui.group(|ui| {
                            ui.heading(self.text("Pre-listening"));
                            ui.horizontal(|ui| {
                                if ui
                                    .add_enabled(
                                        self.preview.active,
                                        egui::Button::new(if self.preview.paused {
                                            self.text("Resume")
                                        } else {
                                            self.text("Pause")
                                        }),
                                    )
                                    .clicked()
                                {
                                    self.playback
                                        .send(AudioCommand::Pause(!self.preview.paused));
                                }
                                if ui.button(self.text("Stop")).clicked() {
                                    self.stop_preview();
                                }
                                let label = self.text("Volume");
                                if ui
                                    .add(egui::Slider::new(&mut self.volume, 0.0..=1.0).text(label))
                                    .changed()
                                {
                                    self.playback.send(AudioCommand::Volume(self.volume));
                                }
                            });
                            if !self.preview.message.is_empty() {
                                ui.label(self.text("Playback details"))
                                    .on_hover_text(&self.preview.message);
                            }
                        });
                        let mut play_track = None;
                        let mut movement = None;
                        let mut remove = None;
                        for (i, t) in self.project.tracks.iter().enumerate() {
                            ui.horizontal(|ui| {
                                if ui.add_enabled(i > 0, egui::Button::new("↑")).clicked() {
                                    movement = Some((i, i - 1));
                                }
                                if ui
                                    .add_enabled(
                                        i + 1 < self.project.tracks.len(),
                                        egui::Button::new("↓"),
                                    )
                                    .clicked()
                                {
                                    movement = Some((i, i + 1));
                                }
                                if ui.small_button("×").clicked() {
                                    remove = Some(i);
                                }
                                if ui.small_button(catalog::text(fr, "Play")).clicked() {
                                    play_track = Some(i);
                                }
                                ui.label(format!(
                                    "{} · {:.2}s · {} · {} Hz · {} ch",
                                    t.path.file_name().unwrap_or_default().to_string_lossy(),
                                    t.duration_ms as f64 / 1000.,
                                    t.codec,
                                    t.sample_rate,
                                    t.channels
                                ));
                            });
                        }
                        if let Some(index) = play_track {
                            self.play_preview(index, 0);
                        }
                        if let Some((a, b)) = movement {
                            self.stop_preview();
                            self.stream_track = None;
                            self.stream_choices.clear();
                            let mut order: Vec<_> = (0..self.project.tracks.len()).collect();
                            order.swap(a, b);
                            if let Err(e) = self.project.reorder_tracks(&order) {
                                self.log(e.to_string());
                            }
                        }
                        if let Some(i) = remove {
                            self.stop_preview();
                            self.stream_track = None;
                            self.stream_choices.clear();
                            if let Err(e) = self.project.remove_track(i) {
                                self.log(e.to_string());
                            }
                        }
                        ui.separator();
                    }
                    if self.section == Section::Chapters {
                        ui.heading(self.text("Chapters — start in milliseconds"));
                        let mut chapter_jump = None;
                        let mut delete = None;
                        for (i, c) in self.project.chapters.iter_mut().enumerate() {
                            ui.horizontal(|ui| {
                                if ui.small_button(catalog::text(fr, "Play")).clicked() {
                                    chapter_jump = Some(c.start_ms);
                                }
                                ui.label(format!("{}", i + 1));
                                ui.add(egui::DragValue::new(&mut c.start_ms).speed(100));
                                ui.text_edit_singleline(&mut c.title);
                                if ui.small_button("×").clicked() {
                                    delete = Some(i);
                                }
                            });
                        }
                        if let Some(ms) = chapter_jump {
                            if let Some((index, offset)) = playback::locate(
                                self.project.tracks.iter().map(|t| t.duration_ms),
                                ms,
                            ) {
                                self.play_preview(index, offset);
                            }
                        }
                        if let Some(i) = delete {
                            self.project.chapters.remove(i);
                        }
                        ui.horizontal(|ui| {
                            if ui.button(self.text("Add chapter")).clicked() {
                                let start = self
                                    .project
                                    .chapters
                                    .last()
                                    .map(|c| c.start_ms.saturating_add(1000))
                                    .unwrap_or(0);
                                self.project.chapters.push(Chapter {
                                    start_ms: start,
                                    title: format!(
                                        "{} {}",
                                        self.text("Chapter"),
                                        self.project.chapters.len() + 1
                                    ),
                                });
                            }
                            if ui.button(self.text("One per track")).clicked() {
                                self.project.reset_chapters();
                            }
                        });
                        ui.separator();
                    }
                    if self.section == Section::Export {
                        if ui.button(self.text("Export audio…")).clicked() {
                            self.export();
                        }
                        let previous_format = self.project.settings.format;
                        ui.horizontal(|ui| {
                            for format in [
                                m4b_editor_core::ExportFormat::M4b,
                                m4b_editor_core::ExportFormat::Mp3,
                                m4b_editor_core::ExportFormat::Flac,
                            ] {
                                ui.selectable_value(
                                    &mut self.project.settings.format,
                                    format,
                                    format.extension().to_uppercase(),
                                );
                            }
                        });
                        if previous_format != self.project.settings.format {
                            self.bitrate_available = None;
                        }
                        ui.heading(self.text("Audio export"));
                        if self.project.settings.format == m4b_editor_core::ExportFormat::Flac {
                            ui.label(
                                self.text("FLAC uses lossless compression; bitrate is automatic."),
                            );
                        }
                        if ui.button(self.text("Encoded excerpt (10 s)")).clicked() {
                            self.encode_preview(false);
                        }
                        if ui.button(self.text("Encoded transition (6 s)")).clicked() {
                            self.encode_preview(true);
                        }
                        ui.checkbox(
                            &mut self.project.settings.lossless_remux,
                            catalog::text(fr, "Lossless remux for single AAC M4B/M4A"),
                        );
                        if self.project.can_remux() {
                            ui.label(self.text(
                                "Audio is copied without re-encoding; bitrate is preserved.",
                            ));
                        }
                        ui.horizontal(|ui| {
                            ui.label(self.text("Bitrate (kb/s)"));
                            egui::ComboBox::from_id_salt("bitrate")
                                .selected_text(self.project.settings.bitrate_kbps.to_string())
                                .show_ui(ui, |ui| {
                                    for rate in [48, 64, 96, 128, 160, 192, 256, 320] {
                                        ui.selectable_value(
                                            &mut self.project.settings.bitrate_kbps,
                                            rate,
                                            rate.to_string(),
                                        );
                                    }
                                });
                            if ui.button(self.text("Check encoder")).clicked() {
                                self.busy = true;
                                let backend = self.backend.clone();
                                let encoder = self.project.settings.format.encoder();
                                let tx = self.tx.clone();
                                std::thread::spawn(move || {
                                    let _ = tx.send(Message::Encoders(
                                        backend
                                            .encoders()
                                            .map(|e| e.iter().any(|e| e == encoder))
                                            .map_err(|e| e.to_string()),
                                    ));
                                });
                            }
                            if let Some(available) = self.bitrate_available {
                                ui.label(if available {
                                    catalog::text(fr, "Encoder available")
                                } else {
                                    catalog::text(fr, "Encoder unavailable")
                                });
                            }
                        });
                        ui.label(format!(
                            "{:.1} min · ~{:.1} MiB",
                            self.project.duration_ms() as f64 / 60000.,
                            self.project.estimated_bytes() as f64 / 1048576.
                        ));
                    }
                    if self.prefs.language != self.project.settings.language {
                        self.prefs.language = self.project.settings.language.clone();
                        let _ = self.prefs.save(&preferences_path());
                    }
                    if before != format!("{:?}", self.project) {
                        self.dirty = true;
                    }
                });
            });
        });
        ctx.request_repaint_after(Duration::from_millis(100));
    }
}
impl eframe::App for App {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        self.render(ctx);
    }
    fn on_exit(&mut self, _gl: Option<&eframe::glow::Context>) {
        let _ = self.prefs.save(&preferences_path());
        self.playback.send(AudioCommand::Exit);
        self.cancel.store(true, Ordering::Relaxed);
        if self.dirty {
            {
                let path = self
                    .recovery_store
                    .slot(self.path.as_deref(), &self.project);
                let _ = self.project.save(&path);
            }
        }
    }
}

fn project_save_target(path: &std::path::Path) -> PathBuf {
    path.with_extension("m4bproj")
}
fn overwrite_allowed(exists: bool, consent: bool) -> bool {
    !exists || consent
}

// A cancelled/failed save must never authorize destructive navigation.
fn replacement_allowed(choice: rfd::MessageDialogResult, saved: bool) -> bool {
    choice == rfd::MessageDialogResult::No || (choice == rfd::MessageDialogResult::Yes && saved)
}
fn load_cover(backend: &DesktopFfmpeg, path: &std::path::Path) -> anyhow::Result<egui::ColorImage> {
    use anyhow::ensure;
    let dir = tempfile::tempdir()?;
    let extracted = dir.path().join("cover.png");
    let is_image = path
        .extension()
        .is_some_and(|e| ["png"].iter().any(|v| e.eq_ignore_ascii_case(v)));
    let source = if is_image {
        path
    } else {
        backend.extract_cover_preview(path, &extracted)?;
        &extracted
    };
    ensure!(
        std::fs::metadata(source)?.len() <= 16 * 1024 * 1024,
        "Cover exceeds 16 MiB"
    );
    let mut reader = image::ImageReader::open(source)?.with_guessed_format()?;
    let mut limits = image::Limits::default();
    limits.max_image_width = Some(8192);
    limits.max_image_height = Some(8192);
    limits.max_alloc = Some(64 * 1024 * 1024);
    reader.limits(limits);
    let image = reader.decode()?;
    let image = if image.width() > 560 || image.height() > 560 {
        image.thumbnail(560, 560)
    } else {
        image
    }
    .to_rgba8();
    Ok(egui::ColorImage::from_rgba_unmultiplied(
        [image.width() as usize, image.height() as usize],
        image.as_raw(),
    ))
}

#[cfg(test)]
mod project_ui_tests {
    use super::*;
    #[test]
    fn normalized_save_target_requires_overwrite_consent() {
        for input in ["book", "book.json", "book.m4bproj", "book.M4BPROJ"] {
            assert_eq!(
                project_save_target(std::path::Path::new(input)),
                PathBuf::from("book.m4bproj")
            );
        }
        assert!(!overwrite_allowed(true, false));
        assert!(overwrite_allowed(true, true));
        assert!(overwrite_allowed(false, false));
    }
    #[test]
    fn welcome_requires_explicit_selection() {
        let app = App::default();
        assert!(!app.selected);
        assert!(app.path.is_none());
        assert!(!app.startup_checked);
    }
    #[test]
    fn cover_decode_limits_and_absent_state() {
        let d = tempfile::tempdir().unwrap();
        let path = d.path().join("cover.png");
        let image = image::RgbaImage::from_pixel(80, 40, image::Rgba([255, 0, 0, 255]));
        image.save(&path).unwrap();
        let decoded = load_cover(&DesktopFfmpeg::default(), &path).unwrap();
        assert_eq!(decoded.size, [80, 40]);
        std::fs::write(&path, b"invalid image").unwrap();
        assert!(load_cover(&DesktopFfmpeg::default(), &path).is_err());
        let f = std::fs::File::create(&path).unwrap();
        f.set_len(16 * 1024 * 1024 + 1).unwrap();
        assert!(load_cover(&DesktopFfmpeg::default(), &path).is_err());
    }
    #[test]
    fn unsaved_navigation_requires_discard_or_successful_save() {
        assert!(replacement_allowed(rfd::MessageDialogResult::No, false));
        assert!(replacement_allowed(rfd::MessageDialogResult::Yes, true));
        assert!(!replacement_allowed(rfd::MessageDialogResult::Yes, false));
        assert!(!replacement_allowed(rfd::MessageDialogResult::Cancel, true));
        assert!(!replacement_allowed(
            rfd::MessageDialogResult::Cancel,
            false
        ));
    }
}

#[cfg(test)]
mod release040_ui_tests {
    use super::*;
    #[test]
    fn every_editor_section_renders_in_both_languages_without_network_or_device() {
        for language in ["fr", "en"] {
            let mut app = App {
                startup_checked: true,
                project: Project::named("Synthetic UI").unwrap(),
                ..App::default()
            };
            app.project.settings.language = language.into();
            let ctx = egui::Context::default();
            let _ = ctx.run(egui::RawInput::default(), |ctx| app.render(ctx));
            app.selected = true;
            for section in [
                Section::Tracks,
                Section::Metadata,
                Section::Chapters,
                Section::Export,
                Section::Settings,
            ] {
                app.section = section;
                let _ = ctx.run(egui::RawInput::default(), |ctx| app.render(ctx));
            }
            app.creating = true;
            let _ = ctx.run(egui::RawInput::default(), |ctx| app.render(ctx));
        }
    }
}
