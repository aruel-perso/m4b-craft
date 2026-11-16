//! Opt-in updater. Trust is rooted only in the compiled Ed25519 public key.
use anyhow::{ensure, Context, Result};
use ed25519_dalek::{Signature, VerifyingKey};
use reqwest::{blocking::Client, redirect::Policy, Url};
use semver::Version;
use serde::Deserialize;
use sha2::{Digest, Sha256};
use std::{
    io::{Read, Write},
    time::Duration,
};

const REPO: &str = "https://github.com/aruel-perso/m4b-craft/releases/download/";
const MAX_MSI: u64 = 512 * 1024 * 1024;
const PLATFORM: &str = "windows-x86_64";
#[derive(Clone, Copy, PartialEq, Debug)]
pub enum Channel {
    Stable,
    Prerelease,
}
#[derive(Clone, Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Manifest {
    pub version: String,
    pub platform: String,
    pub filename: String,
    pub sha256: String,
    pub size: u64,
    pub url: String,
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Envelope {
    payload: String,
    signature: String,
}
#[derive(Clone, Debug)]
pub struct Verified {
    pub manifest: Manifest,
    envelope: Vec<u8>,
    channel: Channel,
}
pub struct Download {
    pub verified: Verified,
    file: tempfile::NamedTempFile,
}
#[derive(Deserialize)]
struct Release {
    tag_name: String,
    draft: bool,
    prerelease: bool,
}

pub fn supported() -> bool {
    cfg!(all(target_os = "windows", target_arch = "x86_64"))
}

pub fn guard(busy: bool, dirty: bool, installing: bool) -> Result<()> {
    ensure!(
        !busy,
        "Updates are unavailable during media work or playback"
    );
    ensure!(
        !installing || !dirty,
        "Save the project before installing an update"
    );
    Ok(())
}
fn release_url(url: &Url) -> bool {
    url.scheme() == "https"
        && url.username().is_empty()
        && url.password().is_none()
        && url.port().is_none()
        && url.query().is_none()
        && url.fragment().is_none()
        && url.as_str().starts_with(REPO)
}
fn allowed_redirect(url: &Url) -> bool {
    url.scheme() == "https"
        && url.username().is_empty()
        && url.password().is_none()
        && url.port().is_none()
        && matches!(
            url.host_str(),
            Some(
                "github.com"
                    | "release-assets.githubusercontent.com"
                    | "objects.githubusercontent.com"
            )
        )
        && (url.host_str() != Some("github.com") || release_url(url))
}
fn client() -> Result<Client> {
    Ok(Client::builder()
        .user_agent("m4b-Craft/0.2 updater")
        .connect_timeout(Duration::from_secs(10))
        .timeout(Duration::from_secs(180))
        .https_only(true)
        .redirect(Policy::custom(|attempt| {
            if attempt.previous().len() < 5 && allowed_redirect(attempt.url()) {
                attempt.follow()
            } else {
                attempt.error("Untrusted update redirect")
            }
        }))
        .build()?)
}
fn get(client: &Client, url: &str, max: u64) -> Result<Vec<u8>> {
    let response = client.get(url).send()?.error_for_status()?;
    ensure!(
        response.content_length().is_none_or(|n| n <= max),
        "Oversize response"
    );
    let mut bytes = Vec::new();
    response.take(max + 1).read_to_end(&mut bytes)?;
    ensure!(bytes.len() as u64 <= max, "Oversize response");
    Ok(bytes)
}
fn verify_with(bytes: &[u8], channel: Channel, current: &str, key: &[u8; 32]) -> Result<Verified> {
    ensure!(bytes.len() <= 16384, "Oversize manifest");
    let envelope: Envelope = serde_json::from_slice(bytes)?;
    let signature = Signature::from_slice(&hex::decode(&envelope.signature)?)?;
    VerifyingKey::from_bytes(key)?
        .verify_strict(envelope.payload.as_bytes(), &signature)
        .context("Invalid update signature")?;
    let m: Manifest = serde_json::from_str(&envelope.payload)?;
    let version = Version::parse(&m.version)?;
    ensure!(
        version > Version::parse(current)?,
        "Update must be newer than this application"
    );
    ensure!(version.build.is_empty(), "Build metadata is not supported");
    ensure!(
        channel == Channel::Prerelease || version.pre.is_empty(),
        "Prerelease on stable channel"
    );
    ensure!(m.platform == PLATFORM, "Wrong platform");
    ensure!(
        m.filename == format!("m4b-Craft-{}-windows-x86_64.msi", m.version),
        "Unexpected filename"
    );
    ensure!(m.size > 0 && m.size <= MAX_MSI, "Invalid installer size");
    ensure!(
        m.sha256.len() == 64 && hex::decode(&m.sha256)?.len() == 32,
        "Invalid hash"
    );
    let expected = format!("{REPO}v{}/{}", m.version, m.filename);
    ensure!(
        m.url == expected && release_url(&Url::parse(&m.url)?),
        "Untrusted installer URL"
    );
    Ok(Verified {
        manifest: m,
        envelope: bytes.to_vec(),
        channel,
    })
}
pub fn verify(bytes: &[u8], channel: Channel) -> Result<Verified> {
    let key: [u8; 32] =
        hex::decode(include_str!("../../../packaging/update-public-key.hex").trim())?
            .try_into()
            .map_err(|_| anyhow::anyhow!("Invalid compiled public key"))?;
    verify_with(bytes, channel, env!("CARGO_PKG_VERSION"), &key)
}
pub fn check(channel: Channel, busy: bool) -> Result<Verified> {
    guard(busy, false, false)?;
    ensure!(supported(), "Updates are available on Windows x86_64 only");
    let client = client()?;
    let releases: Vec<Release> = serde_json::from_slice(&get(
        &client,
        "https://api.github.com/repos/aruel-perso/m4b-craft/releases?per_page=100",
        1024 * 1024,
    )?)?;
    let mut versions: Vec<Version> = releases
        .into_iter()
        .filter(|r| !r.draft && (channel == Channel::Prerelease || !r.prerelease))
        .filter_map(|r| Version::parse(r.tag_name.strip_prefix('v')?).ok())
        .filter(|v| {
            (channel == Channel::Prerelease || v.pre.is_empty())
                && *v > Version::parse(env!("CARGO_PKG_VERSION")).expect("package version")
        })
        .collect();
    versions.sort();
    let version = versions.last().context("No newer release found")?;
    let result = verify(
        &get(
            &client,
            &format!("{REPO}v{version}/update-windows-x86_64.json"),
            16384,
        )?,
        channel,
    )?;
    ensure!(
        result.manifest.version == version.to_string(),
        "Release/manifest version mismatch"
    );
    Ok(result)
}
impl Verified {
    pub fn download(self, busy: bool) -> Result<Download> {
        guard(busy, false, false)?;
        ensure!(supported(), "Updates are available on Windows x86_64 only");
        let verified = verify(&self.envelope, self.channel)?;
        let client = client()?;
        let response = client
            .get(&verified.manifest.url)
            .send()?
            .error_for_status()?;
        ensure!(
            response
                .content_length()
                .is_none_or(|n| n == verified.manifest.size),
            "Incorrect download size"
        );
        let mut file = tempfile::Builder::new()
            .prefix("m4b-update-")
            .suffix(".msi")
            .tempfile()?;
        let count = std::io::copy(&mut response.take(verified.manifest.size + 1), &mut file)?;
        ensure!(count == verified.manifest.size, "Incorrect download size");
        file.flush()?;
        let download = Download { verified, file };
        download.reverify()?;
        Ok(download)
    }
}
impl Download {
    fn reverify(&self) -> Result<()> {
        let verified = verify(&self.verified.envelope, self.verified.channel)?;
        verify_file(self.file.reopen()?, &verified.manifest)
    }

    pub fn install(self, busy: bool, dirty: bool) -> Result<()> {
        guard(busy, dirty, true)?;
        self.reverify()?;
        #[cfg(target_os = "windows")]
        {
            let installer = std::path::PathBuf::from(
                std::env::var_os("SystemRoot").context("Missing Windows system directory")?,
            )
            .join("System32/msiexec.exe");
            // The editor must release its executable before MSI replaces it. Retain the
            // verified download across process exit; never delete it while MSI is using it.
            let path = self.file.into_temp_path().keep()?;
            match std::process::Command::new(installer)
                .arg("/i")
                .arg(&path)
                .spawn()
            {
                Ok(_child) => Ok(()),
                Err(error) => {
                    let _ = std::fs::remove_file(&path);
                    Err(error).context("Launch Windows Installer")
                }
            }
        }
        #[cfg(not(target_os = "windows"))]
        anyhow::bail!("Installation is supported on Windows only")
    }
}
fn verify_file(mut file: std::fs::File, manifest: &Manifest) -> Result<()> {
    ensure!(
        file.metadata()?.len() == manifest.size,
        "Installer size changed"
    );
    let mut digest = Sha256::new();
    let mut buffer = [0u8; 65536];
    let mut count = 0u64;
    loop {
        let n = file.read(&mut buffer)?;
        if n == 0 {
            break;
        }
        count += n as u64;
        ensure!(count <= manifest.size, "Installer grew during verification");
        digest.update(&buffer[..n]);
    }
    ensure!(
        count == manifest.size,
        "Installer size changed during verification"
    );
    ensure!(
        hex::encode(digest.finalize()) == manifest.sha256,
        "Installer hash mismatch"
    );
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use ed25519_dalek::{Signer, SigningKey};
    fn signed(version: &str, change: impl FnOnce(&mut serde_json::Value)) -> (Vec<u8>, [u8; 32]) {
        let key = SigningKey::from_bytes(&[42; 32]);
        let mut value = serde_json::json!({"version":version,"platform":PLATFORM,
            "filename":format!("m4b-Craft-{version}-windows-x86_64.msi"),"sha256":"ab".repeat(32),"size":10,
            "url":format!("{REPO}v{version}/m4b-Craft-{version}-windows-x86_64.msi")});
        change(&mut value);
        let payload = value.to_string();
        let signature = hex::encode(key.sign(payload.as_bytes()).to_bytes());
        (
            serde_json::to_vec(&serde_json::json!({"payload":payload,"signature":signature}))
                .unwrap(),
            key.verifying_key().to_bytes(),
        )
    }
    #[test]
    fn rejects_file_tampering_before_launch() {
        let (bytes, key) = signed("0.3.0", |m| {
            m["sha256"] = hex::encode(Sha256::digest(b"0123456789")).into();
        });
        let update = verify_with(&bytes, Channel::Stable, "0.2.0", &key).unwrap();
        let mut file = tempfile::NamedTempFile::new().unwrap();
        file.write_all(b"0123456789").unwrap();
        assert!(verify_file(file.reopen().unwrap(), &update.manifest).is_ok());
        // Same length, different bytes: size alone cannot authenticate an MSI.
        std::fs::write(file.path(), b"9876543210").unwrap();
        assert!(verify_file(file.reopen().unwrap(), &update.manifest).is_err());
        std::fs::write(file.path(), b"truncated").unwrap();
        assert!(verify_file(file.reopen().unwrap(), &update.manifest).is_err());
    }
    #[test]
    fn malformed_signature_and_envelope() {
        let (bytes, key) = signed("0.3.0", |_| {});
        let mut envelope: serde_json::Value = serde_json::from_slice(&bytes).unwrap();
        for signature in ["00", "zz", &"00".repeat(64)] {
            envelope["signature"] = signature.into();
            assert!(verify_with(
                &serde_json::to_vec(&envelope).unwrap(),
                Channel::Stable,
                "0.2.0",
                &key
            )
            .is_err());
        }
        assert!(verify_with(&[b' '; 16385], Channel::Stable, "0.2.0", &key).is_err());
        assert!(verify_with(b"{}", Channel::Stable, "0.2.0", &key).is_err());
    }
    #[test]
    fn valid_and_tampered() {
        let (bytes, key) = signed("0.3.0", |_| {});
        assert!(verify_with(&bytes, Channel::Stable, "0.2.0-alpha.1", &key).is_ok());
        assert!(verify_with(&bytes, Channel::Stable, "0.2.0-alpha.1", &[0; 32]).is_err());
        let mut value: serde_json::Value = serde_json::from_slice(&bytes).unwrap();
        value["payload"] = value["payload"]
            .as_str()
            .unwrap()
            .replace("0.3.0", "9.0.0")
            .into();
        assert!(verify_with(
            &serde_json::to_vec(&value).unwrap(),
            Channel::Stable,
            "0.2.0",
            &key
        )
        .is_err());
    }
    #[test]
    fn rejects_signed_bad_fields() {
        for (field, value) in [
            ("platform", serde_json::json!("linux")),
            ("filename", serde_json::json!("../x.msi")),
            ("url", serde_json::json!("https://evil.example/x")),
            ("size", serde_json::json!(MAX_MSI + 1)),
            ("sha256", serde_json::json!("00")),
            ("extra", serde_json::json!(true)),
        ] {
            let (bytes, key) = signed("0.3.0", |m| m[field] = value);
            assert!(
                verify_with(&bytes, Channel::Stable, "0.2.0", &key).is_err(),
                "{field}"
            );
        }
    }
    #[test]
    fn versions_channels() {
        for version in ["0.1.9", "0.2.0", "0.3.0-alpha.1"] {
            let (bytes, key) = signed(version, |_| {});
            assert!(verify_with(&bytes, Channel::Stable, "0.2.0", &key).is_err());
        }
        let (bytes, key) = signed("0.3.0-alpha.1", |_| {});
        assert!(verify_with(&bytes, Channel::Prerelease, "0.2.0", &key).is_ok());
        let (bytes, key) = signed("0.10.0", |_| {});
        assert!(verify_with(&bytes, Channel::Stable, "0.9.0", &key).is_ok());
    }
    #[test]
    fn busy_dirty_and_origins() {
        assert!(check(Channel::Stable, true).is_err());
        let (bytes, key) = signed("0.3.0", |_| {});
        let verified = verify_with(&bytes, Channel::Stable, "0.2.0", &key).unwrap();
        assert!(verified.download(true).is_err());
        assert!(guard(true, false, false).is_err());
        assert!(guard(true, false, true).is_err());
        assert!(guard(false, true, true).is_err());
        assert!(guard(false, true, false).is_ok());
        for url in [
            "http://github.com/x",
            "https://github.com.evil.test/x",
            "https://evil.test/x",
            "https://github.com/aruel-perso/other/releases/download/a",
        ] {
            assert!(!allowed_redirect(&Url::parse(url).unwrap()));
        }
    }
}
