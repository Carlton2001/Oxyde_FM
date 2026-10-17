use super::*;
use crate::platform::windows_reparse_tests::junction;

fn tree(root: &Path, folders: usize) -> std::path::PathBuf {
    let selected = root.join("selected");
    for folder in 0..folders {
        let nested = selected.join(format!("f{folder:02}")).join("inner");
        std::fs::create_dir_all(&nested).unwrap();
        for file in 0..20 {
            std::fs::write(nested.join(format!("{file}.bin")), b"data").unwrap();
        }
    }
    std::fs::write(selected.join("top.bin"), b"top").unwrap();
    selected
}

#[test]
fn parallel_subfolders_are_removed_without_following_nested_junctions() {
    let root = tempfile::tempdir().unwrap();
    let outside = root.path().join("outside");
    std::fs::create_dir(&outside).unwrap();
    std::fs::write(outside.join("witness"), b"outside witness").unwrap();
    let selected = tree(root.path(), 24);
    junction(&selected.join("f07").join("inner").join("link"), &outside);
    let calls = std::sync::atomic::AtomicUsize::new(0);
    remove(&selected, 4, &|| {
        calls.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
        Ok(())
    })
    .unwrap();
    assert!(!selected.exists());
    assert!(calls.into_inner() > 24 * 20);
    assert_eq!(std::fs::read(outside.join("witness")).unwrap(), b"outside witness");
}

#[test]
fn a_failing_subfolder_stops_the_others_and_keeps_the_selected_folder() {
    let root = tempfile::tempdir().unwrap();
    let selected = tree(root.path(), 24);
    let protected = selected.join("f03").join("inner").join("readonly");
    std::fs::write(&protected, b"protected data").unwrap();
    let mut permissions = std::fs::metadata(&protected).unwrap().permissions();
    permissions.set_readonly(true);
    std::fs::set_permissions(&protected, permissions.clone()).unwrap();
    let error = remove(&selected, 4, &|| Ok(())).unwrap_err();
    assert_eq!(error.kind(), io::ErrorKind::PermissionDenied);
    assert!(selected.exists());
    assert_eq!(std::fs::read(&protected).unwrap(), b"protected data");
    permissions.set_readonly(false);
    std::fs::set_permissions(&protected, permissions).unwrap();
}

#[test]
fn cancellation_reaches_every_worker_and_releases_handles() {
    let root = tempfile::tempdir().unwrap();
    let selected = tree(root.path(), 24);
    let calls = std::sync::atomic::AtomicUsize::new(0);
    let error = remove(&selected, 4, &|| {
        if calls.fetch_add(1, std::sync::atomic::Ordering::SeqCst) > 40 {
            return Err(io::Error::new(io::ErrorKind::Interrupted, "cancelled"));
        }
        Ok(())
    })
    .unwrap_err();
    assert_eq!(error.kind(), io::ErrorKind::Interrupted);
    assert!(selected.exists());
    // No handle is left open on the remaining tree.
    std::fs::rename(&selected, root.path().join("released")).unwrap();
}
