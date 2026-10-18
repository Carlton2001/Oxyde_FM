//! Compare the retained Tauri extractor with current GPUI, outside either UI.
use super::*;
use serde_json::{json, Value};
use std::hint::black_box;
use std::io::{Cursor, Write};
use std::path::Path;
use std::time::Instant;

#[path = "tauri/source.rs"]
mod tauri;

enum Output {
    Png(Vec<u8>),
    Pixels(RgbaImage),
}
impl Output {
    fn pixels(self) -> RgbaImage {
        match self {
            Self::Png(bytes) => super::super::windows_icon_cache::decode_icon_png(&bytes).unwrap(),
            Self::Pixels(image) => image,
        }
    }
}

fn clear() {
    tauri::purge_icon_cache();
    ICON_CACHE.lock().unwrap().clear();
}

fn tauri_png(path: &str, size: &str, directory: Option<&Path>, key: &str) -> Vec<u8> {
    tauri::extract_icon_png(
        path,
        size,
        false,
        directory.map(Path::to_path_buf),
        key.into(),
    )
    .unwrap()
}

fn gpui_pixels(path: &str, size: &str, directory: Option<&Path>, key: &str) -> Output {
    Output::Pixels(
        extract_icon_image(
            path,
            size,
            false,
            directory.map(Path::to_path_buf),
            key.into(),
        )
        .unwrap(),
    )
}

fn gpui_png(path: &str, size: &str) -> Output {
    let image = generate_icon_image(path, size, false).unwrap();
    let mut output = Cursor::new(Vec::new());
    image
        .write_to(&mut output, image::ImageFormat::Png)
        .unwrap();
    Output::Png(output.into_inner())
}

fn tauri_with_gpui_persistence(path: &str, size: &str, directory: &Path, key: &str) -> Output {
    let bytes = tauri_png(path, size, None, key);
    super::super::platform()
        .write_preview_cache(
            &directory.join(format!("{key}.png")),
            &directory.join(format!("{key}.{}.0.tmp", std::process::id())),
            &mut |file| file.write_all(&bytes),
        )
        .unwrap();
    Output::Png(bytes)
}

fn timed(work: &mut impl FnMut() -> Output) -> (f64, Output) {
    let started = Instant::now();
    let output = black_box(work());
    (started.elapsed().as_secs_f64() * 1e6, output)
}

fn statistics(samples: &mut [f64]) -> Value {
    samples.sort_by(f64::total_cmp);
    json!({"samples": samples.len(), "median_us": samples[samples.len()/2],
        "p90_us": samples[(samples.len()*9/10).min(samples.len()-1)]})
}

fn paired(
    count: usize,
    mut setup: impl FnMut(),
    mut tauri: impl FnMut() -> Output,
    mut gpui: impl FnMut() -> Output,
) -> Value {
    let mut before = Vec::new();
    let mut after = Vec::new();
    let mut mismatched_pairs = 0;
    let mut max_channel_difference = 0;
    let mut max_differing_pixels = 0;
    for index in 0..count + 4 {
        let ((old_time, old), (new_time, new)) = if index % 2 == 0 {
            setup();
            let old = timed(&mut tauri);
            setup();
            (old, timed(&mut gpui))
        } else {
            setup();
            let new = timed(&mut gpui);
            setup();
            (timed(&mut tauri), new)
        };
        // Decode outside timing: Tauri returns PNG to WebView, GPUI keeps pixels.
        let old = old.pixels();
        let new = new.pixels();
        assert_eq!(old.dimensions(), new.dimensions());
        assert!(old.pixels().any(|pixel| pixel[3] > 0));
        assert!(new.pixels().any(|pixel| pixel[3] > 0));
        if old != new {
            mismatched_pairs += 1;
            let mut differing = 0;
            for (old, new) in old.pixels().zip(new.pixels()) {
                differing += usize::from(old != new);
                for (old, new) in old.0.into_iter().zip(new.0) {
                    max_channel_difference = max_channel_difference.max(old.abs_diff(new));
                }
            }
            max_differing_pixels = max_differing_pixels.max(differing);
        }
        if index >= 4 {
            before.push(old_time);
            after.push(new_time);
        }
    }
    json!({"tauri": statistics(&mut before), "gpui": statistics(&mut after),
        "tauri_samples_us": before, "gpui_samples_us": after,
        "mismatched_pairs_including_warmup": mismatched_pairs,
        "max_differing_pixels": max_differing_pixels,
        "max_channel_difference": max_channel_difference})
}

#[test]
#[ignore = "manual native benchmark against retained Tauri extraction"]
fn compare_tauri_and_gpui_icons() {
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
            // Both extractors use the same warmed Shell lists and input files.
            black_box(gpui_png(&path, size));
            clear();
            let extraction = paired(
                30,
                clear,
                || Output::Png(tauri_png(&path, size, None, &key)),
                || gpui_png(&path, size),
            );

            let reset = || {
                clear();
                if persisted.exists() {
                    std::fs::remove_file(&persisted).unwrap();
                }
            };
            let own_persistence = paired(
                30,
                reset,
                || Output::Png(tauri_png(&path, size, Some(directory.path()), &key)),
                || gpui_pixels(&path, size, Some(directory.path()), &key),
            );

            let same_persistence = paired(
                30,
                reset,
                || tauri_with_gpui_persistence(&path, size, directory.path(), &key),
                || gpui_pixels(&path, size, Some(directory.path()), &key),
            );

            let disk = paired(
                300,
                clear,
                || Output::Png(tauri_png(&path, size, Some(directory.path()), &key)),
                || {
                    Output::Pixels(
                        super::super::windows_icon_cache::read_cached_png(&persisted).unwrap(),
                    )
                },
            );

            clear();
            black_box(tauri_png(&path, size, None, &key));
            black_box(gpui_pixels(&path, size, None, &key));
            let memory = paired(
                1000,
                || {},
                || Output::Png(tauri_png(&path, size, None, &key)),
                || gpui_pixels(&path, size, None, &key),
            );
            let case = json!({"executable": name, "size": size,
                "extraction_png_no_disk": extraction,
                "cold_own_persistence": own_persistence,
                "cold_same_persistence": same_persistence,
                "disk_backend_hit": disk, "memory_backend_hit": memory});
            for phase in [
                "extraction_png_no_disk",
                "cold_own_persistence",
                "cold_same_persistence",
                "disk_backend_hit",
                "memory_backend_hit",
            ] {
                eprintln!(
                    "{name} {size} {phase}: Tauri {:.2} us, GPUI {:.2} us, {} mismatched pairs",
                    case[phase]["tauri"]["median_us"].as_f64().unwrap(),
                    case[phase]["gpui"]["median_us"].as_f64().unwrap(),
                    case[phase]["mismatched_pairs_including_warmup"]
                );
            }
            cases.push(case);
        }
    }
    let report = json!({
        "tauri_revision": "54b8b4c2586908c29d0e60af0c4ec8d7c5c49dd4",
        "tauri_original_sha256": "3d3800a27412f351da198ba9f611f235272214580b87b737074add5d10861a37",
        "method": "Alternating paired order, four warmups, Windows Shell/OS caches warm, Oxyde caches cleared outside timing except memory phase. PNG decoded outside timing for pixel comparison.",
        "scope": "Same native APIs and compiler/dependencies, no Tauri/GPUI UI, no IPC/process startup. Extraction-only produces PNG in both versions; other phases preserve actual backend output formats.",
        "cases": cases,
    });
    let output = std::env::var_os("OXYDE_ICON_BENCH_REPORT")
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from("target/native-tests/tauri-icon-comparison.json"));
    std::fs::create_dir_all(output.parent().unwrap()).unwrap();
    std::fs::write(&output, serde_json::to_vec_pretty(&report).unwrap()).unwrap();
    eprintln!("Report: {}", output.display());
}
