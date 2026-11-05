# Distributing Oxyde on Linux

RPM and DEB packages and a per-user archive are built and tested in CI. The Windows side has [its own guide](distribution.md). Packaging sources: [`packaging/`](../../packaging/README.md).

## Build

```bash
bash scripts/build-linux-packages.sh --rpm   # on Fedora
bash scripts/build-linux-packages.sh --deb   # anywhere: podman outside Ubuntu 24.04
bash scripts/build-linux-packages.sh --tar   # per-user archive, same image as the DEB
```

Files and their SHA-256 land in `target/linux-packages/output`. Beyond the [development dependencies](linux-development.md#setup-on-fedora-workstation), Fedora needs `rpm-build`, `desktop-file-utils`, `appstream`, `python3` and `cargo-about` 0.9.2. The DEB and the archive build in the image from `packaging/linux/debian/Containerfile`, with its own `target/ubuntu-24.04` folder; `lintian` stops the build on an error, `rpmlint` is informational. `--skip-build` reuses existing release executables. `--offline` keeps the previous build's notices: `cargo-about` reads crates for every target, macOS included, which a Linux cache does not hold.

A package installs only on a system at least as recent as the one that built it (glibc): Fedora 44+ for the RPM, Ubuntu 24.04+ and Debian 13+ for the DEB, glibc 2.39+ for the archive. x86-64 only, with no repository, by choice: a release publishes the archive, the DEB, the RPM and `install.sh`.

## Signatures

Linux files are signed with Oxyde's own GPG key, distinct from the Windows updater's minisign key and from Soliton's: `Oxyde (package signing) <carlton2001@gmail.com>`, Ed25519, fingerprint `F9D7 6B9E 3BED D5EA CC9C  FE7D AC2E 961F 3202 9706`, public key in `packaging/linux/signing/oxyde-packages.asc`. `scripts/sign-linux-release.sh`, run by the release job with the secrets `OXYDE_GPG_PRIVATE_KEY` (armored secret key) and `OXYDE_GPG_PASSPHRASE`:

- signs the RPM's header (`rpmsign`), checked by `rpm -K` once the key is imported with `rpm --import`;
- writes a detached armored `.asc` for the archive, the DEB and the RPM, which `latest.json` carries;
- replaces the per-file `.sha256` with one `SHA256SUMS` listing every release file, Windows included, and signs it into `SHA256SUMS.asc`;
- checks every signature against the committed public key before the release is created.

A DEB carries no signature of its own, which `apt` would not check: `SHA256SUMS.asc` covers it. By hand:

```bash
gpg --import oxyde-packages.asc
gpg --verify SHA256SUMS.asc SHA256SUMS && sha256sum --ignore-missing -c SHA256SUMS
```

The key is also compiled into Oxyde (`platform/linux_signature.rs`, fingerprint pinned) and written into `install.sh`; tests check both copies against the committed file.

## Per-user installation

`packaging/linux/install.sh` is the single entry point, published with each release:

```bash
curl -fsSL https://github.com/Carlton2001/Oxyde_FM/releases/latest/download/install.sh | bash
```

The build writes the release version into the copy it publishes; run from a checkout, the script asks GitHub for the latest release. It downloads `SHA256SUMS` and `SHA256SUMS.asc`, checks their signature with `gpgv` and the key it embeds, then checks each download against its line. Without `gpgv` it says so and relies on the checksums alone. Then:

- **Debian and Ubuntu**: installs the DEB with `apt`, because only `/usr/bin/oxyde` gets the AppArmor profile that lets bubblewrap confine decoders.
- **Everything else**, Fedora Silverblue and other image-based systems included: places `oxyde-<version>-x86_64-linux` in `~/.local/lib/oxyde/<version>` (executable and workers in `bin/`), switches the `current` link atomically, links `~/.local/bin/oxyde`, and writes the desktop entry (marked `X-Oxyde-Install=user`), icons and AppStream metadata under `~/.local/share`. Nothing needs root or a reboot. The replaced version is kept for an Oxyde still running from it; older ones are removed. No `data` folder is created next to the executable, which would make Oxyde portable.

It then probes the runtime dependencies listed below and, when some are missing, prints the feature each one carries and the exact `dnf`, `rpm-ostree`, `pacman`, `zypper` or `apt-get` command, and offers to run it. Codecs are left to the user.

Options: `--version X.Y.Z`, `--archive FILE|FOLDER` (offline, or `./install.sh` from an extracted archive), `--set-default` (default application for folders through `xdg-mime`), `--yes`, `--uninstall` (settings are kept), `--update` (what Oxyde runs, below). Running it again also updates. Concurrent runs wait on a `flock` of `~/.local/lib/oxyde/.lock`.

## Updates

The flow is Windows' ([in-app updates](distribution.md#in-app-updates)); `platform/linux_update.rs` tells installations apart from the resolved executable:

| Executable | `Platform::installation` | Settings › About |
| --- | --- | --- |
| `~/.local/lib/oxyde/<version>/bin/oxyde`, beside a `current` link | `SelfUpdating`, entry `linux-x86_64` | installs |
| `/usr/bin/oxyde`, listed by dpkg | `Package`, entry `linux-x86_64-deb` | downloads the DEB in the browser |
| `/usr/bin/oxyde`, another package | `Package`, entry `linux-x86_64-rpm` | downloads the RPM in the browser |
| anything else, such as `target/` | `Unmanaged` | no check, no row |

`latest.json` carries the three Linux entries with the base64 of each file's `.asc`; Tauri clients ignore them. A release without a Linux entry is no update for a per-user installation, and a package without its entry gets the release page.

Installing downloads the archive with `curl`, or `wget`, the tools `install.sh` already needs (`platform/linux_download.rs`: HTTPS only, redirects included, `~/.curlrc` ignored, bounded size). Oxyde writes it to a temporary folder and checks its GPG signature there with `gpgv`, a private keyring holding only the compiled-in key and an empty home; the signing key's fingerprint must match too. Without `gpgv` (package `gnupg2` on Fedora, `gpgv` on Debian) the update fails, and `install.sh` lists it among recommended packages. Once the signature is verified, Oxyde copies the archive's own `install.sh` out of it and runs `install.sh --archive <file> --update` with `HOME` set to the home holding the running copy: no dependency check, no question. The new version's script thus places the new version, and those two options are a contract between releases. A detached `sh` then waits for Oxyde's process to end before starting `current/bin/oxyde`, so the new version reads the session saved on close. Oxyde closes as on Windows. The version it ran from stays on disk until the next update.

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
- **distribution**, `_linux-distribution.yml`, also run by `release.yml`: on `ubuntu-24.04`, builds the DEB and the archive from the same executables, installs the DEB, installs the archive twice for a user then removes it; in a `fedora:44` container, builds and installs the RPM. Signing happens only in the release job.

Rust is pinned to 1.99.1 and `cargo-about` verified by hash. Containers forbid bubblewrap, so they do not exercise confinement.
