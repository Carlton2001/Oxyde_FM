//! Where tests find the executables built by `cargo build --bins`.

use std::path::PathBuf;

/// The built executable `name`, in the nearest folder above the test binary that holds it.
/// Cargo places test binaries in `deps` or, since Rust 1.100, deeper under `build`.
pub(crate) fn path(name: &str) -> PathBuf {
    let test = std::env::current_exe().unwrap();
    test.ancestors()
        .skip(1)
        .map(|folder| folder.join(name))
        .find(|candidate| candidate.is_file())
        .unwrap_or_else(|| test.parent().unwrap().parent().unwrap().join(name))
}
