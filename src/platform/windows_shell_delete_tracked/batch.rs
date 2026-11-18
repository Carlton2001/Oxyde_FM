//! Bounded batches of ordinary files. Directories retain the descendant-aware path.
use super::*;
use crate::platform::ShellDeleteControl;
use std::sync::atomic::Ordering;

fn failure(message: &str) -> ShellDeleteOutcome {
    ShellDeleteOutcome {
        completed: false,
        error: Some(message.into()),
        effects: Vec::new(),
        destroyed: false,
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
    let mut timing = Timing::default();
    let receipts: Vec<_> = files
        .iter()
        .map(|(_, recycle, _)| Arc::new(Mutex::new(Receipt { recycle: *recycle, ..Default::default() })))
        .collect();
    let mut results: Vec<windows::core::Result<bool>> = files.iter().map(|_| Ok(false)).collect();
    match Apartment::new() {
        Err(error) => results.iter_mut().for_each(|result| *result = Err(error.clone())),
        Ok(_apartment) => {
            control.follow_turbo();
            let mut pending: Vec<usize> = (0..files.len()).collect();
            while !pending.is_empty() {
                let result = perform(files, &receipts, &pending, &control, &mut timing);
                for &index in &pending {
                    results[index] = result.clone();
                }
                // A refusal of the Shell sink cancels every later item of the
                // operation: retry those Windows never reached, in a new one.
                let lock = |index: usize| receipts[index].lock().unwrap_or_else(|p| p.into_inner());
                if control.cancel.load(Ordering::Relaxed) || !pending.iter().any(|&index| lock(index).refused) {
                    break;
                }
                pending.retain(|&index| {
                    let receipt = lock(index);
                    !receipt.refused && !receipt.preflight && !receipt.completed
                });
                for &index in &pending {
                    *lock(index) = Receipt { recycle: files[index].1, ..Default::default() };
                }
            }
        }
    }
    let proof: std::time::Duration = receipts
        .iter()
        .map(|receipt| receipt.lock().unwrap_or_else(|p| p.into_inner()).proof_time)
        .sum();
    let outcomes: Vec<_> = receipts
        .iter()
        .zip(&results)
        .map(|(receipt, result)| {
            let mut receipt = receipt.lock().unwrap_or_else(|p| p.into_inner());
            // A file confirmed without its own error is not tainted by a sibling's refusal.
            if receipt.completed && receipt.error.is_none() {
                return receipt.outcome(Ok(()), false);
            }
            let aborted = result.as_ref().copied().unwrap_or(false);
            // Preserve every confirmed success even if a later item fails/cancels.
            let result = result
                .as_ref()
                .map(|_| ())
                .map_err(|error| Error::from_hresult(error.code()));
            receipt.outcome(result, aborted)
        })
        .collect();
    let wall = started.elapsed();
    let setup = timing.setup.unwrap_or(wall);
    let native = timing.native;
    log::info!(target: "oxyde_lib::delete_timing",
        "trash_native_batch first={:?} files={} completed={} recycle={} failed={} cancelled={} wall_s={:.6} setup_s={:.6} native_s={:.6} proof_s={:.6} remaining_s={:.6}",
        files.first().map(|(path, _, _)| path), files.len(), outcomes.iter().filter(|outcome| outcome.completed).count(),
        files.first().map(|(_, recycle, _)| *recycle).unwrap_or(false), outcomes.iter().any(|outcome| !outcome.completed || outcome.error.is_some()),
        control.cancel.load(Ordering::Relaxed), wall.as_secs_f64(), setup.as_secs_f64(),
        native.as_secs_f64(), proof.as_secs_f64(), wall.saturating_sub(setup + native).as_secs_f64());
    outcomes
}

#[derive(Default)]
struct Timing {
    setup: Option<std::time::Duration>,
    native: std::time::Duration,
}

/// One native operation over the pending files. A file that cannot be queued
/// (vanished, refused name) fails alone and the others still run.
fn perform(
    files: &[(PathBuf, bool, HistoryFileIdentity)],
    receipts: &[Arc<Mutex<Receipt>>],
    pending: &[usize],
    control: &ShellDeleteControl,
    timing: &mut Timing,
) -> windows::core::Result<bool> {
    let started = std::time::Instant::now();
    let operation: IFileOperation =
        unsafe { CoCreateInstance(&FileOperation, None, CLSCTX_INPROC_SERVER)? };
    // Caller splits batches by recycle policy, never mixing permanent and recycled entries.
    let recycle = files.first().map(|(_, recycle, _)| *recycle);
    if let Some(recycle) = recycle {
        unsafe {
            // Independent files: one refusal must not abort the rest of the batch.
            operation.SetOperationFlags(base_flags(recycle))?;
        }
    }
    let mut sinks = Vec::new();
    for &index in pending {
        let ((path, file_recycle, proof), receipt) = (&files[index], &receipts[index]);
        if control.cancel.load(Ordering::Relaxed) {
            return Err(Error::from_hresult(E_ABORT));
        }
        if Some(*file_recycle) != recycle {
            return Err(Error::from_hresult(E_ABORT));
        }
        let queued = (|| -> Result<IFileOperationProgressSink, String> {
            let buffer = super::super::windows_shell_delete::source_buffer(std::slice::from_ref(path))?;
            let root: IShellItem = unsafe { SHCreateItemFromParsingName(PCWSTR(buffer.as_ptr()), None) }
                .map_err(|error| error.to_string())?;
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
            unsafe { operation.DeleteItem(&root, &sink) }.map_err(|error| error.to_string())?;
            Ok(sink)
        })();
        match queued {
            Ok(sink) => sinks.push(sink),
            Err(error) => {
                let mut receipt = receipt.lock().unwrap_or_else(|p| p.into_inner());
                receipt.refused = true;
                receipt.fail(error);
            }
        }
    }
    timing.setup.get_or_insert(started.elapsed());
    if sinks.is_empty() {
        return Ok(false);
    }
    let native_started = std::time::Instant::now();
    let performed = unsafe { operation.PerformOperations() };
    let aborted = unsafe { operation.GetAnyOperationsAborted() };
    timing.native += native_started.elapsed();
    performed?;
    Ok(aborted?.as_bool())
}

#[cfg(test)]
#[path = "batch/tests/operations.rs"]
mod tests;
