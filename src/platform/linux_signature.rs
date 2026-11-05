//! Release signatures on Linux: GPG, with Oxyde's package signing key compiled in and
//! checked by gpgv, which every distribution's package manager already relies on.
//! gpgv reads a private keyring and an empty home, never the user's keys or trust.
use base64::Engine;
use std::io;
use std::path::Path;
use std::process::{Command, Stdio};

/// Committed beside the packaging; scripts/sign-linux-release.sh checks the secret key against it.
const PACKAGE_KEY: &str = include_str!("../../packaging/linux/signing/oxyde-packages.asc");
const PACKAGE_KEY_FINGERPRINT: &str = "F9D76B9E3BEDD5EACC9CFE7DAC2E961F32029706";

/// Check `signature`, the base64 of an armored detached signature, as the update
/// manifest carries it, or the armored text itself.
pub(super) fn verify(data: &Path, signature: &[u8]) -> io::Result<()> {
    verify_with(PACKAGE_KEY, PACKAGE_KEY_FINGERPRINT, data, signature)
}

fn verify_with(key: &str, fingerprint: &str, data: &Path, signature: &[u8]) -> io::Result<()> {
    let invalid = |message: &str| io::Error::new(io::ErrorKind::InvalidData, message.to_owned());
    let signature = if signature.starts_with(b"-----BEGIN PGP SIGNATURE-----") {
        signature.to_vec()
    } else {
        base64::engine::general_purpose::STANDARD
            .decode(signature.iter().copied().filter(|byte| !byte.is_ascii_whitespace()).collect::<Vec<_>>())
            .map_err(|_| invalid("The update signature is not base64"))?
    };
    let folder = tempfile::Builder::new().prefix("oxyde-gpgv-").tempdir()?;
    let home = folder.path().join("home");
    std::fs::create_dir(&home)?;
    let keyring = folder.path().join("keyring.gpg");
    std::fs::write(&keyring, dearmor(key).ok_or_else(|| invalid("The package signing key is not armored OpenPGP"))?)?;
    let detached = folder.path().join("signature.asc");
    std::fs::write(&detached, signature)?;
    let output = Command::new("gpgv")
        .arg("--homedir").arg(&home)
        .arg("--keyring").arg(&keyring)
        .args(["--status-fd", "1"])
        .arg(&detached).arg(data)
        .stdin(Stdio::null()).stderr(Stdio::null())
        .output()
        .map_err(|error| if error.kind() == io::ErrorKind::NotFound {
            io::Error::new(io::ErrorKind::NotFound, "gpgv is needed to verify updates")
        } else {
            error
        })?;
    let status = String::from_utf8_lossy(&output.stdout);
    // VALIDSIG ends with the primary key's fingerprint, whichever subkey signed.
    let signed_by_key = status.lines()
        .filter_map(|line| line.strip_prefix("[GNUPG:] VALIDSIG "))
        .any(|fields| fields.split_whitespace().last().is_some_and(|primary| primary.eq_ignore_ascii_case(fingerprint)));
    if output.status.success() && signed_by_key {
        Ok(())
    } else {
        Err(invalid("The update's signature does not match Oxyde's package signing key"))
    }
}

/// The binary packets of an ASCII-armored block: gpgv only reads binary keyrings.
fn dearmor(armored: &str) -> Option<Vec<u8>> {
    let mut lines = armored.lines().map(str::trim_end)
        .skip_while(|line| !line.starts_with("-----BEGIN PGP "));
    lines.next()?;
    // Header lines ("Comment: …") end at the first blank line.
    let body: String = lines.skip_while(|line| !line.is_empty())
        .skip(1)
        .take_while(|line| !line.starts_with("-----END PGP "))
        .filter(|line| !line.starts_with('='))
        .collect();
    base64::engine::general_purpose::STANDARD.decode(body).ok().filter(|bytes| !bytes.is_empty())
}

#[cfg(test)]
#[path = "linux_signature/tests/signature.rs"]
mod tests;
