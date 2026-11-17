use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};
#[derive(Serialize, Deserialize)]
#[serde(default)]
pub struct Preferences {
    pub language: String,
    pub sidebar: f32,
    pub recent: Vec<PathBuf>,
    pub prerelease: bool,
}
impl Default for Preferences {
    fn default() -> Self {
        Self {
            language: os_language(),
            sidebar: 180.,
            recent: vec![],
            prerelease: !semver::Version::parse(env!("CARGO_PKG_VERSION"))
                .unwrap()
                .pre
                .is_empty(),
        }
    }
}
pub fn language(locale: &str) -> String {
    if locale
        .split(['_', '-', '.'])
        .next()
        .is_some_and(|v| v.eq_ignore_ascii_case("fr"))
    {
        "fr"
    } else {
        "en"
    }
    .into()
}
fn os_language() -> String {
    #[cfg(not(any(target_os = "macos", target_os = "windows")))]
    for name in ["LC_ALL", "LC_MESSAGES", "LANG"] {
        if let Ok(value) = std::env::var(name) {
            if !value.is_empty() {
                return language(&value);
            }
        }
    }
    #[cfg(target_os = "windows")]
    {
        #[link(name = "kernel32")]
        extern "system" {
            fn GetUserDefaultUILanguage() -> u16;
        }
        // Windows primary language identifiers cover all French regional variants.
        // Use the UI language, not regional formatting, and never spawn a console.
        return if unsafe { GetUserDefaultUILanguage() } & 0x03ff == 0x000c {
            "fr"
        } else {
            "en"
        }
        .into();
    }
    #[cfg(target_os = "macos")]
    {
        if let Ok(output) = std::process::Command::new("/usr/bin/defaults")
            .args(["read", "-g", "AppleLanguages"])
            .output()
        {
            if output.status.success() {
                let text = String::from_utf8_lossy(&output.stdout);
                if let Some(preferred) = text
                    .split(['(', ')', '"', ',', '\n'])
                    .map(str::trim)
                    .find(|part| !part.is_empty())
                {
                    return language(preferred);
                }
            }
        }
    }
    #[cfg(target_os = "macos")]
    let result = std::process::Command::new("/usr/bin/defaults")
        .args(["read", "-g", "AppleLocale"])
        .output();
    #[cfg(not(any(target_os = "macos", target_os = "windows")))]
    let result = std::process::Command::new("locale").output();
    #[cfg(not(target_os = "windows"))]
    {
        result
            .ok()
            .filter(|v| v.status.success())
            .map(|v| {
                let text = String::from_utf8_lossy(&v.stdout);
                let locale = text
                    .lines()
                    .find_map(|line| line.strip_prefix("LANG="))
                    .unwrap_or(text.trim())
                    .trim_matches('"');
                language(locale)
            })
            .unwrap_or_else(|| "en".into())
    }
}
impl Preferences {
    pub fn load(path: &Path) -> Self {
        std::fs::read(path)
            .ok()
            .and_then(|b| serde_json::from_slice(&b).ok())
            .map(|mut p: Self| {
                p.language = language(&p.language);
                p.sidebar = if p.sidebar.is_finite() {
                    p.sidebar.clamp(120., 400.)
                } else {
                    180.
                };
                p.recent.truncate(20);
                p
            })
            .unwrap_or_default()
    }
    pub fn save(&self, path: &Path) -> std::io::Result<()> {
        let mut file = tempfile::NamedTempFile::new_in(path.parent().unwrap())?;
        serde_json::to_writer(&mut file, self)?;
        file.persist(path).map_err(|e| e.error)?;
        Ok(())
    }
    pub fn remember(&mut self, path: PathBuf) {
        self.recent.retain(|p| p != &path);
        self.recent.insert(0, path);
        self.recent.truncate(20);
    }
    pub fn remove(&mut self, path: &Path) {
        self.recent.retain(|p| p != path);
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn locale_fallback() {
        for v in ["fr_FR.UTF-8", "FR-ca", "fr"] {
            assert_eq!(language(v), "fr");
        }
        for v in ["de_DE", "", "en_US"] {
            assert_eq!(language(v), "en");
        }
    }
    #[test]
    fn manual_language_sidebar_and_channel_survive_restart() {
        let d = tempfile::tempdir().unwrap();
        let path = d.path().join("prefs.json");
        let p = Preferences {
            language: "fr".into(),
            sidebar: 250.,
            recent: vec![],
            prerelease: true,
        };
        p.save(&path).unwrap();
        let p = Preferences::load(&path);
        assert_eq!(p.language, "fr");
        assert_eq!(p.sidebar, 250.);
        assert!(p.prerelease);
    }
    #[test]
    fn recent_removal_only_changes_link() {
        let d = tempfile::tempdir().unwrap();
        let p = d.path().join("book.m4bproj");
        std::fs::write(&p, b"project").unwrap();
        let config = d.path().join("prefs.json");
        let mut prefs = Preferences::default();
        prefs.remember(p.clone());
        prefs.save(&config).unwrap();
        let mut prefs = Preferences::load(&config);
        assert_eq!(prefs.recent, vec![p.clone()]);
        prefs.remove(&p);
        prefs.save(&config).unwrap();
        assert!(Preferences::load(&config).recent.is_empty());
        assert!(p.exists());
    }
}
