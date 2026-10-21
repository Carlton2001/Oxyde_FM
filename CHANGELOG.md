# Changelog

## 2.0.0 — Native GPUI edition

This major release replaces the Tauri edition introduced in the 1.x series with a native Rust application. It brings an integrated Viewer, substantial security hardening, expanded archive workflows, and improvements throughout the file browsing experience.

### Upgrading from 1.2.1

- **Windows migration:** the new installer replaces the Tauri edition and removes its application data. Tauri preferences and saved sessions are not imported; configure your workspace again after upgrading. Back up the old application data before installation if you want to retain it.
- **Portable edition:** extract the complete portable ZIP and keep the three worker executables alongside `Oxyde_FM.exe`. The included `data` folder stores preferences, session data, caches, logs, and temporary files. Existing Tauri data is not imported.
- **Platform support:** Windows x64 is the current distribution target. Linux platform code is present, but its build and runtime are not currently validated, and several system integrations remain Windows-only.

### Native application foundation

- Replaced Tauri, React, TypeScript, and the webview with **Rust and GPUI**, the GPU-accelerated UI framework developed for Zed.
- Brought interface state and application logic into one language, removing the Rust-to-JavaScript bridge from UI interactions.
- Moved filesystem access, listing preparation, search processing, and expensive preview work off the UI thread, using Tokio, background threads, and dedicated workers.
- Reworked native menus, tooltips, dialogs, and secondary windows, including ownership, focus routing, placement, and close behavior.

### Integrated Viewer and previews

- Added a dedicated **Viewer panel** that follows the selection in the active file browser and can detach into a resizable window.
- Added full-resolution image viewing, pointer-centered zoom, panning, fullscreen display, and previous/next image navigation.
- Added multipage PDF viewing with on-demand rendering of pages near the viewport.
- Added virtualized text viewing with line numbers, wrapping, and background syntax highlighting, including additional PowerShell and SCSS syntax support.
- Added native Markdown rendering with headings, lists, tables, quotations, highlighted code blocks, and local images, plus a source view. Long documents render progressively.
- Added text viewing for Office Open XML and OpenDocument files, including worksheet and slide content.
- Added native Windows audio and video playback with playback controls, seeking, an autoplay preference, fullscreen video, and previous/next video navigation.
- Added audio metadata and embedded album artwork display.
- Improved hover previews, thumbnail generation, cache invalidation, worker reuse, and the release of unused images and textures.
- Preserved optional PowerToys Peek integration with its configured activation shortcut.

### Browsing and interface

- Rebuilt multipane and tabbed browsing in GPUI, with resizable layouts, drag-and-drop, and session restoration.
- Added natural filename ordering, so numbered names such as `file2` appear before `file10`.
- Added contextual grouping and group selection controls, including per-tab date grouping.
- Improved rectangle selection, modifier-based selection, multi-selection context actions, and drag feedback when Ctrl or Shift changes.
- Improved path completion, breadcrumb scrolling, sidebar layout, and slow-click renaming; added support for case-only renames.
- Added live removable-drive updates and native safe ejection on Windows, along with conditional OneDrive sidebar integration.
- Refined all thirteen themes, contrast, file icons, scrollbars, keyboard focus, and layouts at different UI scales and window sizes.
- Added an animations preference and refreshed application logos and icons.
- Retained English, French, Spanish, German, and Italian localization using Fluent catalogs.

### Search

- Reworked background search with progressive results, cancellation, and checks that prevent outdated results from replacing a newer navigation or search.
- Improved consistency of search criteria and hidden/system-file visibility across file and archive searches.
- Improved text-content search for very long lines while preserving regular expression and Unicode matching behavior.
- Restored Recycle Bin quick search using displayed item names.
- Retained duplicate detection by name, size, content, or combined criteria, with overlapping search roots deduplicated.

### File operations and archives

- Redesigned operation notifications and added a dedicated operations manager with progress, throughput graphs, expandable details, and per-item outcomes.
- Improved pause, cancellation, conflict handling, and reporting of partial results. Destination collisions within a transfer batch are checked before file mutations begin.
- Improved supported Undo/Redo workflows and responsiveness. Operation history remains in memory for the current session only.
- Improved Recycle Bin listing and restoration; independent items continue restoring when another item fails.
- Expanded archive workflows with browsing, name/metadata search, extraction, and creation or modification of ZIP, 7z, TAR, and TAR.ZST archives.
- Added stronger checks before archive replacement and recovery of partially published archive groups when possible.
- Added direct read-only browsing of ISO9660 ISO/IMG images alongside native Windows disk-image mounting.
- Extended Turbo with storage-aware copy concurrency, parallel deletion of eligible subfolders, and bounded parallel ZIP, 7z, and TAR.ZST compression.
- Reduced archive worker transport delays and improved ZIP extraction read reuse.

### Security hardening

- Moved Windows archive codecs, archive listing, image/SVG decoding, Office text extraction, and audio metadata/artwork extraction into restricted LPAC worker processes.
- Added shared worker resource budgets, execution deadlines, explicit handle inheritance, and validation of worker responses.
- Strengthened archive-path validation, file identity and provenance checks, destination publication, and checks against unexpected replacement of source files.
- Added authenticated recovery records for managed Windows archive caches and stronger handling of interrupted writes and uncertain recovery states.
- Kept native PDF, media, and Shell services in separate worker paths where applicable; these services do not have the same LPAC confinement as the restricted codecs.

See [security protections and remaining limits](docs/security/status.md) for the precise scope of these changes.

### Distribution and updates

- Added a Windows Inno Setup installer and a folder-based portable ZIP for the native edition.
- Added in-app updates using a signed installer and a manifest compatible with the existing Tauri updater. Portable users update through the releases page.
- Linked the Microsoft C runtime statically and bundled third-party dependency and syntax license notices.
- Added tag-triggered draft releases, updater signatures, and SHA-256 checksums. Updater signatures are separate from Windows Authenticode signing; the executables are not Authenticode-signed.
- Added Windows native-test automation and dependency security checks.

### Current limitations

- Archive search covers names and supported metadata; searching file contents inside archives is unavailable.
- Office viewing extracts stored text and values; it does not reproduce Office page layouts or calculate spreadsheet formulas.
- Archive modifications do not participate in filesystem Undo/Redo, and grouped archive replacement is not a durable multi-file transaction across crashes.
- Native audio/video playback depends on the codecs available on Windows.
