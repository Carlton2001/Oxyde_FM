//! Root-item receipts for Delete history; Shell lookup is still path based.
use super::ShellDeleteOutcome;
use crate::models::transaction::HistoryFileIdentity;
use crate::systems::path_parents::HeldParents;
use std::{
    path::{Path, PathBuf},
    sync::{Arc, Mutex},
};
use windows::core::{implement, Error, Ref, HRESULT, PCWSTR};
use windows::Win32::Foundation::{E_ABORT, S_OK};
use windows::Win32::System::Com::{CoCreateInstance, CLSCTX_INPROC_SERVER};
use windows::Win32::System::Ole::{OleInitialize, OleUninitialize};
use windows::Win32::UI::Shell::*;
mod batch;
#[path = "windows_shell_delete_tracked/effects.rs"]
mod effects;
pub(super) use batch::run_files;

struct Apartment;
impl Apartment {
    fn new() -> windows::core::Result<Self> {
        unsafe {
            OleInitialize(None)?;
        }
        Ok(Self)
    }
}

#[cfg(test)]
#[path = "windows_shell_delete_tracked/tests/partial_effects.rs"]
mod partial_effect_tests;
impl Drop for Apartment {
    fn drop(&mut self) {
        unsafe {
            OleUninitialize();
        }
    }
}

#[derive(Default)]
struct Receipt {
    preflight: bool,
    completed: bool,
    error: Option<String>,
    effects: effects::Effects,
    payload: Option<PathBuf>,
    proof_time: std::time::Duration,
}
impl Receipt {
    fn fail(&mut self, error: String) {
        if self.error.is_none() {
            self.error = Some(error);
        }
    }
    fn post(&mut self, result: HRESULT) {
        // A completed native recycle/remove can report that the Shell should
        // not traverse children (also observed for files), rather than S_OK.
        // Do not accept all nonnegative HRESULTs: skips/pending are not receipts.
        if self.preflight && matches!(result, S_OK | COPYENGINE_S_DONT_PROCESS_CHILDREN) {
            self.completed = true;
        } else {
            self.fail(format!(
                "Windows did not confirm deletion of the selected item (0x{:08X})",
                result.0 as u32
            ));
        }
    }
    fn outcome(&mut self, result: windows::core::Result<()>, aborted: bool) -> ShellDeleteOutcome {
        if let Err(error) = result {
            self.fail(error.to_string());
        }
        if aborted {
            self.fail("Deletion was aborted".into());
        }
        if !self.completed && self.error.is_none() {
            self.fail("Windows did not provide a completion receipt for the selected item".into());
        }
        ShellDeleteOutcome {
            completed: self.completed,
            error: self.error.take(),
            effects: self.effects.take(self.completed, self.payload.as_deref()),
        }
    }
}

fn check_source(path: &Path, proof: Option<HistoryFileIdentity>) -> Result<(), String> {
    let Some(proof) = proof else {
        return Ok(());
    };
    let check = || -> std::io::Result<bool> {
        let parents = HeldParents::new(path)?;
        let file = super::platform()
            .track_source_item(parents.directory(), &parents.resolved_path(path)?)?;
        proof.matches(&file)
    };
    if check().unwrap_or(false) {
        Ok(())
    } else {
        Err(crate::models::error::SourceSelectionFailure {
            changed: true,
            path: path.to_string_lossy().into_owned(),
        }
        .to_string())
    }
}

#[implement(IFileOperationProgressSink)]
struct DeleteSink {
    root: IShellItem,
    path: PathBuf,
    proof: Option<HistoryFileIdentity>,
    receipt: Arc<Mutex<Receipt>>,
    control: Option<super::ShellDeleteControl>,
}
impl DeleteSink_Impl {
    fn checkpoint(&self) -> windows::core::Result<()> {
        if let Some(control) = &self.control {
            loop {
                if control.cancel.load(std::sync::atomic::Ordering::Relaxed) {
                    return Err(self.record_failure("Operation cancelled".into()));
                }
                if !control.pause.load(std::sync::atomic::Ordering::Relaxed) {
                    break;
                }
                std::thread::sleep(std::time::Duration::from_millis(50));
            }
        }
        Ok(())
    }
    fn root_item(&self, item: &Ref<IShellItem>) -> windows::core::Result<bool> {
        let item = item.ok()?;
        Ok(unsafe { item.Compare(&self.root, SICHINT_CANONICAL.0 as u32)? } == 0)
    }
    fn record_failure(&self, error: String) -> Error {
        self.receipt
            .lock()
            .unwrap_or_else(|p| p.into_inner())
            .fail(error);
        Error::from_hresult(E_ABORT)
    }
}
#[allow(non_snake_case)]
impl IFileOperationProgressSink_Impl for DeleteSink_Impl {
    fn StartOperations(&self) -> windows::core::Result<()> {
        self.checkpoint()
    }
    fn FinishOperations(&self, result: HRESULT) -> windows::core::Result<()> {
        if result.is_err() {
            self.receipt
                .lock()
                .unwrap_or_else(|p| p.into_inner())
                .fail(Error::from_hresult(result).to_string());
        }
        Ok(())
    }
    fn PreDeleteItem(&self, _: u32, item: Ref<IShellItem>) -> windows::core::Result<()> {
        self.checkpoint()?;
        if self
            .root_item(&item)
            .map_err(|e| self.record_failure(e.to_string()))?
        {
            let started = std::time::Instant::now();
            let checked = check_source(&self.path, self.proof);
            self.receipt.lock().unwrap_or_else(|p| p.into_inner()).proof_time += started.elapsed();
            checked.map_err(|e| self.record_failure(e))?;
            self.receipt
                .lock()
                .unwrap_or_else(|p| p.into_inner())
                .preflight = true;
        } else {
            let result = self
                .receipt
                .lock()
                .unwrap_or_else(|p| p.into_inner())
                .effects
                .pre(&self.path, item);
            result.map_err(|e| self.record_failure(e))?;
        }
        Ok(())
    }
    fn PostDeleteItem(
        &self,
        _: u32,
        item: Ref<IShellItem>,
        result: HRESULT,
        payload: Ref<IShellItem>,
    ) -> windows::core::Result<()> {
        if self
            .root_item(&item)
            .map_err(|e| self.record_failure(e.to_string()))?
        {
            // Preserve the native completion before querying optional metadata.
            self.receipt
                .lock()
                .unwrap_or_else(|p| p.into_inner())
                .post(result);
            let destination = if payload.ok().is_ok() {
                Some(effects::path(payload).map_err(|e| self.record_failure(e))?)
            } else {
                None
            };
            let mut receipt = self.receipt.lock().unwrap_or_else(|p| p.into_inner());
            receipt.payload = destination;
        } else {
            let recorded = self
                .receipt
                .lock()
                .unwrap_or_else(|p| p.into_inner())
                .effects
                .post(item, result, payload);
            recorded.map_err(|e| self.record_failure(e))?;
        }
        Ok(())
    }
    fn PreRenameItem(&self, _: u32, _: Ref<IShellItem>, _: &PCWSTR) -> windows::core::Result<()> {
        Ok(())
    }
    fn PostRenameItem(
        &self,
        _: u32,
        _: Ref<IShellItem>,
        _: &PCWSTR,
        _: HRESULT,
        _: Ref<IShellItem>,
    ) -> windows::core::Result<()> {
        Ok(())
    }
    fn PreMoveItem(
        &self,
        _: u32,
        _: Ref<IShellItem>,
        _: Ref<IShellItem>,
        _: &PCWSTR,
    ) -> windows::core::Result<()> {
        Ok(())
    }
    fn PostMoveItem(
        &self,
        _: u32,
        _: Ref<IShellItem>,
        _: Ref<IShellItem>,
        _: &PCWSTR,
        _: HRESULT,
        _: Ref<IShellItem>,
    ) -> windows::core::Result<()> {
        Ok(())
    }
    fn PreCopyItem(
        &self,
        _: u32,
        _: Ref<IShellItem>,
        _: Ref<IShellItem>,
        _: &PCWSTR,
    ) -> windows::core::Result<()> {
        Ok(())
    }
    fn PostCopyItem(
        &self,
        _: u32,
        _: Ref<IShellItem>,
        _: Ref<IShellItem>,
        _: &PCWSTR,
        _: HRESULT,
        _: Ref<IShellItem>,
    ) -> windows::core::Result<()> {
        Ok(())
    }
    fn PreNewItem(&self, _: u32, _: Ref<IShellItem>, _: &PCWSTR) -> windows::core::Result<()> {
        Ok(())
    }
    fn PostNewItem(
        &self,
        _: u32,
        _: Ref<IShellItem>,
        _: &PCWSTR,
        _: &PCWSTR,
        _: u32,
        _: HRESULT,
        _: Ref<IShellItem>,
    ) -> windows::core::Result<()> {
        Ok(())
    }
    fn UpdateProgress(&self, _: u32, _: u32) -> windows::core::Result<()> {
        self.checkpoint()
    }
    fn ResetTimer(&self) -> windows::core::Result<()> {
        Ok(())
    }
    fn PauseTimer(&self) -> windows::core::Result<()> {
        Ok(())
    }
    fn ResumeTimer(&self) -> windows::core::Result<()> {
        Ok(())
    }
}

fn flags(recycle: bool) -> FILEOPERATION_FLAGS {
    FILEOPERATION_FLAGS(u32::from(super::windows_shell_delete::operation_flags(
        recycle,
    ))) | FOFX_EARLYFAILURE
}

fn delete(
    path: &Path,
    recycle: bool,
    proof: Option<HistoryFileIdentity>,
    control: Option<super::ShellDeleteControl>,
) -> ShellDeleteOutcome {
    // Validate literal names before initializing any native operation.
    let buffer = match super::windows_shell_delete::source_buffer(&[path.to_path_buf()]) {
        Ok(buffer) => buffer,
        Err(error) => {
            return ShellDeleteOutcome {
                completed: false,
                error: Some(error),
                effects: Vec::new(),
            }
        }
    };
    let receipt = Arc::new(Mutex::new(Receipt::default()));
    let result = (|| -> windows::core::Result<bool> {
        let _apartment = Apartment::new()?;
        unsafe {
            let root: IShellItem = SHCreateItemFromParsingName(PCWSTR(buffer.as_ptr()), None)?;
            let operation: IFileOperation =
                CoCreateInstance(&FileOperation, None, CLSCTX_INPROC_SERVER)?;
            operation.SetOperationFlags(flags(recycle))?;
            let sink: IFileOperationProgressSink = DeleteSink {
                root: root.clone(),
                path: path.to_path_buf(),
                proof,
                receipt: receipt.clone(),
                control,
            }
            .into();
            // Observe the whole operation, including child deletions. Do not
            // also register the item sink: that duplicates root notifications.
            let cookie = operation.Advise(&sink)?;
            operation.DeleteItem(&root, None)?;
            // Completion receipts remain available even when PerformOperations fails.
            let performed = operation.PerformOperations();
            let aborted = operation.GetAnyOperationsAborted();
            let _ = operation.Unadvise(cookie);
            performed?;
            Ok(aborted?.as_bool())
        }
    })();
    let aborted = result.as_ref().copied().unwrap_or(false);
    let outcome = receipt
        .lock()
        .unwrap_or_else(|p| p.into_inner())
        .outcome(result.map(|_| ()), aborted);
    outcome
}

pub(super) fn run(
    path: &Path,
    recycle: bool,
    proof: Option<HistoryFileIdentity>,
) -> ShellDeleteOutcome {
    run_controlled(path, recycle, proof, None)
}

pub(super) fn run_controlled(
    path: &Path,
    recycle: bool,
    proof: Option<HistoryFileIdentity>,
    control: Option<super::ShellDeleteControl>,
) -> ShellDeleteOutcome {
    // A fresh STA avoids borrowing the Tokio worker's existing COM apartment.
    let path = path.to_path_buf();
    match std::thread::Builder::new()
        .name("oxyde-shell-delete".into())
        .spawn(move || delete(&path, recycle, proof, control))
    {
        Ok(thread) => thread.join().unwrap_or_else(|_| ShellDeleteOutcome {
            completed: false,
            error: Some("Windows deletion worker failed".into()),
            effects: Vec::new(),
        }),
        Err(error) => ShellDeleteOutcome {
            completed: false,
            error: Some(error.to_string()),
            effects: Vec::new(),
        },
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::{fs, os::windows::ffi::OsStrExt};
    use windows::Win32::Foundation::E_FAIL;

    fn item(path: &Path) -> IShellItem {
        let wide: Vec<_> = path.as_os_str().encode_wide().chain([0]).collect();
        unsafe { SHCreateItemFromParsingName(PCWSTR(wide.as_ptr()), None).unwrap() }
    }

    #[test]
    fn completed_root_survives_later_error_and_abort_but_skips_never_become_success() {
        for status in [S_OK, COPYENGINE_S_DONT_PROCESS_CHILDREN] {
            let mut receipt = Receipt {
                preflight: true,
                ..Default::default()
            };
            receipt.post(status);
            let outcome = receipt.outcome(Ok(()), false);
            assert!(outcome.completed);
            assert!(outcome.error.is_none());
            // A later error must still retain the completed root's receipt.
            let outcome = receipt.outcome(Err(Error::from_hresult(E_FAIL)), true);
            assert!(outcome.completed);
            assert!(outcome.error.is_some());
            let mut unchecked = Receipt::default();
            unchecked.post(status);
            assert!(!unchecked.outcome(Ok(()), false).completed);
        }
        for status in [
            E_FAIL,
            HRESULT(1),
            COPYENGINE_S_USER_IGNORED,
            COPYENGINE_S_PENDING_DELETE,
            COPYENGINE_S_PENDING,
            COPYENGINE_S_USER_RETRY,
            COPYENGINE_S_ALREADY_DONE,
            COPYENGINE_E_USER_CANCELLED,
            COPYENGINE_E_CANCELLED,
        ] {
            let mut receipt = Receipt {
                preflight: true,
                ..Default::default()
            };
            receipt.post(status);
            let outcome = receipt.outcome(Ok(()), false);
            assert!(!outcome.completed);
            assert!(outcome.error.is_some());
        }
        let mut receipt = Receipt::default();
        receipt.post(S_OK);
        assert!(!receipt.outcome(Ok(()), false).completed);
        assert!(Receipt::default().outcome(Ok(()), false).error.is_some());
    }

    #[test]
    fn native_sink_accepts_observed_recycle_status_for_unicode_files_and_directories() {
        std::thread::spawn(|| {
            let _apartment = Apartment::new().unwrap();
            for directory in [false, true] {
                let temp = tempfile::tempdir().unwrap();
                let path = temp.path().join(if directory {
                    "Dossier école"
                } else {
                    "Annuler école.txt"
                });
                if directory {
                    fs::create_dir(&path).unwrap();
                    fs::write(path.join("contenu.txt"), b"witness").unwrap();
                } else {
                    fs::write(&path, b"witness").unwrap();
                }
                let proof = {
                    let parents = HeldParents::new(&path).unwrap();
                    let file = super::super::platform()
                        .track_source_item(
                            parents.directory(),
                            &parents.resolved_path(&path).unwrap(),
                        )
                        .unwrap();
                    HistoryFileIdentity::from_file(&file).unwrap()
                };
                let root = item(&path);
                let child_or_sibling = if directory {
                    path.join("contenu.txt")
                } else {
                    let sibling = temp.path().join("other.txt");
                    fs::write(&sibling, b"other").unwrap();
                    sibling
                };
                let unrelated = item(&child_or_sibling);
                let receipt = Arc::new(Mutex::new(Receipt::default()));
                let sink: IFileOperationProgressSink = DeleteSink {
                    root: root.clone(),
                    path: path.clone(),
                    proof: Some(proof),
                    receipt: receipt.clone(),
                    control: None,
                }
                .into();
                unsafe {
                    sink.PreDeleteItem(0, &unrelated).unwrap();
                    sink.PostDeleteItem(0, &unrelated, COPYENGINE_S_DONT_PROCESS_CHILDREN, None)
                        .unwrap();
                    assert!(!receipt.lock().unwrap().completed);
                    sink.PreDeleteItem(0, &root).unwrap();
                    // Only a private rename: this test never touches the Recycle Bin.
                    fs::rename(&path, temp.path().join("saved")).unwrap();
                    sink.PostDeleteItem(0, &root, COPYENGINE_S_DONT_PROCESS_CHILDREN, None)
                        .unwrap();
                    sink.FinishOperations(S_OK).unwrap();
                }
                let outcome = receipt.lock().unwrap().outcome(Ok(()), false);
                assert!(outcome.completed);
                assert!(outcome.error.is_none());
                let saved = temp.path().join("saved");
                let content = if directory {
                    saved.join("contenu.txt")
                } else {
                    saved
                };
                assert_eq!(fs::read(content).unwrap(), b"witness");
            }
        })
        .join()
        .unwrap();
    }

    #[test]
    fn native_sink_ignores_other_items_and_keeps_root_receipt_after_rename_without_deletion() {
        std::thread::spawn(|| {
            let _apartment = Apartment::new().unwrap();
            let temp = tempfile::tempdir().unwrap();
            let path = temp.path().join("selected.txt");
            let other = temp.path().join("other.txt");
            fs::write(&path, "selected").unwrap();
            fs::write(&other, "witness").unwrap();
            let proof = HistoryFileIdentity::from_file(&fs::File::open(&path).unwrap()).unwrap();
            let root = item(&path);
            let sibling = item(&other);
            let callback_root = item(&path);
            let receipt = Arc::new(Mutex::new(Receipt::default()));
            let sink: IFileOperationProgressSink = DeleteSink {
                root: root.clone(),
                path: path.clone(),
                proof: Some(proof),
                receipt: receipt.clone(),
                control: None,
            }
            .into();
            unsafe {
                sink.PreDeleteItem(0, &sibling).unwrap();
                sink.PostDeleteItem(0, &sibling, S_OK, None).unwrap();
                assert!(!receipt.lock().unwrap().completed);
                sink.PreDeleteItem(0, &callback_root).unwrap();
                fs::rename(&path, path.with_extension("saved")).unwrap();
                sink.PostDeleteItem(0, &callback_root, S_OK, None).unwrap();
                sink.FinishOperations(E_FAIL).unwrap();
            }
            let outcome = receipt.lock().unwrap().outcome(Ok(()), false);
            assert!(outcome.completed);
            assert!(outcome.error.is_some());
            assert_eq!(fs::read(other).unwrap(), b"witness");
            assert_eq!(fs::read(path.with_extension("saved")).unwrap(), b"selected");
        })
        .join()
        .unwrap();
    }

    #[test]
    fn native_predelete_refuses_a_same_content_replacement_without_deleting_anything() {
        std::thread::spawn(|| {
            let _apartment = Apartment::new().unwrap();
            let temp = tempfile::tempdir().unwrap();
            let path = temp.path().join("selected.txt");
            fs::write(&path, "same").unwrap();
            let proof = HistoryFileIdentity::from_file(&fs::File::open(&path).unwrap()).unwrap();
            let root = item(&path);
            let receipt = Arc::new(Mutex::new(Receipt::default()));
            let sink: IFileOperationProgressSink = DeleteSink {
                root: root.clone(),
                path: path.clone(),
                proof: Some(proof),
                receipt: receipt.clone(),
                control: None,
            }
            .into();
            let saved = path.with_extension("saved");
            fs::rename(&path, &saved).unwrap();
            fs::write(&path, "same").unwrap();
            assert!(unsafe { sink.PreDeleteItem(0, &root) }.is_err());
            assert!(!receipt.lock().unwrap().preflight);
            assert!(!receipt.lock().unwrap().completed);
            assert_eq!(
                crate::models::error::SourceSelectionFailure::from_message(
                    receipt.lock().unwrap().error.as_deref().unwrap()
                ),
                Some(true)
            );
            assert_eq!(fs::read(path).unwrap(), b"same");
            assert_eq!(fs::read(saved).unwrap(), b"same");
        })
        .join()
        .unwrap();
    }

    #[test]
    fn invalid_literals_fail_before_native_dispatch_and_flags_stop_on_error() {
        let outcome = run(Path::new(r"C:\unselected*"), true, None);
        assert!(!outcome.completed);
        assert!(outcome.error.unwrap().contains("literal"));
        let flags = flags(true);
        assert_ne!((flags & FOFX_EARLYFAILURE).0, 0);
        assert_ne!((flags & FOF_NOERRORUI).0, 0);
        assert_ne!((flags & FOF_NO_CONNECTED_ELEMENTS).0, 0);
        assert_ne!((flags & FOF_ALLOWUNDO).0, 0);
        assert_eq!((super::flags(false) & FOF_ALLOWUNDO).0, 0);
    }
}
