//! Run with --help. CSV goes to stdout; diagnostics go to stderr.
use oxyde_lib::systems::copy_benchmark::{copy_one, digest, Mode};
use std::io;
use std::path::PathBuf;
use std::time::Instant;

fn main() {
    if let Err(error) = run() {
        eprintln!("Benchmark failed: {error}");
        std::process::exit(1);
    }
}

fn run() -> io::Result<()> {
    let args: Vec<_> = std::env::args_os().skip(1).collect();
    if args.iter().any(|arg| arg == "--help") {
        eprintln!(
            "copy_benchmark SOURCE DESTINATION_DIRECTORY [REPETITIONS=3] [--validate]\n\
            Runs all five modes sequentially, rotating order each repetition.\n\
            Outputs CSV. Creates and removes private temporary copies in the destination.\n\
            --validate rereads source and outputs outside timings (affects cache).\n\
            Native flush time is included in transfer_s; stream hashing is inline.\n\
            Streaming copies ordinary file data only, not native metadata/alternate streams."
        );
        return Ok(());
    }
    if args.len() < 2 || args.len() > 4 {
        return Err(io::Error::other(
            "Usage: copy_benchmark SOURCE DESTINATION_DIRECTORY [REPETITIONS] [--validate]",
        ));
    }
    let source = std::fs::canonicalize(PathBuf::from(&args[0]))?;
    let destination = std::fs::canonicalize(PathBuf::from(&args[1]))?;
    let validate = args[2..].iter().any(|arg| arg == "--validate");
    let counts: Vec<_> = args[2..]
        .iter()
        .filter(|arg| *arg != "--validate")
        .collect();
    let repetitions = match counts.as_slice() {
        [] => 3,
        [count] => count
            .to_str()
            .and_then(|value| value.parse::<usize>().ok())
            .filter(|count| (1..=100).contains(count))
            .ok_or_else(|| io::Error::other("Repetitions must be between 1 and 100"))?,
        _ => return Err(io::Error::other("Too many arguments")),
    };
    if !destination.is_dir() || destination.starts_with(&source) {
        return Err(io::Error::other(
            "Destination must be an existing directory outside the source",
        ));
    }
    if !cfg!(target_os = "windows") {
        return Err(io::Error::other("This comparison requires Windows"));
    }
    let mut files = Vec::new();
    let mut directories = Vec::new();
    for entry in walkdir::WalkDir::new(&source)
        .follow_links(false)
        .sort_by_file_name()
    {
        let entry = entry?;
        let metadata = std::fs::symlink_metadata(entry.path())?;
        if metadata.file_type().is_symlink()
            || oxyde_lib::platform::platform()
                .file_attributes(&metadata, "")
                .2
        {
            return Err(io::Error::other(
                "Symlinks and reparse points are excluded from this benchmark",
            ));
        }
        if metadata.is_file() {
            let relative = if source.is_file() {
                PathBuf::from("file")
            } else {
                entry.path().strip_prefix(&source).unwrap().to_path_buf()
            };
            let expected = if validate {
                Some(digest(entry.path())?)
            } else {
                None
            };
            files.push((entry.path().to_path_buf(), relative, expected));
        } else if metadata.is_dir() {
            directories.push(entry.path().strip_prefix(&source).unwrap().to_path_buf());
        } else {
            return Err(io::Error::other(
                "Only ordinary files and directories are supported",
            ));
        }
    }
    eprintln!("{} files; {} repetitions; validation={validate}. Keep the source unchanged. OS cache is not cleared.", files.len(), repetitions);
    println!(
        "repetition,mode,files,bytes,total_s,transfer_s,stream_flush_s,post_hash_s,publish_s,mib_s,source_hash_s,hash_wait_s"
    );
    for repetition in 0..repetitions {
        for index in 0..Mode::ALL.len() {
            let mode = Mode::ALL[(index + repetition) % Mode::ALL.len()];
            eprintln!("Pass {}: {}", repetition + 1, mode.name());
            let temporary = tempfile::Builder::new()
                .prefix("oxyde-copy-benchmark-")
                .tempdir_in(&destination)?;
            let started = Instant::now();
            for directory in &directories {
                std::fs::create_dir_all(temporary.path().join(directory))?;
            }
            let mut totals = [0.0_f64; 6];
            let mut bytes = 0_u64;
            let mut digests = Vec::new();
            for (input, relative, _) in &files {
                let measurement = copy_one(input, &temporary.path().join(relative), mode)?;
                bytes += measurement.bytes;
                totals[0] += measurement.transfer.as_secs_f64();
                totals[1] += measurement.flush.as_secs_f64();
                totals[2] += measurement.post_hash.as_secs_f64();
                totals[3] += measurement.publish.as_secs_f64();
                totals[4] += measurement.source_hash.as_secs_f64();
                totals[5] += measurement.hash_wait.as_secs_f64();
                if validate {
                    digests.push(measurement.digest);
                }
            }
            let elapsed = started.elapsed().as_secs_f64();
            if validate {
                for ((input, relative, expected), computed) in files.iter().zip(digests) {
                    let expected = expected.as_ref().unwrap();
                    if digest(input)? != *expected
                        || digest(&temporary.path().join(relative))? != *expected
                        || computed.as_ref().is_some_and(|value| value != expected)
                    {
                        return Err(io::Error::other(format!(
                            "Validation failed: {}",
                            input.display()
                        )));
                    }
                }
            }
            // Cleanup is outside timings; TempDir only owns this pass's generated directory.
            temporary.close()?;
            println!(
                "{},{},{},{},{:.6},{:.6},{:.6},{:.6},{:.6},{:.2},{:.6},{:.6}",
                repetition + 1,
                mode.name(),
                files.len(),
                bytes,
                elapsed,
                totals[0],
                totals[1],
                totals[2],
                totals[3],
                bytes as f64 / (1024.0 * 1024.0) / elapsed.max(f64::EPSILON),
                totals[4],
                totals[5]
            );
        }
    }
    Ok(())
}
