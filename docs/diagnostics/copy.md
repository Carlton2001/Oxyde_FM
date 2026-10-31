# Measuring a real copy

Normal operations write to `oxyde.log`, next to `config.json`. The `session start` line gives the exact path; in portable mode use `data/oxyde.log`. The separate [benchmark](../guides/copy-benchmark.md) produces a CSV and does not measure the application.

After [building](../guides/development.md), copy a sample from the interface, wait for the end and read the `copy_timing` lines:

```powershell
Get-Content "$env:APPDATA\oxyde\oxyde.log" | Select-String 'copy_timing'
```

The `copy_concurrency_benchmark DESTINATION...` example compares the production policy at 1, 2 and 4 workers on generated files (large, medium, small); `COPY_BENCH_SET` limits the set. Compare several normal/Turbo runs in alternating order, with the same files, destination and media. The Windows cache stays active. A small batch and a batch of large files are enough for a first diagnosis.

## Reading phases

Target: `oxyde_lib::copy_timing`, durations in seconds. For the exact inventory, see `src/systems/copy_diagnostics.rs` and search `copy_timing` in `src/`.

| Event | Measures |
| --- | --- |
| `copy_operation_start` / `copy_operation_end` | Operation, Turbo mode, duration and result |
| `copy_source_selection` | Root capture and overlap check |
| `copy_plan_traversal` | Traversal and plan/conflict preparation, pauses included |
| `copy_file` | One file, from copy to publication/cleanup |

| Per-file fields | Meaning |
| --- | --- |
| `published`, `bytes` | Successful publication and size completed before hashing; zero possible on early failure |
| `wall_s`, `pause_s`, `active_s` | Real duration, observed pauses and their difference |
| `native_s` | Platform copy, flush included, observed pauses excluded |
| `flush_s`, `flush_calls` | Explicit flush calls; **time already in `native_s`** |
| `hash_s` | BLAKE3 re-read of the destination, observed pauses excluded |
| `publish_s` | Final rename, failed attempt included |
| `parents_s`, `checks_s` | Parent resolution and checks before copy/publication |
| `staging_create_s`, `writer_release_s`, `output_open_s` | Private preparation, closing the writer and verified reopening |
| `cleanup_s`, `remaining_s` | Staging destructor and unattributed remainder, without counting flush twice |

## Avoiding wrong conclusions

Operation time includes planning, waiting for workers and pauses between files. Worker durations overlap: their sum is not real time. The per-file path excludes some preparation and later history recording.

`native_s - flush_s` is not pure hardware throughput. `hash_s` may read the system cache. A same-volume move may produce no copy line. Summaries may be missing after a hard stop; `published=false` means no publication. Remove personal paths from logs before sharing.
