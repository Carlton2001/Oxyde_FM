use super::*;

/// A throwaway signing key in its own GnuPG home.
struct Signer {
    folder: tempfile::TempDir,
    key: String,
    fingerprint: String,
}

impl Signer {
    fn new(name: &str) -> Self {
        let folder = tempfile::tempdir().unwrap();
        let home = folder.path().join("home");
        std::fs::create_dir(&home).unwrap();
        std::fs::set_permissions(&home, std::os::unix::fs::PermissionsExt::from_mode(0o700)).unwrap();
        let signer = Self { folder, key: String::new(), fingerprint: String::new() };
        signer.gpg(&["--passphrase", "", "--quick-gen-key", &format!("{name} <{name}@example.com>"), "ed25519", "sign", "never"]);
        let listing = signer.gpg(&["--with-colons", "--list-keys"]);
        let fingerprint = listing.lines().find_map(|line| line.strip_prefix("fpr:")).unwrap()
            .split(':').find(|field| field.len() == 40).unwrap().to_owned();
        let key = signer.gpg(&["--armor", "--export", &fingerprint]);
        Self { key, fingerprint, ..signer }
    }

    fn gpg(&self, arguments: &[&str]) -> String {
        let output = Command::new("gpg").arg("--homedir").arg(self.folder.path().join("home"))
            .args(["--batch", "--pinentry-mode", "loopback"]).args(arguments).output().unwrap();
        assert!(output.status.success(), "{}", String::from_utf8_lossy(&output.stderr));
        String::from_utf8(output.stdout).unwrap()
    }

    /// An armored detached signature of `path`.
    fn sign(&self, path: &Path) -> Vec<u8> {
        self.gpg(&["--armor", "--detach-sign", "--output", "-", path.to_str().unwrap()]).into_bytes()
    }
}

fn file(contents: &[u8]) -> (tempfile::TempDir, std::path::PathBuf) {
    let folder = tempfile::tempdir().unwrap();
    let path = folder.path().join("oxyde-9.9.9-x86_64-linux.tar.gz");
    std::fs::write(&path, contents).unwrap();
    (folder, path)
}

#[test]
fn signature_of_the_key_is_accepted_armored_or_in_base64() {
    let signer = Signer::new("release");
    let (_folder, path) = file(b"archive");
    let armored = signer.sign(&path);
    verify_with(&signer.key, &signer.fingerprint, &path, &armored).unwrap();
    let encoded = base64::engine::general_purpose::STANDARD.encode(&armored);
    verify_with(&signer.key, &signer.fingerprint, &path, encoded.as_bytes()).unwrap();
}

#[test]
fn altered_file_is_refused() {
    let signer = Signer::new("release");
    let (_folder, path) = file(b"archive");
    let signature = signer.sign(&path);
    std::fs::write(&path, b"archive, altered").unwrap();
    assert_eq!(verify_with(&signer.key, &signer.fingerprint, &path, &signature).unwrap_err().kind(), io::ErrorKind::InvalidData);
}

#[test]
fn another_key_is_refused() {
    let signer = Signer::new("release");
    let intruder = Signer::new("intruder");
    let (_folder, path) = file(b"archive");
    let signature = intruder.sign(&path);
    assert!(verify_with(&signer.key, &signer.fingerprint, &path, &signature).is_err());
    // Its own key in the keyring is not enough: the fingerprint is pinned too.
    assert!(verify_with(&intruder.key, &signer.fingerprint, &path, &signature).is_err());
}

#[test]
fn garbage_signature_is_refused() {
    let signer = Signer::new("release");
    let (_folder, path) = file(b"archive");
    assert!(verify_with(&signer.key, &signer.fingerprint, &path, b"not base64 !").is_err());
    assert!(verify_with(&signer.key, &signer.fingerprint, &path, b"bm90IGEgc2lnbmF0dXJl").is_err());
}

#[test]
fn committed_key_dearmors_as_gpg_does_and_has_the_pinned_fingerprint() {
    let output = Command::new("gpg").args(["--batch", "--dearmor"]).stdin(Stdio::piped()).stdout(Stdio::piped())
        .spawn().and_then(|mut child| {
            use std::io::Write;
            child.stdin.take().unwrap().write_all(PACKAGE_KEY.as_bytes())?;
            child.wait_with_output()
        }).unwrap();
    assert_eq!(dearmor(PACKAGE_KEY).unwrap(), output.stdout);
    let listing = Command::new("gpg").args(["--batch", "--with-colons", "--show-keys"])
        .arg(concat!(env!("CARGO_MANIFEST_DIR"), "/packaging/linux/signing/oxyde-packages.asc")).output().unwrap();
    let listing = String::from_utf8(listing.stdout).unwrap();
    let primary = listing.lines().find_map(|line| line.strip_prefix("fpr:")).unwrap();
    assert!(primary.contains(PACKAGE_KEY_FINGERPRINT), "{listing}");
}

#[test]
fn armor_without_body_is_rejected() {
    assert_eq!(dearmor("-----BEGIN PGP PUBLIC KEY BLOCK-----\n\n-----END PGP PUBLIC KEY BLOCK-----\n"), None);
    assert_eq!(dearmor("no armor"), None);
}

const INSTALL_SCRIPT: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/packaging/linux/install.sh");

#[test]
fn install_script_embeds_the_committed_key() {
    let script = std::fs::read_to_string(INSTALL_SCRIPT).unwrap();
    assert!(script.contains(&format!("cat <<'KEY'\n{PACKAGE_KEY}KEY\n")));
}

/// install.sh's checks of a release folder, with a throwaway key standing for Oxyde's.
fn install_script_checks(signer: &Signer, folder: &Path, name: &str) -> bool {
    let key = folder.join("key.asc");
    std::fs::write(&key, &signer.key).unwrap();
    Command::new("bash").arg("-c")
        .arg(r#"source "$1"; key=$2; package_key() { cat "$key"; }; verify_checksums "$3" && check_file "$3" "$4""#)
        .arg("install-test").arg(INSTALL_SCRIPT).arg(&key).arg(folder).arg(name)
        .stdin(Stdio::null()).stdout(Stdio::null()).stderr(Stdio::null())
        .status().unwrap().success()
}

#[test]
fn install_script_trusts_only_checksums_signed_by_the_key() {
    let signer = Signer::new("release");
    let intruder = Signer::new("intruder");
    let name = "oxyde-9.9.9-x86_64-linux.tar.gz";
    let (folder, path) = file(b"archive");
    let sums = folder.path().join("SHA256SUMS");
    let listing = Command::new("sha256sum").arg(name).current_dir(folder.path()).output().unwrap().stdout;
    std::fs::write(&sums, &listing).unwrap();
    std::fs::write(folder.path().join("SHA256SUMS.asc"), signer.sign(&sums)).unwrap();
    assert!(install_script_checks(&signer, folder.path(), name));
    assert!(!install_script_checks(&signer, folder.path(), "install.sh"), "a file missing from SHA256SUMS");

    std::fs::write(&path, b"archive, altered").unwrap();
    assert!(!install_script_checks(&signer, folder.path(), name));
    std::fs::write(&path, b"archive").unwrap();

    std::fs::write(folder.path().join("SHA256SUMS.asc"), intruder.sign(&sums)).unwrap();
    assert!(!install_script_checks(&signer, folder.path(), name));
}
