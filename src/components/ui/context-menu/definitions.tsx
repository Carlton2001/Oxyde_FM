import { invoke } from '@tauri-apps/api/core';
import {
    Copy, Scissors, Trash2, ClipboardPaste,
    ChevronDown, ChevronUp, Undo2, Redo2,
    FolderPlus, Edit2, Settings, ExternalLink, RotateCcw,
    Archive, Box, FileArchive, Star, ListOrdered, Check, MoreHorizontal, Globe, RefreshCw, Network, ServerOff
} from 'lucide-react';
import { TFunc } from '../../../i18n';
import { DriveInfo, SortConfig, SortField, SortDirection } from '../../../types';

export interface MenuAction {
    id: string;
    label: string;
    icon?: any;
    action: () => void;
    disabled?: boolean;
    shortcut?: string;
    color?: string;
}

export interface MenuSeparator {
    id: string;
    type: 'separator';
}

export interface MenuItem extends Partial<MenuAction> {
    type: 'action' | 'separator' | 'submenu' | 'native_menu';
    submenu?: MenuItem[];
    children?: MenuItem[];
    danger?: boolean;
    color?: string; // Explicit color override
    data?: any;
}

export interface MenuContext {
    target?: string;
    isDir?: boolean;
    isTreeContext?: boolean;
    isTrashContext?: boolean;
    isSearchContext?: boolean;
    isBackground?: boolean;
    isDrive?: boolean;
    isMediaDevice?: boolean;
    isNetworkComputer?: boolean;
    hasWebPage?: boolean;
    driveType?: DriveInfo['drive_type'];
    isReadOnly?: boolean;
    canPaste?: boolean;
    isShiftPressed?: boolean;
    isFavorite?: boolean;
    isImageMounted?: boolean;
    isInputContext?: boolean;
    isTextSelected?: boolean;

    canUndo: boolean;
    undoLabel?: string;
    canRedo: boolean;
    redoLabel?: string;

    sortConfig?: SortConfig;

    showNetwork?: boolean;
    t: TFunc;
    onClose: () => void;

    actions: {
        onRefresh: () => void;
        onUndo: () => void;
        onRedo: () => void;
        onCopy: () => void;
        onCut: () => void;
        onPaste: () => void;
        onDelete: () => void;
        onRename: () => void;
        onProperties: () => void;
        onNewFolder: () => void;
        onCopyName: () => void;
        onCopyPath: () => void;
        onGoToFolder?: (path: string) => void;
        onRestore?: () => void;
        onExpandAll?: () => void;
        onCollapseAll?: () => void;
        onOpenNewTab?: (path: string) => void;
        onOpenFile?: (path: string) => void;
        onExtract?: (path: string, toSubfolder: boolean) => void;
        onCompress?: (format: 'zip' | '7z' | 'tar' | 'zst') => void;
        onMount?: () => void;
        onUnmount?: () => void;
        onAddToFavorites?: () => void;
        onRemoveFromFavorites?: () => void;
        onSort?: (field: SortField) => void;
        onSortDirection?: (direction: SortDirection) => void;
        onDisconnectDrive?: (letter: string) => void;
        onEmptyTrash?: () => void;
        openMapNetworkDriveDialog?: () => void;
        openDisconnectNetworkDriveDialog?: () => void;
        onSelectAll?: () => void;
    }
}

const BlankIcon = () => <div className="icon-md" style={{ width: '1rem', height: '1rem' }} />;

export function getMenuItems(ctx: MenuContext): MenuItem[] {
    const { target, isDir, isTreeContext, isTrashContext, isBackground, isDrive, canPaste, canUndo, undoLabel, canRedo, redoLabel, t, actions, isShiftPressed, isInputContext, isTextSelected, showNetwork = true } = ctx;
    const items: MenuItem[] = [];

    // --- Special Context: Input fields ---
    if (isInputContext) {
        items.push({
            id: 'cut',
            type: 'action',
            label: t('cut'),
            icon: Scissors,
            action: () => actions.onCut(),
            disabled: !isTextSelected
        });
        items.push({
            id: 'copy',
            type: 'action',
            label: t('copy'),
            icon: Copy,
            action: () => actions.onCopy(),
            disabled: !isTextSelected
        });
        items.push({
            id: 'paste',
            type: 'action',
            label: t('paste'),
            icon: ClipboardPaste,
            action: () => actions.onPaste()
        });
        items.push({ id: 'sep_input_1', type: 'separator' });
        items.push({
            id: 'select_all',
            type: 'action',
            label: t('select_all' as any) || 'Select All',
            icon: BlankIcon,
            action: () => actions.onSelectAll?.()
        });

        return items;
    }

    // --- Special Context: Voisinage Réseau ---
    if (target === '__network_vincinity__' && showNetwork) {
        if (actions.onExpandAll || actions.onCollapseAll) {
            items.push({
                id: 'expand_all',
                type: 'action',
                label: t('expand_all' as any),
                icon: ChevronDown,
                action: () => actions.onExpandAll?.()
            });
            items.push({
                id: 'collapse_all',
                type: 'action',
                label: t('collapse_all' as any),
                icon: ChevronUp,
                action: () => actions.onCollapseAll?.()
            });
            items.push({ id: 'sep_network_nav', type: 'separator' });
        }
        if (actions.onRefresh) {
            items.push({
                id: 'refresh',
                type: 'action',
                label: t('refresh'),
                icon: RefreshCw,
                action: () => actions.onRefresh?.()
            });
        }
        if (actions.onOpenNewTab) {
            items.push({
                id: 'open_new_tab',
                type: 'action',
                label: t('open_in_new_tab'),
                icon: ExternalLink,
                action: () => actions.onOpenNewTab?.(target)
            });
        }
        items.push({ id: 'sep_network_drives', type: 'separator' });
        if (actions.openMapNetworkDriveDialog) {
            items.push({
                id: 'map_network_drive',
                type: 'action',
                label: t('map_network_drive' as any),
                icon: Network,
                action: () => actions.openMapNetworkDriveDialog?.()
            });
        }
        if (actions.openDisconnectNetworkDriveDialog) {
            items.push({
                id: 'disconnect_network_drive',
                type: 'action',
                label: t('disconnect_network_drive' as any),
                icon: ServerOff,
                action: () => actions.openDisconnectNetworkDriveDialog?.()
            });
        }

        return items;
    }

    // --- 1. Navigation & Special Context Actions (TOP) ---
    if (ctx.isSearchContext && target && !isBackground && !ctx.isNetworkComputer) {
        items.push({
            id: 'go_to_folder',
            type: 'action',
            label: t('go_to_folder' as any),
            icon: ExternalLink,
            action: () => actions.onGoToFolder?.(target)
        });
    }

    if (isTreeContext && !ctx.isNetworkComputer && target !== 'trash://') {
        items.push({
            id: 'expand_all',
            type: 'action',
            label: t('expand_all' as any),
            icon: ChevronDown,
            action: () => actions.onExpandAll?.()
        });
        items.push({
            id: 'collapse_all',
            type: 'action',
            label: t('collapse_all' as any),
            icon: ChevronUp,
            action: () => actions.onCollapseAll?.()
        });
    }

    if (target && !isBackground) {
        // Allow Open for everything except files that are in the Trash
        // (but allow opening the Trash Root or folders inside the Trash)
        const canOpen = !isTrashContext || isDir;

        if (canOpen && isDir) {
            items.push({
                id: 'open_file',
                type: 'action',
                label: t('open'),
                icon: ExternalLink,
                action: () => actions.onOpenFile?.(target)
            });
            if (actions.onOpenNewTab) {
                items.push({
                    id: 'open_new_tab',
                    type: 'action',
                    label: t('open_in_new_tab'),
                    icon: ExternalLink,
                    action: () => actions.onOpenNewTab?.(target)
                });
            }
        } else if (!isDrive) {
            if (ctx.isMediaDevice) {
                if (ctx.hasWebPage) {
                    items.push({
                        id: 'open_file',
                        type: 'action',
                        label: t('view_device_webpage' as any),
                        icon: Globe,
                        action: () => actions.onOpenFile?.(target)
                    });
                }
            } else {
                items.push({
                    id: 'open_file',
                    type: 'action',
                    label: t('open'),
                    icon: ExternalLink,
                    action: () => actions.onOpenFile?.(target)
                });
            }
        }

        // If it's a network computer, we allow it to fall through to get Properties/Sort/etc
        // but we'll filter out other sections using the isNetworkComputer flag
    }

    items.push({ id: 'sep_nav', type: 'separator' });

    // --- 2. System & State (Mount, Favorites) ---
    if (target && !isBackground && !isTrashContext) {
        const imageExts = ['.iso', '.vhd', '.vhdx', '.img', '.bin', '.mdf', '.nrg', '.ccd', '.cue', '.isz'];
        const isImage = imageExts.some(ext => target.toLowerCase().endsWith(ext));

        if (isImage && !isDir) {
            if (ctx.isImageMounted) {
                items.push({
                    id: 'unmount_file',
                    type: 'action',
                    label: t('unmount' as any) || 'Éjecter',
                    icon: ExternalLink,
                    action: () => actions.onUnmount?.()
                });
            } else {
                items.push({
                    id: 'mount',
                    type: 'action',
                    label: t('mount' as any) || 'Monter',
                    icon: ExternalLink,
                    action: () => actions.onMount?.()
                });
            }
        }

        if (isDrive && (ctx.driveType === 'removable' || ctx.driveType === 'cdrom')) {
            items.push({
                id: 'unmount_drive',
                type: 'action',
                label: t('unmount' as any) || 'Éjecter',
                icon: ExternalLink,
                action: () => actions.onUnmount?.()
            });
        }

        if (isDir && !isTrashContext && target !== '__network_vincinity__' && !ctx.isMediaDevice) {
            items.push({
                id: 'favorite_toggle',
                type: 'action',
                label: ctx.isFavorite ? t('remove_from_favorites' as any) : t('add_to_favorites' as any),
                icon: Star,
                action: () => ctx.isFavorite ? actions.onRemoveFromFavorites?.() : actions.onAddToFavorites?.()
            });
        }

        if (isDrive && ctx.driveType === 'remote' && actions.onDisconnectDrive) {
            items.push({
                id: 'disconnect_network_drive_item',
                type: 'action',
                label: t('disconnect_network_drive' as any),
                icon: ServerOff,
                action: () => actions.onDisconnectDrive?.(target!)
            });
        }
    }

    // --- 3. History (Undo / Redo) ---
    if (canUndo && !ctx.isNetworkComputer) {
        items.push({
            id: 'undo',
            type: 'action',
            label: undoLabel || t('undo'),
            icon: Undo2,
            action: () => actions.onUndo()
        });
    }
    if (canRedo && !ctx.isNetworkComputer) {
        items.push({
            id: 'redo',
            type: 'action',
            label: redoLabel || t('redo'),
            icon: Redo2,
            action: () => actions.onRedo()
        });
    }

    items.push({ id: 'sep_system', type: 'separator' });

    // --- 4. Content Creation & Transformation ---
    if (!isTrashContext) {
        if (isBackground && !ctx.isNetworkComputer) {
            items.push({
                id: 'new_folder',
                type: 'action',
                label: t('new_folder'),
                icon: FolderPlus,
                action: () => actions.onNewFolder()
            });
        }

        if (target && !isBackground && !ctx.isMediaDevice && !ctx.isNetworkComputer) {
            const ext = target.split('.').pop()?.toLowerCase() || '';
            const isArchive = ['zip', '7z', 'tar', 'tgz', 'txz', 'zst', 'rar', 'tbz2', 'tzst', 'gz', 'bz2', 'xz', 'iso', 'img'].includes(ext);

            if (isArchive) {
                items.push({
                    id: 'extract_here',
                    type: 'action',
                    label: t('extract_here' as any),
                    icon: Archive,
                    action: () => actions.onExtract?.(target, false)
                });
                items.push({
                    id: 'extract_to_folder',
                    type: 'action',
                    label: t('extract_to_folder' as any),
                    icon: Archive,
                    action: () => actions.onExtract?.(target, true)
                });
            } else if (!isDrive) {
                const submenu: MenuItem[] = [
                    { id: 'zip', type: 'action', label: 'Archive .zip', icon: Box, action: () => actions.onCompress?.('zip') },
                    { id: '7z', type: 'action', label: 'Archive .7z', icon: FileArchive, action: () => actions.onCompress?.('7z') },
                    { id: 'tar', type: 'action', label: 'Archive .tar', icon: FileArchive, action: () => actions.onCompress?.('tar') },
                    { id: 'zst', type: 'action', label: 'Archive .zstd', icon: FileArchive, action: () => actions.onCompress?.('zst') }
                ];
                items.push({
                    id: 'compress',
                    type: 'submenu',
                    label: t('compress' as any),
                    icon: Box,
                    children: submenu
                });
            }
        }
    }

    items.push({ id: 'sep_creation', type: 'separator' });

    // --- 5. Clipboard & Modification ---
    if (!isTrashContext) {
        if (!isBackground && !isDrive && !ctx.isMediaDevice && !ctx.isNetworkComputer) {
            items.push({
                id: 'cut',
                type: 'action',
                label: t('cut'),
                icon: Scissors,
                action: () => actions.onCut()
            });
            const copySubmenu: MenuItem[] = [
                { id: 'copy_name', type: 'action', label: t('copy_name' as any), icon: Copy, action: () => actions.onCopyName() },
                { id: 'copy_path', type: 'action', label: t('copy_path' as any), icon: Copy, action: () => actions.onCopyPath() }
            ];
            items.push({
                id: 'copy',
                type: 'submenu',
                label: t('copy'),
                icon: Copy,
                children: copySubmenu,
                action: () => actions.onCopy()
            });
        }

        if (canPaste && !ctx.isNetworkComputer && !isDrive) {
            items.push({
                id: 'paste',
                type: 'action',
                label: t('paste'),
                icon: ClipboardPaste,
                action: () => actions.onPaste()
            });
        }

        if (target && !isBackground && !isDrive && !ctx.isMediaDevice && !ctx.isNetworkComputer) {
            items.push({
                id: 'rename',
                type: 'action',
                label: t('rename'),
                icon: Edit2,
                action: () => actions.onRename()
            });
        }
    }

    items.push({ id: 'sep_edit', type: 'separator' });

    // --- 6. Trash & Destructive Actions ---
    if (isTrashContext) {
        const isTrashRoot = target === 'trash://';

        if (actions.onEmptyTrash) {
            items.push({
                id: 'empty_trash',
                type: 'action',
                label: t('empty_recycle_bin' as any),
                icon: Trash2,
                action: () => actions.onEmptyTrash?.(),
                danger: true
            });
            items.push({ id: 'sep_trash_ops', type: 'separator' });
        }

        if (!isTrashRoot) {
            items.push({
                id: 'restore',
                type: 'action',
                label: t('restore' as any),
                icon: RotateCcw,
                action: () => actions.onRestore?.()
            });
            items.push({
                id: 'delete_perm',
                type: 'action',
                label: t('delete'),
                icon: Trash2,
                action: () => actions.onDelete(),
                color: 'var(--error-color)',
                danger: true
            });
        }
    } else {
        if (!isBackground && !isDrive && !ctx.isMediaDevice && !ctx.isNetworkComputer) {
            items.push({
                id: 'delete',
                type: 'action',
                label: isShiftPressed ? t('perm_delete' as any) : t('delete'),
                icon: Trash2,
                action: () => actions.onDelete(),
                color: 'var(--error-color)',
                danger: true
            });
        }
    }

    // --- 7. Information (ALWAYS LAST) ---
    if (target && !ctx.isNetworkComputer && target !== 'trash://') {
        items.push({ id: 'sep_properties', type: 'separator' });
        items.push({
            id: 'properties',
            type: 'action',
            label: t('properties'),
            icon: Settings,
            action: () => {
                if (ctx.isMediaDevice || target.startsWith('::{')) {
                    invoke('show_system_properties', { path: target }).catch(console.error);
                } else {
                    actions.onProperties();
                }
            }
        });
    }

    // --- 8. View & Sorting (e.g. for Grid View) ---
    if (ctx.sortConfig && actions.onSort) {
        items.push({ id: 'sep_sort', type: 'separator' });
        const sortItems: MenuItem[] = [
            { id: 'sort_name', type: 'action', label: t('name'), icon: ctx.sortConfig.field === 'name' ? Check : BlankIcon, action: () => actions.onSort?.('name') },
            { id: 'sort_ext', type: 'action', label: t('extension' as any), icon: ctx.sortConfig.field === 'type' ? Check : BlankIcon, action: () => actions.onSort?.('type') },
            { id: 'sort_size', type: 'action', label: t('size'), icon: ctx.sortConfig.field === 'size' ? Check : BlankIcon, action: () => actions.onSort?.('size') },
            { id: 'sort_date', type: 'action', label: t('date'), icon: ctx.sortConfig.field === 'date' ? Check : BlankIcon, action: () => actions.onSort?.('date') },
            { id: 'sep_sort_dir', type: 'separator' },
            { id: 'sort_asc', type: 'action', label: t('sort_asc' as any), icon: ctx.sortConfig.direction === 'asc' ? Check : BlankIcon, action: () => actions.onSortDirection?.('asc') },
            { id: 'sort_desc', type: 'action', label: t('sort_desc' as any), icon: ctx.sortConfig.direction === 'desc' ? Check : BlankIcon, action: () => actions.onSortDirection?.('desc') },
        ];
        items.push({
            id: 'sort_by',
            type: 'submenu',
            label: t('sort_by' as any),
            icon: ListOrdered,
            children: sortItems
        });
    }

    // --- 9. Native Shell Integration ---
    if (target && !isTrashContext && !ctx.isMediaDevice && !ctx.isNetworkComputer) {
        items.push({ id: 'sep_native', type: 'separator' });
        items.push({
            id: 'more_options',
            type: 'native_menu',
            label: t('more_options' as any),
            icon: MoreHorizontal,
            data: { target, isBackground: !!isBackground }
        });
    }

    // --- Cleanup ---
    let result: MenuItem[] = [];
    items.forEach((item) => {
        if (item.type === 'separator') {
            if (result.length > 0 && result[result.length - 1].type !== 'separator') {
                result.push(item);
            }
        } else {
            result.push(item);
        }
    });

    // 2. Second pass: Remove leading and trailing separators
    while (result.length > 0 && result[0].type === 'separator') {
        result.shift();
    }
    while (result.length > 0 && result[result.length - 1].type === 'separator') {
        result.pop();
    }

    return result;
}
