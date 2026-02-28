
import React, { useState, useRef, useCallback, useEffect } from 'react';
import cx from 'classnames';
import { ArrowUp } from 'lucide-react';
import { FilePanelHeader } from './FilePanelHeader';
import { FilePanelFooter } from './FilePanelFooter';
import { FileEntry, ViewMode, SortConfig, ColumnWidths, SortField, DriveInfo, LayoutMode } from '../../types';
import { PathBar } from '../layout/PathBar';
import { FileHeader } from './FileHeader';
import { getFileEntryIcon, IMAGE_EXTENSIONS } from '../../utils/fileIcons';
import { Thumbnail } from '../ui/Thumbnail';
import { TFunc } from '../../i18n';
import { useSelectionMarquee } from '../../hooks/useSelectionMarquee';
import { SelectionMarquee } from './SelectionMarquee';
import { VirtualizedFileList, VirtualizedFileListHandle } from './VirtualizedFileList';
import { ExtensionFilterMenu } from './ExtensionFilterMenu';
import { SizeFilterMenu, getSizeCategoryForFile, SizeCategoryKey } from './SizeFilterMenu';
import { DateFilterMenu, getDateCategoryForFile, DateCategoryKey } from './DateFilterMenu';
import { NameFilterMenu } from './NameFilterMenu';
import { LocationFilterMenu } from './LocationFilterMenu';
import { useApp } from '../../context/AppContext';
import { useFileStats } from '../../hooks/useFileStats';
import { getVisibleColumns, getColumnMode, buildGridTemplate } from '../../config/columnDefinitions';
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
    onFileDrop: (targetPath?: string, e?: React.MouseEvent) => void;
    isDragging: boolean;
    onSort: (field: SortField) => void;
    onResize: (field: keyof ColumnWidths, delta: number) => void;
    onResizeMultiple?: (updates: Partial<ColumnWidths>) => void;
    t: TFunc;
    searchQuery: string;
    searchResults: FileEntry[] | null;
    isSearching: boolean;
    onSearch: () => void;
    onQueryChange: (query: string) => void;
    onClearSearch: () => void;
    showSearch?: boolean;
    isDragTarget?: boolean;
    dragOverPath?: string | null;
    showHidden?: boolean;
    showSystem?: boolean;
    layout: LayoutMode;
    cutPaths?: string[];
    onRename?: (oldPath: string, newName: string) => void;
    showHistogram?: boolean;
    isTrashView?: boolean;
    useSystemIcons?: boolean;
    onItemMiddleClick?: (entry: FileEntry) => void;
    diffPaths?: Set<string>;
    panelId: string;
    searchLimitReached?: boolean;
    onViewModeChange: (mode: ViewMode) => void;
    loading?: boolean;
}

export const FilePanel: React.FC<FilePanelProps> = React.memo(({
    files, viewMode, selected, isActive, currentPath, drives, showDrives, sortConfig,
    colWidths, onNavigate, onOpenFile, onSelect, onSelectMultiple, onClearSelection,
    onContextMenu, onActivate, onFileDragStart, onFileDrop, isDragging, onSort,
    onResize, onResizeMultiple, t, searchQuery, searchResults, isSearching,
    onSearch, onQueryChange, onClearSearch, showSearch = true, isDragTarget,
    dragOverPath, showHidden = false, showSystem = false, layout, cutPaths = [],
    onRename, showHistogram: propShowHistogram, isTrashView = false,
    useSystemIcons: propUseSystemIcons, onItemMiddleClick, diffPaths, searchLimitReached,
    panelId, onViewModeChange, loading
}) => {
    const { useSystemIcons: contextUseSystemIcons, searchLimit, showGridThumbnails, notify } = useApp();
    const useSystemIcons = propUseSystemIcons ?? contextUseSystemIcons;

    const currentDrive = drives?.find(d => currentPath.toLowerCase().startsWith(d.path.toLowerCase()));
    const isReadOnly = currentDrive?.is_readonly || false;

    const panelRef = useRef<HTMLDivElement>(null);
    const containerRef = useRef<HTMLDivElement>(null);
    const headerScrollRef = useRef<HTMLDivElement>(null);
    const scrollHandleRef = useRef<VirtualizedFileListHandle>(null);
    const [showScrollTop, setShowScrollTop] = useState(false);
    const [mouseNearScrollbar, setMouseNearScrollbar] = useState(false);

    const [extensionFilter, setExtensionFilter] = useState<Set<string> | null>(null);
    const [sizeFilter, setSizeFilter] = useState<Set<SizeCategoryKey> | null>(null);
    const [dateFilter, setDateFilter] = useState<Set<string> | null>(null);
    const [nameFilter, setNameFilter] = useState<string | null>(null);
    const [locationFilter, setLocationFilter] = useState<string | null>(null);
    const [deletedDateFilter, setDeletedDateFilter] = useState<Set<string> | null>(null);
    const [filterMenuAnchor, setFilterMenuAnchor] = useState<{ x: number, y: number } | null>(null);
    const [activeFilterMenu, setActiveFilterMenu] = useState<'extension' | 'size' | 'date' | 'name' | 'location' | 'deletedDate' | null>(null);

    useEffect(() => {
        setExtensionFilter(null);
        setSizeFilter(null);
        setDateFilter(null);
        setNameFilter(null);
        setFilterMenuAnchor(null);
        setActiveFilterMenu(null);
    }, [currentPath]);

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
        return files.filter(f => {
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

            if (deletedDateFilter) {
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
            onClick={() => onActivate()}
            onMouseUp={(e) => { if (isDragging) onFileDrop(currentPath, e); }}
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
                    getVisibleColumns(getColumnMode(!!isTrashView, !!searchResults)),
                    colWidths as unknown as Record<string, number>
                ),
            } as React.CSSProperties : undefined}
        >
            {(layout === 'dual' || showDrives || showSearch) && (
                <div className="panel-navigation">
                    {layout === 'dual' && (
                        <div className="panel-path-bar">
                            <PathBar
                                className="path-bar"
                                path={currentPath}
                                onNavigate={onNavigate}
                                isDragging={isDragging}
                                onDrop={(p) => onFileDrop(p || currentPath)}
                                drives={drives || []}
                                showHidden={showHidden}
                                panelId={panelId}
                                t={t}
                            />
                        </div>
                    )}
                    {(showDrives || showSearch) && (
                        <FilePanelHeader
                            currentPath={currentPath}
                            drives={drives || []}
                            showDrives={showDrives || false}
                            onNavigate={onNavigate}
                            onContextMenu={onContextMenu}
                            showSearch={showSearch}
                            searchQuery={searchQuery}
                            isSearching={isSearching}
                            onQueryChange={onQueryChange}
                            onSearch={onSearch}
                            onClearSearch={onClearSearch}
                            t={t}
                        />
                    )}
                </div>
            )}

            <div className="file-view-container">
                {searchLimitReached && (
                    <div className="search-limit-banner">
                        <span className="search-limit-icon">⚠️</span>
                        <span>{t('search_limit_reached', { count: searchLimit })}</span>
                    </div>
                )}
                <div className="file-header-scroll-wrapper" ref={headerScrollRef}>
                    <FileHeader
                        viewMode={viewMode}
                        searchResults={searchResults}
                        isTrashView={isTrashView}
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
                    />
                </div>

                <div
                    className={cx("file-list", viewMode, { "search-mode": !!searchResults, "trash-mode": isTrashView, "virtualized": true })}
                    onClick={(e) => {
                        if (isDragging || isMarqueeRef.current) return;
                        const isFileItem = (e.target as HTMLElement).closest('.file-item');
                        if (!isFileItem) {
                            onClearSelection();
                            onActivate();
                            if (renamingPath) cancelRename();
                        }
                    }}
                    onContextMenu={(e) => { e.preventDefault(); onContextMenu(e); onActivate(); }}
                    ref={containerRef}
                    onMouseDown={handleListMouseDown}
                    onMouseUp={(e) => {
                        if (isDragging) {
                            e.stopPropagation();
                            onFileDrop(dragOverPath || currentPath, e);
                        }
                    }}
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
                        ref={scrollHandleRef}
                        onScrollToggle={setShowScrollTop}
                        onItemMiddleClick={onItemMiddleClick}
                        diffPaths={diffPaths}
                        colWidths={colWidths}
                        isSearching={isSearching}
                        loading={loading}
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

