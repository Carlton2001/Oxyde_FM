//! Time permanent deletion of one generated folder, without and with Turbo.
//! Usage: delete_tree_benchmark [PARENT_DIRECTORY] ; trees are created in a private subdirectory.
fn main() -> Result<(), Box<dyn std::error::Error>> {
    use oxyde_lib::systems::file_ops::{FileOpType, FileOperation, FileOperationManager};
    use std::{fs, path::PathBuf, sync::{atomic::Ordering, Arc, Mutex}, time::Instant};

    env_logger::Builder::from_env(env_logger::Env::default().default_filter_or("error")).init();
    let parent = match std::env::args_os().nth(1) {
        Some(path) => fs::canonicalize(path)?,
        None => PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("target"),
    };
    let base = parent.join(format!("delete-tree-{}", uuid::Uuid::new_v4()));
    fs::create_dir(&base)?;
    // Shapes: (name, directories, files per directory, bytes per file).
    let shapes = [("flat", 1usize, 20_000usize, 512usize), ("nested", 400, 50, 512), ("large", 4, 16, 16 << 20)];
    println!("shape,turbo,repetition,files,seconds");
    let result = (|| -> Result<(), Box<dyn std::error::Error>> {
        for (name, directories, files, size) in shapes {
            let payload = vec![0x5au8; size];
            for repetition in 0..2 {
                for turbo in [repetition == 1, repetition == 0] {
                    let root = base.join(format!("{name}-{turbo}-{repetition}"));
                    for directory in 0..directories {
                        let directory = root.join(format!("d{:03}", directory % 20)).join(format!("e{directory:04}"));
                        fs::create_dir_all(&directory)?;
                        for file in 0..files {
                            fs::write(directory.join(format!("{file:05}.bin")), &payload)?;
                        }
                    }
                    let operation = FileOperation::new(FileOpType::Delete, vec![root.clone()], None, None);
                    operation.turbo_flag.store(turbo, Ordering::Relaxed);
                    let operation = Arc::new(Mutex::new(operation));
                    let started = Instant::now();
                    FileOperationManager::perform_delete(&operation, vec![root.clone()])?;
                    let seconds = started.elapsed().as_secs_f64();
                    if root.exists() {
                        return Err(format!("{} was not removed", root.display()).into());
                    }
                    println!("{name},{turbo},{repetition},{},{seconds:.3}", directories * files);
                }
            }
        }
        Ok(())
    })();
    fs::remove_dir_all(&base)?;
    result
}
