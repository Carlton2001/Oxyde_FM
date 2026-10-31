# Developing the Linux version

The Linux port is experimental and lives in `main` alongside the published Windows versions. **Windows remains the reference platform and must not be altered**: a Linux change goes through the `Platform` trait or a `cfg(not(target_os = "windows"))` module. Touching a shared file requires checking Windows (see below).

## Setup on Fedora Workstation

Recent Linux Rust (validated with Rust 1.99.1, Fedora 44), then:

```bash
sudo dnf install gcc gcc-c++ make pkgconf-pkg-config \
  fontconfig-devel freetype-devel libxkbcommon-devel \
  libxkbcommon-x11-devel libxcb-devel wayland-devel vulkan-loader
```

On physical machines check the Vulkan driver: `mesa-vulkan-drivers` for Intel/AMD, the proprietary driver (RPM Fusion `akmod-nvidia`) for NVIDIA. The GPUI pin and `Cargo.lock` are shared with Windows: do not update them to fix an SDK problem.

## Check and run

```bash
bash scripts/check-linux.sh            # cargo check --locked --all-targets, warning-free
export CARGO_TARGET_DIR="${XDG_CACHE_HOME:-$HOME/.cache}/oxyde-gpui/target"
cargo build --locked --bin oxyde
```

Run on an **isolated profile** when testing file operations, so configuration, cache, trash and temporary files stay in a disposable folder:

```bash
P=~/oxyde-test; mkdir -p $P/{config,cache,data,tmp}
XDG_CONFIG_HOME=$P/config XDG_CACHE_HOME=$P/cache XDG_DATA_HOME=$P/data TMPDIR=$P/tmp \
  RUST_LOG=info "$CARGO_TARGET_DIR/debug/oxyde"
```

The log goes to `$P/config/Oxyde/oxyde.log` (chosen GPU adapter, decorations). An isolated trash only receives deletions on the same filesystem as `$P`, and **GNOME trash settings (`gsettings`) stay global**. Do not run writes on real data.

`bash scripts/install-linux-desktop.sh --exec <binary> --profile ~/oxyde-test` installs the desktop entry and icon for the user: see [desktop identity](../architecture/desktop-identity.md).

`oxyde-image-worker` is an empty `main` on Linux; previews run in a worker of Oxyde itself, confined by bubblewrap and seccomp ([Linux previews](../architecture/linux-previews.md)). Worker tests launch the executables in `target`: after a change, run `cargo build --offline --bins` before `cargo test`. WSL works for `check-linux.sh` but validates neither GPU, desktop nor volumes.

Root permission tests run in a user namespace: `unshare --map-auto --map-root-user <test binary> linux_permissions`.

## Preserving Windows

After any shared-file change, check from Linux that Windows compiles:

```bash
bash scripts/check-windows.sh          # cargo xwin check, MSVC target, all targets
```

It catches misplaced `cfg` and broken imports, without linking or running. One-time setup: `sudo dnf install llvm lld clang`, `rustup target add x86_64-pc-windows-msvc`, `cargo install --locked cargo-xwin`, then a first run with `XWIN_ACCEPT_LICENSE=1`, which accepts Microsoft's CRT and SDK license and downloads them (1.2 GB in `~/.cache/cargo-xwin`). `scripts/llvm-rc-crate-root.sh` makes `llvm-rc` look for resources from each crate root, as on Windows.

Tests still run on Windows: `./scripts/test-windows.ps1 -Offline`, or without rebooting by manually dispatching `tests.yml` on a pushed branch.

## Desktop behavior worth knowing

- Hybrid GPU: at startup, Vulkan drivers of cards with no connected screen are excluded (`VK_LOADER_DRIVERS_DISABLE`, `platform/linux_gpu.rs`), unless the user or desktop already chose a GPU, and applications opened by Oxyde do not inherit this. It keeps a discrete GPU suspended.
- Rounded corners: `client_decorations` rounds floating corners of client-decorated windows by 10 px, and no corner touching a screen edge. GPUI only clips to rectangles, so a background reaching a corner rounds itself with `inner_corners`.
- Scales: screens report logical size through xdg-output, because `wl_output` gives the next whole scale (1280x800 instead of 2048x1280 for 2560x1600 at 125%). The compositor hides window positions, so only the largest screen bounds a restored size. The minimum size yields on a logical screen too small for it.
- KDE: titled windows request `WindowDecorations::Client` (`ui::window_decorations`).
- Drag and drop: an incoming drop is always a copy, since GPUI only negotiates Copy on Wayland and X11; Ctrl and Shift stay with the source. Volumes are told apart by the deepest mount point (`/proc/self/mountinfo`, reread in the background), without resolving symlinks.
- Trash: `trash_transfer_entries` (`linux_trash/transfer.rs`) handles cut, paste and drag out of the trash. The `.trashinfo` record is set aside in `$trash/.oxyde-trash-*.tmp` only once the item has left, and Undo restores it. Other applications do not receive these items, as they would split content from its record. Contents of a trashed folder stay refused. Restoring to another filesystem goes through a move. KDE's `ktrashrc` limits and GNOME's cleanup stay theirs; Oxyde sets them and never purges on its own.

## Remaining work

Not yet verified:

- Vulkan on a physical Intel then NVIDIA GPU: backend obtained, shadows, resizing. External screen plugged into the NVIDIA card with Oxyde already running.
- A real external screen with mixed scales. Nested KWin ignores fractional scale.
- An X11/Xwayland session separately from Wayland.
- A real KDE Plasma session: shadows, secondary windows, trash limits.

Known gaps:

- X11 tooltips are still bounded by the window size and can bite a few pixels into the shadow margin (`tooltip/position.rs`).
- Undoing a deletion across filesystems, with no operation to show, is still refused.
- Dragging files to other applications is impossible on X11: GPUI does not offer files there.
- "More options" (Shell menu) is hidden outside Windows.
