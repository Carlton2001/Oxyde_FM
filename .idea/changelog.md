# Changelog

## [1.1.1] - 2026-03-17

### Added
- **Active Filters Indicator**: Implemented a dynamic "Active Filters Bar" that mirrors group header styling. It provides clear visual feedback when the view is filtered and allows clearing all filters with a single click.
- **Persistent Filter Navigation**: Filters now persist when navigating through subfolders in the same context (Normal, Search, or Trash), enabling efficient deep-hierarchy exploration without re-applying filters.

### Fixed
- **Stuck Filter State**: Resolved an issue where users could get trapped in a filtered view after navigating between different contexts (e.g., from Trash back to a normal folder) where the filtering column was no longer visible.
- **Production Dialog Layout**: Fixed styling inconsistencies in the `ConflictDialog` for production builds by migrating inline styles to a centralized CSS architecture.
- **Breadcrumb Visibility**: Updated the breadcrumb component to ensure the current (last) folder name is always fully visible, even when the path is too long for the container.
- **UI Logic**: Refined the visibility rules for the secondary toolbar separator to only show when contextual actions are available.

### Changed
- **Filter Reset Logic**: Standardized filter cleanup to trigger only on major context changes (Normal view ↔ Search ↔ Trash), improving consistency across different navigation flows.
- **Refined Filter UX**: Replaced the "Clear All" text button in the filter bar with a discrete 'X' icon and tooltip to match the application's premium aesthetic.

## [1.1.0] - 2026-03-08

### Added
- **Unified Path Normalization**: Implemented a mandatory `normalizeEntry` pipeline for all file entries coming from the backend. This ensures 100% consistency across the application state, eliminating bugs caused by Windows path variations (case-sensitivity, trailing slashes, or drive root formats).

### Fixed
- **File Watcher Reliability**: 
    - Resolved a React Hook violation in `useFileSystem.ts` where an effect was incorrectly nested, causing silent refresh failures.
    - Updated `DirectoryTree` to support the new backend multi-path payload format for real-time updates.
    - Standardized `fs-change` relevance filtering using robust normalization to prevent missed auto-refreshes.
- **Tree Navigation**: Fixed a bug where new folders created at disk roots (e.g., `C:\`) were not appearing in the sidebar due to a mismatch between `C:` and `C:\` identifiers.
- **UI State Consistency**: Fixed node expansion and active state marking in the directory tree by enforcing normalized path comparisons.

## [1.0.9] - 2026-03-07

### Added
- **Icon Performance Optimization**: Implemented a "Generic First" strategy and persistent disk cache for folder and files icons, combined with pre-warming for instant directory navigation.
- **Protected Folder Handling**: Implemented detection of "Access Denied" directories. Protected folders now feature a vibrant red icon and a specific message with a Ban icon in the file panel when accessed.
- **Request ID Tracking**: Introduced a request-locking mechanism using unique IDs (based on navigation versions) for all asynchronous file system operations and searches.

### Fixed
- **Stale Data Collision**: Prevented "ghost" data from slow backend operations (like loading `WinSxS`) from leaking into the current view when switching tabs or navigating away during loading.
- **Search Result Isolation**: Search events are now strictly tab-scoped and request-scoped, ensuring that results from a cancelled or previous search never pollute the active results list.
- **Network Vicinity Freezes**: Blocked slow Windows icon retrieval for network items, preventing UI lag during network discovery.
- **React Rendering**: Resolved non-standard attribute warnings in virtualized lists.

## [1.0.8] - 2026-03-06

### Added
- **Native Drag In & Out Support**: Seamlessly drag and drop files from Windows Explorer into Oxyde and from Oxyde out to other applications and external targets.
- **Persistent File Operation Notifications**: Introduced a new notification system for long-running operations like deletions and emptying the recycle bin.
- **Infinite Loading Integration**: Added a "Loading" notification type with an infinite progress bar for operations when Windows APIs do not provide accurate progress reporting.


### Fixed
- **Trash Context Menu**: Restored the missing "Restore" option for items inside the Recycle Bin.
- **Notification Sync (Race Condition)**: Implemented advanced tracking and a safety net to ensure loading notifications are correctly dismissed, even for near-instant operations.
- **Pluralization Support**: French and English status messages now correctly handle singular/plural forms (e.g., "1 element deleted" vs "2 elements deleted").


### Changed
- **Clean Context Menus**: 
    - Renamed "Delete" to "Permanently Delete" in Trash and shift-action contexts for better clarity.
    - Removed redundant "Empty Recycle Bin" option from file context menus (available in the top bar).
- **Performance & Quality**: Performed a complete Rust Clippy pass and TypeScript type compliance check across the codebase.

## [1.0.7] - 2026-03-04

### Added
- **Smart Folder Naming**: Automatically suggests unique folder names (e.g., "New Folder (2)") if the name already exists in the current directory.
- **Per-Panel Settings**: Group-by-date preference is now isolated and saved per panel.
- **Improved Settings**: Moved selection checkboxes toggle from status bar to general settings menu.

### Fixed
- **File Watcher (Windows)**: Improved path normalization (stripping `\\?\` prefix) ensuring real-time updates for file system changes on Windows.
- **Scroll Management**:
    - Restored scroll position persistence during history navigation (Back/Forward).
    - Fixed scrollbar stuttering and infinite re-render loops during navigation.
- **Tab Isolation**: Navigation history and selection are now strictly isolated per tab.
- **UI Consistency**: Restored the selection gutter in the top corner for both list and grid views.

### Changed
- **Style Overhaul**: Replaced the admin shield badge with subtle red icon tinting for a cleaner look.
- **UI Polish**: Removed ellipsis (`...`) from folder creation and rename placeholders for a sleeker interface.
- **Code Quality**: Cleaned up unused imports and variables identifying by TypeScript compiler for better performance.
