common-cancel = Cancel
rename-title = Rename
rename-name-placeholder = File name
rename-error-empty-name = The name cannot be empty.
rename-error-already-exists = A file or folder with this name already exists here.
rename-error-not-found = The item to rename no longer exists or is no longer accessible.
rename-error-access-denied = Renaming was denied. Check this item’s permissions.
rename-error-invalid-name = Renaming failed. Check the name and try again.
rename-error-unavailable = This item cannot be renamed right now.
rename-error-task-failed = Renaming failed. Try again.
conflict-title = Files already exist
conflict-description = { $count ->
    [one] { $number } conflict detected. What would you like to do?
   *[other] { $number } conflicts detected. What would you like to do?
    }
conflict-skip-all = Skip all
conflict-replace-all = Replace all

common-close = Close
common-retry = Retry
settings-title = Preferences
settings-appearance = APPEARANCE
settings-theme = Theme
settings-scale = Interface scale
settings-scale-value = { $reference } px
settings-scale-reset = Reset the reference to 16 px per rem
settings-language = Language
theme-dark = Dark
theme-light = Light
settings-files = FILES
settings-hidden = Hidden files
settings-hidden-description = Show hidden files and folders, including names starting with a dot
settings-system = System files
settings-system-description = Show files and folders marked as system items by the OS
settings-previews = Media previews
settings-previews-description = Show file previews in the list’s tooltips
settings-grid-thumbnails = Grid image thumbnails
settings-grid-thumbnails-description = Show image thumbnails directly on grid cards
settings-about = ABOUT
settings-version = Version { $version }
settings-description = Modern file manager
settings-save-error = Preferences applied, but could not be saved. Try again.
settings-load-error = Saved preferences could not be read. Changes will not be saved to protect that file.
status-folders = { $count ->
    [one] { $number } folder
   *[other] { $number } folders
    }
status-files = { $count ->
    [one] { $number } file
   *[other] { $number } files
    }
view-details = Details view
view-grid = Grid view
selected-files = { $count ->
    [one] { $number } file selected ({ $size })
   *[other] { $number } files selected ({ $size })
    }
selected-folders = { $count ->
    [one] { $number } folder selected ({ $size })
   *[other] { $number } folders selected ({ $size })
    }
selected-mixed = { $folder_count ->
    [one] { $folders } folder
   *[other] { $folders } folders
    } and { $file_count ->
    [one] { $files } file
   *[other] { $files } files
    } selected ({ $size })
tooltip-type = Type: { $value }
tooltip-size = Size: { $value }
tooltip-modified = Modified: { $value }

## Main interface and commands
menu-open = Open
menu-open-new-tab = Open in new tab
menu-pin-quick-access = Pin to Quick Access
menu-unpin-quick-access = Unpin from Quick Access
menu-cut = Cut
menu-copy = Copy
menu-paste = Paste
menu-rename = Rename
menu-delete = Delete
menu-properties = Properties
menu-more-options = More options
menu-new-folder = New folder
menu-duplicate-tab = Duplicate tab
menu-close-other-tabs = Close other tabs
menu-close-tab = Close tab
menu-select-all = Select all
menu-select-all-checks = Check all
menu-deselect-all-checks = Uncheck all
column-name = Name
column-type = Type
column-size = Size
column-modified = Date modified
file-type-folder = Folder
file-type-file = File
file-type-symlink = Symbolic link
file-type-junction = Junction
empty-filtered = No items match the filters.
folder-loading = Loading folder…
folder-empty = This folder is empty.
preview-loading = Loading preview…
operation-failed = Operation failed
path-placeholder = Path
navigation-back = Back
navigation-forward = Forward
navigation-refresh = Refresh
navigation-undo = Undo
navigation-redo = Redo
operation-copy = Copy
operation-move = Move
operation-rename = Rename
operation-delete = Delete
operation-compress = Compression
operation-extract = Extraction
operation-compress-zip = Compress to ZIP
operation-extract-here = Extract archive
operation-new-folder = New folder
operation-restore = Restore
operations-title = { $count ->
    [1] Operation
   *[other] Operations
    }
operations-show-panel = Show operations
operations-hide-panel = Hide operations
operations-queued = Queued
operations-calculating = Calculating…
operations-running = Running
operations-paused = Paused
operations-cancelled = Cancelled
operations-completed = Completed
operations-error = Failed
operations-from = From: { $path }
operations-to = To: { $path }
operations-items = { $done } / { $total } items
operations-remaining = Time remaining: { $seconds } s
operations-duration = Duration: { $duration }
operations-average = Average: { $speed }
operations-speed-waiting = Waiting
operations-pause = Pause
operations-resume = Resume
operations-stop = Cancel operation
operations-fast = Fast mode active — click to switch to normal mode
operations-normal = Normal mode active — click to switch to fast mode
operations-dismiss = Dismiss
operations-dismiss-all = Remove all completed operations
operations-empty = No operations
name-filter-placeholder = Filter by name…
undo-operation = Undo: { $operation }
redo-operation = Redo: { $operation }
shortcut-back = Alt+Left
shortcut-forward = Alt+Right

## Sidebar, filters and navigation errors
sidebar-quick-access = QUICK ACCESS
sidebar-drives = DRIVES
sidebar-locations = LOCATIONS
sidebar-network = Network
sidebar-trash = Trash
size-tiny = Tiny (< 10 KB)
size-small = Small (10 KB - 1 MB)
size-medium = Medium (1 MB - 100 MB)
size-large = Large (100 MB - 1 GB)
size-huge = Very large (> 1 GB)
date-today = Today
date-yesterday = Yesterday
date-this-week = This week
date-last-week = Last week
date-older = Older
path-error-not-found = This folder was not found or is no longer available.
path-error-access-denied = Access to this folder was denied. Check your permissions.
path-error-failed = Could not open this folder. Check the path and try again.

## File operations
undo-failed = Could not undo: { $detail }
undo-overwrite-unavailable = This copy cannot be undone because it replaced existing files.
redo-failed = Could not redo: { $detail }
prepare-failed = Could not prepare operation: { $detail }
transfer-failed = Transfer failed: { $detail }
drag-copy = Copy
drag-move = Move
new-folder-name = New folder
new-folder-numbered = New folder ({ $number })
native-menu-empty = No commands available
native-menu-failed = Could not load native menu: { $detail }
drag-items = { $count ->
    [one] { $number } item
   *[other] { $number } items
    }

native-menu-loading = Loading menu…
local-drive = Local Disk ({ $drive })
unknown-item = Unknown
menu-expand-all = Expand all
menu-collapse-all = Collapse all
menu-copy-path = Copy path

sidebar-collapse-all = Collapse all
sidebar-scroll-top = Scroll to top
sidebar-collapse = Collapse sidebar
sidebar-expand = Expand sidebar

settings-windows-icons = Windows icons
settings-windows-icons-description = Use Windows file and folder icons in list and grid views.
