//! Compare the production ZIP broker with the same codec directly, on read-only sources.
//! Generated archives are verified then removed; the JSON report remains under target/.
#[cfg(target_os = "windows")]
fn main() -> Result<(), Box<dyn std::error::Error>> {
    use oxyde_lib::{
        platform::{platform, ThreadPriority},
        systems::file_ops::{FileOpType, FileOperation, FileOperationManager},
        utils::archive_io_protocol::with_worker_executable_for_tests,
    };
    use std::{
        collections::BTreeMap,
        fs::{self, File},
        io::{self, BufWriter, Read, Write},
        path::{Path, PathBuf},
        sync::{Arc, Mutex},
        time::Instant,
    };

    fn hash(mut reader: impl Read) -> io::Result<String> {
        let mut hash = blake3::Hasher::new();
        io::copy(&mut reader, &mut hash)?;
        Ok(hash.finalize().to_hex().to_string())
    }
    fn verify(
        path: &Path,
        expected: &BTreeMap<String, Option<String>>,
    ) -> Result<(), Box<dyn std::error::Error>> {
        if path.extension().and_then(|s| s.to_str()) == Some("7z") {
            let mut archive =
                sevenz_rust2::ArchiveReader::open(path, sevenz_rust2::Password::empty())?;
            let mut actual = BTreeMap::new();
            archive.for_each_entries(|entry, reader| {
                let name = entry.name().trim_end_matches('/').to_owned();
                let value =
                    if entry.is_directory() {
                        None
                    } else {
                        Some(hash(reader).map_err(|e| {
                            sevenz_rust2::Error::Io(e, "benchmark verification".into())
                        })?)
                    };
                if actual.insert(name, value).is_some() {
                    return Err(sevenz_rust2::Error::Io(
                        io::Error::other("Duplicate member"),
                        "benchmark verification".into(),
                    ));
                }
                Ok(true)
            })?;
            if &actual != expected {
                return Err("7z contents differ".into());
            }
            return Ok(());
        }
        if matches!(
            path.extension().and_then(|s| s.to_str()),
            Some("tar" | "zst")
        ) {
            let input: Box<dyn Read> = if path.extension().and_then(|s| s.to_str()) == Some("zst") {
                Box::new(zstd::stream::read::Decoder::new(File::open(path)?)?)
            } else {
                Box::new(File::open(path)?)
            };
            let mut archive = tar::Archive::new(input);
            let mut actual = BTreeMap::new();
            for entry in archive.entries()? {
                let mut entry = entry?;
                let name = entry
                    .path()?
                    .to_str()
                    .ok_or("Non-Unicode archive path")?
                    .replace('\\', "/")
                    .trim_end_matches('/')
                    .to_owned();
                let value = if entry.header().entry_type().is_dir() {
                    None
                } else {
                    Some(hash(&mut entry)?)
                };
                if actual.insert(name, value).is_some() {
                    return Err("Duplicate member".into());
                }
            }
            if &actual != expected {
                return Err("TAR contents differ".into());
            }
            return Ok(());
        }
        let mut archive = zip::ZipArchive::new(File::open(path)?)?;
        let mut actual = BTreeMap::new();
        for index in 0..archive.len() {
            let mut entry = archive.by_index(index)?;
            let name = entry.name()?.trim_end_matches('/').to_owned();
            let value = if entry.is_dir() {
                None
            } else {
                Some(hash(&mut entry)?)
            };
            if actual.insert(name, value).is_some() {
                return Err("Duplicate ZIP member".into());
            }
        }
        if &actual != expected {
            return Err("ZIP contents do not match the source manifest".into());
        }
        Ok(())
    }
    fn direct(
        output: &Path,
        entries: &[(PathBuf, String, bool)],
        level: i64,
    ) -> Result<(), Box<dyn std::error::Error>> {
        let mut writer = zip::ZipWriter::new(BufWriter::new(File::create_new(output)?));
        let mut buffer = [0; 64 * 1024];
        for (path, name, directory) in entries {
            let options = zip::write::SimpleFileOptions::default()
                .compression_method(zip::CompressionMethod::Deflated)
                .compression_level(Some(level));
            if *directory {
                writer.add_directory(format!("{name}/"), options)?;
            } else {
                writer.start_file(name, options)?;
                let mut source = File::open(path)?;
                loop {
                    let count = source.read(&mut buffer)?;
                    if count == 0 {
                        break;
                    }
                    writer.write_all(&buffer[..count])?;
                }
            }
        }
        let mut output = writer.finish()?;
        output.flush()?;
        output.get_ref().sync_all()?;
        Ok(())
    }

    env_logger::Builder::from_env(
        env_logger::Env::default().default_filter_or("warn,oxyde_lib::archive_timing=info"),
    )
    .format_timestamp_millis()
    .init();
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    let output_root = root
        .join("target")
        .join(format!("archive-compression-{}", uuid::Uuid::new_v4()));
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
    let preparation = Instant::now();
    let mut entries = Vec::new();
    let mut expected = BTreeMap::new();
    let mut total_bytes = 0u64;
    for entry in walkdir::WalkDir::new(&source)
        .follow_links(false)
        .sort_by_file_name()
    {
        let entry = entry?;
        let metadata = fs::symlink_metadata(entry.path())?;
        if metadata.file_type().is_symlink() || (!metadata.is_dir() && !metadata.is_file()) {
            return Err("Only ordinary files and directories are accepted".into());
        }
        if platform().file_attributes(&metadata, "").2 {
            return Err("Reparse points are not accepted".into());
        }
        let name = entry
            .path()
            .strip_prefix(parent)?
            .to_str()
            .ok_or("Non-Unicode path")?
            .replace('\\', "/");
        let digest = if metadata.is_dir() {
            None
        } else {
            total_bytes += metadata.len();
            Some(hash(File::open(entry.path())?)?)
        };
        expected.insert(name.clone(), digest);
        entries.push((entry.path().to_owned(), name, metadata.is_dir()));
    }
    eprintln!(
        "MANIFEST entries={} bytes={} hash_and_walk_s={:.6} output={}",
        entries.len(),
        total_bytes,
        preparation.elapsed().as_secs_f64(),
        output_root.display()
    );
    let worker = root.join("target/debug/oxyde-archive-io-worker.exe");
    if !worker.is_file() {
        return Err("Build all debug binaries before this benchmark".into());
    }
    let mut results = Vec::new();
    let format = std::env::args().nth(2).unwrap_or_else(|| "zip".into());
    if !matches!(format.as_str(), "zip" | "7z" | "tar" | "tar.zst") {
        return Err("Unsupported benchmark format".into());
    }
    let selected_mode = std::env::args().nth(3);
    for (mode, level) in [
        ("production-fast", 1),
        ("production-fast-turbo", 1),
        ("production-normal-turbo", 6),
        ("direct-fast", 1),
        ("direct-normal", 6),
    ] {
        if selected_mode.as_deref().is_some_and(|selected| {
            if selected == "turbo" {
                !mode.ends_with("-turbo")
            } else {
                selected != mode
            }
        }) {
            continue;
        }
        if format != "zip" && mode.starts_with("direct-") {
            continue;
        }
        platform().set_thread_priority(ThreadPriority::Normal);
        let output = output_root.join(format!("{mode}.{format}"));
        eprintln!("BEGIN {mode}");
        let started = Instant::now();
        if mode.starts_with("production-") {
            let mut operation = FileOperation::new(
                FileOpType::Compress,
                vec![source.clone()],
                Some(output.clone()),
                None,
            );
            operation.compression_quality = if level == 1 { "fast" } else { "normal" }.into();
            operation.turbo_flag.store(
                mode.ends_with("-turbo"),
                std::sync::atomic::Ordering::Relaxed,
            );
            let operation = Arc::new(Mutex::new(operation));
            with_worker_executable_for_tests(&worker, || {
                FileOperationManager::perform_compress(
                    &operation,
                    vec![source.clone()],
                    Some(output.clone()),
                )
            })
            .map_err(io::Error::other)?;
        } else {
            direct(&output, &entries, level)?;
        }
        let seconds = started.elapsed().as_secs_f64();
        platform().set_thread_priority(ThreadPriority::Normal);
        let bytes = fs::metadata(&output)?.len();
        // Independent decompression and BLAKE3 checks are outside the timed interval.
        verify(&output, &expected)?;
        let actual_level = match format.as_str() {
            "tar" => 0,
            "tar.zst" if level == 6 => 3,
            _ => level,
        };
        let result = serde_json::json!({"mode": mode, "quality": if level == 1 { "fast" } else { "normal" }, "level": actual_level, "seconds": seconds, "archive_bytes": bytes, "contents_verified": true});
        eprintln!("RESULT {result}");
        results.push(result);
        fs::remove_file(&output)?;
    }
    if results.is_empty() {
        return Err("No benchmark mode selected".into());
    }
    let report = serde_json::json!({
        "format": format, "source": source, "entries": entries.len(), "source_bytes": total_bytes,
        "results": results,
        "limits": "One warm-cache pass, production first; direct modes omit sandbox, broker and source guards. Source manifest hashing and output verification are excluded. No Explorer timing is measured."
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
