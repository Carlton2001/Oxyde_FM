import React from 'react';
import { PanelState, PanelId, ConflictEntry, ConflictAction, ClipboardState } from './index';

// Interface modeled after the return value of useFileOperations
export interface FileOperations {
    // State
    pendingOp: { action: 'copy' | 'move', paths: string[], targetDir: string, turbo: boolean, estimates?: { total_bytes: number, total_files: number, is_cross_volume: boolean, likely_large: boolean } } | null;
    conflicts: ConflictEntry[] | null;
    deleteConfirm: { isOpen: boolean, count: number } | null;
    isExecuting: boolean;
    canUndo: boolean;
    canRedo: boolean;
    historyState: import('./index').HistoryState;

    // Actions
    initiateFileOp: (action: 'copy' | 'move', paths: string[], targetDir: string, turbo?: boolean) => Promise<boolean>;
    resolveConflicts: (resolutions: Map<string, ConflictAction>) => Promise<void>;
    cancelOp: () => void;
    deleteItems: (paths: string[], permanent?: boolean, turbo?: boolean) => Promise<void>;
    renameItem: (oldPath: string, newPath: string) => Promise<void>;
    createFolder: (path: string) => Promise<void>;
    setDeleteConfirm: (state: { isOpen: boolean, count: number } | null) => void;
    setConflicts: (conflicts: ConflictEntry[] | null) => void;
    setPendingOp: (op: { action: 'copy' | 'move', paths: string[], targetDir: string, turbo: boolean, estimates?: { total_bytes: number, total_files: number, is_cross_volume: boolean, likely_large: boolean } } | null) => void;

    undo: () => Promise<any>;
    redo: () => Promise<any>;
}

// Interface modeled after the return value of useClipboard
export interface ClipboardOperations {
    clipboard: ClipboardState | null;
    copy: (paths: string[]) => Promise<void>;
    cut: (paths: string[]) => Promise<void>;
    clearClipboard: () => Promise<void>;
    copyToSystem: (text: string) => Promise<boolean>;
    refreshClipboard: () => Promise<void>;
}

import { NotificationType, CompressionQuality } from './index';
import { DialogContextType } from '../context/DialogContext';

export interface ActionContext {
    activePanelId: PanelId;
    activePanel: PanelState;
    fileOps: FileOperations;
    clipboard: ClipboardOperations;
    notify: (message: string, type?: NotificationType, duration?: number) => void;
    t: (key: string) => string;

    // UI State Setters (Temporary hooks until full refactor)
    // Dialog Context
    dialogs: DialogContextType;

    // App Settings (for Archives etc)
    settings: {
        zipQuality: CompressionQuality;
        sevenZipQuality: CompressionQuality;
        zstdQuality: CompressionQuality;
        defaultTurboMode: boolean;
    };

    // UI Feedback
    setProgress?: (state: { visible: boolean; message: string; cancellable?: boolean } | null) => void;

    otherPanel?: PanelState;
    refreshBothPanels?: () => void;
    refreshTreePath?: (path: string) => void;

    // Tab Management
    tabs?: import('../hooks/useRustSession').Tab[];
    activeTabId?: string;
    setActiveTab?: (id: string) => void;
    closeTab?: (id: string) => void;

    // Allow extensions
    [key: string]: any;
}

export interface ActionDefinition {
    id: string;
    label: string; // Translation key
    getLabel?: (context: ActionContext) => string;
    icon?: React.ComponentType<{ className?: string, size?: number | string }>;
    shortcut?: string; // Default shortcut
    handler: (context: ActionContext) => void | Promise<void>;
    isVisible?: (context: ActionContext) => boolean;
    isEnabled?: (context: ActionContext) => boolean;
}
