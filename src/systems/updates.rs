//! Release updates, in the Tauri updater format shared with Oxyde 1.x.
//! The manifest and the signatures come from the release workflow. Each installation
//! reads its own manifest entry: Tauri clients ignore the Linux ones.
use crate::platform::{Installation, ReleaseSignature};
use base64::Engine;
use std::collections::HashMap;
use std::path::PathBuf;

const MANIFEST_URL: &str = "https://github.com/Carlton2001/Oxyde_FM/releases/latest/download/latest.json";
const DOWNLOAD_PREFIX: &str = "https://github.com/Carlton2001/Oxyde_FM/releases/download/";
const RELEASE_PAGE_PREFIX: &str = "https://github.com/Carlton2001/Oxyde_FM/releases/tag/v";
/// Minisign public key of the updater, as published in the Tauri configuration.
const PUBLIC_KEY: &str = "dW50cnVzdGVkIGNvbW1lbnQ6IG1pbmlzaWduIHB1YmxpYyBrZXk6IDc0RjI1QzVCNzlERkZGNzcKUldSMy85OTVXMXp5ZEJMSEIzV254Z0FhTjFBeENLVUpXbHpEVlBYdzRNY3Q0MlZmeGliYnJvQmIK";
const MANIFEST_LIMIT: usize = 64 * 1024;
const SETUP_LIMIT: usize = 512 * 1024 * 1024;

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Update {
    pub version: String,
    url: String,
    signature: String,
    /// What a user downloads by hand: the package when published, else the release page.
    manual: String,
}

impl Update {
    pub fn manual_download(&self) -> &str { &self.manual }
}

#[derive(serde::Deserialize)]
struct Manifest {
    version: String,
    platforms: HashMap<String, Asset>,
}

#[derive(serde::Deserialize)]
struct Asset {
    url: String,
    signature: String,
}

/// Fetch the latest manifest; `None` when this build is current or receives no releases.
pub fn check() -> Result<Option<Update>, String> {
    let installation = crate::platform::platform().installation();
    if installation == Installation::Unmanaged {
        return Ok(None);
    }
    let bytes = crate::platform::platform().download(MANIFEST_URL, MANIFEST_LIMIT).map_err(|error| error.to_string())?;
    parse_manifest(&bytes, env!("CARGO_PKG_VERSION"), installation)
}

/// Download the release file and keep it only if its signature matches the key of its
/// system: the Tauri updater key on Windows, Oxyde's package signing key on Linux.
pub fn download(update: &Update) -> Result<PathBuf, String> {
    let platform = crate::platform::platform();
    let Installation::SelfUpdating { signature, .. } = platform.installation() else {
        return Err("This copy of Oxyde does not install updates".into());
    };
    let name = update.url.rsplit('/').next()
        .filter(|name| !name.is_empty() && name.chars().all(|part| part.is_ascii_alphanumeric() || matches!(part, '.' | '_' | '-')))
        .ok_or_else(|| format!("Unexpected update file {}", update.url))?;
    let bytes = platform.download(&update.url, SETUP_LIMIT).map_err(|error| error.to_string())?;
    if signature == ReleaseSignature::Minisign {
        verify(&bytes, &update.signature, PUBLIC_KEY)?;
    }
    // The directory outlives Oxyde on Windows: Setup runs after the application exits.
    let directory = tempfile::Builder::new().prefix("oxyde-update-").tempdir().map_err(|error| error.to_string())?.keep();
    let path = directory.join(name);
    let written = std::fs::write(&path, bytes).map_err(|error| error.to_string()).and_then(|()| match signature {
        ReleaseSignature::Minisign => Ok(()),
        // gpgv reads files: the archive is checked where it was written, before any use.
        ReleaseSignature::OpenPgp => platform.verify_openpgp(&path, update.signature.as_bytes()).map_err(|error| error.to_string()),
    });
    if let Err(error) = written {
        let _ = std::fs::remove_dir_all(&directory);
        return Err(error);
    }
    Ok(path)
}

/// Install the downloaded release; Oxyde starts again once this process has exited.
pub fn launch(setup: &std::path::Path) -> Result<(), String> {
    crate::platform::platform().launch_update(setup).map_err(|error| error.to_string())
}

fn parse_manifest(bytes: &[u8], current: &str, installation: Installation) -> Result<Option<Update>, String> {
    let manifest: Manifest = serde_json::from_slice(bytes).map_err(|error| error.to_string())?;
    let version = manifest.version.trim_start_matches('v').to_owned();
    if !is_newer(&version, current)? {
        return Ok(None);
    }
    let key = match installation {
        Installation::Unmanaged => return Ok(None),
        Installation::SelfUpdating { manifest_key, .. } => Some(manifest_key),
        Installation::Package { manifest_key } => manifest_key,
    };
    let asset = key.and_then(|key| manifest.platforms.get(key));
    if let Some(asset) = asset {
        if !asset.url.starts_with(DOWNLOAD_PREFIX) {
            return Err(format!("Unexpected update location {}", asset.url));
        }
    }
    let page = format!("{RELEASE_PAGE_PREFIX}{version}");
    match (installation, asset) {
        // A release without a build for this system is not an update for it.
        (Installation::SelfUpdating { .. }, None) => Ok(None),
        (Installation::SelfUpdating { .. }, Some(asset)) => Ok(Some(Update {
            version, url: asset.url.clone(), signature: asset.signature.clone(), manual: page,
        })),
        (_, asset) => {
            let manual = asset.map_or(page, |asset| asset.url.clone());
            Ok(Some(Update { version, url: manual.clone(), signature: String::new(), manual }))
        }
    }
}

fn is_newer(candidate: &str, current: &str) -> Result<bool, String> {
    let parse = |version: &str| -> Result<Vec<u64>, String> {
        let parts = version.split('.').map(str::parse).collect::<Result<Vec<u64>, _>>()
            .map_err(|_| format!("Invalid version {version}"))?;
        if parts.len() == 3 { Ok(parts) } else { Err(format!("Invalid version {version}")) }
    };
    Ok(parse(candidate)? > parse(current)?)
}

/// Tauri stores both the key and the signature as base64-wrapped minisign text.
fn verify(bytes: &[u8], signature: &str, public_key: &str) -> Result<(), String> {
    let text = |encoded: &str| -> Result<String, String> {
        let decoded = base64::engine::general_purpose::STANDARD.decode(encoded.trim()).map_err(|error| error.to_string())?;
        String::from_utf8(decoded).map_err(|error| error.to_string())
    };
    let key = minisign_verify::PublicKey::decode(&text(public_key)?).map_err(|error| error.to_string())?;
    let signature = minisign_verify::Signature::decode(&text(signature)?).map_err(|error| error.to_string())?;
    key.verify(bytes, &signature, false).map_err(|error| error.to_string())
}

#[cfg(test)]
#[path = "updates/tests/manifest.rs"]
mod tests;
