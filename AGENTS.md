# Oxyde: agent instructions

Native file explorer in Rust, GPUI and Tokio. Documentation: [index](docs/README.md).

## Standing rules

- Keep `cargo check` free of project warnings.
- Keep expensive work and file operations off the UI thread.
- System APIs go through the `Platform` trait (`src/platform/mod.rs`), with implementations and dependencies gated by target.
- Prefer native interactions and left click for primary actions.
- Use the official Lucide SVGs from `lucide-icons/lucide/icons/<name>.svg`, with unmodified paths. If the source is unreachable, report it.
- Do not add or change a cursor change unless the user explicitly asks. New interactions keep the default cursor.
- Write commit messages in English. **Every push to GitHub requires an explicit request for that push**, branches and tags included, whatever the tool. A request to change, commit or merge does not authorize it, and an earlier approval does not cover the next push.
- After changing workers or their shared code, say they must be rebuilt: close Oxyde, `cargo build --offline --bins`, then `cargo run --offline`. `cargo run` alone does not rebuild the helper executables.

## Read before changing

| Area | Reference |
| --- | --- |
| Interface | [UI conventions](docs/architecture/interface.md) |
| Menus, tooltips, secondary windows | [Windows](docs/architecture/windows.md) |
| Previews, thumbnails, sessions | [Previews](docs/architecture/previews.md) |
| Archives, confinement, guarantees | [Archives](docs/architecture/archives.md), [security limits](docs/security/status.md) |
| Final process exit | [Shutdown](docs/diagnostics/shutdown.md) |
| Module splitting and tests | [Development](docs/guides/development.md) |

## Code organization

- `src/ui/app/` owns business state and actions, `components/` composes the interface, `elements/` provides primitives. Business decisions stay in the calling view.
- One identifiable responsibility per module. Facades declare and assemble; they do not accumulate traversals and rendering.
- Aim for 200–600 production lines. Past 800, consider splitting before a significant extension, or justify keeping it. Do not split artificially or count tests as production.
- Share a component only when several uses share presentation or behavior. Preserve minimal visibility, GPUI identifiers and lifecycles. Limit refactoring to the change.
- New unit tests go in the module's `tests/` folder, without changing their Rust parent or widening visibility: see the development guide.
- Use semantic colors and the catalogs of all five languages.

`reference_zed/` and the partial Zed `v1.21.0` checkout in `C:\Users\Biluf\Downloads\zed-1.21.0` are GPUI references for rendering and style; do not take their dependencies.
