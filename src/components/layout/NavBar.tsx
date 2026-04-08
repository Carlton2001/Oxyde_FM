import React, { useState, useRef, useEffect, useLayoutEffect } from 'react';
import { ArrowLeft, ArrowRight, ArrowUp, Home, RefreshCw, Undo2, Redo2, Copy, Scissors, Trash2, ClipboardPaste, RotateCcw, Folder, HardDrive, Globe, Network, Usb, Disc, X } from 'lucide-react';
import { homeDir } from '@tauri-apps/api/path';
import { SearchBox } from '../ui/SearchBox';
import { AsyncFileIcon } from '../ui/AsyncFileIcon';
import { PathBar } from './PathBar';
import { DriveInfo, PanelId } from '../../types';
import { TFunc } from '../../i18n';
import { useApp } from '../../context/AppContext';
import { getParent } from '../../utils/path';
import './NavBar.css';

interface NavBarProps {
    panelId: PanelId;
    t: TFunc;
    activePanel: any;
    canUndo: boolean;
    undoLabel?: string;
    canRedo: boolean;
    redoLabel?: string;
    onNavigate: (path: string) => void;
    onNavigateUp: () => void;
    onNavigateBack: () => void;
    onNavigateForward: () => void;
    onNavigateToIndex: (index: number) => void;
    onRefresh: () => void;
    onUndo: () => void;
    onRedo: () => void;
    onCopy: () => void;
    onCopyName: () => void;
    onCopyPath: () => void;
    onCut: () => void;
    onDelete: () => void;
    onPaste: () => void;
    canPaste: boolean;

    isShiftPressed?: boolean;
    drives: DriveInfo[];

    // Trash actions
    isTrashView?: boolean;
    onEmptyTrash?: () => void;
    onRestoreAll?: () => void;
    onRestoreSelected?: () => void;

    isTrashEmpty?: boolean;
    isNukeOverride?: boolean;
    // Quick Search integration
    searchQuery?: string;
    isSearchActive?: boolean;
    onSearchChange?: (q: string) => void;
    onSearchSubmit?: () => void;
    onSearchClear?: () => void;
    onSearchCancel?: () => void;
    onClosePanel?: () => void;
}

export const NavBar: React.FC<NavBarProps> = ({
    panelId,
    activePanel,
    canUndo,
    undoLabel,
    canRedo,
    redoLabel,
    onNavigate,
    onNavigateUp,
    onNavigateBack,
    onNavigateForward,
    onNavigateToIndex,
    onRefresh,
    onUndo,
    onRedo,
    onCopy,
    onCopyName,
    onCopyPath,
    onCut,
    onDelete,
    onPaste,
    canPaste,
    t,
    drives,
    isTrashView = false,
    onEmptyTrash,
    onRestoreAll,
    onRestoreSelected,
    isTrashEmpty = false,
    isNukeOverride = false,
    isShiftPressed,
    searchQuery,
    isSearchActive = false,
    onSearchChange,
    onSearchSubmit,
    onSearchClear,
    onSearchCancel,
    onClosePanel
}) => {
    const { useSystemIcons } = useApp();
    const [copyMenuOpen, setCopyMenuOpen] = useState(false);
    const copyBtnRef = useRef<HTMLButtonElement>(null);
    const copyMenuRef = useRef<HTMLDivElement>(null);
    const [copyMenuPos, setCopyMenuPos] = useState<{ top: number; anchorCenterX: number } | null>(null);

    type HistMenuPos = { top: number; anchorCenterX: number };
    const [backMenuPos, setBackMenuPos] = useState<HistMenuPos | null>(null);
    const [fwdMenuPos, setFwdMenuPos] = useState<HistMenuPos | null>(null);
    const backBtnRef = useRef<HTMLButtonElement>(null);
    const fwdBtnRef = useRef<HTMLButtonElement>(null);
    const backMenuRef = useRef<HTMLDivElement>(null);
    const fwdMenuRef = useRef<HTMLDivElement>(null);

    const [homePath, setHomePath] = useState<string>('C:\\Users');

    useEffect(() => {
        homeDir().then(path => {
            if (path) setHomePath(path);
        }).catch(err => console.error("Failed to get home dir", err));
    }, []);

    useEffect(() => {
        if (!copyMenuOpen) return;
        const handleClose = (e: MouseEvent) => {
            const target = e.target as Node;
            if (
                copyMenuRef.current && !copyMenuRef.current.contains(target) &&
                copyBtnRef.current && !copyBtnRef.current.contains(target)
            ) {
                setCopyMenuOpen(false);
            }
        };
        document.addEventListener('mousedown', handleClose);
        document.addEventListener('contextmenu', handleClose, true);
        return () => {
            document.removeEventListener('mousedown', handleClose);
            document.removeEventListener('contextmenu', handleClose, true);
        };
    }, [copyMenuOpen]);

    useLayoutEffect(() => {
        if (backMenuPos && backMenuRef.current) {
            const { width, height } = backMenuRef.current.getBoundingClientRect();
            const PAD = 8;
            let left = backMenuPos.anchorCenterX - width / 2;
            left = Math.max(PAD, Math.min(left, window.innerWidth - width - PAD));
            let top = backMenuPos.top;
            if (top + height > window.innerHeight - PAD) top = Math.max(PAD, window.innerHeight - height - PAD);
            backMenuRef.current.style.left = `${left}px`;
            backMenuRef.current.style.top = `${top}px`;
            backMenuRef.current.style.visibility = 'visible';
        }
    }, [backMenuPos]);

    useLayoutEffect(() => {
        if (fwdMenuPos && fwdMenuRef.current) {
            const { width, height } = fwdMenuRef.current.getBoundingClientRect();
            const PAD = 8;
            let left = fwdMenuPos.anchorCenterX - width / 2;
            left = Math.max(PAD, Math.min(left, window.innerWidth - width - PAD));
            let top = fwdMenuPos.top;
            if (top + height > window.innerHeight - PAD) top = Math.max(PAD, window.innerHeight - height - PAD);
            fwdMenuRef.current.style.left = `${left}px`;
            fwdMenuRef.current.style.top = `${top}px`;
            fwdMenuRef.current.style.visibility = 'visible';
        }
    }, [fwdMenuPos]);

    useEffect(() => {
        if (!backMenuPos && !fwdMenuPos) return;
        const handleClose = (e: MouseEvent) => {
            const t = e.target as Node;
            if (backMenuRef.current?.contains(t) || backBtnRef.current?.contains(t)) return;
            if (fwdMenuRef.current?.contains(t) || fwdBtnRef.current?.contains(t)) return;
            setBackMenuPos(null);
            setFwdMenuPos(null);
        };
        document.addEventListener('mousedown', handleClose);
        document.addEventListener('contextmenu', handleClose, true);
        return () => {
            document.removeEventListener('mousedown', handleClose);
            document.removeEventListener('contextmenu', handleClose, true);
        };
    }, [backMenuPos, fwdMenuPos]);

    useLayoutEffect(() => {
        if (copyMenuOpen && copyMenuPos && copyMenuRef.current) {
            const { width, height } = copyMenuRef.current.getBoundingClientRect();
            const PAD = 8;
            let left = copyMenuPos.anchorCenterX - width / 2;
            left = Math.max(PAD, Math.min(left, window.innerWidth - width - PAD));
            let top = copyMenuPos.top;
            if (top + height > window.innerHeight - PAD) top = Math.max(PAD, window.innerHeight - height - PAD);
            copyMenuRef.current.style.left = `${left}px`;
            copyMenuRef.current.style.top = `${top}px`;
            copyMenuRef.current.style.visibility = 'visible';
        }
    }, [copyMenuOpen, copyMenuPos]);

    return (
        <div className="header">
            <div className="nav-group">
                <button
                    ref={backBtnRef}
                    className="btn-icon"
                    onClick={onNavigateBack}
                    disabled={activePanel.historyIndex <= 0}
                    data-tooltip={t('back')}
                    data-tooltip-pos="bottom"
                    onContextMenu={(e) => {
                        e.preventDefault();
                        if (activePanel.historyIndex <= 0 || !backBtnRef.current) return;
                        setFwdMenuPos(null);
                        const rect = backBtnRef.current.getBoundingClientRect();
                        setBackMenuPos({ top: rect.bottom + 4, anchorCenterX: rect.left + rect.width / 2 });
                    }}
                ><ArrowLeft className="icon-lg" /></button>
                <button
                    ref={fwdBtnRef}
                    className="btn-icon"
                    onClick={onNavigateForward}
                    disabled={activePanel.historyIndex >= activePanel.history.length - 1}
                    data-tooltip={t('forward' as any)}
                    data-tooltip-pos="bottom"
                    onContextMenu={(e) => {
                        e.preventDefault();
                        if (activePanel.historyIndex >= activePanel.history.length - 1 || !fwdBtnRef.current) return;
                        setBackMenuPos(null);
                        const rect = fwdBtnRef.current.getBoundingClientRect();
                        setFwdMenuPos({ top: rect.bottom + 4, anchorCenterX: rect.left + rect.width / 2 });
                    }}
                ><ArrowRight className="icon-lg" /></button>
                <button className="btn-icon" onClick={onNavigateUp} disabled={!getParent(activePanel.path)} data-tooltip={t('up' as any)} data-tooltip-pos="bottom"><ArrowUp className="icon-lg" /></button>
                <button className="btn-icon" onClick={() => onNavigate(homePath)} disabled={activePanel.path === homePath} data-tooltip={t('home' as any) || 'Home'} data-tooltip-pos="bottom"><Home className="icon-lg" /></button>
                <button className="btn-icon" onClick={onRefresh} data-tooltip={t('refresh')} data-tooltip-pos="bottom"><RefreshCw className="icon-lg" /></button>

                {onClosePanel && (
                    <button className="btn-icon danger" onClick={onClosePanel} data-tooltip={t('close' as any)} data-tooltip-pos="bottom">
                        <X className="icon-lg" />
                    </button>
                )}

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

            {activePanel?.path && (
                <PathBar
                    panelId={panelId}
                    path={activePanel.path}
                    onNavigate={onNavigate}
                    t={t}
                    drives={drives}
                />
            )}

            <div className="flex-spacer" />

            <div className="toolbar-actions">
                {activePanel.selected.size > 0 && (
                    <button className="btn-icon" data-tooltip={t("cut")} data-tooltip-pos="bottom" onClick={onCut}><Scissors className="icon-lg" /></button>
                )}
                {activePanel.selected.size > 0 && !isTrashView && (
                    <button
                        ref={copyBtnRef}
                        className="btn-icon"
                        data-tooltip={t("copy")}
                        data-tooltip-pos="bottom"
                        onClick={onCopy}
                        onContextMenu={(e) => {
                            e.preventDefault();
                            if (!copyBtnRef.current) return;
                            const rect = copyBtnRef.current.getBoundingClientRect();
                            setCopyMenuPos({ top: rect.bottom + 4, anchorCenterX: rect.left + rect.width / 2 });
                            setCopyMenuOpen(o => !o);
                        }}
                    >
                        <Copy className="icon-lg" />
                    </button>
                )}
                {canPaste && !isTrashView && (
                    <button className="btn-icon" data-tooltip={t("paste")} data-tooltip-pos="bottom" onClick={onPaste}><ClipboardPaste className="icon-lg" /></button>
                )}
                {activePanel.selected.size > 0 && (
                    <button
                        className="btn-icon danger"
                        data-tooltip={(isTrashView || isShiftPressed || isNukeOverride) ? t("perm_delete" as any) : t("delete")}
                        data-tooltip-pos="bottom"
                        onClick={onDelete}
                    >
                        <Trash2 className="icon-lg" />
                    </button>
                )}
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
                        disabled={isTrashEmpty}
                        data-tooltip={t('restore_all' as any)}
                        data-tooltip-pos="bottom"
                    >
                        <RotateCcw className="icon-lg" />
                        <span style={{ fontSize: '0.625rem', marginLeft: '2px' }}>{t('all' as any)}</span>
                    </button>
                    <button
                        className="btn-icon danger"
                        onClick={onEmptyTrash}
                        disabled={isTrashEmpty}
                        data-tooltip={t('empty_recycle_bin' as any)}
                        data-tooltip-pos="bottom"
                    >
                        <Trash2 className="icon-lg" />
                        <span style={{ fontSize: '0.625rem', marginLeft: '2px' }}>{t('all' as any)}</span>
                    </button>
                </div>
            )}

            <div className="header-search-container">
                <SearchBox
                    query={searchQuery || ''}
                    placeholder={t('search') + "..."}
                    isSearching={isSearchActive}
                    onChange={onSearchChange || (() => {})}
                    onSubmit={onSearchSubmit || (() => {})}
                    onClear={onSearchClear || (() => {})}
                    onCancel={onSearchCancel}
                    autoExpand={true}
                    className="header-search-box"
                />
            </div>

            {(backMenuPos || fwdMenuPos) && (() => {
                const getHistIcon = (path: string, driveList: typeof drives) => {
                    if (path === 'trash://') return <Trash2 size={14} />;
                    if (path === '__network_vincinity__') return <Globe size={14} />;
                    if (path.startsWith('\\\\')) return <Network size={14} />;
                    if (useSystemIcons) {
                        const isRoot = /^[a-zA-Z]:\\$/.test(path);
                        return <AsyncFileIcon path={path} isDir={true} name={isRoot ? path : (path.split('\\').filter(Boolean).pop() || path)} size={16} className="system-icon-img" />;
                    }
                    if (/^[a-zA-Z]:\\$/.test(path)) {
                        const drive = driveList?.find(d => d.path.toLowerCase() === path.toLowerCase());
                        if (drive?.drive_type === 'removable') return <Usb size={14} />;
                        if (drive?.drive_type === 'cdrom') return <Disc size={14} />;
                        return <HardDrive size={14} />;
                    }
                    return <Folder size={14} />;
                };
                const getHistLabel = (path: string) =>
                    path === '__network_vincinity__' ? t('network_vincinity' as any)
                    : path === 'trash://' ? t('recycle_bin' as any)
                    : (path.split('\\').filter(Boolean).pop() || path);

                return (
                    <>
                        {backMenuPos && (() => {
                            const items = activePanel.history.slice(0, activePanel.historyIndex).reverse();
                            return (
                                <div ref={backMenuRef} className="hamburger-menu" style={{ position: 'fixed', top: backMenuPos.top, left: backMenuPos.anchorCenterX, visibility: 'hidden', minWidth: 0, width: 'fit-content', marginTop: 0, maxHeight: '400px', overflowY: 'auto' }}>
                                    {items.map((entry: any, i: number) => {
                                        const targetIndex = activePanel.historyIndex - 1 - i;
                                        return (
                                            <div key={targetIndex} className="hamburger-item" onClick={() => { onNavigateToIndex(targetIndex); setBackMenuPos(null); }} data-tooltip={entry.path}>
                                                <div className="hamburger-item-content">
                                                    {getHistIcon(entry.path, drives)}
                                                    {getHistLabel(entry.path)}
                                                </div>
                                            </div>
                                        );
                                    })}
                                </div>
                            );
                        })()}
                        {fwdMenuPos && (() => {
                            const items = activePanel.history.slice(activePanel.historyIndex + 1);
                            return (
                                <div ref={fwdMenuRef} className="hamburger-menu" style={{ position: 'fixed', top: fwdMenuPos.top, left: fwdMenuPos.anchorCenterX, visibility: 'hidden', minWidth: 0, width: 'fit-content', marginTop: 0, maxHeight: '400px', overflowY: 'auto' }}>
                                    {items.map((entry: any, i: number) => {
                                        const targetIndex = activePanel.historyIndex + 1 + i;
                                        return (
                                            <div key={targetIndex} className="hamburger-item" onClick={() => { onNavigateToIndex(targetIndex); setFwdMenuPos(null); }} data-tooltip={entry.path}>
                                                <div className="hamburger-item-content">
                                                    {getHistIcon(entry.path, drives)}
                                                    {getHistLabel(entry.path)}
                                                </div>
                                            </div>
                                        );
                                    })}
                                </div>
                            );
                        })()}
                    </>
                );
            })()}

            {copyMenuOpen && copyMenuPos && (
                <div
                    ref={copyMenuRef}
                    className="hamburger-menu"
                    style={{ position: 'fixed', top: copyMenuPos.top, left: copyMenuPos.anchorCenterX, visibility: 'hidden', minWidth: 0, width: 'fit-content', marginTop: 0 }}
                >
                    <div className="hamburger-item" onClick={() => { onCopyName(); setCopyMenuOpen(false); }}>
                        <div className="hamburger-item-content">
                            <Copy size={14} />
                            {t('copy_name' as any)}
                        </div>
                    </div>
                    <div className="hamburger-item" onClick={() => { onCopyPath(); setCopyMenuOpen(false); }}>
                        <div className="hamburger-item-content">
                            <Copy size={14} />
                            {t('copy_path' as any)}
                        </div>
                    </div>
                </div>
            )}
        </div>
    );
};
