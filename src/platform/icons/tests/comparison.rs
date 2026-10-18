//! Paired native benchmark; no timing thresholds in the regular test suite.
use super::*;
use serde_json::{json, Value};
use std::hint::black_box;
use std::path::Path;
use std::time::{Duration, Instant};

#[allow(dead_code)]
#[path = "baseline/mod.rs"]
mod baseline;

fn optimized(path: &str, size: &str, directory: Option<&Path>, key: &str) -> RgbaImage {
    extract_icon_image(
        path,
        size,
        false,
        directory.map(Path::to_path_buf),
        key.into(),
    )
    .unwrap()
}

fn clear() {
    baseline::clear();
    ICON_CACHE.lock().unwrap().clear();
}

fn timed(work: impl FnOnce() -> RgbaImage) -> (Duration, RgbaImage) {
    let started = Instant::now();
    let image = black_box(work());
    (started.elapsed(), image)
}

fn statistics(samples: &mut [f64]) -> Value {
    samples.sort_by(f64::total_cmp);
    json!({
        "samples": samples.len(),
        "median_us": samples[samples.len() / 2],
        "p90_us": samples[(samples.len() * 9 / 10).min(samples.len() - 1)],
    })
}

fn paired(
    repetitions: usize,
    mut setup: impl FnMut(),
    mut original: impl FnMut() -> RgbaImage,
    mut updated: impl FnMut() -> RgbaImage,
) -> Value {
    let mut before = Vec::new();
    let mut after = Vec::new();
    for index in 0..repetitions + 4 {
        let ((old_time, old), (new_time, new)) = if index % 2 == 0 {
            setup();
            let old = timed(&mut original);
            setup();
            (old, timed(&mut updated))
        } else {
            setup();
            let new = timed(&mut updated);
            setup();
            (timed(&mut original), new)
        };
        assert_eq!(
            old, new,
            "Both implementations must produce identical pixels"
        );
        if index >= 4 {
            before.push(old_time.as_secs_f64() * 1e6);
            after.push(new_time.as_secs_f64() * 1e6);
        }
    }
    let old = statistics(&mut before);
    let new = statistics(&mut after);
    let reduction =
        100.0 * (1.0 - new["median_us"].as_f64().unwrap() / old["median_us"].as_f64().unwrap());
    json!({"original": old, "optimized": new, "median_time_reduction_percent": reduction,
        "original_samples_us": before, "optimized_samples_us": after})
}

#[test]
#[ignore = "manual paired native benchmark against the frozen original implementation"]
fn compare_original_and_optimized_icons() {
    let _com = super::super::windows_com::ComApartment::mta_or_existing().unwrap();
    let directory = tempfile::tempdir().unwrap();
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
            let persisted = directory.path().join(format!("{key}.png"));
            // Warm the Windows Shell/image-list cache before comparing Oxyde.
            let expected = optimized(&path, size, Some(directory.path()), &key);
            assert_eq!(baseline::image(&path, size, None, &key), expected);

            let cold = paired(
                30,
                || {
                    clear();
                    if persisted.exists() {
                        std::fs::remove_file(&persisted).unwrap();
                    }
                },
                || {
                    baseline::image(&path, size, Some(directory.path()), &key)
                },
                || {
                    optimized(&path, size, Some(directory.path()), &key)
                },
            );

            let disk = paired(
                300,
                || {},
                || baseline::disk(&persisted),
                || super::super::windows_icon_cache::read_cached_png(&persisted).unwrap(),
            );

            clear();
            baseline::image(&path, size, None, &key);
            optimized(&path, size, None, &key);
            let memory = paired(
                3000,
                || {},
                || baseline::image(&path, size, None, &key),
                || optimized(&path, size, None, &key),
            );

            // Parent RAM caching has the same code in both versions; this test
            // specifically measures the worker's PNG vs decoded-pixel cache.
            let case = json!({"executable": name, "size": size, "cold_oxyde_cache": cold,
                "parent_disk_cache": disk, "worker_memory_cache": memory});
            for phase in [
                "cold_oxyde_cache",
                "parent_disk_cache",
                "worker_memory_cache",
            ] {
                eprintln!(
                    "{name} {size} {phase}: {:.2} -> {:.2} us ({:+.1}% less time)",
                    case[phase]["original"]["median_us"].as_f64().unwrap(),
                    case[phase]["optimized"]["median_us"].as_f64().unwrap(),
                    case[phase]["median_time_reduction_percent"]
                        .as_f64()
                        .unwrap()
                );
            }
            cases.push(case);
        }
    }
    let report = json!({
        "baseline_commit": "fcf724874a05fc4c58ab4f6f3093b9bafe13a645",
        "scope": "Native Shell extraction + persistence; parent disk cache; worker memory cache. No IPC, process startup or GPUI rendering.",
        "method": "Alternating paired order, 4 warmups per case/phase, identical executables and pixels, Windows/OS caches warm. Raw samples sorted in microseconds.",
        "cases": cases,
    });
    let output = std::env::var_os("OXYDE_ICON_BENCH_REPORT")
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from("target/native-tests/icon-comparison.json"));
    std::fs::create_dir_all(output.parent().unwrap()).unwrap();
    std::fs::write(&output, serde_json::to_vec_pretty(&report).unwrap()).unwrap();
    eprintln!("Report: {}", output.display());
}
