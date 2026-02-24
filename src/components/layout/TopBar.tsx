import React, { useState } from 'react';
import { ArrowLeft, ArrowRight, ArrowUp, Home, RefreshCw, Undo2, Redo2, Copy, Scissors, Trash2, ClipboardPaste, Minus, Square, X, ChartBarBig, RotateCcw, ArrowLeftRight, StretchVertical, GitCompare, Sidebar, Columns, Wrench, Search } from 'lucide-react';
import { getCurrentWindow } from '@tauri-apps/api/window';
import { invoke } from '@tauri-apps/api/core';
import { homeDir } from '@tauri-apps/api/path';
import { PathBar } from './PathBar';
import { SettingsMenu } from './SettingsMenu';
import { TFunc } from '../../i18n';
import { getParent } from '../../utils/path';
import './TopBar.css';
import cx from 'classnames';
import { PanelState, LayoutMode, DriveInfo } from '../../types';

interface TopBarProps {
    activePanel: PanelState;
    activePanelId: 'left' | 'right';
    canUndo: boolean;
    undoLabel?: string;
    canRedo: boolean;
    redoLabel?: string;
    onNavigate: (path: string) => void;
    onNavigateUp: () => void;
    onNavigateBack: () => void;
    onNavigateForward: () => void;
    onRefresh: () => void;
    onUndo: () => void;
    onRedo: () => void;
    onCopy: () => void;
    onCut: () => void;
    onDelete: () => void;
    onPaste: () => void;
    canPaste: boolean;
    t: TFunc;

    // Layout & settings
    layout: LayoutMode;
    onLayoutChange: (mode: LayoutMode) => void;
    showHidden: boolean;
    onShowAbout: () => void;

    // Drag Drop (needed for PathBar in single mode)
    isDragging: boolean;
    onDrop: (path: string | null, e?: React.MouseEvent) => void;
    isShiftPressed?: boolean;
    drives: DriveInfo[];
    onCalculateAllSizes: () => void;
    onAdvancedSearch: () => void;
    onDuplicateSearch: () => void;
    // Trash actions
    isTrashView?: boolean;
    onEmptyTrash?: () => void;
    onRestoreAll?: () => void;
    onRestoreSelected?: () => void;
    // Dual Panel Management
    onSwapPanels?: () => void;
    onSyncPanels?: () => void;
    isSyncDisabled?: boolean;
    onComparePanels?: () => void;
    isComparing?: boolean;
}

export const TopBar: React.FC<TopBarProps> = ({
    activePanel,
    activePanelId,
    canUndo,
    undoLabel,
    canRedo,
    redoLabel,
    onNavigate,
    onNavigateUp,
    onNavigateBack,
    onNavigateForward,
    onRefresh,
    onUndo,
    onRedo,
    onCopy,
    onCut,
    onDelete,
    onPaste,
    canPaste,
    t,
    layout,
    onLayoutChange,
    showHidden,
    onShowAbout,
    isDragging,
    onDrop,
    drives,
    onCalculateAllSizes,
    onAdvancedSearch,
    onDuplicateSearch,
    isTrashView = false,
    onEmptyTrash,
    onRestoreAll,
    onRestoreSelected,
    onSwapPanels,
    onSyncPanels,
    isSyncDisabled = false,
    onComparePanels,
    isComparing = false,
    isShiftPressed
}) => {
    const [settingsOpen, setSettingsOpen] = useState(false);
    const [settingsPage, setSettingsPage] = useState<'main' | 'themes' | 'languages' | 'dates' | 'compression'>('main');

    const [hamburgerOpen, setHamburgerOpen] = useState(false);
    const [homePath, setHomePath] = useState<string>('C:\\Users');
    const [isMaximized, setIsMaximized] = useState(false);

    React.useEffect(() => {
        homeDir().then(path => {
            if (path) setHomePath(path);
        }).catch(err => console.error("Failed to get home dir", err));
    }, []);

    const closeSettings = () => {
        setSettingsOpen(false);
        setSettingsPage('main');
    };

    const toggleSettings = () => {
        setSettingsOpen(!settingsOpen);
        setSettingsPage('main');
        if (hamburgerOpen) setHamburgerOpen(false);
    };

    const closeHamburger = () => setHamburgerOpen(false);
    const toggleHamburger = () => {
        setHamburgerOpen(!hamburgerOpen);
        if (settingsOpen) setSettingsOpen(false);
    };

    // Close settings/hamburger when clicking outside
    React.useEffect(() => {
        if (!settingsOpen && !hamburgerOpen) return;

        const handleClickOutside = (e: MouseEvent) => {
            const target = e.target as HTMLElement;
            if (settingsOpen && !target.closest('.settings-container')) {
                closeSettings();
            }
            if (hamburgerOpen && !target.closest('.hamburger-container')) {
                closeHamburger();
            }
        };

        document.addEventListener('mousedown', handleClickOutside);
        return () => document.removeEventListener('mousedown', handleClickOutside);
    }, [settingsOpen, hamburgerOpen]);

    // Windows 11 Snap Layouts Support
    React.useEffect(() => {
        const btn = document.getElementById('titlebar-maximize');
        if (!btn) return;

        const updateRect = async () => {
            const rect = btn.getBoundingClientRect();
            try {
                const max = await getCurrentWindow().isMaximized();
                setIsMaximized(max);
                await invoke('oxide_sync_snap_rect', {
                    rect: {
                        x: Math.round(rect.x),
                        y: Math.round(rect.y),
                        width: Math.round(rect.width),
                        height: Math.round(rect.height)
                    }
                });
            } catch (err) {
            }
        };

        const observer = new ResizeObserver(() => {
            updateRect();
        });

        observer.observe(btn);
        window.addEventListener('resize', updateRect);
        updateRect();

        return () => {
            observer.disconnect();
            window.removeEventListener('resize', updateRect);
        };
    }, []);

    return (
        <div className="header" data-tauri-drag-region>
            <div className="settings-container branding" onClick={(e) => e.stopPropagation()}>
                <button className="btn-icon" onClick={toggleSettings} data-tooltip={t('settings' as any)} data-tooltip-pos="bottom">
                    <img src="/logo.svg" className="icon-lg app-logo-icon" alt="Oxyde" />
                </button>
                <SettingsMenu
                    isOpen={settingsOpen}
                    onClose={closeSettings}
                    page={settingsPage}
                    onPageChange={setSettingsPage}
                    onShowAbout={onShowAbout}
                />
            </div>

            <div className="nav-group">
                <button className="btn-icon" onClick={onNavigateBack} disabled={activePanel.historyIndex <= 0} data-tooltip={t('back')} data-tooltip-pos="bottom"><ArrowLeft className="icon-lg" /></button>
                <button className="btn-icon" onClick={onNavigateForward} disabled={activePanel.historyIndex >= activePanel.history.length - 1} data-tooltip={t('forward' as any)} data-tooltip-pos="bottom"><ArrowRight className="icon-lg" /></button>
                <button className="btn-icon" onClick={onNavigateUp} disabled={!getParent(activePanel.path)} data-tooltip={t('up' as any)} data-tooltip-pos="bottom"><ArrowUp className="icon-lg" /></button>
                <button className="btn-icon" onClick={() => onNavigate(homePath)} disabled={activePanel.path === homePath} data-tooltip={t('home' as any) || 'Home'} data-tooltip-pos="bottom"><Home className="icon-lg" /></button>
                <button className="btn-icon" onClick={onRefresh} data-tooltip={t('refresh')} data-tooltip-pos="bottom"><RefreshCw className="icon-lg" /></button>

                {canUndo && (
                    <button
                        className="btn-icon"
                        onClick={onUndo}
                        data-tooltip={undoLabel || t('undo_action')}
                        data-tooltip-pos="bottom"
                    >
                        <Undo2 className="icon-lg" />
                    </button>
                )}
                {canRedo && (
                    <button
                        className="btn-icon"
                        onClick={onRedo}
                        data-tooltip={redoLabel || t('redo_action')}
                        data-tooltip-pos="bottom"
                    >
                        <Redo2 className="icon-lg" />
                    </button>
                )}
            </div>

            {layout === 'standard' ? (
                <div className="path-bar-container" data-tauri-drag-region>
                    <PathBar
                        className="path-bar"
                        path={activePanel.path}
                        onNavigate={onNavigate}
                        isDragging={isDragging}
                        onDrop={onDrop}
                        drives={drives}
                        showHidden={showHidden}
                        panelId={activePanelId}
                        t={t}
                    />
                </div>
            ) : (
                <>
                    <div className="flex-spacer" />
                    <div className="dual-panel-tools" data-tauri-drag-region>
                        <button
                            className="btn-icon"
                            onClick={onSwapPanels}
                            data-tooltip={t('swap_panels' as any)}
                            data-tooltip-pos="bottom"
                        >
                            <ArrowLeftRight className="icon-lg" />
                        </button>
                        <button
                            className="btn-icon"
                            onClick={onSyncPanels}
                            disabled={isSyncDisabled}
                            data-tooltip={t('sync_panels' as any)}
                            data-tooltip-pos="bottom"
                        >
                            <StretchVertical className="icon-lg" />
                        </button>
                        <button
                            className={cx("btn-icon", { active: isComparing })}
                            onClick={onComparePanels}
                            data-tooltip={t('compare_panels' as any)}
                            data-tooltip-pos="bottom"
                        >
                            <GitCompare className="icon-lg" />
                        </button>
                    </div>
                </>
            )}

            <div className="toolbar-actions">
                {activePanel.selected.size > 0 && (
                    <button className="btn-icon" data-tooltip={t("cut")} data-tooltip-pos="bottom" onClick={onCut}><Scissors className="icon-lg" /></button>
                )}
                {activePanel.selected.size > 0 && !isTrashView && (
                    <button className="btn-icon" data-tooltip={t("copy")} data-tooltip-pos="bottom" onClick={onCopy}><Copy className="icon-lg" /></button>
                )}
                {canPaste && !isTrashView && (
                    <button className="btn-icon" data-tooltip={t("paste")} data-tooltip-pos="bottom" onClick={onPaste}><ClipboardPaste className="icon-lg" /></button>
                )}
                {activePanel.selected.size > 0 && (
                    <button
                        className="btn-icon danger"
                        data-tooltip={isTrashView || isShiftPressed ? t("perm_delete" as any) : t("delete")}
                        data-tooltip-pos="bottom"
                        onClick={onDelete}
                    >
                        <Trash2 className="icon-lg" />
                    </button>
                )}
            </div>

            <div className="app-tools-container">
                <button
                    className="btn-icon"
                    onClick={() => onLayoutChange(layout === 'standard' ? 'dual' : 'standard')}
                    data-tooltip={layout === 'standard' ? t('dual') : t('single')}
                    data-tooltip-pos="bottom"
                >
                    {layout === 'standard' ? <Columns className="icon-lg" /> : <Sidebar className="icon-lg" />}
                </button>

                <div className="hamburger-container">
                    <button className={cx("btn-icon", { active: hamburgerOpen })} onClick={toggleHamburger} data-tooltip={t('tools' as any) || 'Tools'} data-tooltip-pos="bottom">
                        <Wrench className="icon-lg" />
                    </button>
                    {hamburgerOpen && (
                        <div className="hamburger-menu" onClick={(e) => e.stopPropagation()}>
                            <div className="hamburger-item" onClick={() => { onCalculateAllSizes(); closeHamburger(); }}>
                                <div className="hamburger-item-content">
                                    <ChartBarBig size={14} />
                                    {t('calculate_size' as any) || 'Histogram'}
                                </div>
                            </div>
                            <div className="hamburger-item" onClick={() => { onAdvancedSearch(); closeHamburger(); }}>
                                <div className="hamburger-item-content">
                                    <Search size={14} />
                                    {t('advanced_search' as any) || 'Advanced Search'}
                                </div>
                            </div>
                            <div className="hamburger-item" onClick={() => { onDuplicateSearch(); closeHamburger(); }}>
                                <div className="hamburger-item-content">
                                    <Copy size={14} />
                                    {t('duplicates' as any) || 'Duplicate Search'}
                                </div>
                            </div>
                        </div>
                    )}
                </div>
            </div>

            {isTrashView && (
                <div className="toolbar-actions trash-actions">
                    <button
                        className="btn-icon"
                        onClick={onRestoreSelected}
                        disabled={activePanel.selected.size === 0}
                        data-tooltip={t('restore_selected' as any)}
                        data-tooltip-pos="bottom"
                    >
                        <RotateCcw className="icon-lg" />
                    </button>
                    <button
                        className="btn-icon"
                        onClick={onRestoreAll}
                        data-tooltip={t('restore_all' as any)}
                        data-tooltip-pos="bottom"
                    >
                        <RotateCcw className="icon-lg" />
                        <span style={{ fontSize: '0.625rem', marginLeft: '2px' }}>{t('all' as any)}</span>
                    </button>
                    <button
                        className="btn-icon danger"
                        onClick={onEmptyTrash}
                        data-tooltip={t('empty_recycle_bin' as any)}
                        data-tooltip-pos="bottom"
                    >
                        <Trash2 className="icon-lg" />
                        <span style={{ fontSize: '0.625rem', marginLeft: '2px' }}>{t('all' as any)}</span>
                    </button>
                </div>
            )}

            <div className="window-controls">
                <div className="btn-icon" onClick={() => getCurrentWindow().minimize()} data-tooltip={t('minimize' as any)} data-tooltip-pos="bottom"><Minus className="icon-sm" /></div>
                <div className="btn-icon" id="titlebar-maximize" onClick={async () => {
                    await getCurrentWindow().toggleMaximize();
                    setIsMaximized(await getCurrentWindow().isMaximized());
                }} data-tooltip={isMaximized ? t('restore' as any) || 'Restore' : t('maximize' as any)} data-tooltip-pos="bottom">
                    {isMaximized ? <Copy className="icon-xs" style={{ transform: 'rotate(180deg) scaleY(-1)' }} /> : <Square className="icon-xs" />}
                </div>
                <div className="btn-icon danger" onClick={() => getCurrentWindow().close()} data-tooltip={t('close' as any)} data-tooltip-pos="bottom"><X className="icon-sm" /></div>
            </div>
        </div>
    );
};
