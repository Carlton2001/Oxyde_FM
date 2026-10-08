//! Resolve Windows tools without PATH or environment-provided Windows roots.
use super::SystemExecutable;
use std::ffi::OsString;
use std::io;
use std::os::windows::ffi::OsStringExt;
use std::path::PathBuf;
use windows::Win32::System::SystemInformation::{GetSystemDirectoryW, GetSystemWindowsDirectoryW};

fn directory(get: impl Fn(&mut [u16]) -> u32) -> io::Result<PathBuf> {
    let mut buffer = vec![0u16; 260];
    loop {
        let length = get(&mut buffer) as usize;
        if length == 0 {
            return Err(io::Error::last_os_error());
        }
        if length < buffer.len() {
            let path = PathBuf::from(OsString::from_wide(&buffer[..length]));
            if !path.is_absolute() {
                return Err(io::Error::other(
                    "Windows returned a relative system directory",
                ));
            }
            return Ok(path);
        }
        if length >= 32768 {
            return Err(io::Error::other("Windows system directory is too long"));
        }
        buffer.resize(length + 1, 0);
    }
}

pub(super) fn resolve(executable: SystemExecutable) -> io::Result<PathBuf> {
    match executable {
        SystemExecutable::PowerShell => {
            Ok(
                directory(|buffer| unsafe { GetSystemDirectoryW(Some(buffer)) })?
                    .join("WindowsPowerShell")
                    .join("v1.0")
                    .join("powershell.exe"),
            )
        }
        SystemExecutable::Explorer => {
            Ok(
                directory(|buffer| unsafe { GetSystemWindowsDirectoryW(Some(buffer)) })?
                    .join("explorer.exe"),
            )
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn system_powershell_ignores_child_path_and_working_directory() {
        use std::os::windows::process::CommandExt;
        let dir = tempfile::tempdir().unwrap();
        std::fs::write(dir.path().join("powershell.exe"), b"not an executable").unwrap();
        let path = crate::platform::platform()
            .system_executable(SystemExecutable::PowerShell)
            .unwrap();
        assert!(path.is_absolute() && path.is_file());
        let output = std::process::Command::new(path)
            .args([
                "-NoProfile",
                "-NonInteractive",
                "-Command",
                "[Console]::Write('oxyde-system')",
            ])
            .env("PATH", dir.path())
            .current_dir(dir.path())
            .creation_flags(0x08000000)
            .output()
            .unwrap();
        assert!(
            output.status.success(),
            "{}",
            String::from_utf8_lossy(&output.stderr)
        );
        assert_eq!(output.stdout, b"oxyde-system");
        let explorer = resolve(SystemExecutable::Explorer).unwrap();
        assert!(explorer.is_absolute() && explorer.is_file());
    }

    #[test]
    fn system_directories_ignore_environment_overrides() {
        use std::os::windows::process::CommandExt;
        const EXPECTED: &str = "OXYDE_TEST_SYSTEM_POWERSHELL";
        if let Some(expected) = std::env::var_os(EXPECTED) {
            assert_eq!(
                resolve(SystemExecutable::PowerShell).unwrap(),
                PathBuf::from(expected)
            );
            assert_eq!(
                resolve(SystemExecutable::Explorer).unwrap(),
                PathBuf::from(std::env::var_os("OXYDE_TEST_SYSTEM_EXPLORER").unwrap())
            );
            return;
        }
        let dir = tempfile::tempdir().unwrap();
        let status = std::process::Command::new(std::env::current_exe().unwrap())
            .args(["--exact", "platform::windows_executables::tests::system_directories_ignore_environment_overrides"])
            .env(EXPECTED, resolve(SystemExecutable::PowerShell).unwrap())
            .env("OXYDE_TEST_SYSTEM_EXPLORER", resolve(SystemExecutable::Explorer).unwrap())
            .env("PATH", dir.path()).env("SystemRoot", dir.path()).env("WINDIR", dir.path())
            .current_dir(dir.path()).creation_flags(0x08000000).status().unwrap();
        assert!(status.success());
    }
}
