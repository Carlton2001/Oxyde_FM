import React, { useCallback } from 'react';
import { Sidebar } from '../layout/Sidebar';
import { FilePanel } from '../file-list/FilePanel';
import { TopBar } from '../layout/TopBar';
import { TitleBar } from '../layout/TitleBar';
import cx from 'classnames';
import { PanelState, DriveInfo, FileEntry, SortField, ColumnWidths, ViewMode, SizeCategoryKey, PanelId } from '../../types';
import { Tabs } from '../ui/Tabs';
import { TFunc } from '../../i18n';
import { useApp } from '../../context/AppContext';
import { useTabs } from '../../context/TabsContext';
import { invoke } from '@tauri-apps/api/core';

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

interface MultipaneLayoutProps {
    t: TFunc;
    sidebarReduced: boolean;
    setSidebarReduced: (val: boolean) => void;
    drives: DriveInfo[];
    panels: Record<PanelId, FullPanelState>;
    activePanelId: PanelId;
    setActivePanelId: (id: PanelId) => void;
    navigate: (id: PanelId, path: string) => void;
    handleSearch: (id: PanelId, query: string) => void;
    executeSearch: (id: PanelId) => void;
    openAdvancedSearch: (id: PanelId) => void;
    clearSearch: (id: PanelId) => void;
    handleCancelSearch: (id: PanelId) => void;
    handleDragStart: (id: PanelId, files: FileEntry[]) => void;
    handleDrop: (e: React.DragEvent | React.MouseEvent | undefined, targetPath: string | null, currentPath: string) => void;
    dragState: { sourcePanel: PanelId; files: FileEntry[] } | null;
    handleSelect: (id: PanelId, path: string, selected: boolean, range: boolean) => void;
    handleSelectMultiple: (id: PanelId, paths: string[], isAdditive: boolean) => void;
    handleClearSelection: (id: PanelId) => void;
    handleContextMenu: (e: React.MouseEvent, id: PanelId, entry?: FileEntry) => void;
    handleOpenFile: (path: string, id: PanelId) => void;
    handleSort: (id: PanelId, field: SortField) => void;
    handleResize: (id: PanelId, field: keyof ColumnWidths, delta: number) => void;
    handleResizeMultiple: (id: PanelId, updates: Partial<ColumnWidths>) => void;
    handleInlineRename: (oldPath: string, newPath: string) => void;
    propShowHidden: boolean;
    propShowSystem: boolean;
    cutPaths: string[];
    useSystemIcons: boolean;
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
    setShowAbout: (show: boolean) => void;
    showHidden: boolean;
    onRefresh: () => void;
    onRestoreAll?: () => void;
    onRestoreSelected?: () => void;
    onEmptyTrash?: () => void;
    handleCopy: () => void;
    handleCopyName: () => void;
    handleCopyPath: () => void;
    handleCut: () => void;
    handlePaste: () => void;
    handleDelete: () => void;
    handleUndo: () => void;
    handleRedo: () => void;
    canPaste: boolean;
    canUndo: boolean;
    undoLabel?: string;
    canRedo: boolean;
    redoLabel?: string;
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

interface MultipaneDropZonesProps {
    panelId: PanelId;
    draggedTab: { id: string, panelId: PanelId } | null;
}

const MultipaneDropZones: React.FC<MultipaneDropZonesProps> = ({ panelId, draggedTab }) => {
    const [activeZone, setActiveZone] = React.useState<'top' | 'bottom' | 'left' | 'right' | null>(null);
    const { setActiveDropZone } = useTabs();

    if (!draggedTab) return null;

    const handleMouseEnter = (side: 'top' | 'bottom' | 'left' | 'right') => {
        setActiveZone(side);
        setActiveDropZone({ panelId, side });
    };

    const handleMouseLeave = () => {
        setActiveZone(null);
        setActiveDropZone(null);
    };

    return (
        <div className="multipane-drop-zones" data-panel-id={panelId}>
            {(['top', 'bottom', 'left', 'right'] as const).map(side => (
                <div
                    key={side}
                    className={cx("drop-zone", `drop-zone-${side}`, { active: activeZone === side })}
                    onMouseEnter={() => handleMouseEnter(side)}
                    onMouseLeave={handleMouseLeave}
                />
            ))}
        </div>
    );
};

export const MultipaneLayout: React.FC<MultipaneLayoutProps> = ({
    t, sidebarReduced, setSidebarReduced, drives, panels,
    activePanelId, setActivePanelId, navigate,
    handleSearch, executeSearch, openAdvancedSearch, clearSearch, handleCancelSearch,
    handleDragStart, handleDrop, dragState, handleSelect, handleSelectMultiple, handleClearSelection,
    handleContextMenu, handleOpenFile, handleSort, handleResize, handleResizeMultiple, handleInlineRename,
    propShowHidden, propShowSystem, cutPaths, useSystemIcons, treeRef,
    onTreeCut, onTreeCopy, onTreeCopyName, onTreeCopyPath, onTreeDelete, isShiftPressed,
    onTreeRename, onTreeNewFolder, onTreeUnmount, onTreeDisconnectDrive, onTreeProperties, onTreePaste,
    onCalculateAllSizes, histogramPanels, setShowAbout, onRefresh,
    onRestoreAll, onRestoreSelected, onEmptyTrash,
    handleCopy, handleCopyName, handleCopyPath, handleCut, handlePaste, handleDelete, handleUndo, handleRedo,
    canPaste, canUndo, undoLabel, canRedo, redoLabel,
    onTabSwitch, onTabClose, onItemMiddleClick, onOpenNewTab, onDuplicateSearch, onTrashProperties,
    dragOverPath, setDragOverPath, onTabDrop, onAddToFavorites, onRemoveFromFavorites, onDriveContextMenu,
    isTrashEmpty, favorites
}) => {
    const { driveTrashConfigs } = useApp();
    const { draggedTab } = useTabs();
    const activePanel = panels[activePanelId] || Object.values(panels)[0];

    if (!activePanel) return null;

    const getIsNukeOverride = (p: FullPanelState) => {
        if (!p.path) return false;
        const targetDriveMatch = p.path.match(/^([a-zA-Z]:)/);
        if (!targetDriveMatch) return false;
        const targetDrive = targetDriveMatch[1].toLowerCase();
        return driveTrashConfigs[targetDrive]?.nukeOnDelete || false;
    };

    const renderPanel = (id: PanelId) => {
        const panel = panels[id];
        const isActive = activePanelId === id;
        
        if (!panel) {
            // Panel might be missing during rapid updates/tree transitions
            return <div key={id} className="individual-panel-wrapper placeholder" />;
        }

        const isNuke = getIsNukeOverride(panel);

        return (
            <div key={id} className={cx("individual-panel-wrapper", { active: isActive })}>
                <MultipaneDropZones
                    panelId={id}
                    draggedTab={draggedTab}
                />
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
                    canUndo={canUndo}
                    undoLabel={undoLabel}
                    canRedo={canRedo}
                    redoLabel={redoLabel}
                    onNavigate={(p) => navigate(id, p)}
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
                    searchQuery={panel.searchQuery}
                    isSearchActive={panel.isSearching}
                    onClosePanel={Object.keys(panels).length > 1 ? () => invoke('remove_panel', { panelId: id }).catch(console.error) : undefined}
                />
                <FilePanel
                    panelId={id}
                    files={panel.files}
                    viewMode={panel.viewMode}
                    selected={panel.selected}
                    isActive={isActive}
                    currentPath={panel.path}
                    drives={drives}
                    showDrives={false}
                    sortConfig={panel.sortConfig}
                    colWidths={panel.colWidths}
                    onNavigate={(p) => navigate(id, p)}
                    onOpenFile={(p) => handleOpenFile(p, id)}
                    onSelect={(p, v, r) => handleSelect(id, p, v, r)}
                    onSelectMultiple={(ps, a) => handleSelectMultiple(id, ps, a)}
                    onClearSelection={() => handleClearSelection(id)}
                    onContextMenu={(e, entry) => handleContextMenu(e, id, entry)}
                    onActivate={() => setActivePanelId(id)}
                    onFileDragStart={(entry) => {
                        const isSelected = panel.selected.has(entry.path);
                        if (isSelected) {
                            const selectedFiles = (panel.searchResults || panel.files).filter(f => panel.selected.has(f.path));
                            handleDragStart(id, selectedFiles);
                        } else {
                            handleSelect(id, entry.path, false, false);
                            handleDragStart(id, [entry]);
                        }
                    }}
                    onFileDrop={(target, e) => handleDrop(e, target || null, panel.path)}
                    isDragging={!!dragState}
                    onSort={(field) => handleSort(id, field)}
                    onResize={(field, delta) => handleResize(id, field, delta)}
                    onResizeMultiple={(updates) => handleResizeMultiple(id, updates)}
                    t={t}
                    searchQuery={panel.searchQuery || ''}
                    searchResults={panel.searchResults}
                    isSearching={panel.isSearching}
                    onClearSearch={() => clearSearch(id)}
                    onCancelSearch={() => handleCancelSearch(id)}
                    isDragTarget={!!dragState && dragState.sourcePanel !== id}
                    dragOverPath={dragOverPath}
                    showHidden={propShowHidden}
                    showSystem={propShowSystem}
                    cutPaths={cutPaths}
                    onRename={handleInlineRename}
                    isTrashView={panel.isTrashView}
                    isNetworkView={panel.isNetworkView}
                    useSystemIcons={useSystemIcons}
                    searchLimitReached={panel.searchLimitReached}
                    onViewModeChange={(m) => panel.setViewMode(m)}
                    loading={panel.loading}
                    initialScrollOffset={panel.currentEntry?.scrollOffset}
                    updateCurrentScroll={(o) => panel.updateCurrentScroll!(o)}
                    showHistogram={histogramPanels.has(id)}
                    groupByDate={panel.groupByDate}
                    onGroupByDateChange={(v) => panel.setGroupByDate(v)}
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
                    onItemMiddleClick={onItemMiddleClick ? (entry) => onItemMiddleClick(entry, id) : undefined}
                />
            </div>
        );
    };

    const { session } = useTabs();

    const handleResizeSplit = useCallback((splitId: string, _axis: 'horizontal' | 'vertical', index: number, delta: number, totalSize: number) => {
        if (!session) return;

        const findAndResize = (node: import('../../hooks/useRustSession').LayoutNode): boolean => {
            if (node.type === 'Split') {
                if (node.data.id === splitId) {
                    const newWeights = [...node.data.weights];
                    const weightDelta = (delta / totalSize) * newWeights.reduce((a, b) => a + b, 0);
                    
                    // Adjust weights of neighbor children
                    if (newWeights[index] !== undefined && newWeights[index + 1] !== undefined) {
                        const minWeight = 0.1;
                        const oldA = newWeights[index];
                        const oldB = newWeights[index + 1];
                        
                        newWeights[index] = Math.max(minWeight, oldA + weightDelta);
                        newWeights[index + 1] = Math.max(minWeight, oldB - (newWeights[index] - oldA));
                        
                        invoke('update_layout_weights', { splitId, weights: newWeights }).catch(console.error);
                        return true;
                    }
                }
                return node.data.children.some(findAndResize);
            }
            return false;
        };

        findAndResize(session.root);
    }, [session]);

    const renderNode = (node: import('../../hooks/useRustSession').LayoutNode): React.ReactNode => {
        if (node.type === 'Pane') {
            return renderPanel(node.data.id as PanelId);
        }

        const { axis, children, weights, id } = node.data;
        const totalWeight = weights.reduce((a, b) => a + b, 0);

        return (
            <div 
                key={id}
                className={cx("layout-split", axis)}
                style={{
                    display: 'flex',
                    flexDirection: axis === 'horizontal' ? 'row' : 'column',
                    flex: 1,
                    width: '100%',
                    height: '100%',
                    minWidth: 0,
                    minHeight: 0,
                    position: 'relative'
                }}
            >
                {children.map((child, i) => {
                    const weight = weights[i] || 1;
                    const isLast = i === children.length - 1;
                    
                    return (
                        <React.Fragment key={i}>
                            <div 
                                className="layout-child"
                                style={{ 
                                    flex: `${(weight / totalWeight) * 100}%`,
                                    display: 'flex',
                                    minWidth: 0,
                                    minHeight: 0
                                }}
                            >
                                {renderNode(child)}
                            </div>
                            {!isLast && (
                                <TreeResizeHandle
                                    axis={axis}
                                    onResize={(delta, totalSize) => handleResizeSplit(id, axis, i, delta, totalSize)}
                                />
                            )}
                        </React.Fragment>
                    );
                })}
            </div>
        );
    };

    return (
        <div className="app">
            <TitleBar
                t={t}
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
                    activePanelId={activePanelId}
                    drives={drives}
                    currentPath={activePanel.path}
                    onNavigate={navigate}
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
                    <div className="panels-container multipane-view">
                        {session && renderNode(session.root)}
                    </div>
                </div>
            </div>
        </div>
    );
};

const TreeResizeHandle: React.FC<{ axis: 'horizontal' | 'vertical', onResize: (delta: number, totalSize: number) => void }> = ({ axis, onResize }) => {
    const [isDragging, setIsDragging] = React.useState(false);
    const handleRef = React.useRef<HTMLDivElement>(null);

    const onMouseDown = (e: React.MouseEvent) => {
        e.preventDefault();
        setIsDragging(true);
    };

    React.useEffect(() => {
        if (!isDragging) return;

        const onMouseMove = (e: MouseEvent) => {
            if (!handleRef.current?.parentElement) return;
            const parentRect = handleRef.current.parentElement.getBoundingClientRect();
            const delta = axis === 'horizontal' ? e.movementX : e.movementY;
            const totalSize = axis === 'horizontal' ? parentRect.width : parentRect.height;
            onResize(delta, totalSize);
        };

        const onMouseUp = () => setIsDragging(false);

        window.addEventListener('mousemove', onMouseMove);
        window.addEventListener('mouseup', onMouseUp);
        return () => {
            window.removeEventListener('mousemove', onMouseMove);
            window.removeEventListener('mouseup', onMouseUp);
        };
    }, [isDragging, axis, onResize]);

    return (
        <div
            ref={handleRef}
            className={cx("tree-resize-handle", axis, { dragging: isDragging })}
            onMouseDown={onMouseDown}
            style={{
                width: axis === 'horizontal' ? '4px' : '100%',
                height: axis === 'vertical' ? '4px' : '100%',
                cursor: axis === 'horizontal' ? 'col-resize' : 'row-resize',
                backgroundColor: isDragging ? 'var(--accent-color)' : 'transparent',
                zIndex: 10,
                flexShrink: 0,
                position: 'relative',
                margin: axis === 'horizontal' ? '0 -2px' : '-2px 0',
                transition: 'background-color 0.2s'
            }}
        />
    );
};
