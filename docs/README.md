# Oxyde documentation

Code paths in backticks start at `src/` or the repository root.

## Guides

- [Development](guides/development.md): build, test, split modules
- [Linux development](guides/linux-development.md): setup, Windows guard, remaining work
- [Windows distribution](guides/distribution.md): portable, installer, updates, CI and release
- [Linux distribution](guides/linux-distribution.md): RPM, DEB, CI
- [Copy engine benchmark](guides/copy-benchmark.md)
- [Visual checks](guides/visual-checks.md)

## Architecture

Shared:
[interface](architecture/interface.md),
[windows and menus](architecture/windows.md),
[previews](architecture/previews.md),
[archives](architecture/archives.md),
[operations and Turbo](architecture/operations.md),
[navigation and search](architecture/search.md),
[volumes](architecture/volumes.md),
[file clipboard](architecture/clipboard.md),
[folder watching](architecture/watchers.md),
[quick access](architecture/quick-access.md).

Linux:
[previews and sandbox](architecture/linux-previews.md),
[copies](architecture/copies.md),
[moves](architecture/moves.md),
[symbolic links](architecture/links.md),
[held-directory effects](architecture/linux-held-effects.md),
[access and deletion](architecture/linux-access.md),
[names and paths](architecture/linux-names.md),
[non-UTF-8 names](architecture/non-utf8-names.md),
[archive open cache](architecture/linux-open-cache.md),
[properties](architecture/linux-properties.md),
[network](architecture/linux-network.md),
[desktop identity](architecture/desktop-identity.md).

## Diagnostics

[Copy](diagnostics/copy.md),
[deletion](diagnostics/delete.md),
[archive compression](diagnostics/archive-compression.md),
[memory](diagnostics/memory.md),
[shutdown](diagnostics/shutdown.md).

## Security

[Protections and limits](security/status.md),
[retained dependencies and audit](security/dependencies.md).

## Maintaining these docs

- English, `kebab-case` file names. `guides/` holds procedures, `architecture/` current contracts, `diagnostics/` problem-solving, `security/` guarantees and decisions.
- State the current behavior, with useful exceptions and code entry points. No work logs, dated results or exhaustive test lists; Git keeps the history. Dated reports and work logs were removed after commit `6d5d720c`, where they can still be read.
- One rule, one reference; other documents link to it. Update this index when adding, moving or removing a page.
