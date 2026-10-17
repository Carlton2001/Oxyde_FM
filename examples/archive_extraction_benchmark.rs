//! Compare production extraction without and with Turbo, on archives built from a read-only source.
//! Extracted trees are verified then removed; the JSON report remains under target/.
#[cfg(target_os = "windows")]
fn main() -> Result<(), Box<dyn std::error::Error>> {
    use oxyde_lib::{
        platform::{apply_thread_priority, platform, ThreadPriority},
        systems::file_ops::{FileOpType, FileOperation, FileOperationManager},
        utils::archive_io_protocol::with_worker_executable_for_tests,
    };
    use std::{
        collections::BTreeMap,
        fs::{self, File},
        io::{self, Read},
        path::{Path, PathBuf},
        sync::{atomic::Ordering, Arc, Mutex},
        time::Instant,
    };

    fn hash(mut reader: impl Read) -> io::Result<String> {
        let mut hash = blake3::Hasher::new();
        io::copy(&mut reader, &mut hash)?;
        Ok(hash.finalize().to_hex().to_string())
    }
    fn manifest(root: &Path, parent: &Path) -> Result<BTreeMap<String, Option<String>>, Box<dyn std::error::Error>> {
        let mut entries = BTreeMap::new();
        for entry in walkdir::WalkDir::new(root).follow_links(false).sort_by_file_name() {
            let entry = entry?;
            let metadata = fs::symlink_metadata(entry.path())?;
            if metadata.file_type().is_symlink() || (!metadata.is_dir() && !metadata.is_file()) {
                return Err("Only ordinary files and directories are accepted".into());
            }
            if platform().file_attributes(&metadata, "").2 {
                return Err("Reparse points are not accepted".into());
            }
            let name = entry.path().strip_prefix(parent)?.to_str().ok_or("Non-Unicode path")?.replace('\\', "/");
            let digest = if metadata.is_dir() { None } else { Some(hash(File::open(entry.path())?)?) };
            entries.insert(name, digest);
        }
        Ok(entries)
    }
    fn operation(kind: FileOpType, source: &Path, destination: &Path, turbo: bool) -> Arc<Mutex<FileOperation>> {
        let mut operation = FileOperation::new(kind, vec![source.to_owned()], Some(destination.to_owned()), None);
        operation.compression_quality = "fast".into();
        operation.turbo_flag.store(turbo, Ordering::Relaxed);
        Arc::new(Mutex::new(operation))
    }

    env_logger::Builder::from_env(
        env_logger::Env::default().default_filter_or("warn,oxyde_lib::archive_timing=info"),
    )
    .format_timestamp_millis()
    .init();
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    let output_root = root.join("target").join(format!("archive-extraction-{}", uuid::Uuid::new_v4()));
    fs::create_dir(&output_root)?;
    let source = if let Some(path) = std::env::args_os().nth(1) {
        fs::canonicalize(path)?
    } else {
        let source = output_root.join("fixture");
        fs::create_dir(&source)?;
        fs::write(source.join("repeated.bin"), vec![b'A'; 8 * 1024 * 1024])?;
        let mut state = 0x1234_5678u32;
        let bytes: Vec<_> = (0..8 * 1024 * 1024)
            .map(|_| {
                state ^= state << 13;
                state ^= state >> 17;
                state ^= state << 5;
                state as u8
            })
            .collect();
        fs::write(source.join("random.bin"), bytes)?;
        for index in 0..64 {
            fs::write(source.join(format!("small-{index}.txt")), vec![b'x'; 4096])?;
        }
        source
    };
    if !source.is_dir() || output_root.starts_with(&source) {
        return Err("Source must be a directory outside the output directory".into());
    }
    let parent = source.parent().ok_or("Missing source parent")?;
    let expected = manifest(&source, parent)?;
    let source_bytes: u64 = walkdir::WalkDir::new(&source).into_iter().filter_map(Result::ok)
        .filter(|entry| entry.file_type().is_file()).map(|entry| entry.metadata().map_or(0, |m| m.len())).sum();
    eprintln!("MANIFEST entries={} bytes={} output={}", expected.len(), source_bytes, output_root.display());
    let worker = root.join("target/debug/oxyde-archive-io-worker.exe");
    if !worker.is_file() {
        return Err("Build all debug binaries before this benchmark".into());
    }
    let formats: Vec<String> = match std::env::args().nth(2) {
        Some(format) => vec![format],
        None => ["zip", "7z", "tar", "tar.zst"].map(String::from).to_vec(),
    };
    let mut results = Vec::new();
    for format in formats {
        if !matches!(format.as_str(), "zip" | "7z" | "tar" | "tar.zst") {
            return Err("Unsupported benchmark format".into());
        }
        // The archive is built by the production encoder with Turbo, outside the timed interval.
        let archive = output_root.join(format!("source.{format}"));
        let build = operation(FileOpType::Compress, &source, &archive, true);
        with_worker_executable_for_tests(&worker, || {
            FileOperationManager::perform_compress(&build, vec![source.clone()], Some(archive.clone()))
        })
        .map_err(io::Error::other)?;
        let archive_bytes = fs::metadata(&archive)?.len();
        for turbo in [false, true, false, true] {
            let requested = output_root.join(format!("extract-{}", if turbo { "turbo" } else { "normal" }));
            let extract = operation(FileOpType::Extract, &archive, &requested, turbo);
            apply_thread_priority(ThreadPriority::Normal);
            eprintln!("BEGIN {format} turbo={turbo}");
            let started = Instant::now();
            with_worker_executable_for_tests(&worker, || {
                FileOperationManager::perform_extract(&extract, vec![archive.clone()], Some(requested.clone()))
            })
            .map_err(io::Error::other)?;
            let seconds = started.elapsed().as_secs_f64();
            apply_thread_priority(ThreadPriority::Normal);
            let target = extract.lock().unwrap().destination.clone().ok_or("Missing extraction target")?;
            // Verification is outside the timed interval.
            if manifest(&target.join(source.file_name().ok_or("Missing source name")?), &target)? != expected {
                return Err(format!("{format} extraction differs from the source manifest").into());
            }
            fs::remove_dir_all(&target)?;
            let result = serde_json::json!({"format": format, "turbo": turbo, "seconds": seconds, "archive_bytes": archive_bytes, "contents_verified": true});
            eprintln!("RESULT {result}");
            results.push(result);
        }
        fs::remove_file(&archive)?;
    }
    let report = serde_json::json!({
        "source": source, "entries": expected.len(), "source_bytes": source_bytes, "results": results,
        "limits": "Warm cache, alternating Normal/Turbo twice per format; archives built by the production Turbo encoder at fast quality. Verification excluded from timing."
    });
    let report_path = output_root.join("report.json");
    fs::write(&report_path, serde_json::to_vec_pretty(&report)?)?;
    println!("{}", report_path.display());
    Ok(())
}

#[cfg(not(target_os = "windows"))]
fn main() {
    eprintln!("This comparison requires the production Windows archive worker.");
    std::process::exit(1);
}
