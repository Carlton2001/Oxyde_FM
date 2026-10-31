# Copy engine benchmark

An experimental tool, independent of Oxyde's normal operations and history. It runs five modes on an existing file or folder:

| CSV `mode` | Transfer | BLAKE3 |
| --- | --- | --- |
| `native_blake3` | Current Windows API via `Platform` | Re-reads the destination |
| `native_plain` | Same Windows API | None |
| `stream_blake3` | Rust loop, 1 MiB blocks | Computed during transfer |
| `stream_plain` | Same Rust loop | None |
| `native_source_blake3` | Windows API, with a parallel source reader | Source hash |

All modes process files sequentially at the same priority. It benchmarks ordinary data: the Rust engine does not reproduce dates, attributes, alternate streams or the production engine's guarantees, and it is not a drop-in replacement. Links and reparse points are refused.

## Usage (Windows)

Close Oxyde before building if its executables are in use.

```powershell
cargo build --offline --release --bin copy_benchmark
.\target\release\copy_benchmark.exe --help
.\target\release\copy_benchmark.exe "C:\data\large-file.bin" "E:\trials" 5 > copy-large.csv
.\target\release\copy_benchmark.exe "C:\data\small-files" "E:\trials" 5 > copy-small.csv
```

The destination must be an existing folder outside the source, with room for a full copy. Each trial creates a temporary folder there and removes it after measuring; existing files are not overwritten. A hard kill can leave an `oxyde-copy-benchmark-*` folder to clean up by hand.

Run a correctness trial first, on a small sample:

```powershell
.\target\release\copy_benchmark.exe "C:\data\sample" "E:\trials" 1 --validate
```

`--validate` computes a source reference before the trials and compares source, destination and any engine hash after each pass. These reads are untimed but warm the cache. Do not change the source during trials. Without this option only sizes are checked.

## Reading results

- `total_s`: wall time of the pass, including folder creation, preparation, transfer, hashing, flush, publication and staging cleanup. Independent validation and final removal are excluded.
- `transfer_s`: cumulative transfer time. Includes the native flush in `native` modes and inline BLAKE3 in `stream_blake3`.
- `stream_flush_s`: explicit `sync_all` of the Rust engine. Zero for native does not mean no flush: it is in `transfer_s`.
- `post_hash_s`: BLAKE3 re-read after copy, `native_blake3` only.
- `publish_s`: rename from temporary to final names.
- `mib_s`: ordinary data volume over total time.
- `source_hash_s`: source hashing, `native_source_blake3` only. It overlaps the native copy: do not add it to `transfer_s`.
- `hash_wait_s`: remaining wait for the source reader after the native copy, thread join included.

The native hash re-reads the destination; the inline hash covers the blocks transferred; the fifth mode hashes the source with an independent reader, holding the Windows read lock through transfer, hashing and publication. A copy or hash error prevents publication. Because it reads the source twice, that mode can compete with the transfer or benefit from the cache: use `--validate` on a sample, then compare timings without it, across and within disks.

Mode order rotates each repetition (five repetitions balance positions). The Windows cache is not purged. Compare several runs, look at spread, test large files, small files and each disk pairing separately, and close other I/O. The figures do not measure the interface, conflicts, history or concurrent copy.
