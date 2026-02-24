
import React, { useMemo } from 'react';
import './ContextMenu.css';
import { TFunc } from '../../i18n';
import { DriveInfo, SortConfig, SortField, SortDirection } from '../../types';
import { useApp } from '../../context/AppContext';
import { getMenuItems, MenuContext } from './context-menu/definitions';
import { ContextMenuView } from './context-menu/ContextMenuView';

export interface ContextMenuProps {
    x: number;
    y: number;
    target?: string;

    canUndo: boolean;
    undoLabel?: string;
    canRedo: boolean;
    redoLabel?: string;
    onClose: () => void;
    onRefresh: () => void;
    onUndo: () => void;
    onRedo: () => void;
    onCopy: () => void;
    onCut: () => void;
    onPaste: () => void;
    canPaste: boolean;
    onDelete: () => void;
    onRename: () => void;
    onProperties: () => void;
    onNewFolder: () => void;
    onCopyName: () => void;
    onCopyPath: () => void;
    onGoToFolder?: (path: string) => void;
    t: TFunc;
    // Trash context
    isTrashContext?: boolean;
    isSearchContext?: boolean;
    onRestore?: () => void;
    // Tree context (DirectoryTree)
    isTreeContext?: boolean;
    onExpandAll?: () => void;
    onCollapseAll?: () => void;
    // Tabs
    onOpenNewTab?: (path: string) => void;
    onOpenFile?: (path: string) => void;
    isDir?: boolean;
    isBackground?: boolean;
    isDrive?: boolean;
    driveType?: DriveInfo['drive_type']; // 'fixed' | 'removable' | 'remote' | 'cdrom' | 'unknown';
    onExtract?: (path: string, toSubfolder: boolean) => void;
    onCompress?: (format: 'zip' | '7z' | 'tar' | 'zst') => void;
    onMount?: () => void;
    onUnmount?: () => void;
    isReadOnly?: boolean;
    isShiftPressed?: boolean;
    isFavorite?: boolean;
    onAddToFavorites?: () => void;
    onRemoveFromFavorites?: () => void;
    sortConfig?: SortConfig;
    onSort?: (field: SortField) => void;
    onSortDirection?: (direction: SortDirection) => void;
}


export const ContextMenu: React.FC<ContextMenuProps> = (props) => {
    const { mountedImages } = useApp();

    // Memoize the context and items generation to avoid recalculations if props haven't changed meaningfully
    // though usually a context menu is mounted once and then unmounted.

    // Explicit mapping to avoid passing everything blindly
    const context: MenuContext = useMemo(() => ({
        target: props.target,
        isDir: props.isDir,
        isTreeContext: props.isTreeContext,
        isTrashContext: props.isTrashContext,
        isSearchContext: props.isSearchContext,
        isBackground: props.isBackground,
        isDrive: props.isDrive,
        driveType: props.driveType,
        isReadOnly: props.isReadOnly,
        canPaste: props.canPaste,
        isShiftPressed: props.isShiftPressed,
        isFavorite: props.isFavorite,
        isImageMounted: props.target ? mountedImages.some(img => img.toLowerCase().replace(/\\/g, '/') === props.target!.toLowerCase().replace(/\\/g, '/')) : false,

        canUndo: props.canUndo,
        undoLabel: props.undoLabel,
        canRedo: props.canRedo,
        redoLabel: props.redoLabel,
        sortConfig: props.sortConfig,
        t: props.t,
        onClose: props.onClose,
        actions: {
            onRefresh: props.onRefresh,
            onUndo: props.onUndo,
            onRedo: props.onRedo,
            onCopy: props.onCopy,
            onCut: props.onCut,
            onPaste: props.onPaste,
            onDelete: props.onDelete,
            onRename: props.onRename,
            onProperties: props.onProperties,
            onNewFolder: props.onNewFolder,
            onCopyName: props.onCopyName,
            onCopyPath: props.onCopyPath,
            onGoToFolder: props.onGoToFolder,
            onRestore: props.onRestore,
            onExpandAll: props.onExpandAll,
            onCollapseAll: props.onCollapseAll,
            onOpenNewTab: props.onOpenNewTab,
            onOpenFile: props.onOpenFile,
            onExtract: props.onExtract,
            onCompress: props.onCompress,
            onMount: props.onMount,
            onUnmount: props.onUnmount,
            onAddToFavorites: props.onAddToFavorites,
            onRemoveFromFavorites: props.onRemoveFromFavorites,
            onSort: props.onSort,
            onSortDirection: props.onSortDirection
        }
    }), [
        props.target, props.isDir, props.isTreeContext, props.isTrashContext,
        props.isBackground, props.isDrive, props.driveType, props.isReadOnly,
        props.canPaste, props.canUndo, props.undoLabel, props.canRedo, props.redoLabel, props.t,
        props.onClose, props.onRefresh, props.onUndo, props.onRedo, props.isSearchContext,
        props.isFavorite, props.onAddToFavorites, props.onRemoveFromFavorites, mountedImages
    ]);

    const items = useMemo(() => getMenuItems(context), [context]);

    return (
        <ContextMenuView
            items={items}
            x={props.x}
            y={props.y}
            onClose={props.onClose}
        />
    );
};

