import React, { useState, useRef, useCallback, useEffect, useMemo, useLayoutEffect } from 'react';
import cx from 'classnames';
import { ArrowUp, X, Search, Square } from 'lucide-react';
import { FilePanelHeader } from './FilePanelHeader';
import { FilePanelFooter } from './FilePanelFooter';
import { FileEntry, ViewMode, SortConfig, ColumnWidths, SortField, DriveInfo, LayoutMode } from '../../types';
import { FileHeader } from './FileHeader';
import { getFileEntryIcon, IMAGE_EXTENSIONS } from '../../utils/fileIcons';
import { Thumbnail } from '../ui/Thumbnail';
import { TFunc } from '../../i18n';
import { useSelectionMarquee } from '../../hooks/useSelectionMarquee';
import { SelectionMarquee } from './SelectionMarquee';
import { VirtualizedFileList, VirtualizedFileListHandle } from './VirtualizedFileList';
import { ExtensionFilterMenu } from './ExtensionFilterMenu';
import { SizeFilterMenu, getSizeCategoryForFile } from './SizeFilterMenu';
import { DateFilterMenu, getDateCategoryForFile, DateCategoryKey } from './DateFilterMenu';
import { NameFilterMenu } from './NameFilterMenu';
import { LocationFilterMenu } from './LocationFilterMenu';
import { useApp } from '../../context/AppContext';
import { useAutoFitColumns } from '../../hooks/useAutoFitColumns';
import { useFileStats } from '../../hooks/useFileStats';
import { getVisibleColumns, getColumnMode, buildGridTemplate, getOtherColumnsWidthSum, getFlexibleColumn, calculateIdealFlexWidth } from '../../config/columnDefinitions';
import { SizeCategoryKey } from '../../types';
import './FilePanel.css';

interface FilePanelProps {
    files: FileEntry[];
    viewMode: ViewMode;
    selected: Set<string>;
    isActive: boolean;
    currentPath: string;
    drives?: DriveInfo[];
    showDrives?: boolean;
    sortConfig: SortConfig;
    colWidths: ColumnWidths;
    onNavigate: (path: string) => void;
    onOpenFile: (path: string) => void;
    onSelect: (path: string, val: boolean, range: boolean) => void;
    onSelectMultiple: (paths: string[], isAdditive: boolean) => void;
    onClearSelection: () => void;
    onContextMenu: (e: React.MouseEvent, entry?: FileEntry) => void;
    onActivate: () => void;
    onFileDragStart: (entry: FileEntry) => void;
    onFileDrop?: (targetPath?: string, e?: React.MouseEvent) => void;
    isDragging: boolean;
    onSort: (field: SortField) => void;
    onResize: (field: keyof ColumnWidths, delta: number) => void;
    onResizeMultiple?: (updates: Partial<ColumnWidths>) => void;
    t: TFunc;
    searchQuery: string;
    searchResults: FileEntry[] | null;
    isSearching: boolean;
    onClearSearch: () => void;
    onCancelSearch?: () => void;
    isDragTarget?: boolean;
    dragOverPath?: string | null;
    showHidden?: boolean;
    showSystem?: boolean;
    layout: LayoutMode;
    cutPaths?: string[];
    onRename?: (oldPath: string, newName: string) => void;
    showHistogram?: boolean;
    isTrashView?: boolean;
    isNetworkView?: boolean;
    useSystemIcons?: boolean;
    onItemMiddleClick?: (entry: FileEntry, panelId: string) => void;
    diffPaths?: Set<string>;
    panelId: string;
    searchLimitReached?: boolean;
    onViewModeChange: (mode: ViewMode) => void;
    loading?: boolean;
    initialScrollOffset?: number;
    updateCurrentScroll?: (offset: number) => void;
    groupByDate: boolean;
    onGroupByDateChange: (val: boolean) => void;
    isProtected?: boolean;
    favorites?: import('../../types').QuickAccessItem[];

    // Filter Props
    extensionFilter: Set<string> | null;
    setExtensionFilter: (val: Set<string> | null | ((prev: Set<string> | null) => Set<string> | null)) => void;
    sizeFilter: Set<SizeCategoryKey> | null;
    setSizeFilter: (val: Set<SizeCategoryKey> | null | ((prev: Set<SizeCategoryKey> | null) => Set<SizeCategoryKey> | null)) => void;
    dateFilter: Set<string> | null;
    setDateFilter: (val: Set<string> | null | ((prev: Set<string> | null) => Set<string> | null)) => void;
    nameFilter: string | null;
    setNameFilter: (val: string | null) => void;
    locationFilter: string | null;
    setLocationFilter: (val: string | null) => void;
    deletedDateFilter: Set<string> | null;
    setDeletedDateFilter: (val: Set<string> | null | ((prev: Set<string> | null) => Set<string> | null)) => void;
    clearAllFilters: () => void;
    forwardPath?: string | null;
}

export const FilePanel: React.FC<FilePanelProps> = React.memo(({
    files, viewMode, selected, isActive, currentPath, drives, showDrives, sortConfig,
    colWidths, onNavigate, onOpenFile, onSelect, onSelectMultiple, onClearSelection,
    onContextMenu, onActivate, onFileDragStart, onFileDrop: _onFileDrop, isDragging, onSort,
    onResize, onResizeMultiple, t, searchQuery, searchResults, isSearching,
    onClearSearch, onCancelSearch, isDragTarget,
    dragOverPath, showHidden = false, showSystem = false, layout, cutPaths = [],
    onRename, showHistogram: propShowHistogram, isTrashView = false, isNetworkView = false,
    useSystemIcons: propUseSystemIcons, onItemMiddleClick, diffPaths, searchLimitReached,
    panelId, onViewModeChange, loading, initialScrollOffset, updateCurrentScroll,
    groupByDate, onGroupByDateChange, isProtected, favorites: _favorites,
    extensionFilter, setExtensionFilter, sizeFilter, setSizeFilter, dateFilter, setDateFilter,
    nameFilter, setNameFilter, locationFilter, setLocationFilter,
    deletedDateFilter, setDeletedDateFilter, clearAllFilters, forwardPath: _forwardPath
}) => {
    const { useSystemIcons: contextUseSystemIcons, searchLimit, showGridThumbnails, notify, showNetwork, showCheckboxes } = useApp();
    const useSystemIcons = propUseSystemIcons ?? contextUseSystemIcons;

    const currentDrive = drives?.find(d => currentPath.toLowerCase().startsWith(d.path.toLowerCase()));
    const isReadOnly = currentDrive?.is_readonly || false;

    const panelRef = useRef<HTMLDivElement>(null);
    const containerRef = useRef<HTMLDivElement>(null);
    const headerScrollRef = useRef<HTMLDivElement>(null);
    const filterScrollRef = useRef<HTMLDivElement>(null);
    const scrollHandleRef = useRef<VirtualizedFileListHandle>(null);
    const [showScrollTop, setShowScrollTop] = useState(false);
    const [mouseNearScrollbar, setMouseNearScrollbar] = useState(false);

    const [filterMenuAnchor, setFilterMenuAnchor] = useState<{ x: number, y: number } | null>(null);
    const [activeFilterMenu, setActiveFilterMenu] = useState<'extension' | 'size' | 'date' | 'name' | 'location' | 'deletedDate' | null>(null);



    const removeExtensionFilter = useCallback((ext: string) => {
        setExtensionFilter(prev => {
            if (!prev) return null;
            const next = new Set(prev);
            next.delete(ext);
            return next.size > 0 ? next : null;
        });
    }, [setExtensionFilter]);

    const removeSizeFilter = useCallback((cat: SizeCategoryKey) => {
        setSizeFilter(prev => {
            if (!prev) return null;
            const next = new Set(prev);
            next.delete(cat);
            return next.size > 0 ? next : null;
        });
    }, [setSizeFilter]);

    const removeDateFilter = useCallback((cat: string) => {
        setDateFilter(prev => {
            if (!prev) return null;
            const next = new Set(prev);
            next.delete(cat);
            return next.size > 0 ? next : null;
        });
    }, [setDateFilter]);

    const removeDeletedDateFilter = useCallback((cat: string) => {
        setDeletedDateFilter(prev => {
            if (!prev) return null;
            const next = new Set(prev);
            next.delete(cat);
            return next.size > 0 ? next : null;
        });
    }, [setDeletedDateFilter]);

    const activeFilters = useMemo(() => ({
        extensions: extensionFilter,
        sizes: sizeFilter,
        deletedDates: deletedDateFilter,
        date: dateFilter,
        name: nameFilter,
        location: locationFilter,
        onClearAll: clearAllFilters,
        onRemoveExtension: removeExtensionFilter,
        onRemoveSize: removeSizeFilter,
        onRemoveDate: removeDateFilter,
        onRemoveDeletedDate: removeDeletedDateFilter,
        onRemoveName: () => setNameFilter(null),
        onRemoveLocation: () => setLocationFilter(null),
        onClearExtensions: () => setExtensionFilter(null),
        onClearSizes: () => setSizeFilter(null),
        onClearDate: () => setDateFilter(null),
        onClearDeletedDates: () => setDeletedDateFilter(null),
    }), [extensionFilter, sizeFilter, deletedDateFilter, dateFilter, nameFilter, locationFilter, clearAllFilters, removeExtensionFilter, removeSizeFilter, removeDateFilter, removeDeletedDateFilter, setNameFilter, setLocationFilter, setExtensionFilter, setSizeFilter, setDateFilter, setDeletedDateFilter]);

    const currentMode = isTrashView ? 'trash' : (searchResults ? 'search' : 'normal');

    useEffect(() => {
        clearAllFilters();
        setFilterMenuAnchor(null);
        setActiveFilterMenu(null);
    }, [currentMode, clearAllFilters]);

    useEffect(() => {
        if (viewMode === 'grid') {
            clearAllFilters();
        }
    }, [viewMode, clearAllFilters]);

    // Intelligent distribution tracking
    const lastPanelWidthRef = useRef(0);
    const colWidthsRef = useRef(colWidths);
    colWidthsRef.current = colWidths;

    const mode = useMemo(() => getColumnMode(!!isTrashView, !!searchResults, isNetworkView), [isTrashView, searchResults, isNetworkView]);

    /**
     * Lightweight live-resize sync: only adjusts the Name (flex) column to fill available space.
     * Fixed columns are left untouched during active resize for performance (60fps re-renders).
     * Full proportional recalibration happens via autoFit once resize ends.
     */
    const syncFlexColumn = useCallback((currentPanelWidth: number) => {
        if (viewMode !== 'details' || !onResizeMultiple) return;

        const visibleCols = getVisibleColumns(mode);
        const flexCol = getFlexibleColumn(visibleCols);
        if (!flexCol) {
            lastPanelWidthRef.current = currentPanelWidth;
            return;
        }

        const otherColsSum = getOtherColumnsWidthSum(
            visibleCols,
            colWidthsRef.current as unknown as Record<string, number>,
            flexCol.key
        );
        const idealWidth = calculateIdealFlexWidth(currentPanelWidth, otherColsSum, flexCol.minWidth);
        const currentFlexWidth = colWidthsRef.current[flexCol.key as keyof ColumnWidths] || flexCol.defaultWidth;
        if (Math.abs(idealWidth - currentFlexWidth) > 2) {
            onResizeMultiple({ [flexCol.key]: idealWidth });
        }
        lastPanelWidthRef.current = currentPanelWidth;
    }, [viewMode, onResizeMultiple, mode]);

    // Use a ref for syncFlexColumn to avoid triggering the effect below on every render
    const syncFlexRef = useRef(syncFlexColumn);
    syncFlexRef.current = syncFlexColumn;

    // Re-sync when mode changes (e.g. switching to search or trash after a window resize)
    useEffect(() => {
        if (lastPanelWidthRef.current > 0) {
            syncFlexRef.current(lastPanelWidthRef.current);
        }
    }, [mode]);

    // Auto-fit instance (same as double-click on separator)
    const displayFiles = useMemo(() => searchResults ?? files, [searchResults, files]);
    const autoFit = useAutoFitColumns({
        panelRef: containerRef,
        files: displayFiles,
        searchResults: !!searchResults,
        isTrashView: !!isTrashView,
        t,
        onResizeMultiple,
        onResize,
    });
    const autoFitRef = useRef(autoFit);
    autoFitRef.current = autoFit;

    // Trigger auto-fit when path changes and files are loaded
    const lastAutoFitPathRef = useRef<string>('');
    useEffect(() => {
        if (viewMode !== 'details' || !displayFiles.length) return;
        if (lastAutoFitPathRef.current === currentPath) return;
        lastAutoFitPathRef.current = currentPath;
        const timer = setTimeout(() => autoFitRef.current(), 50);
        return () => clearTimeout(timer);
    }, [currentPath, displayFiles.length, viewMode]);

    // ResizeObserver: proportional sync during resize + autoFit for structural panel changes (>= 30px)
    // Uses refs to avoid recreating the observer on every render (which would cause choppy updates).
    const resizeEndTimerRef = useRef<ReturnType<typeof setTimeout> | null>(null);
    useLayoutEffect(() => {
        if (viewMode !== 'details' || !containerRef.current) return;

        const resizeObserver = new ResizeObserver(entries => {
            const width = entries[0]?.contentRect.width;
            if (!width || width <= 0) return;
            const delta = Math.abs(width - lastPanelWidthRef.current);
            if (delta > 2) syncFlexRef.current(width);
            // Only arm autoFit for structural panel resizes (sidebar toggle, layout change)
            // Column drag causes < 5px, sidebar toggle causes 100+px
            if (delta >= 30) {
                if (resizeEndTimerRef.current) clearTimeout(resizeEndTimerRef.current);
                resizeEndTimerRef.current = setTimeout(() => autoFitRef.current(), 250);
            }
        });

        resizeObserver.observe(containerRef.current);
        return () => {
            resizeObserver.disconnect();
            if (resizeEndTimerRef.current) clearTimeout(resizeEndTimerRef.current);
        };
    }, [viewMode]);

    const availableExtensions = React.useMemo(() => {
        const exts = new Set<string>();
        files.forEach(f => {
            if (f.is_system) { if (!showSystem) return; }
            else if (f.is_hidden) { if (!showHidden) return; }
            if (!f.is_dir) {
                const ext = f.name.includes('.') ? f.name.split('.').pop()?.toLowerCase() || '' : '';
                exts.add(ext);
            }
        });
        return Array.from(exts).sort();
    }, [files, showHidden, showSystem]);

    const availableSizeCategories = React.useMemo(() => {
        const cats = new Set<SizeCategoryKey>();
        files.forEach(f => {
            if (f.is_system) { if (!showSystem) return; }
            else if (f.is_hidden) { if (!showHidden) return; }
            if (!f.is_dir) {
                cats.add(getSizeCategoryForFile(f.size));
            }
        });
        return cats;
    }, [files, showHidden, showSystem]);

    const availableDateCategories = React.useMemo(() => {
        const cats = new Set<DateCategoryKey>();
        files.forEach(f => {
            if (f.is_system) { if (!showSystem) return; }
            else if (f.is_hidden) { if (!showHidden) return; }
            cats.add(getDateCategoryForFile(f.modified || 0));
        });
        return cats;
    }, [files, showHidden, showSystem]);

    const availableDeletedDateCategories = React.useMemo(() => {
        const cats = new Set<DateCategoryKey>();
        files.forEach(f => {
            if (f.is_system) { if (!showSystem) return; }
            else if (f.is_hidden) { if (!showHidden) return; }
            if (f.deleted_time) cats.add(getDateCategoryForFile(f.deleted_time));
        });
        return cats;
    }, [files, showHidden, showSystem]);

    const visibleFiles = React.useMemo(() => {
        const entriesToFilter = files;
        return entriesToFilter.filter(f => {
            if (f.is_system) return showSystem;
            if (f.is_hidden) return showHidden;

            if (extensionFilter && !f.is_dir) {
                const ext = f.name.includes('.') ? f.name.split('.').pop()?.toLowerCase() || '' : '';
                if (!extensionFilter.has(ext)) return false;
            }

            if (sizeFilter && !f.is_dir) {
                const cat = getSizeCategoryForFile(f.size);
                if (!sizeFilter.has(cat)) return false;
            }

            if (nameFilter) {
                if (!f.name.toLowerCase().includes(nameFilter.toLowerCase())) return false;
            }

            if (dateFilter) {
                const cat = getDateCategoryForFile(f.modified || 0);
                if (!dateFilter.has(cat)) return false;
            }

            if (locationFilter) {
                const pathToCheck = f.original_path || f.path;
                const lastSlashIndex = Math.max(pathToCheck.lastIndexOf('/'), pathToCheck.lastIndexOf('\\'));
                const dirPath = lastSlashIndex >= 0 ? pathToCheck.substring(0, lastSlashIndex) : pathToCheck;
                if (!dirPath.toLowerCase().includes(locationFilter.toLowerCase())) return false;
            }

            if (deletedDateFilter && isTrashView) {
                const cat = getDateCategoryForFile(f.deleted_time || 0);
                if (!deletedDateFilter.has(cat)) return false;
            }

            return true;
        });
    }, [files, searchResults, showHidden, showSystem, extensionFilter, sizeFilter, nameFilter, dateFilter, locationFilter, deletedDateFilter]);

    const finalFiles = visibleFiles;
    const { stats, totalStats } = useFileStats(finalFiles, selected);
    const totalItemsSize = totalStats.tSize;
    const showHistogram = propShowHistogram && (totalStats.allFoldersCalculated || !totalStats.hasFolders) && totalStats.tSize > 0;

    const getIcon = useCallback((entry: FileEntry, sizeOverride?: number) => {
        const isLarge = sizeOverride ? sizeOverride > 32 : viewMode !== 'details';
        const size = sizeOverride || (isLarge ? 36 : 16);
        const fallback = getFileEntryIcon(entry as any, { size }, useSystemIcons);
        const ext = entry.name.split('.').pop()?.toLowerCase() || '';

        // Only show for IMAGE_EXTENSIONS in grid mode if the specific setting is enabled
        if (showGridThumbnails && viewMode === 'grid' && !entry.is_dir && IMAGE_EXTENSIONS.includes(ext)) {
            return (
                <Thumbnail
                    path={entry.path}
                    name={entry.name}
                    isDir={entry.is_dir}
                    fallback={fallback}
                />
            );
        }

        return fallback;
    }, [viewMode, useSystemIcons, showGridThumbnails]);

    const [renamingPath, setRenamingPath] = useState<string | null>(null);
    const [renameText, setRenameText] = useState("");
    const lastClickTimeRef = useRef<number>(0);
    const selectedRef = useRef(selected);
    selectedRef.current = selected;

    const ignoreClickRef = useRef(false);
    const prevDraggingRef = useRef(isDragging);

    useEffect(() => {
        if (prevDraggingRef.current && !isDragging) {
            ignoreClickRef.current = true;
            const timer = setTimeout(() => { ignoreClickRef.current = false; }, 200);
            return () => clearTimeout(timer);
        }
        prevDraggingRef.current = isDragging;
    }, [isDragging]);

    const handleItemDoubleClick = useCallback((entry: FileEntry) => {
        if (entry.is_protected) {
            notify(t('protected_access'), 'warning');
            return;
        }
        if (entry.is_dir) onNavigate(entry.path);
        else onOpenFile(entry.path);
    }, [onNavigate, onOpenFile, t, notify]);

    const handleItemContextMenu = useCallback((entry: FileEntry, e: React.MouseEvent) => {
        const currentSelected = selectedRef.current;
        let isSelected = currentSelected.has(entry.path);
        if (!isSelected) {
            const lowerPath = entry.path.toLowerCase();
            for (const p of currentSelected) {
                if (p.toLowerCase() === lowerPath) { isSelected = true; break; }
            }
        }
        if (!isSelected) onSelect(entry.path, false, false);
        onContextMenu(e, entry);
        onActivate();
    }, [onSelect, onContextMenu, onActivate]);

    const handleItemClick = useCallback((entry: FileEntry, e: React.MouseEvent) => {
        e.stopPropagation();
        if (e.button !== 0 || isDragging || isMarqueeRef.current || ignoreClickRef.current || e.detail > 1) return;

        const currentSelected = selectedRef.current;
        const now = Date.now();
        const isAlreadySelected = currentSelected.has(entry.path) && currentSelected.size === 1;
        if (isAlreadySelected && !isReadOnly && !isTrashView) {
            const timeSinceLastClick = now - lastClickTimeRef.current;
            if (timeSinceLastClick > 500 && timeSinceLastClick < 2000) {
                setRenamingPath(entry.path);
                setRenameText(entry.name);
                return;
            }
        }
        lastClickTimeRef.current = now;
        if (!e.shiftKey && !e.ctrlKey) onSelect(entry.path, false, false);
        else onSelect(entry.path, e.ctrlKey, e.shiftKey);
    }, [isDragging, onSelect, isReadOnly, isTrashView]);

    const commitRename = useCallback(() => {
        if (renamingPath && onRename && renameText.trim() !== "") {
            const originalFile = files.find(f => f.path === renamingPath);
            if (originalFile && originalFile.name !== renameText) {
                onRename(renamingPath, renameText);
            }
        }
        setRenamingPath(null);
    }, [renamingPath, onRename, renameText, files]);

    const cancelRename = useCallback(() => {
        setRenamingPath(null);
        setRenameText("");
    }, []);

    useEffect(() => {
        cancelRename();
    }, [currentPath, cancelRename]);

    // Sync header horizontal scroll with the virtualized list's horizontal scroll
    useEffect(() => {
        const container = containerRef.current;
        const headerScroll = headerScrollRef.current;
        if (!container || !headerScroll) return;

        const scrollEl = container.querySelector('.virtualized-list') as HTMLElement | null;
        if (!scrollEl) return;

        const onScroll = () => {
            headerScroll.scrollLeft = scrollEl.scrollLeft;
            if (filterScrollRef.current) {
                filterScrollRef.current.scrollLeft = scrollEl.scrollLeft;
            }
        };

        scrollEl.addEventListener('scroll', onScroll, { passive: true });
        return () => scrollEl.removeEventListener('scroll', onScroll);
    });

    const {
        selectionRect,
        pendingSelection,
        isMarqueeRef,
        handleMouseDown: handleListMouseDown,
    } = useSelectionMarquee(
        containerRef as React.RefObject<HTMLDivElement>,
        onSelectMultiple,
        onClearSelection,
        onActivate,
        isDragging,
        renamingPath,
        isActive,
        cancelRename
    );

    const handleHeaderClick = (e: React.MouseEvent) => {
        e.stopPropagation();
        const allSelected = finalFiles.length > 0 && finalFiles.every(f => selected.has(f.path));
        if (allSelected) onClearSelection();
        else onSelectMultiple(finalFiles.map(f => f.path), false);
    };

    const handleHeaderContextMenu = useCallback((field: keyof ColumnWidths, e: React.MouseEvent) => {
        if (field === 'type') {
            e.preventDefault();
            e.stopPropagation();
            if (availableExtensions.length > 0) {
                setActiveFilterMenu('extension');
                setFilterMenuAnchor({ x: e.clientX, y: e.clientY });
            }
        } else if (field === 'size') {
            e.preventDefault();
            e.stopPropagation();
            setActiveFilterMenu('size');
            setFilterMenuAnchor({ x: e.clientX, y: e.clientY });
        } else if (field === 'name') {
            e.preventDefault();
            e.stopPropagation();
            setActiveFilterMenu('name');
            setFilterMenuAnchor({ x: e.clientX, y: e.clientY });
        } else if (field === 'date') {
            e.preventDefault();
            e.stopPropagation();
            setActiveFilterMenu('date');
            setFilterMenuAnchor({ x: e.clientX, y: e.clientY });
        } else if (field === 'location') {
            e.preventDefault();
            e.stopPropagation();
            setActiveFilterMenu('location');
            setFilterMenuAnchor({ x: e.clientX, y: e.clientY });
        } else if (field === 'deletedDate') {
            e.preventDefault();
            e.stopPropagation();
            setActiveFilterMenu('deletedDate');
            setFilterMenuAnchor({ x: e.clientX, y: e.clientY });
        }
    }, [availableExtensions.length]);

    return (
        <div
            ref={panelRef}
            className={cx("panel", layout, { active: isActive, 'drag-over': isDragTarget })}
            data-panel-id={panelId}
            onClick={() => onActivate()}
            onMouseMove={(e) => {
                if (panelRef.current) {
                    const rect = panelRef.current.getBoundingClientRect();
                    const distFromRight = rect.right - e.clientX;
                    const isNear = distFromRight < 100;
                    if (isNear !== mouseNearScrollbar) setMouseNearScrollbar(isNear);
                }
            }}
            onMouseLeave={() => setMouseNearScrollbar(false)}
            style={viewMode === 'details' ? {
                '--grid-template': buildGridTemplate(
                    getVisibleColumns(getColumnMode(!!isTrashView, !!searchResults, isNetworkView)),
                    colWidths as unknown as Record<string, number>
                ),
            } as React.CSSProperties : undefined}
        >
            {showDrives && (
                <div className="panel-navigation">
                    <FilePanelHeader
                        currentPath={currentPath}
                        drives={drives || []}
                        showDrives={showDrives || false}
                        onNavigate={onNavigate}
                        onContextMenu={onContextMenu}
                        showNetwork={showNetwork}
                        t={t}
                    />
                </div>
            )}

            <div className="file-view-container">
                {searchLimitReached && (
                    <div className="search-limit-banner">
                        <span className="search-limit-icon">⚠️</span>
                        <span>{t('search_limit_reached', { count: searchLimit })}</span>
                    </div>
                )}

                {searchResults && (
                    <div className="search-results-banner">
                        <div className="search-results-info">
                            <Search size={14} style={{ color: 'var(--accent-color)' }} />
                            <span className="search-results-label">{t('search_results' as any)}</span>
                            <span className="search-results-query">"{searchQuery}"</span>
                        </div>
                        <div className="search-results-actions">
                            {isSearching && (
                                <button
                                    className="clear-search-premium-btn"
                                    onClick={onCancelSearch}
                                    data-tooltip={t('stop' as any)}
                                >
                                    <Square size={10} fill="currentColor" />
                                    <span>{t('stop' as any)}</span>
                                </button>
                            )}
                            <button
                                className="clear-search-premium-btn"
                                onClick={onClearSearch}
                            >
                                <X size={14} />
                                <span>{t('clear' as any)}</span>
                            </button>
                        </div>
                    </div>
                )}
                <div
                    className={cx("header-selection-gutter", { "with-checkbox": showCheckboxes })}
                    onClick={handleHeaderClick}
                    data-tooltip={t(finalFiles.length > 0 && finalFiles.every(f => selected.has(f.path)) ? ('deselect_all' as any) : 'select_all')}
                    data-tooltip-pos="right"
                />

                {viewMode === 'details' && (
                    <div className="file-header-scroll-wrapper" ref={headerScrollRef}>
                        <FileHeader
                            viewMode={viewMode}
                            searchResults={searchResults}
                            isTrashView={isTrashView}
                            isNetworkView={isNetworkView}
                            finalFiles={finalFiles}
                            sortConfig={sortConfig}
                            colWidths={colWidths}
                            onSort={onSort}
                            onResize={onResize}
                            onResizeMultiple={onResizeMultiple}
                            onClearSearch={onClearSearch}
                            onSelectAll={handleHeaderClick}
                            onHeaderContextMenu={handleHeaderContextMenu}
                            isTypeFiltered={extensionFilter !== null}
                            isSizeFiltered={sizeFilter !== null}
                            isNameFiltered={nameFilter !== null}
                            isLocationFiltered={locationFilter !== null}
                            isDeletedDateFiltered={deletedDateFilter !== null}
                            isDateFiltered={dateFilter !== null}
                            t={t}
                            panelRef={panelRef}
                            selected={selected}
                        />
                    </div>
                )} {/* Filters bar is now integrated into VirtualizedFileList */}

                <div
                    className={cx("file-list", viewMode, { "search-mode": !!searchResults, "trash-mode": isTrashView, "virtualized": true })}
                    onClick={(e) => {
                        if (isDragging || isMarqueeRef.current) return;
                        const isFileItem = (e.target as HTMLElement).closest('.file-item');
                        if (!isFileItem) {
                            onActivate();
                            if (renamingPath) cancelRename();
                        }
                    }}
                    onContextMenu={(e) => { e.preventDefault(); onClearSelection(); onContextMenu(e); onActivate(); }}
                    ref={containerRef}
                    onMouseDown={handleListMouseDown}
                >
                    <SelectionMarquee selectionRect={selectionRect} containerRef={containerRef as React.RefObject<HTMLDivElement>} />
                    <VirtualizedFileList
                        files={finalFiles}
                        viewMode={viewMode}
                        selected={selected}
                        pendingSelection={pendingSelection}
                        searchResults={searchResults}
                        renamingPath={renamingPath}
                        renameText={renameText}
                        isDragging={isDragging}
                        dragOverPath={dragOverPath || null}
                        cutPaths={cutPaths}
                        t={t}
                        onItemClick={handleItemClick}
                        onItemDoubleClick={handleItemDoubleClick}
                        onItemContextMenu={handleItemContextMenu}
                        onFileDragStart={onFileDragStart}
                        onRenameTextChange={setRenameText}
                        onRenameCommit={commitRename}
                        onRenameCancel={cancelRename}
                        getIcon={getIcon}
                        totalItemsSize={totalItemsSize}
                        showHistogram={!!showHistogram}
                        isTrashView={isTrashView}
                        isNetworkView={isNetworkView}
                        ref={scrollHandleRef}
                        onScrollToggle={setShowScrollTop}
                        onItemMiddleClick={onItemMiddleClick ? (entry) => onItemMiddleClick(entry, panelId) : undefined}
                        diffPaths={diffPaths}
                        colWidths={colWidths}
                        isSearching={isSearching}
                        loading={loading}
                        groupByDate={groupByDate}
                        initialScrollOffset={initialScrollOffset}
                        updateCurrentScroll={updateCurrentScroll}
                        isProtected={isProtected}
                        activeFilters={activeFilters}
                        currentPath={currentPath}
                        onNavigate={onNavigate}
                    />
                </div>
            </div>

            {showScrollTop && mouseNearScrollbar && (
                <button
                    className="scroll-to-top-btn"
                    onClick={(e) => {
                        e.stopPropagation();
                        scrollHandleRef.current?.scrollToTop();
                    }}
                    data-tooltip={t('scroll_to_top' as any) || "Top"}
                >
                    <ArrowUp size={16} />
                </button>
            )}

            <FilePanelFooter
                stats={stats}
                viewMode={viewMode}
                onViewModeChange={onViewModeChange}
                onActivate={onActivate}
                t={t}
                groupByDate={groupByDate}
                onGroupByDateChange={onGroupByDateChange}
            />

            {filterMenuAnchor && activeFilterMenu === 'extension' && (
                <ExtensionFilterMenu
                    x={filterMenuAnchor.x}
                    y={filterMenuAnchor.y}
                    availableExtensions={availableExtensions}
                    selectedExtensions={extensionFilter}
                    onChange={setExtensionFilter}
                    onClose={() => setFilterMenuAnchor(null)}
                    t={t}
                />
            )}

            {filterMenuAnchor && activeFilterMenu === 'size' && (
                <SizeFilterMenu
                    x={filterMenuAnchor.x}
                    y={filterMenuAnchor.y}
                    selectedSizes={sizeFilter}
                    availableSizeCategories={availableSizeCategories}
                    onChange={setSizeFilter}
                    onClose={() => setFilterMenuAnchor(null)}
                    t={t}
                />
            )}

            {filterMenuAnchor && activeFilterMenu === 'date' && (
                <DateFilterMenu
                    x={filterMenuAnchor.x}
                    y={filterMenuAnchor.y}
                    selectedDates={dateFilter as Set<DateCategoryKey>}
                    availableDateCategories={availableDateCategories}
                    onChange={(val) => setDateFilter(val as Set<string>)}
                    onClose={() => setFilterMenuAnchor(null)}
                    t={t}
                />
            )}

            {filterMenuAnchor && activeFilterMenu === 'name' && (
                <NameFilterMenu
                    x={filterMenuAnchor.x}
                    y={filterMenuAnchor.y}
                    value={nameFilter || ''}
                    onChange={setNameFilter}
                    onClose={() => setFilterMenuAnchor(null)}
                    t={t}
                />
            )}

            {filterMenuAnchor && activeFilterMenu === 'location' && (
                <LocationFilterMenu
                    x={filterMenuAnchor.x}
                    y={filterMenuAnchor.y}
                    value={locationFilter || ''}
                    onChange={setLocationFilter}
                    onClose={() => setFilterMenuAnchor(null)}
                    t={t}
                />
            )}

            {filterMenuAnchor && activeFilterMenu === 'deletedDate' && (
                <DateFilterMenu
                    x={filterMenuAnchor.x}
                    y={filterMenuAnchor.y}
                    selectedDates={deletedDateFilter as Set<DateCategoryKey>}
                    availableDateCategories={availableDeletedDateCategories}
                    onChange={(val) => setDeletedDateFilter(val as Set<string>)}
                    onClose={() => setFilterMenuAnchor(null)}
                    t={t}
                />
            )}
        </div>
    );
});

