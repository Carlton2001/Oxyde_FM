use super::*;

const MANIFEST: &str = r#"{
  "version": "2.0.0",
  "notes": "Notes",
  "pub_date": "2026-10-20T21:43:37.322Z",
  "platforms": {
    "windows-x86_64": {
      "signature": "c2ln",
      "url": "https://github.com/Carlton2001/Oxyde_FM/releases/download/v2.0.0/Oxyde_FM_2.0.0_x64-setup.exe"
    }
  }
}"#;

#[test]
fn newer_manifest_yields_the_windows_setup() {
    let update = parse_manifest(MANIFEST.as_bytes(), "1.2.1").unwrap().unwrap();
    assert_eq!(update.version, "2.0.0");
    assert!(update.url.ends_with("Oxyde_FM_2.0.0_x64-setup.exe"));
    assert_eq!(update.signature, "c2ln");
}

#[test]
fn current_or_older_manifest_is_not_an_update() {
    assert_eq!(parse_manifest(MANIFEST.as_bytes(), "2.0.0").unwrap(), None);
    assert_eq!(parse_manifest(MANIFEST.as_bytes(), "10.0.0").unwrap(), None);
}

#[test]
fn foreign_download_location_is_rejected() {
    let manifest = MANIFEST.replace("https://github.com/Carlton2001/Oxyde_FM/", "https://example.com/");
    assert!(parse_manifest(manifest.as_bytes(), "1.2.1").is_err());
}

#[test]
fn versions_compare_numerically() {
    assert!(is_newer("1.10.0", "1.9.9").unwrap());
    assert!(!is_newer("1.2.1", "1.2.1").unwrap());
    assert!(is_newer("2.0", "1.0.0").is_err());
}

/// Signature published with the Tauri 1.2.1 NSIS setup.
const TAURI_1_2_1_NSIS_SIGNATURE: &str = "dW50cnVzdGVkIGNvbW1lbnQ6IHNpZ25hdHVyZSBmcm9tIHRhdXJpIHNlY3JldCBrZXkKUlVSMy85OTVXMXp5ZEZEKzJmaDlERy93SHZHMzQvZ3hHUGlneklKRWkvSThZYnZ3YlBXMGpTdFd4YmN6aU8wdnhjaU9DYWZZSjE2cHp4My9HaVJ6Z2pNSUI3ZmJYc25UNGdRPQp0cnVzdGVkIGNvbW1lbnQ6IHRpbWVzdGFtcDoxNzc1ODUzNjA3CWZpbGU6T3h5ZGVfRk1fMS4yLjFfeDY0LXNldHVwLmV4ZQpqK2w0OWlzenVPci9JTXhGZzVLWFVNR0I2S1ZBM3JrcGpQdzNVR2xqMjRPbFR6Mkk5QmppVTJueVhQeHNCNGdPL21zT3M0Z3JnWC9ubCtEcnU5L2tDZz09Cg==";

#[test]
fn altered_setup_fails_signature_verification() {
    assert!(verify(b"not the published setup", TAURI_1_2_1_NSIS_SIGNATURE, PUBLIC_KEY).is_err());
}

#[test]
#[ignore = "downloads the published Tauri 1.2.1 setup"]
fn published_tauri_setup_matches_the_updater_key() {
    let url = "https://github.com/Carlton2001/Oxyde_FM/releases/download/v1.2.1/Oxyde_FM_1.2.1_x64-setup.exe";
    let bytes = crate::platform::platform().download(url, SETUP_LIMIT).unwrap();
    verify(&bytes, TAURI_1_2_1_NSIS_SIGNATURE, PUBLIC_KEY).unwrap();
}
