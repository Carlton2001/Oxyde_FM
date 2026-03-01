import React, { useState, useEffect } from 'react';
import cx from 'classnames';
import { PanelLeft, PanelLeftClose, Trash, ArrowUpToLine, ChevronsUp, Globe } from 'lucide-react';
import { invoke } from '@tauri-apps/api/core';
import { listen } from '@tauri-apps/api/event';
import { FileEntry, DriveInfo, QuickAccessItem } from '../../types';
import { TFunc } from '../../i18n';
import { DirectoryTree, DirectoryTreeHandle } from '../ui/DirectoryTree';
import { FavoritesMenu } from '../ui/FavoritesMenu';
import { getDriveTooltip, shouldShowDriveCapacity } from '../../utils/drive';
import { useApp } from '../../context/AppContext';
import './Sidebar.css';

interface SidebarProps {
    minimized: boolean;
    onToggle: () => void;
    drives: DriveInfo[];
    currentPath: string;
    onNavigate: (path: string) => void;
    t: TFunc;
    treeRef?: React.RefObject<DirectoryTreeHandle | null>;
    // DirectoryTree action callbacks
    onTreeCut?: (paths: string[]) => void;
    onTreeCopy?: (paths: string[]) => void;
    onTreeCopyName?: (name: string) => void;
    onTreeCopyPath?: (path: string) => void;
    onTreeDelete?: (paths: string[]) => void;
    isShiftPressed?: boolean;
    onTreeRename?: (path: string) => void;
    onTreeNewFolder?: (parentPath: string) => void;
    onTreeUnmount?: (path: string) => void;
    onTreeDisconnectDrive?: (path: string) => void;
    onTreeProperties?: (path: string) => void;
    onTreePaste?: (path: string) => void;
    canPaste?: boolean;
    canUndo?: boolean;
    undoLabel?: string;
    canRedo?: boolean;
    redoLabel?: string;
    onUndo?: () => void;
    onRedo?: () => void;
    // DnD props
    // DnD props
    onDragStart?: (sourcePanel: 'left' | 'right', files: FileEntry[]) => void;
    onDrop?: (e: React.DragEvent, targetPath: string) => void;
    dragState?: { sourcePanel: 'left' | 'right'; files: FileEntry[] } | null;
    useSystemIcons?: boolean;
    onItemMiddleClick?: (entry: FileEntry) => void;
    onOpenNewTab?: (path: string) => void;
    onDriveContextMenu?: (e: React.MouseEvent, path: string) => void;
    onAddToFavorites?: (path: string) => void;
    onRemoveFromFavorites?: (path: string) => void;
    onTreeEmptyTrash?: () => void;
}

export const Sidebar: React.FC<SidebarProps> = ({
    minimized,
    onToggle,
    drives,
    currentPath,
    onNavigate,
    t,
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
    canPaste,
    canUndo,
    undoLabel,
    canRedo,
    redoLabel,
    onUndo,
    onRedo,
    onDragStart,
    onDrop,
    dragState,
    useSystemIcons,
    onItemMiddleClick,
    onOpenNewTab,
    onDriveContextMenu,
    onAddToFavorites,
    onRemoveFromFavorites,
    onTreeEmptyTrash
}) => {
    const { showNetwork } = useApp();
    const sidebarRef = React.useRef<HTMLDivElement>(null);
    const [width, setWidth] = React.useState(() => {
        const saved = localStorage.getItem('sidebarWidth');
        return saved ? parseInt(saved, 10) : 200;
    });
    const isResizingRef = React.useRef(false);
    const [favorites, setFavorites] = useState<QuickAccessItem[]>([]);

    useEffect(() => {
        let isFetching = false;
        let pendingFetch = false;

        const fetchFavorites = async () => {
            if (isFetching) {
                pendingFetch = true;
                return;
            }
            if (minimized) return;

            isFetching = true;
            try {
                const items: QuickAccessItem[] = await invoke('get_quick_access_items');
                setFavorites(items);
            } catch (err) {
                console.error("Failed to fetch favorites:", err);
            } finally {
                isFetching = false;
                if (pendingFetch) {
                    pendingFetch = false;
                    fetchFavorites();
                }
            }
        };

        if (!minimized) {
            fetchFavorites();
        }

        const unlistenPromise = listen('quick-access-changed', () => {
            // Add a small delay to debounce rapid successive events from the filesystem watcher
            setTimeout(fetchFavorites, 200);
        });

        return () => {
            unlistenPromise.then(unlisten => unlisten());
        };
    }, [minimized]);


    const handleMouseDown = React.useCallback((e: React.MouseEvent) => {
        e.preventDefault();
        isResizingRef.current = true;
        document.body.style.cursor = 'col-resize';
        document.body.style.userSelect = 'none';

        // Add will-change for GPU acceleration during resize
        if (sidebarRef.current) {
            sidebarRef.current.style.willChange = 'width';
        }

        const handleMouseMove = (moveEvent: MouseEvent) => {
            if (isResizingRef.current && sidebarRef.current) {
                const newWidth = Math.max(150, Math.min(window.innerWidth / 2, moveEvent.clientX));
                sidebarRef.current.style.width = `${newWidth}px`;
            }
        };

        const handleMouseUp = (upEvent: MouseEvent) => {
            if (isResizingRef.current) {
                const finalWidth = Math.max(150, Math.min(window.innerWidth / 2, upEvent.clientX));
                setWidth(finalWidth);
                localStorage.setItem('sidebarWidth', String(finalWidth));
                isResizingRef.current = false;
                document.body.style.cursor = '';
                document.body.style.userSelect = '';

                if (sidebarRef.current) {
                    sidebarRef.current.style.willChange = '';
                }
            }

            window.removeEventListener('mousemove', handleMouseMove);
            window.removeEventListener('mouseup', handleMouseUp);
        };

        window.addEventListener('mousemove', handleMouseMove);
        window.addEventListener('mouseup', handleMouseUp);
    }, []);

    return (
        <div
            ref={sidebarRef}
            className={cx("sidebar", { reduced: minimized })}
            style={{ width: minimized ? undefined : width }}
        >
            <div className="sidebar-header">
                {!minimized && (
                    <div className="sidebar-tools">
                        <button
                            className="btn-icon"
                            onClick={() => treeRef?.current?.collapseAll()}
                            data-tooltip={t('collapse_all' as any)}
                            data-tooltip-pos="bottom"
                        >
                            <ChevronsUp className="icon-md" />
                        </button>
                        <button
                            className="btn-icon"
                            onClick={() => {
                                treeRef?.current?.scrollToTop();
                            }}
                            data-tooltip={t('scroll_to_top' as any)}
                            data-tooltip-pos="bottom"
                        >
                            <ArrowUpToLine className="icon-md" />
                        </button>
                    </div>
                )}
                <div className="spacer" />
                <button
                    className="btn-icon"
                    onClick={onToggle}
                    data-tooltip={minimized ? t('expand' as any) : t('reduce' as any)}
                    data-tooltip-pos="right"
                >
                    {minimized ? <PanelLeft className="icon-md" /> : <PanelLeftClose className="icon-md" />}
                </button>
            </div>

            {minimized && (
                <div className="minimized-actions">
                    <FavoritesMenu
                        onNavigate={onNavigate}
                        currentPath={currentPath}
                        buttonClassName="drive-icon-btn favorites-btn"
                    />
                    <div className="minimized-divider" />
                    {drives.map(drive => {
                        const letter = drive.path.charAt(0).toUpperCase();
                        const isActive = currentPath.startsWith(drive.path);
                        return (
                            <div key={drive.path} className="minimized-drive-wrapper">
                                <button
                                    className={cx("drive-icon-btn", { active: isActive })}
                                    onClick={() => onNavigate(drive.path)}
                                    onContextMenu={(e) => onDriveContextMenu?.(e, drive.path)}
                                    onMouseDown={(e) => {
                                        if (e.button === 1) {
                                            e.preventDefault();
                                            onOpenNewTab?.(drive.path);
                                        }
                                    }}
                                    data-tooltip={getDriveTooltip(drive, t)}
                                    data-tooltip-total={shouldShowDriveCapacity(drive) ? drive.total_bytes : undefined}
                                    data-tooltip-free={shouldShowDriveCapacity(drive) ? drive.free_bytes : undefined}
                                    data-tooltip-multiline={shouldShowDriveCapacity(drive) ? "true" : undefined}
                                    data-tooltip-pos="right"
                                >
                                    {letter}
                                </button>
                            </div>
                        );
                    })}
                    <div className="minimized-divider" />
                    {showNetwork && (
                        <button
                            className={cx("drive-icon-btn", { active: currentPath === '__network_vincinity__' })}
                            onClick={() => onNavigate('__network_vincinity__')}
                            onContextMenu={(e) => onDriveContextMenu?.(e, '__network_vincinity__')}
                            onMouseDown={(e) => {
                                if (e.button === 1) {
                                    e.preventDefault();
                                    onOpenNewTab?.('__network_vincinity__');
                                }
                            }}
                            data-tooltip={t('network_vincinity' as any)}
                            data-tooltip-pos="right"
                        >
                            <Globe size="1.125rem" />
                        </button>
                    )}
                    <button
                        className={cx("drive-icon-btn", { active: currentPath === 'trash://' })}
                        onClick={() => onNavigate('trash://')}
                        onContextMenu={(e) => onDriveContextMenu?.(e, 'trash://')}
                        onMouseDown={(e) => {
                            if (e.button === 1) {
                                e.preventDefault();
                                onOpenNewTab?.('trash://');
                            }
                        }}
                        data-tooltip={t('recycle_bin' as any)}
                        data-tooltip-pos="right"
                    >
                        <Trash size="1.125rem" />
                    </button>
                </div>
            )}

            {!minimized && (
                <DirectoryTree
                    ref={treeRef}
                    drives={drives}
                    currentPath={currentPath}
                    onNavigate={onNavigate}
                    minimized={minimized}
                    t={t}
                    onCut={onTreeCut}
                    onCopy={onTreeCopy}
                    onCopyName={onTreeCopyName}
                    onCopyPath={onTreeCopyPath}
                    onDelete={onTreeDelete}
                    isShiftPressed={isShiftPressed}
                    onRename={onTreeRename}
                    onNewFolder={onTreeNewFolder}
                    onProperties={onTreeProperties}
                    onPaste={onTreePaste}
                    canPaste={canPaste}
                    canUndo={canUndo}
                    undoLabel={undoLabel}
                    canRedo={canRedo}
                    redoLabel={redoLabel}
                    onUndo={onUndo}
                    onRedo={onRedo}
                    onDragStart={onDragStart}
                    onDrop={onDrop}
                    dragState={dragState}
                    useSystemIcons={useSystemIcons}
                    onItemMiddleClick={onItemMiddleClick}
                    onOpenNewTab={onOpenNewTab}
                    onUnmount={onTreeUnmount}
                    onDisconnectDrive={onTreeDisconnectDrive}
                    favorites={favorites}
                    onAddToFavorites={onAddToFavorites}
                    onRemoveFromFavorites={onRemoveFromFavorites}
                    onEmptyTrash={onTreeEmptyTrash}
                />
            )}

            {!minimized && (
                <div
                    className="sidebar-resizer"
                    onMouseDown={handleMouseDown}
                />
            )}
        </div>
    );
};


