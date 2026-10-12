use std::path::{Path, PathBuf};
use std::process::Command;

thread_local! {
    static PROBE: std::cell::RefCell<Option<(tempfile::TempDir, PathBuf)>> = const { std::cell::RefCell::new(None) };
}
// Each test thread owns its fixture and removes it after the last child exits.
pub fn executable() -> PathBuf {
    PROBE.with(|probe| {
        let mut probe = probe.borrow_mut();
        let (_, path) = probe.get_or_insert_with(|| {
            let directory = tempfile::tempdir().unwrap();
            let path = directory.path().join("sandbox-probe.exe");
            let status = Command::new("rustc")
                .args(["--edition=2021", "-C", "panic=abort", "-O"])
                .arg(Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/support/sandbox_probe.rs"))
                .arg("-o")
                .arg(&path)
                .status()
                .unwrap();
            assert!(
                status.success(),
                "native sandbox fixture compilation failed"
            );
            (directory, path)
        });
        path.clone()
    })
}
pub fn command(mode: &str) -> Command {
    let mut command = Command::new(executable());
    command.arg(mode);
    command
}
