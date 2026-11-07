# Distributing Oxyde on Linux

RPM and DEB packages and a per-user archive are built and tested in CI. The Windows side has [its own guide](distribution.md). Packaging sources: [`packaging/`](../../packaging/README.md).

## Build

```bash
bash scripts/build-linux-packages.sh --rpm   # on Fedora
bash scripts/build-linux-packages.sh --deb   # anywhere: podman outside Ubuntu 24.04
bash scripts/build-linux-packages.sh --tar   # per-user archive, same image as the DEB
```

Files and their SHA-256 land in `target/linux-packages/output`. Beyond the [development dependencies](linux-development.md#setup-on-fedora-workstation), Fedora needs `rpm-build`, `desktop-file-utils`, `appstream`, `python3` and `cargo-about` 0.9.2 (`cargo install --locked cargo-about --version 0.9.2 --features cli`: without `cli`, no command is installed). The DEB and the archive build in the image from `packaging/linux/debian/Containerfile`, with its own `target/ubuntu-24.04` folder; `lintian` stops the build on an error, `rpmlint` is informational. `--skip-build` reuses existing release executables. `--offline` keeps the previous build's notices: `cargo-about` reads crates for every target, macOS included, which a Linux cache does not hold.

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

Options: `--version X.Y.Z`, `--archive FILE|FOLDER` (offline, or `./install.sh` from an extracted archive), `--set-default` (default application for folders through `xdg-mime`), `--yes`, `--uninstall` (also removes `~/.cache/Oxyde`, kept while Oxyde runs, and Oxyde from every `mimeapps.list` and from the portal's file chooser unless a system package still provides its entry; then asks, `[y/N]`, whether to delete `~/.config/Oxyde`, which `--purge` does without asking; never while Oxyde runs or a system package shares the settings), `--update` (what Oxyde runs, below). Running it again also updates. Concurrent runs wait on a `flock` of `~/.local/lib/oxyde/.lock`.

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

`Exec=oxyde %U` and `MimeType=inode/directory` put Oxyde in "Open With" without making it the default handler. Received locations are parsed in `platform/linux_startup.rs`: a file stands for its folder, `file://` URIs are decoded, other schemes stay URIs for GIO.

**One instance per session** (`platform/linux_instance.rs`, `ui/app/instance.rs`). Before any window, Oxyde asks the session bus for `io.github.Carlton2001.Oxyde` (`.Devel` for debug builds, so a `cargo run` never talks to an installed copy) with `DoNotQueue`. The first process keeps it and serves `io.github.Carlton2001.Oxyde1.Open(as locations, s token)` at `/io/github/Carlton2001/Oxyde`; it restores the saved session and adds each received location as a new tab. A later launch calls `Open` with `XDG_ACTIVATION_TOKEN` (or `DESKTOP_STARTUP_ID`) and exits before GPUI starts; the running one adds the tabs and presents the token through `Window::activate_window_with_token`, a local GPUI patch (`vendor/gpui_linux/README.oxyde.md`), so Mutter and KWin bring it forward. Without a session bus, or if the running Oxyde does not answer within five seconds, the process runs alone as before: it shows only the received locations and leaves the saved session untouched.

**Default file manager** (`ui/app/folder_handler.rs`, GIO in `platform/linux_gio/applications.rs`). On, the Settings switch:

- makes Oxyde the default for `inode/directory` in the user's `mimeapps.list` and keeps the application it replaced in `folder_handler_before`; off, it gives folders back to that application, or resets the user's folder associations when it is gone;
- writes `$XDG_DATA_HOME/dbus-1/services/io.github.Carlton2001.Oxyde.FileManager1.service`, whose `Exec` is the desktop entry's program, asks the bus to reload, and requests `org.freedesktop.FileManager1` with `AllowReplacement`. User service files take precedence over `/usr/share`, so the bus starts Oxyde rather than Nautilus; a program already holding the name keeps it, and Oxyde waits in the queue. Off, the file is removed and the name released.

At each start, a thread makes the service file and the name follow the folder default, which `install.sh --set-default` or another program may have changed. `ShowFolders` opens tabs, `ShowItems` opens one tab per folder with its items selected, `ShowItemProperties` opens Properties. The switch is shown only when GIO finds the desktop entry. `install.sh --uninstall` removes the service file.

Limits: another program's user service file for `org.freedesktop.FileManager1` (Strata writes one) competes with Oxyde's, and the bus keeps whichever it reads first; the dock's Files icon stays the desktop's; a non-UTF-8 name is ignored.

**Open and Save dialogs** (`platform/linux_portal.rs` and `linux_portal/`, `ui/app/file_chooser/`). Applications that ask through `xdg-desktop-portal` (Flatpak, Firefox, Chrome, recent Electron) get Oxyde as their file chooser, as Strata does. On, the Settings switch, off by default and shown only when the portal and a program to start exist:

- writes `$XDG_DATA_HOME/xdg-desktop-portal/portals/oxyde.portal` (`DBusName=org.freedesktop.impl.portal.desktop.oxyde`, `Interfaces=org.freedesktop.impl.portal.FileChooser;`) and `$XDG_DATA_HOME/dbus-1/services/org.freedesktop.impl.portal.desktop.oxyde.service`, whose `Exec` is the desktop entry's program with `--portal`; debug builds use `oxyde-devel` and `….oxyde.Devel`, and the build itself as program;
- puts `oxyde` first in the `org.freedesktop.impl.portal.FileChooser` line of the user's `$XDG_CONFIG_HOME/xdg-desktop-portal/<desktop>-portals.conf` (or the `portals.conf` they have), the portals it named staying after it as fallback; without such a line, it adds one, marked `# Added by Oxyde for its file chooser.`, with the `default` portals after `oxyde`; without a file of the user's, it writes a copy of the configuration the portal read, marked `# Written by Oxyde from <path>`. Links are written through. Off, removing `oxyde` gives back the line as it was, the marked line goes with its comment, and the copy goes when nothing else changed in it;
- asks the bus to reload, requests the name, and restarts `xdg-desktop-portal.service` through systemd, as the portal reads its files only at startup.

The running Oxyde serves `org.freedesktop.impl.portal.FileChooser` at `/org/freedesktop/portal/desktop` on its instance connection; at each start, while the configuration names it first, it writes the two files again if an update changed them and requests the name. When nothing serves the name, the bus starts `oxyde --portal`: that process becomes the session's Oxyde without a main window, which a later launch opens on its tabs; if an Oxyde already runs, it calls `ServeFileChooser` on it and exits, and the running one takes the name. `OpenFile`, `SaveFile` and `SaveFiles` export an `org.freedesktop.impl.portal.Request` at the call's handle; `Close` closes the window unanswered (response 2), Cancel, Esc or the window's close button answer 1, and a choice 0 with the `file://` URIs chosen and, when the application gave filters, the `current_filter` shown, and its `choices` answered. The window lists real folders off the UI thread, filtered and sorted there too. Its pieces are copies of the main window's, adapted to choosing; what they computed word for word (crumb motion, type-ahead, arrows, column measurement) is the main window's own code: the sidebar (resizable; its places, drives and the volumes to mount on a click follow the main window's), the navigation bar (Back, Forward, Up, crumbs with their folder menus, the path edited on a double click or in the blank area, also with Ctrl+L or `/` and `~` in the list; a typed folder opens, a typed file is chosen), the quick search control as a name filter (Ctrl+F; its arrows move the selection), the measured Name, Type, Size, Permissions (when the main window shows it) and Modified columns sorted from their headers, the grid with the main window's thumbnails, the status bar with its view switch, tooltips under the main window's rules, hidden files (Ctrl+H), Refresh (F5), Backspace and type-ahead selection. With `multiple`, Ctrl, Shift and Ctrl+Shift presses, Shift with the arrows and Ctrl+A select several entries as in the main window; Enter opens a single entry and chooses several; the button chooses the files selected, or opens a single folder selected. With `directory`, only folders are listed, a typed file is refused, and the button ("Select") chooses the folders selected, else the folder open. `filters` (`a(sa(us))`: shell patterns that keep case, as GTK's, and MIME types matched through the shared MIME database's aliases and subclasses, `image/*` by media) are offered in a menu at the start of the footer; `current_filter` is shown first, alone when no list comes with it, added when the list lacks it; folders are always listed. Files' MIME types are read with the listing, only when a filter names one. `current_folder` (`ay`) opens first when it is a folder. Its view, sort, sidebar width and last folder are kept in the preferences as `file_chooser_options`, and the next window starts there unless the application suggests a folder. `SaveFile` adds a name field at the start of the footer, which has the keyboard first with the stem of `current_name` (or of `current_file`, the file saved again, whose folder then opens first) selected; a file pressed or reached in the list gives its name, and a double press or Enter on it saves over it. The name is typed relative to the folder open, `~`, `..` and subfolders included, as is a path typed in the navigation bar: a folder opens and the name is cleared, an existing file (or link) is replaced once a prompt confirms it, a free name in an existing folder is chosen, and anything else shakes the field in red. Without a name, the button ("Save") opens the single folder selected. The prompt is a `WindowKind::Dialog` window parented to the chooser, which takes no input meanwhile (Cancel first, Tab, Enter or Space, Esc); type filters apply to the list as when opening, and changing them leaves the name's extension alone, as GTK 4 does. `SaveFiles` chooses a folder as `directory` does; its `files` (`aay`, only their last component kept) go in the folder chosen, those already there replaced once the same prompt, listing the first three, confirms it. `choices` (`a(ssa(ss)s)`) sit in the footer after the type filter: a checkbox for those without answers (`"true"` or `"false"`), a label and a menu otherwise, an answer given first that is not offered becoming the first one; they go back as `choices` (`a(ss)`) with any answer, opening included. When Oxyde's interface cannot take a call any more, it goes to the next portal of the line (its `.portal` file gives the bus name), on a connection without Oxyde's five-second limit, `Close` included. Oxyde ends with its last window only once no call waits for an answer, and waits up to two seconds at exit for the last replies; an Oxyde started for the portal that never gets a window ends after 30 seconds. `install.sh --uninstall` removes the two files and, when no system copy remains, takes `oxyde` out of the line the same way, then restarts the portal.

Remaining steps ([plan](../plans/portal-file-chooser.md)):

4. Parenting to the caller's window: `xdg-foreign` on Wayland needs a GPUI patch; without it the picker is an independent, non-modal window.
5. Robustness: once chosen, every portal dialog depends on Oxyde, and the `gnome;gtk` fallback applies only when the backend is missing, not when it fails. Timeouts, cancellation, tests with Firefox, Chrome and a Flatpak.

Applications that use their toolkit's own dialog without the portal keep it.

## Continuous integration

`.github/workflows/linux.yml`, on `main` and its PRs:

- **tests** on `ubuntu-24.04`: warning-free compile, full suite, then `preview_jobs` separately, whose 2 s timeouts fail when the whole suite loads the machine. The AppArmor restriction is lifted so confinement tests run.
- **distribution**, `_linux-distribution.yml`, also run by `release.yml`: on `ubuntu-24.04`, builds the DEB and the archive from the same executables, installs the DEB, installs the archive twice for a user then removes it; in a `fedora:44` container, builds and installs the RPM. Signing happens only in the release job.

Rust is pinned to 1.99.1 and `cargo-about` verified by hash. Containers forbid bubblewrap, so they do not exercise confinement.
