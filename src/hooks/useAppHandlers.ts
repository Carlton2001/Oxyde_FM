import { useCallback, useMemo } from 'react';
import { PanelId, FileEntry, ColumnWidths, SortField, DriveInfo } from '../types';
import { ActionContext } from '../types/actions';
import { AppContextValue } from '../context/AppContext';
import { actionService } from '../services/ActionService';
import { formatCommandError } from '../utils/error';
import { getParent } from '../utils/path';
import { DirectoryTreeHandle } from '../components/ui/DirectoryTree';
import { invoke } from '@tauri-apps/api/core';

interface AppHandlersProps {
    left: any;
    right: any;
    activePanelId: PanelId;
    setActivePanelId: (id: PanelId) => void;
    layout: string;
    fileOps: any;
    treeRef: React.RefObject<DirectoryTreeHandle | null>;
    notify: (message: string, type: 'error' | 'info' | 'success' | 'warning', duration?: number) => void;
    t: any;
    dialogs: any;
    clipboard: any;
    refreshDrives: () => void;
    tabs: any[];
    activeTabId: string;
    setActiveTab: (id: string, state?: any) => void;
    closeTab: (id: string, nextId?: string) => void;
    addTab: (path: string, id?: string, background?: boolean) => void;
    setContextMenu: (menu: any) => void;
    contextMenu: any;
    drives: DriveInfo[];
    defaultTurboMode: boolean;
    zipQuality: any;
    sevenZipQuality: any;
    zstdQuality: any;
    favorites: any[];
    peekStatus?: AppContextValue['peekStatus'];
}

export const useAppHandlers = ({
    left,
    right,
    activePanelId,
    setActivePanelId,
    layout,
    fileOps,
    treeRef,
    notify,
    t,
    dialogs,
    clipboard,
    tabs,
    activeTabId,
    setActiveTab,
    closeTab,
    addTab,
    setContextMenu,
    contextMenu,
    drives,
    refreshDrives,
    defaultTurboMode,
    zipQuality,
    sevenZipQuality,
    zstdQuality,
    favorites,
    peekStatus
}: AppHandlersProps) => {
    const activePanel = activePanelId === 'left' ? left : right;

    const refreshTreePath = useCallback((path: string) => {
        if (treeRef.current) {
            treeRef.current.refreshPath(path);
        }
    }, [treeRef]);

    const refreshBothPanels = useCallback(() => {
        left.refresh();
        if (layout === 'dual') {
            right.refresh();
        }
        if (treeRef.current) {
            if (left.path) treeRef.current.refreshPath(left.path);
            if (layout === 'dual' && right.path && right.path !== left.path) {
                treeRef.current.refreshPath(right.path);
            }
        }
    }, [left, right, layout, treeRef]);

    const initiateFileOp = useCallback(async (action: 'copy' | 'move', paths: string[], targetDir: string, turbo?: boolean) => {
        const success = await fileOps.initiateFileOp(action, paths, targetDir, turbo ?? defaultTurboMode);
        if (success) {
            refreshBothPanels();
            treeRef.current?.refreshPath(targetDir);
            if (action === 'move' && paths.length > 0) {
                const sourceParent = getParent(paths[0]);
                if (sourceParent) treeRef.current?.refreshPath(sourceParent);
            }
        }
        return success;
    }, [fileOps, refreshBothPanels, treeRef, defaultTurboMode]);

    const actionContextBase = useMemo(() => ({
        activePanelId,
        activePanel: activePanelId === 'left' ? left : right,
        otherPanel: activePanelId === 'left' ? right : left,
        fileOps,
        clipboard: {
            clipboard: clipboard.clipboard,
            copy: clipboard.copy,
            cut: clipboard.cut,
            clearClipboard: clipboard.clearClipboard,
            copyToSystem: clipboard.copyToSystem,
            refreshClipboard: clipboard.refreshClipboard
        },
        notify,
        t,
        dialogs,
        settings: {
            zipQuality,
            sevenZipQuality,
            zstdQuality,
            defaultTurboMode
        },
        refreshBothPanels,
        refreshTreePath,
        setContextMenu,
        peekStatus
    }), [activePanelId, left, right, fileOps, clipboard, notify, t, dialogs, zipQuality, sevenZipQuality, zstdQuality, defaultTurboMode, refreshBothPanels, refreshTreePath, setContextMenu, peekStatus]);

    const handleAction = useCallback(async (actionId: string, contextOverride?: Partial<ActionContext>) => {
        try {
            await actionService.execute(actionId, { ...actionContextBase, ...contextOverride } as ActionContext);
        } catch (e) {
            notify(`${t('error')}: ${formatCommandError(e)}`, 'error');
        }
    }, [actionContextBase, notify, t]);

    const handleUndo = useCallback(() => handleAction('file.undo'), [handleAction]);
    const handleRedo = useCallback(() => handleAction('file.redo'), [handleAction]);

    const handleNavigate = useCallback((id: PanelId, path: string) => {
        const panel = id === 'left' ? left : right;
        panel.navigate(path);
        setActivePanelId(id);
    }, [left, right, setActivePanelId]);

    const handleSearch = useCallback((id: PanelId, query: string) => {
        const panel = id === 'left' ? left : right;
        panel.setSearchQuery(query);
    }, [left, right]);

    const executeSearch = useCallback((id: PanelId) => {
        const panel = id === 'left' ? left : right;
        const query = panel.searchQuery;

        if (!query.trim()) {
            panel.setSearchResults(null);
            panel.setIsSearching(false);
            return;
        }

        const root = panel.currentSearchRoot || 'C:\\';
        // Prevent UI flash by eagerly resetting search state
        panel.setIsSearching(true);
        panel.setSearchResults(null);
        panel.navigate(`search://${encodeURIComponent(query)}?root=${encodeURIComponent(root)}`);
    }, [left, right]);

    const clearSearch = useCallback((id: PanelId) => {
        const panel = id === 'left' ? left : right;
        let root = (panel as any).currentSearchRoot || 'C:\\';

        const isSearch = panel.path.startsWith('search://') || panel.path.startsWith('search:\\\\');
        if (isSearch) {
            const searchPart = panel.path.startsWith('search://')
                ? panel.path.replace('search://', '')
                : panel.path.replace('search:\\\\', '');
            const querySepIndex = searchPart.indexOf('?');
            if (querySepIndex !== -1) {
                const params = new URLSearchParams(searchPart.substring(querySepIndex + 1));
                const uriRoot = params.get('root');
                if (uriRoot) root = uriRoot;
            }
        }

        panel.setSearchResults(null);
        panel.setSearchQuery("");
        panel.setIsSearching(false);
        panel.setSearchLimitReached(false);

        // If we were in a search view, navigate back to the origin folder
        if (isSearch) {
            panel.navigate(root);
        }
    }, [left, right]);

    const openAdvancedSearch = useCallback(async (id: PanelId) => {
        const panel = id === 'left' ? left : right;
        const initialRoot = panel.currentSearchRoot || 'C:\\';

        // Parse current URI to restore previous state
        let initialOptions: any = { query: panel.searchQuery || '' };
        if (panel.path.startsWith('search://') || panel.path.startsWith('search:\\\\')) {
            const searchPart = panel.path.startsWith('search://')
                ? panel.path.replace('search://', '')
                : panel.path.replace('search:\\\\', '');
            const querySepIndex = searchPart.indexOf('?');
            if (querySepIndex !== -1) {
                initialOptions.query = decodeURIComponent(searchPart.substring(0, querySepIndex));
                const params = new URLSearchParams(searchPart.substring(querySepIndex + 1));
                initialOptions.root = params.get('root') || initialRoot;
                initialOptions.regex = params.get('regex') === 'true';
                initialOptions.caseSensitive = params.get('case_sensitive') === 'true';
                initialOptions.recursive = params.get('recursive') !== 'false';
                initialOptions.minSize = params.get('min_size') ? parseInt(params.get('min_size')!) : undefined;
                initialOptions.maxSize = params.get('max_size') ? parseInt(params.get('max_size')!) : undefined;
                initialOptions.minDate = params.get('min_date') ? parseInt(params.get('min_date')!) : undefined;
                initialOptions.maxDate = params.get('max_date') ? parseInt(params.get('max_date')!) : undefined;
                initialOptions.contentQuery = params.get('content') || undefined;
                initialOptions.contentRegex = params.get('content_regex') === 'true';
                initialOptions.ignoreAccents = params.get('ignore_accents') === 'true';
                initialOptions.searchInArchives = params.get('search_in_archives') === 'true';
                initialOptions.sizeUnit = params.get('size_unit') || undefined;
            }
        }

        const options = await dialogs.openSearchDialog({ initialRoot, initialOptions });
        if (options) {
            // Build the advanced search URI
            const query = encodeURIComponent(options.query);
            const params = new URLSearchParams();
            params.set('root', options.root);
            if (options.regex) params.set('regex', 'true');
            if (options.caseSensitive) params.set('case_sensitive', 'true');
            if (options.recursive === false) params.set('recursive', 'false');
            if (options.minSize) params.set('min_size', options.minSize.toString());
            if (options.maxSize) params.set('max_size', options.maxSize.toString());
            if (options.minDate) params.set('min_date', options.minDate.toString());
            if (options.maxDate) params.set('max_date', options.maxDate.toString());
            if (options.contentQuery) params.set('content', options.contentQuery);
            if (options.contentRegex) params.set('content_regex', 'true');
            if (options.ignoreAccents) params.set('ignore_accents', 'true');
            if (options.searchInArchives) params.set('search_in_archives', 'true');
            if (options.sizeUnit) params.set('size_unit', options.sizeUnit);

            // Prevent UI flash
            panel.setIsSearching(true);
            panel.setSearchResults(null);
            panel.navigate(`search://${query}?${params.toString()}`);
        }
    }, [left, right, dialogs]);

    const openDuplicateSearchHandler = useCallback((id: PanelId) => {
        const panel = id === 'left' ? left : right;
        const initialRoot = panel.path.startsWith('search://') ? (panel as any).currentSearchRoot || 'C:\\' : panel.path;
        dialogs.openDuplicateSearch({ initialRoot });
    }, [left, right, dialogs]);

    const handleOpenFile = useCallback(async (path: string, panelId?: PanelId) => {
        const targetId = panelId || activePanelId;
        const panel = targetId === 'left' ? left : right;

        const ext = path.split('.').pop()?.toLowerCase() || '';
        const isArchive = ['zip', '7z', 'tar', 'tgz', 'txz', 'zst', 'rar', 'tbz2', 'tzst', 'gz', 'bz2', 'xz', 'iso', 'img', 'vhd', 'vhdx'].includes(ext);

        if (isArchive) {
            panel.navigate(path);
            return;
        }

        try {
            await invoke('open_item', { path });
        } catch (e) {
            notify(`${t('error')}: ${formatCommandError(e)}`, 'error');
        }
    }, [left, right, activePanelId, notify, t]);

    const handleSwapPanels = useCallback(() => {
        const lp = left.path;
        const rp = right.path;
        left.navigate(rp);
        right.navigate(lp);
    }, [left, right]);

    const handleSyncPanels = useCallback(() => {
        const activePath = activePanelId === 'left' ? left.path : right.path;
        left.navigate(activePath);
        right.navigate(activePath);
    }, [left, right, activePanelId]);

    const handleSort = useCallback((id: PanelId, field: SortField) => {
        const panel = id === 'left' ? left : right;
        const direction = panel.sortConfig.field === field && panel.sortConfig.direction === 'asc' ? 'desc' : 'asc';
        panel.setSortConfig({ field, direction });
    }, [left, right]);

    const handleSortDirection = useCallback((id: PanelId, direction: 'asc' | 'desc') => {
        const panel = id === 'left' ? left : right;
        panel.setSortConfig((prev: any) => ({ ...prev, direction }));
    }, [left, right]);

    const handleResize = useCallback((id: PanelId, field: string, newWidth: number) => {
        const panel = id === 'left' ? left : right;
        panel.setColWidths((prev: ColumnWidths) => ({
            ...prev,
            [field]: Math.max(30, newWidth)
        }));
    }, [left, right]);

    const handleResizeMultiple = useCallback((id: PanelId, updates: Partial<ColumnWidths>) => {
        const panel = id === 'left' ? left : right;
        panel.setColWidths((prev: ColumnWidths) => ({ ...prev, ...updates }));
    }, [left, right]);

    const handleGoToFolder = useCallback(async (path: string) => {
        const parent = getParent(path);
        if (parent) {
            const panel = contextMenu ? (contextMenu.panelId === 'left' ? left : right) : activePanel;
            panel.navigate(parent);
            setContextMenu(null);
        }
    }, [left, right, activePanel, contextMenu, setContextMenu]);

    const handleRestoreAll = useCallback(async () => {
        const panel = activePanel;
        try {
            const restoredPaths = await invoke<string[]>('restore_items', { paths: panel.files.map((f: FileEntry) => f.path) });
            notify(t('items_restored' as any), 'success');
            refreshBothPanels();

            if (restoredPaths && restoredPaths.length > 0) {
                restoredPaths.forEach(p => {
                    const parent = getParent(p);
                    if (parent) refreshTreePath(parent);
                });
            }
        } catch (e) {
            notify(`${t('error')}: ${formatCommandError(e)}`, 'error');
        }
    }, [activePanel, notify, t, refreshBothPanels, refreshTreePath]);

    const handleEmptyTrash = useCallback(async () => {
        const confirmed = await dialogs.confirm(t('confirm_empty_recycle_bin' as any), t('empty_recycle_bin' as any), true);
        if (confirmed) {
            try {
                await invoke('empty_trash');
                notify(t('recycle_bin_emptied' as any), 'success');
                refreshBothPanels();
            } catch (e) {
                notify(`${t('error')}: ${formatCommandError(e)}`, 'error');
            }
        }
    }, [dialogs, notify, t, refreshBothPanels]);

    const handleRestoreSelected = useCallback(async () => {
        const panel = contextMenu ? (contextMenu.panelId === 'left' ? left : right) : activePanel;
        const selected = Array.from(panel.selected);
        if (selected.length === 0) return;
        try {
            const restoredPaths = await invoke<string[]>('restore_items', { paths: selected });
            notify(`${selected.length} ${t('items_restored' as any)}`, 'success');
            refreshBothPanels();

            if (restoredPaths && restoredPaths.length > 0) {
                restoredPaths.forEach(p => {
                    const parent = getParent(p);
                    if (parent) refreshTreePath(parent);
                });
            }

            setContextMenu(null);
        } catch (e) {
            notify(`${t('error')}: ${formatCommandError(e)}`, 'error');
        }
    }, [left, right, activePanel, contextMenu, notify, t, refreshBothPanels, refreshTreePath, setContextMenu]);

    const handleTabSwitch = useCallback((targetId: string, overridePath?: string) => {
        if (targetId === activeTabId && !overridePath) return;
        setActiveTab(targetId);
        if (overridePath) {
            left.navigate(overridePath);
        }
    }, [activeTabId, left, setActiveTab]);

    const handleTabClose = useCallback((id: string) => {
        if (id !== activeTabId) {
            closeTab(id);
            return;
        }
        const nextTabs = tabs.filter(t => t.id !== id);
        if (nextTabs.length === 0) {
            closeTab(id);
        } else {
            const currentIndex = tabs.findIndex(t => t.id === id);
            const nextIndex = currentIndex > 0 ? currentIndex - 1 : 0;
            const nextTab = nextTabs[nextIndex];
            closeTab(id, nextTab.id);
        }
    }, [activeTabId, tabs, closeTab]);

    const handleItemMiddleClick = useCallback((entry: FileEntry) => {
        if (entry.is_dir && layout === 'standard') {
            addTab(entry.path, undefined, true);
        }
    }, [addTab, layout]);

    const handleAddToFavorites = useCallback(async (path: string) => {
        try {
            await invoke('add_to_quick_access', { path });
        } catch (e) {
            notify(`${t('error')}: ${formatCommandError(e)}`, 'error');
        }
    }, [notify, t]);

    const handleRemoveFromFavorites = useCallback(async (path: string) => {
        try {
            await invoke('remove_from_quick_access', { path });
        } catch (e) {
            notify(`${t('error')}: ${formatCommandError(e)}`, 'error');
        }
    }, [notify, t]);

    const handleDisconnectDrive = useCallback(async (path: string) => {
        const cleanLetter = path.replace(/[\\/]+$/, '');
        try {
            await invoke('disconnect_network_drive', { letter: cleanLetter, force: false });
            notify(t('disconnect_network_drive_success' as any), 'success');
            refreshDrives();
        } catch (e: any) {
            const errorStr = e.toString();
            if (errorStr.includes('2401')) {
                // ERROR_OPEN_FILES (2401) - Ask for forced disconnection
                const confirmed = await dialogs.confirm(
                    t('disconnect_network_drive_force_msg' as any) || "There are open files on this connection. Do you want to force disconnection anyway?",
                    t('disconnect_network_drive' as any),
                    true // isDanger
                );

                if (confirmed) {
                    try {
                        await invoke('disconnect_network_drive', { letter: cleanLetter, force: true });
                        notify(t('disconnect_network_drive_success' as any), 'success');
                        refreshDrives();
                    } catch (err) {
                        notify(`${t('error')}: ${formatCommandError(err)}`, 'error');
                    }
                }
            } else {
                notify(`${t('error')}: ${formatCommandError(e)}`, 'error');
            }
        }
    }, [notify, t, refreshDrives, dialogs]);

    const handleContextMenu = useCallback((e: React.MouseEvent, id: PanelId, entry?: FileEntry) => {
        e.preventDefault();
        e.stopPropagation();
        setActivePanelId(id);
        const panel = id === 'left' ? left : right;

        if (entry) {
            if (!panel.selected.has(entry.path)) {
                panel.handleSelect(entry.path, false, false);
            }
            setContextMenu({
                x: e.clientX,
                y: e.clientY,
                target: entry.path,
                panelId: id,
                isDir: entry.is_dir,
                isBackground: false,
                isDrive: (entry as any).isDrive,
                driveType: (entry as any).driveType,
                isMediaDevice: entry.is_media_device,
                isNetworkComputer: (entry.path.startsWith('\\\\') && entry.path.split('\\').filter(Boolean).length === 1) && !entry.is_media_device,
                hasWebPage: entry.has_web_page,
                isFavorite: entry.is_dir ? favorites.some(f => {
                    const fp = f.path.replace(/[\\/]+$/, '').toLowerCase();
                    const ep = entry.path.replace(/[\\/]+$/, '').toLowerCase();
                    return fp === ep;
                }) : false
            });
        } else {
            if (panel.path?.startsWith('trash://')) return;

            // Background context menu - check if we are at the root of a drive
            const path = panel.path;
            const isDriveRoot = /^[a-zA-Z]:[\\/]?$/.test(path);
            let driveType: DriveInfo['drive_type'] | undefined;

            if (isDriveRoot) {
                const drive = drives.find(d => {
                    const dPath = d.path.replace(/[\\/]$/, '').toLowerCase();
                    const pPath = path.replace(/[\\/]$/, '').toLowerCase();
                    return dPath === pPath;
                });
                if (drive) {
                    driveType = drive.drive_type;
                }
            }

            setContextMenu({
                x: e.clientX,
                y: e.clientY,
                target: path,
                panelId: id,
                isDir: true,
                isBackground: true,
                isDrive: isDriveRoot,
                driveType,
                isFavorite: favorites.some(f => {
                    const fp = f.path.replace(/[\\/]+$/, '').toLowerCase();
                    const pp = path.replace(/[\\/]+$/, '').toLowerCase();
                    return fp === pp;
                }),
                isNetworkComputer: (path.startsWith('\\\\') && path.split('\\').filter(Boolean).length === 1) || path.toLowerCase() === 'network'
            });
        }
    }, [left, right, setActivePanelId, setContextMenu, drives]);

    return {
        refreshBothPanels,
        initiateFileOp,
        handleUndo,
        handleRedo,
        handleNavigate,
        handleSearch,
        executeSearch,
        openAdvancedSearch,
        openDuplicateSearchHandler,
        clearSearch,
        handleOpenFile,
        handleAction,
        handleSwapPanels,
        handleSyncPanels,
        handleSort,
        handleSortDirection,
        handleResize,
        handleResizeMultiple,
        handleGoToFolder,
        handleRestoreAll,
        handleEmptyTrash,
        handleRestoreSelected,
        handleTabSwitch,
        handleTabClose,
        handleItemMiddleClick,
        handleContextMenu,
        handleDisconnectDrive,
        handleAddToFavorites,
        handleRemoveFromFavorites
    };
};
