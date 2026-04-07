import { useRef, useMemo, useCallback } from 'react';
import { Sidebar } from '../layout/Sidebar';
import { FilePanel } from '../file-list/FilePanel';
import { TopBar } from '../layout/TopBar';
import { TitleBar } from '../layout/TitleBar';
import cx from 'classnames';
import { PanelState, DriveInfo, FileEntry, SortField, ColumnWidths, ViewMode, SizeCategoryKey, PanelId } from '../../types';
import { Tabs } from '../ui/Tabs';
import { TFunc } from '../../i18n';
import { useApp } from '../../context/AppContext';

interface FullPanelState extends PanelState {
    goUp: () => void;
    goBack: () => void;
    goForward: () => void;
    goToIndex: (index: number) => void;
    setViewMode: (mode: ViewMode) => void;
    currentEntry?: { scrollOffset?: number };
    updateCurrentScroll?: (offset: number) => void;
    setGroupByDate: (val: boolean) => void;
    isProtected: boolean;
    // Filters
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
}

interface DualPanelLayoutProps {
    t: TFunc;
    // Sidebar props
    sidebarReduced: boolean;
    setSidebarReduced: (val: boolean) => void;
    drives: DriveInfo[];
    left: FullPanelState;
    right: FullPanelState;
    // File Ops History
    canUndo: boolean;
    undoLabel?: string;
    canRedo: boolean;
    redoLabel?: string;
    // Panel props
    activePanelId: 'left' | 'right';
    setActivePanelId: (id: 'left' | 'right') => void;
    layout: 'standard' | 'dual';
    // Search props
    searchQuery: { left: string; right: string };
    // Callbacks
    navigate: (id: 'left' | 'right', path: string) => void;
    handleSearch: (id: 'left' | 'right', query: string) => void;
    executeSearch: (id: 'left' | 'right') => void;
    openAdvancedSearch: (id: 'left' | 'right') => void;
    clearSearch: (id: 'left' | 'right') => void;
    handleCancelSearch: (id: 'left' | 'right') => void;
    // Handlers
    handleDragStart: (id: 'left' | 'right', files: FileEntry[]) => void;
    handleDrop: (e: React.DragEvent | React.MouseEvent | undefined, targetPath: string | null, currentPath: string) => void;
    dragState: { sourcePanel: 'left' | 'right'; files: FileEntry[] } | null;
    handleSelect: (id: 'left' | 'right', path: string, selected: boolean, range: boolean) => void;
    handleSelectMultiple: (id: 'left' | 'right', paths: string[], isAdditive: boolean) => void;
    handleClearSelection: (id: 'left' | 'right') => void;
    handleContextMenu: (e: React.MouseEvent, id: 'left' | 'right', entry?: FileEntry) => void;
    handleOpenFile: (path: string, id: 'left' | 'right') => void;
    handleSort: (id: 'left' | 'right', field: SortField) => void;
    handleResize: (id: 'left' | 'right', field: keyof ColumnWidths, delta: number) => void;
    handleResizeMultiple: (id: 'left' | 'right', updates: Partial<ColumnWidths>) => void;
    handleInlineRename: (oldPath: string, newPath: string) => void;
    // State
    propPaths: any;
    propShowHidden: boolean;
    propShowSystem: boolean;
    cutPaths: string[];
    useSystemIcons: boolean;
    // Tree
    treeRef: any;
    onTreeCut: (paths: string[]) => void;
    onTreeCopy: (paths: string[]) => void;
    onTreeCopyName: (name: string) => void;
    onTreeCopyPath: (path: string) => void;
    onTreeDelete: (paths: string[]) => void;
    isShiftPressed?: boolean;
    onTreeRename: (path: string) => void;
    onTreeNewFolder: (path: string) => void;
    onTreeUnmount: (path: string) => void;
    onTreeDisconnectDrive: (path: string) => void;
    onTreeProperties: (path: string) => void;
    onTreePaste: (path: string) => void;
    onCalculateAllSizes: () => void;
    histogramPanels: Set<PanelId>;
    onTabDrop: (files: any[], index?: number) => void;
    // TopBar (only what TopBar still needs)
    setShowAbout: (show: boolean) => void;
    onLayoutChange: (mode: 'standard' | 'dual') => void;
    showHidden: boolean;
    // Actions
    onRefresh: () => void;
    onRestoreAll?: () => void;
    onRestoreSelected?: () => void;
    onEmptyTrash?: () => void;
    // Clipboard / Edit Actions
    handleCopy: () => void;
    handleCopyName: () => void;
    handleCopyPath: () => void;
    handleCut: () => void;
    handlePaste: () => void;
    handleDelete: () => void;
    handleUndo: () => void;
    handleRedo: () => void;
    canPaste: boolean;
    // Tabs
    onTabSwitch?: (id: string, path?: string, panelId?: PanelId) => void;
    onTabClose?: (id: string, panelId?: PanelId) => void;
    onItemMiddleClick?: (entry: FileEntry, panelId?: PanelId) => void;
    onOpenNewTab?: (path: string) => void;
    isTrashEmpty: boolean;
    favorites: import('../../types').QuickAccessItem[];
    onDuplicateSearch?: () => void;
    onTrashProperties: () => void;
    dragOverPath: string | null;
    setDragOverPath: (p: string | null) => void;
    onAddToFavorites: (path: string) => void;
    onRemoveFromFavorites: (path: string) => void;
    onDriveContextMenu: (e: React.MouseEvent, p: string) => void;
}

export const DualPanelLayout: React.FC<DualPanelLayoutProps> = ({
    t,
    sidebarReduced,
    setSidebarReduced,
    drives,
    left,
    right,
    canUndo,
    undoLabel,
    canRedo,
    redoLabel,
    activePanelId,
    setActivePanelId,
    layout,
    navigate,
    handleSearch,
    executeSearch,
    openAdvancedSearch,
    clearSearch,
    handleCancelSearch,
    handleDragStart,
    handleDrop,
    dragState,
    handleSelect,
    handleSelectMultiple,
    handleClearSelection,
    handleContextMenu,
    handleOpenFile,
    handleSort,
    handleResize,
    handleResizeMultiple,
    handleInlineRename,
    propShowHidden,
    propShowSystem,
    cutPaths,
    useSystemIcons,
    treeRef,
    onTreeCut,
    onTreeCopy,
    onTreeCopyName,
    onTreeCopyPath,
    onTreeDelete,
    isShiftPressed,
    onTreeRename,
    onTreeNewFolder,
    onTreeUnmount,
    onTreeDisconnectDrive,
    onTreeProperties,
    onTreePaste,
    onCalculateAllSizes,
    histogramPanels,
    setShowAbout,
    showHidden,
    onRefresh,
    onRestoreAll,
    onRestoreSelected,
    onEmptyTrash,
    handleCopy,
    handleCopyName,
    handleCopyPath,
    handleCut,
    handlePaste,
    handleDelete,
    handleUndo,
    handleRedo,
    canPaste,
    onTabSwitch,
    onTabClose,
    onItemMiddleClick,
    onOpenNewTab,
    onLayoutChange,
    onDuplicateSearch,
    onTrashProperties,
    dragOverPath,
    setDragOverPath,
    onTabDrop,
    onAddToFavorites,
    onRemoveFromFavorites,
    onDriveContextMenu,
    isTrashEmpty,
    favorites
}) => {
    const { driveTrashConfigs } = useApp();
    const activePanel = activePanelId === 'left' ? left : right;

    const getIsNukeOverride = (p: FullPanelState) => {
        if (!p.path) return false;
        const targetDriveMatch = p.path.match(/^([a-zA-Z]:)/);
        if (!targetDriveMatch) return false;
        const targetDrive = targetDriveMatch[1].toLowerCase();
        return driveTrashConfigs[targetDrive]?.nukeOnDelete || false;
    };

    const leftIsNukeOverride = useMemo(() => getIsNukeOverride(left), [left.path, driveTrashConfigs]);
    const rightIsNukeOverride = useMemo(() => getIsNukeOverride(right), [right.path, driveTrashConfigs]);

    // Use Refs to ensure drag handlers always access latest state (files/selection)
    // and to properly handle switching between files vs searchResults
    const leftSelectedRef = useRef(left.selected);
    leftSelectedRef.current = left.selected;
    const leftFilesRef = useRef(left.files);
    leftFilesRef.current = left.files;
    const leftResultsRef = useRef(left.searchResults);
    leftResultsRef.current = left.searchResults;

    const rightSelectedRef = useRef(right.selected);
    rightSelectedRef.current = right.selected;
    const rightFilesRef = useRef(right.files);
    rightFilesRef.current = right.files;
    const rightResultsRef = useRef(right.searchResults);
    rightResultsRef.current = right.searchResults;

    // Stable handler sets for panels to prevent VirtualizedFileList re-renders
    const leftHandlers = useMemo(() => ({
        onNavigate: (p: string) => navigate('left', p),
        onOpenFile: (p: string) => handleOpenFile(p, 'left'),
        onSelect: (p: string, v: boolean, r: boolean) => handleSelect('left', p, v, r),
        onSelectMultiple: (ps: string[], a: boolean) => handleSelectMultiple('left', ps, a),
        onClearSelection: () => handleClearSelection('left'),
        onContextMenu: (e: React.MouseEvent, entry: any) => handleContextMenu(e, 'left', entry),
        onActivate: () => setActivePanelId('left'),
        onSort: (field: any) => handleSort('left', field),
        onResize: (field: any, delta: number) => handleResize('left', field, delta),
        onResizeMultiple: (updates: any) => handleResizeMultiple('left', updates),
        onSearch: () => executeSearch('left'),
        onQueryChange: (q: string) => handleSearch('left', q),
        onClearSearch: () => clearSearch('left'),
        onAdvancedSearch: () => openAdvancedSearch('left'),
        onFileDrop: (target: string | undefined, e: any) => handleDrop(e, target || null, left.path),
        setViewMode: (mode: ViewMode) => left.setViewMode(mode),
        onGroupByDateChange: (val: boolean) => left.setGroupByDate(val),
    }), [navigate, handleOpenFile, handleSelect, handleSelectMultiple, handleClearSelection, handleContextMenu, setActivePanelId, handleSort, handleResize, handleResizeMultiple, executeSearch, handleSearch, clearSearch, handleDrop, left]);

    const rightHandlers = useMemo(() => ({
        onNavigate: (p: string) => navigate('right', p),
        onOpenFile: (p: string) => handleOpenFile(p, 'right'),
        onSelect: (p: string, v: boolean, r: boolean) => handleSelect('right', p, v, r),
        onSelectMultiple: (ps: string[], a: boolean) => handleSelectMultiple('right', ps, a),
        onClearSelection: () => handleClearSelection('right'),
        onContextMenu: (e: React.MouseEvent, entry: any) => handleContextMenu(e, 'right', entry),
        onActivate: () => setActivePanelId('right'),
        onSort: (field: any) => handleSort('right', field),
        onResize: (field: any, delta: number) => handleResize('right', field, delta),
        onResizeMultiple: (updates: any) => handleResizeMultiple('right', updates),
        onSearch: () => executeSearch('right'),
        onQueryChange: (q: string) => handleSearch('right', q),
        onClearSearch: () => clearSearch('right'),
        onAdvancedSearch: () => openAdvancedSearch('right'),
        onFileDrop: (target: string | undefined, e: any) => handleDrop(e, target || null, right.path),
        setViewMode: (mode: ViewMode) => right.setViewMode(mode),
        onGroupByDateChange: (val: boolean) => right.setGroupByDate(val),
    }), [navigate, handleOpenFile, handleSelect, handleSelectMultiple, handleClearSelection, handleContextMenu, setActivePanelId, handleSort, handleResize, handleResizeMultiple, executeSearch, handleSearch, clearSearch, handleDrop, right]);

    const makeDragStartHandler = useCallback((panelId: 'left' | 'right') => {
        const selectedRef = panelId === 'left' ? leftSelectedRef : rightSelectedRef;
        const filesRef = panelId === 'left' ? leftFilesRef : rightFilesRef;
        const resultsRef = panelId === 'left' ? leftResultsRef : rightResultsRef;

        return (entry: any) => {
            const currentSelected = selectedRef.current;
            const sourceFiles = resultsRef.current || filesRef.current;
            const isSelected = currentSelected.has(entry.path);
            if (isSelected) {
                const selectedFiles = sourceFiles.filter((f: any) => {
                    if (currentSelected.has(f.path)) return true;
                    const lowerF = f.path.toLowerCase();
                    for (const s of currentSelected) {
                        if (s.toLowerCase() === lowerF) return true;
                    }
                    return false;
                });
                handleDragStart(panelId, selectedFiles.length > 0 ? selectedFiles : [entry]);
            } else {
                handleSelect(panelId, entry.path, false, false);
                handleDragStart(panelId, [entry]);
            }
        };
    }, [handleDragStart, handleSelect]);

    const handleLeftDragStart = useMemo(() => makeDragStartHandler('left'), [makeDragStartHandler]);
    const handleRightDragStart = useMemo(() => makeDragStartHandler('right'), [makeDragStartHandler]);

    const getFurthestDescendant = useCallback((panel: FullPanelState) => {
        if (!panel.history || panel.historyIndex === undefined || panel.historyIndex >= panel.history.length - 1) return null;
        let furthest = null;
        for (let i = panel.historyIndex + 1; i < panel.history.length; i++) {
            const hPath = panel.history[i].path;
            if (hPath.toLowerCase().startsWith(panel.path.toLowerCase() + '\\') || hPath.toLowerCase() === panel.path.toLowerCase()) {
                furthest = hPath;
            } else {
                break;
            }
        }
        return furthest;
    }, []);

    const leftForwardPath = useMemo(() => getFurthestDescendant(left), [left, getFurthestDescendant]);
    const rightForwardPath = useMemo(() => getFurthestDescendant(right), [right, getFurthestDescendant]);

    const renderPanel = (id: 'left' | 'right') => {
        const isLeft = id === 'left';
        const panel = isLeft ? left : right;
        const handlers = isLeft ? leftHandlers : rightHandlers;
        const isActive = activePanelId === id;
        const isNuke = isLeft ? leftIsNukeOverride : rightIsNukeOverride;
        const forwardPath = isLeft ? leftForwardPath : rightForwardPath;
        const dragStart = isLeft ? handleLeftDragStart : handleRightDragStart;

        if (id === 'right' && layout === 'standard') return null;

        return (
            <div key={id} className={cx("individual-panel-wrapper", { active: isActive })}>
                {onTabSwitch && (
                    <Tabs
                        panelId={id}
                        onSwitch={(tabId, path) => onTabSwitch(tabId, path, id)}
                        onClose={(tabId) => onTabClose!(tabId, id)}
                        isDraggingFiles={!!dragState}
                        dragState={dragState}
                        onTabDrop={onTabDrop}
                    />
                )}
                <TopBar
                    activePanel={panel}
                    activePanelId={id}
                    canUndo={canUndo}
                    undoLabel={undoLabel}
                    canRedo={canRedo}
                    redoLabel={redoLabel}
                    onNavigate={handlers.onNavigate}
                    onRefresh={onRefresh}
                    onNavigateUp={() => panel.goUp()}
                    onNavigateBack={() => panel.goBack()}
                    onNavigateForward={() => panel.goForward()}
                    onNavigateToIndex={(i) => panel.goToIndex(i)}
                    onUndo={handleUndo}
                    onRedo={handleRedo}
                    onCopy={handleCopy}
                    onCopyName={handleCopyName}
                    onCopyPath={handleCopyPath}
                    onCut={handleCut}
                    onDelete={handleDelete}
                    onPaste={handlePaste}
                    canPaste={canPaste}
                    t={t}
                    layout="standard" // Force standard TopBar layout inside per-panel container
                    showHidden={showHidden}
                    isDragging={!!dragState}
                    onDrop={(p, e) => p && handleDrop(e, p, panel.path)}
                    drives={drives}
                    isTrashView={panel.path?.startsWith('trash://')}
                    onRestoreAll={onRestoreAll}
                    onRestoreSelected={onRestoreSelected}
                    onEmptyTrash={onEmptyTrash}
                    isTrashEmpty={isTrashEmpty}
                    isNukeOverride={isNuke}
                    onSearchChange={(q) => handleSearch(id, q)}
                    onSearchSubmit={() => executeSearch(id)}
                    onSearchClear={() => clearSearch(id)}
                    onSearchCancel={() => handleCancelSearch(id)}
                    isShiftPressed={isShiftPressed}
                    favorites={favorites}
                    searchQuery={panel.searchQuery}
                    isSearchActive={panel.isSearching}
                />
                <FilePanel
                    files={panel.files}
                    viewMode={panel.viewMode}
                    selected={panel.selected}
                    isActive={isActive}
                    currentPath={panel.path}
                    drives={drives}
                    showDrives={false}
                    sortConfig={panel.sortConfig}
                    colWidths={panel.colWidths}
                    onNavigate={handlers.onNavigate}
                    onOpenFile={handlers.onOpenFile}
                    onSelect={handlers.onSelect}
                    onSelectMultiple={handlers.onSelectMultiple}
                    onClearSelection={handlers.onClearSelection}
                    onContextMenu={handlers.onContextMenu}
                    onActivate={handlers.onActivate}
                    onFileDragStart={dragStart}
                    onFileDrop={handlers.onFileDrop}
                    isDragging={!!dragState}
                    onSort={handlers.onSort}
                    onResize={handlers.onResize}
                    onResizeMultiple={handlers.onResizeMultiple}
                    t={t}
                    searchQuery={panel.searchQuery || ''}
                    searchResults={panel.searchResults}
                    isSearching={panel.isSearching}
                    onClearSearch={handlers.onClearSearch}
                    onCancelSearch={() => handleCancelSearch(id)}
                    isDragTarget={!!dragState && dragState.sourcePanel !== id}
                    dragOverPath={dragOverPath}
                    showHidden={propShowHidden}
                    showSystem={propShowSystem}
                    layout={layout}
                    cutPaths={cutPaths}
                    onRename={handleInlineRename}
                    isTrashView={panel.isTrashView}
                    isNetworkView={panel.isNetworkView}
                    useSystemIcons={useSystemIcons}
                    searchLimitReached={panel.searchLimitReached}
                    panelId={id}
                    onViewModeChange={handlers.setViewMode}
                    loading={panel.loading}
                    initialScrollOffset={panel.currentEntry?.scrollOffset}
                    updateCurrentScroll={(o) => panel.updateCurrentScroll!(o)}
                    showHistogram={histogramPanels.has(id)}
                    groupByDate={panel.groupByDate}
                    onGroupByDateChange={handlers.onGroupByDateChange}
                    isProtected={panel.isProtected}
                    favorites={favorites}
                    extensionFilter={panel.extensionFilter}
                    setExtensionFilter={panel.setExtensionFilter}
                    sizeFilter={panel.sizeFilter}
                    setSizeFilter={panel.setSizeFilter}
                    dateFilter={panel.dateFilter}
                    setDateFilter={panel.setDateFilter}
                    nameFilter={panel.nameFilter}
                    setNameFilter={panel.setNameFilter}
                    locationFilter={panel.locationFilter}
                    setLocationFilter={panel.setLocationFilter}
                    deletedDateFilter={panel.deletedDateFilter}
                    setDeletedDateFilter={panel.setDeletedDateFilter}
                    clearAllFilters={panel.clearAllFilters}
                    forwardPath={forwardPath}
                    onItemMiddleClick={onItemMiddleClick ? (entry) => onItemMiddleClick(entry, id) : undefined}
                />
            </div>
        );
    };

    return (
        <div className="app">
            <TitleBar
                t={t}
                layout={layout}
                onLayoutChange={onLayoutChange}
                onAdvancedSearch={() => openAdvancedSearch(activePanelId)}
                onDuplicateSearch={onDuplicateSearch || (() => { })}
                onShowAbout={() => setShowAbout(true)}
                onCalculateAllSizes={onCalculateAllSizes}
                onRefresh={onRefresh}
            />
            <div className="main-area">
                <Sidebar
                    minimized={sidebarReduced}
                    onToggle={() => setSidebarReduced(!sidebarReduced)}
                    drives={drives}
                    currentPath={activePanel.path}
                    onNavigate={(path) => navigate(activePanelId, path)}
                    t={t}
                    treeRef={treeRef}
                    onTreeCut={onTreeCut}
                    onTreeCopy={onTreeCopy}
                    onTreeCopyName={onTreeCopyName}
                    onTreeCopyPath={onTreeCopyPath}
                    onTreeDelete={onTreeDelete}
                    isShiftPressed={isShiftPressed}
                    onTreeRename={onTreeRename}
                    onTreeNewFolder={onTreeNewFolder}
                    onTreeUnmount={onTreeUnmount}
                    onTreeDisconnectDrive={onTreeDisconnectDrive}
                    onTreeProperties={onTreeProperties}
                    onTreePaste={onTreePaste}
                    canPaste={true}
                    canUndo={canUndo}
                    undoLabel={undoLabel}
                    canRedo={canRedo}
                    redoLabel={redoLabel}
                    onUndo={handleUndo}
                    onRedo={handleRedo}
                    onDragStart={handleDragStart}
                    onDrop={(e, target) => handleDrop(e, target, activePanel.path)}
                    dragState={dragState}
                    useSystemIcons={useSystemIcons}
                    onItemMiddleClick={onItemMiddleClick}
                    onOpenNewTab={onOpenNewTab}
                    onDriveContextMenu={onDriveContextMenu}
                    onAddToFavorites={onAddToFavorites}
                    onRemoveFromFavorites={onRemoveFromFavorites}
                    onTreeEmptyTrash={onEmptyTrash}
                    onTreeRestoreAll={onRestoreAll}
                    onTrashProperties={onTrashProperties}
                    onDragOver={setDragOverPath}
                />

                <div className="panel-container">
                    <div
                        className={cx("panels-container", { "dual-view": layout === 'dual' })}
                    >
                        {renderPanel('left')}
                        {layout === 'dual' && renderPanel('right')}
                    </div>
                </div>
            </div>
        </div>
    );
};

