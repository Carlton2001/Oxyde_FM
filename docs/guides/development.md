# Developing and checking Oxyde

On Windows, use Rust MSVC x64 and the Visual C++ Build Tools. `--offline` commands need dependencies already downloaded; drop the flag the first time.

## Build and run

Close Oxyde before rebuilding its executables:

```powershell
cargo build --offline --bins
cargo run --offline
```

`cargo run` alone does not rebuild workers. Keep the three helper executables next to the main binary. To ship: [distribution](distribution.md).

## Check a change

```powershell
cargo check --offline --locked --all-targets
```

Run the relevant tests and report the scenarios actually discovered, failures and unverified targets. A filter matching no test validates nothing. Compilation, native tests and [visual checks](visual-checks.md) are different things.

The full Windows suite uses a private TEMP/TMP, compiles all targets, builds binaries explicitly, then runs tests serially and prints the time of each phase:

```powershell
./scripts/test-windows.ps1 -Offline
```

`.github/workflows/tests.yml` uses the same script. Linux uses `scripts/check-linux.sh`: see [Linux development](linux-development.md), which also covers `scripts/check-windows.sh` for compiling the Windows target from Linux.

Native CI uses current stable Rust and the dev profile, with Cargo dependencies restored by a pinned `Swatinem/rust-cache`. This cache is separate from the release distribution cache and saved only after success on the default branch. An Oxyde-only change keeps the dependency key; compiler, manifests, lockfile and Rust configuration are part of it.

## Split without changing behavior

A facade declares private submodules and assembles their results. Name children by responsibility (`details`, `validation`, `thumbnails`), without catch-all modules or circular dependencies. Spread `impl` blocks across children when that keeps state private. Keep helpers local and use targeted re-exports.

When extracting, preserve action types, captures, GPUI identifiers, focus, event propagation, overlay order, geometry, virtualization and task/subscription owners. Work in verifiable steps, within the change's scope.

## Where unit tests go

New tests go in a `tests/` subfolder of the module. The root [`tests/`](../../tests/README.md) folder is for public-interface integration tests. Keep fixtures and helpers near their scenarios.

Moving a file keeps the Rust parent and module name:

```rust
#[cfg(test)]
#[path = "search/tests/content_tests.rs"]
mod content_tests;
```

The extracted file holds the former module's body directly, with no added `mod tests`. Keep target conditions, attributes (`ignore`, Tokio, `should_panic`), assertions, synchronization and cleanup. Do not widen visibility; conditional helpers may stay in the parent.

Check the paths of `include_str!`, `include_bytes!`, `file!` and `#[path]`. Compare test discovery before and after, then compile and run the same available configurations. Do not migrate every small existing block as part of a local task.
