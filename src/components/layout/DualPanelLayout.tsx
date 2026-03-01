import { useRef, useMemo, useCallback } from 'react';
import { Sidebar } from '../layout/Sidebar';
import { FilePanel } from '../file-list/FilePanel';
import { TopBar } from '../layout/TopBar';
import cx from 'classnames';
import { PanelState, DriveInfo, FileEntry, SortField, ColumnWidths, ViewMode } from '../../types';
import { Tabs } from '../ui/Tabs';
import { TFunc } from '../../i18n';

interface FullPanelState extends PanelState {
    goUp: () => void;
    goBack: () => void;
    goForward: () => void;
    setViewMode: (mode: ViewMode) => void;
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
    // Handlers
    handleDragStart: (id: 'left' | 'right', files: FileEntry[]) => void;
    handleDrop: (e: React.DragEvent | React.MouseEvent | undefined, targetPath: string | null, currentPath: string) => void;
    onTabDrop?: (files: FileEntry[], index?: number) => void;
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
    histogramPanels: Set<'left' | 'right'>;
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
    // TopBar (only what TopBar still needs)
    setShowAbout: (show: boolean) => void;
    onLayoutChange: (mode: 'standard' | 'dual') => void;
    showHidden: boolean;
    // Actions
    onCalculateAllSizes: () => void;
    onRefresh: () => void;
    onRestoreAll?: () => void;
    onRestoreSelected?: () => void;
    onEmptyTrash?: () => void;
    // Clipboard / Edit Actions
    handleCopy: () => void;
    handleCut: () => void;
    handlePaste: () => void;
    handleDelete: () => void;
    handleUndo: () => void;
    handleRedo: () => void;
    canPaste: boolean;
    // Tabs
    onTabSwitch?: (id: string, path?: string) => void;
    onTabClose?: (id: string) => void;
    onItemMiddleClick?: (entry: FileEntry) => void;
    onOpenNewTab?: (path: string) => void;
    // Dual Panel Management
    onSwapPanels?: () => void;
    onSyncPanels?: () => void;
    isSyncDisabled?: boolean;
    onComparePanels?: () => void;
    isComparing?: boolean;
    diffPaths?: Set<string>;
    onDriveContextMenu?: (e: React.MouseEvent, path: string) => void;
    onAddToFavorites?: (path: string) => void;
    onRemoveFromFavorites?: (path: string) => void;
    onDuplicateSearch?: () => void;
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
    histogramPanels,
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
    setShowAbout,
    onLayoutChange,
    showHidden,
    onCalculateAllSizes,
    onRefresh,
    onRestoreAll,
    onRestoreSelected,
    onEmptyTrash,
    handleCopy,
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
    onSwapPanels,
    onSyncPanels,
    isSyncDisabled,
    onComparePanels,
    isComparing,
    diffPaths,
    onTabDrop,
    onDriveContextMenu,
    onAddToFavorites,
    onRemoveFromFavorites,
    onDuplicateSearch
}) => {
    const activePanel = activePanelId === 'left' ? left : right;

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
                handleDragStart(panelId, [entry]);
            }
        };
    }, [handleDragStart]);

    const handleLeftDragStart = useMemo(() => makeDragStartHandler('left'), [makeDragStartHandler]);
    const handleRightDragStart = useMemo(() => makeDragStartHandler('right'), [makeDragStartHandler]);

    return (
        <div className="app">
            <TopBar
                activePanel={activePanel}
                activePanelId={activePanelId}
                canUndo={canUndo}
                undoLabel={undoLabel}
                canRedo={canRedo}
                redoLabel={redoLabel}
                onNavigate={activePanelId === 'left' ? leftHandlers.onNavigate : rightHandlers.onNavigate}
                onRefresh={onRefresh}
                onNavigateUp={() => activePanel.goUp()}
                onNavigateBack={() => activePanel.goBack()}
                onNavigateForward={() => activePanel.goForward()}
                onUndo={handleUndo}
                onRedo={handleRedo}
                onCopy={handleCopy}
                onCut={handleCut}
                onDelete={handleDelete}
                onPaste={handlePaste}
                canPaste={canPaste}
                t={t}
                layout={layout}
                onLayoutChange={onLayoutChange}
                showHidden={showHidden}
                onShowAbout={() => setShowAbout(true)}
                isDragging={!!dragState}
                isShiftPressed={isShiftPressed}
                onDrop={(p, e) => p && handleDrop(e, p, activePanel.path)}
                drives={drives}
                isTrashView={activePanel.path?.startsWith('trash://')}
                onCalculateAllSizes={onCalculateAllSizes}
                onRestoreAll={onRestoreAll}
                onRestoreSelected={onRestoreSelected}
                onEmptyTrash={onEmptyTrash}
                onSwapPanels={onSwapPanels}
                onSyncPanels={onSyncPanels}
                isSyncDisabled={isSyncDisabled}
                onComparePanels={onComparePanels}
                isComparing={isComparing}
                onAdvancedSearch={() => openAdvancedSearch(activePanelId)}
                onDuplicateSearch={onDuplicateSearch || (() => { })}
            />

            <div className="main-area">
                {layout === 'standard' && (
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
                    />
                )}

                <div
                    className="panel-container"
                    style={{ flexDirection: 'column' }}
                >
                    {layout === 'standard' && onTabSwitch && (
                        <Tabs
                            onSwitch={onTabSwitch}
                            onClose={onTabClose!}
                            isDraggingFiles={!!dragState}
                            dragState={dragState}
                            onTabDrop={onTabDrop}
                            searchQuery={activePanel.searchQuery}
                            onSearchChange={(q) => handleSearch(activePanelId, q)}
                            onSearchSubmit={() => executeSearch(activePanelId)}
                            onSearchClear={() => clearSearch(activePanelId)}
                            isSearching={activePanel.isSearching}
                        />
                    )}
                    <div
                        className={cx("panels-container", { "dual-view": layout === 'dual' })}
                        style={{ display: 'flex', flex: 1, overflow: 'hidden', width: '100%' }}
                    >
                        <FilePanel
                            files={left.files}
                            viewMode={left.viewMode}
                            selected={left.selected}
                            isActive={activePanelId === 'left'}
                            currentPath={left.path}
                            drives={drives}
                            showDrives={layout === 'dual'}
                            sortConfig={left.sortConfig}
                            colWidths={left.colWidths}
                            onNavigate={leftHandlers.onNavigate}
                            onOpenFile={leftHandlers.onOpenFile}
                            onSelect={leftHandlers.onSelect}
                            onSelectMultiple={leftHandlers.onSelectMultiple}
                            onClearSelection={leftHandlers.onClearSelection}
                            onContextMenu={leftHandlers.onContextMenu}
                            onActivate={leftHandlers.onActivate}
                            onFileDragStart={handleLeftDragStart}
                            onFileDrop={leftHandlers.onFileDrop}
                            isDragging={!!dragState}
                            onSort={leftHandlers.onSort}
                            onResize={leftHandlers.onResize}
                            onResizeMultiple={leftHandlers.onResizeMultiple}
                            t={t}
                            searchQuery={left.searchQuery || ''}
                            searchResults={left.searchResults}
                            isSearching={left.isSearching}
                            onSearch={leftHandlers.onSearch}
                            onQueryChange={leftHandlers.onQueryChange}
                            onClearSearch={leftHandlers.onClearSearch}
                            showSearch={layout === 'dual'}
                            isDragTarget={!!dragState && dragState.sourcePanel !== 'left'}
                            dragOverPath={null}
                            showHidden={propShowHidden}
                            showSystem={propShowSystem}
                            layout={layout}
                            cutPaths={cutPaths}
                            onRename={handleInlineRename}
                            showHistogram={histogramPanels.has('left')}
                            isTrashView={left.path?.startsWith('trash://')}
                            useSystemIcons={useSystemIcons}
                            onItemMiddleClick={onItemMiddleClick}
                            diffPaths={diffPaths}
                            searchLimitReached={left.searchLimitReached}
                            panelId="left"
                            onViewModeChange={leftHandlers.setViewMode}
                            loading={left.loading}
                        />

                        {layout === 'dual' && (
                            <FilePanel
                                files={right.files}
                                viewMode={right.viewMode}
                                selected={right.selected}
                                isActive={activePanelId === 'right'}
                                currentPath={right.path}
                                drives={drives}
                                showDrives={true}
                                sortConfig={right.sortConfig}
                                colWidths={right.colWidths}
                                onNavigate={rightHandlers.onNavigate}
                                onOpenFile={rightHandlers.onOpenFile}
                                onSelect={rightHandlers.onSelect}
                                onSelectMultiple={rightHandlers.onSelectMultiple}
                                onClearSelection={rightHandlers.onClearSelection}
                                onContextMenu={rightHandlers.onContextMenu}
                                onActivate={rightHandlers.onActivate}
                                onFileDragStart={handleRightDragStart}
                                onFileDrop={rightHandlers.onFileDrop}
                                isDragging={!!dragState}
                                onSort={rightHandlers.onSort}
                                onResize={rightHandlers.onResize}
                                onResizeMultiple={rightHandlers.onResizeMultiple}
                                t={t}
                                searchQuery={right.searchQuery || ''}
                                searchResults={right.searchResults}
                                isSearching={right.isSearching}
                                onSearch={rightHandlers.onSearch}
                                onQueryChange={rightHandlers.onQueryChange}
                                onClearSearch={rightHandlers.onClearSearch}
                                showSearch={true}
                                isDragTarget={!!dragState && dragState.sourcePanel !== 'right'}
                                dragOverPath={null}
                                showHidden={propShowHidden}
                                showSystem={propShowSystem}
                                layout={layout}
                                cutPaths={cutPaths}
                                onRename={handleInlineRename}
                                showHistogram={histogramPanels.has('right')}
                                isTrashView={right.path?.startsWith('trash://')}
                                useSystemIcons={useSystemIcons}
                                diffPaths={diffPaths}
                                searchLimitReached={right.searchLimitReached}
                                panelId="right"
                                onViewModeChange={rightHandlers.setViewMode}
                                loading={right.loading}
                            />
                        )}
                    </div>
                </div>
            </div>
        </div>
    );
};

