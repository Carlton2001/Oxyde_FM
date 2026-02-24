import React, { useState, useEffect, useMemo, useCallback, useRef } from 'react';
import { List, Grid, RowComponentProps, CellComponentProps } from 'react-window';
import { AutoSizer } from 'react-virtualized-auto-sizer';
import cx from 'classnames';
import { Check } from 'lucide-react';

import { FileEntry, ViewMode, DateFormat, ColumnWidths } from '../../types';
import { formatSize, formatDate, getFileTypeString } from '../../utils/format';
import { getParent } from '../../utils/path';
import { TFunc } from '../../i18n';
import { useApp } from '../../context/AppContext';
import { useFileItemState } from '../../hooks/useFileItemState';
import { RenameInput } from './RenameInput';

interface VirtualizedFileListProps {
    files: FileEntry[];
    viewMode: ViewMode;
    selected: Set<string>;
    pendingSelection: Set<string>;
    searchResults: FileEntry[] | null;
    renamingPath: string | null;
    renameText: string;
    isDragging: boolean;
    dragOverPath: string | null;
    cutPaths: string[];
    t: TFunc;
    onItemClick: (entry: FileEntry, e: React.MouseEvent) => void;
    onItemDoubleClick: (entry: FileEntry) => void;
    onItemContextMenu: (entry: FileEntry, e: React.MouseEvent) => void;
    onFileDragStart: (entry: FileEntry) => void;
    onRenameTextChange: (text: string) => void;
    onRenameCommit: () => void;
    onRenameCancel: () => void;
    getIcon: (entry: FileEntry, sizeOverride?: number) => React.ReactNode;
    totalItemsSize: number;
    showHistogram: boolean;
    isTrashView: boolean;
    onSelect?: (path: string, val: boolean, range: boolean) => void; // Deprecated, items use onItemClick
    onScrollToggle: (show: boolean) => void;
    onItemMiddleClick?: (entry: FileEntry) => void;
    diffPaths?: Set<string>;
    colWidths?: ColumnWidths;
}

export interface VirtualizedFileListHandle {
    scrollToTop: () => void;
}

// Shared props passed to row/cell components via rowProps/cellProps
interface SharedItemProps {
    entries: FileEntry[];
    selected: Set<string>;
    pendingSelection: Set<string>;
    renamingPath: string | null;
    renameText: string;
    isDragging: boolean;
    dragOverPath: string | null;
    cutPathsSet: Set<string>;
    searchResults: FileEntry[] | null;
    t: TFunc;
    onItemClick: (entry: FileEntry, e: React.MouseEvent) => void;
    onItemDoubleClick: (entry: FileEntry) => void;
    onItemContextMenu: (entry: FileEntry, e: React.MouseEvent) => void;
    onFileDragStart: (entry: FileEntry) => void;
    onRenameTextChange: (text: string) => void;
    onRenameCommit: () => void;
    onRenameCancel: () => void;
    getIcon: (entry: FileEntry, sizeOverride?: number) => React.ReactNode;
    totalItemsSize: number;
    showHistogram: boolean;
    isTrashView: boolean;
    onItemMiddleClick?: (entry: FileEntry) => void;
    dateFormat: DateFormat;
    diffPaths?: Set<string>;
    columnCount?: number;
    viewMode: ViewMode;
    rootFontSize: number;
    colWidths?: any;
    showCheckboxes: boolean;
}

// Memoized Details Row Component
const DetailsRow = React.memo((props: RowComponentProps<SharedItemProps>) => {
    const { index, style, ...sharedProps } = props as any;
    const {
        entries, selected, pendingSelection, renamingPath, renameText,
        isDragging, dragOverPath, cutPathsSet, searchResults,
        t, onItemClick, onItemDoubleClick, onItemContextMenu, onFileDragStart,
        onRenameTextChange, onRenameCommit, onRenameCancel, getIcon,
        totalItemsSize, showHistogram, isTrashView, onItemMiddleClick,
        dateFormat, diffPaths, rootFontSize, colWidths, showCheckboxes
    } = sharedProps;

    const entry = entries[index];
    if (!entry) return null;

    const { isSelected, isRenaming, tooltipText, itemClassName, handlers } = useFileItemState({
        entry, selected, pendingSelection, renamingPath,
        isDragging, dragOverPath, cutPathsSet, diffPaths,
        isTrashView, dateFormat, t,
        onItemClick, onItemDoubleClick, onItemContextMenu,
        onFileDragStart, onItemMiddleClick, showCheckboxes
    });

    const pureColumnWidth = useMemo(() => {
        if (!colWidths) return 0;
        let sum = colWidths.name + (colWidths.type || 0) + (colWidths.size || 0) + (colWidths.date || 0);
        if (searchResults) sum += (colWidths.location || 0);
        if (isTrashView) sum += (colWidths.location || 0) + (colWidths.deletedDate || 0);
        return sum;
    }, [colWidths, searchResults, isTrashView]);

    const safeParse = (val: any) => {
        if (typeof val === 'number') return isNaN(val) ? 0 : val;
        const p = parseFloat(val);
        return isNaN(p) ? 0 : p;
    };

    const adjustedStyle = {
        ...style,
        left: safeParse(style.left) + (rootFontSize * 0.5),
        width: pureColumnWidth > 0 ? pureColumnWidth : `calc(${safeParse(style.width)}px - ${rootFontSize}px)`,
        top: safeParse(style.top),
        height: safeParse(style.height),
        pointerEvents: 'auto' as const
    };

    return (
        <div
            className={itemClassName}
            data-path={entry.path}
            data-tooltip={tooltipText}
            data-tooltip-multiline="true"
            data-tooltip-image-path={entry.path}
            style={adjustedStyle}
            onClick={handlers.onClick}
            onDoubleClick={handlers.onDoubleClick}
            onContextMenu={handlers.onContextMenu}
            draggable={!isRenaming}
            onDragStart={handlers.onDragStart}
            onMouseDown={handlers.onMouseDown}
        >
            {showHistogram && (
                <div
                    className="size-histogram-bar"
                    style={{ width: `${(entry.size / totalItemsSize) * 100}%` }}
                />
            )}

            <div className="file-name-group">
                {showCheckboxes && (
                    <div className="item-checkbox" onClick={handlers.onCheckboxClick}>
                        <div className={cx("checkbox-indicator", { checked: isSelected })}>
                            {isSelected && <Check size={10} strokeWidth={4} />}
                        </div>
                    </div>
                )}
                <div className="file-icon-small">{getIcon(entry)}</div>
                <div className="file-name-container">
                    {isRenaming ? (
                        <RenameInput
                            renameText={renameText}
                            onRenameTextChange={onRenameTextChange}
                            onRenameCommit={onRenameCommit}
                            onRenameCancel={onRenameCancel}
                        />
                    ) : (
                        <span className="file-name">{entry.name}</span>
                    )}
                </div>
            </div>

            {searchResults && (
                <div className="file-info col-location">
                    <span className="text-truncate">{getParent(entry.path)}</span>
                </div>
            )}

            {isTrashView && (
                <>
                    <div className="file-info col-location">
                        <span className="text-truncate">{entry.original_path || ''}</span>
                    </div>
                    <div className="file-info col-date">
                        <span className="text-truncate">{entry.deleted_time ? formatDate(entry.deleted_time, dateFormat) : ''}</span>
                    </div>
                </>
            )}

            <div className="file-info col-type">
                <span className="text-truncate">{getFileTypeString(entry, t)}</span>
            </div>
            <div className="file-info col-size">
                <span className="text-truncate">
                    {entry.is_dir
                        ? (entry.is_calculated
                            ? (entry.size === 0 ? t('empty_dir') : formatSize(entry.size, 1, t))
                            : (entry.is_calculating ? t('calculating') : ''))
                        : formatSize(entry.size, 1, t)}
                </span>
            </div>
            <div className="file-info col-date">
                <span className="text-truncate">{formatDate(entry.modified, dateFormat)}</span>
            </div>
        </div>
    );
});

// Memoized Grid Item Component
const GridCell = React.memo((props: CellComponentProps<SharedItemProps>) => {
    const { columnIndex, rowIndex, style, ...sharedProps } = props as any;
    const {
        entries, selected, pendingSelection, renamingPath, renameText,
        isDragging, dragOverPath, cutPathsSet, showCheckboxes,
        t, onItemClick, onItemDoubleClick, onItemContextMenu, onFileDragStart,
        onRenameTextChange, onRenameCommit, onRenameCancel, getIcon,
        totalItemsSize, showHistogram, isTrashView, onItemMiddleClick,
        dateFormat, diffPaths, columnCount = 1, rootFontSize
    } = sharedProps;

    const index = rowIndex * columnCount + columnIndex;
    const entry = entries[index];
    if (!entry) return null;

    const { isSelected, isRenaming, tooltipText, itemClassName, handlers } = useFileItemState({
        entry, selected, pendingSelection, renamingPath,
        isDragging, dragOverPath, cutPathsSet, diffPaths,
        isTrashView, dateFormat, t,
        onItemClick, onItemDoubleClick, onItemContextMenu,
        onFileDragStart, onItemMiddleClick, showCheckboxes
    });

    const leftOffset = rootFontSize * 0.125;
    const topOffset = rootFontSize * 0.125;
    const widthReduction = rootFontSize * 0.25;
    const heightReduction = rootFontSize * 0.25;

    const safeParse = (val: any) => {
        if (typeof val === 'number') return isNaN(val) ? 0 : val;
        const p = parseFloat(val);
        return isNaN(p) ? 0 : p;
    };

    const adjustedStyle = {
        ...style,
        left: safeParse(style.left) + leftOffset,
        top: safeParse(style.top) + topOffset,
        width: safeParse(style.width) - widthReduction,
        height: safeParse(style.height) - heightReduction,
        pointerEvents: 'auto' as const
    };

    return (
        <div style={adjustedStyle} data-path={entry.path}>
            <div
                className={itemClassName}
                data-path={entry.path}
                data-tooltip={tooltipText}
                data-tooltip-multiline="true"
                data-tooltip-image-path={entry.path}
                onClick={handlers.onClick}
                onDoubleClick={handlers.onDoubleClick}
                onContextMenu={handlers.onContextMenu}
                draggable={!isRenaming}
                onDragStart={handlers.onDragStart}
                onMouseDown={handlers.onMouseDown}
                style={{ height: '100%', width: '100%', boxSizing: 'border-box' }}
            >
                {showHistogram && (
                    <div
                        className="size-histogram-bar"
                        style={{ width: `${(entry.size / totalItemsSize) * 100}%` }}
                    />
                )}

                {showCheckboxes && (
                    <div className="item-checkbox grid-checkbox" onClick={handlers.onCheckboxClick}>
                        <div className={cx("checkbox-indicator", { checked: isSelected })}>
                            {isSelected && <Check size={10} strokeWidth={4} />}
                        </div>
                    </div>
                )}
                <div className="file-icon">{getIcon(entry, 38)}</div>
                <div className="grid-name-wrapper">
                    {isRenaming ? (
                        <RenameInput
                            renameText={renameText}
                            onRenameTextChange={onRenameTextChange}
                            onRenameCommit={onRenameCommit}
                            onRenameCancel={onRenameCancel}
                            className="rename-input grid-mode"
                        />
                    ) : (
                        <div className="grid-name-container">
                            <span className="file-name">
                                {entry.is_dir ? entry.name : (() => {
                                    const lastDot = entry.name.lastIndexOf('.');
                                    return (lastDot > 0) ? entry.name.substring(0, lastDot) : entry.name;
                                })()}
                            </span>
                            {!entry.is_dir ? (
                                <span className="file-extension">
                                    {entry.name.includes('.') ? entry.name.split('.').pop()?.toUpperCase() : '\u00A0'}
                                </span>
                            ) : (
                                <span className="file-extension" style={{ visibility: 'hidden' }}>&nbsp;</span>
                            )}
                        </div>
                    )}
                </div>
            </div>
        </div>
    );
});

export const VirtualizedFileList = React.forwardRef<VirtualizedFileListHandle, VirtualizedFileListProps>((props, ref) => {
    const {
        viewMode, files,
        selected, pendingSelection, renamingPath, renameText,
        isDragging, dragOverPath, cutPaths,
        t, onItemClick, onItemDoubleClick, onItemContextMenu, onFileDragStart,
        onRenameTextChange, onRenameCommit, onRenameCancel, getIcon,
        totalItemsSize, showHistogram, isTrashView, searchResults,
        onScrollToggle, onItemMiddleClick,
        diffPaths, colWidths
    } = props;

    const { dateFormat, showCheckboxes } = useApp();

    const [rootFontSize, setRootFontSize] = useState(16);

    useEffect(() => {
        const ghost = document.createElement('div');
        ghost.style.width = '1rem';
        ghost.style.height = '1rem';
        ghost.style.position = 'absolute';
        ghost.style.visibility = 'hidden';
        ghost.style.pointerEvents = 'none';
        document.body.appendChild(ghost);

        const updateFontSize = () => {
            const size = ghost.getBoundingClientRect().width;
            if (size > 0) setRootFontSize(size);
        };

        const observer = new ResizeObserver(updateFontSize);
        observer.observe(ghost);
        updateFontSize();

        return () => {
            observer.disconnect();
            if (document.body.contains(ghost)) {
                document.body.removeChild(ghost);
            }
        };
    }, []);

    const isGrid = viewMode !== 'details';

    const handleScroll = useCallback((event: React.UIEvent<HTMLElement>) => {
        const top = event.currentTarget.scrollTop;
        if (top !== undefined) {
            const shouldShow = top > 300;
            onScrollToggle(shouldShow);
        }
    }, [onScrollToggle]);

    const listRef = useRef<any>(null);
    const gridRef = useRef<any>(null);

    const scrollToTop = () => {
        if (isGrid) {
            gridRef.current?.scrollToCell({ behavior: 'smooth', columnIndex: 0, rowIndex: 0 });
        } else {
            listRef.current?.scrollToRow({ behavior: 'smooth', index: 0 });
        }
    };

    React.useImperativeHandle(ref, () => ({
        scrollToTop
    }));

    const cutPathsSet = useMemo(() => new Set(cutPaths), [cutPaths]);

    const sharedProps: SharedItemProps = useMemo(() => ({
        entries: files, selected, pendingSelection, renamingPath, renameText,
        isDragging, dragOverPath, cutPathsSet, searchResults,
        t, onItemClick, onItemDoubleClick, onItemContextMenu, onFileDragStart,
        onRenameTextChange, onRenameCommit, onRenameCancel, getIcon,
        totalItemsSize, showHistogram, isTrashView, onItemMiddleClick,
        dateFormat, diffPaths, viewMode, rootFontSize, colWidths, showCheckboxes
    }), [
        files, selected, pendingSelection, renamingPath, renameText,
        isDragging, dragOverPath, cutPathsSet, searchResults,
        t, onItemClick, onItemDoubleClick, onItemContextMenu, onFileDragStart,
        onRenameTextChange, onRenameCommit, onRenameCancel, getIcon,
        totalItemsSize, showHistogram, isTrashView, onItemMiddleClick,
        dateFormat, diffPaths, viewMode, rootFontSize, colWidths, showCheckboxes
    ]);

    const listRowHeight = rootFontSize * 1.75;
    const gridRowHeightBase = rootFontSize * 6.0; // Reduced from 7.0
    const gridGap = rootFontSize * 0.25;

    const totalColumnWidth = useMemo(() => {
        if (!colWidths || isGrid) return 0;
        let sum = colWidths.name + (colWidths.type || 0) + (colWidths.size || 0) + (colWidths.date || 0);
        if (searchResults) sum += (colWidths.location || 0);
        if (isTrashView) sum += (colWidths.location || 0) + (colWidths.deletedDate || 0);
        return sum + (rootFontSize * 2.5);
    }, [colWidths, isGrid, searchResults, isTrashView, rootFontSize]);

    return (
        <div
            className={cx("virtualized-list", {
                "details": !isGrid,
                "grid": isGrid,
                "search-mode": !!props.searchResults,
                "trash-mode": props.isTrashView
            })}
            style={{ width: '100%', height: '100%', overflowX: 'auto', overflowY: 'hidden' }}
        >
            <AutoSizer renderProp={({ height, width }: { height: number | undefined; width: number | undefined }) => {
                if (!height || !width) return null;

                if (isGrid) {
                    const minColumnWidth = rootFontSize * 6.0; // More balanced with 6.0 height
                    const horizontalPadding = rootFontSize * 2.5;
                    const columnCount = Math.max(1, Math.floor((width - horizontalPadding) / (minColumnWidth + gridGap)));
                    const rowCount = Math.ceil(files.length / columnCount);
                    const columnWidth = (width - horizontalPadding - (columnCount - 1) * gridGap) / columnCount;

                    return (
                        <Grid
                            key={`grid-${rootFontSize}-${columnCount}`} // Force refresh on font or layout change
                            className="virtualized-scroller grid"
                            columnCount={columnCount}
                            columnWidth={columnWidth + gridGap}
                            rowCount={rowCount}
                            rowHeight={gridRowHeightBase + gridGap}
                            cellComponent={GridCell as any}
                            cellProps={{ ...sharedProps, columnCount }}
                            gridRef={gridRef}
                            style={{ height, width, overflowX: 'hidden', overflowY: 'auto' }}
                            onScroll={handleScroll}
                        />
                    );
                }

                const finalWidth = Math.max(width, totalColumnWidth);

                return (
                    <List
                        key={`list-${rootFontSize}`} // Force refresh on font change
                        className="virtualized-scroller details"
                        rowCount={files.length}
                        rowHeight={listRowHeight}
                        rowComponent={DetailsRow as any}
                        rowProps={sharedProps}
                        listRef={listRef}
                        style={{ height, width: finalWidth, overflowY: 'auto', overflowX: 'auto' }}
                        onScroll={handleScroll}
                    />
                );
            }} />
        </div>
    );
});

