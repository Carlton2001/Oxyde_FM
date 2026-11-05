use super::*;

const WINDOWS: Installation = Installation::SelfUpdating { manifest_key: "windows-x86_64" };
const ARCHIVE: Installation = Installation::SelfUpdating { manifest_key: "linux-x86_64" };
const DEB: Installation = Installation::Package { manifest_key: Some("linux-x86_64-deb") };
const RPM: Installation = Installation::Package { manifest_key: None };

const MANIFEST: &str = r#"{
  "version": "2.0.0",
  "notes": "Notes",
  "pub_date": "2026-10-20T21:43:37.322Z",
  "platforms": {
    "windows-x86_64": {
      "signature": "c2ln",
      "url": "https://github.com/Carlton2001/Oxyde_FM/releases/download/v2.0.0/Oxyde_FM_2.0.0_x64-setup.exe"
    },
    "linux-x86_64": {
      "signature": "dGFy",
      "url": "https://github.com/Carlton2001/Oxyde_FM/releases/download/v2.0.0/oxyde-2.0.0-x86_64-linux.tar.gz"
    },
    "linux-x86_64-deb": {
      "signature": "ZGVi",
      "url": "https://github.com/Carlton2001/Oxyde_FM/releases/download/v2.0.0/oxyde_2.0.0-1_amd64.deb"
    }
  }
}"#;

/// Oxyde 2.0.0's manifest, published before the Linux entries.
const WINDOWS_ONLY: &str = r#"{
  "version": "2.0.0",
  "platforms": {
    "windows-x86_64": {
      "signature": "c2ln",
      "url": "https://github.com/Carlton2001/Oxyde_FM/releases/download/v2.0.0/Oxyde_FM_2.0.0_x64-setup.exe"
    }
  }
}"#;

#[test]
fn newer_manifest_yields_the_windows_setup() {
    let update = parse_manifest(MANIFEST.as_bytes(), "1.2.1", WINDOWS).unwrap().unwrap();
    assert_eq!(update.version, "2.0.0");
    assert!(update.url.ends_with("Oxyde_FM_2.0.0_x64-setup.exe"));
    assert_eq!(update.signature, "c2ln");
    // A portable copy downloads by hand from the release page.
    assert_eq!(update.manual_download(), "https://github.com/Carlton2001/Oxyde_FM/releases/tag/v2.0.0");
}

#[test]
fn per_user_linux_installation_gets_the_signed_archive() {
    let update = parse_manifest(MANIFEST.as_bytes(), "1.9.0", ARCHIVE).unwrap().unwrap();
    assert!(update.url.ends_with("oxyde-2.0.0-x86_64-linux.tar.gz"));
    assert_eq!(update.signature, "dGFy");
}

#[test]
fn release_without_a_build_for_this_system_is_not_an_update() {
    assert_eq!(parse_manifest(WINDOWS_ONLY.as_bytes(), "1.9.0", ARCHIVE).unwrap(), None);
}

#[test]
fn packages_download_the_deb_or_open_the_release_page() {
    let deb = parse_manifest(MANIFEST.as_bytes(), "1.9.0", DEB).unwrap().unwrap();
    assert!(deb.manual_download().ends_with("oxyde_2.0.0-1_amd64.deb"));
    let rpm = parse_manifest(MANIFEST.as_bytes(), "1.9.0", RPM).unwrap().unwrap();
    assert_eq!(rpm.manual_download(), "https://github.com/Carlton2001/Oxyde_FM/releases/tag/v2.0.0");
    // A release without the DEB still announces itself through its page.
    let unpublished = parse_manifest(WINDOWS_ONLY.as_bytes(), "1.9.0", DEB).unwrap().unwrap();
    assert_eq!(unpublished.manual_download(), "https://github.com/Carlton2001/Oxyde_FM/releases/tag/v2.0.0");
}

#[test]
fn unmanaged_copies_get_no_update() {
    assert_eq!(parse_manifest(MANIFEST.as_bytes(), "1.9.0", Installation::Unmanaged).unwrap(), None);
}

#[test]
fn current_or_older_manifest_is_not_an_update() {
    assert_eq!(parse_manifest(MANIFEST.as_bytes(), "2.0.0", WINDOWS).unwrap(), None);
    assert_eq!(parse_manifest(MANIFEST.as_bytes(), "10.0.0", ARCHIVE).unwrap(), None);
}

#[test]
fn foreign_download_location_is_rejected() {
    let manifest = MANIFEST.replace("https://github.com/Carlton2001/Oxyde_FM/", "https://example.com/");
    assert!(parse_manifest(manifest.as_bytes(), "1.2.1", WINDOWS).is_err());
    assert!(parse_manifest(manifest.as_bytes(), "1.2.1", ARCHIVE).is_err());
    assert!(parse_manifest(manifest.as_bytes(), "1.2.1", DEB).is_err());
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
