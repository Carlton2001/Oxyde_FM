# Changelog
 
## [1.0.8] - 2026-03-06

### Added
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
