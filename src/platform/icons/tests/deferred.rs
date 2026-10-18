use super::*;
use serde_json::json;
use std::time::{Duration, Instant};

#[test]
#[ignore = "manual native latency benchmark before and after deferred persistence"]
fn compare_synchronous_and_deferred_persistence() {
    let _com = super::super::windows_com::ComApartment::mta_or_existing().unwrap();
    let root = tempfile::tempdir().unwrap();
    let system = PathBuf::from(std::env::var_os("SystemRoot").unwrap()).join("System32");
    let mut cases = Vec::new();
    for name in [
        "cmd.exe",
        "reg.exe",
        "WindowsPowerShell/v1.0/powershell.exe",
    ] {
        let path = shell_icon_path(system.join(name).to_str().unwrap());
        for size in ["small", "large"] {
            let key = cache_key(&path, size);
            let persisted = root.path().join(format!("{key}.png"));
            // Preheat the Windows Shell cache, as in the two earlier comparisons.
            generate_icon_image(&path, size, false).unwrap();
            let mut synchronous = Vec::new();
            let mut deferred = Vec::new();
            let mut persisted_total = Vec::new();
            for index in 0..34 {
                let mut images = Vec::new();
                let mut total = 0.0;
                for position in 0..2 {
                    purge_icon_cache();
                    if persisted.exists() {
                        std::fs::remove_file(&persisted).unwrap();
                    }
                    let defer = (position + index) % 2 != 0;
                    let revision = super::super::windows_icon_persistence::revision().unwrap();
                    let started = Instant::now();
                    let image = if defer {
                        // Actual worker path returns pixels without encoding/writing.
                        let image = get_file_icon(path.clone(), size.into()).unwrap();
                        // The parent admits only validated pixels; IPC and parent
                        // RAM insertion remain outside this native-path benchmark.
                        super::super::windows_icon_persistence::enqueue(
                            revision,
                            root.path(),
                            &key,
                            &image,
                        );
                        image
                    } else {
                        extract_icon_image(
                            &path,
                            size,
                            false,
                            Some(root.path().into()),
                            key.clone(),
                        )
                        .unwrap()
                    };
                    let ready = started.elapsed().as_secs_f64() * 1e6;
                    if defer {
                        // Wait outside the ready-to-display measurement and verify
                        // every deferred file really completed with identical pixels.
                        assert!(super::super::windows_icon_persistence::wait_idle(
                            Duration::from_secs(2)
                        ));
                        total = started.elapsed().as_secs_f64() * 1e6;
                    }
                    assert_eq!(
                        super::super::windows_icon_cache::read_cached_png(&persisted).unwrap(),
                        image
                    );
                    images.push(image);
                    if index >= 4 {
                        if defer {
                            deferred.push(ready);
                        } else {
                            synchronous.push(ready);
                        }
                    }
                }
                assert_eq!(images[0], images[1]);
                if index >= 4 {
                    persisted_total.push(total);
                }
            }
            synchronous.sort_by(f64::total_cmp);
            deferred.sort_by(f64::total_cmp);
            persisted_total.sort_by(f64::total_cmp);
            eprintln!("{name} {size}: synchronous {:.2} us, pixels ready {:.2} us, persistence complete {:.2} us",
                synchronous[15], deferred[15], persisted_total[15]);
            cases.push(json!({"executable": name, "size": size,
                "synchronous_samples_us": synchronous, "deferred_ready_samples_us": deferred,
                "deferred_persisted_samples_us": persisted_total}));
        }
    }
    let report = json!({"scope": "Native worker extraction plus parent queue admission; no IPC, process startup, parent RAM insertion or GPUI rendering. Every deferred write drained and verified outside ready timing.",
        "method": "Same executable corpus, warm Windows Shell, 4 excluded warmups + 30 alternating pairs per size/executable. App caches cleared outside timing.", "cases": cases});
    let output = std::env::var_os("OXYDE_ICON_BENCH_REPORT")
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from("target/native-tests/deferred-icons/comparison.json"));
    std::fs::create_dir_all(output.parent().unwrap()).unwrap();
    std::fs::write(output, serde_json::to_vec_pretty(&report).unwrap()).unwrap();
}
