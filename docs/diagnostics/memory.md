# Viewer and preview memory

## Capturing a session

Close Oxyde, rebuild the executables including workers, then run with diagnostics on:

```powershell
cargo build --offline --bins
$env:OXYDE_MEMORY_DIAGNOSTICS = '1'
$env:RUST_LOG = 'warn,gpui_windows::directx_atlas=debug'
cargo run --offline
```

In a fresh session: wait a few seconds, browse the same images, close the Viewer, wait 30 seconds, then repeat. Keep the same tooltip preview and grid state between passes.

`memory-sample:` lines go to `oxyde.log`, next to `config.json` (usually `%APPDATA%/Oxyde`, `data` when portable). They cover startup, activity grouped at most once per second, then idle at 5 s, 30 s and every minute. `last_event` is the last one observed, not an exhaustive log. The registry is capped at 4096 images and holds only weak references. Probing and writing stay off the UI thread. When not enabled, no image is registered and no monitor starts.

## Reading counters

- `process_bytes`: Private Bytes, Working Set and committed private, mapped and executable Windows regions, with reserved space separate. Committed regions are not all resident. Main process only, not workers.
- `viewer_pixels.live_bytes`: Viewer image pixels still referenced, including loads awaiting publication and old frames. `pending_bytes` are decoded results not yet published; `retired_bytes` are removed from content but still strongly referenced.
- `preview_budget_entries_bytes`: live pixels in the shared preview budget; caches and frames may share ownership.
- `tooltip_cache_entries_bytes`: entries and bytes of the image then PDF caches. `audio_cache_unique_entries_bytes` dedupes covers by image identity. `icon_cache_entries_bytes` measures the parent's native cache.

Values are **bytes**, except entry counts. Measurements are sequential, not an atomic snapshot. Scopes overlap: do not add caches, budget and Viewer pixels, or subtract their sum from Private Bytes. `None` means a busy lock or unavailable measure, not zero. `untracked_registrations > 0` means incomplete coverage, so a zero pixel value then proves nothing. Tracking covers full-resolution images, PDF pages and Markdown images from decoding through publication and removal. Audio covers register at content publication. Viewer fallback previews stay in the shared budget and are not all in this registry. It is not an inventory of every GPUI object or transport buffer.

If Viewer pixels fall to zero and caches stay small, the remainder is not due to these pixel owners. Attributing it between heap/allocator, GPU atlas, driver, loaded code and other objects needs a native profile. This instrumentation measures neither individual textures nor free heap space.

## Grammar initialization

Syntect and the embedded grammars add about 24 MiB of Private Bytes when initialized, allocator retention included. The Viewer no longer preloads them when browsing images: they initialize off the UI thread when syntax analysis is requested. To replay alone, in a fresh process:

```powershell
cargo test --offline --locked --bin oxyde syntax_initialization_memory -- --ignored --test-threads=1 --nocapture
```

## Windows backend and initial upload

The GPUI Windows backend is a local copy of the same pinned revision with a targeted atlas fix: a new texture with both axes at least 1024 px receives its pixels directly at creation. Partial sizes and existing textures keep UpdateSubresource. On the Intel GPU tested this avoids two large native buffers retained after uploads. It adds no memory purge or GPU wait to production rendering. Provenance: `vendor/gpui_windows/README.oxyde.md`.

The `gpu-atlas-create`, `gpu-atlas-remove` and `gpu-atlas-destroy` logs come from the `RUST_LOG` value above. Removing a key and destroying a texture differ when several keys share it. These events do not directly measure allocations the driver keeps after COM Release.

Manual probes, each run alone:

```powershell
cargo test --offline --locked --lib directx_image_upload_memory_lifecycle -- --ignored --test-threads=1 --nocapture
cargo test --offline --locked --lib directx_image_initial_data_memory_lifecycle -- --ignored --test-threads=1 --nocapture
cargo test --offline --locked --bin oxyde native_viewer_large_image_graphics_memory -- --ignored --test-threads=1 --nocapture
```

The first two use synthetic data only and may call ClearState/Flush/Trim for diagnosis; these calls are not part of the rendering fix. The GPUI probe shows eight synthetic images, waits 30 s after removal and forces no driver purge. Measured amounts depend on the driver and are not portable assertions.

Pure tests of bounds and lengths before data reaches D3D11:

```powershell
rustc --test vendor/gpui_windows/src/atlas_upload.rs --edition 2024 -o target/atlas-upload-tests.exe
./target/atlas-upload-tests.exe
```

Direct compilation tests the production helper without enabling `gpui/test-support`, which adds Git test dependencies Oxyde does not need.
