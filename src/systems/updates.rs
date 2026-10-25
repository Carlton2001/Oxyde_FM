//! Release updates, in the Tauri updater format shared with Oxyde 1.x.
//! The manifest and the setup signature come from the release workflow.
use base64::Engine;
use std::collections::HashMap;
use std::path::PathBuf;

const MANIFEST_URL: &str = "https://github.com/Carlton2001/Oxyde_FM/releases/latest/download/latest.json";
const DOWNLOAD_PREFIX: &str = "https://github.com/Carlton2001/Oxyde_FM/releases/download/";
/// Minisign public key of the updater, as published in the Tauri configuration.
const PUBLIC_KEY: &str = "dW50cnVzdGVkIGNvbW1lbnQ6IG1pbmlzaWduIHB1YmxpYyBrZXk6IDc0RjI1QzVCNzlERkZGNzcKUldSMy85OTVXMXp5ZEJMSEIzV254Z0FhTjFBeENLVUpXbHpEVlBYdzRNY3Q0MlZmeGliYnJvQmIK";
const PLATFORM: &str = "windows-x86_64";
const MANIFEST_LIMIT: usize = 64 * 1024;
const SETUP_LIMIT: usize = 512 * 1024 * 1024;

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Update {
    pub version: String,
    url: String,
    signature: String,
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

/// Fetch the latest manifest; `None` when this build is current.
pub fn check() -> Result<Option<Update>, String> {
    let bytes = crate::platform::platform().download(MANIFEST_URL, MANIFEST_LIMIT).map_err(|error| error.to_string())?;
    parse_manifest(&bytes, env!("CARGO_PKG_VERSION"))
}

/// Download the setup and keep it only if its signature matches the updater key.
pub fn download(update: &Update) -> Result<PathBuf, String> {
    let bytes = crate::platform::platform().download(&update.url, SETUP_LIMIT).map_err(|error| error.to_string())?;
    verify(&bytes, &update.signature, PUBLIC_KEY)?;
    // The directory outlives Oxyde: Setup runs after the application exits.
    let directory = tempfile::Builder::new().prefix("oxyde-update-").tempdir().map_err(|error| error.to_string())?.keep();
    let path = directory.join(format!("Oxyde_FM_{}_x64-setup.exe", update.version));
    std::fs::write(&path, bytes).map_err(|error| error.to_string())?;
    Ok(path)
}

/// Start Setup in update mode; it waits for this process, skips its pages and relaunches Oxyde.
pub fn launch(setup: &std::path::Path) -> Result<(), String> {
    crate::platform::platform().launch_update(setup).map_err(|error| error.to_string())
}

fn parse_manifest(bytes: &[u8], current: &str) -> Result<Option<Update>, String> {
    let manifest: Manifest = serde_json::from_slice(bytes).map_err(|error| error.to_string())?;
    let version = manifest.version.trim_start_matches('v').to_owned();
    if !is_newer(&version, current)? {
        return Ok(None);
    }
    let asset = manifest.platforms.get(PLATFORM).ok_or("No Windows build in the update manifest")?;
    if !asset.url.starts_with(DOWNLOAD_PREFIX) {
        return Err(format!("Unexpected update location {}", asset.url));
    }
    Ok(Some(Update { version, url: asset.url.clone(), signature: asset.signature.clone() }))
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
