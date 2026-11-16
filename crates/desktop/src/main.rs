#![cfg_attr(target_os = "windows", windows_subsystem = "windows")]
mod playback;
mod updater;
use m4b_core::{Cancel, Chapter, DesktopFfmpeg, ExportEvent, MediaBackend, Project};
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
        "m4b-Craft",
        eframe::NativeOptions {
            viewport: egui::ViewportBuilder::default().with_inner_size([1000., 800.]),
            ..Default::default()
        },
        Box::new(|_| Ok(Box::new(App::default()))),
    )
}
use eframe::egui;
enum Message {
    UpdateChecked(Result<updater::Verified, String>),
    UpdateDownloaded(Result<updater::Download, String>),
    UpdateInstalled(Result<(), String>),
    Loaded(Box<Result<Project, String>>, Option<Option<PathBuf>>),
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
    updates_enabled: bool,
    update_channel: updater::Channel,
    update_candidate: Option<updater::Verified>,
    update_download: Option<updater::Download>,
    section: Section,
    creating: bool,
    new_name: String,
    allow_close: bool,
    project: Project,
    path: Option<PathBuf>,
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
        let recovery = recovery_path();
        Self {
            updates_enabled: false,
            update_channel: updater::Channel::Stable,
            update_candidate: None,
            update_download: None,
            section: Section::Tracks,
            creating: false,
            new_name: String::new(),
            allow_close: false,
            project: Project::default(),
            path: None,
            backend: DesktopFfmpeg::default(),
            tx,
            rx,
            busy: false,
            exporting: false,
            cancel: Arc::new(AtomicBool::new(false)),
            progress: 0.,
            logs: vec![],
            status: String::new(),
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
fn recovery_path() -> Option<PathBuf> {
    let root = std::env::var_os("LOCALAPPDATA")
        .or_else(|| std::env::var_os("XDG_STATE_HOME"))
        .map(PathBuf::from)
        .or_else(|| std::env::var_os("HOME").map(|h| PathBuf::from(h).join(".local/state")))?;
    let dir = root.join("m4b-Craft");
    std::fs::create_dir_all(&dir).ok()?;
    Some(dir.join("recovery.autosave.json"))
}
impl App {
    fn confirm_replace(&mut self) -> bool {
        if !self.dirty {
            return true;
        }
        let choice = rfd::MessageDialog::new()
            .set_title("Unsaved project / Projet modifié")
            .set_description("Save changes before continuing? / Enregistrer les modifications ?")
            .set_buttons(rfd::MessageButtons::YesNoCancel)
            .show();
        if choice == rfd::MessageDialogResult::Yes {
            self.save(false);
        }
        replacement_allowed(choice, !self.dirty)
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
            ));
            self.preview_track = Some(index);
            self.seek_ms = position.min(track.duration_ms);
        }
    }
    fn text<'a>(&self, en: &'a str, fr: &'a str) -> &'a str {
        if self.project.settings.language == "fr" {
            fr
        } else {
            en
        }
    }
    fn log(&mut self, s: String) {
        self.status = s.clone();
        self.logs.push(s);
        if self.logs.len() > 500 {
            self.logs.remove(0);
        }
    }
    fn save(&mut self, choose: bool) {
        let path = if choose || self.path.is_none() {
            rfd::FileDialog::new()
                .add_filter("m4b-Craft project", &["json"])
                .set_file_name("book.json")
                .save_file()
        } else {
            self.path.clone()
        };
        if let Some(path) = path {
            match self.project.save(&path) {
                Ok(()) => {
                    self.path = Some(path);
                    self.dirty = false;
                    if let Some(recovery) = recovery_path() {
                        let _ = std::fs::remove_file(recovery);
                    }
                    self.recovery = None;
                    self.log("Project saved / Projet enregistré".into());
                }
                Err(e) => self.log(format!("Save error: {e:#}")),
            }
        }
    }
    fn import(&mut self) {
        if self.path.is_none() {
            if !self.confirm_replace() {
                return;
            }
            self.creating = true;
            self.new_name = self.project.metadata.title.clone();
            self.log(
                "Create a named project first, then import / Créez un projet nommé, puis importez"
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
                "Audio",
                &[
                    "m4b", "m4a", "mp3", "flac", "ogg", "opus", "wav", "aac", "zip", "rar", "7z",
                ],
            )
            .pick_files()
        {
            let mut p = self.project.clone();
            let b = self.backend.clone();
            let tx = self.tx.clone();
            self.busy = true;
            std::thread::spawn(move || {
                let result = p
                    .import_assets(&b, paths, &assets)
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
            .add_filter("Project / M4B", &["json", "m4b", "m4a"])
            .pick_file()
        {
            let is_project = path
                .extension()
                .is_some_and(|s| s.eq_ignore_ascii_case("json"));
            let project_path = if is_project { Some(path.clone()) } else { None };
            let b = self.backend.clone();
            let tx = self.tx.clone();
            self.busy = true;
            std::thread::spawn(move || {
                let r = if is_project {
                    Project::load(&path)
                } else {
                    b.open(&path)
                };
                let _ = tx.send(Message::Loaded(
                    Box::new(r.map_err(|e| format!("{e:#}"))),
                    Some(project_path),
                ));
            });
        }
    }
    fn export(&mut self) {
        if let Err(e) = self.project.validate() {
            self.log(format!("Validation: {e:#}"));
            return;
        }
        if let Some(output) = rfd::FileDialog::new()
            .add_filter("M4B", &["m4b"])
            .set_file_name("audiobook.m4b")
            .save_file()
        {
            self.stop_preview();
            self.busy = true;
            self.exporting = true;
            self.progress = 0.;
            self.cancel = Arc::new(AtomicBool::new(false));
            let cancel = self.cancel.clone();
            let p = self.project.clone();
            let b = self.backend.clone();
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
impl eframe::App for App {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        while let Ok(state) = self.playback.rx.try_recv() {
            self.preview = state;
        }
        while let Ok(m) = self.rx.try_recv() {
            match m {
                Message::UpdateChecked(result) => {
                    self.busy = false;
                    match result {
                        Ok(update) => {
                            self.status = format!("Verified update {}", update.manifest.version);
                            self.update_candidate = Some(update);
                        }
                        Err(error) => self.log(error),
                    }
                }
                Message::UpdateDownloaded(result) => {
                    self.busy = false;
                    match result {
                        Ok(download) => {
                            self.update_download = Some(download);
                            self.status =
                                "Verified installer ready. Save project before installing.".into();
                        }
                        Err(error) => self.log(error),
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
                        Err(error) => self.log(error),
                    }
                }

                Message::Loaded(r, path) => {
                    self.busy = false;
                    match *r {
                        Ok(p) => {
                            self.stop_preview();
                            self.project = p;
                            let clean = matches!(&path, Some(Some(_)));
                            if let Some(path) = path {
                                self.path = path;
                            }
                            self.dirty = !clean;
                            self.log("Loaded / Chargé".into());
                        }
                        Err(e) => self.log(e),
                    }
                }
                Message::Event(ExportEvent::Log(s)) => self.log(s),
                Message::Event(ExportEvent::Progress(p)) => self.progress = p,
                Message::Done(r) => {
                    self.busy = false;
                    self.exporting = false;
                    self.log(match r {
                        Ok(()) => "Export complete / Export terminé".into(),
                        Err(e) => e,
                    });
                }
            }
        }
        if self.busy && ctx.input(|i| i.viewport().close_requested()) {
            ctx.send_viewport_cmd(egui::ViewportCommand::CancelClose);
            self.cancel.store(true, Ordering::Relaxed);
            self.status = if self.exporting {
                "Waiting for media job; export cancellation requested. Close again when finished."
            } else {
                "Waiting for background operation. Close again when finished."
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
        // Follow OS theme when system is selected; egui polls native theme changes.
        match self.project.settings.theme.as_str() {
            "dark" => ctx.set_theme(egui::ThemePreference::Dark),
            "light" => ctx.set_theme(egui::ThemePreference::Light),
            _ => ctx.set_theme(egui::ThemePreference::System),
        }
        if self.dirty && self.last_save.elapsed() > Duration::from_secs(15) {
            if let Some(path) = recovery_path() {
                if let Err(e) = self.project.save(&path) {
                    self.log(format!("Autosave: {e:#}"));
                }
            }
            self.last_save = Instant::now();
        }
        egui::TopBottomPanel::top("menu").show(ctx, |ui| {
            egui::menu::bar(ui, |ui| {
                ui.add_enabled_ui(!self.busy && !self.creating, |ui| {
                    ui.menu_button(self.text("Project", "Projet"), |ui| {
                        if ui.button(self.text("New…", "Nouveau…")).clicked() {
                            if self.confirm_replace() {
                                self.creating = true;
                                self.new_name.clear();
                            }
                            ui.close_menu();
                        }
                        if ui.button(self.text("Open…", "Ouvrir…")).clicked() {
                            self.open();
                            ui.close_menu();
                        }
                        if ui.button(self.text("Save", "Enregistrer")).clicked() {
                            self.save(false);
                            ui.close_menu();
                        }
                        if ui
                            .button(self.text("Save as…", "Enregistrer sous…"))
                            .clicked()
                        {
                            self.save(true);
                            ui.close_menu();
                        }
                        ui.separator();
                        if ui.button(self.text("Import…", "Importer…")).clicked() {
                            self.import();
                            ui.close_menu();
                        }
                        if ui
                            .button(self.text("Export M4B…", "Exporter M4B…"))
                            .clicked()
                        {
                            self.section = Section::Export;
                            self.export();
                            ui.close_menu();
                        }
                    });
                });
                ui.label(format!(
                    "{}{}",
                    if self.project.name.is_empty() {
                        "m4b-Craft"
                    } else {
                        &self.project.name
                    },
                    if self.dirty { " *" } else { "" }
                ));
                if self.exporting && ui.button("Cancel export / Annuler export").clicked() {
                    self.cancel.store(true, Ordering::Relaxed);
                }
            });
            if let Some(recovery) = self.recovery.clone() {
                ui.horizontal(|ui| {
                    ui.label(self.text("Recovery available", "Récupération disponible"));
                    if ui
                        .add_enabled(
                            !self.busy && !self.creating,
                            egui::Button::new(self.text("Restore", "Restaurer")),
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
                                self.path = None;
                                self.dirty = true;
                                self.recovery = None;
                            }
                            Err(e) => self.log(e.to_string()),
                        }
                    }
                    if ui.button(self.text("Dismiss", "Ignorer")).clicked() {
                        self.recovery = None;
                    }
                });
            }
        });
        egui::TopBottomPanel::bottom("status").show(ctx, |ui| {
            ui.add(egui::ProgressBar::new(self.progress).show_percentage());
            ui.label(&self.status);
            egui::CollapsingHeader::new(self.text("Logs / errors", "Journal / erreurs")).show(
                ui,
                |ui| {
                    egui::ScrollArea::vertical()
                        .max_height(140.)
                        .stick_to_bottom(true)
                        .show(ui, |ui| {
                            for log in &self.logs {
                                ui.label(log);
                            }
                        });
                },
            );
        });
        if self.creating {
            egui::Window::new("New project / Nouveau projet")
                .collapsible(false)
                .show(ctx, |ui| {
                    ui.label("Project name / Nom du projet");
                    ui.text_edit_singleline(&mut self.new_name);
                    if ui
                        .add_enabled(
                            !self.new_name.trim().is_empty(),
                            egui::Button::new("Create… / Créer…"),
                        )
                        .clicked()
                    {
                        if let Some(path) = rfd::FileDialog::new()
                            .add_filter("Project", &["json"])
                            .set_file_name("book.json")
                            .save_file()
                        {
                            let mut project =
                                Project::named(&self.new_name).expect("validated project name");
                            project.settings = self.project.settings.clone();
                            match project.save_new(&path) {
                                Ok(()) => {
                                    self.stop_preview();
                                    self.project = project;
                                    self.path = Some(path);
                                    self.dirty = false;
                                    self.creating = false;
                                    self.section = Section::Tracks;
                                }
                                Err(e) => self.log(format!("Create project: {e:#}")),
                            }
                        }
                    }
                    if ui.button("Cancel / Annuler").clicked() {
                        self.creating = false;
                    }
                });
        }
        egui::SidePanel::left("navigation")
            .resizable(false)
            .show(ctx, |ui| {
                for (section, en, fr) in [
                    (Section::Tracks, "Tracks", "Pistes"),
                    (Section::Metadata, "Metadata", "Métadonnées"),
                    (Section::Chapters, "Chapters", "Chapitres"),
                    (Section::Export, "Export", "Export"),
                    (Section::Settings, "Settings", "Réglages"),
                ] {
                    let label = self.text(en, fr);
                    ui.selectable_value(&mut self.section, section, label);
                }
            });
        egui::CentralPanel::default().show(ctx, |ui| {
            egui::ScrollArea::vertical().show(ui, |ui| {
                ui.add_enabled_ui(!self.busy && !self.creating, |ui| {
                    let before = format!("{:?}", self.project);
                    let fr = self.project.settings.language == "fr";
                    if self.section == Section::Settings {
                        ui.heading(format!("m4b-Craft {}", env!("CARGO_PKG_VERSION")));
                        ui.add_enabled(updater::supported(), egui::Checkbox::new(&mut self.updates_enabled, "Enable GitHub update actions (opt-in this session)"));
                        if !updater::supported() { ui.label("Updates are available on Windows x86_64 only."); }
                        let previous_channel = self.update_channel;
                        ui.horizontal(|ui| {
                            ui.selectable_value(&mut self.update_channel, updater::Channel::Stable, "Stable");
                            ui.selectable_value(&mut self.update_channel, updater::Channel::Prerelease, "Include prereleases");
                        });
                        if previous_channel != self.update_channel || !self.updates_enabled {
                            self.update_candidate = None;
                            self.update_download = None;
                        }
                        ui.label("Checks/downloads run in the background. Updates lock media actions until complete. Installation requires a saved project and explicit confirmation.");
                        let idle = updater::guard(self.busy || self.playback.is_busy(), self.dirty, false).is_ok();
                        ui.add_enabled_ui(self.updates_enabled && idle, |ui| {
                            if ui.button("Check GitHub Releases").clicked() {
                                self.update_candidate = None;
                                self.update_download = None;
                                let media_busy = self.busy || self.playback.is_busy();
                                self.status = "Checking signed GitHub Releases…".into();
                                self.busy = true;
                                let tx = self.tx.clone();
                                let channel = self.update_channel;
                                std::thread::spawn(move || { let _ = tx.send(Message::UpdateChecked(updater::check(channel, media_busy).map_err(|e| format!("Update check: {e:#}")))); });
                            }
                            if self.update_candidate.is_some() && ui.button("Download verified update").clicked() {
                                let update = self.update_candidate.take().unwrap();
                                let media_busy = self.busy || self.playback.is_busy();
                                self.status = "Downloading verified update…".into();
                                self.busy = true;
                                let tx = self.tx.clone();
                                std::thread::spawn(move || { let _ = tx.send(Message::UpdateDownloaded(update.download(media_busy).map_err(|e| format!("Update download: {e:#}")))); });
                            }
                            let install_clicked = cfg!(target_os = "windows")
                                && self.update_download.is_some()
                                && ui.add_enabled(!self.dirty, egui::Button::new("Install update…")).clicked();
                            if install_clicked
                                && rfd::MessageDialog::new().set_title("Install verified update")
                                    .set_description("Launch the normal interactive Windows installer and close m4b-Craft? Your project must already be saved. Reopen m4b-Craft after installation. This package has a cryptographic update signature but no Authenticode publisher certificate.")
                                    .set_buttons(rfd::MessageButtons::OkCancel).show() == rfd::MessageDialogResult::Ok
                                && updater::guard(self.busy || self.playback.is_busy(), self.dirty, true).is_ok()
                            {
                                let download = self.update_download.take().unwrap();
                                let media_busy = self.busy || self.playback.is_busy();
                                let dirty = self.dirty;
                                self.busy = true;
                                let tx = self.tx.clone();
                                std::thread::spawn(move || {
                                    let result = download.install(media_busy, dirty)
                                        .map_err(|e| format!("Installer: {e:#}"));
                                    let _ = tx.send(Message::UpdateInstalled(result));
                                });
                            }
                        });
                        ui.separator();
                        ui.horizontal(|ui| {
                            ui.label("Language / Langue");
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
                            ui.label(self.text("Theme", "Thème"));
                            egui::ComboBox::from_id_salt("theme")
                                .selected_text(&self.project.settings.theme)
                                .show_ui(ui, |ui| {
                                    for t in ["system", "light", "dark"] {
                                        ui.selectable_value(
                                            &mut self.project.settings.theme,
                                            t.into(),
                                            t,
                                        );
                                    }
                                });
                        });
                        ui.separator();
                    }
                    if self.section == Section::Metadata {
                        ui.heading(self.text("Book metadata", "Métadonnées du livre"));
                        egui::Grid::new("metadata").num_columns(2).show(ui, |ui| {
                            for (label, value) in [
                                (
                                    if fr { "Titre" } else { "Title" },
                                    &mut self.project.metadata.title,
                                ),
                                (
                                    if fr { "Auteur" } else { "Author" },
                                    &mut self.project.metadata.author,
                                ),
                                (
                                    if fr { "Narrateur" } else { "Narrator" },
                                    &mut self.project.metadata.narrator,
                                ),
                            ] {
                                ui.label(label);
                                ui.text_edit_singleline(value);
                                ui.end_row();
                            }
                            for (label, value) in [
                                ("Genre", &mut self.project.metadata.genre),
                                ("Date", &mut self.project.metadata.date),
                                ("Copyright", &mut self.project.metadata.copyright),
                            ] {
                                ui.label(label);
                                ui.text_edit_singleline(value);
                                ui.end_row();
                            }
                            ui.label("Description");
                            ui.text_edit_multiline(&mut self.project.metadata.description);
                            ui.end_row();
                        });
                        ui.horizontal(|ui| {
                            if ui
                                .button(self.text("Choose cover…", "Choisir couverture…"))
                                .clicked()
                            {
                                if let Some(p) = rfd::FileDialog::new()
                                    .add_filter("Image", &["jpg", "jpeg", "png"])
                                    .pick_file()
                                {
                                    self.project.cover = Some(p);
                                }
                            }
                            if ui
                                .button(self.text("Remove cover", "Retirer couverture"))
                                .clicked()
                            {
                                self.project.cover = None;
                            }
                            ui.label(
                                self.project
                                    .cover
                                    .as_ref()
                                    .map(|p| p.display().to_string())
                                    .unwrap_or_else(|| {
                                        self.text("No cover", "Sans couverture").into()
                                    }),
                            );
                        });
                        ui.separator();
                    }
                    if self.section == Section::Tracks {
                        if ui
                            .button(
                                self.text("Import audio / archives…", "Importer audio / archives…"),
                            )
                            .clicked()
                        {
                            self.import();
                        }
                        ui.heading(self.text(
                            "Tracks (reorder resets chapters)",
                            "Pistes (réordonner réinitialise les chapitres)",
                        ));
                        ui.group(|ui| {
                            ui.heading(self.text("Pre-listening", "Pré-écoute"));
                            ui.horizontal(|ui| {
                                if ui
                                    .add_enabled(
                                        self.preview.active,
                                        egui::Button::new(if self.preview.paused {
                                            self.text("Resume", "Reprendre")
                                        } else {
                                            self.text("Pause", "Pause")
                                        }),
                                    )
                                    .clicked()
                                {
                                    self.playback
                                        .send(AudioCommand::Pause(!self.preview.paused));
                                }
                                if ui.button(self.text("Stop", "Arrêter")).clicked() {
                                    self.stop_preview();
                                }
                                let label = self.text("Volume", "Volume");
                                if ui
                                    .add(egui::Slider::new(&mut self.volume, 0.0..=1.0).text(label))
                                    .changed()
                                {
                                    self.playback.send(AudioCommand::Volume(self.volume));
                                }
                            });
                            if let Some(index) = self.preview_track {
                                if let Some(track) = self.project.tracks.get(index) {
                                    let total = track.duration_ms;
                                    ui.label(
                                        track
                                            .path
                                            .file_name()
                                            .unwrap_or_default()
                                            .to_string_lossy(),
                                    );
                                    ui.label(format!(
                                        "{} / {}",
                                        playback::clock(self.preview.position_ms),
                                        playback::clock(total)
                                    ));
                                    let response = ui.add(
                                        egui::Slider::new(&mut self.seek_ms, 0..=total)
                                            .show_value(false),
                                    );
                                    if response.drag_stopped()
                                        || (response.changed() && !response.dragged())
                                    {
                                        self.play_preview(index, self.seek_ms);
                                    } else if !response.dragged() {
                                        self.seek_ms = self.preview.position_ms.min(total);
                                    }
                                }
                            }
                            ui.label(&self.preview.message);
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
                                if ui
                                    .small_button(if fr { "Écouter" } else { "Play" })
                                    .clicked()
                                {
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
                            self.project.tracks.swap(a, b);
                            self.project.reset_chapters();
                        }
                        if let Some(i) = remove {
                            self.stop_preview();
                            self.project.tracks.remove(i);
                            self.project.reset_chapters();
                        }
                        ui.separator();
                    }
                    if self.section == Section::Chapters {
                        ui.heading(self.text(
                            "Chapters — start in milliseconds",
                            "Chapitres — début en millisecondes",
                        ));
                        let mut chapter_jump = None;
                        let mut delete = None;
                        for (i, c) in self.project.chapters.iter_mut().enumerate() {
                            ui.horizontal(|ui| {
                                if ui
                                    .small_button(if fr { "Écouter" } else { "Play" })
                                    .clicked()
                                {
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
                            if ui
                                .button(self.text("Add chapter", "Ajouter chapitre"))
                                .clicked()
                            {
                                let start = self
                                    .project
                                    .chapters
                                    .last()
                                    .map(|c| c.start_ms.saturating_add(1000))
                                    .unwrap_or(0);
                                self.project.chapters.push(Chapter {
                                    start_ms: start,
                                    title: format!("Chapter {}", self.project.chapters.len() + 1),
                                });
                            }
                            if ui
                                .button(self.text("One per track", "Un par piste"))
                                .clicked()
                            {
                                self.project.reset_chapters();
                            }
                        });
                        ui.separator();
                    }
                    if self.section == Section::Export {
                        if ui
                            .button(self.text("Export M4B…", "Exporter M4B…"))
                            .clicked()
                        {
                            self.export();
                        }
                        ui.heading(self.text("AAC export", "Export AAC"));
                        ui.horizontal(|ui| {
                            ui.label(self.text("Bitrate (kb/s)", "Débit (kb/s)"));
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
                            if ui
                                .button(self.text("Check encoder", "Vérifier encodeur"))
                                .clicked()
                            {
                                match self.backend.encoders() {
                                    Ok(e) => {
                                        self.bitrate_available = Some(e.iter().any(|e| e == "aac"))
                                    }
                                    Err(e) => self.log(e.to_string()),
                                }
                            }
                            if let Some(available) = self.bitrate_available {
                                ui.label(if available {
                                    "AAC ✓"
                                } else {
                                    "AAC unavailable / indisponible"
                                });
                            }
                        });
                        ui.label(format!(
                            "{:.1} min · ~{:.1} MiB (AAC audio)",
                            self.project.duration_ms() as f64 / 60000.,
                            self.project.estimated_bytes() as f64 / 1048576.
                        ));
                    }
                    if before != format!("{:?}", self.project) {
                        self.dirty = true;
                    }
                });
            });
        });
        ctx.request_repaint_after(Duration::from_millis(100));
    }
    fn on_exit(&mut self, _gl: Option<&eframe::glow::Context>) {
        self.playback.send(AudioCommand::Exit);
        self.cancel.store(true, Ordering::Relaxed);
        if self.dirty {
            if let Some(path) = recovery_path() {
                let _ = self.project.save(&path);
            }
        }
    }
}

// A cancelled/failed save must never authorize destructive navigation.
fn replacement_allowed(choice: rfd::MessageDialogResult, saved: bool) -> bool {
    choice == rfd::MessageDialogResult::No || (choice == rfd::MessageDialogResult::Yes && saved)
}
#[cfg(test)]
mod project_ui_tests {
    use super::*;
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
