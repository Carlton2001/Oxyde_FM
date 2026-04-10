import { invoke } from '@tauri-apps/api/core';
import {
    Copy, Scissors, Trash2, ClipboardPaste,
    ChevronDown, ChevronUp, Undo2, Redo2,
    FolderPlus, Edit2, Settings, ExternalLink, RotateCcw,
    FileArchive, Pin, ListOrdered, Check, MoreHorizontal, Globe, RefreshCw, Network, ServerOff, MousePointerSquareDashed
} from 'lucide-react';
import { TFunc } from '../../../i18n';
import { DriveInfo, SortConfig, SortField, SortDirection } from '../../../types';

const RotatedPin = (props: any) => <Pin {...props} style={{ ...props.style, transform: 'rotate(45deg)' }} />;
const RotatedPinOff = (props: any) => (
    <div className={props.className} style={{ position: 'relative', display: 'flex', alignItems: 'center', justifyContent: 'center', width: '1em', height: '1em' }}>
        <Pin {...props} className="" style={{ ...props.style, transform: 'rotate(45deg)', opacity: 0.7 }} />
        <div style={{
            position: 'absolute',
            width: '105%',
            height: '1px',
            backgroundColor: 'currentColor',
            transform: 'rotate(45deg)',
            pointerEvents: 'none',
            borderRadius: '1px'
        }} />
    </div>
);

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
    isTrashEmpty?: boolean;
    isNukeOverride?: boolean;
    isQuickAccessShortcut?: boolean;

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
        onRestoreAll?: () => void;
        onTrashProperties?: () => void;
        openMapNetworkDriveDialog?: () => void;
        openDisconnectNetworkDriveDialog?: () => void;
        onSelectAll?: () => void;
    }
}

const BlankIcon = () => <div className="icon-md" style={{ width: '1rem', height: '1rem' }} />;

export function getMenuItems(ctx: MenuContext): MenuItem[] {
    const { target, isDir, isTreeContext, isTrashContext, isBackground, isDrive, canPaste, canUndo, undoLabel, canRedo, redoLabel, t, actions, isShiftPressed, isInputContext, isTextSelected, isNukeOverride, showNetwork = true, isQuickAccessShortcut } = ctx;
    const items: MenuItem[] = [];

    if (isQuickAccessShortcut && target) {
        // Limited menu for Quick Access shortcuts
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
        items.push({ id: 'sep_fav', type: 'separator' });
        items.push({
            id: 'favorite_toggle',
            type: 'action',
            label: t('remove_from_favorites' as any),
            icon: RotatedPinOff,
            action: () => actions.onRemoveFromFavorites?.()
        });
        return items;
    }

    // --- Special Context: Input fields ---
    if (isInputContext) {
        items.push({
            id: 'cut',
            type: 'action',
            label: t('cut'),
            icon: Scissors,
            action: () => actions.onCut(),
            disabled: !isTextSelected,
            shortcut: 'Ctrl+X'
        });
        items.push({
            id: 'copy',
            type: 'action',
            label: t('copy'),
            icon: Copy,
            action: () => actions.onCopy(),
            disabled: !isTextSelected,
            shortcut: 'Ctrl+C'
        });
        items.push({
            id: 'paste',
            type: 'action',
            label: t('paste'),
            icon: ClipboardPaste,
            action: () => actions.onPaste(),
            shortcut: 'Ctrl+V'
        });
        items.push({ id: 'sep_input_1', type: 'separator' });
        items.push({
            id: 'select_all',
            type: 'action',
            label: t('select_all' as any) || 'Select All',
            icon: MousePointerSquareDashed,
            action: () => actions.onSelectAll?.(),
            shortcut: 'Ctrl+A'
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
                action: () => actions.onRefresh?.(),
                shortcut: 'F5'
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

    if (isTreeContext && !ctx.isNetworkComputer && !ctx.isFavorite && target !== 'trash://') {
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
                action: () => actions.onOpenFile?.(target),
                shortcut: 'Enter'
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
                    action: () => actions.onOpenFile?.(target),
                    shortcut: 'Enter'
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
                icon: ctx.isFavorite ? RotatedPinOff : RotatedPin,
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
            action: () => actions.onUndo(),
            shortcut: 'Ctrl+Z'
        });
    }
    if (canRedo && !ctx.isNetworkComputer) {
        items.push({
            id: 'redo',
            type: 'action',
            label: redoLabel || t('redo'),
            icon: Redo2,
            action: () => actions.onRedo(),
            shortcut: 'Ctrl+Y'
        });
    }

    // --- 4. Content Creation & Transformation ---
    if (!isTrashContext) {
        if (isBackground && !ctx.isNetworkComputer) {
            items.push({
                id: 'new_folder',
                type: 'action',
                label: t('new_folder'),
                icon: FolderPlus,
                action: () => actions.onNewFolder(),
                shortcut: 'Ctrl+Shift+N'
            });
            items.push({
                id: 'select_all',
                type: 'action',
                label: t('select_all' as any) || 'Select All',
                icon: MousePointerSquareDashed,
                action: () => actions.onSelectAll?.(),
                shortcut: 'Ctrl+A'
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
                    icon: FileArchive,
                    action: () => actions.onExtract?.(target, false)
                });
                items.push({
                    id: 'extract_to_folder',
                    type: 'action',
                    label: t('extract_to_folder' as any),
                    icon: FileArchive,
                    action: () => actions.onExtract?.(target, true)
                });
            } else if (!isDrive) {
                const submenu: MenuItem[] = [
                    { id: 'zip', type: 'action', label: 'Archive .zip', icon: FileArchive, action: () => actions.onCompress?.('zip') },
                    { id: '7z', type: 'action', label: 'Archive .7z', icon: FileArchive, action: () => actions.onCompress?.('7z') },
                    { id: 'tar', type: 'action', label: 'Archive .tar', icon: FileArchive, action: () => actions.onCompress?.('tar') },
                    { id: 'zst', type: 'action', label: 'Archive .zstd', icon: FileArchive, action: () => actions.onCompress?.('zst') }
                ];
                items.push({
                    id: 'compress',
                    type: 'submenu',
                    label: t('compress' as any),
                    icon: FileArchive,
                    children: submenu
                });
            }
        }
    }

    // --- 5. Clipboard & Modification ---
    if (!isTrashContext) {
        if (!isBackground && (!isDrive || isBackground) && !ctx.isMediaDevice && !ctx.isNetworkComputer) {
            items.push({
                id: 'cut',
                type: 'action',
                label: t('cut'),
                icon: Scissors,
                action: () => actions.onCut(),
                shortcut: 'Ctrl+X'
            });
            const copySubmenu: MenuItem[] = [
                { id: 'copy_name', type: 'action', label: t('copy_name' as any), icon: Copy, action: () => actions.onCopyName(), shortcut: 'Ctrl+Alt+C' },
                { id: 'copy_path', type: 'action', label: t('copy_path' as any), icon: Copy, action: () => actions.onCopyPath(), shortcut: 'Ctrl+Shift+C' }
            ];
            items.push({
                id: 'copy',
                type: 'submenu',
                label: t('copy'),
                icon: Copy,
                children: copySubmenu,
                action: () => actions.onCopy(),
                shortcut: 'Ctrl+C'
            });
        }

        if (canPaste && !ctx.isNetworkComputer && (!isDrive || isBackground)) {
            items.push({
                id: 'paste',
                type: 'action',
                label: t('paste'),
                icon: ClipboardPaste,
                action: () => actions.onPaste(),
                shortcut: 'Ctrl+V'
            });
        }

        if (target && !isBackground && (!isDrive || isBackground) && !ctx.isMediaDevice && !ctx.isNetworkComputer) {
            items.push({
                id: 'rename',
                type: 'action',
                label: t('rename'),
                icon: Edit2,
                action: () => actions.onRename(),
                shortcut: 'F2'
            });
        }
    }

    items.push({ id: 'sep_edit', type: 'separator' });

    // --- 6. Trash & Destructive Actions ---
    if (isTrashContext) {
        const isTrashRoot = target === 'trash://';

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
                label: t('perm_delete' as any),
                icon: Trash2,
                action: () => actions.onDelete(),
                color: 'var(--error-color)',
                danger: true,
                shortcut: 'Del'
            });
        }
        
        if (isTrashRoot) {
            if (actions.onRestoreAll && !ctx.isTrashEmpty) {
                items.push({
                    id: 'restore_all',
                    type: 'action',
                    label: t('restore_all' as any),
                    icon: RotateCcw,
                    action: () => actions.onRestoreAll?.()
                });
            }

            if (actions.onEmptyTrash && !ctx.isTrashEmpty) {
                items.push({
                    id: 'empty_trash',
                    type: 'action',
                    label: t('empty_recycle_bin' as any),
                    icon: Trash2,
                    action: () => actions.onEmptyTrash?.(),
                    color: 'var(--error-color)'
                });
            }
        }

        if (actions.onTrashProperties) {
            items.push({ id: 'sep_trash_props', type: 'separator' });
            items.push({
                id: 'trash_properties',
                type: 'action',
                label: t('properties'),
                icon: Settings,
                action: () => actions.onTrashProperties?.(),
                shortcut: 'Alt+Enter'
            });
        }
    } else {
        if (!isBackground && (!isDrive || isBackground) && !ctx.isMediaDevice && !ctx.isNetworkComputer) {
            items.push({
                id: 'delete',
                type: 'action',
                label: (isShiftPressed || isNukeOverride) ? t('perm_delete' as any) : t('delete'),
                icon: Trash2,
                action: () => actions.onDelete(),
                color: 'var(--error-color)',
                danger: true,
                shortcut: (isShiftPressed || isNukeOverride) ? 'Shift+Del' : 'Del'
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
            },
            shortcut: 'Alt+Enter'
        });
    }

    // --- 8. View & Sorting (e.g. for Grid View) ---
    if (ctx.sortConfig && actions.onSort) {
        items.push({ id: 'sep_sort', type: 'separator' });
        const sortItems: MenuItem[] = [
            { id: 'sort_name', type: 'action', label: t('name'), icon: ctx.sortConfig.field === 'name' ? Check : BlankIcon, action: () => actions.onSort?.('name') },
            { id: 'sort_ext', type: 'action', label: t('sort_ext' as any), icon: ctx.sortConfig.field === 'type' ? Check : BlankIcon, action: () => actions.onSort?.('type') },
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
