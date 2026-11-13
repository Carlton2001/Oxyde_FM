# File operations and history

`FileOperationManager` runs in the background; `OxydeApp` owns interface state. `components/operations/` separates the compact notifications (`compact`), the window's list (`list`), the selected operation's detail (`detail`, with its controls, metrics and graph) and its items (`items`).

## Presentation and persistence

Each active operation has a compact progress notification. Its end removes the line and shows the usual result. A click opens or reactivates the single resizable operations window on that operation; the toolbar button opens it on the current selection. It never opens automatically. Closing it keeps operations and history; closing the main window removes it.

The window is master-detail and follows the secondary windows' layout (the Duplicates window): a header, padded content, then the shared `tool_window::footer`. On the left, the operations sit as the Duplicates locations: a section per group (running, then finished) with its icon, capitalised name and count, its rows in a framed box. A row shows the kind, the percentage while running or the state on its badge, the sources ("name and N others") and a thin bar while running; the selected one takes the selection fill and an accent name. An edge between the list and the detail, a copy of the file chooser's sidebar edge, resizes the list; its width is saved in rems (`operations_list_width`) and a double click restores the default. The window restores its size and maximized state with its position (`tool_geometry::restore_sized`). The list reads `list_summaries()`, which does not clone sources. The window opens on the operation asked for (a compact notification's), else on the newest running one; the selection otherwise stays on a listed operation, else falls back to the newest running one, else the newest.

On the right, the selected operation's kind, sources and state badge, then the metrics: volume and item count on one line, the shared source folder once ("From", `source_parent`) and the destination; both open in the main window on click, a compression's destination selecting the archive. Then the items: a title with a badge per state count and a filter field, which takes its own line when the pane is narrow, over the main window's details view in a framed box (Name, Location only without a shared folder, Type, Size and State columns, sortable from their headers). Rows load with their metadata in the background (moved and copied items resolved at their destination once their source is gone); sorting (`file_ops::compare_names` for names) and filtering (by full path, or by name when a shared folder hides the path) also run off the UI thread. A double click shows the item where it is now, selected in its folder in the main window (`OxydeApp::show_in_main_window`).

States are written on badges (`items::outcome::badge`) whose fill also differs by lightness: solid accent while running, solid danger on failure, tinted success and warning, neutral otherwise. The footer holds "Remove finished", then the selected operation's commands as labelled buttons (pause or resume, Turbo as a toggle, cancel; remove once final) and Close.

Progress uses `elements::progress_bar` in the detail, the list and the compact notifications: a rail, a fill gliding between updates and a sweep while the work finalizes past its last byte (off with reduced motion). A pause dims the bar, percentage and graph to `text_muted` rather than switching amber for amber, shows "Paused" instead of the stale speed and remaining time, and the compact notification shows the pause icon and label.

The graph shows only during active operations with throughput measures, as wide as the progress bar, on a grid drawn in the same canvas so it frames the plotted area. In final states, duration and average throughput stay in the metrics; both leave out the time spent paused (`FileOperation::active_duration`). Main window summaries go through `operation_counts()`, `list_active_progress()` and `operation_state()`; do not clone every source and result for a counter or hover.

The operation list stays in memory for the session only. No operation history is written or restored at startup; old `operation_history.json` files are ignored.

### Interrupted operations

Copies, compressions and undone moves write into a private staging folder (`.oxyde-copy-`, `.oxyde-compress-`, `.oxyde-history-` followed by a UUID) beside their destination, removed when the work ends, fails or is cancelled. A process that ends during the work (crash, kill, power loss) leaves it behind. `systems::staging_journal` therefore notes each folder, with the identities of the folder and of its `.pin` marker, in `interrupted/` beside `config.json`; the owner keeps the note locked and removes it after its cleanup. At start, a note that can be locked again belongs to an ended process: if the folder is still the one created, its `output` and marker are removed with it and a notification names the folders concerned; anything else (replaced folder or marker, unknown child) is left in place and logged. The application turns the notes on in `main`, its workers included; tests and tools write none. Interrupted operations are not resumed.

### Closing during an operation

Closing the main window (title bar button, Alt+F4, the compositor's close request) while an operation runs or is paused opens the `QuitConfirmation` fixed dialog (`ui/app/quit.rs`, `components/quit_confirmation.rs`) instead of closing. *Ne pas quitter*, focused first, Escape and the dialog's own close keep Oxyde open. *Annuler et quitter* cancels every active operation, so each cleans its partial output as an in-session cancellation does, waits for them to end (at most 15 s, polled every 100 ms; presses are ignored meanwhile, and only a wait past 800 ms disables the buttons and says so, since a quicker end would just flash the message) and then closes through `close_main_window_now`. Installing an update waits too: while operations are active, `install_update` refuses with a notification.

## Per-item results

| State | Meaning |
| --- | --- |
| Processed | Confirmed kept result |
| Not processed / skipped | No processing, or a decision to skip |
| Failed | Refused by the system (in use, access denied…) while the operation went on with the others |
| Interrupted | Started, with no confirmed kept effect |
| Partially processed | Confirmed kept effects: published file or created folder |
| Unknown | Insufficient proof, old result or uncovered virtual path |

Real receipts take priority over progress. Transfers track descendants and removal of the moved root; deletions track actual confirmation. Old non-discriminating Partial results load as unknown, with JSON versioning. A final error does not remove confirmed receipts.

### Partial deletions

A permanent deletion or a move to the trash goes on after an item the system refuses: the item is marked Failed, the others are processed. At the end, `DeleteBatchFailure` summarizes them in a fixed notice (`app/dialog_windows/fixed/notice.rs`): how many items failed out of how many, the cause when the system named one (file in use, access denied), the paths concerned and a Retry button that runs the same deletion again on the selected items still on disk. Operation notices share this layout: a heading, the cause set apart, kept locations in their own panel, then a hint.

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
