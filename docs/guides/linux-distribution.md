# Distributing Oxyde on Linux

RPM and DEB packages are built and tested in CI. The Windows side has [its own guide](distribution.md). Packaging sources: [`packaging/`](../../packaging/README.md).

## Build

```bash
bash scripts/build-linux-packages.sh --rpm   # on Fedora
bash scripts/build-linux-packages.sh --deb   # anywhere: podman outside Ubuntu 24.04
```

Files and their SHA-256 land in `target/linux-packages/output`. Beyond the [development dependencies](linux-development.md#setup-on-fedora-workstation), Fedora needs `rpm-build`, `desktop-file-utils`, `appstream`, `python3` and `cargo-about` 0.9.2. The DEB builds in the image from `packaging/linux/debian/Containerfile`, with its own `target/ubuntu-24.04` folder; `lintian` stops the build on an error, `rpmlint` is informational. `--skip-build` reuses existing release executables. `--offline` keeps the previous build's notices: `cargo-about` reads crates for every target, macOS included, which a Linux cache does not hold.

A package installs only on a system at least as recent as the one that built it (glibc): Fedora 44+ for the RPM, Ubuntu 24.04+ and Debian 13+ for the DEB. x86-64 only, with no signature or repository for now. `release.yml` does not publish these packages: a release remains Windows only.

## Package contents

- `/usr/bin`: `oxyde`, `oxyde-archive-list-worker`, `oxyde-archive-io-worker`, side by side because Oxyde looks for them next to its executable. `oxyde-image-worker` is empty on Linux and not shipped.
- Desktop entry, hicolor icons (SVG and PNG 16–512), AppStream metadata. PNGs are kept in Git and regenerated with the other logo variants: `cargo run --offline --locked --example generate_application_icons`.
- License and third-party notices, regenerated at each build by `scripts/linux-third-party-notices.py`, the counterpart of `update-third-party-licenses.ps1`; the file in Git stays the Windows one. The RPM `License` field is computed by `scripts/rpm-license.py` from crates linked on Linux and embedded resources.
- `StartupNotify=false`: GPUI does not consume the startup token, and GNOME would keep the busy cursor until its timeout.

No RPM scriptlets: Fedora's package triggers refresh the desktop and icon caches, including on Silverblue where only `/usr` is delivered.

## Dependencies

GIO, libseccomp, Vulkan, Wayland, Fontconfig, poppler and GStreamer load at runtime, so packaging tools only find `libxcb`, `libxkbcommon` and libc. The rest is declared by hand:

| Role | Fedora | Debian/Ubuntu | Level |
| --- | --- | --- | --- |
| Decoder confinement | `bubblewrap`, `libseccomp` | `bubblewrap`, `libseccomp2` | required |
| GIO and trash settings | `libgio-2.0.so.0` (`glib2`) | `libglib2.0-0t64`, `libglib2.0-bin` | required |
| Rendering | `vulkan-loader`, `mesa-vulkan-drivers` (lavapipe fallback) | `libvulkan1`, `mesa-vulkan-drivers` | required |
| Windows and fonts | `libwayland-client`, `libfontconfig`, `default-fonts-core-sans` | `libwayland-client0`, `libfontconfig1`, a system font | required |
| Opening in another application | `xdg-utils` | `xdg-utils` | required |
| PDF | `poppler-glib` | `libpoppler-glib8t64` | recommended |
| Video and audio | `gstreamer1-plugins-base`, `-good` | `gstreamer1.0-plugins-base`, `-good`, `libgstreamer-plugins-base1.0-0` | recommended |
| Network | `gvfs`, `gvfs-fuse`, `gvfs-smb` | `gvfs`, `gvfs-fuse`, `gvfs-backends` | recommended |
| Volumes | `udisks2` | `udisks2` | recommended |

Confinement is required because Oxyde otherwise decodes without it, with only a log warning. Each recommended item removes only its own feature.

## Ubuntu and AppArmor

Ubuntu 23.10+ denies user namespaces to programs without an AppArmor profile allowing them: bubblewrap fails and decoders would run unconfined. As Ubuntu does for Nautilus, the DEB installs `/etc/apparmor.d/io.github.Carlton2001.Oxyde`, an `unconfined` profile attached to `/usr/bin/oxyde` that only adds `userns`. It applies to that path only: an Oxyde run from elsewhere stays unconfined on Ubuntu. The profile is unloaded on removal and the file deleted on purge. A system without the restriction (Debian) does not need it.

Without the profile the log shows `bwrap: loopback: Failed RTM_NEWADDR` and "Preview workers run without a sandbox".

## Opening a folder

`Exec=oxyde %U` and `MimeType=inode/directory` put Oxyde in "Open With" without making it the default handler. Like a new Nautilus window, launching shows only the received locations, one tab each, and leaves saved tabs for the next launch (`platform/linux_startup.rs`).

Limits: each launch is a new process, with no single instance, and its preference save rewrites the session it read at startup; a received file opens its folder without selecting it; a non-UTF-8 name is ignored.

## Continuous integration

`.github/workflows/linux.yml`, on `main` and its PRs:

- **tests** on `ubuntu-24.04`: warning-free compile, full suite, then `preview_jobs` separately, whose 2 s timeouts fail when the whole suite loads the machine. The AppArmor restriction is lifted so confinement tests run.
- **rpm** in a `fedora:44` container and **deb** on `ubuntu-24.04`: build, install with dependencies, downloadable artifact.

Rust is pinned to 1.99.1 and `cargo-about` verified by hash. Containers forbid bubblewrap, so they do not exercise confinement.
