import React, { useState, useEffect, useMemo, useRef, useImperativeHandle, useCallback } from 'react';
import { List, Grid } from 'react-window';
import { AutoSizer } from 'react-virtualized-auto-sizer';
import cx from 'classnames';
import { Check, Loader2, ChevronDown, ChevronRight, Ban, X, Search } from 'lucide-react';

import { FileEntry, ViewMode, ColumnWidths, DateFormat, SIZE_CATEGORIES } from '../../types';
import { TFunc } from '../../i18n';
import { useApp } from '../../context/AppContext';
import { useFileItemState } from '../../hooks/useFileItemState';
import { RenameInput } from './RenameInput';
import { getFileTypeString } from '../../utils/format';
import { getColumnMode, getVisibleColumns } from '../../config/columnDefinitions';
import { getDateCategoryForFile, DATE_CATEGORIES, DateCategoryKey } from './DateFilterMenu';
import { getParent } from '../../utils/path';

// --- Error Boundary for the virtualized list ---
class ErrorBoundary extends React.Component<{ children: React.ReactNode; fallback: React.ReactNode }, { hasError: boolean }> {
    constructor(props: any) {
        super(props);
        this.state = { hasError: false };
    }
    static getDerivedStateFromError() { return { hasError: true }; }
    componentDidCatch(error: any, errorInfo: any) { console.error("VirtualizedList Crash:", error, errorInfo); }
    render() {
        if (this.state.hasError) return this.props.fallback;
        return this.props.children;
    }
}

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
    groupByDate?: boolean;
    initialScrollOffset?: number;
    updateCurrentScroll?: (offset: number) => void;
    isProtected?: boolean;
    activeFilters?: ActiveFilters;
    currentPath?: string;
    onNavigate?: (path: string) => void;
    onActivate: () => void;
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
    isNetworkView?: boolean;
    activeFilters?: ActiveFilters;
    showCheckboxes: boolean;
    currentIndex: number;
    onActivate: () => void;
    index?: number;
}

export interface ActiveFilters {
    extensions: Set<string> | null;
    sizes: Set<string> | null;
    date: Set<string> | null;
    deletedDates: Set<string> | null;
    name: string | null;
    location: string | null;
    onRemoveExtension: (ext: string) => void;
    onRemoveSize: (size: any) => void;
    onRemoveDate: (date: any) => void;
    onRemoveDeletedDate: (date: any) => void;
    onRemoveName: () => void;
    onRemoveLocation: () => void;
    onClearExtensions: () => void;
    onClearSizes: () => void;
    onClearDate: () => void;
    onClearDeletedDates: () => void;
    onClearAll: () => void;
}

type GroupedItem =
    | { type: 'filters' }
    | { type: 'header'; category: DateCategoryKey; count: number }
    | { type: 'file'; entry: FileEntry };

type GroupedGridRowItem =
    | { type: 'filters' }
    | { type: 'header'; category: DateCategoryKey; count: number }
    | { type: 'grid-row'; entries: FileEntry[]; category: DateCategoryKey };

const CATEGORY_ORDER: DateCategoryKey[] = [
    'today',
    'yesterday',
    'this_week',
    'last_week',
    'this_month',
    'older'
];

// --- Standard Row sub-component for hook safety ---
const DetailsRowContent = React.memo(({ entry, style, sharedProps }: { entry: FileEntry, style: React.CSSProperties, sharedProps: SharedItemProps }) => {
    const { t, dateFormat, searchResults, isTrashView, isNetworkView, getIcon, showHistogram, totalItemsSize, showCheckboxes, renameText, onRenameTextChange, onRenameCommit, onRenameCancel } = sharedProps;

    const { isSelected, isRenaming, handlers, itemClassName, tooltipText } = useFileItemState({
        ...sharedProps,
        entry,
        isFocused: sharedProps.currentIndex === sharedProps.index // Using index passed from List
    });

    const isDragOver = sharedProps.dragOverPath === entry.path;
    const mode = getColumnMode(!!isTrashView, !!searchResults, isNetworkView);
    const visibleCols = getVisibleColumns(mode);

    return (
        <div
            className={cx(itemClassName, "details", { "drag-over": isDragOver })}
            style={{ ...style, width: 'max-content', minWidth: 'max-content' }}
            data-path={entry.path}
            data-is-dir={entry.is_dir ? 'true' : 'false'}
            onClick={handlers.onClick}
            onDoubleClick={handlers.onDoubleClick}
            onContextMenu={handlers.onContextMenu}
            draggable={!isRenaming}
            onDragStart={handlers.onDragStart}
            onMouseDown={handlers.onMouseDown}
        >
            {showHistogram && (
                <div className="size-histogram-bar" style={{ width: `${(entry.size / totalItemsSize) * 100}%` }} />
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
                            </div>
                            <div className="file-name-container">
                                {isRenaming ? (
                                    <RenameInput renameText={renameText} onRenameTextChange={onRenameTextChange} onRenameCommit={onRenameCommit} onRenameCancel={onRenameCancel} />
                                ) : (
                                    <span className="file-name">
                                        <span className="file-name-text"
                                            data-tooltip={tooltipText}
                                            data-tooltip-multiline
                                            data-tooltip-image-path={entry.path}
                                        >{entry.name}</span>
                                        {entry.is_dir && !isNetworkView && entry.folders_count !== undefined && entry.folders_count > 0 && (
                                            <ChevronRight size={14} className="folder-has-subdirs" />
                                        )}
                                    </span>
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

const DetailsRow = React.memo((props: any) => {
    const { index, style, entries } = props;
    const entry = entries[index];
    if (!entry) return null;
    return <DetailsRowContent entry={entry} style={style} sharedProps={{ ...props, index }} />;
});

// --- Grid Cell sub-component for hook safety ---
const GridCellContent = React.memo(({ entry, style, sharedProps }: { entry: FileEntry, style: React.CSSProperties, sharedProps: SharedItemProps }) => {
    const { getIcon, rootFontSize, t, renameText, onRenameTextChange, onRenameCommit, onRenameCancel, showCheckboxes, isNetworkView } = sharedProps;

    const { isSelected, isRenaming, handlers, itemClassName, tooltipText } = useFileItemState({
        ...sharedProps,
        entry,
        isFocused: sharedProps.currentIndex === sharedProps.index // Using index passed from Grid
    });

    const isDragOver = sharedProps.dragOverPath === entry.path;
    const gridGap = rootFontSize * 0.25;
    const adjustedStyle = {
        ...style,
        width: Math.max(0, (typeof style.width === 'number' ? style.width : parseFloat(String(style.width)) || 0) - gridGap),
        height: Math.max(0, (typeof style.height === 'number' ? style.height : parseFloat(String(style.height)) || 0) - gridGap),
    };

    return (
        <div
            className={cx(itemClassName, "grid", {
                "drag-over": isDragOver,
                "is-dir": entry.is_dir
            })}
            style={adjustedStyle}
            data-path={entry.path}
            data-is-dir={entry.is_dir ? 'true' : 'false'}
            onClick={handlers.onClick}
            onDoubleClick={handlers.onDoubleClick}
            onContextMenu={handlers.onContextMenu}
            draggable={!isRenaming}
            onDragStart={handlers.onDragStart}
            onMouseDown={handlers.onMouseDown}
            data-tooltip={tooltipText}
            data-tooltip-multiline
            data-tooltip-image-path={entry.path}
        >
            <div className="grid-selection-overlay" />
            <div className="grid-item-inner">
                <div className="file-icon-large">
                    {getIcon(entry, rootFontSize * 3)}
                    {entry.is_dir && !isNetworkView && entry.folders_count !== undefined && entry.folders_count > 0 && (
                        <div className="folder-grid-badge">
                            <ChevronRight size={10} strokeWidth={3} />
                        </div>
                    )}
                </div>
                <div className="file-name-container">
                    {isRenaming ? (
                        <RenameInput renameText={renameText} onRenameTextChange={onRenameTextChange} onRenameCommit={onRenameCommit} onRenameCancel={onRenameCancel} className="rename-input grid-mode" />
                    ) : (
                        <>
                            <span className="file-name">
                                {!entry.is_dir && entry.name.lastIndexOf('.') > 0 ? entry.name.slice(0, entry.name.lastIndexOf('.')) : entry.name}
                            </span>
                            <span className="file-extension">{getFileTypeString(entry, t)}</span>
                        </>
                    )}
                </div>
            </div>
            {showCheckboxes && (
                <div className="item-checkbox grid-checkbox" onClick={handlers.onCheckboxClick}>
                    <div className={cx("checkbox-indicator", { checked: isSelected })}>
                        {isSelected && <Check size={12} strokeWidth={3.5} />}
                    </div>
                </div>
            )}
        </div>
    );
});

const GridCell = React.memo((props: any) => {
    const { columnIndex, rowIndex, style, entries, columnCount = 1 } = props;
    const index = rowIndex * columnCount + columnIndex;
    const entry = entries[index];
    if (!entry) return null;
    return <GridCellContent entry={entry} style={style} sharedProps={{ ...props, index }} />;
});

// --- Grouped Details Row Components ---
interface GroupedSharedProps extends SharedItemProps {
    groupedItems: GroupedItem[];
    collapsedGroups: Set<DateCategoryKey>;
    onToggleGroup: (category: DateCategoryKey) => void;
}

const FiltersRow = React.memo(({ activeFilters, t, style }: { activeFilters: ActiveFilters; t: TFunc; style: React.CSSProperties }) => {
    return (
        <div style={style}>
            <div className="group-header details is-filters" style={{ width: 'max-content', minWidth: 'max-content', height: '1.85rem' }}>
                <div className="active-filters-info">
                    <div className="active-filters-label">
                        <Search size={14} />
                        <span>{t('active_filters' as any) || 'Active filters'}</span>
                    </div>
                    <div className="active-filters-pills">
                        {activeFilters.extensions && activeFilters.extensions.size > 0 && (
                            <div className="filter-pill-group">
                                {activeFilters.extensions.size === 1
                                    ? Array.from(activeFilters.extensions).map(ext => (
                                        <span key={ext} className="filter-pill" onClick={() => (activeFilters as any).onRemoveExtension(ext)}>
                                            {t('type')}: {ext === '__DIR__' ? (t('folder' as any) || 'Folder') : (ext === '' ? `(${t('none_fem' as any) || 'None'})` : ext.toUpperCase())} <X size={10} style={{ marginLeft: '4px' }} />
                                        </span>
                                    ))
                                    : (
                                        <span className="filter-pill" onClick={() => (activeFilters as any).onClearExtensions()}>
                                            {(t('type') || 'Type')}: {activeFilters.extensions.size} <X size={10} style={{ marginLeft: '4px' }} />
                                        </span>
                                    )
                                }
                            </div>
                        )}
                        {activeFilters.sizes && activeFilters.sizes.size > 0 && (
                            <div className="filter-pill-group">
                                {activeFilters.sizes.size === 1
                                    ? Array.from(activeFilters.sizes).map(size => {
                                        const info = (SIZE_CATEGORIES as any)[size];
                                        const label = info ? t(info.key as any) : size;
                                        return (
                                            <span key={size} className="filter-pill" onClick={() => (activeFilters as any).onRemoveSize(size)}>
                                                {t('size')}: {label} <X size={10} style={{ marginLeft: '4px' }} />
                                            </span>
                                        );
                                    })
                                    : (
                                        <span className="filter-pill" onClick={() => (activeFilters as any).onClearSizes()}>
                                            {(t('size') || 'Size')}: {activeFilters.sizes.size} <X size={10} style={{ marginLeft: '4px' }} />
                                        </span>
                                    )
                                }
                            </div>
                        )}
                        {activeFilters.deletedDates && activeFilters.deletedDates.size > 0 && (
                            <div className="filter-pill-group">
                                {activeFilters.deletedDates.size === 1
                                    ? Array.from(activeFilters.deletedDates).map(date => {
                                        const key = (DATE_CATEGORIES as any)[date];
                                        const label = key ? t(key as any) : date;
                                        return (
                                            <span key={date} className="filter-pill" onClick={() => (activeFilters as any).onRemoveDeletedDate(date)}>
                                                {t('deleted_date')}: {label} <X size={10} style={{ marginLeft: '4px' }} />
                                            </span>
                                        );
                                    })
                                    : (
                                        <span className="filter-pill" onClick={() => (activeFilters as any).onClearDeletedDates()}>
                                            {t('deleted_date')}: {activeFilters.deletedDates.size} <X size={10} style={{ marginLeft: '4px' }} />
                                        </span>
                                    )
                                }
                            </div>
                        )}
                        {activeFilters.date && activeFilters.date.size > 0 && (
                            <div className="filter-pill-group">
                                {activeFilters.date.size === 1
                                    ? Array.from(activeFilters.date).map(date => {
                                        const key = (DATE_CATEGORIES as any)[date];
                                        const label = key ? t(key as any) : date;
                                        return (
                                            <span key={date} className="filter-pill" onClick={() => activeFilters.onRemoveDate(date)}>
                                                {t('date')}: {label} <X size={10} style={{ marginLeft: '4px' }} />
                                            </span>
                                        );
                                    })
                                    : (
                                        <span className="filter-pill" onClick={() => activeFilters.onClearDate()}>
                                            {t('date')}: {activeFilters.date.size} <X size={10} style={{ marginLeft: '4px' }} />
                                        </span>
                                    )
                                }
                            </div>
                        )}
                        {activeFilters.name && (
                            <div className="filter-pill-group">
                                <span className="filter-pill" onClick={activeFilters.onRemoveName}>
                                    {t('name')}: {activeFilters.name} <X size={10} style={{ marginLeft: '4px' }} />
                                </span>
                            </div>
                        )}
                        {activeFilters.location && (
                            <div className="filter-pill-group">
                                <span className="filter-pill" onClick={activeFilters.onRemoveLocation}>
                                    {t('location')}: {activeFilters.location} <X size={10} style={{ marginLeft: '4px' }} />
                                </span>
                            </div>
                        )}
                    </div>
                    <button className="clear-filters-x-btn" onClick={activeFilters.onClearAll} data-tooltip={t('remove_all_filters' as any)}>
                        <X size={14} />
                    </button>
                </div>
            </div>
        </div>
    );
});

// --- Grouped File Row sub-component for hook safety ---
const GroupedFileRow = React.memo(({ entry, style, sharedProps }: { entry: FileEntry, style: React.CSSProperties, sharedProps: GroupedSharedProps }) => {
    const { t, dateFormat, searchResults, isTrashView, isNetworkView, getIcon, showHistogram, totalItemsSize, showCheckboxes, renameText, onRenameTextChange, onRenameCommit, onRenameCancel } = sharedProps;

    const { isSelected, isRenaming, handlers, itemClassName, tooltipText } = useFileItemState({
        ...sharedProps,
        entry,
        isFocused: sharedProps.currentIndex === (sharedProps as any).index
    });

    const isDragOver = sharedProps.dragOverPath === entry.path;
    const mode = getColumnMode(!!isTrashView, !!searchResults, isNetworkView);
    const visibleCols = getVisibleColumns(mode);

    return (
        <div
            className={cx(itemClassName, "details", { "drag-over": isDragOver })}
            style={{ ...style, width: 'max-content', minWidth: 'max-content' }}
            data-path={entry.path}
            data-is-dir={entry.is_dir ? 'true' : 'false'}
            onClick={handlers.onClick}
            onDoubleClick={handlers.onDoubleClick}
            onContextMenu={handlers.onContextMenu}
            draggable={!isRenaming}
            onDragStart={handlers.onDragStart}
            onMouseDown={handlers.onMouseDown}
        >
            {showHistogram && (
                <div className="size-histogram-bar" style={{ width: `${(entry.size / totalItemsSize) * 100}%` }} />
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
                            </div>
                            <div className="file-name-container">
                                {isRenaming ? (
                                    <RenameInput renameText={renameText} onRenameTextChange={onRenameTextChange} onRenameCommit={onRenameCommit} onRenameCancel={onRenameCancel} />
                                ) : (
                                    <span className="file-name">
                                        <span className="file-name-text"
                                            data-tooltip={tooltipText}
                                            data-tooltip-multiline
                                            data-tooltip-image-path={entry.path}
                                        >{entry.name}</span>
                                        {entry.is_dir && !isNetworkView && entry.folders_count !== undefined && entry.folders_count > 0 && (
                                            <ChevronRight size={14} className="folder-has-subdirs" />
                                        )}
                                    </span>
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

const GroupedDetailsRow = React.memo((props: any) => {
    const { index, style, groupedItems, collapsedGroups, onToggleGroup, t, activeFilters } = props;

    const item = groupedItems[index];
    if (!item) return null;

    if (item.type === 'filters') {
        if (!activeFilters) return null;
        return <FiltersRow activeFilters={activeFilters} t={t} style={style} />;
    }

    if (item.type === 'header') {
        const isCollapsed = collapsedGroups.has(item.category);
        const labelKey = (DATE_CATEGORIES as any)[item.category] || item.category;
        const label = t(labelKey as any) || item.category;
        return (
            <div className="group-header details" style={{ ...style, width: 'max-content', minWidth: 'max-content' }} onClick={() => onToggleGroup(item.category)}>
                <div className="group-header-content">
                    {isCollapsed ? <ChevronRight size={14} /> : <ChevronDown size={14} />}
                    <span className="group-header-label">{label}</span>
                    <span className="group-header-count">{t('items_count' as any, { count: item.count })}</span>
                </div>
            </div>
        );
    }

    // File item 
    // For grouped items, we need to find the actual file index in the 'entries' array to compare with currentIndex
    const fileIndex = props.entries.indexOf(item.entry);
    return <GroupedFileRow entry={item.entry} style={style} sharedProps={{ ...props, index: fileIndex }} />;
});



const GroupedGridRowComponent = React.memo((props: any) => {
    const { index, style, groupedGridItems, collapsedGroups, onToggleGroup, t, activeFilters, columnCount, columnWidth, gridGap, gridRowHeight } = props;

    const item = groupedGridItems[index];
    if (!item) return null;

    if (item.type === 'filters') {
        if (!activeFilters) return null;
        return <FiltersRow activeFilters={activeFilters} t={t} style={style} />;
    }

    if (item.type === 'header') {
        const isCollapsed = collapsedGroups.has(item.category);
        const labelKey = (DATE_CATEGORIES as any)[item.category] || item.category;
        const label = t(labelKey as any) || item.category;
        return (
            <div className="group-header grid" style={style} onClick={() => onToggleGroup(item.category)}>
                <div className="group-header-content">
                    {isCollapsed ? <ChevronRight size={14} /> : <ChevronDown size={14} />}
                    <span className="group-header-label">{label}</span>
                    <span className="group-header-count">{t('items_count' as any, { count: item.count })}</span>
                </div>
            </div>
        );
    }

    // Grid row
    return (
        <div className="group-grid-items" style={{
            ...style,
            display: 'grid',
            gridTemplateColumns: `repeat(${columnCount}, ${columnWidth}px)`,
            gap: `${gridGap}px`,
        }}>
            {item.entries.map((entry: FileEntry) => {
                const fileIndex = props.entries.indexOf(entry);
                return <GroupedGridItem key={entry.path} entry={entry} sharedProps={{ ...props, index: fileIndex }} gridRowHeight={gridRowHeight} />;
            })}
        </div>
    );
});

const GroupedGridItem = React.memo<{ entry: FileEntry; sharedProps: SharedItemProps; gridRowHeight: number }>(({ entry, sharedProps, gridRowHeight }) => {
    const { getIcon, rootFontSize, t, renameText, onRenameTextChange, onRenameCommit, onRenameCancel, showCheckboxes, isNetworkView } = sharedProps;

    const { isSelected, isRenaming, handlers, itemClassName, tooltipText } = useFileItemState({
        ...sharedProps,
        entry,
        isFocused: sharedProps.currentIndex === (sharedProps as any).index
    });

    const isDragOver = sharedProps.dragOverPath === entry.path;

    return (
        <div
            className={cx(itemClassName, "grid", { "drag-over": isDragOver, "is-dir": entry.is_dir })}
            style={{ height: gridRowHeight }}
            data-path={entry.path}
            data-is-dir={entry.is_dir ? 'true' : 'false'}
            onClick={handlers.onClick}
            onDoubleClick={handlers.onDoubleClick}
            onContextMenu={handlers.onContextMenu}
            draggable={!isRenaming}
            onDragStart={handlers.onDragStart}
            onMouseDown={handlers.onMouseDown}
            data-tooltip={tooltipText}
            data-tooltip-multiline
            data-tooltip-image-path={entry.path}
        >
            <div className="grid-selection-overlay" />
            <div className="grid-item-inner">
                <div className="file-icon-large">
                    {getIcon(entry, rootFontSize * 3)}
                    {entry.is_dir && !isNetworkView && entry.folders_count !== undefined && entry.folders_count > 0 && (
                        <div className="folder-grid-badge">
                            <ChevronRight size={10} strokeWidth={3} />
                        </div>
                    )}
                </div>
                <div className="file-name-container">
                    {isRenaming ? (
                        <RenameInput renameText={renameText} onRenameTextChange={onRenameTextChange} onRenameCommit={onRenameCommit} onRenameCancel={onRenameCancel} className="rename-input grid-mode" />
                    ) : (
                        <>
                            <span className="file-name">
                                {!entry.is_dir && entry.name.lastIndexOf('.') > 0 ? entry.name.slice(0, entry.name.lastIndexOf('.')) : entry.name}
                            </span>
                            <span className="file-extension">{getFileTypeString(entry, t)}</span>
                        </>
                    )}
                </div>
            </div>
            {showCheckboxes && (
                <div className="item-checkbox grid-checkbox" onClick={handlers.onCheckboxClick}>
                    <div className={cx("checkbox-indicator", { checked: isSelected })}>
                        {isSelected && <Check size={12} strokeWidth={3.5} />}
                    </div>
                </div>
            )}
        </div>
    );
});

// --- Main Component ---
export const VirtualizedFileList = React.forwardRef<VirtualizedFileListHandle, VirtualizedFileListProps>((props, ref) => {
    const {
        viewMode, files,
        selected, pendingSelection, renamingPath, renameText,
        isDragging, dragOverPath, cutPaths,
        t, onItemClick, onItemDoubleClick, onItemContextMenu, onFileDragStart,
        onRenameTextChange, onRenameCommit, onRenameCancel, getIcon,
        totalItemsSize, showHistogram, isTrashView, searchResults, isNetworkView,
        onScrollToggle, onItemMiddleClick,
        diffPaths, colWidths, isSearching, loading,
        groupByDate = false,
        initialScrollOffset = 0, updateCurrentScroll,
        isProtected = false,
        activeFilters,
        currentPath, onNavigate, onActivate
    } = props;

    const { dateFormat, showCheckboxes } = useApp();
    const columnCountRef = useRef(1);

    const currentIndex = useMemo(() => {
        if (selected.size === 0) return -1;
        // Search in the original files array or searchResults if present
        return files.findIndex(f => selected.has(f.path));
    }, [files, selected]);

    const handleKeyDown = useCallback((e: React.KeyboardEvent) => {
        if (!files.length || renamingPath) return;

        const selectIndex = (index: number) => {
            const entry = files[index];
            if (entry) {
                onItemClick(entry, {
                    ctrlKey: false,
                    shiftKey: false,
                    button: 0,
                    preventDefault: () => { },
                    stopPropagation: () => { }
                } as any);
            }
        };

        const colCount = viewMode === 'grid' ? columnCountRef.current : 1;

        switch (e.key) {
            case 'ArrowDown':
                e.preventDefault();
                if (currentIndex === -1) selectIndex(0);
                else if (currentIndex + colCount < files.length) selectIndex(currentIndex + colCount);
                break;
            case 'ArrowUp':
                e.preventDefault();
                if (currentIndex === -1) selectIndex(files.length - 1);
                else if (currentIndex - colCount >= 0) selectIndex(currentIndex - colCount);
                break;
            case 'ArrowRight':
                if (viewMode === 'grid') {
                    e.preventDefault();
                    if (currentIndex < files.length - 1) selectIndex(currentIndex + 1);
                }
                break;
            case 'ArrowLeft':
                if (viewMode === 'grid') {
                    e.preventDefault();
                    if (currentIndex > 0) selectIndex(currentIndex - 1);
                }
                break;
            case 'Enter':
                if (!e.ctrlKey && !e.altKey && !e.shiftKey) {
                    e.preventDefault();
                    if (currentIndex !== -1) {
                        const focusedEntry = files[currentIndex];
                        if (focusedEntry) onItemDoubleClick(focusedEntry);
                    }
                }
                break;
            case 'Backspace':
                e.preventDefault();
                if (currentPath && onNavigate) {
                    const parentPath = getParent(currentPath);
                    if (parentPath) onNavigate(parentPath);
                }
                break;
            case 'ContextMenu':
            case 'Apps':
                e.preventDefault();
                e.stopPropagation();
                if ((e as any).nativeEvent) {
                    (e as any).nativeEvent.stopPropagation();
                    (e as any).nativeEvent.stopImmediatePropagation();
                }
                // On Windows/WebView2, pressing ContextMenu key fires a 'contextmenu' DOM event
                // with the MOUSE CURSOR coordinates (not 0,0). FilePanel catches it and overwrites
                // the correct position. Block it at document capture level before it reaches anything.
                {
                    const blockContextMenu = (ev: Event) => {
                        ev.preventDefault();
                        ev.stopPropagation();
                        ev.stopImmediatePropagation();
                        document.removeEventListener('contextmenu', blockContextMenu, true);
                    };
                    document.addEventListener('contextmenu', blockContextMenu, true);
                }
                if (currentIndex !== -1) {
                    const entry = files[currentIndex];
                    if (entry) {
                        onItemContextMenu(entry, null as any);
                    }
                }
                break;
        }
    }, [files, selected, currentIndex, renamingPath, viewMode, onItemClick, onItemDoubleClick, onItemContextMenu, currentPath, onNavigate]);
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
    const scrollContainerRef = useRef<HTMLDivElement>(null);

    useImperativeHandle(ref, () => ({
        scrollToTop: () => {
            const listEl = listRef.current?.element;
            if (listEl) listEl.scrollTop = 0;
            const gridEl = gridRef.current?.element;
            if (gridEl) gridEl.scrollTop = 0;
            if (scrollContainerRef.current) scrollContainerRef.current.scrollTop = 0;
        }
    }));

    const isGrid = viewMode === 'grid';
    const cutPathsSet = useMemo(() => new Set(cutPaths), [cutPaths]);

    const isRestoringRef = useRef(false);
    const currentScrollRef = useRef(0);
    const initialScrollOffsetRef = useRef(initialScrollOffset);

    useEffect(() => {
        initialScrollOffsetRef.current = initialScrollOffset;
    }, [initialScrollOffset]);

    const groupedByCategory = useMemo(() => {
        if (!groupByDate) return null;
        const groups = new Map<DateCategoryKey, FileEntry[]>();
        CATEGORY_ORDER.forEach(cat => groups.set(cat, []));

        files.forEach(file => {
            const cat = getDateCategoryForFile(file.modified || 0);
            groups.get(cat)?.push(file);
        });
        return groups;
    }, [files, groupByDate]);

    const [collapsedGroups, setCollapsedGroups] = useState<Set<DateCategoryKey>>(new Set());
    const handleToggleGroup = useCallback((cat: DateCategoryKey) => {
        setCollapsedGroups(prev => {
            const next = new Set(prev);
            if (next.has(cat)) next.delete(cat);
            else next.add(cat);
            return next;
        });
    }, []);

    const hasActiveFilters = useMemo(() => {
        if (!activeFilters) return false;
        return (activeFilters.extensions && activeFilters.extensions.size > 0) ||
               (activeFilters.sizes && activeFilters.sizes.size > 0) ||
               (activeFilters.date && activeFilters.date.size > 0) ||
               (activeFilters.deletedDates && activeFilters.deletedDates.size > 0) ||
               !!activeFilters.name ||
               !!activeFilters.location;
    }, [activeFilters]);

    const groupedDetailItems = useMemo<GroupedItem[]>(() => {
        if (!groupByDate && !hasActiveFilters) return [];
        const items: GroupedItem[] = [];

        if (hasActiveFilters) {
            items.push({ type: 'filters' });
        }

        if (groupByDate && groupedByCategory) {
            CATEGORY_ORDER.forEach(cat => {
                const entries = groupedByCategory.get(cat);
                if (!entries || entries.length === 0) return;

                items.push({ type: 'header', category: cat, count: entries.length });
                if (!collapsedGroups.has(cat)) {
                    entries.forEach(entry => items.push({ type: 'file', entry }));
                }
            });
        } else {
            files.forEach(entry => items.push({ type: 'file', entry }));
        }

        return items;
    }, [files, groupByDate, groupedByCategory, collapsedGroups, hasActiveFilters]);

    const listRowHeight = rootFontSize * 1.85;
    const groupHeaderHeight = rootFontSize * 2.0;
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

    const getGroupedRowHeight = useCallback((index: number): number => {
        const item = groupedDetailItems[index];
        if (!item) return listRowHeight;
        if (item.type === 'filters') return groupHeaderHeight + (rootFontSize * 0.5); // Add 0.5rem margin
        return item.type === 'header' ? groupHeaderHeight : listRowHeight;
    }, [groupedDetailItems, listRowHeight, groupHeaderHeight, rootFontSize]);

    const scrollTimeoutRef = useRef<number | null>(null);

    const handleScroll = useCallback((e: any) => {
        let top = 0;
        if (e && e.currentTarget) {
            top = e.currentTarget.scrollTop;
        } else if (e && typeof e.scrollTop === 'number') {
            top = e.scrollTop;
        } else if (e && typeof e.scrollOffset === 'number') {
            top = e.scrollOffset;
        }

        if (onScrollToggle) onScrollToggle(top > 100);

        if (!isRestoringRef.current && updateCurrentScroll) {
            currentScrollRef.current = top;
            if (scrollTimeoutRef.current) window.clearTimeout(scrollTimeoutRef.current);
            scrollTimeoutRef.current = window.setTimeout(() => {
                updateCurrentScroll(top);
            }, 250);
        }
    }, [onScrollToggle, updateCurrentScroll]);

    const sharedProps: SharedItemProps = {
        entries: files, selected, pendingSelection, renamingPath, renameText,
        isDragging, dragOverPath, cutPathsSet, searchResults,
        t, onItemClick, onItemDoubleClick, onItemContextMenu, onFileDragStart,
        onRenameTextChange, onRenameCommit, onRenameCancel, getIcon,
        totalItemsSize, showHistogram, isTrashView, onItemMiddleClick,
        dateFormat, diffPaths, viewMode, rootFontSize, isNetworkView, activeFilters,
        showCheckboxes, currentIndex, onActivate
    };

    const groupedSharedProps: GroupedSharedProps = {
        ...sharedProps,
        entries: files,
        groupedItems: groupedDetailItems,
        collapsedGroups,
        onToggleGroup: handleToggleGroup
    };

    return (
        <div
            onKeyDown={handleKeyDown}
            tabIndex={0}
            style={{ width: '100%', height: '100%', overflowX: 'auto', overflowY: 'hidden', outline: 'none' }}
            className={cx("virtualized-list", {
                "details": !isGrid,
                "grid": isGrid,
                "search-mode": !!searchResults,
                "trash-mode": isTrashView
            })}
        >
            <AutoSizer renderProp={({ height, width }: { height: number | undefined; width: number | undefined }) => {
                if (!height || !width) return null;

                return (
                    <ErrorBoundary key={viewMode} fallback={<div className="empty-msg" style={{ width, height, position: 'absolute', top: 0, left: 0, display: 'flex', alignItems: 'center', justifyContent: 'center' }}>Error in VirtualizedFileList</div>}>
                        {(() => {
                            if (files.length === 0 && (!searchResults || searchResults.length === 0) && !isSearching && !hasActiveFilters) {
                                if (loading) {
                                    return (
                                        <div className="empty-msg loading" style={{ width, height, position: 'absolute', top: 0, left: 0, display: 'flex', alignItems: 'center', justifyContent: 'center' }}>
                                            <Loader2 size={16} className="spinning" style={{ animation: 'spin 2s linear infinite' }} />&nbsp;&nbsp;<span>{t('loading' as any) || "Loading..."}</span>
                                        </div>
                                    );
                                }

                                let emptyKey = 'empty';
                                if (searchResults) emptyKey = 'no_results';
                                else if (isTrashView) emptyKey = 'recycle_bin_empty';

                                return (
                                    <div className="empty-msg" style={{ width, height, position: 'absolute', top: 0, left: 0, display: 'flex', flexDirection: 'column', alignItems: 'center', justifyContent: 'center', gap: '1rem' }}>
                                        {isProtected ? (
                                            <>
                                                <Ban size={48} className="protected-icon" style={{ color: 'var(--danger-color)', opacity: 0.8 }} />
                                                <span style={{ maxWidth: '300px', textAlign: 'center', opacity: 0.7 }}>{t('protected_access' as any)}</span>
                                            </>
                                        ) : (
                                            <span>{t(emptyKey as any)}</span>
                                        )}
                                    </div>
                                );
                            }

                            if (isGrid && groupByDate && groupedByCategory) {
                                const minColumnWidth = rootFontSize * 6.0;
                                const horizontalPadding = rootFontSize * 2.5;
                                const columnCount = Math.max(1, Math.floor((width - horizontalPadding) / (minColumnWidth + gridGap)));
                                columnCountRef.current = columnCount;
                                const columnWidth = (width - horizontalPadding - (columnCount - 1) * gridGap) / columnCount;

                                // Flatten data into rows
                                const items: GroupedGridRowItem[] = [];
                                if (hasActiveFilters) {
                                    items.push({ type: 'filters' });
                                }

                                CATEGORY_ORDER.forEach(cat => {
                                    const entries = groupedByCategory.get(cat);
                                    if (!entries || entries.length === 0) return;

                                    items.push({ type: 'header', category: cat, count: entries.length });
                                    if (!collapsedGroups.has(cat)) {
                                        for (let i = 0; i < entries.length; i += columnCount) {
                                            items.push({ 
                                                type: 'grid-row', 
                                                entries: entries.slice(i, i + columnCount),
                                                category: cat
                                            });
                                        }
                                    }
                                });

                                const getGridRowHeight = (index: number) => {
                                    const item = items[index];
                                    if (!item) return gridRowHeightBase + gridGap;
                                    if (item.type === 'filters') return (rootFontSize * 1.85) + (rootFontSize * 0.5);
                                    if (item.type === 'header') return groupHeaderHeight;
                                    return gridRowHeightBase + gridGap;
                                };

                                return (
                                    // @ts-ignore
                                    <List
                                        key={`grid-grouped-${rootFontSize}-${columnCount}`}
                                        className="virtualized-scroller grid"
                                        rowCount={items.length}
                                        rowHeight={getGridRowHeight}
                                        rowComponent={GroupedGridRowComponent as any}
                                        // @ts-ignore
                                        rowProps={{ 
                                            ...sharedProps, 
                                            groupedGridItems: items, 
                                            collapsedGroups, 
                                            onToggleGroup: handleToggleGroup,
                                            columnCount,
                                            columnWidth,
                                            gridGap,
                                            gridRowHeight: gridRowHeightBase
                                        }}
                                        listRef={listRef}
                                        style={{ height, width, overflowY: 'auto', overflowX: 'hidden' }}
                                        onScroll={handleScroll}
                                    />
                                );
                            }

                            if (!isGrid && (groupByDate || hasActiveFilters) && groupedDetailItems.length > 0) {
                                const finalWidth = Math.max(width, totalColumnWidth);
                                return (
                                    // @ts-ignore
                                    <List
                                        key={`list-grouped-${rootFontSize}`}
                                        className="virtualized-scroller details"
                                        rowCount={groupedDetailItems.length}
                                        rowHeight={getGroupedRowHeight}
                                        rowComponent={GroupedDetailsRow as any}
                                        // @ts-ignore
                                        rowProps={groupedSharedProps}
                                        listRef={listRef}
                                        style={{ height, width: finalWidth, overflowY: 'auto', overflowX: 'hidden' }}
                                        onScroll={handleScroll}
                                    />
                                );
                            }

                            if (isGrid) {
                                const minColumnWidth = rootFontSize * 6.0;
                                const horizontalPadding = rootFontSize * 2.5;
                                const columnCount = Math.max(1, Math.floor((width - horizontalPadding) / (minColumnWidth + gridGap)));
                                columnCountRef.current = columnCount;
                                const columnWidth = (width - horizontalPadding - (columnCount - 1) * gridGap) / columnCount;

                                const filtersHeight = hasActiveFilters ? (rootFontSize * 2.35) : 0;
                                return (
                                    <div style={{ height, width, display: 'flex', flexDirection: 'column' }}>
                                        {hasActiveFilters && activeFilters && (
                                            <div style={{ flexShrink: 0 }}>
                                                <FiltersRow activeFilters={activeFilters} t={t} style={{ position: 'relative', zIndex: 20, marginBottom: '0.5rem' }} />
                                            </div>
                                        )}
                                        {/* @ts-ignore */}
                                        <Grid
                                            key={`grid-${columnCount}-${rootFontSize}`}
                                            className="virtualized-scroller grid"
                                            columnCount={columnCount}
                                            columnWidth={columnWidth + gridGap}
                                            rowCount={Math.ceil(files.length / columnCount)}
                                            rowHeight={gridRowHeightBase + gridGap}
                                            cellComponent={GridCell as any}
                                            // @ts-ignore
                                            cellProps={{ ...sharedProps, columnCount }}
                                            gridRef={gridRef}
                                            style={{ height: height - filtersHeight, width, overflowY: 'auto', overflowX: 'hidden' }}
                                            onScroll={handleScroll}
                                        />
                                    </div>
                                );
                            }

                            const finalWidth = Math.max(width, totalColumnWidth);
                            return (
                                // @ts-ignore
                                <List
                                    key={`list-${rootFontSize}`}
                                    className="virtualized-scroller details"
                                    rowCount={files.length}
                                    rowHeight={listRowHeight}
                                    rowComponent={DetailsRow as any}
                                    // @ts-ignore
                                    rowProps={sharedProps}
                                    listRef={listRef}
                                    style={{ height, width: finalWidth, overflowY: 'auto', overflowX: 'hidden' }}
                                    onScroll={handleScroll}
                                />
                            );
                        })()}
                    </ErrorBoundary>
                );
            }} />
        </div>
    );
});
