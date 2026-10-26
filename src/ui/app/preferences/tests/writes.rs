use super::*;

#[test]
fn an_older_snapshot_never_replaces_a_newer_one() {
    let directory = tempfile::tempdir().unwrap();
    let path = directory.path().join("config.json");
    let newer = AppConfig { theme: "newer".into(), ..AppConfig::default() };
    let older = AppConfig { theme: "older".into(), ..AppConfig::default() };
    // The quit hook writes the last generation while the cancelled chain may still finish an older one.
    write_snapshot(&path, &newer, u64::MAX - 1).unwrap();
    write_snapshot(&path, &older, u64::MAX - 2).unwrap();
    write_snapshot(&path, &older, u64::MAX - 1).unwrap();
    assert!(std::fs::read_to_string(&path).unwrap().contains("\"newer\""));
}
