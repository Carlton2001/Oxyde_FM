import React, { useState, useEffect, useMemo, useCallback, useRef, useImperativeHandle } from 'react';
import { List, Grid, RowComponentProps, CellComponentProps } from 'react-window';
import { AutoSizer } from 'react-virtualized-auto-sizer';
import cx from 'classnames';
import { Check, Shield, Loader2 } from 'lucide-react';

import { FileEntry, ViewMode, ColumnWidths, DateFormat } from '../../types';
import { TFunc } from '../../i18n';
import { useApp } from '../../context/AppContext';
import { useFileItemState } from '../../hooks/useFileItemState';
import { RenameInput } from './RenameInput';
import { getColumnMode, getVisibleColumns } from '../../config/columnDefinitions';

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
    isNetworkView?: boolean;
    onScrollToggle: (show: boolean) => void;
    onItemMiddleClick?: (entry: FileEntry) => void;
    diffPaths?: Set<string>;
    colWidths?: ColumnWidths;
    isSearching?: boolean;
    loading?: boolean;
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
    isNetworkView?: boolean;
}

// Memoized Details Row Component
const DetailsRow = React.memo((props: RowComponentProps<SharedItemProps>) => {
    const { index, style, ...sharedProps } = props;
    const {
        entries, isTrashView, isNetworkView, searchResults,
        t, dateFormat, colWidths, getIcon, showHistogram, totalItemsSize, showCheckboxes,
        renamingPath, renameText, onRenameTextChange, onRenameCommit, onRenameCancel
    } = sharedProps;

    const mode = getColumnMode(!!isTrashView, !!searchResults, isNetworkView);
    const visibleCols = getVisibleColumns(mode);

    const entry = entries[index];
    if (!entry) return null;

    const { isSelected, isRenaming, isCut, isProtected, handlers, itemClassName } = useFileItemState({
        ...sharedProps,
        entry
    });

    const isDragOver = sharedProps.dragOverPath === entry.path;

    // Fixed width based on the calculated column sum
    const adjustedStyle = {
        ...style,
        width: '100%',
        minWidth: 'max-content'
    };

    return (
        <div
            className={cx(itemClassName, "details", {
                "drag-over": isDragOver
            })}
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

            {visibleCols.map(col => {
                if (col.key === 'name') {
                    return (
                        <div key={col.key} className="file-name-group">
                            {showCheckboxes && (
                                <div className="item-checkbox" onClick={handlers.onCheckboxClick}>
                                    <div className={cx("checkbox-indicator", { checked: isSelected })}>
                                        {isSelected && <Check size={10} strokeWidth={4} />}
                                    </div>
                                </div>
                            )}
                            <div className="file-icon-small">
                                {getIcon(entry)}
                                {isProtected && <Shield className="protected-shield-badge" size={12} fill="currentColor" />}
                            </div>
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
                    );
                }

                return (
                    <div key={col.key} className={col.cellClass} style={col.align === 'right' ? { textAlign: 'right' } : {}}>
                        {col.renderCell(entry, { t, dateFormat })}
                    </div>
                );
            })}
        </div>
    );
});

// Memoized Grid Item Component
const GridCell = React.memo((props: CellComponentProps<SharedItemProps>) => {
    const { columnIndex, rowIndex, style, ...sharedProps } = props;
    const {
        entries, renamingPath, renameText, showCheckboxes, getIcon,
        onRenameTextChange, onRenameCommit, onRenameCancel,
        columnCount = 1, rootFontSize
    } = sharedProps;

    const index = rowIndex * columnCount + columnIndex;
    const entry = entries[index];

    const { isSelected, isRenaming, isProtected, handlers, itemClassName } = useFileItemState({
        ...sharedProps,
        entry: entry || ({} as FileEntry)
    });

    if (!entry) return null;

    const isDragOver = sharedProps.dragOverPath === entry.path;

    return (
        <div
            className={cx(itemClassName, "grid", {
                "drag-over": isDragOver,
                "is-dir": entry.is_dir
            })}
            style={style}
            onClick={handlers.onClick}
            onDoubleClick={handlers.onDoubleClick}
            onContextMenu={handlers.onContextMenu}
            draggable={!isRenaming}
            onDragStart={handlers.onDragStart}
            onMouseDown={handlers.onMouseDown}
        >
            <div className="grid-selection-overlay" />
            <div className="file-icon-large">
                {getIcon(entry, rootFontSize * 3)}
                {isProtected && <Shield className="protected-shield-badge" size={16} fill="currentColor" />}
            </div>
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
            {showCheckboxes && (
                <div className="item-checkbox" onClick={handlers.onCheckboxClick}>
                    <div className={cx("checkbox-indicator", { checked: isSelected })}>
                        {isSelected && <Check size={10} strokeWidth={4} />}
                    </div>
                </div>
            )}
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
        totalItemsSize, showHistogram, isTrashView, searchResults, isNetworkView,
        onScrollToggle, onItemMiddleClick,
        diffPaths, colWidths, isSearching, loading
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

    const listRef = useRef<any>(null);
    const gridRef = useRef<any>(null);

    useImperativeHandle(ref, () => ({
        scrollToTop: () => {
            listRef.current?.scrollTo(0);
            gridRef.current?.scrollTo({ scrollTop: 0 });
        }
    }));

    const isGrid = viewMode === 'grid';
    const cutPathsSet = useMemo(() => new Set(cutPaths), [cutPaths]);

    const sharedProps = useMemo<SharedItemProps>(() => ({
        entries: files, selected, pendingSelection, renamingPath, renameText,
        isDragging, dragOverPath, cutPathsSet, searchResults,
        t, onItemClick, onItemDoubleClick, onItemContextMenu, onFileDragStart,
        onRenameTextChange, onRenameCommit, onRenameCancel, getIcon,
        totalItemsSize, showHistogram, isTrashView, isNetworkView, onItemMiddleClick,
        dateFormat, diffPaths, viewMode, rootFontSize, colWidths, showCheckboxes
    }), [
        files, selected, pendingSelection, renamingPath, renameText,
        isDragging, dragOverPath, cutPathsSet, searchResults,
        t, onItemClick, onItemDoubleClick, onItemContextMenu, onFileDragStart,
        onRenameTextChange, onRenameCommit, onRenameCancel, getIcon,
        totalItemsSize, showHistogram, isTrashView, isNetworkView, onItemMiddleClick,
        dateFormat, diffPaths, viewMode, rootFontSize, colWidths, showCheckboxes
    ]);

    const listRowHeight = rootFontSize * 1.75;
    const gridRowHeightBase = rootFontSize * 6.0;
    const gridGap = rootFontSize * 0.25;

    const totalColumnWidth = useMemo(() => {
        if (!colWidths || isGrid) return 0;
        const mode = getColumnMode(!!isTrashView, !!searchResults, isNetworkView);
        const visibleCols = getVisibleColumns(mode);
        const cw = colWidths as any;
        const sum = visibleCols.reduce((acc, col) => acc + (cw[col.key] || col.defaultWidth), 0);
        return sum + (rootFontSize * 1.25);
    }, [colWidths, isGrid, searchResults, isTrashView, isNetworkView, rootFontSize]);

    return (
        <div
            className={cx("virtualized-list", {
                "details": !isGrid,
                "grid": isGrid,
                "search-mode": !!searchResults,
                "trash-mode": isTrashView
            })}
            style={{ width: '100%', height: '100%', overflowX: 'auto', overflowY: 'hidden' }}
        >
            <AutoSizer renderProp={({ height, width }: { height: number | undefined; width: number | undefined }) => {
                if (!height || !width) return null;

                if (files.length === 0 && (!searchResults || searchResults.length === 0) && !isSearching) {
                    if (loading) {
                        return (
                            <div className="empty-msg loading" style={{ width, height, position: 'absolute', top: 0, left: 0, display: 'flex', alignItems: 'center', justifyContent: 'center' }}>
                                <Loader2 size={16} className="spinning" style={{ animation: 'spin 2s linear infinite' }} />&nbsp;&nbsp;<span>{t('loading' as any) || "Loading..."}</span>
                            </div>
                        );
                    }
                    return (
                        <div className="empty-msg" style={{ width, height, position: 'absolute', top: 0, left: 0, display: 'flex', alignItems: 'center', justifyContent: 'center' }}>
                            <span>{t('no_results')}</span>
                        </div>
                    );
                }

                if (isGrid) {
                    const minColumnWidth = rootFontSize * 6.0;
                    const horizontalPadding = rootFontSize * 2.5;
                    const columnCount = Math.max(1, Math.floor((width - horizontalPadding) / (minColumnWidth + gridGap)));
                    const rowCount = Math.ceil(files.length / columnCount);
                    const columnWidth = (width - horizontalPadding - (columnCount - 1) * gridGap) / columnCount;

                    return (
                        <Grid
                            key={`grid-${rootFontSize}-${columnCount}`}
                            className="virtualized-scroller grid"
                            columnCount={columnCount}
                            columnWidth={columnWidth + gridGap}
                            rowCount={rowCount}
                            rowHeight={gridRowHeightBase + gridGap}
                            cellComponent={GridCell as any}
                            cellProps={{ ...sharedProps, columnCount }}
                            gridRef={gridRef}
                            style={{ height, width, overflowX: 'hidden', overflowY: 'auto' }}
                            onScroll={(e: any) => onScrollToggle(e.scrollTop > 100)}
                        />
                    );
                }

                const finalWidth = Math.max(width, totalColumnWidth);

                return (
                    <List
                        key={`list-${rootFontSize}`}
                        className="virtualized-scroller details"
                        rowCount={files.length}
                        rowHeight={listRowHeight}
                        rowComponent={DetailsRow as any}
                        rowProps={sharedProps}
                        listRef={listRef}
                        style={{ height, width: finalWidth, overflowY: 'auto', overflowX: 'hidden' }}
                        onScroll={(e: any) => onScrollToggle(e.scrollOffset > 100)}
                    />
                );
            }} />
        </div>
    );
});
