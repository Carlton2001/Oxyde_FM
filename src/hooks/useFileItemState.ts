/**
 * useFileItemState — Shared state derivation for file items (DetailsRow & GridCell)
 * 
 * Extracts the common state computation and event handler pattern to reduce
 * duplication between DetailsRow and GridCell in VirtualizedFileList.
 */

import React, { useMemo } from 'react';
import { FileEntry, DateFormat } from '../types';
import { formatSize, formatDate, getFileTypeString } from '../utils/format';
import { isArchivePath } from '../utils/archive';
import { TFunc } from '../i18n';

interface UseFileItemStateProps {
    entry: FileEntry;
    selected: Set<string>;
    pendingSelection: Set<string>;
    renamingPath: string | null;
    isDragging: boolean;
    dragOverPath: string | null;
    cutPathsSet: Set<string>;
    diffPaths?: Set<string>;
    isTrashView: boolean;
    dateFormat: DateFormat;
    t: TFunc;
    onItemClick: (entry: FileEntry, e: React.MouseEvent) => void;
    onItemDoubleClick: (entry: FileEntry) => void;
    onItemContextMenu: (entry: FileEntry, e: React.MouseEvent) => void;
    onFileDragStart: (entry: FileEntry) => void;
    onItemMiddleClick?: (entry: FileEntry) => void;
    showCheckboxes: boolean;
}

export interface FileItemState {
    isSelected: boolean;
    isRenaming: boolean;
    isDropTarget: boolean;
    isCut: boolean;
    isDiff: boolean;
    tooltipText: string;
    itemClassName: string;
    handlers: {
        onClick: (e: React.MouseEvent) => void;
        onDoubleClick: (e: React.MouseEvent) => void;
        onContextMenu: (e: React.MouseEvent) => void;
        onDragStart: (e: React.DragEvent) => void;
        onMouseDown: (e: React.MouseEvent) => void;
        onCheckboxClick: (e: React.MouseEvent) => void;
    };
}

export function useFileItemState(props: UseFileItemStateProps): FileItemState {
    const {
        entry, selected, pendingSelection, renamingPath,
        isDragging, dragOverPath, cutPathsSet, diffPaths,
        isTrashView, dateFormat, t,
        onItemClick, onItemDoubleClick, onItemContextMenu,
        onFileDragStart, onItemMiddleClick
    } = props;

    const isSelected = selected.has(entry.path) || pendingSelection.has(entry.path);
    const isRenaming = renamingPath === entry.path;
    const isDropTarget = isDragging && (entry.is_dir || isArchivePath(entry.path)) && dragOverPath === entry.path;
    const isCut = cutPathsSet.has(entry.path);
    const isDiff = !!diffPaths?.has(entry.path);

    const tooltipText = useMemo(() => {
        let text = `${entry.name}\n${t('type')}: ${getFileTypeString(entry, t)}\n${t('size')}: ${entry.is_dir ? t('folder') : formatSize(entry.size, 1, t)}\n${t('date')}: ${formatDate(entry.modified, dateFormat)}`;
        if (isTrashView && entry.deleted_time) {
            text += `\n${t('date_deleted')}: ${formatDate(entry.deleted_time, dateFormat)}`;
        }
        return text;
    }, [entry, t, dateFormat, isTrashView]);

    const itemClassName = useMemo(() => {
        const parts: string[] = ['file-item'];
        if (isSelected) parts.push('selected');
        if (entry.is_hidden) parts.push('hidden');
        if (entry.is_system) parts.push('system-file');
        if (isDropTarget) parts.push('drop-target');
        if (isCut) parts.push('cut');
        if (isRenaming) parts.push('editing');
        if (isDiff) parts.push('diff');
        return parts.join(' ');
    }, [isSelected, entry.is_hidden, entry.is_system, isDropTarget, isCut, isRenaming, isDiff]);

    const handlers = useMemo(() => ({
        onClick: (e: React.MouseEvent) => onItemClick(entry, e),
        onDoubleClick: (e: React.MouseEvent) => {
            e.stopPropagation();
            if (isRenaming) return;
            onItemDoubleClick(entry);
        },
        onContextMenu: (e: React.MouseEvent) => {
            e.stopPropagation();
            if (isRenaming) return;
            onItemContextMenu(entry, e);
        },
        onDragStart: (e: React.DragEvent) => {
            e.preventDefault();
            if (isRenaming) return;
            onFileDragStart(entry);
        },
        onMouseDown: (e: React.MouseEvent) => {
            if (e.button === 1 && onItemMiddleClick) {
                e.preventDefault();
                e.stopPropagation();
                onItemMiddleClick(entry);
            }
        },
        onCheckboxClick: (e: React.MouseEvent) => {
            e.stopPropagation();
            const syntheticEvent = { ...e, ctrlKey: true, shiftKey: false, button: 0, detail: 1, stopPropagation: () => { } } as any;
            onItemClick(entry, syntheticEvent);
        }
    }), [entry, isRenaming, onItemClick, onItemDoubleClick, onItemContextMenu, onFileDragStart, onItemMiddleClick]);

    return { isSelected, isRenaming, isDropTarget, isCut, isDiff, tooltipText, itemClassName, handlers };
}
