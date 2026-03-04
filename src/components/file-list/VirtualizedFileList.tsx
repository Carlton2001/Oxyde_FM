import React, { useState, useEffect, useMemo, useRef, useImperativeHandle, useCallback } from 'react';
import { List, Grid, RowComponentProps, CellComponentProps } from 'react-window';
import { AutoSizer } from 'react-virtualized-auto-sizer';
import cx from 'classnames';
import { Check, Shield, Loader2, ChevronDown, ChevronRight } from 'lucide-react';

import { FileEntry, ViewMode, ColumnWidths, DateFormat } from '../../types';
import { TFunc } from '../../i18n';
import { useApp } from '../../context/AppContext';
import { useFileItemState } from '../../hooks/useFileItemState';
import { RenameInput } from './RenameInput';
import { getFileTypeString } from '../../utils/format';
import { getColumnMode, getVisibleColumns } from '../../config/columnDefinitions';
import { getDateCategoryForFile, DATE_CATEGORIES, DateCategoryKey } from './DateFilterMenu';

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

// --- Grouped item types ---
type GroupHeaderItem = { type: 'header'; category: DateCategoryKey; count: number };
type GroupFileItem = { type: 'file'; entry: FileEntry };
type GroupedItem = GroupHeaderItem | GroupFileItem;

// Category order for display
const CATEGORY_ORDER: DateCategoryKey[] = ['today', 'yesterday', 'this_week', 'last_week', 'this_month', 'older'];

// Memoized Details Row Component
const DetailsRow = React.memo((props: RowComponentProps<SharedItemProps>) => {
    const { index, style, ...sharedProps } = props;
    const {
        entries, isTrashView, isNetworkView, searchResults,
        t, dateFormat, getIcon, showHistogram, totalItemsSize, showCheckboxes,
        renameText, onRenameTextChange, onRenameCommit, onRenameCancel
    } = sharedProps;

    const mode = getColumnMode(!!isTrashView, !!searchResults, isNetworkView);
    const visibleCols = getVisibleColumns(mode);

    const entry = entries[index];
    if (!entry) return null;

    const { isSelected, isRenaming, isProtected, handlers, itemClassName, tooltipText } = useFileItemState({
        ...sharedProps,
        entry
    });

    const isDragOver = sharedProps.dragOverPath === entry.path;

    // Fixed width based on the calculated column sum
    const adjustedStyle = {
        ...style,
        width: 'max-content',
        minWidth: 'max-content'
    };

    return (
        <div
            className={cx(itemClassName, "details", {
                "drag-over": isDragOver
            })}
            style={adjustedStyle}
            data-path={entry.path}
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
        entries, renameText, showCheckboxes, getIcon,
        onRenameTextChange, onRenameCommit, onRenameCancel,
        columnCount = 1, rootFontSize, t
    } = sharedProps;

    const index = rowIndex * columnCount + columnIndex;
    const entry = entries[index];

    const { isSelected, isRenaming, isProtected, handlers, itemClassName, tooltipText } = useFileItemState({
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
            data-path={entry.path}
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
                    {isProtected && <Shield className="protected-shield-badge" size={16} fill="currentColor" />}
                </div>
                <div className="file-name-container">
                    {isRenaming ? (
                        <RenameInput
                            renameText={renameText}
                            onRenameTextChange={onRenameTextChange}
                            onRenameCommit={onRenameCommit}
                            onRenameCancel={onRenameCancel}
                            className="rename-input grid-mode"
                        />
                    ) : (
                        <>
                            <span className="file-name">
                                {!entry.is_dir && entry.name.lastIndexOf('.') > 0
                                    ? entry.name.slice(0, entry.name.lastIndexOf('.'))
                                    : entry.name}
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

// --- Grouped Details Row (handles both headers and file items) ---
interface GroupedSharedProps extends SharedItemProps {
    groupedItems: GroupedItem[];
    collapsedGroups: Set<DateCategoryKey>;
    onToggleGroup: (category: DateCategoryKey) => void;
}

const GroupedDetailsRow = React.memo((props: RowComponentProps<GroupedSharedProps>) => {
    const { index, style, ...sharedProps } = props;
    const { groupedItems, collapsedGroups, onToggleGroup, t } = sharedProps;

    const item = groupedItems[index];
    if (!item) return null;

    if (item.type === 'header') {
        const isCollapsed = collapsedGroups.has(item.category);
        const label = t(DATE_CATEGORIES[item.category] as any) || item.category;
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

    // File item — delegate to the standard DetailsRow logic
    const entry = item.entry;
    const {
        isTrashView, isNetworkView, searchResults,
        dateFormat, getIcon, showHistogram, totalItemsSize, showCheckboxes,
        renameText, onRenameTextChange, onRenameCommit, onRenameCancel
    } = sharedProps;

    const mode = getColumnMode(!!isTrashView, !!searchResults, isNetworkView);
    const visibleCols = getVisibleColumns(mode);

    const { isSelected, isRenaming, isProtected, handlers, itemClassName, tooltipText } = useFileItemState({
        ...sharedProps,
        entry
    });

    const isDragOver = sharedProps.dragOverPath === entry.path;

    const adjustedStyle = {
        ...style,
        width: 'max-content',
        minWidth: 'max-content'
    };

    return (
        <div
            className={cx(itemClassName, "details", { "drag-over": isDragOver })}
            style={adjustedStyle}
            data-path={entry.path}
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
                                {isProtected && <Shield className="protected-shield-badge" size={12} fill="currentColor" />}
                            </div>
                            <div className="file-name-container">
                                {isRenaming ? (
                                    <RenameInput renameText={renameText} onRenameTextChange={onRenameTextChange} onRenameCommit={onRenameCommit} onRenameCancel={onRenameCancel} />
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

// --- Grouped Grid: rendered as a flat scrollable list (no react-window Grid) ---
interface GroupedGridSectionProps {
    category: DateCategoryKey;
    files: FileEntry[];
    collapsed: boolean;
    onToggle: (cat: DateCategoryKey) => void;
    sharedProps: SharedItemProps;
    columnCount: number;
    columnWidth: number;
    gridGap: number;
    gridRowHeight: number;
}

const GroupedGridSection = React.memo<GroupedGridSectionProps>(({
    category, files, collapsed, onToggle, sharedProps, columnCount, columnWidth, gridGap, gridRowHeight
}) => {
    const { t } = sharedProps;
    const label = t(DATE_CATEGORIES[category] as any) || category;

    return (
        <div className="group-section grid">
            <div className="group-header grid" onClick={() => onToggle(category)}>
                <div className="group-header-content">
                    {collapsed ? <ChevronRight size={14} /> : <ChevronDown size={14} />}
                    <span className="group-header-label">{label}</span>
                    <span className="group-header-count">{t('items_count' as any, { count: files.length })}</span>
                </div>
            </div>
            {!collapsed && (
                <div className="group-grid-items" style={{
                    display: 'grid',
                    gridTemplateColumns: `repeat(${columnCount}, ${columnWidth}px)`,
                    gap: `${gridGap}px`,
                }}>
                    {files.map(entry => (
                        <GroupedGridItem key={entry.path} entry={entry} sharedProps={sharedProps} gridRowHeight={gridRowHeight} />
                    ))}
                </div>
            )}
        </div>
    );
});

const GroupedGridItem = React.memo<{ entry: FileEntry; sharedProps: SharedItemProps; gridRowHeight: number }>(({ entry, sharedProps, gridRowHeight }) => {
    const {
        renameText, showCheckboxes, getIcon,
        onRenameTextChange, onRenameCommit, onRenameCancel,
        rootFontSize, t
    } = sharedProps;

    const { isSelected, isRenaming, isProtected, handlers, itemClassName, tooltipText } = useFileItemState({
        ...sharedProps,
        entry
    });

    const isDragOver = sharedProps.dragOverPath === entry.path;

    return (
        <div
            className={cx(itemClassName, "grid", { "drag-over": isDragOver, "is-dir": entry.is_dir })}
            style={{ height: gridRowHeight }}
            data-path={entry.path}
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
                    {isProtected && <Shield className="protected-shield-badge" size={16} fill="currentColor" />}
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
        initialScrollOffset = 0, updateCurrentScroll
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
    const scrollContainerRef = useRef<HTMLDivElement>(null);

    useImperativeHandle(ref, () => ({
        scrollToTop: () => {
            // react-window v2 exposes an .element getter, not a scrollTo() method
            const listEl = listRef.current?.element;
            if (listEl) listEl.scrollTop = 0;
            const gridEl = gridRef.current?.element;
            if (gridEl) gridEl.scrollTop = 0;
            if (scrollContainerRef.current) scrollContainerRef.current.scrollTop = 0;
        }
    }));

    const isGrid = viewMode === 'grid';
    const cutPathsSet = useMemo(() => new Set(cutPaths), [cutPaths]);

    // --- Scroll Restoration ---
    const isRestoringRef = useRef(false);
    const currentScrollRef = useRef(0);
    const initialScrollOffsetRef = useRef(initialScrollOffset);

    useEffect(() => {
        initialScrollOffsetRef.current = initialScrollOffset;
    }, [initialScrollOffset]);

    // Secure async scroll restoration for react-window v2
    useEffect(() => {
        if (!files || files.length === 0) return;

        isRestoringRef.current = true;

        // Wait for react-window to create DOM element placeholders
        const timer = setTimeout(() => {
            const offset = initialScrollOffsetRef.current;
            if (isGrid && gridRef.current?.element) {
                gridRef.current.element.scrollTop = offset;
            } else if (!isGrid && listRef.current?.element) {
                listRef.current.element.scrollTop = offset;
            } else if (scrollContainerRef.current) {
                scrollContainerRef.current.scrollTop = offset;
            }

            // Allow a tiny moment for browser micro-tasks to apply the scroll before unblocking updates
            setTimeout(() => {
                isRestoringRef.current = false;
            }, 50);
        }, 50);

        return () => clearTimeout(timer);
        // eslint-disable-next-line react-hooks/exhaustive-deps
    }, [files.length, isGrid, viewMode, groupByDate]);

    // --- Collapsed groups state ---
    const [collapsedGroups, setCollapsedGroups] = useState<Set<DateCategoryKey>>(new Set());

    // Reset collapsed state when toggling groupByDate off/on or changing path
    useEffect(() => {
        setCollapsedGroups(new Set());
    }, [groupByDate]);

    const handleToggleGroup = useCallback((category: DateCategoryKey) => {
        setCollapsedGroups(prev => {
            const next = new Set(prev);
            if (next.has(category)) next.delete(category);
            else next.add(category);
            return next;
        });
    }, []);

    // --- Grouping logic ---
    const groupedByCategory = useMemo(() => {
        if (!groupByDate) return null;

        const groups = new Map<DateCategoryKey, FileEntry[]>();
        for (const cat of CATEGORY_ORDER) {
            groups.set(cat, []);
        }

        for (const file of files) {
            const cat = getDateCategoryForFile(file.modified || 0);
            groups.get(cat)!.push(file);
        }

        // Remove empty categories
        for (const [cat, entries] of groups) {
            if (entries.length === 0) groups.delete(cat);
        }

        return groups;
    }, [files, groupByDate]);

    // Flat list for details grouped mode
    const groupedDetailItems = useMemo<GroupedItem[]>(() => {
        if (!groupedByCategory) return [];

        const items: GroupedItem[] = [];
        for (const cat of CATEGORY_ORDER) {
            const entries = groupedByCategory.get(cat);
            if (!entries || entries.length === 0) continue;
            items.push({ type: 'header', category: cat, count: entries.length });
            if (!collapsedGroups.has(cat)) {
                for (const entry of entries) {
                    items.push({ type: 'file', entry });
                }
            }
        }
        return items;
    }, [groupedByCategory, collapsedGroups]);

    // Provide a flat entries array for grouped items (needed by SharedItemProps)
    const groupedEntries = useMemo(() => {
        if (!groupedByCategory) return files;
        return groupedDetailItems.filter((i): i is GroupFileItem => i.type === 'file').map(i => i.entry);
    }, [groupedByCategory, groupedDetailItems, files]);

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

    const groupedSharedProps = useMemo<GroupedSharedProps>(() => ({
        ...sharedProps,
        entries: groupedEntries,
        groupedItems: groupedDetailItems,
        collapsedGroups,
        onToggleGroup: handleToggleGroup
    }), [sharedProps, groupedEntries, groupedDetailItems, collapsedGroups, handleToggleGroup]);

    const listRowHeight = rootFontSize * 1.75;
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

    // Variable row height for grouped details mode
    const getGroupedRowHeight = useCallback((index: number): number => {
        const item = groupedDetailItems[index];
        if (!item) return listRowHeight;
        return item.type === 'header' ? groupHeaderHeight : listRowHeight;
    }, [groupedDetailItems, listRowHeight, groupHeaderHeight]);

    const scrollTimeoutRef = useRef<number | null>(null);

    const handleScroll = useCallback((e: React.UIEvent<HTMLElement> | React.UIEvent<HTMLDivElement>) => {
        const top = e.currentTarget.scrollTop;
        if (onScrollToggle) onScrollToggle(top > 100);

        if (!isRestoringRef.current && updateCurrentScroll) {
            currentScrollRef.current = top;

            if (scrollTimeoutRef.current) clearTimeout(scrollTimeoutRef.current);
            scrollTimeoutRef.current = setTimeout(() => {
                updateCurrentScroll(top);
            }, 250); // Debounce to prevent constant React re-renders during smooth scrolling
        }
    }, [onScrollToggle, updateCurrentScroll]);

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

                    // Determine the correct empty message
                    let emptyKey = 'empty';
                    if (searchResults) {
                        emptyKey = 'no_results';
                    } else if (isTrashView) {
                        emptyKey = 'recycle_bin_empty';
                    }

                    return (
                        <div className="empty-msg" style={{ width, height, position: 'absolute', top: 0, left: 0, display: 'flex', alignItems: 'center', justifyContent: 'center' }}>
                            <span>{t(emptyKey as any)}</span>
                        </div>
                    );
                }

                // --- GROUPED GRID MODE ---
                if (isGrid && groupByDate && groupedByCategory) {
                    const minColumnWidth = rootFontSize * 6.0;
                    const horizontalPadding = rootFontSize * 2.5;
                    const columnCount = Math.max(1, Math.floor((width - horizontalPadding) / (minColumnWidth + gridGap)));
                    const columnWidth = (width - horizontalPadding - (columnCount - 1) * gridGap) / columnCount;

                    return (
                        <div
                            ref={scrollContainerRef}
                            className="virtualized-scroller grid grouped-grid-scroller"
                            style={{ height, width, overflowX: 'hidden', overflowY: 'auto' }}
                            onScroll={handleScroll}
                        >
                            {CATEGORY_ORDER.map(cat => {
                                const catFiles = groupedByCategory.get(cat);
                                if (!catFiles || catFiles.length === 0) return null;
                                return (
                                    <GroupedGridSection
                                        key={cat}
                                        category={cat}
                                        files={catFiles}
                                        collapsed={collapsedGroups.has(cat)}
                                        onToggle={handleToggleGroup}
                                        sharedProps={sharedProps}
                                        columnCount={columnCount}
                                        columnWidth={columnWidth}
                                        gridGap={gridGap}
                                        gridRowHeight={gridRowHeightBase}
                                    />
                                );
                            })}
                        </div>
                    );
                }

                // --- GROUPED DETAILS MODE ---
                if (!isGrid && groupByDate && groupedDetailItems.length > 0) {
                    const finalWidth = Math.max(width, totalColumnWidth);

                    return (
                        <List
                            key={`list-grouped-${rootFontSize}`}
                            className="virtualized-scroller details"
                            rowCount={groupedDetailItems.length}
                            rowHeight={getGroupedRowHeight}
                            rowComponent={GroupedDetailsRow as any}
                            rowProps={groupedSharedProps}
                            listRef={listRef}
                            style={{ height, width: finalWidth, overflowY: 'auto', overflowX: 'hidden' }}
                            onScroll={handleScroll}
                        />
                    );
                }

                // --- STANDARD GRID MODE ---
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
                            onScroll={handleScroll}
                        />
                    );
                }

                // --- STANDARD DETAILS MODE ---
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
                        onScroll={handleScroll}
                    />
                );
            }} />
        </div>
    );
});
