//! A copy of more folders than the soft open file limit Oxyde received (E6).
use super::*;

const CHILD: &str = "OXYDE_OPEN_FILE_LIMIT_CHILD";
const RECEIVED: u64 = 256;
const FOLDERS: usize = 400;

#[test]
fn a_copy_of_more_folders_than_the_received_limit_completes() {
    use std::os::unix::process::CommandExt;
    let mut command = std::process::Command::new(std::env::current_exe().unwrap());
    command.args(["--exact", "systems::file_ops::tests::open_file_limit::child_copies_after_raising_the_limit", "--nocapture"])
        .env(CHILD, "1");
    // SAFETY: two raw system calls between fork and exec.
    unsafe {
        command.pre_exec(|| {
            use rustix::process::{getrlimit, setrlimit, Resource, Rlimit};
            let maximum = getrlimit(Resource::Nofile).maximum;
            Ok(setrlimit(Resource::Nofile, Rlimit { current: Some(RECEIVED), maximum })?)
        });
    }
    let output = command.output().unwrap();
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(output.status.success() && stdout.contains("1 passed"), "{stdout}\n{}", String::from_utf8_lossy(&output.stderr));
}

#[test]
fn child_copies_after_raising_the_limit() {
    if std::env::var_os(CHILD).is_none() {
        return;
    }
    let limit = rustix::process::getrlimit(rustix::process::Resource::Nofile);
    assert_eq!(limit.current, Some(RECEIVED));
    if limit.maximum.is_some_and(|maximum| maximum < 4 * FOLDERS as u64) {
        eprintln!("hard open file limit too low for this test");
        return;
    }
    platform().raise_open_file_limit();
    let root = tempfile::tempdir().unwrap();
    let source = root.path().join("tree");
    for index in 0..FOLDERS {
        let folder = source.join(format!("d{index}"));
        std::fs::create_dir_all(&folder).unwrap();
        std::fs::write(folder.join("file"), b"content").unwrap();
    }
    let target = root.path().join("target");
    std::fs::create_dir(&target).unwrap();
    let op = Arc::new(Mutex::new(FileOperation::new(FileOpType::Copy, vec![source], Some(target.clone()), None)));
    let runtime = tokio::runtime::Builder::new_multi_thread().enable_all().build().unwrap();
    runtime.block_on(FileOperationManager::execute_operation(Arc::new(HistoryManager::new()), op.clone()));
    let op = op.lock().unwrap();
    assert_eq!(op.status, OpStatus::Completed);
    assert_eq!(std::fs::read(target.join(format!("tree/d{}/file", FOLDERS - 1))).unwrap(), b"content");
}
