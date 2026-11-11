# Distributing Oxyde on Windows

To build and run for development: [development guide](development.md). Linux: [Linux distribution](linux-distribution.md).

## Logo, icons and cursors

The source logo is `assets/icons/logo_oxyde.svg`. About and the title bar show `logo_about.png` (144 px) and `logo_title.png` (40 px), rendered for 2x screens: GPUI downscales in a single pass and aliases oversized sources. After changing the SVG, regenerate before rebuilding:

```powershell
./scripts/distribution-icon.ps1
```

It uses GPUI's SVG engine to generate `assets/icons/logo.png` for native icons and `assets/icons/oxyde.ico` (16–256 px) for the executable, Explorer, taskbar and installer. Both are kept in Git. Worker icons are independent.

The Viewer cursors (`assets/cursors/search.cur`, `hand-grab.cur`, 32–96 px) come from the Lucide `search` and `hand-grab` SVGs, tinted white with unmodified paths, and are kept in Git. To regenerate, with Node.js and `sharp` (`npm install --no-save sharp`):

```powershell
node scripts/generate-viewer-cursors.cjs
```

## Building

Use Rust MSVC x64 and the Visual C++ Build Tools. Install the license generator once, from the official Windows x64 binary:

```powershell
./scripts/install-cargo-about.ps1
```

This installs `cargo-about` 0.9.2 into `$CARGO_HOME/bin` (or `~/.cargo/bin`), which must be on PATH. It checks pinned SHA-256 hashes of the archive before extraction and of the binary before running it, then its version. A binary with the expected hash is reused without network access. Building locally also works: `cargo install cargo-about --version 0.9.2 --locked --features cli`.

Then build:

```powershell
./scripts/build-distribution.ps1 -PortableOnly
```

The script also builds the dialog library (`shell/oxyde-file-dialog`), which goes into the installer only. Without `-PortableOnly` it builds the installer with Inno Setup 6.3+, from [`packaging/windows/oxyde.iss`](../../packaging/README.md). `-SkipBuild` reuses existing release executables; `-IsccPath` locates Inno Setup. The C runtime is statically linked (`.cargo/config.toml`): no Microsoft DLL ships, and the script refuses an executable that still imports one. Executables are not signed.

Each run regenerates crate licenses from `Cargo.lock` before compiling, even with `-SkipBuild`, and includes the report in both packages; a generation failure stops the build, and network access is needed for missing upstream notices. To generate them separately: [notices guide](../../licenses/dependencies/README.md). The script prints separate timings for licenses, Rust, Inno Setup and ZIP, and verifies required files in the ZIP.

## Portable version

Extract `target/distribution/output/Oxyde_FM-<version>-x64-portable.zip`, then run `Oxyde_FM/Oxyde_FM.exe`. Keep the three workers and `oxyde-folder-launcher.exe` next to the executable. The archive has no `oxyde_file_dialog.dll`: only an installed copy answers the dialogs of other applications. `licenses/syntaxes` and `licenses/dependencies` hold grammar, crate, GPUI/Zed and Lucide notices, in both distribution formats.

A `data` folder next to the executable turns on portable mode, whatever the working directory. It ships in the ZIP and holds:

- `config.json`: preferences and session state
- `themes.json`: custom themes, kept when preferences are reset or `config.json` is removed
- `oxyde.log`, possibly `oxyde.previous.log`: diagnostics
- `cache/icons`, `cache/thumbnails`: generated caches and their control files
- `temp`: temporary files and archive/worker workspaces

TEMP, TMP and TMPDIR are redirected to `data/temp` before threads start, and children inherit them. Installed configuration and Tauri data are neither imported nor modified. If `data` is unusable an error is shown: there is no silent fallback to AppData, so writable media is required.

To move the application, move the whole `Oxyde_FM` folder, `data` included; stored paths to user files are not rewritten. Removing `data` turns portable mode off at next start. Clearing the cache in settings keeps configuration, logs and session. No shortcut or uninstall entry is created.

This isolation covers files Oxyde manages, not traces left by Windows, external tools or file operations the user requests. Ephemeral AppContainer profiles are still used and removed as usual; protections are not reduced in portable mode.

## Installer

`Oxyde_FM_<version>_x64-setup.exe` installs per user into `%LOCALAPPDATA%\Programs\Oxyde_FM`, with a Start shortcut, an optional desktop shortcut and its own uninstall entry. Identity: `Oxyde.GPUI.Windows`. It deploys no `data` folder. Uninstalling removes `%LOCALAPPDATA%\Oxyde` (caches) and offers, except in silent mode, to remove `%APPDATA%\Oxyde` (preferences, session, logs). No file association or PATH change. Before removing files, it runs `Oxyde_FM.exe --restore-folder-handler`, which removes Oxyde from the menus of folders, gives folders back when this copy is their default and gives the Open and Save dialogs back to Windows (see below).

Run with `/UPDATE` (by the Tauri or GPUI updater), it skips its pages then restarts Oxyde. NSIS options added by the Tauri updater (`/P /R /ARGS`) are ignored.

## Default file manager

Off by default. The **System integration** submenu of the settings (`ui/components/settings/integration.rs`, `ui/app/folder_handler.rs`) lists **Default file manager**; the registry work is in `platform/windows_folder_handler.rs`. Everything is written for the user under `HKCU\Software\Classes`, without elevation, and the Shell is told with `SHChangeNotify(SHCNE_ASSOCCHANGED)`, so the change applies to the next folder opened. The commands name `oxyde-folder-launcher.exe`, shipped next to the main executable; the option is not listed when it is missing, nor in a portable copy: deleted or moved, that copy would leave folders to a launcher that is gone, with nothing left to give them back. A portable copy that already is the default still lists the option, to turn it off. Checked, the option:

- sets the default verb of `Directory` and `Drive` to `open` (the system's is `none`, which leaves the choice to `Folder`) and their `shell\open\command` to `"<launcher>" "%V"`, with an empty `DelegateExecute`;
- sets `CLSID\{52205fd8-5dfb-447d-801a-d0b52f2e83e1}\shell\opennewwindow\command`, the command behind Win+E, to the launcher, with an empty `DelegateExecute`;
- keeps the user's values it replaced, as JSON in `folder_handler_before`. Off, they are written back, values that did not exist are removed and keys left empty are deleted, which gives the system's values back. Kept values are not written back when a command among them runs a program that is no longer there, such as a file manager uninstalled since: the system's values are given back instead.

`Folder` is not touched: Control Panel, the Recycle Bin and the other namespace folders stay Explorer's. The option reads as checked when the merged `HKCR\Directory` names this copy's launcher, so a copy moved elsewhere reads as unchecked and can be chosen again; the values kept at the first change are then reused. Files (files-community) registers a launcher the same way, on `Folder`.

**The launcher** (`src/bin/oxyde-folder-launcher.rs`, `platform/windows_folder_launcher.rs`) is what Explorer starts for every folder opened, its own windows included: a windowed program of about 1 MB, without console, interface or runtime, with the workers' icon. In order, it leaves the folder to the Explorer tab it came from, hands it to the running Oxyde over the instance pipe, or starts `Oxyde_FM.exe` (`oxyde.exe` in a Cargo build) on it; unless Explorer kept the folder, it then stays up to ten seconds for a file to show in it (below). Without a folder (Win+E), the running Oxyde comes forward or one starts.

**Explorer keeps its own windows** (`platform/windows_explorer_browse.rs`). With the verb replaced, Explorer runs it for a folder opened in its own list instead of browsing in place. When Explorer is the launcher's parent process, the launcher therefore looks for the tab the folder came from and asks it to browse there (`IShellWindows`, `IShellBrowser::BrowseObject`). Opening a folder in a list selects it, so the tab is an active one where the folder is selected, itself or as the target of a selected shortcut: the foreground window's, else the only one. Selected with other items, the folder gets an Explorer window of its own (`explorer.exe <folder>`, which does not go through the verb), as Explorer gives each folder of a selection. From the desktop, the taskbar or another program, the folder is Oxyde's, as when Explorer does not answer; so is one Explorer opens for the Run dialog, the Start menu or a jump list, selected in no tab, whatever Explorer window is in the foreground.

**Received folders** (`platform/windows_startup.rs`): arguments that are existing folders open as tabs, a file stands for its folder, `"C:\"` (received as `C:"`) is the drive's root; switches and Shell names (`::{CLSID}`, `shell:`) are ignored.

**One instance per session** (`platform/windows_instance.rs`, `ui/app/instance.rs`), as on Linux. The first process owns the only instance of a named pipe (`\\.\pipe\Oxyde.Instance.<Release|Devel>[.Elevated].<session>.<hash of the storage folder>`), so an installed copy, a portable copy and a debug build keep to themselves. The pipe is created as the user's own, writable by the user's processes at medium integrity, whatever rights Oxyde runs with. An Oxyde run as administrator owns the `.Elevated` name and does not give way to one that is not; the launcher tries the name of its own rights, then the other, so a folder opens in the Oxyde that runs. A later launch, or the launcher, checks that the pipe's owner is its own account, lets the owner come forward with `AllowSetForegroundWindow`, writes its locations and ends; the running Oxyde adds a tab for each and activates its window. Without locations, the window only comes forward. When the pipe cannot be reached, a launch of Oxyde runs alone and shows only what it received. As it starts to quit, Oxyde closes the pipe (`Platform::release_instance`, from the GPUI quit hook): its last saves can take seconds, during which a folder handed over would be taken and never shown; the next launch finds no pipe and starts an Oxyde of its own.

**Show in folder** (`platform/windows_shell_window.rs`). A program that shows a file in its folder calls `SHOpenFolderAndSelectItems`: the Shell opens the folder, which starts the launcher, then looks in `IShellWindows` for the window showing that folder and asks its view to select the file. As Files' launcher does, the launcher registers a window nobody sees as showing the folder (`RegisterPending`, `Register`, `OnNavigate`), an `IWebBrowserApp` whose `IShellView::SelectItem` only notes the items. The Shell matches a window by the identifier list of its folder, compared as written, and a folder of the user's profile has two: through the drive, as `ILCreateFromPath` writes it (Firefox, Zed), and through the known folder it is in, as `SHParseDisplayName` does (Explorer, Task Manager). The launcher therefore registers one window for each; under the other list a caller finds nothing, waits twenty seconds and gives up with `E_ABORT`. The folder is handed to Oxyde at once, without waiting for a selection; the launcher then answers the Shell for ten seconds at most, or until 20 ms after the last item, and hands the items over the pipe (`oxyde-select 1`, where folders are `oxyde-open 1`), waiting up to twenty seconds for an Oxyde it just started. Oxyde selects them in the active tab when it shows their folder, as it does after the folder was just opened, and in a new tab otherwise. A second request for the same folder within those seconds finds the launcher's window and selects through it. `the_file_a_program_shows_in_the_folder_is_the_selection` checks the exchange with the real Shell, for a caller of each kind; it is ignored by default, as it needs a desktop session.

Limits: a program that starts `explorer.exe` itself (`explorer.exe /select,<file>`, as NanaZip appears to) opens Explorer, which no association can prevent. The Open and Save dialogs are a separate, experimental option (below).

## Open and Save dialogs (experimental)

Off by default. The **System integration** submenu lists **Open and Save dialogs (experimental)** (`ui/app/file_chooser/setting.rs`, `Platform::chooses_files`); checked, applications that ask for a file get Oxyde's file chooser (`ui/app/file_chooser.rs`, the window the Linux portal uses) in place of the Windows dialog. Four pieces take part:

- **The library** `oxyde_file_dialog.dll` (`shell/oxyde-file-dialog`, a package of its own, about 300 KB, C runtime linked statically) is what applications load. It serves the two classes of the common item dialog, `CLSID_FileOpenDialog` and `CLSID_FileSaveDialog`, and gives each application the system's own dialog, created straight from `System32\comdlg32.dll`, inside a wrapper. Everything the application sets is passed on to the system's dialog, which stays ready to be shown. `Show` asks Oxyde for a plain dialog and shows the system's dialog in every other case: the application added a control Oxyde has nothing like (a menu, a push button, an edit box or an open drop-down), used `SetFilter`, `SetNavigationRoot` or `FOS_ALLNONSTORAGEITEMS`, it took from the system's dialog an interface the wrapper does not have (the undocumented `IFileDialogPrivate` of the older `GetOpenFileName` path, for instance), the application is Oxyde itself, Oxyde cannot be reached or shows no window within fifteen seconds, or anything fails. The application's combo boxes and radio button lists, visible and enabled, become menus in Oxyde's footer and its check buttons checkboxes, labeled by their own label, their visual group's or the text before them; the answers are set back on the system's dialog, whose listeners are told of each change, before `OnFileOk`. While Oxyde is shown, the application's window is disabled and its messages keep being dispatched. The listeners' `OnFileOk` is called on the choice, and a refused choice is asked again. A shortcut chosen to be opened gives its target unless `FOS_NODEREFERENCELINKS` is set.
- **The launcher** (`platform/windows_file_dialog/relay.rs`): the library holds no protocol. It runs `oxyde-folder-launcher.exe --choose` with the request on its standard input, inheriting those two pipes and nothing else, and reads the answer on its standard output. The launcher creates a pipe for the answer (`\\.\pipe\Oxyde.Dialog.<pid>.<random>`, the user's own, as the instance pipe), hands its name and the request to the running Oxyde (`oxyde-choose 1` on the instance pipe), or starts `Oxyde_FM.exe --file-chooser`, which opens no main window and ends with the chooser. Ending with code 2 means "show the system's dialog". The library ends the launcher when the application calls `Close`.
- **Oxyde** (`platform/windows_file_dialog.rs`) turns the request into the `FileChooserRequest` of the portal, answers on the launcher's pipe (`shown`, then `ok` with the paths and the filter, or `cancel`) and closes the window unanswered when the launcher's process ends. The chooser is owned by the application's window, adds the extension of the shown filter (else the default one) to a name saved without one, and matches filters without regard to case.
- **The registration** (`platform/windows_file_dialog/registration.rs`): `InprocServer32` of the two classes under `HKCU\Software\Classes\CLSID`, pointing to the library, `ThreadingModel` `Apartment` as the system's. Unchecking removes the keys, which restores the machine's registration. Applications find the change at their next dialog; one that already loaded the library keeps it loaded until it ends.

Not covered, by design of Windows: applications run as administrator (COM ignores the user's classes for them), 32-bit applications (they read `Software\Classes\WOW6432Node`, where nothing is written, and would need a 32-bit library), packaged applications that pick files through the system's broker, and applications that draw their own dialog. An Oxyde run as administrator answers the applications of the user all the same.

**A registration that points nowhere would leave every application without a dialog**, since COM does not fall back to the machine's class. Hence: the option is offered only where the library is, next to the executable, and not in a portable copy, whose archive does not hold the library; the uninstaller withdraws the registration (`--restore-folder-handler`) before removing files; and Oxyde removes a registration to a copy of its library that is gone each time it reads the option. By hand: delete `HKCU\Software\Classes\CLSID\{DC1C5A9C-E88A-4DDE-A5A1-60F82A20AEF7}` and `{C0B4E2F3-BA21-4773-8DBA-335EC946EB8B}`.

**Updates.** A library loaded by a running application cannot be replaced, but it can be renamed: the Setup sets the old copy aside as `oxyde_file_dialog.dll.old<date>`, puts a copy of it back at the registered path at once, so that a setup that stops leaves applications a library, then installs the new one; later setups, and the uninstaller, delete the copies no application holds any more. An uninstall while an application holds the library leaves that one file behind.

**Antivirus and security products.** Registering a class of the system for the user so that other programs load one's library is the technique known as COM hijacking (MITRE ATT&CK T1546.015), which malware uses to persist and to run inside other processes. A security product cannot tell the intent from the registry alone, so this option can:

- raise an alert, or be reverted or blocked, when the keys are written, more likely so since the executables are not signed;
- have the library quarantined, which leaves the registration pointing nowhere (see above: applications then have no dialog until the keys are removed or Oxyde's settings are opened);
- be refused by applications that only load libraries signed by Microsoft or by themselves (browsers' sandboxed processes, some protected or anti-cheat software): the class then fails to load there, and such an application may show no dialog while the option is on;
- make Oxyde's library part of every application that shows a dialog: a fault in it is a fault of that application. It is kept small for that reason, starts no thread but the one reading the launcher's answer, catches its own panics in `Show` and falls back to the system's dialog.

The library writes nothing and contacts nothing but the launcher next to it. It is off by default, labelled experimental, and removed entirely by unchecking the option. Diagnostics: set `OXYDE_FILE_DIALOG_LOG` to a file path in the environment of an application, and the library appends what it was asked, what Oxyde answered and which interface made it leave the dialog to Windows. Files (files-community) replaces the dialogs the same way.

Tests: `shell/oxyde-file-dialog/tests/dialog.rs` loads the library as an application does, with a fake launcher, so nothing shows on screen (Open with several files, Save under a free name, folders, cancellation, a refused choice, the interfaces); `platform/windows_file_dialog/tests` answer requests over real pipes. The system's dialog being shown instead, and the registration itself, are checked by hand.

## Menus of folders and of the taskbar

`ui/app/system_menus.rs` tells the platform what to list, at start and whenever the pinned folders or the default for folders change; the platform writes off the UI thread.

**Open in Oxyde** (`platform/windows_folder_handler/menu.rs`): a verb `Oxyde` of `Directory`, `Directory\Background` and `Drive` under `HKCU\Software\Classes`, labelled in the user's language, with Oxyde's icon, running `"Oxyde_FM.exe" "%V"`. Only the copy the Setup installed lists itself (its folder is the `InstallLocation` of `Uninstall\Oxyde.GPUI.Windows_is1`), and only while it is not the default for folders, when opening a folder already shows it in Oxyde; checking the option removes the verb, unchecking it writes it back, and the uninstaller removes it. No setting: it follows the default file manager option. A portable copy and a Cargo build never write it, so it can only be tried from a Setup. On Windows 11 a verb of the registry shows under **Show more options** (Shift+F10); the short menu only lists packaged applications, which Oxyde is not.

**Taskbar menu** (`platform/windows_jump_list.rs`): right-clicking Oxyde's taskbar button lists the pinned folders of the sidebar (Quick access), each opened as a tab, and a task **Run as administrator**. There is no list of recent folders, which Oxyde does not keep, and no new window, which panes replace. The task is a shortcut to `Oxyde_FM.exe --run-as-administrator`, which starts Oxyde again with the `runas` verb and ends: a shortcut cannot ask for elevation. The elevated Oxyde is an instance of its own (see above). When the user turned off recently opened items in Start and jump lists (`Start_TrackDocs` 0), Windows refuses an application's categories with `E_ACCESSDENIED`; the folders are then listed as tasks above a separator. Folders the user removed from the menu are left out of the next list, as Windows requires. `the_shell_accepts_the_menu` writes a list under a test identity and deletes it; it is ignored by default, as it needs a desktop session.

**Application identity.** The list belongs to an `AppUserModelID`. The process takes `Oxyde.GPUI.Windows`, the one of the Setup's shortcuts, before its first window (`Platform::identify_process`), so the button, a pinned shortcut and the menu are the same however Oxyde was started; a debug build takes `Oxyde.GPUI.Windows.Devel`. A taskbar pin made from a window of an earlier version, which had no identity of its own, no longer matches and has to be pinned again.

Transition from the Tauri edition: after installing, the Setup uninstalls any `Oxyde_FM` entry from publisher `Oxyde` (per-machine MSI with elevation, or per-user NSIS), then removes `%APPDATA%\com.oxyde.desktop` and `%LOCALAPPDATA%\com.oxyde.desktop`. This section of `oxyde.iss` is temporary.

## In-app updates

`systems/updates.rs` reads `latest.json` from the latest published release, compares it with `CARGO_PKG_VERSION`, and verifies the Setup's minisign signature with the updater's public key before writing it to a temporary folder. The download goes through `Platform::download` (WinHTTP, system proxy, HTTPS only, bounded size). `Platform::installation` names the manifest entry each copy reads: `windows-x86_64` here.

An automatic check runs 30 s after start. Each version is announced once (`last_notified_update` in `config.json`), never in portable mode. Settings › About checks and installs: it runs the Setup with `/UPDATE /WAITPID=<pid>` then closes Oxyde normally; the Setup waits for exit (60 s at most), replaces files and restarts Oxyde. In portable mode the button opens the releases page.

Linux follows the same flow for its per-user installation and only announces releases to system packages: see [Linux updates](linux-distribution.md#updates).

## Version and tag

```powershell
./scripts/bump-version.ps1 -Version <x.y.z>
```

From a clean `main`, the script checks that the version exceeds the current one and that tag `v<x.y.z>` exists neither locally nor on `origin`. It updates `Cargo.toml` and `Cargo.lock`, commits `Release v<x.y.z>` and creates the annotated tag, without pushing. Publish by hand: `git push origin main v<x.y.z>`. Existing `v1.x` tags belong to the Tauri edition.

## Automation

Checks are defined once in reusable workflows: `_tests.yml` (native suite), `_audit.yml` (RustSec audit) and `_distribution.yml` (portable tests, Setup, ZIP). The last publishes `Oxyde_FM-portable-x64` (ZIP and SHA-256) and `Oxyde_FM-setup-x64` (unsigned Setup), downloadable for 90 days from the run page.

On `main`, `tests.yml`, `security.yml` and `distribution.yml` call them according to changed files. `distribution.yml` runs automatically only when `Cargo.toml`, `Cargo.lock`, `.cargo/` or the distribution action change, that is when the dependency cache may be invalidated; it stays available manually. The audit also runs weekly. The commit created by `bump-version.ps1` (detected by `scripts/detect-release-commit.ps1`) triggers nothing on `main`: its tag runs every check. A manual run is never skipped.

The distribution workflow uses `.github/actions/setup-distribution/action.yml`: Rust **1.98.1** (pinned for this workflow only), a shared cache of compiled dependencies, and `cargo-about` **0.9.2**. Update the toolchain in that action after validating a new Rust version. The action uses a dedicated `RUSTUP_HOME` under `RUNNER_TOOL_CACHE` holding only the chosen toolchain, because `rust-cache` includes every installed toolchain in its key. When updating `cargo-about`, change the version and hashes in its install script, then the version in the shared cache key. Portable tests use `--release --target x86_64-pc-windows-msvc`, like the shipped executables, to share dependency builds.

The cache is saved only on the default branch; the release neither compiles nor restores a cache. A toolchain or dependency change or an eviction may force a rebuild on `main`. Distribution builds select only the application and its three workers.

Pushing a `v<x.y.z>` tag starts `.github/workflows/release.yml`. A single run checks the tag against `Cargo.toml`, then runs native tests, audit and distribution in parallel on the tagged commit. The `release` job needs all of them: a failure or cancellation prevents signing. It downloads the files built **by that same run**, signs the Setup with the updater's minisign key (secrets `TAURI_SIGNING_PRIVATE_KEY` and `_PASSWORD`, through the GitHub `release` environment) and checks that signature against the public key compiled into Oxyde (`scripts/check-updater-signature.sh`), so that a secret holding another key stops the release instead of publishing a Setup every updater refuses, signs the Linux files and `SHA256SUMS` with Oxyde's GPG key (secrets `OXYDE_GPG_PRIVATE_KEY` and `OXYDE_GPG_PASSPHRASE`, see [Linux signatures](linux-distribution.md#signatures)), then generates `latest.json` with `scripts/write-update-manifest.ps1`. All three Windows manifest keys point to the Inno Setup installer. Checksums of every file, Windows included, are in the release's single `SHA256SUMS`. The workflow creates a draft release; publishing it by hand makes it visible to updaters. If a check fails, re-run the failed jobs from the tag's run.

Artifacts are uploaded with `actions/upload-artifact` 7.0.2, pinned to an immutable commit.

## Tests

```powershell
./scripts/tests/detect-release-commit.tests.ps1
./scripts/tests/install-cargo-about.tests.ps1 -ArchivePath <cargo-about-0.9.2-x86_64-pc-windows-msvc.tar.gz>
./scripts/tests/setup-distribution.tests.ps1
./scripts/tests/setup-distribution.tests.ps1 -RealToolchain   # real isolated install under target/
```

The installer tests use an unmodified official archive as a local fixture and cover installation, reuse, hash rejection, corrupted caches, interrupted or tampered extraction and cleanup, without touching the user's Cargo install. The setup tests leave the machine's Rustup alone.

Unit tests: `cargo test --lib app_paths::tests`. Also manually test session, previews, archives and shutdown from a moved folder and a Windows 10/11 USB stick.
