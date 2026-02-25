import React, { useState, useCallback, useEffect, useRef, useMemo } from 'react';
import { invoke } from '@tauri-apps/api/core';
import { listen } from '@tauri-apps/api/event';
import cx from 'classnames';
import { ChevronRight, ChevronDown, Folder, FolderOpen, HardDrive, Usb, Disc, Trash, Star } from 'lucide-react';
import { List, ListImperativeAPI } from 'react-window';
import { AutoSizer } from 'react-virtualized-auto-sizer';

import { DriveInfo, FileEntry, SidebarNode } from '../../types';
import { TFunc } from '../../i18n';
import { useApp } from '../../context/AppContext';
import { ContextMenu } from './ContextMenu';
import { AsyncFileIcon } from './AsyncFileIcon';
import './DirectoryTree.css';

interface TreeNode {
    path: string;
    name: string;
    isHidden?: boolean;
    isSystem?: boolean;
    driveType?: DriveInfo['drive_type'];
    hasSubdirs?: boolean;
    isTrash?: boolean;
    isReadOnly?: boolean;
    isFavorite?: boolean;
    isSpacer?: boolean;
    totalBytes?: number;
    freeBytes?: number;
}

interface FlattenedNode {
    node: TreeNode;
    level: number;
}

interface DirectoryTreeProps {
    drives: DriveInfo[];
    currentPath: string;
    onNavigate: (path: string) => void;
    minimized?: boolean;
    t: TFunc;
    onCut?: (paths: string[]) => void;
    onCopy?: (paths: string[]) => void;
    onCopyName?: (name: string) => void;
    onCopyPath?: (path: string) => void;
    onDelete?: (paths: string[]) => void;
    isShiftPressed?: boolean;
    onRename?: (path: string) => void;
    onNewFolder?: (parentPath: string) => void;
    onProperties?: (path: string) => void;
    onPaste?: (path: string) => void;
    canPaste?: boolean;
    canUndo?: boolean;
    undoLabel?: string;
    canRedo?: boolean;
    redoLabel?: string;
    onUndo?: () => void;
    onRedo?: () => void;
    onUnmount?: (path: string) => void;
    useSystemIcons?: boolean;
    onDragStart?: (sourcePanel: 'left' | 'right', files: FileEntry[]) => void;
    onDrop?: (e: React.DragEvent, targetPath: string) => void;
    dragState?: { sourcePanel: 'left' | 'right'; files: FileEntry[] } | null;
    onItemMiddleClick?: (entry: FileEntry) => void;
    onOpenNewTab?: (path: string) => void;
    skipExpandAndScroll?: boolean;
    favorites?: Array<{ name: string; path: string }>;
    onAddToFavorites?: (path: string) => void;
    onRemoveFromFavorites?: (path: string) => void;
}

export interface DirectoryTreeHandle {
    refreshPath: (path: string) => Promise<void>;
    collapseAll: () => void;
    scrollToTop: () => void;
}

export const DirectoryTree = React.forwardRef<DirectoryTreeHandle, DirectoryTreeProps>(({
    drives,
    currentPath,
    onNavigate,
    minimized = false,
    t,
    onCut,
    onCopy,
    onCopyName,
    onCopyPath,
    onDelete,
    isShiftPressed,
    onRename,
    onNewFolder,
    onProperties,
    onPaste,
    canPaste,
    canUndo,
    undoLabel,
    canRedo,
    redoLabel,
    onUndo,
    onRedo,
    onUnmount,
    onDragStart,
    onDrop,
    dragState,
    useSystemIcons: propUseSystemIcons,
    onItemMiddleClick,
    onOpenNewTab,
    skipExpandAndScroll = false,
    favorites = [],
    onAddToFavorites,
    onRemoveFromFavorites
}, ref) => {
    const { useSystemIcons: contextUseSystemIcons, showHidden, showSystem } = useApp();
    const useSystemIcons = propUseSystemIcons ?? contextUseSystemIcons;
    const [expandedPaths, setExpandedPaths] = useState<Set<string>>(new Set());
    const [treeData, setTreeData] = useState<Map<string, TreeNode[]>>(new Map());
    const [skipSyncInternal, setSkipSyncInternal] = useState(false);
    const [contextMenu, setContextMenu] = useState<{
        x: number;
        y: number;
        path: string;
        name: string;
        isExpanded: boolean;
        isDrive?: boolean;
        driveType?: DriveInfo['drive_type'];
        isReadOnly?: boolean;
        isFavorite?: boolean;
    } | null>(null);

    const [isExpanding, setIsExpanding] = useState(false);

    const [dragOverNode, setDragOverNode] = useState<string | null>(null);
    const loadedPathsRef = useRef<Set<string>>(new Set());
    const listRef = useRef<ListImperativeAPI>(null);
    const treeDataRef = useRef<Map<string, TreeNode[]>>(new Map());
    const manuallyCollapsedRef = useRef<string | null>(null);

    const loadPathContent = useCallback(async (path: string) => {
        const lowerPath = path.toLowerCase();
        loadedPathsRef.current.add(lowerPath);

        try {
            const nodes: SidebarNode[] = await invoke('get_sidebar_nodes', { path });
            const treeNodes: TreeNode[] = nodes.map(n => ({
                path: n.path,
                name: n.name,
                isHidden: n.is_hidden,
                isSystem: n.is_system,
                isReadOnly: n.is_readonly,
                hasSubdirs: n.has_subdirs
            }));
            setTreeData(prev => {
                const next = new Map(prev);
                next.set(path, treeNodes);
                treeDataRef.current = next;
                return next;
            });
        } catch (error) {
            loadedPathsRef.current.delete(lowerPath);
            setTreeData(prev => {
                const next = new Map(prev);
                next.set(path, []);
                treeDataRef.current = next;
                return next;
            });
        }
    }, []);

    const refreshPath = React.useCallback(async (path: string) => {
        const lowerPath = path.toLowerCase();
        if (expandedPaths.has(lowerPath) || loadedPathsRef.current.has(lowerPath)) {
            loadedPathsRef.current.delete(lowerPath);
            await loadPathContent(path);
        }
    }, [expandedPaths, loadPathContent]);

    React.useImperativeHandle(ref, () => ({
        refreshPath,
        collapseAll: () => {
            setExpandedPaths(new Set());
        },
        scrollToTop: () => {
            if (listRef.current) {
                listRef.current.scrollToRow({ index: 0, behavior: 'smooth' });
            }
        }
    }), [refreshPath]);

    // Listen for file system changes
    useEffect(() => {
        const unlisten = listen<{ watcher_id: string; path: string; kind: string }>('fs-change', (event) => {
            const { path } = event.payload;
            const separator = path.includes('\\') ? '\\' : '/';
            const parts = path.split(separator);
            parts.pop();
            const parentPath = parts.join(separator);
            const lowerParent = parentPath.toLowerCase();

            if (loadedPathsRef.current.has(lowerParent)) {
                loadedPathsRef.current.delete(lowerParent);
                loadPathContent(parentPath);
            }
        });

        return () => {
            unlisten.then(f => f());
        };
    }, [loadPathContent]);

    // Root nodes
    const rootNodes = useMemo<TreeNode[]>(() => {
        const nodes: TreeNode[] = [];

        // Add Favorites if any
        if (favorites.length > 0) {
            favorites.forEach(fav => {
                nodes.push({
                    path: fav.path,
                    name: fav.name,
                    hasSubdirs: true,
                    isFavorite: true
                });
            });
            nodes.push({ path: '__favorites_spacer__', name: '', isSpacer: true });
        }

        // Add Drives
        drives.forEach(drive => {
            const pathClean = drive.path.replace(/[/\\]+$/, '');
            const displayName = drive.label ? `${drive.label} (${pathClean})` : pathClean;
            nodes.push({
                path: drive.path,
                name: displayName,
                driveType: drive.drive_type,
                hasSubdirs: true,
                isReadOnly: drive.is_readonly,
                totalBytes: drive.total_bytes,
                freeBytes: drive.free_bytes,
            });
        });

        // Add Trash
        nodes.push({ path: '__trash_spacer__', name: '', isSpacer: true });
        nodes.push({
            path: 'trash://',
            name: t('recycle_bin' as any),
            hasSubdirs: false,
            isTrash: true
        });

        return nodes;
    }, [drives, favorites, t]);

    // Flatten logic
    const visibleNodes = useMemo(() => {
        const result: FlattenedNode[] = [];

        const addNodes = (nodes: TreeNode[], level: number) => {
            for (const node of nodes) {
                if (node.isSpacer) {
                    result.push({ node, level });
                    continue;
                }
                if (node.isSystem) { if (!showSystem) continue; }
                else if (node.isHidden) { if (!showHidden) continue; }

                result.push({ node, level });

                const lowerPath = node.path.toLowerCase();
                if (expandedPaths.has(lowerPath)) {
                    const children = treeData.get(node.path);
                    if (children) {
                        addNodes(children, level + 1);
                    }
                }
            }
        };

        addNodes(rootNodes, 0);
        return result;
    }, [rootNodes, expandedPaths, treeData, showHidden, showSystem]);

    // Auto-expand and sync to current path
    useEffect(() => {
        if (!currentPath || currentPath === 'trash://' || skipExpandAndScroll || skipSyncInternal) return;

        const parts = currentPath.split(/[/\\]/).filter(Boolean);
        const pathsToExpand: string[] = [];
        let accumulated = '';

        // Expand everything EXCEPT the leaf node (currentPath)
        // This ensures single-click navigates without expanding.
        for (let i = 0; i < parts.length - 1; i++) {
            const part = parts[i];
            if (i === 0) {
                accumulated = `${part}\\`;
            } else {
                accumulated = `${accumulated}${part}`;
            }
            pathsToExpand.push(accumulated);
            if (i > 0) {
                accumulated = `${accumulated}\\`;
            }
        }

        setExpandedPaths(prev => {
            const next = new Set(prev);
            pathsToExpand.forEach(p => {
                const pLower = p.toLowerCase();
                if (pLower === manuallyCollapsedRef.current) return;
                next.add(pLower);
            });
            return next;
        });

        const loadPathsSequentially = async () => {
            for (const path of pathsToExpand) {
                const lowerPath = path.toLowerCase();
                if (loadedPathsRef.current.has(lowerPath)) continue;
                await loadPathContent(path);
            }
        };

        loadPathsSequentially();
    }, [currentPath, loadPathContent]);

    const lastScrolledPathRef = useRef<string | null>(null);

    // Scroll to active node
    useEffect(() => {
        if (!currentPath || minimized || skipExpandAndScroll || skipSyncInternal) return;

        // Only scroll if the path has actually changed since the last scroll
        const lowerPath = currentPath.toLowerCase();
        if (lowerPath === lastScrolledPathRef.current) return;

        const index = visibleNodes.findIndex(vn => vn.node.path.toLowerCase() === lowerPath);
        if (index !== -1 && listRef.current) {
            listRef.current.scrollToRow({ index, align: 'smart' });
            lastScrolledPathRef.current = lowerPath;
        }
    }, [currentPath, visibleNodes, minimized]);

    const toggleExpand = useCallback(async (e: React.MouseEvent, node: TreeNode) => {
        e.stopPropagation();
        const lowerPath = node.path.toLowerCase();

        setExpandedPaths(prev => {
            const next = new Set(prev);
            if (next.has(lowerPath)) {
                manuallyCollapsedRef.current = lowerPath;
                next.delete(lowerPath);
            } else {
                if (manuallyCollapsedRef.current === lowerPath) {
                    manuallyCollapsedRef.current = null;
                }
                next.add(lowerPath);
                if (!loadedPathsRef.current.has(lowerPath)) {
                    loadPathContent(node.path);
                }
            }
            return next;
        });
    }, [loadPathContent]);

    const handleContextMenu = (e: React.MouseEvent, node: TreeNode) => {
        e.preventDefault();
        e.stopPropagation();
        if (node.isTrash) return;

        const isExpanded = expandedPaths.has(node.path.toLowerCase());

        setContextMenu({
            x: e.clientX,
            y: e.clientY,
            path: node.path,
            name: node.name,
            isExpanded,
            isDrive: !!node.driveType,
            driveType: node.driveType,
            isReadOnly: node.isReadOnly,
            isFavorite: favorites.some(f => {
                const fp = f.path.replace(/[\\/]+$/, '').toLowerCase();
                const np = node.path.replace(/[\\/]+$/, '').toLowerCase();
                return fp === np;
            })
        });
    };

    const expandAll = async (path: string) => {
        setIsExpanding(true);
        try {
            const subtreeData = await invoke<Record<string, SidebarNode[]>>('get_subtree_nodes', { path });

            setTreeData(prev => {
                const next = new Map(prev);
                Object.entries(subtreeData).forEach(([parentPath, nodes]) => {
                    const treeNodes: TreeNode[] = nodes.map(n => ({
                        path: n.path,
                        name: n.name,
                        isHidden: n.is_hidden,
                        isSystem: n.is_system,
                        isReadOnly: n.is_readonly,
                        hasSubdirs: n.has_subdirs
                    }));
                    next.set(parentPath, treeNodes);
                });
                treeDataRef.current = next;
                return next;
            });

            setExpandedPaths(prev => {
                const next = new Set(prev);
                Object.keys(subtreeData).forEach(p => next.add(p.toLowerCase()));
                return next;
            });

            Object.keys(subtreeData).forEach(p => loadedPathsRef.current.add(p.toLowerCase()));
        } catch (err) {
            console.error(`Expand all failed for ${path}`, err);
        } finally {
            setIsExpanding(false);
        }
    };

    const collapseAll = (path: string) => {
        setExpandedPaths(prev => {
            const next = new Set<string>();
            const lowerPath = path.toLowerCase();
            const prefix = lowerPath.endsWith('\\') ? lowerPath : `${lowerPath}\\`;

            for (const pLower of prev) {
                if (pLower !== lowerPath && !pLower.startsWith(prefix)) {
                    next.add(pLower);
                }
            }
            return next;
        });
    };

    const getFolderIcon = (node: TreeNode, isExpanded: boolean) => {
        const driveClass = "drive-root-icon";
        if (node.driveType) {
            if (node.driveType === 'removable') return <Usb size="1rem" className={cx(driveClass)} />;
            if (node.driveType === 'cdrom') return <Disc size="1rem" className={cx(driveClass)} />;
            return <HardDrive size="1rem" className={cx(driveClass)} />;
        }
        if (node.isTrash) return <Trash size="1rem" className={cx(driveClass)} />;
        if (node.isFavorite) return <Star size="1rem" className="sidebar-favorite-icon" />;
        if (useSystemIcons) {
            return <AsyncFileIcon path={node.path} isDir={true} name={node.name} size={16} className="system-icon-img" />;
        }
        return isExpanded ?
            <FolderOpen size="1rem" className="file-icon folder" strokeWidth={1.5} fill="currentColor" fillOpacity={0.2} /> :
            <Folder size="1rem" className="file-icon folder" strokeWidth={1.5} fill="currentColor" fillOpacity={0.2} />;
    };

    const Row = ({ index, style }: { index: number; style: React.CSSProperties }) => {
        const { node, level } = visibleNodes[index];
        if (!node) return null;
        if (node.isSpacer) return (
            <div style={style} className="tree-node-row">
                <div className="tree-separator-container">
                    <div className="tree-divider" />
                </div>
            </div>
        );

        const isExpanded = expandedPaths.has(node.path.toLowerCase());
        const isActive = currentPath.toLowerCase() === node.path.toLowerCase();
        const isDragOver = dragOverNode === node.path;
        const isRootDrive = !!node.driveType || !!node.isTrash;

        const children = treeData.get(node.path);
        const visibleChildren = children?.filter(c => {
            if (c.isSystem) { return showSystem; }
            if (c.isHidden) { return showHidden; }
            return true;
        });
        const hasEffectiveChildren = visibleChildren ? visibleChildren.length > 0 : node.hasSubdirs;

        return (
            <div style={style} className="tree-node-row">
                <div
                    className={cx('tree-node-content', {
                        active: isActive,
                        'context-active': contextMenu?.path.toLowerCase() === node.path.toLowerCase(),
                        dimmed: node.isHidden || node.isSystem,
                        'drag-over': isDragOver,
                        'root-drive-item': isRootDrive
                    })}
                    data-path={node.path}
                    style={{ paddingLeft: `${level * 1 + 0.5}rem` }}
                    onClick={(e) => {
                        if (e.button !== 0) return;
                        if (node.isFavorite) {
                            setSkipSyncInternal(true);
                            onNavigate(node.path);
                            setTimeout(() => setSkipSyncInternal(false), 500);
                        } else {
                            onNavigate(node.path);
                        }
                    }}
                    onDoubleClick={(e) => {
                        toggleExpand(e, node);
                    }}
                    onMouseDown={(e) => {
                        if (e.button === 1) e.preventDefault();
                    }}
                    onContextMenu={(e) => {
                        handleContextMenu(e, node);
                    }}
                    onMouseEnter={() => {
                        if (dragState && dragOverNode !== node.path) {
                            setDragOverNode(node.path);
                        }
                    }}
                    onMouseLeave={() => setDragOverNode(null)}
                    onMouseUp={(e) => {
                        if (onItemMiddleClick && e.button === 1) {
                            onItemMiddleClick({ path: node.path, name: node.name, is_dir: true } as any);
                        }
                        if (dragState) {
                            onDrop?.(e as any, node.path);
                            setDragOverNode(null);
                        }
                    }}
                    draggable
                    onDragStart={(e) => {
                        e.preventDefault();
                        if (onDragStart) {
                            onDragStart('left', [{ path: node.path, name: node.name, is_dir: true } as any]);
                        }
                    }}
                    data-tooltip={isRootDrive && !node.isTrash && node.totalBytes
                        ? node.path
                        : (isRootDrive ? node.name : `${node.name}\n${node.path}`)}
                    data-tooltip-total={node.totalBytes}
                    data-tooltip-free={node.freeBytes}
                    data-tooltip-multiline={isRootDrive ? "true" : (isRootDrive ? undefined : "true")}
                >
                    <div
                        className={cx('tree-chevron', { invisible: !hasEffectiveChildren })}
                        onClick={(e) => {
                            e.stopPropagation();
                            toggleExpand(e, node);
                        }}
                    >
                        {hasEffectiveChildren && (
                            isExpanded ? <ChevronDown className="icon-sm" /> : <ChevronRight className="icon-sm" />
                        )}
                    </div>
                    <div className="tree-icon">
                        {getFolderIcon(node, isExpanded)}
                    </div>
                    <div className="tree-label">{node.name}</div>
                </div>
            </div>
        );
    };

    if (minimized) return null;

    return (
        <div className={cx("directory-tree", { "is-loading": isExpanding })}>
            <AutoSizer renderProp={({ height, width }: any) => {
                if (height === undefined || width === undefined) return null;
                return (
                    <List
                        listRef={listRef}
                        rowCount={visibleNodes.length}
                        rowHeight={(index: number) => {
                            const vn = visibleNodes[index];
                            if (vn?.node.isSpacer) return 13;
                            if (vn?.node.driveType || vn?.node.isTrash) return 34;
                            return 28;
                        }}
                        className="virtual-tree-list"
                        rowComponent={Row}
                        rowProps={{} as any}
                        style={{ height, width, overflowX: 'auto' }}
                    />
                );
            }} />
            {contextMenu && (
                <ContextMenu
                    x={contextMenu.x}
                    y={contextMenu.y}
                    target={contextMenu.path}
                    canUndo={canUndo || false}
                    undoLabel={undoLabel}
                    canRedo={canRedo || false}
                    redoLabel={redoLabel}
                    onClose={() => setContextMenu(null)}
                    onRefresh={() => { refreshPath(contextMenu.path); setContextMenu(null); }}
                    onUndo={onUndo || (() => { })}
                    onRedo={onRedo || (() => { })}
                    onCopy={() => { onCopy?.([contextMenu.path]); setContextMenu(null); }}
                    onCut={() => { onCut?.([contextMenu.path]); setContextMenu(null); }}
                    onPaste={() => { onPaste?.(contextMenu.path); setContextMenu(null); }}
                    canPaste={canPaste || false}
                    onDelete={() => { onDelete?.([contextMenu.path]); setContextMenu(null); }}
                    isShiftPressed={isShiftPressed}
                    onRename={() => { onRename?.(contextMenu.path); setContextMenu(null); }}
                    onProperties={() => { onProperties?.(contextMenu.path); setContextMenu(null); }}
                    onNewFolder={() => { onNewFolder?.(contextMenu.path); setContextMenu(null); }}
                    onCopyName={() => { onCopyName?.(contextMenu.name); setContextMenu(null); }}
                    onCopyPath={() => {
                        if (onCopyPath) {
                            onCopyPath(contextMenu.path);
                        } else {
                            navigator.clipboard.writeText(contextMenu.path);
                        }
                        setContextMenu(null);
                    }}
                    t={t}
                    isTreeContext={true}
                    onExpandAll={() => { expandAll(contextMenu.path); setContextMenu(null); }}
                    onCollapseAll={() => { collapseAll(contextMenu.path); setContextMenu(null); }}
                    isDrive={contextMenu.isDrive}
                    driveType={contextMenu.driveType}
                    onUnmount={() => { onUnmount?.(contextMenu.path); setContextMenu(null); }}
                    onOpenNewTab={(path) => { onOpenNewTab?.(path); setContextMenu(null); }}
                    isDir={true}
                    isFavorite={contextMenu.isFavorite}
                    onAddToFavorites={() => { onAddToFavorites?.(contextMenu.path); setContextMenu(null); }}
                    onRemoveFromFavorites={() => { onRemoveFromFavorites?.(contextMenu.path); setContextMenu(null); }}
                />
            )}
        </div>
    );
});

