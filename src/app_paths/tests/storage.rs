use super::*;

fn resolve(root: &Path) -> io::Result<StoragePaths> {
    StoragePaths::resolve(&root.join("Oxyde_FM.exe"), Some(root.join("profile/config")),
        Some(root.join("profile/cache")), root.join("system-temp"))
}

#[test]
fn absent_data_directory_preserves_installed_paths() {
    let root = tempfile::tempdir().unwrap();
    let paths = resolve(root.path()).unwrap();
    assert!(!paths.portable);
    assert_eq!(paths.config.unwrap(), root.path().join("profile/config/Oxyde"));
    assert_eq!(paths.cache.unwrap(), root.path().join("profile/cache/Oxyde"));
    assert_eq!(paths.temporary, root.path().join("system-temp"));
}

#[test]
fn portable_paths_and_temporary_files_stay_beside_executable() {
    let root = tempfile::tempdir().unwrap();
    let data = root.path().join("data");
    std::fs::create_dir(&data).unwrap();
    let paths = resolve(root.path()).unwrap();
    paths.prepare().unwrap();
    assert!(paths.portable);
    assert_eq!(paths.config.unwrap(), data);
    assert_eq!(paths.cache.unwrap(), data.join("cache"));
    assert_eq!(paths.temporary, data.join("temp"));
    assert!(data.join("temp").is_dir());
    assert!(!root.path().join("profile").exists());
    assert_eq!(std::fs::read_dir(&data).unwrap().count(), 1);
}

#[test]
fn a_data_file_is_an_error_instead_of_a_profile_fallback() {
    let root = tempfile::tempdir().unwrap();
    std::fs::write(root.path().join("data"), b"invalid marker").unwrap();
    assert!(resolve(root.path()).is_err());
    assert!(!root.path().join("profile").exists());
}

#[test]
fn portable_storage_never_uses_existing_user_preferences() {
    let root = tempfile::tempdir().unwrap();
    let profile = root.path().join("profile/config/Oxyde");
    std::fs::create_dir_all(&profile).unwrap();
    std::fs::write(profile.join("config.json"), b"existing preferences").unwrap();
    std::fs::create_dir(root.path().join("data")).unwrap();
    let paths = resolve(root.path()).unwrap();
    assert!(!paths.config.unwrap().join("config.json").exists());
    assert_eq!(std::fs::read(profile.join("config.json")).unwrap(), b"existing preferences");
}

#[test]
fn unavailable_portable_temp_does_not_fall_back_to_system_temp() {
    let root = tempfile::tempdir().unwrap();
    std::fs::create_dir(root.path().join("data")).unwrap();
    std::fs::write(root.path().join("data/temp"), b"blocked").unwrap();
    assert!(resolve(root.path()).unwrap().prepare().is_err());
    assert!(!root.path().join("system-temp").exists());
}

#[test]
fn portable_process_routes_configuration_caches_and_environment_locally() {
    const CHILD: &str = "OXYDE_PORTABLE_STORAGE_TEST";
    if let Some(root) = std::env::var_os(CHILD) {
        let root = PathBuf::from(root);
        initialize().unwrap();
        assert_eq!(config_dir().unwrap(), root.join("data"));
        assert_eq!(cache_dir().unwrap(), root.join("data/cache"));
        assert_eq!(temporary_dir().unwrap(), root.join("data/temp"));
        assert_eq!(std::env::temp_dir(), root.join("data/temp"));
        let config = crate::models::ConfigManager::config_path().unwrap();
        assert_eq!(config, root.join("data/config.json"));
        std::fs::write(&config, b"portable session").unwrap();
        let cache = cache_dir().unwrap();
        for name in ["icons", "thumbnails"] {
            std::fs::create_dir_all(cache.join(name)).unwrap();
            std::fs::write(cache.join(name).join("fixture"), b"cached").unwrap();
        }
        crate::commands::system::clear_app_cache().unwrap();
        assert!(!cache.join("icons").exists());
        assert!(!cache.join("thumbnails").exists());
        assert_eq!(std::fs::read(config).unwrap(), b"portable session");
        let temp = tempfile::tempdir().unwrap();
        assert!(temp.path().starts_with(root.join("data/temp")));
        return;
    }
    let root = tempfile::tempdir().unwrap();
    let unrelated = tempfile::tempdir().unwrap();
    std::fs::create_dir(root.path().join("data")).unwrap();
    let executable = root.path().join(if cfg!(windows) { "storage-test.exe" } else { "storage-test" });
    std::fs::copy(std::env::current_exe().unwrap(), &executable).unwrap();
    let run = || std::process::Command::new(&executable)
        .args(["--exact", "app_paths::tests::portable_process_routes_configuration_caches_and_environment_locally", "--nocapture"])
        .env(CHILD, root.path()).current_dir(unrelated.path()).output();
    // Linux: a process forked by another test while the copy was open for writing holds
    // that descriptor until it executes, and the copy is busy until then.
    let mut output = run();
    for _ in 0..100 {
        if !matches!(&output, Err(error) if error.kind() == std::io::ErrorKind::ExecutableFileBusy) { break }
        std::thread::sleep(std::time::Duration::from_millis(20));
        output = run();
    }
    let output = output.unwrap();
    assert!(output.status.success(), "{}\n{}", String::from_utf8_lossy(&output.stdout), String::from_utf8_lossy(&output.stderr));
    assert!(String::from_utf8_lossy(&output.stdout).contains("1 passed"));
    assert_eq!(std::fs::read(root.path().join("data/config.json")).unwrap(), b"portable session");
    assert_eq!(std::fs::read_dir(unrelated.path()).unwrap().count(), 0);
}
