use super::*;
use std::time::Instant;

#[test]
fn executable_icons_keep_pixels_across_generation_disk_and_memory() {
    let _com = super::super::windows_com::ComApartment::mta_or_existing().unwrap();
    let directory = tempfile::tempdir().unwrap();
    let system = PathBuf::from(std::env::var_os("SystemRoot").unwrap()).join("System32");
    for name in [
        "cmd.exe",
        "reg.exe",
        "WindowsPowerShell/v1.0/powershell.exe",
    ] {
        let path = system.join(name);
        let path = shell_icon_path(path.to_str().unwrap());
        for (size, side) in [("small", 32), ("large", 96)] {
            let memory = Mutex::new(super::super::windows_icon_cache::IconCache::default());
            let key = cache_key(&path, size);
            let started = Instant::now();
            let generated = super::super::windows_icon_cache::cached_or_generate(
                &memory,
                Some(directory.path()),
                &key,
                || generate_icon_image(&path, size, false),
            )
            .unwrap();
            let generation = started.elapsed();
            assert_eq!(generated.dimensions(), (side, side));
            assert!(generated.pixels().any(|pixel| pixel[3] != 0));

            memory.lock().unwrap().clear();
            let started = Instant::now();
            let disk = super::super::windows_icon_cache::cached_or_generate(
                &memory,
                Some(directory.path()),
                &key,
                || panic!("Persisted executable icon must avoid Shell extraction"),
            )
            .unwrap();
            let disk_time = started.elapsed();
            assert_eq!(disk, generated);

            std::fs::remove_file(directory.path().join(format!("{key}.png"))).unwrap();
            let started = Instant::now();
            let cached = super::super::windows_icon_cache::cached_or_generate(
                &memory,
                Some(directory.path()),
                &key,
                || panic!("Decoded executable icon must remain cached in memory"),
            )
            .unwrap();
            eprintln!("{name} ({size}): generation + persistence {generation:?}, disk {disk_time:?}, memory {:?}", started.elapsed());
            assert_eq!(cached, generated);
        }
    }
}
