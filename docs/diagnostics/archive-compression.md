# Measuring archive compression

`oxyde_lib::archive_timing` logs separate plan preparation, encoding and publication. The Windows supervisor adds worker startup, parent handler time, checks, event waits and frame/read/write/checkpoint counters. Counters change neither limits nor protocol.

The `archive_extraction_benchmark` example measures production extraction without and with Turbo, for the four formats built from one folder; extracted content is verified untimed. `archive_worker_timing` lines separate `read_handler_s` (source reads) and `chunk_handler_s` (writing decoded data).

## Reproducible comparison

Close Oxyde before rebuilding shared binaries:

```powershell
cargo build --offline --bins --example archive_compression_benchmark
$env:RUST_LOG = "warn,oxyde_lib::archive_timing=info"
cargo run --offline --example archive_compression_benchmark -- "C:\data\folder"
```

A second argument picks the format: `zip` (default), `7z`, `tar` or `tar.zst`. An optional third filters a mode, such as `production-normal-turbo`, or `turbo` for both Turbo qualities. The report shows the codec's effective level (TAR: 0).

With no argument the tool generates a small private set under `target/`. With a folder, it reads without writing; links and reparse points are refused. Output goes in a new `target/archive-compression-<uuid>/`.

| Mode | What it measures |
| --- | --- |
| `production-fast` | Windows production path, isolated worker, ZIP level 1, no Turbo |
| `production-fast-turbo` | Same path and level, with Turbo |
| `production-normal-turbo` | Production path, normal quality kept, with Turbo |
| `direct-fast` | Same ZIP library at level 1, written directly by the tool |
| `direct-normal` | Same direct writing at level 6 |

Direct modes apply to ZIP only. Production modes run for each chosen format; ZIP/7z numeric levels do not match Zstandard levels.

Archives are decompressed and their inventory/content checked against BLAKE3 hashes of the sources, untimed, then removed. The JSON report and generated fixtures stay.

## Interpretation

- The direct comparison is an experimental control, without source guards, confinement or production exchanges. No direct fallback is added to the application.
- A single run with a cache warmed by the manifest and a fixed order gives no guaranteed throughput. Direct ZIP is not Explorer's engine.
- `transport_wait_s` is time waiting on transport events. It can overlap worker computation: it is neither CPU time nor an exact IPC overhead.
- `handler_s` covers parent callbacks; `controls_s` covers loop checks. The rest includes frame encoding/decoding, communication and worker work. Do not call it "pure compression".
- `write_bytes` counts transmitted bytes, header rewrites included, and can exceed the final size. Exchange count and average size help spot too-fine granularity.
- Read preparation/publication times and counters before attributing the whole duration to the codec or isolation.

To measure in the interface, rebuild workers with `cargo build --offline --bins`, run `cargo run --offline`, and filter `archive_timing` in `oxyde.log`.
