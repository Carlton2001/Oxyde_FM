import { useState, useEffect, useRef, useMemo, useCallback } from 'react';
import { listen } from '@tauri-apps/api/event';
import { invoke } from '@tauri-apps/api/core';
import { File as FileIcon } from 'lucide-react';
import cx from 'classnames';

import './styles/global.css';
import './styles/components/Utilities.css';
import './styles/components/Buttons.css';
import './styles/components/Inputs.css';
import './styles/components/Dialogs.css';
import './styles/themes/github-light.css';
import './styles/themes/github-dark.css';
import './styles/themes/ayu-light.css';
import './styles/themes/ayu-dark.css';
import './styles/themes/one-light.css';
import './styles/themes/one-dark.css';
import './styles/themes/monokai.css';
import './styles/themes/solarized-light.css';
import './styles/themes/solarized-dark.css';
import './styles/themes/windows-light.css';
import './styles/themes/windows-dark.css';
import './styles/themes/oxyde-light.css';
import './styles/themes/oxyde-dark.css';

import { useApp } from './context/AppContext';
import { useTabs } from './context/TabsContext';
import { useDialogs } from './context/DialogContext';
import { usePanelContext } from './context/PanelContext';
import { useFileOperations } from './hooks/useFileOperations';
import { useClipboard } from './hooks/useClipboard';
import { useAppHandlers } from './hooks/useAppHandlers';
import { useGlobalShortcuts } from './hooks/useGlobalShortcuts';
import { useDragDrop } from './hooks/useDragDrop';
import { useNativeDragDrop } from './hooks/useNativeDragDrop';
import { useFileDrop } from './hooks/useFileDrop';
import { usePanelComparison } from './hooks/usePanelComparison';
import { useFavorites } from './hooks/useFavorites';

import { ActionContext } from './types/actions';
import { actionService } from './services/ActionService';
import { normalizePath } from './utils/path';

import { DualPanelLayout } from './components/layout/DualPanelLayout';
import { ContextMenu } from './components/ui/ContextMenu';
import { GlobalDialogContainer } from './components/managers/GlobalDialogContainer';
import { NotificationArea } from './components/ui/NotificationArea';
import { ProgressOverlay } from './components/ui/ProgressOverlay';
import { Tooltip } from './components/ui/Tooltip';
import { DirectoryTreeHandle } from './components/ui/DirectoryTree';
import { PanelId, DriveInfo } from './types';

function App() {
  const {
    layout, setLayout,
    showHidden, showSystem, t,
    notifications, notify, dismissNotification, drives, mountedImages,
    useSystemIcons, refreshDrives,
    zipQuality, sevenZipQuality, zstdQuality, defaultTurboMode,
  } = useApp();

  const clipboardObj = useClipboard();
  const { clipboard, copy, cut, clearClipboard, copyToSystem, refreshClipboard } = clipboardObj;
  const { tabs, activeTabId, setActiveTab, updateTabPath, addTab, closeTab } = useTabs();
  const dialogs = useDialogs();
  const { left, right, activePanelId, setActivePanelId } = usePanelContext();
  const fileOps = useFileOperations(notify, t as any);
  const { favorites } = useFavorites();

  const [contextMenu, setContextMenu] = useState<{ x: number, y: number, target?: string, panelId: PanelId, isDir?: boolean, isBackground?: boolean, isDrive?: boolean, driveType?: DriveInfo['drive_type'], isFavorite?: boolean } | null>(null);
  const [sidebarReduced, setSidebarReduced] = useState(() => localStorage.getItem('sidebarReduced') === 'true');
  const treeRef = useRef<DirectoryTreeHandle>(null);
  const dragGhostRef = useRef<HTMLDivElement>(null);
  const [modifiers, setModifiers] = useState({ ctrl: false, shift: false, alt: false });
  const [showProgress, setShowProgress] = useState(false);
  const activeOpRef = useRef(fileOps.activeOperation);
  activeOpRef.current = fileOps.activeOperation;

  // Effect 1: Decide when to SHOW the progress overlay
  useEffect(() => {
    if (fileOps.activeOperation) {
      const op = fileOps.activeOperation;

      if (op.status === 'Completed' || op.status === 'Cancelled') {
        setShowProgress(false);
        return;
      }

      const isCrossVolume = op.is_cross_volume;
      const isLikelyLarge = op.likely_large;
      const totalMB = op.total_bytes / (1024 * 1024);

      // Show immediately for guaranteed-heavy tasks
      if (isLikelyLarge || totalMB > 150 || (isCrossVolume && totalMB > 10)) {
        setShowProgress(true);
      } else {
        // Delayed show: wait before displaying to avoid flashes for near-instant operations.
        // In Discret mode, OS deprioritizes I/O, so even small ops can take 1-2 seconds.
        const delay = (op.op_type === 'Move' && !isCrossVolume) ? 3000 : 2000;
        const timer = setTimeout(() => {
          const current = activeOpRef.current;
          if (current && current.status !== 'Completed' && current.status !== 'Cancelled'
            && !(current.total_bytes > 0 && current.processed_bytes >= current.total_bytes)) {
            setShowProgress(true);
          }
        }, delay);
        return () => clearTimeout(timer);
      }
    } else {
      setShowProgress(false);
    }
  }, [fileOps.activeOperation?.id, fileOps.activeOperation?.status]);

  // Effect 2: Immediately HIDE when the operation reaches a final state
  useEffect(() => {
    if (!fileOps.activeOperation) {
      setShowProgress(false);
    }
  }, [fileOps.activeOperation]);

  useEffect(() => {
    const handleKey = (e: KeyboardEvent) => {
      setModifiers(prev => {
        if (prev.ctrl === e.ctrlKey && prev.shift === e.shiftKey && prev.alt === e.altKey) return prev;
        return {
          shift: e.shiftKey,
          ctrl: e.ctrlKey,
          alt: e.altKey
        };
      });
    };
    const handleBlur = () => setModifiers({ shift: false, ctrl: false, alt: false });
    window.addEventListener('keydown', handleKey, true);
    window.addEventListener('keyup', handleKey, true);
    window.addEventListener('blur', handleBlur);
    return () => {
      window.removeEventListener('keydown', handleKey, true);
      window.removeEventListener('keyup', handleKey, true);
      window.removeEventListener('blur', handleBlur);
    };
  }, []);

  const [progress, setProgress] = useState<{ visible: boolean; message: string; cancellable?: boolean; cancelling?: boolean; task?: string; current?: number; total?: number; filename?: string; } | null>(null);

  // Handlers Integration
  const handlers = useAppHandlers({
    left, right, activePanelId, setActivePanelId, layout, fileOps, treeRef, notify, t, dialogs, clipboard: clipboardObj, refreshDrives,
    tabs, activeTabId, setActiveTab, closeTab, addTab, setContextMenu, contextMenu, drives, defaultTurboMode,
    zipQuality, sevenZipQuality, zstdQuality, favorites
  });

  const {
    refreshBothPanels, initiateFileOp, handleUndo, handleRedo, handleNavigate, handleSearch, executeSearch, clearSearch,
    handleOpenFile, handleAction, handleSort, handleSortDirection, handleResize, handleResizeMultiple,
    handleRestoreAll, handleEmptyTrash, handleRestoreSelected, handleTabSwitch, handleTabClose,
    handleItemMiddleClick, handleContextMenu, handleSwapPanels, handleSyncPanels, openAdvancedSearch,
    openDuplicateSearchHandler,
    handleGoToFolder, handleAddToFavorites, handleRemoveFromFavorites
  } = handlers;

  const handleDuplicateSearch = useCallback(() => {
    openDuplicateSearchHandler(activePanelId);
  }, [openDuplicateSearchHandler, activePanelId]);

  // Comparison Hook
  const { histogramPanels, diffPaths, isComparing, handleComparePanels, handleCalculateAllSizes } = usePanelComparison({
    left, right, activePanelId, notify, t: t as any
  });

  // Drag & Drop
  const { onDropFile } = useFileDrop({ t: t as any, notify, refreshBothPanels, refreshTreePath: (p) => treeRef.current?.refreshPath(p), setProgress, initiateFileOp, defaultTurboMode });
  const { dragState, dragTargetPath, handleDragStart, handleDrop, setDragState, setDragOverPath, setDragTargetPath } = useDragDrop(onDropFile);

  const actionContext: ActionContext = useMemo(() => ({
    activePanelId, activePanel: activePanelId === 'left' ? left : right, otherPanel: activePanelId === 'left' ? right : left,
    fileOps, clipboard: { clipboard, copy, cut, clearClipboard, copyToSystem, refreshClipboard },
    notify, t: t as any, dialogs, settings: { zipQuality, sevenZipQuality, zstdQuality, defaultTurboMode }, setProgress,
    contextMenuTarget: contextMenu?.target, isDrive: contextMenu?.isDrive, refreshDrives, mountedImages,
    tabs, activeTabId, setActiveTab, closeTab, refreshBothPanels,
    modifiers
  }), [
    activePanelId, left, right, fileOps, clipboard, copy, cut, clearClipboard, copyToSystem, refreshClipboard,
    notify, t, dialogs, zipQuality, sevenZipQuality, zstdQuality, defaultTurboMode, setProgress,
    contextMenu?.target, contextMenu?.isDrive, refreshDrives,
    tabs, activeTabId, setActiveTab, closeTab, refreshBothPanels,
    modifiers
  ]);

  useGlobalShortcuts(actionContext, tabs, activeTabId, handleTabSwitch);

  // Sync conflicts
  useEffect(() => {
    if ((fileOps.conflicts?.length ?? 0) > 0 && !dialogs.dialogs.some(d => d.type === 'conflict')) {
      dialogs.openConflictDialog({
        conflicts: fileOps.conflicts!, operation: fileOps.pendingOp?.action, totalCount: fileOps.pendingOp?.paths.length,
        onResolve: (res) => res ? fileOps.resolveConflicts(res) : fileOps.cancelOp()
      });
    }
  }, [fileOps.conflicts, fileOps.pendingOp, fileOps.resolveConflicts, fileOps.cancelOp, dialogs]);

  // Tab Sync
  const prevActiveTabIdRef = useRef(activeTabId);
  useEffect(() => {
    if (layout === 'dual') return;
    if (prevActiveTabIdRef.current !== activeTabId) { prevActiveTabIdRef.current = activeTabId; return; }
    const activeTab = tabs.find(t => t.id === activeTabId);
    if (activeTab) {
      const normTab = normalizePath(activeTab.path);
      const normPanel = normalizePath(left.path);
      if (normTab !== normPanel) {
        // Use current panel version for sync to avoid Rust-side increment loops
        updateTabPath(activeTabId, left.path, left.version);
      }
    }
  }, [left.path, left.version, activeTabId, layout, tabs, updateTabPath]);



  // Ref for callbacks used in stable IPC listeners
  const callbacksRef = useRef({ refreshBothPanels, t, setProgress });
  useEffect(() => {
    callbacksRef.current = { refreshBothPanels, t, setProgress };
  }, [refreshBothPanels, t, setProgress]);

  // IPC listeners - must be stable to not lose events during re-renders
  useEffect(() => {
    // 1. Legacy Progress listener
    const unlistenProgress = listen<any>('progress', (e) => {
      const { task, current, total, status, filename } = e.payload;
      if (status === 'completed') { setTimeout(() => callbacksRef.current.setProgress(null), 500); return; }
      let message = callbacksRef.current.t('processing');
      let cancellable = (task === 'copy' || task === 'move');
      if (task === 'copy') message = callbacksRef.current.t('op_copy' as any);
      if (task === 'move') message = callbacksRef.current.t('op_move' as any);
      if (task === 'calculate_size') message = callbacksRef.current.t('calculating_size');
      callbacksRef.current.setProgress(prev => ({ visible: true, message, task, cancellable: prev?.cancellable ?? cancellable, current, total, filename, cancelling: prev?.cancelling ?? false }));
    });

    // 2. New File Operation Event listener (Real-time tracking and refresh)
    const unlistenOp = listen<any>('file_op_event', (e) => {
      const op = e.payload; // FileOperation struct

      if (op.status === 'Completed' || (typeof op.status === 'object' && op.status.Error)) {
        // Operation finished - Refresh UI
        callbacksRef.current.refreshBothPanels();
        return;
      }
    });

    return () => {
      unlistenProgress.then(fn => fn());
      unlistenOp.then(fn => fn());
    };
  }, []);

  // Global Mouse Navigation
  const stateRef = useRef({ leftPanel: left, rightPanel: right, activePanelId, fileOps });
  stateRef.current = { leftPanel: left, rightPanel: right, activePanelId, fileOps };
  useEffect(() => {
    const handleUp = (e: MouseEvent) => {
      if (e.button === 3 || e.button === 4) {
        const pan = stateRef.current.activePanelId === 'left' ? stateRef.current.leftPanel : stateRef.current.rightPanel;
        if (e.button === 3 && pan.historyIndex > 0) pan.goBack();
        if (e.button === 4 && pan.historyIndex < pan.history.length - 1) pan.goForward();
        e.preventDefault();
      }
    };
    window.addEventListener('mouseup', handleUp, true);
    return () => window.removeEventListener('mouseup', handleUp, true);
  }, []);

  useNativeDragDrop({
    leftPanel: left, rightPanel: right, activePanelId, dragState, dragGhostRef,
    handleFileDrop: handleDrop, setDragState, setDragOverPath, setDragTargetPath, setModifiers
  });


  // Filter out completed/cancelled/error operations - they should never drive the progress UI
  const liveOperation = fileOps.activeOperation &&
    fileOps.activeOperation.status !== 'Completed' &&
    fileOps.activeOperation.status !== 'Cancelled' &&
    !(typeof fileOps.activeOperation.status === 'object' && 'Error' in fileOps.activeOperation.status)
    ? fileOps.activeOperation : null;

  const effectiveProgress = liveOperation ? {
    visible: showProgress,
    message: liveOperation.op_type === 'Copy' ? t('op_copy' as any) : liveOperation.op_type === 'Move' ? t('op_move' as any) :
      liveOperation.op_type === 'Delete' || liveOperation.op_type === 'Trash' ? t('op_delete' as any) : liveOperation.status,
    cancellable: true,
    cancelling: liveOperation.status === 'Cancelled',
    current: liveOperation.processed_bytes,
    total: liveOperation.total_bytes,
    filename: liveOperation.status === 'Paused' ? t('paused') : liveOperation.current_file,
    task: liveOperation.op_type.toLowerCase(),
    paused: liveOperation.status === 'Paused',
    canPause: liveOperation.op_type === 'Copy' || liveOperation.op_type === 'Move',
    sources: liveOperation.sources,
    destination: liveOperation.destination,
    speed: liveOperation.bytes_per_second,
    processedFiles: liveOperation.processed_files,
    totalFiles: liveOperation.total_files,
    turbo: liveOperation.turbo
  } : progress;


  const effectiveAction = useMemo(() => {
    return (dragState && dragTargetPath) ? (modifiers.ctrl ? 'copy' : modifiers.shift ? 'move' : (dragState.files[0].path.charAt(0) !== dragTargetPath.charAt(0) ? 'copy' : 'move')) : null;
  }, [dragState, dragTargetPath, modifiers.ctrl, modifiers.shift]);



  return (
    <div className="app-container" onContextMenu={(e) => handleContextMenu(e, activePanelId)}>
      <GlobalDialogContainer />
      <DualPanelLayout
        t={t} sidebarReduced={sidebarReduced} setSidebarReduced={setSidebarReduced}
        drives={drives} left={left} right={right} activePanelId={activePanelId} setActivePanelId={setActivePanelId}
        layout={layout} onLayoutChange={setLayout}
        showHidden={showHidden}
        useSystemIcons={useSystemIcons}
        searchQuery={{ left: left.searchQuery, right: right.searchQuery }}
        navigate={handleNavigate} onRefresh={refreshBothPanels} handleSearch={handleSearch}
        executeSearch={executeSearch}
        openAdvancedSearch={openAdvancedSearch}
        clearSearch={clearSearch}
        handleDragStart={handleDragStart} handleDrop={handleDrop} dragState={dragState}
        handleSelect={(id: PanelId, path: string, m: boolean, r: boolean) => { setContextMenu(null); (id === 'left' ? left : right).handleSelect(path, m, r); setActivePanelId(id); }}
        handleSelectMultiple={(id: PanelId, paths: string[], a: boolean) => { (id === 'left' ? left : right).selectMultiple(paths, a); setActivePanelId(id); }}
        handleClearSelection={(id: PanelId) => (id === 'left' ? left : right).clearSelection()}
        handleContextMenu={handleContextMenu} handleOpenFile={handleOpenFile} handleSort={handleSort} handleResize={handleResize}
        handleResizeMultiple={handleResizeMultiple} handleInlineRename={(old: string, n: string) => handleAction('file.rename', { ...actionContext, contextMenuTarget: old, renameValue: n })}
        propPaths={dialogs.propertiesPaths} histogramPanels={histogramPanels} propShowHidden={showHidden} propShowSystem={showSystem}
        cutPaths={clipboard?.action === 'cut' ? clipboard.paths : []} treeRef={treeRef}
        onTreeCut={(paths: string[]) => handleAction('file.cut', { contextMenuTarget: paths[0] })}
        onTreeCopy={(paths: string[]) => handleAction('file.copy', { contextMenuTarget: paths[0] })}
        onTreeCopyName={(name: string) => copyToSystem(name)}
        onTreeCopyPath={(path: string) => copyToSystem(path)}
        onTreeDelete={(paths: string[]) => handleAction('file.delete', { contextMenuTarget: paths[0] })}
        isShiftPressed={modifiers.shift}
        onTreeRename={(path: string) => handleAction('file.rename', { contextMenuTarget: path })}
        onTreeNewFolder={(parent: string) => handleAction('file.new_folder', { contextMenuTarget: parent })}
        onTreeUnmount={(path: string) => handleAction('drive.unmount_image', { contextMenuTarget: path, isDrive: true })}
        onTreeProperties={(path: string) => dialogs.openPropertiesDialog([path])}
        onTreePaste={(path: string) => handleAction('file.paste', { ...actionContext, contextMenuTarget: path })}
        setShowAbout={() => dialogs.openAboutDialog()} onCalculateAllSizes={handleCalculateAllSizes}
        onRestoreAll={handleRestoreAll} onRestoreSelected={handleRestoreSelected} onEmptyTrash={handleEmptyTrash}
        onTabSwitch={handleTabSwitch} onTabClose={handleTabClose} onItemMiddleClick={handleItemMiddleClick}
        onOpenNewTab={layout === 'standard' ? (path: string) => addTab(path) : undefined}
        onTabDrop={async (files: any[], index?: number) => {
          const folders = files.filter(f => f.is_dir);
          for (let i = 0; i < folders.length; i++) {
            const targetIndex = index !== undefined ? index + i : undefined;
            await addTab(folders[i].path, { index: targetIndex });
          }
          setDragState(null);
        }}
        onSwapPanels={handleSwapPanels} onSyncPanels={handleSyncPanels} isSyncDisabled={left.path === right.path}
        onComparePanels={handleComparePanels} isComparing={isComparing} diffPaths={diffPaths}
        onAddToFavorites={handleAddToFavorites}
        onRemoveFromFavorites={handleRemoveFromFavorites}
        onDriveContextMenu={(e: React.MouseEvent, p: string) => {
          const drive = drives.find(d => d.path === p);
          handleContextMenu(e, activePanelId, {
            path: p,
            is_dir: true,
            isDrive: true,
            driveType: drive?.drive_type
          } as any);
        }}
        canPaste={!!clipboard && clipboard.paths.length > 0} canUndo={fileOps.canUndo} canRedo={fileOps.canRedo}
        handleCopy={() => handleAction('file.copy', actionContext)} handleCut={() => handleAction('file.cut', actionContext)}
        handlePaste={() => handleAction('file.paste', actionContext)} handleDelete={() => handleAction('file.delete', actionContext)}
        handleUndo={handleUndo} handleRedo={handleRedo}
        undoLabel={actionService.get('file.undo')?.getLabel?.(actionContext)}
        redoLabel={actionService.get('file.redo')?.getLabel?.(actionContext)}
        onDuplicateSearch={handleDuplicateSearch}
      />
      <ProgressOverlay
        progress={effectiveProgress as any}
        t={t as any}
        onCancel={() => fileOps.cancelOp(fileOps.activeOperation?.id)}
        onPause={() => fileOps.activeOperation && invoke('pause_file_operation', { id: fileOps.activeOperation.id })}
        onResume={() => fileOps.activeOperation && invoke('resume_file_operation', { id: fileOps.activeOperation.id })}
        onToggleTurbo={(enabled) => fileOps.activeOperation && invoke('toggle_turbo', { id: fileOps.activeOperation.id, enabled })}
      />

      {contextMenu && (
        <ContextMenu
          key={`${contextMenu.x}-${contextMenu.y}`} x={contextMenu.x} y={contextMenu.y} target={contextMenu.target}
          canPaste={!!clipboard && clipboard.paths.length > 0} canUndo={fileOps.canUndo} canRedo={fileOps.canRedo}
          undoLabel={actionService.get('file.undo')?.getLabel?.(actionContext)}
          redoLabel={actionService.get('file.redo')?.getLabel?.(actionContext)}
          sortConfig={(contextMenu.panelId === 'left' ? left : right).viewMode === 'grid' ? (contextMenu.panelId === 'left' ? left : right).sortConfig : undefined}
          onSort={(f) => handleSort(activePanelId, f)}
          onSortDirection={(d) => handleSortDirection(activePanelId, d)}
          onClose={() => setContextMenu(null)} onRefresh={refreshBothPanels} onUndo={handleUndo} onRedo={handleRedo}
          onCopy={() => handleAction('file.copy', actionContext)} onCut={() => handleAction('file.cut', actionContext)}
          onPaste={() => handleAction('file.paste', actionContext)}
          onDelete={() => handleAction('file.delete', actionContext)}
          isShiftPressed={modifiers.shift}
          onRename={() => handleAction('file.rename', actionContext)} onProperties={() => handleAction('file.properties', actionContext)}
          onNewFolder={() => handleAction('file.new_folder', actionContext)} onCopyName={() => handleAction('file.copy_name', actionContext)}
          onCopyPath={() => handleAction('file.copy_path', actionContext)} t={t}
          isTrashContext={contextMenu.panelId === 'left' ? left.path.startsWith('trash://') : right.path.startsWith('trash://')}
          isSearchContext={contextMenu.panelId === 'left' ? left.path.startsWith('search://') : right.path.startsWith('search://')}
          onRestore={handleRestoreSelected} onGoToFolder={handleGoToFolder}
          onOpenNewTab={layout === 'standard' ? (path: string) => { addTab(path); setContextMenu(null); } : undefined}
          isDir={contextMenu.isDir} isBackground={contextMenu.isBackground} isDrive={contextMenu.isDrive} driveType={contextMenu.driveType}
          isReadOnly={false}
          onExtract={(p: string, toSub: boolean) => handleAction(toSub ? 'archive.extract_to_folder' : 'archive.extract_here', { ...actionContext, contextMenuTarget: p })}
          onCompress={(format: any) => handleAction(`archive.compress_${format}`, actionContext)}
          onMount={() => handleAction('drive.mount_image', actionContext)}
          onUnmount={() => handleAction('drive.unmount_image', actionContext)}
          onOpenFile={() => handleAction('file.open', actionContext)}
          isFavorite={contextMenu.isFavorite}
          onAddToFavorites={() => handleAddToFavorites(contextMenu.target!)}
          onRemoveFromFavorites={() => handleRemoveFromFavorites(contextMenu.target!)}
        />
      )}

      {dragState && (
        <div ref={dragGhostRef} className="drag-ghost" style={{ position: 'fixed', left: 0, top: 0, zIndex: 10000, pointerEvents: 'none', willChange: 'transform' }}>
          <div className="drag-ghost-main">
            <FileIcon size={16} />
            <span className="drag-ghost-text">{dragState.files.length > 1 ? `${dragState.files.length} items` : (dragState.files[0]?.name || "Item")}</span>
          </div>
          <div className="drag-ghost-hints">
            {dragTargetPath === '__TABS__' ? (
              <span className="drag-hint active">{t('new_tab') || "Open New Tab"}</span>
            ) : (
              <>
                <span className={cx("drag-hint", { active: effectiveAction === 'copy' })}>+Ctrl {t('copy')}</span>
                <span className="drag-hint-separator">|</span>
                <span className={cx("drag-hint", { active: effectiveAction === 'move' })}>+Shift {t('move')}</span>
              </>
            )}
          </div>
        </div>
      )}
      <NotificationArea notifications={notifications} onDismiss={dismissNotification} />
      <Tooltip isShiftPressed={modifiers.shift} />
    </div>
  );
}

export default App;

