export type FileEntry = {
    name: string;
    path: string;
    is_dir: boolean;
    is_hidden?: boolean;
    is_system?: boolean;
    is_symlink?: boolean;
    is_junction?: boolean;
    size: number;
    is_calculated?: boolean;
    is_calculating?: boolean;
    modified: number;
    is_readonly?: boolean;
    // Trash-specific fields (populated when viewing Recycle Bin)
    original_path?: string;
    deleted_time?: number;
};

export type ViewMode = 'grid' | 'details';
export type Theme = 'github-light' | 'github-dark' | 'ayu-light' | 'ayu-dark' | 'one-light' | 'one-dark' | 'monokai' | 'solarized-light' | 'solarized-dark' | 'windows-light' | 'windows-dark' | 'oxyde-light' | 'oxyde-dark';
export type LayoutMode = 'standard' | 'dual';
export type Language = 'en' | 'fr';
export type DateFormat = 'US' | 'European' | 'ISO';
export type CompressionQuality = 'fast' | 'normal' | 'best';
export type PanelId = 'left' | 'right';

export interface HistoryEntry {
    path: string;
    selected?: string[];
}

export interface PanelState {
    path: string;
    files: FileEntry[];
    selected: Set<string>;
    viewMode: ViewMode;
    sortConfig: SortConfig;
    history: HistoryEntry[];
    historyIndex: number;
    searchQuery: string;
    searchResults: FileEntry[] | null;
    isSearching: boolean;
    searchLimitReached: boolean;
    currentSearchRoot: string;
    colWidths: ColumnWidths;
    lastSelectedPath?: string;
    refresh: () => void;
    updateFileSize: (path: string, size: number) => void;
    setFileCalculating: (path: string, isCalculating: boolean) => void;
    navigate: (path: string) => void;
}

export interface ClipboardState {
    paths: string[];
    action: 'copy' | 'cut';
}

export type SortField = 'name' | 'size' | 'date' | 'type' | 'location' | 'deletedDate';
export type SortDirection = 'asc' | 'desc';

export interface SortConfig {
    field: SortField;
    direction: SortDirection;
}

export interface ColumnWidths {
    name: number;
    location: number;
    type: number;
    size: number;
    date: number;
    deletedDate: number;
}

export interface ShortcutInfo {
    target: string;
    arguments: string;
    working_dir: string;
    description: string;
    icon_location: string;
    icon_index: number;
    run_window: number;
}

export interface FileProperties {
    name: string;
    path: string;
    parent: string;
    is_dir: boolean;
    size: number;
    is_calculated: boolean;
    created: number;
    modified: number;
    accessed: number;
    readonly: boolean;
    is_hidden: boolean;
    is_system: boolean;
    // Trash-specific fields
    original_path?: string;
    deleted_time?: number;
    // Optional item counts
    folders_count?: number;
    files_count?: number;
    shortcut?: ShortcutInfo;
}

export interface FolderSizeResult {
    size: number;
    folders_count: number;
    files_count: number;
}

export interface FileSummary {
    count: number;
    total_size: number;
    files_count: number;
    folders_count: number;
    all_readonly: boolean;
    any_readonly: boolean;
    all_hidden: boolean;
    any_hidden: boolean;
    parent_path?: string;
}

export interface DirResponse {
    entries: FileEntry[];
    summary: FileSummary;
    is_complete: boolean;
}

export interface DirBatchEvent {
    panel_id: string;
    path: string;
    entries: FileEntry[];
    is_complete: boolean;
}

export interface DriveInfo {
    path: string;
    label: string;
    drive_type: 'fixed' | 'removable' | 'remote' | 'cdrom' | 'unknown';
    is_readonly: boolean;
    total_bytes?: number;
    free_bytes?: number;
    media_type?: string;
    physical_id?: string;
}

export interface QuickAccessItem {
    name: string;
    path: string;
}

export type NotificationType = 'error' | 'success' | 'info' | 'warning';

export interface AppNotification {
    id: string;
    type: NotificationType;
    message: string;
    duration?: number;
}

export interface ConflictEntry {
    name: string;
    source: FileEntry;
    target: FileEntry;
}

export interface ConflictResponse {
    conflicts: ConflictEntry[];
    total_size: number;
    total_files: number;
    is_cross_volume: boolean;
    likely_large: boolean;
}

export type ConflictAction = 'replace' | 'skip';

export type TransactionType = 'copy' | 'move' | 'rename' | 'new_folder' | 'delete';

export interface SidebarNode {
    name: string;
    path: string;
    is_hidden: boolean;
    is_system: boolean;
    is_readonly: boolean;
    has_subdirs: boolean;
}

export interface FileTransaction {
    type: TransactionType;
    details: {
        // For copy/move
        paths?: string[];     // Source paths
        targetDir?: string;   // Destination directory

        // For rename
        oldPath?: string;
        newPath?: string;

        // For new_folder
        path?: string;
    };
    timestamp: number;
}


export interface ProgressInfo {
    message: string;
    cancellable?: boolean;
    cancelling?: boolean;
}

export interface TransactionDetails {
    paths: string[];
    target_dir?: string;
    old_path?: string;
    new_path?: string;
}

export interface Transaction {
    id: string;
    timestamp: number;
    op_type: 'Copy' | 'Move' | 'Rename' | 'Delete' | 'NewFolder' | 'Restore';
    details: TransactionDetails;
}

export interface HistoryState {
    undo_stack: Transaction[];
    redo_stack: Transaction[];
}

export type OpStatus =
    | 'Queued'
    | 'Calculating'
    | 'Running'
    | 'Paused'
    | 'Cancelled'
    | 'Completed'
    | { Error: string }
    | 'WaitingForConflictResolution';

export type FileOpType = 'Copy' | 'Move' | 'Delete' | 'Trash';

export interface FileOperation {
    id: string;
    op_type: FileOpType;
    sources: string[];
    destination?: string;
    status: OpStatus;
    total_bytes: number;
    processed_bytes: number;
    total_files: number;
    processed_files: number;
    current_file?: string;
    bytes_per_second?: number;
    turbo?: boolean;
    is_cross_volume?: boolean;
    likely_large?: boolean;
}

export interface SearchEventPayload {
    panel_id: string;
    results: FileEntry[];
    completed: boolean;
}

export interface SearchOptions {
    query: string;
    root: string;
    regex: boolean;
    caseSensitive: boolean;
    recursive: boolean;
    searchInArchives: boolean;
    contentRegex?: boolean;
    ignoreAccents?: boolean;
    minSize?: number;
    maxSize?: number;
    minDate?: number;
    maxDate?: number;
    contentQuery?: string;
    sizeUnit?: 'bytes' | 'kb' | 'mb' | 'gb' | 'tb';
}

export const APP_CONSTANTS = {
    MAX_SEARCH_RESULTS: 3000,
    MAX_ICON_CACHE_SIZE: 400,
};
