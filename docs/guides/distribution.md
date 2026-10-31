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

Without `-PortableOnly` the script also builds the installer with Inno Setup 6.3+. `-SkipBuild` reuses existing release executables; `-IsccPath` locates Inno Setup. The C runtime is statically linked (`.cargo/config.toml`): no Microsoft DLL ships, and the script refuses an executable that still imports one. Executables are not signed.

Each run regenerates crate licenses from `Cargo.lock` before compiling, even with `-SkipBuild`, and includes the report in both packages; a generation failure stops the build, and network access is needed for missing upstream notices. To generate them separately: [notices guide](../../licenses/dependencies/README.md). The script prints separate timings for licenses, Rust, Inno Setup and ZIP, and verifies required files in the ZIP.

## Portable version

Extract `target/distribution/output/Oxyde_FM-<version>-x64-portable.zip`, then run `Oxyde_FM/Oxyde_FM.exe`. Keep the three workers next to the executable. `licenses/syntaxes` and `licenses/dependencies` hold grammar, crate, GPUI/Zed and Lucide notices, in both distribution formats.

A `data` folder next to the executable turns on portable mode, whatever the working directory. It ships in the ZIP and holds:

- `config.json`: preferences and session state
- `oxyde.log`, possibly `oxyde.previous.log`: diagnostics
- `cache/icons`, `cache/thumbnails`: generated caches and their control files
- `temp`: temporary files and archive/worker workspaces

TEMP, TMP and TMPDIR are redirected to `data/temp` before threads start, and children inherit them. Installed configuration and Tauri data are neither imported nor modified. If `data` is unusable an error is shown: there is no silent fallback to AppData, so writable media is required.

To move the application, move the whole `Oxyde_FM` folder, `data` included; stored paths to user files are not rewritten. Removing `data` turns portable mode off at next start. Clearing the cache in settings keeps configuration, logs and session. No shortcut or uninstall entry is created.

This isolation covers files Oxyde manages, not traces left by Windows, external tools or file operations the user requests. Ephemeral AppContainer profiles are still used and removed as usual; protections are not reduced in portable mode.

## Installer

`Oxyde_FM_<version>_x64-setup.exe` installs per user into `%LOCALAPPDATA%\Programs\Oxyde_FM`, with a Start shortcut, an optional desktop shortcut and its own uninstall entry. Identity: `Oxyde.GPUI.Windows`. It deploys no `data` folder. Uninstalling removes `%LOCALAPPDATA%\Oxyde` (caches) and offers, except in silent mode, to remove `%APPDATA%\Oxyde` (preferences, session, logs). No file association, PATH change or Explorer replacement.

Run with `/UPDATE` (by the Tauri or GPUI updater), it skips its pages then restarts Oxyde. NSIS options added by the Tauri updater (`/P /R /ARGS`) are ignored.

Transition from the Tauri edition: after installing, the Setup uninstalls any `Oxyde_FM` entry from publisher `Oxyde` (per-machine MSI with elevation, or per-user NSIS), then removes `%APPDATA%\com.oxyde.desktop` and `%LOCALAPPDATA%\com.oxyde.desktop`. This section of `oxyde.iss` is temporary.

## In-app updates

`systems/updates.rs` reads `latest.json` from the latest published release, compares it with `CARGO_PKG_VERSION`, and verifies the Setup's minisign signature with the updater's public key before writing it to a temporary folder. The download goes through `Platform::download` (WinHTTP, system proxy, HTTPS only, bounded size).

An automatic check runs 30 s after start. Each version is announced once (`last_notified_update` in `config.json`), never in portable mode. Settings › About checks and installs: it runs the Setup with `/UPDATE /WAITPID=<pid>` then closes Oxyde normally; the Setup waits for exit (60 s at most), replaces files and restarts Oxyde. In portable mode the button opens the releases page.

Outside Windows `Platform::updates_itself` is false: no check, no button. Like Nautilus, Oxyde leaves RPM, DEB or Flatpak packages to the system's package manager.

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

Pushing a `v<x.y.z>` tag starts `.github/workflows/release.yml`. A single run checks the tag against `Cargo.toml`, then runs native tests, audit and distribution in parallel on the tagged commit. The `release` job needs all of them: a failure or cancellation prevents signing. It downloads the files built **by that same run**, signs the Setup with the updater's minisign key (secrets `TAURI_SIGNING_PRIVATE_KEY` and `_PASSWORD`, through the GitHub `release` environment), then generates `latest.json` with `scripts/write-update-manifest.ps1`. All three Windows manifest keys point to the Inno Setup installer. The workflow creates a draft release; publishing it by hand makes it visible to updaters. If a check fails, re-run the failed jobs from the tag's run.

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
