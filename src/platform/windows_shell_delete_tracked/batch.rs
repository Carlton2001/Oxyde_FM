//! Bounded batches of ordinary files. Directories retain the descendant-aware path.
use super::*;
use crate::platform::ShellDeleteControl;
use std::sync::atomic::Ordering;

fn failure(message: &str) -> ShellDeleteOutcome {
    ShellDeleteOutcome {
        completed: false,
        error: Some(message.into()),
        effects: Vec::new(),
    }
}

pub(in crate::platform) fn run_files(
    files: Vec<(PathBuf, bool, HistoryFileIdentity)>,
    control: ShellDeleteControl,
) -> Vec<ShellDeleteOutcome> {
    let count = files.len();
    match std::thread::Builder::new()
        .name("oxyde-trash-batch".into())
        .spawn(move || execute(&files, control))
    {
        Ok(thread) => thread.join().unwrap_or_else(|_| {
            (0..count)
                .map(|_| failure("Windows deletion worker failed"))
                .collect()
        }),
        Err(error) => (0..count).map(|_| failure(&error.to_string())).collect(),
    }
}

fn execute(
    files: &[(PathBuf, bool, HistoryFileIdentity)],
    control: ShellDeleteControl,
) -> Vec<ShellDeleteOutcome> {
    let started = std::time::Instant::now();
    let mut setup = None;
    let mut native = std::time::Duration::ZERO;
    let receipts: Vec<_> = files
        .iter()
        .map(|_| Arc::new(Mutex::new(Receipt::default())))
        .collect();
    let result = (|| -> windows::core::Result<bool> {
        let _apartment = Apartment::new()?;
        super::super::platform().set_thread_priority(if control.turbo.load(Ordering::Relaxed) {
            super::super::ThreadPriority::Normal
        } else {
            super::super::ThreadPriority::Background
        });
        let operation: IFileOperation =
            unsafe { CoCreateInstance(&FileOperation, None, CLSCTX_INPROC_SERVER)? };
        // Caller splits batches by recycle policy, never mixing permanent and recycled entries.
        if let Some((_, recycle, _)) = files.first() {
            unsafe {
                operation.SetOperationFlags(flags(*recycle))?;
            }
        }
        let mut sinks = Vec::new();
        for ((path, recycle, proof), receipt) in files.iter().zip(&receipts) {
            if control.cancel.load(Ordering::Relaxed) {
                return Err(Error::from_hresult(E_ABORT));
            }
            if Some(*recycle) != files.first().map(|(_, recycle, _)| *recycle) {
                return Err(Error::from_hresult(E_ABORT));
            }
            let buffer = super::super::windows_shell_delete::source_buffer(&[path.clone()])
                .map_err(|_| Error::from_hresult(E_ABORT))?;
            let root: IShellItem =
                unsafe { SHCreateItemFromParsingName(PCWSTR(buffer.as_ptr()), None)? };
            let sink: IFileOperationProgressSink = DeleteSink {
                root: root.clone(),
                path: path.clone(),
                proof: Some(*proof),
                receipt: receipt.clone(),
                control: Some(control.clone()),
            }
            .into();
            // Only ordinary files are queued here; per-item sinks cannot lose
            // directory descendant receipts. No global Advise duplicates callbacks.
            unsafe {
                operation.DeleteItem(&root, &sink)?;
            }
            sinks.push(sink);
        }
        setup = Some(started.elapsed());
        let native_started = std::time::Instant::now();
        let performed = unsafe { operation.PerformOperations() };
        let aborted = unsafe { operation.GetAnyOperationsAborted() };
        native = native_started.elapsed();
        performed?;
        Ok(aborted?.as_bool())
    })();
    let aborted = result.as_ref().copied().unwrap_or(false);
    let proof: std::time::Duration = receipts
        .iter()
        .map(|receipt| receipt.lock().unwrap_or_else(|p| p.into_inner()).proof_time)
        .sum();
    let outcomes: Vec<_> = receipts
        .iter()
        .map(|receipt| {
            // Preserve every confirmed success even if a later item fails/cancels.
            let result = result
                .as_ref()
                .map(|_| ())
                .map_err(|error| Error::from_hresult(error.code()));
            receipt
                .lock()
                .unwrap_or_else(|p| p.into_inner())
                .outcome(result, aborted)
        })
        .collect();
    let wall = started.elapsed();
    let setup = setup.unwrap_or(wall);
    log::info!(target: "oxyde_lib::delete_timing",
        "trash_native_batch first={:?} files={} completed={} recycle={} failed={} cancelled={} wall_s={:.6} setup_s={:.6} native_s={:.6} proof_s={:.6} remaining_s={:.6}",
        files.first().map(|(path, _, _)| path), files.len(), outcomes.iter().filter(|outcome| outcome.completed).count(),
        files.first().map(|(_, recycle, _)| *recycle).unwrap_or(false), outcomes.iter().any(|outcome| !outcome.completed || outcome.error.is_some()),
        control.cancel.load(Ordering::Relaxed), wall.as_secs_f64(), setup.as_secs_f64(),
        native.as_secs_f64(), proof.as_secs_f64(), wall.saturating_sub(setup + native).as_secs_f64());
    outcomes
}

#[cfg(test)]
#[path = "batch/tests/operations.rs"]
mod tests;
