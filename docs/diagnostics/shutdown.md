# Shutdown hang after a PDF preview

Reference: `SHUTDOWN-PDF-INTEL-2026-10-04`. The Windows workaround was manually validated by the user. An old mention of the hang does not prove a current regression.

## Contract to preserve

1. Close windows and finish application cleanup: history, temporary files and last preferences, including the geometry debounce.
2. Leave GPUI, then stop Tokio with a bound and flush logs.
3. Call `Platform::finish_process_shutdown` at the very end.
4. On Windows, stop the image then icon supervisors, then the icon persistence queue (bounded waits of two seconds each), then `TerminateProcess(GetCurrentProcess(), 0)`. The queue abandons pending requests; the active write can finish in that time. Linux returns normally.

`TerminateProcess` avoids final DLL detach callbacks; `std::process::exit` is not equivalent. This workaround replaces no save or shutdown hook. Stateless workers serialize and flush their response before exiting.

The GPUI hook schedules history cleanup on Tokio's blocking pool, waiting first for preferences and then for that cleanup. During this wait GPUI only polls the hook's future and runs no foreground task, so the `save_preferences` chain would never reach its write (`preferences save queued` without `write started`, then `timed out waiting on app_will_quit`). The hook therefore cancels that chain and writes the last snapshot on the blocking pool; a generation number stops an older write still in progress from replacing it. Identity/hash checks and backup removal thus no longer block the UI callback. GPUI gives hooks only 200 ms: independent references keep these writes alive and `main` waits for them after GPUI returns, before stopping Tokio and the native exit. Blocked hardware I/O can still delay this last wait.

The image supervisor also stops the audio cover session, with the same two-second wait per session; that cache holds only regenerable data. The native session supervisor stops Shell icons then reusable PDF previews, at most two seconds per service. The Viewer's PDF keeps its per-document session and existing owner.

Entry points: `src/main.rs`, `platform/windows_impl.rs`, `platform/linux_impl.rs`, `platform/windows_preview_worker/`. See [preview sessions](../architecture/previews.md).

## If the problem returns

1. Read the end of `oxyde.log` next to `config.json` (usually `%APPDATA%\Oxyde`, `data` when portable) and note the session PID.
2. Look for `application cleanup complete; final process exit`. If this step is missing, investigate the last step reached: the final workaround does not handle an earlier hang.
3. Also check WMI and `tasklist`. Absence from `Get-Process` is not enough to conclude the process ended.
4. If possible capture a dump before killing the process, then check the binary and reproduce with several PDF previews and closings.

## Original evidence

The window had disappeared and the log reached `main returning`, but a thread continued in the Intel driver `igd10um64xe.DLL` (offset `0x71398`), with D3D11 and `Windows.Data.Pdf.dll` on the stack. This locates the hang in the final native exit; without a fully symbolized stack, the exact driver defect is not established. `shutdown_returns_while_a_blocking_worker_is_still_running` checks Tokio's bounded stop, not the PDF/Intel defect.
