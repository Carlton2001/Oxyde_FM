# Scripts

| Script | Purpose |
| --- | --- |
| `test-windows.ps1` | Full native Windows suite, also used by CI |
| `check-linux.sh` | Warning-free `cargo check` on Linux |
| `check-windows.sh`, `llvm-rc-crate-root.sh` | Compile the Windows target from Linux |
| `install-linux-desktop.sh` | Install the desktop entry and icon for the user |
| `build-distribution.ps1` | Windows portable ZIP and installer |
| `build-linux-packages.sh`, `linux-third-party-notices.py`, `rpm-license.py` | RPM and DEB packages |
| `bump-version.ps1`, `detect-release-commit.ps1`, `write-update-manifest.ps1` | Version commit and tag, CI detection, updater manifest |
| `distribution-icon.ps1`, `generate-viewer-cursors.cjs` | Regenerate icons and cursors kept in Git |
| `install-cargo-about.ps1`, `update-third-party-licenses.ps1`, `third-party-notices.hbs` | License notices |
| `audit-security.ps1`, `install-cargo-audit.ps1` | RustSec audit |

`tests/` holds PowerShell tests of these scripts.

Guides: [development](../docs/guides/development.md), [Linux development](../docs/guides/linux-development.md), [Windows distribution](../docs/guides/distribution.md), [Linux distribution](../docs/guides/linux-distribution.md), [audit](../docs/security/dependencies.md).

`update-third-party-licenses.ps1` and `third-party-notices.hbs` are hashed in `licenses/dependencies/inputs.json`: changing them requires regenerating the notices.
