# Diagnosing deletion and the Trash

On Windows, ordinary files with a captured identity go through Shell batches of at most 256; folders and special objects keep individual tracking. Batches are split by the volume's trash policy. Each item keeps its checks and receipt, even if the batch ends in error or cancellation.

Progress is published when a batch returns. Pause and cancel are checked before batches and in callbacks; a Windows call already blocked stays system-dependent. Priorities follow Turbo. No artificial per-item wait is added.

The `delete_tree_benchmark [PARENT]` example times permanent deletion of a generated folder (flat, nested, large files), without and with Turbo.

## Where to look in `oxyde.log`

| Target / event | Use |
| --- | --- |
| `oxyde_lib::delete_timing` | Start/end, selection, plan and batch execution |
| `operation_cancel_requested` / `operation_cancel_accepted` | Time to obtain locks; then compare with the end of the operation |
| `trash_native_batch` | `setup_s`, `native_s`, `proof_s`, `remaining_s`, `wall_s` and results |
| `oxyde_lib::trash_restore` | Enumeration, names, progress, errors and restore result |
| `oxyde_lib::trash_purge` | Separates `empty_native` emptying and `selected` deletion, then preparation and Shell call |

`proof_s` is inside `native_s`: do not add them. `wall_s` starts in the native thread and excludes its creation and later receipt recording. These durations do not measure prior waiting in the UI loop.

## Comparing batch sizes

On fixture copies, close Oxyde between runs and recreate the same set before each deletion:

```powershell
$env:OXYDE_TRASH_BATCH_SIZE = "64"
cargo run --offline
# Close Oxyde and recreate fixtures before the second trial.
$env:OXYDE_TRASH_BATCH_SIZE = "256"
cargo run --offline
Remove-Item Env:OXYDE_TRASH_BATCH_SIZE -ErrorAction SilentlyContinue
```

Build the [binaries and workers](../guides/development.md) first. Only 64 and 256 are accepted; an invalid value is logged and uses 256. The value is read at the start of each operation, without changing persistent preferences.

## Smoothness and partial effects

Start with about 500 small files. For an idle slowdown, keep the existing history and hover with the manager closed: no need to recreate a large transfer. UI summaries and persistence should avoid periodically recopying every selection: see [operations](../architecture/operations.md).

Direct restore handles items independently, without overwriting or creating a missing parent folder. An error does not stop the next ones; the result holds successes, failures and the first failing path. The Undo journal keeps its own proof protocol. Tests must not empty the user's real Trash.
