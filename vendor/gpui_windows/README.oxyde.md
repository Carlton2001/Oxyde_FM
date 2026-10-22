# Oxyde GPUI Windows patch

Source: `crates/gpui_windows` from Zed commit
`33c95853ed2b6956f339733c63a8220964ecbeb6` (the application's existing pin).
Upstream source and Apache license are preserved. This is the same dependency,
not a second renderer or an upgrade. Upstream files larger than the project's
module guideline are kept intact to make the native patch reviewable.

The crate's upstream LICENSE-APACHE symlink is expanded to the license text
from the same repository revision, so it remains readable in this standalone copy.

The manifest resolves workspace inheritance to the same versions, features and
pinned Git packages already in Oxyde's lockfile. No upstream dependencies are
added to Oxyde. `Cargo.toml` selects this copy through a source patch.

Local source changes are confined to `src/directx_atlas.rs`, its pure
`atlas_upload.rs` validation helper, and one module declaration:

- New full-size atlases (1024..16384 pixels on both axes) receive their first
  pixels via `CreateTexture2D` initial data instead of `UpdateSubresource`.
- Existing atlas slots, subregions and smaller images keep the original upload.
- Input length is checked before passing initial data to D3D11. Unexpected
  nonzero tile origins fall back to the existing upload for correct placement.
- Debug logs expose texture creation/removal/destruction; no graphics purge,
  working-set trim, sleep or GPU wait is added to production rendering.

Reason: on the tested Intel adapter, the original upload retained two 64 MB
native staging buffers after the texture and all `RenderImage`s were released.
A real GPUI eight-image/30-second-idle probe measured 261,808,128 versus
133,226,496 Private Bytes with initial-data creation (about 122.6 MiB less).
These are local hardware measurements, not a cross-driver memory guarantee.

The focused tests live in `src/tests/directx_atlas_memory.rs` and run directly
with `rustc --test src/atlas_upload.rs --edition 2024 -o <test.exe>` (then run
the executable). This tests the actual helper without enabling GPUI's upstream
`test-support` feature or adding its extra dependencies. Oxyde also has
opt-in D3D11 and real-GPUI lifecycle probes. See `docs/diagnostics/memory.md` in
the application repository. When updating GPUI, compare this copy to the new
upstream and remove the source patch once the upload fix exists upstream.
