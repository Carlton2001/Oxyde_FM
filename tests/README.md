# Integration tests

Tests of public interfaces, each file its own Cargo test binary. Unit tests live in the `tests/` folder of their module: see the [development guide](../docs/guides/development.md#where-unit-tests-go).

Most link the library without `cfg(test)` and launch real workers, so rebuild first: `cargo build --offline --bins`. On Windows, prefer `./scripts/test-windows.ps1 -Offline`.

| File | Target | Covers |
| --- | --- | --- |
| `archive_io_pipeline.rs` | Windows | Production extraction, creation and mutation through real workers |
| `archive_io_worker.rs` | Windows | Decode/Encode worker protocol, transport and hostile processes |
| `archive_listing_worker.rs` | Windows | Listing worker |
| `archive_path_resolution.rs` | Windows | Resolution of paths inside archives |
| `linux_archive_workers.rs` | Linux | Production archive path: confined workers, effects in the parent |
| `preview_worker.rs` | Windows | Preview worker transport |
| `search_content_memory.rs` | All | Text search memory; separate binary so allocator counters exclude other suites |
| `search_office.rs` | Windows | Public Office search with its real LPAC worker |
| `transfer_destination_collisions.rs` | Windows | Production transfer planning |

`support/` holds what these binaries pull in with `#[path]`: shared fixtures (`iso.rs`, `seven_crc.rs`), scenario groups, and standalone adversarial programs compiled by the tests and never shipped (`forged_tar_worker.rs`, `sandbox_probe.rs`). `src/platform/windows_process_launch.rs` also uses `native_probe.rs`.
