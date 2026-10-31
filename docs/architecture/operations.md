# File operations and history

`FileOperationManager` runs in the background; `OxydeApp` owns interface state. `components/operations/` separates notifications, windows, cards, indicators and graph.

## Presentation and persistence

Each active operation has a compact progress notification. Its end removes the line and shows the usual result. A click opens or reactivates the single resizable manager, which never opens automatically. Closing or hiding the manager keeps operations and history; closing the main window removes its windows.

Active operations stay expanded; final states collapse by default. Details show at most five selected roots. "Affected items" opens a virtualized list per operation, with a full-path filter prepared in the background and stale results rejected. Icons reuse `render_file_icon`.

The graph shows only during active operations with throughput measures. In final states, duration and average throughput stay in the details. Main window summaries go through `operation_counts()`, `list_active_progress()` and `operation_state()`; do not clone every source and result for a counter or hover.

The operation list stays in memory for the session only. No operation history is written or restored at startup; old `operation_history.json` files are ignored.

## Per-item results

| State | Meaning |
| --- | --- |
| Processed | Confirmed kept result |
| Not processed / skipped | No processing, or a decision to skip |
| Interrupted | Started, with no confirmed kept effect |
| Partially processed | Confirmed kept effects: published file or created folder |
| Unknown | Insufficient proof, old result or uncovered virtual path |

Real receipts take priority over progress. Transfers track descendants and removal of the moved root; deletions track actual confirmation. Old non-discriminating Partial results load as unknown, with JSON versioning. A final error does not remove confirmed receipts.

## Turbo contract

The preference sets the initial mode of the six operation types; the button acts on one operation. It also drives folder size calculations, including those in progress and in Properties; operation buttons stay independent of these. Turbo drives neither navigation, search nor previews. Without Turbo, worker threads go to Windows background mode.

All priorities go through `apply_thread_priority` / `follow_turbo_priority` (`src/platform/thread_priority.rs`). Each transition is absolute, including the return to `THREAD_PRIORITY_NORMAL`, and an already applied state is not resent. `RestoreNormalPriority` returns reused threads to normal; an unexpected native failure is logged. Linux applies no priority: lowering a thread's is irreversible without privilege, and NVMe disks without a scheduler ignore I/O priority. On Linux, Turbo acts only through concurrency.

| Operation | Turbo priority | Turbo concurrency | When applied |
| --- | --- | --- | --- |
| Copy, cross-volume move | Normal | SSD (sources and destination): up to 4 files from 2 files. Rotating disk, network or unknown: up to 4 only with at least 8 files averaging under 32 MiB | Priority: between files and during copy. Concurrency: at file boundaries. |
| Same-volume rename | Normal | None | At operation start |
| Permanent deletion | High, from start | Up to 4 selected items; in a single selected folder, its first-level subfolders on up to 4 workers. Linux: one worker on Btrfs, slower in parallel ([details](linux-access.md#turbo-and-parallelism)) | Priority: between items and during traversal. Workers: fixed at each item's start. |
| Trash | Normal | None | Before each batch and at Shell checkpoints, folders included |
| Compression | Worker process priority | ZIP, 7z, TAR.ZST: [limits](archives.md#compression-and-turbo-on-windows) | At file or block boundaries; TAR.ZST at stream start |
| Extraction | Worker process priority | None; decoding stays single-threaded | At checkpoints |
| Folder sizes | Normal | Two global traversals; on SSD, up to 2 workers per traversal without Turbo and up to 8 with it, by available logical processors. HDD, network or unknown: 1 worker per traversal in both modes | Priority: during traversal. Worker budget: at each traversal's start. |

Storage type comes from `Platform::is_ssd` (seek penalty), queried once per drive; `\\?\` paths are normalized. Each copy rereads its output to verify it: on Windows, this read waits for Defender's scan (15–25 ms per file measured), a latency several workers overlap.

Structural limits: the Shell handles the Trash sequentially, TAR has nothing to compress, a rename transfers no data and a ZIP file stays one DEFLATE stream. Current implementation limits:

- Deleting a folder in Turbo: the calling thread enumerates the selected folder and opens each child; held first-level subfolders go to a bounded queue and are deleted whole by a worker. The selected folder goes last. The `Platform::remove_operation_tree` checkpoint is `Fn + Sync`: pause, cancel and priority apply in each worker, and a failure stops the others at their next checkpoint. Files directly in the selected folder stay deleted one by one (delegating each proved slower).
- lzma-rust2's multithreaded 7z decoding buffers whole LZMA2 segments without a bound: it stays disabled.
- Parallel ZIP extraction would need several simultaneous streams in the worker protocol.

## Conflicts and integrity

The plan reserves all destinations before the first effect, also across roots. Permission to replace an existing destination does not allow overwriting another source of the same batch. A move onto itself is skipped; a copy onto itself picks a free translated name and the backend keeps exclusive publication.

The dialog keeps source/target metadata and per-path decisions. Advanced mode inherits folder decisions, with explicit exceptions first. At the first individual choice, undecided items receive the opposite choice; this automatic default is identifiable and does not reapply. No conflicting transfer starts before full validation. Esc cancels the pending operation.

Copy/cut from the Trash keeps physical paths as identity and resolves names through `Platform`. Restore stays a separate path. Direct restore handles independent items without overwriting or creating a missing parent; its failures do not stop the next ones. Shell deletion keeps its path race limits: [security R2](../security/status.md).

To measure operations: [copy](../diagnostics/copy.md), [deletion](../diagnostics/delete.md). For virtual transfers: [archives](archives.md).
