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
conflict-skip-all = Skip all
conflict-replace-all = Replace all

common-close = Close
common-retry = Retry
settings-title = Preferences
settings-appearance = APPEARANCE
settings-theme = Theme
settings-scale = Interface scale
settings-scale-value = { $percentage }%
settings-scale-reset = Reset the scale to 100%
settings-language = Language
theme-dark = { $name } Dark
theme-light = { $name } Light
settings-files = FILES
settings-hidden = Hidden files
settings-hidden-description = Show hidden files and folders, including names starting with a dot
settings-system = System files
settings-system-description = Show files and folders marked as system items by the OS
settings-previews = Media previews
settings-previews-description = Show file previews in the list’s tooltips
settings-grid-thumbnails = Grid image thumbnails
settings-grid-thumbnails-description = Show image thumbnails directly on grid cards
settings-about = About
settings-version = Version { $version }
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
tooltip-location = Location: { $value }
tooltip-original-location = Original location: { $value }
tooltip-deleted = Deleted: { $value }

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
menu-deselect-all = Deselect all
menu-select-all-checks = Check all
menu-deselect-all-checks = Uncheck all
column-name = Name
column-type = Type
column-size = Size
column-modified = Date modified
file-type-folder = Folder
file-type-file = File
file-type-symlink = Symbolic link
file-type-broken-link = Broken link
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
operation-extract-here = Extract archive
operation-new-folder = New folder
operations-title = { $count ->
    [1] Operation ({ $count })
   *[other] Operations ({ $count })
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
operations-to = To: { $path }
operations-items = { $count ->
    [1] { $done } / { $total } item
   *[other] { $done } / { $total } items
    }
operations-remaining = Time remaining: { $seconds } s
operations-duration = Duration: { $duration }
operations-average = Average: { $speed }
operations-speed-waiting = Waiting
operations-finalizing = Finalizing…
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

## Sidebar, filters and navigation errors
sidebar-quick-access = Quick access
sidebar-drives = DRIVES
sidebar-locations = LOCATIONS
sidebar-network = Network
sidebar-trash = Trash
sidebar-home = Home
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
path-error-access-denied = Protected folder (Administrator rights required)
path-error-failed = Could not open this folder. Check the path and try again.

## File operations
undo-failed = Could not undo: { $detail }
undo-copy-identity-changed = This item no longer matches the original copied object: { $detail }. Undo stopped to preserve your files.
undo-copy-unverified = The history cannot verify the identity of this copy: { $detail }. Your files have been preserved.
undo-overwrite-unavailable = This copy cannot be undone because it replaced existing files.
redo-failed = Could not redo: { $detail }
redo-copy-source-changed = The source has changed since the copy: { $detail }. Redo stopped to preserve your files.
redo-copy-unverified = The history cannot verify this copy: { $detail }. Start a new copy from the source folder.
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
unlabeled-volume = { $size } Volume
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

settings-checkboxes = Selection checkboxes
settings-checkboxes-description = Select multiple items without holding Ctrl.

settings-path-tracking = Follow active path
settings-path-tracking-description = Expand parent folders and reveal the active folder in the sidebar.

settings-date-format = Date format
date-format-us = United States (MM/DD/YYYY)
date-format-european = European (DD/MM/YYYY)
date-format-iso = ISO 8601 (YYYY-MM-DD)
settings-network = Network neighborhood
settings-network-description = Show the network neighborhood in the left panel.

settings-region-language = Region & language
settings-performance = Performance
settings-turbo = Turbo mode
settings-search-limit = Search limit
settings-data = Application data
settings-clear-cache = Clear cache
settings-reset-preferences = Reset preferences

title-new-panel = New panel on the right
title-tools = Search
tool-sizes = Calculate sizes
tool-search = Advanced search
quick-search = Quick search
quick-search-title = Search “{ $query }” in { $folder }
quick-search-results = Search results
quick-search-running = Searching…
quick-search-limited = Result limit reached
quick-search-empty = No results
quick-search-failed = Unable to search
quick-search-clear = Clear
quick-search-stop = Stop searching
open-file-location = Open file location
column-location = Location
tool-duplicates = Find duplicates

tabs-new-tab = New tab

date-this-month = This month
settings-group-by-date = Group by date
date-group-count = { $count ->
    [one] { $number } item
   *[other] { $number } items
}

menu-sort-by = Sort by
menu-sort-ascending = Ascending
menu-sort-descending = Descending
menu-sort-extension = Extension
drive-eject = Eject
drive-unmount = Unmount
map-network-drive = Map network drive
disconnect-network-drive = Disconnect network drive
disconnect-this-network-drive = Disconnect this network drive
network-force-prompt = Files are open or operations are in progress on { $drive }. Forcing disconnection may interrupt a copy or lose unsaved changes. Do you want to continue?
network-force-action = Force disconnection
network-category = Category
tooltip-network-category = Category: { $value }
tooltip-network-workgroup = Workgroup: { $value }
tooltip-network-location = Network location: { $value }
network-workgroup = Workgroup
network-location = Network location
network-computer = Computer
network-other-devices = Other devices
network-drive-failed = Unable to change the network connection. { $detail }
open-item-failed = Unable to open this item. { $detail }
network-map-description = What network folder would you like to map?
network-map-hint = Specify the drive letter and the folder that you want to connect to.
network-disconnect-description = Select the network drives to disconnect, then click Disconnect.
network-letter = Drive
network-folder = Folder
network-username = Username
network-password = Password
network-reconnect = Reconnect at sign-in
network-finish = Finish
network-disconnect = Disconnect
network-no-drives = No network drives found.
network-no-letter = No drive letter is available.
network-invalid-path = Enter a valid network path, for example \\server\share.
network-connected = Network drive mapped successfully.
network-disconnected = Network drives disconnected successfully.
network-working = Working…
network-loading = Loading drives…
drive-eject-failed = Unable to eject the drive. Another application may be using it. { $detail }
drive-unmount-failed = Unable to unmount the volume. Another application may be using it. { $detail }
drive-holders = Open in: { $applications }.
drive-mount-failed = Unable to mount the volume. { $detail }
drive-mount-hibernated = Windows was not fully shut down (hibernation or Fast Startup), so this volume stays locked. Shut Windows down completely, then try again.

archive-format = Archive .{ $format }
compression-fast = Fast
compression-normal = Balanced
compression-best = Maximum
compress = Compress
compression = Compression
compression-format = Format

disk-image-mount = Mount
disk-image-unmount = Unmount
disk-image-mount-failed = Could not mount the image. { $detail }
disk-image-unmount-failed = Could not unmount the image. { $detail }
disk-image-open-failed = Could not open the image. { $detail }
disk-image-busy = A file operation is still using this image.
disk-image-no-volume = No accessible volume was assigned to this image.

advanced-settings = Settings
advanced-help = Help
advanced-search-for = Search for
advanced-search-for-placeholder = *.* or filename.txt
advanced-search-in = Search in
advanced-search-subfolders = Search subfolders
advanced-search-in-archives = Search inside archives
advanced-find-text = Find text in file (non-binary)
advanced-find-text-placeholder = Search inside files content (non-binary)...
advanced-global-options = Global options
advanced-case-sensitive = Case sensitive
advanced-ignore-accents = Ignore accents
advanced-reset = Reset
advanced-search = Search
advanced-regex-help-title = Common Patterns
advanced-use-regex = Regular expression
advanced-regex-any-char = Any character
advanced-regex-any-char-desc = Matches any single character. Example: "b.t" matches "bat", "bit", "but".
advanced-regex-digit = Digits
advanced-regex-digit-desc = Matches any number from 0 to 9. Example: "file\d" matches "file1", "file2".
advanced-regex-word-char = Word characters
advanced-regex-word-char-desc = Matches letters, numbers, and underscores. Example: "i\w" matches "image", "item".
advanced-regex-start-line = Start of line
advanced-regex-start-line-desc = Matches only at the beginning of the name. Example: "^a" finds names starting with "a".
advanced-regex-end-line = End of line
advanced-regex-end-line-desc = Matches only at the end of the name. Example: "txt$" finds names ending in ".txt".
advanced-regex-one-or-more = One or more
advanced-regex-one-or-more-desc = Matches one or more of the previous character. Example: "a+" matches "a", "aa", "aaa".
advanced-regex-optional = Optional
advanced-regex-optional-desc = Matches zero or one of the previous character. Example: "colors?" matches "color" and "colors".
advanced-regex-either-or = Either / Or
advanced-regex-either-or-desc = Matches either the left or right part. Example: "cat|dog" matches "cat" or "dog".
advanced-regex-range = Range of characters
advanced-regex-range-desc = Matches any character inside the brackets. Example: "[abc]" matches "a", "b", or "c".
advanced-regex-negated-set = Negated set
advanced-regex-negated-set-desc = Matches any character NOT in the brackets. Example: "[^0-9]" matches anything except digits.
advanced-regex-group = Group
advanced-regex-group-desc = Groups multiple symbols together. Example: "(ab)+" matches "ab", "abab".
advanced-regex-count-exact = Exact count
advanced-regex-count-exact-desc = Matches exactly n times. Example: "a{ "{" }3{ "}" }" matches exactly "aaa".
advanced-size = Size
advanced-date = Modified
advanced-min = Min
advanced-max = Max
advanced-invalid = Check patterns, sizes and dates (YYYY-MM-DD).
advanced-date-placeholder = YYYY-MM-DD
advanced-weekdays = Mon Tue Wed Thu Fri Sat Sun

duplicate-locations = Search locations
duplicate-settings = Search settings
duplicate-name = Name
duplicate-size = Size
duplicate-content = Content (slow)
duplicate-current = Current folder
duplicate-warning = Selecting multiple locations can considerably slow down the search.
duplicate-filter = Filter results…
duplicate-idle = Configure the search and click Search.
duplicate-empty = No duplicates found.
duplicate-scanning = Scanning files: { $current }
duplicate-hashing = Comparing content: { $current } / { $total }
duplicate-groups = { $groups ->
    [one] { $group_count } duplicate group
   *[other] { $group_count } duplicate groups
    }
duplicate-files = { $file_count ->
    [one] { $file_number } file
   *[other] { $file_number } files
    }
duplicate-group = Group { $index }
duplicate-failed = Search failed: { $error }
duplicate-size-range = { $min } – { $max }
duplicate-disk = Disk #{ $disk }
duplicate-go-to-folder = Click to go to the folder
folder-size-empty = Empty
folder-size-provisional-description =
    Size awaiting an update.
    Automatic recalculation waits for activity to settle.
    Refresh to force the update.
quick-search-current-folder = Current folder

settings-clear-cache-confirm = Delete icon and thumbnail caches? They will be recreated when needed.
settings-reset-confirm = Restore default preferences? Open tabs will be preserved.
settings-cache-clearing = Clearing cache…
settings-cache-cleared = Cache cleared.
settings-cache-error = Could not clear the entire cache. Try again.
settings-preferences-reset = Preferences reset.

settings-turbo-description = Default Turbo mode for new copies, moves, deletions, Trash moves, compressions and extractions. Also applies to folder size calculations, including those already running. Running file operations keep their individual setting.
settings-search-limit-description = Maximum results for new quick and advanced searches. Steps of 500, minimum 500.

navigation-up = Up

about-repository = GitHub repository
about-main-tools = Main tools
update-available-toast = Oxyde { $detail } is available. Install it from Settings › About.
update-failed-toast = Update failed: { $detail }
update-check = Check for updates
update-checking = Checking…
update-current = Oxyde is up to date
update-check-failed = Check failed, try again
update-install = Install version { $version }
update-download-portable = Download version { $version }
update-downloading = Downloading the update…

settings-search-limit-reset = Reset to the default limit of 3,000 results

menu-copy-name = Copy name

trash-restore = Restore
trash-restore-all = Restore all
trash-delete-permanent = Delete permanently
trash-delete-all = Empty Recycle Bin
trash-original-location = Original location
trash-deleted-at = Date deleted

trash-restore-selection = Restore selection

original-location-filter-placeholder = Filter by original location…

trash-properties = Recycle Bin properties
trash-locations = Recycle Bin location
trash-available-space = Available space
trash-selected-settings = Settings for selected location
trash-custom-size = Custom size
trash-max-capacity = Maximum size
trash-dont-move = Do not move files to the Recycle Bin
trash-delete-immediately = Delete files immediately when removed
trash-confirm-delete = Display delete confirmation
trash-megabytes = MB
trash-settings-error = Could not apply settings. Check the entered capacity.
trash-auto-delete = Automatic deletion
trash-auto-delete-old = Automatically delete old items
trash-retention-days = Keep items for (days)
trash-gnome-retention-note = GNOME setting, applied whichever application deleted the files.
trash-kde-time-limit = Delete items older than (days)
trash-kde-size-limit = Limit size (% of disk)
trash-kde-limit-action = When the limit is reached
trash-kde-warn = Warn
trash-kde-delete-oldest = Delete oldest
trash-kde-delete-largest = Delete largest
trash-kde-retention-note = KDE settings, applied to files deleted from Dolphin and KDE applications.
trash-retention-error = Could not apply settings. Check the entered values.

trash-settings-ok = OK
trash-delete-prompt = Delete the selected items?

trash-item-count = { $count ->
    [one] ({ $formatted } item)
   *[other] ({ $formatted } items)
    }

trash-permanent-delete-prompt = Permanently delete the selected items?

drive-tooltip-usage = { $used } / { $total }

properties-system = System properties
properties-created = Created
properties-accessed = Accessed
properties-link-target = Link target
properties-used = Used space
properties-capacity = Capacity
properties-contains = Contains
properties-counts = { $files } files, { $folders } folders
properties-drive = Drive
properties-file-system = File system

properties-copy-name-double-click = Double-click to copy the name

toast-copied = Copied to clipboard
toast-cut = Cut to clipboard
toast-names-copied = { $count ->
    [one] { $number } name copied
   *[other] { $number } names copied
    }
toast-paths-copied = { $count ->
    [one] { $number } path copied
   *[other] { $number } paths copied
    }
toast-restored = { $count ->
    [one] { $number } item restored
   *[other] { $number } items restored
    }
toast-purged = { $count ->
    [one] { $number } item permanently deleted
   *[other] { $number } items permanently deleted
    }
toast-mounted = Disk image mounted
toast-unmounted = Disk image unmounted

toast-undo = Operation undone
toast-redo = Operation redone

toast-failed = Operation failed: { $detail }

toast-copy-completed = Copy completed
toast-move-completed = Move completed
toast-trash-completed = Moved to the recycle bin
toast-delete-completed = Permanent deletion completed
toast-compress-completed = Archive created
toast-extract-completed = Extraction completed

conflict-source = New file
conflict-target = Existing file
conflict-newer = Newer
conflict-older = Older
conflict-larger = Larger
conflict-items = items
conflict-skip = Skip
conflict-replace = Replace
conflict-all = All
conflict-identical = Identical
conflict-search = Filter results…
conflict-empty = No matching conflicts
conflict-prompt = An item named “{ $name }” already exists. What would you like to do?
conflict-advanced-title = Conflict resolution
conflict-advanced = Advanced
conflict-apply = Apply decisions
conflict-step-title = { $current } / { $total } File conflict
conflict-inherited = Inherited from { $parent }: { $action }
conflict-back = Back
conflict-recent-tab = Newer
conflict-older-tab = Older
conflict-automatic = Auto
conflict-apply-summary = Apply: { $replace_count ->
        [one] { $replace_number } replacement
       *[other] { $replace_number } replacements
    }, { $skip_count ->
        [one] { $skip_number } skipped
       *[other] { $skip_number } skipped
    }

peek-failed = Could not open Peek
properties-general = General
properties-shortcut = Shortcut
shortcut-target = Target
shortcut-arguments = Arguments
shortcut-working-directory = Start in
shortcut-comment = Comment
shortcut-run = Run
shortcut-normal = Normal window
shortcut-minimized = Minimized
shortcut-maximized = Maximized
shortcut-saving = Saving…
shortcut-invalid-target = The target must not be empty and the fields must be valid.

# Archive cache recovery and resource limits
archive-cache-full = Cannot open { $detail }. Oxyde cannot free enough space: copies are still in use, have been modified, or were just opened. Close unused documents and try again, or extract this file to a folder of your choice.
archive-cache-busy = Cannot open { $detail }. Another archive opening is still being prepared. Try again shortly, or extract this file to a folder of your choice.
archive-cache-unavailable = Cannot open { $detail }. Oxyde could not prepare the temporary copy. You can extract this file to a folder of your choice.
archive-cache-extract = Extract and open…
archive-cache-exported = File extracted: { $detail }
archive-limit-bytes = Processing stopped: the total decompressed size exceeds the { $limit } limit for this action. The compressed archive size is not the size being measured.
archive-processing-failed = Archive processing failed. The file may be damaged or require too many resources.
archive-recovery-notice = Retained files
archive-recovery-completed = The action completed, but some files or folders were kept. Check these locations before deleting them: { $locations }
archive-recovery-stopped = The action did not complete. Some files or folders were kept, including results already published. Check these locations before trying again: { $locations }
archive-recovery-more = { $count } other locations are recorded in the application log.
archive-limit-source-bytes = Archive creation stopped: the total source file size exceeds the { $limit } limit for this action.
archive-limit-encoded-bytes = Archive creation stopped: the output size exceeds the { $limit } limit for this action.
archive-limit-entries = Processing stopped: the archive contains more than { $limit } files and folders.
archive-limit-depth = Processing stopped: an archive path exceeds { $limit } levels, including the file name.
archive-limit-time = Processing stopped: the maximum active processing time of { $limit } minutes was reached. Pauses do not count.
archive-limit-decoded = Processing stopped: decompressed data, including headers, exceeds { $limit }.
archive-source-changed = The archive changed during the operation. Oxyde stopped the operation and preserved the current archive. Retry the operation using this version.

quick-search-archives-skipped =
    { $count ->
        [one] Incomplete results: { $number } archive skipped
       *[other] Incomplete results: { $number } archives skipped
    }
quick-search-archive-skipped-detail = First skipped archive: { $path }. { $reason }
quick-search-archive-unreadable = This archive is invalid, unsupported, or unreadable.
quick-search-archive-content-not-searched = Content inside archives is not searched. Entries from this archive are excluded while a content filter is active.
quick-search-archive-dates-unavailable = Entry dates are unavailable for this archive format. Its entries are excluded while a date filter is active.

undo-new-folder-changed = The folder no longer matches the one that was created: { $detail }. Undo stopped to preserve your files.
undo-new-folder-unverified = The history cannot verify this folder: { $detail }. The folder and its contents have been preserved.
undo-new-folder-not-empty = The folder now contains files or subfolders: { $detail }. Undo does not delete its contents.
undo-new-folder-missing = The created folder could not be found: { $detail }. Undo stopped without deleting other items.
redo-new-folder-conflict = An item already exists at this location: { $detail }. Redo does not replace it.
redo-new-folder-parent-missing = The parent folder could not be found: { $detail }. Redo does not recreate parent folders.
redo-new-folder-unverified = The history cannot redo this folder creation: { $detail }. No item was created.

redo-move-changed = The file or folder changed since the operation: { $detail }. Redo Move has stopped to preserve your files.
redo-move-conflict = An item occupies the destination: { $detail }. Redo Move will not replace it.
redo-move-unverified = This history cannot safely redo the move: { $detail }. Start the move again from the current files.
redo-move-incomplete = The move stopped and some items could not return to their original locations. Files or folders have been kept for recovery: { $detail }. Check these locations before retrying.

undo-recovery-incomplete = Undo stopped and some items could not return to their original locations. Files or folders have been kept for recovery. Check these locations before retrying: { $detail }.
preview-unavailable = This preview could not be loaded. Try opening the file directly.
preview-image-limit = This image exceeds the preview limits. The file remains accessible.
preview-animation-static = Still preview: the animation exceeds the preview limits.

undo-move-changed = Cannot undo this move: the file or folder was replaced or recreated. It was preserved at its current location. { $detail }
undo-move-unverified = Cannot safely undo this move: the file or folder’s identity could not be verified. It was preserved. { $detail }

undo-rename-changed = Cannot undo this rename: the file or folder was replaced. It was preserved at its current location. { $detail }
undo-rename-unverified = Cannot safely undo this rename: the file or folder’s identity could not be verified. It was preserved. { $detail }
redo-rename-changed = Cannot redo this rename: the file or folder was replaced. It was preserved at its current location. { $detail }
redo-rename-unverified = Cannot safely redo this rename: the file or folder’s identity could not be verified. It was preserved. { $detail }

transfer-source-changed = The source was replaced or is no longer a regular file or folder. The operation stopped; items already transferred were kept. Check the source and try again.
transfer-source-unverified = The source identity cannot be verified at this location. The operation stopped; items already transferred were kept. Try from a compatible local folder.

trash-restore-ambiguous = Several Recycle Bin items came from the same location. Open the Recycle Bin and select the specific item to restore. Items already restored were kept.
invalid-encoding-name = { $name } (invalid encoding)
unrepresentable-path = The name of “{ $path }” is not valid UTF-8 text. Rename it before copying, moving, deleting or compressing it; nothing was changed.
trash-restore-changed = The Recycle Bin item changed after selection. Restoration stopped. Refresh the Recycle Bin and select the item again. Items already restored were kept.
trash-restore-conflict = A file or folder already exists at the restore location. It was kept, and the affected item remains in the Recycle Bin. Rename or move the destination and try again.
trash-restore-unavailable = This item could not be restored. Check that it is still in the Recycle Bin and that its original folder exists and is accessible, then try again. Items already restored were kept.

history-delete-changed = The item no longer matches the object recorded when it was deleted, or it is unavailable. Undo or Redo stopped to preserve current files. Items already processed were kept.
history-delete-unverified = Oxyde does not have the information needed to undo or redo this deletion safely. The action stopped. You can check the Recycle Bin and manually restore the item you want.

preview-timed-out = This preview exceeded the 15-second time limit. The file remains accessible.
copy-name-suffix = Copy
shell-delete-completed-error = Windows finished deleting this item, then reported an error. The completed operation remains in history. Check the folder and Recycle Bin before trying again.
shell-delete-unconfirmed = Windows did not confirm deletion of this item. The deletion may have been partial. Check the folder and Recycle Bin before trying again.

archive-listing-metadata-limit = Cannot display the contents: the file list exceeds { $limit }. This limit applies to names and file information, not decompressed file sizes. You can try extracting with the usual limits.
archive-listing-worker-failed = Cannot read this archive’s contents: the reading process failed or returned an invalid response. The archive has not been modified. You can retry or try extracting with the usual limits.

toast-transfer-skipped = All items were skipped.

properties-calculating-size = Calculating size…
drag-new-tab = Create a new tab
drag-unavailable = Drop unavailable
properties-loading = Loading properties…

storage-unavailable = Could not access Oxyde data. In portable mode, the data folder must be writable.

properties-selection-items = { $number } items
properties-total-size = Total size
properties-multiple-locations = Multiple locations
operations-affected-items = { $count ->
    [1] Item involved ({ $number })
   *[other] Items involved ({ $number })
    }
operations-view-items = View all { $count } items…
operations-items-filter = Search names or paths…
operations-no-matching-items = No matching items.
operations-items-loading = Loading items…
operations-item-done = Processed
operations-item-not-processed = Not processed
operations-item-skipped = Skipped
operations-item-partial = Partially processed
operations-item-unknown = Unknown status
operations-item-interrupted = Interrupted

default-archive-format = Default archive format

settings-onedrive = OneDrive
settings-onedrive-description = Show OneDrive in the sidebar when configured.

trash-restore-summary = Restoration finished: { $restored } item(s) restored, { $failed } failed. Other items were processed. See the log for error details.
transfer-destination-collision = Several selected items would use the same destination: { $path }. Rename them or transfer them separately. No files have been transferred.
archive-link-refused = An archive cannot contain a symbolic link: { $path }. Oxyde refuses links when opening and extracting archives, so that no extraction can write outside its destination. Nothing has been changed.

grouping = Group by
group-none = None
group-date = Date modified
group-type = File type
group-folders = Folders
group-archives = Archives
group-applications = Applications and installers
group-documents = Documents
group-images = Images
group-video = Videos
group-audio = Audio
group-code = Code and scripts
group-other = Other
group-disk-images = Disk images
mouse-right-click = Right click
grouping-current = Grouped by: { $mode }
group-size = Size
group-extension = Extension
group-created = Date created
group-deleted = Date deleted
group-size-empty = Empty (0 bytes)
group-no-extension = No extension
group-origin-unknown = Unknown original location
group-date-unknown = Unknown date
group-select-all = Select all items in the group
group-deselect-all = Deselect all items in the group

viewer-title = Viewer
viewer-visual-paused = Video display paused
viewer-file-unavailable = This file cannot be displayed.
viewer-open = Open the Viewer
viewer-close = Close the Viewer
viewer-slide = Slide
viewer-text-truncated = Only the first 512 MiB of the file are shown.
viewer-empty = Select a file in an explorer pane.
viewer-detach = Detach Viewer
viewer-detached = The Viewer is open in a secondary window.
viewer-show-window = Show Viewer window
viewer-play = Play
viewer-pause = Pause

viewer-rendered = Rendered
viewer-code = Code
viewer-highlighted = Syntax highlighting

viewer-enable-highlighting = Enable syntax highlighting
viewer-disable-highlighting = Disable syntax highlighting

group-scripts = Scripts

viewer-loading = Loading file…
viewer-autoplay = Autoplay
viewer-enable-autoplay = Enable autoplay for the next media files
viewer-disable-autoplay = Disable autoplay for the next media files

audio-no-tags = No tags available
audio-album = Album
audio-track = Track
audio-year = Year


quick-search-content-skipped =
    { $count ->
        [one] Content not checked: { $number } file
       *[other] Content not checked: { $number } files
    }
quick-search-content-skipped-detail = Content search incomplete for { $path }: unreadable document or resource limit reached.
