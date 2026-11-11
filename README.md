# Oxyde

<p align="center">
  <img src="screenshots/win-dark-1920.webp" alt="Oxyde on Windows" width="100%" />
</p>

Oxyde is a native file manager built in **Rust**, with a **GPUI** interface and **Tokio** for background work. It combines flexible multipane browsing, an integrated viewer, advanced search, archives and tracked file operations in a customizable desktop workspace.

Oxyde runs on **Windows 10 and 11** and, since version 2.1, on **Linux** (x86-64, Wayland or X11). A few integrations differ between the two systems: see [Platform differences](#platform-differences).

<p align="center">
  <img src="screenshots/linux-light-1920.webp" alt="Oxyde on Linux" width="100%" />
</p>

## Features

### Workspace and browsing

- **Multipane workspace**: browse several locations in resizable panes, rearrange tabs and panes, and drag files between them. Double-click a separator to share two panes equally, or the sidebar's edge to fit it to its content. Tabs, layout and window geometry are restored between sessions.
- **Tabs**: duplicate tabs, open folders with a middle click, switch locations independently in each pane, and hover a tab while dragging files to switch to it.
- **List and grid views**: virtualized lists and thumbnail grids, natural filename sorting, date grouping, an optional checkbox column, type-ahead selection, and rectangle selection that scrolls the list when it reaches an edge.
- **Video thumbnails**: grid views and tooltips show video thumbnails, MKV, WMV, FLV and MPEG included, from the Windows shell or from a decoded frame.
- **Hidden files**: show hidden and system files on demand. On Linux, backup files and names listed in `.hidden` are hidden as in Nautilus.
- **Unix permissions**: an optional octal column on Linux, with its filter, and permissions in file tooltips.
- **Sidebar**: quick access shared with the system (Windows quick access, GTK bookmarks) and reordered by dragging, drives and volumes, OneDrive, network locations, the trash, and WSL distributions on Windows.
- **Folder sizes**: calculated in the background with Alt + Shift + S, faster with Turbo mode, and kept up to date while folders change.
- **Details**: [interface](docs/architecture/interface.md), [quick access](docs/architecture/quick-access.md), [folder watching](docs/architecture/watchers.md).

### Viewer and previews

- **Integrated Viewer**: follows the selection in a pane, or opens as a separate, resizable window.
- **Documents**: full-resolution images with their technical labels and wheel zoom, animated GIF and WebP, multipage PDFs, and text and source code with syntax highlighting and selection.
- **Markdown and Office**: Markdown rendered GitHub-style (task lists, tables, alerts, footnotes), and text from Office Open XML and OpenDocument files.
- **Folders and archives as a tree**: a read-only tree in the Viewer, with content size, archive size and compression ratio.
- **Audio and video player**: album artwork, track skipping, and technical labels (resolution, codecs, frame rate, sample rate, channels). When a codec is missing, the Viewer says which one.
- **Hover previews**: images, animations, documents and media straight from file tooltips. PowerToys Peek remains available as an optional Windows integration.
- **Details**: [previews](docs/architecture/previews.md), [Linux previews and sandbox](docs/architecture/linux-previews.md).

### Search

- **Advanced search**: recursive search by name with wildcards or regular expressions, content search (Office documents included), and date and size filters. Results appear while the search runs, and stopping keeps what was found.
- **Duplicate search**: compares files by name, size, content or combined criteria across several locations, with virtualized results for large sets.
- **Details**: [navigation and search](docs/architecture/search.md).

### File operations

- **Operations manager**: a dedicated window with progress, throughput graphs, pause and cancel, and per-item results.
- **Conflicts and Undo**: destination conflicts are resolved before transfers start, and Undo and Redo work for the session's supported operations.
- **Partial failures**: a deletion keeps going past items it cannot remove, marks them as failed, names the cause (file in use, access denied) and offers to retry them.
- **Turbo mode**: higher priority and extra parallelism, with storage-aware transfers, deletions and folder sizes, and parallel ZIP, 7z and Zstandard compression.
- **Recycle Bin and trash**: browse, restore and delete trashed items, and set their limits. On Linux, Oxyde uses the freedesktop trash, with GNOME and KDE settings.
- **Properties**: sections for general details, permissions and owners (Unix permissions and MIME types on Linux), and folder contents.
- **Open With**: the system's applications, with a search field and a default application switch on Linux.
- **Terminal**: open a folder in a terminal with Ctrl + . on Linux.
- **Details**: [operations and Turbo](docs/architecture/operations.md), [Undo and Redo](docs/architecture/undo.md), [trash](docs/architecture/trash.md), [copies](docs/architecture/copies.md), [moves](docs/architecture/moves.md).

### Archives and disk images

- **Archive browsing**: browse, open and extract ZIP, 7z, RAR (multivolume included), TAR in all its compressions, and single compressed files.
- **Archive editing**: create and modify ZIP, 7z, TAR and TAR.ZST archives from the workspace.
- **Encrypted archives**: open and create password-protected ZIP and 7z archives, open protected RAR archives, and remember passwords for the session.
- **Disk images**: ISO9660 ISO/IMG images can be browsed directly. ISO and IMG images can be mounted and unmounted on both systems, and VHD and VHDX images on Windows.
- **Details**: [archives](docs/architecture/archives.md), [volumes](docs/architecture/volumes.md).

### Volumes and network

- **Windows volumes**: mount and eject drives, and see which applications block a USB ejection.
- **Linux volumes**: through UDisks2, mount on click, unmount and eject, and see which processes keep a volume busy.
- **Network on Windows**: map and disconnect network drives.
- **Network on Linux**: SMB, SFTP, FTP and WebDAV shares through GVfs, with recent servers shared with Nautilus and passwords remembered until logout.
- **Details**: [volumes](docs/architecture/volumes.md), [network on Linux](docs/architecture/linux-network.md).

### System integration

- **Default file manager** (Preferences › System integration, off by default): folders opened by other applications, and their "Show in folder", open in Oxyde in a new tab of the running window, with the file selected. On Windows, Windows + E opens Oxyde too, while folders opened inside a File Explorer window stay there. Turning the option off gives folders back to the application they opened with before.
- **Open and Save dialogs** (off by default): applications that ask for a file to open or a place to save get an Oxyde window. On Linux this goes through the desktop portal (Firefox, Chrome, Flatpak applications). On Windows it is **experimental** and installer-only: applications that add their own options to the dialog, run as administrator or are 32-bit keep the Windows dialog, and security software may flag the registration it relies on (see [System integration](docs/architecture/system-integration.md#windows-dialogs-experimental)).
- **Windows menus**: the installed copy adds **Open in Oxyde** to the menus of folders while it is not the default file manager, and Oxyde's taskbar menu lists the pinned folders and **Run as administrator**.
- **Details**: [system integration](docs/architecture/system-integration.md).

### Security

- **Confined workers**: decoders and archive codecs run in separate worker processes with resource limits, and their responses are checked. Windows uses AppContainer; Linux uses bubblewrap and seccomp, or Landlock where bubblewrap cannot run.
- **Safer file operations**: file identities and archive paths are checked, and outputs are published only once complete.
- **Signed releases**: the Windows updater is signed with minisign, and Linux packages with Oxyde's own GPG key.
- **Details**: the [security documentation](docs/security/status.md) covers the scope and remaining limits.

### Personalization

- **Themes**: thirteen built-in themes, including GitHub, Ayu, Monokai and Solarized, plus your own themes built from six colors.
- **Theme files**: user themes are saved apart from preferences, so a preferences reset keeps them.
- **Color vision**: a setting that keeps file colors distinct for red-green and blue-yellow color vision.
- **Interface**: adjustable UI scale, window buttons in the style of the Linux desktop (GNOME or KDE), and five languages: English, French, Spanish, German and Italian.
- **Details**: [interface](docs/architecture/interface.md#themes-and-dimensions).

## Installation

Downloads are on the [releases page](https://github.com/Carlton2001/Oxyde_FM/releases/latest). Every release lists its files and their SHA-256 in `SHA256SUMS`, signed in `SHA256SUMS.asc`.

### Windows

Windows 10 or 11, x64.

| Edition | File | Updates |
| --- | --- | --- |
| **Installer** (recommended) | `Oxyde_FM_<version>_x64-setup.exe` | Automatic: Oxyde announces new versions and installs them from Preferences › About |
| **Portable** | `Oxyde_FM-<version>-x64-portable.zip` | By hand: download the new ZIP from the releases page |

- **Installer**:
  - installs for the current user into `%LOCALAPPDATA%\Programs\Oxyde_FM`, without administrator rights;
  - adds a Start menu shortcut, an optional desktop shortcut and an uninstall entry;
  - adds **Open in Oxyde** to the menus of folders and changes no file association: Oxyde takes over folders from File Explorer only when **Default file manager** is turned on.
- **Portable edition**: extract the ZIP and run `Oxyde_FM/Oxyde_FM.exe`. Preferences, session, logs and caches stay in the `data` folder next to it, so the whole folder can move to a USB stick. Keep the worker executables and `oxyde-folder-launcher.exe` next to `Oxyde_FM.exe`. The Open and Save dialogs and the Open in Oxyde menu need the installer.
- **Executables** are not code-signed: Windows SmartScreen may ask for confirmation on first launch.

### Linux

x86-64 only, glibc 2.39 or newer: Fedora 44+, Ubuntu 24.04+, Debian 13+, and other distributions of the same generation. Wayland is recommended; X11 works with the limits listed below.

#### Recommended: the install script

```bash
curl -fsSL https://github.com/Carlton2001/Oxyde_FM/releases/latest/download/install.sh | bash
```

The script checks the release signature with `gpgv` (or the checksums alone when `gpgv` is missing), then picks the right format for your system:

| Distribution | What the script installs | Updates |
| --- | --- | --- |
| **Ubuntu, Debian** and derivatives | The DEB package, through `apt` (asks for your password) | Oxyde announces new versions and downloads the DEB |
| **Fedora Workstation**, **Fedora Silverblue / Kinoite** and other image-based systems, **Arch**, **openSUSE** and others | A per-user installation in `~/.local`: no root, no reboot | Automatic: Oxyde downloads, verifies and installs new versions from Preferences › About |

- **Why the DEB on Debian and Ubuntu**: it installs the AppArmor profile Ubuntu needs to let Oxyde confine its decoders.
- **Missing dependencies**: the script lists the missing runtime dependencies with the feature each one carries, shows the exact `dnf`, `rpm-ostree`, `apt-get`, `pacman` or `zypper` command, and offers to run it. On image-based systems such as Silverblue, Oxyde itself needs no system change: only these components are layered with `rpm-ostree`, and become available after a restart.

**Update or uninstall**: run the same command again. A menu, driven with the arrow keys, says which version is installed and offers to update, reinstall or uninstall it; uninstalling asks whether to keep your settings.

**Preferences › System integration** makes Oxyde the default file manager and the Open and Save dialogs of other applications (see [System integration](#system-integration)). The dock's Files icon stays the desktop's.

Options, for scripts (with an option, no menu is shown):

| Option | Effect |
| --- | --- |
| `--version X.Y.Z` | Installs a given version |
| `--archive FILE` | Installs a downloaded archive, offline |
| `--set-default` | Makes Oxyde the default application for folders, as its option in Preferences does |
| `--yes` | Installs missing dependencies without asking |
| `--uninstall` | Removes the per-user installation, its cache and the associations naming it (folders and Open dialogs included), then asks whether to delete your settings |
| `--purge` | With `--uninstall`, deletes your settings without asking |

To pass options through the pipe, add `-s --` after `bash`:

```bash
curl -fsSL https://github.com/Carlton2001/Oxyde_FM/releases/latest/download/install.sh | bash -s -- --version 2.1.0
```

Running the `install.sh` found in an extracted archive installs that archive.

#### Other installation methods

| Method | Command | When to use it |
| --- | --- | --- |
| **RPM** (Fedora 44+) | `sudo dnf install ./oxyde-<version>-1.fc44.x86_64.rpm` | You want Oxyde in `/usr`, managed by `dnf`. Oxyde announces new versions; install them with `dnf`. |
| **RPM on Silverblue** | `rpm-ostree install ./oxyde-<version>-1.fc44.x86_64.rpm` | Possible, but needs a reboot for every version: the per-user installation is better suited. |
| **DEB** (Ubuntu 24.04+, Debian 13+) | `sudo apt install ./oxyde_<version>-1_amd64.deb` | What the script does on these systems. |
| **Archive** | `tar -xzf oxyde-<version>-x86_64-linux.tar.gz && ./oxyde-<version>-x86_64-linux/install.sh` | Offline installation, any distribution. |

Install Oxyde one way only: a system package and a per-user installation side by side give two copies, and the per-user one takes over the menu entry and the `oxyde` command. Settings in `~/.config/Oxyde` are shared, so switching methods loses nothing.

#### Verifying a download by hand

```bash
gpg --import oxyde-packages.asc   # packaging/linux/signing/oxyde-packages.asc
gpg --verify SHA256SUMS.asc SHA256SUMS && sha256sum --ignore-missing -c SHA256SUMS
rpm --import oxyde-packages.asc && rpm -K oxyde-<version>-1.fc44.x86_64.rpm   # RPM signature
```

Key fingerprint: `F9D7 6B9E 3BED D5EA CC9C  FE7D AC2E 961F 3202 9706`.

#### Runtime dependencies

The packages and the install script take care of these; the signature check is only proposed by the install script. Recommended dependencies only remove their own feature when missing.

| Role | Fedora | Debian / Ubuntu | Level |
| --- | --- | --- | --- |
| Decoder confinement | `bubblewrap`, `libseccomp` | `bubblewrap`, `libseccomp2` | required |
| GIO, trash settings | `glib2` | `libglib2.0-0t64`, `libglib2.0-bin` | required |
| Rendering | `vulkan-loader`, `mesa-vulkan-drivers` | `libvulkan1`, `mesa-vulkan-drivers` | required |
| Windows and fonts | `libwayland-client`, `fontconfig`, a sans font | `libwayland-client0`, `libfontconfig1`, a font | required |
| Opening files elsewhere | `xdg-utils` | `xdg-utils` | required |
| PDF | `poppler-glib` | `libpoppler-glib8t64` | recommended |
| Audio and video | `gstreamer1-plugins-base`, `-good` | `gstreamer1.0-plugins-base`, `-good` | recommended |
| Network shares | `gvfs`, `gvfs-fuse`, `gvfs-smb` | `gvfs`, `gvfs-fuse`, `gvfs-backends` | recommended |
| Volumes | `udisks2` | `udisks2` | recommended |
| Signature check | `gnupg2` | `gpgv` | recommended, required for updates |

**Video codecs** are left to you:
- **Fedora**: has no H.264 or HEVC decoder of its own. Enable [RPM Fusion](https://rpmfusion.org/Configuration), then install `libavcodec-freeworld`, with `dnf`, or `rpm-ostree` on Silverblue.
- **Ubuntu and Debian**: install `gstreamer1.0-libav`.

When a codec is missing, the Viewer names it.

### Platform differences

| Feature | Windows | Linux |
| --- | --- | --- |
| Automatic updates | Installer | Per-user installation |
| Disk image mounting | ISO, IMG, VHD, VHDX | ISO, IMG |
| Network drive mapping | Yes | GVfs shares instead |
| Open and Save dialogs | Experimental, installer only | Through the desktop portal |
| Open in Oxyde menu, taskbar menu | Yes | No |
| PowerToys Peek, Shell "More options" menu | Yes | No |
| Dragging files to other applications | Yes | Wayland only |
| Confinement | AppContainer | bubblewrap + seccomp, or Landlock |

## Building from source

### Windows

- [Rust](https://www.rust-lang.org/tools/install) with the MSVC x64 toolchain.
- Visual Studio Build Tools with the Visual C++ build tools and the Windows SDK.

```powershell
cargo build --bins
cargo run
```

Build all binaries before launching: `cargo run` alone does not rebuild the worker executables. The library shown in place of the Open and Save dialogs is a package of its own:

```powershell
cargo build --manifest-path shell/oxyde-file-dialog/Cargo.toml --target-dir target
```

Add `--offline` once dependencies are cached. The portable ZIP and the installer are built as described in the [Windows distribution guide](docs/guides/distribution.md).

### Linux

On Fedora:

```bash
sudo dnf install gcc gcc-c++ make pkgconf-pkg-config fontconfig-devel freetype-devel \
  libxkbcommon-devel libxkbcommon-x11-devel libxcb-devel wayland-devel vulkan-loader
cargo build --bins
cargo run
```

Packages are built with `bash scripts/build-linux-packages.sh --rpm | --deb | --tar`: see the [Linux distribution guide](docs/guides/linux-distribution.md) and the [Linux development guide](docs/guides/linux-development.md).

## Technology stack

- **Application and UI**: Rust and [GPUI](https://github.com/zed-industries/zed/tree/main/crates/gpui), with GPU-accelerated native rendering.
- **Background work**: Tokio, confined worker processes, and asynchronous folder watching.
- **System integration**: a shared `Platform` abstraction with Windows and Linux implementations (Win32, Shell and COM on Windows; GIO, GVfs, UDisks2, GStreamer, poppler and the desktop portal on Linux).
- **Localization**: Fluent catalogs for five languages.
- **Icons**: official [Lucide](https://lucide.dev/) SVG assets.

## Keyboard shortcuts

These shortcuts apply while the file pane has keyboard focus.

| Navigation and focus | Shortcut |
| --- | --- |
| Switch between panes | Tab / Shift + Tab |
| Go back, or up if there is no history | Backspace |
| Go to the parent directory | Alt + Up |
| Open the selected folder or file | Enter |
| Select the file starting with a letter | Type its first letters |
| Open quick search | Ctrl + F |
| Add a pane | Ctrl + N |
| Open a folder in a new tab | Middle click |
| Open a terminal here (Linux) | Ctrl + . |

| File operations | Shortcut |
| --- | --- |
| Copy / Cut / Paste | Ctrl + C / X / V |
| Copy full paths | Ctrl + Shift + C |
| Copy names | Ctrl + Alt + C |
| Create a folder | Ctrl + Shift + N |
| Move to the trash | Delete |
| Delete permanently | Shift + Delete |
| Rename | F2 |
| Select all / Deselect all | Ctrl + A / Ctrl + Shift + A |
| Show properties | Alt + Enter |
| Open the context menu | Menu or Shift + F10 |
| Calculate folder sizes | Alt + Shift + S |
| Undo / Redo supported operations | Ctrl + Z / Ctrl + Y or Ctrl + Shift + Z |

| Tabs and view | Shortcut |
| --- | --- |
| Next tab | Ctrl + Tab or Ctrl + PageDown |
| Previous tab | Ctrl + Shift + Tab or Ctrl + PageUp |
| Refresh | F5 or Ctrl + R |
| Increase / Decrease UI scale | Ctrl + Plus / Minus |
| Reset UI scale | Ctrl + 0 |

In the media Viewer, Space toggles playback, and Left / Right navigate images or videos of the same type. PowerToys Peek uses its configured activation shortcut; Space is available when enabled in Peek.

## Note on contributions

Oxyde is a personal project developed for educational purposes and personal use. To keep creative control and a manageable pace, **I am not currently accepting pull requests or external contributions** to this repository.

Since the project is licensed under the **GNU GPL v3**, you are welcome to fork it, experiment with the code and build your own versions.

## Documentation

Technical documentation lives in [docs/](docs/README.md), with guides for [development](docs/guides/development.md), [Windows distribution](docs/guides/distribution.md), [Linux distribution](docs/guides/linux-distribution.md) and [security protections and limits](docs/security/status.md).

## License

This project is licensed under the **GNU GPL v3**. See the [LICENSE](LICENSE) file for details.

Developed with passion by **Carlton2001**.
