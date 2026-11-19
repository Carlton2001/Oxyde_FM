use super::*;

#[test]
fn only_oxyde_worker_monikers_are_ours() {
    assert!(ours("oxyde.archive.08e9f81b-2f4a-4cda-81ae-a3e4eda6d597"));
    assert!(ours("Oxyde.Archive.08E9F81B-2F4A-4CDA-81AE-A3E4EDA6D597"));
    assert!(!ours("oxyde.archive."));
    assert!(!ours("oxyde.archive.other"));
    assert!(!ours("microsoft.windowsnotepad_8wekyb3d8bbwe"));
}

#[test]
fn only_profiles_older_than_any_launch_are_stale() {
    let filetime = |seconds: u64| {
        let ticks = seconds * 10_000_000 + 116_444_736_000_000_000;
        FILETIME { dwLowDateTime: ticks as u32, dwHighDateTime: (ticks >> 32) as u32 }
    };
    let now = 1_800_000_000;
    assert!(!stale(filetime(now - 2), now));
    assert!(!stale(filetime(now + 60), now));
    assert!(stale(filetime(now - STALE_SECONDS), now));
}
