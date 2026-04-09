import React, { useState, useCallback, useEffect, useRef, useMemo } from 'react';
import { invoke } from '@tauri-apps/api/core';
import { listen } from '@tauri-apps/api/event';
import cx from 'classnames';
import { ChevronRight, ChevronDown, HardDrive, Usb, Disc, Trash, Network, Folder, FolderOpen, Pin, Globe } from 'lucide-react';
import { getDriveDisplayName, getDriveTooltip, shouldShowDriveCapacity } from '../../utils/drive';
import { List, ListImperativeAPI } from 'react-window';
import { AutoSizer } from 'react-virtualized-auto-sizer';

import { DriveInfo, FileEntry, SidebarNode } from '../../types';
import { TFunc } from '../../i18n';
import { useApp } from '../../context/AppContext';
import { ContextMenu } from './ContextMenu';
import { AsyncFileIcon } from './AsyncFileIcon';
import { getParent, normalizePath } from '../../utils/path';
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
    isProtected?: boolean;
    isFavorite?: boolean;
    isSpacer?: boolean;
    totalBytes?: number;
    freeBytes?: number;
    isNetwork?: boolean;
    isNetworkRoot?: boolean;
    remotePath?: string;
    label?: string;
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
    onDisconnectDrive?: (path: string) => void;
    useSystemIcons?: boolean;
    onDragStart?: (sourcePanel: string, files: FileEntry[]) => void;
    onDrop?: (e: React.DragEvent, targetPath: string) => void;
    dragState?: { sourcePanel: string; files: FileEntry[] } | null;
    onItemMiddleClick?: (entry: FileEntry) => void;
    onOpenNewTab?: (path: string) => void;
    skipExpandAndScroll?: boolean;
    favorites?: Array<{ name: string; path: string }>;
    onAddToFavorites?: (path: string) => void;
    onRemoveFromFavorites?: (path: string) => void;
    onEmptyTrash?: () => void;
    onRestoreAll?: () => void;
    onTrashProperties?: () => void;
    onDragOver?: (path: string | null) => void;
    syncSidebarWithPath?: boolean;
}

export interface DirectoryTreeHandle {
    refreshPath: (path: string) => Promise<void>;
    collapseAll: () => void;
    scrollToTop: () => void;
    revealPath: (path: string) => Promise<void>;
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
    onDisconnectDrive,
    onDragStart,
    onDrop,
    dragState,
    useSystemIcons: propUseSystemIcons,
    onItemMiddleClick,
    onOpenNewTab,
    skipExpandAndScroll = false,
    favorites = [],
    onAddToFavorites,
    onRemoveFromFavorites,
    onEmptyTrash,
    onRestoreAll,
    onTrashProperties,
    onDragOver,
    syncSidebarWithPath = true
}, ref) => {
    const { useSystemIcons: contextUseSystemIcons, showHidden, showSystem, showNetwork } = useApp();
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
        isNetworkComputer?: boolean;
        isTrash?: boolean;
    } | null>(null);

    const [isExpanding, setIsExpanding] = useState(false);

    const [dragOverNode, setDragOverNode] = useState<string | null>(null);
    const loadedPathsRef = useRef<Set<string>>(new Set());
    const listRef = useRef<ListImperativeAPI>(null);
    const treeDataRef = useRef<Map<string, TreeNode[]>>(new Map());
    const manuallyCollapsedRef = useRef<string | null>(null);

    const loadPathContent = useCallback(async (path: string) => {
        const normPath = normalizePath(path);
        const lowerPath = normPath.toLowerCase();

        // Handle network resource discovery for neighborhood or server roots
        const isNetworkServer = path.startsWith('\\\\') && path.split('\\').filter(Boolean).length === 1;
        if (path === '__network_vincinity__' || isNetworkServer) {
            try {
                const networkPath = path === '__network_vincinity__' ? undefined : path;
                const netResources: any[] = await invoke('get_network_resources', { path: networkPath });
                const treeNodes: TreeNode[] = netResources.map(r => ({
                    path: r.remote_path,
                    name: r.name,
                    isNetwork: true,
                    hasSubdirs: (r.usage & 2) !== 0, // RESOURCEUSAGE_CONTAINER
                }));
                setTreeData(prev => {
                    const next = new Map(prev);
                    next.set(normPath, treeNodes);
                    treeDataRef.current = next;
                    return next;
                });
                loadedPathsRef.current.add(lowerPath);
                return;
            } catch (error) {
                console.error("Failed to load network resources:", error);
                setTreeData(prev => {
                    const next = new Map(prev);
                    next.set(path, []);
                    treeDataRef.current = next;
                    return next;
                });
                loadedPathsRef.current.add(lowerPath);
                return;
            }
        }

        loadedPathsRef.current.add(lowerPath);

        try {
            const nodes: SidebarNode[] = await invoke('get_sidebar_nodes', { path });
            const treeNodes: TreeNode[] = nodes.map(n => ({
                path: n.path,
                name: n.name,
                isHidden: n.is_hidden,
                isSystem: n.is_system,
                isReadOnly: n.is_readonly,
                isProtected: n.is_protected,
                hasSubdirs: n.has_subdirs
            }));
            setTreeData(prev => {
                const next = new Map(prev);
                next.set(normPath, treeNodes);
                treeDataRef.current = next;
                return next;
            });
        } catch (error) {
            loadedPathsRef.current.delete(lowerPath);
            setTreeData(prev => {
                const next = new Map(prev);
                next.set(normPath, []);
                treeDataRef.current = next;
                return next;
            });
        }
    }, []);

    const refreshPath = React.useCallback(async (path: string) => {
        const normPath = normalizePath(path);
        const lowerPath = normPath.toLowerCase();
        if (expandedPaths.has(lowerPath) || loadedPathsRef.current.has(lowerPath)) {
            loadedPathsRef.current.delete(lowerPath);
            await loadPathContent(normPath);
        }
    }, [expandedPaths, loadPathContent]);

    const fsChangeDebounceRef = useRef<Record<string, any>>({});

    // Listen for file system changes
    // Listen for file system changes
    useEffect(() => {
        const unlisten = listen<{ kind: string; paths: string[] }>('fs-change', (event) => {
            const { paths } = event.payload;

            paths.forEach(p => {
                const normP = normalizePath(p);
                const normPLower = normP.toLowerCase();

                const debounceRefresh = (path: string) => {
                    const lower = path.toLowerCase();
                    if (fsChangeDebounceRef.current[lower]) {
                        clearTimeout(fsChangeDebounceRef.current[lower]);
                    }
                    fsChangeDebounceRef.current[lower] = setTimeout(() => {
                        loadPathContent(path);
                        delete fsChangeDebounceRef.current[lower];
                    }, 200);
                };

                // 1. Refresh node itself if loaded
                if (loadedPathsRef.current.has(normPLower)) {
                    debounceRefresh(normP);
                }

                // 2. Refresh parent node
                const parentPath = getParent(normP);
                if (parentPath) {
                    const normParent = normalizePath(parentPath);
                    const normParentLower = normParent.toLowerCase();

                    // Check if parent is loaded OR if it's a root drive (always "loaded" conceptually)
                    const isRoot = normParent.length <= 3 && normParent.includes(":");
                    if (loadedPathsRef.current.has(normParentLower) || isRoot) {
                        debounceRefresh(normParent);
                    }
                }
            });
        });

        return () => {
            unlisten.then(f => f());
            Object.values(fsChangeDebounceRef.current).forEach(clearTimeout);
        };
    }, [loadPathContent]);

    // Sync expanded paths with backend watcher
    useEffect(() => {
        const paths = Array.from(expandedPaths).filter(p => !p.startsWith('trash://') && !p.startsWith('search://') && p !== '__network_vincinity__');
        if (paths.length > 0 || expandedPaths.size === 0) {
            invoke('update_sidebar_watchers', { paths }).catch(err => {
                console.error("Failed to update sidebar watchers:", err);
            });
        }
    }, [expandedPaths]);

    // Root nodes
    const rootNodes = useMemo<TreeNode[]>(() => {
        const nodes: TreeNode[] = [];

        // Add Favorites if any
        if (favorites.length > 0) {
            favorites.forEach(fav => {
                nodes.push({
                    path: fav.path,
                    name: fav.name,
                    hasSubdirs: false,
                    isFavorite: true
                });
            });
            nodes.push({ path: '__favorites_spacer__', name: '', isSpacer: true });
        }

        // Add Drives
        drives.forEach(drive => {
            const displayName = getDriveDisplayName(drive, t);

            nodes.push({
                path: drive.path,
                name: displayName,
                driveType: drive.drive_type,
                hasSubdirs: true,
                isReadOnly: drive.is_readonly,
                totalBytes: drive.total_bytes,
                freeBytes: drive.free_bytes,
                remotePath: drive.remote_path,
                label: drive.label
            });
        });

        // Add Network
        nodes.push({ path: '__network_spacer__', name: '', isSpacer: true });
        if (showNetwork) {
            nodes.push({
                path: '__network_vincinity__',
                name: t('network_vincinity' as any),
                hasSubdirs: true,
                isNetworkRoot: true
            });
        }

        // Add Trash
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

                const normPath = normalizePath(node.path);
                const lowerPath = normPath.toLowerCase();
                if (expandedPaths.has(lowerPath) && !node.isFavorite) {
                    const children = treeData.get(normPath);
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
        if (!syncSidebarWithPath || !currentPath || currentPath === 'trash://' || skipExpandAndScroll || skipSyncInternal) return;

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
                // Always refresh if it's the immediate parent of our current target, 
                // otherwise only load if not already loaded
                const isImmediateParent = lowerPath === normalizePath(getParent(currentPath) || "").toLowerCase();
                if (loadedPathsRef.current.has(lowerPath) && !isImmediateParent) continue;
                await loadPathContent(path);
            }
            
            // Also ensure the current path node itself is loaded if expanded
            const currentLower = currentPath.toLowerCase();
            if (expandedPaths.has(currentLower)) {
                await loadPathContent(currentPath);
            }
        };

        loadPathsSequentially();
    }, [currentPath, loadPathContent]);

    const revealPath = useCallback(async (path: string) => {
        if (!path || path === 'trash://' || path === '__network_vincinity__') return;

        const parts = path.split(/[/\\]/).filter(Boolean);
        const pathsToExpand: string[] = [];
        let accumulated = '';

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
                next.add(pLower);
            });
            return next;
        });

        // Sequential load to ensure tree consistency
        for (const p of pathsToExpand) {
            const lowerP = p.toLowerCase();
            if (!loadedPathsRef.current.has(lowerP)) {
                await loadPathContent(p);
            }
        }

        // Scroll to the revealed node after a small delay to let state settle
        setTimeout(() => {
            const lowerPath = normalizePath(path).toLowerCase();
            const index = visibleNodes.findIndex(vn => {
                const vnNorm = normalizePath(vn.node.path);
                return vnNorm.toLowerCase() === lowerPath && !vn.node.isFavorite;
            });
            if (index !== -1 && listRef.current) {
                listRef.current.scrollToRow({ index, align: 'smart' });
                lastScrolledPathRef.current = lowerPath;
            }
        }, 100);
    }, [loadPathContent, visibleNodes]);

    const lastScrolledPathRef = useRef<string | null>(null);

    // Scroll to active node
    useEffect(() => {
        if (!currentPath || minimized || skipExpandAndScroll || skipSyncInternal) return;

        // Only scroll if the path has actually changed since the last scroll
        const normPath = normalizePath(currentPath);
        const lowerPath = normPath.toLowerCase();
        if (lowerPath === lastScrolledPathRef.current) return;

        const index = visibleNodes.findIndex(vn => {
            const vnNorm = normalizePath(vn.node.path);
            return vnNorm.toLowerCase() === lowerPath && !vn.node.isFavorite;
        });
        if (index !== -1 && listRef.current) {
            listRef.current.scrollToRow({ index, align: 'smart' });
            lastScrolledPathRef.current = lowerPath;
        }
    }, [currentPath, visibleNodes, minimized]);

    React.useImperativeHandle(ref, () => ({
        refreshPath,
        collapseAll: () => {
            setExpandedPaths(new Set());
        },
        scrollToTop: () => {
            if (listRef.current) {
                listRef.current.scrollToRow({ index: 0, behavior: 'smooth' });
            }
        },
        revealPath
    }), [refreshPath, revealPath]);

    const toggleExpand = useCallback(async (e: React.MouseEvent, node: TreeNode) => {
        e.stopPropagation();
        if (node.isFavorite) return;
        const normPath = normalizePath(node.path);
        const lowerPath = normPath.toLowerCase();

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

    const handleKeyDown = useCallback((e: React.KeyboardEvent) => {
        if (!visibleNodes.length) return;

        const currentIndex = visibleNodes.findIndex(vn => vn.node.path.toLowerCase() === currentPath.toLowerCase());

        const isNavigable = (index: number) => {
            if (index < 0 || index >= visibleNodes.length) return false;
            const vn = visibleNodes[index];
            if (vn.node.isSpacer || vn.node.isFavorite || vn.node.isTrash || vn.node.isNetworkRoot || vn.node.isNetwork) return false;
            return true;
        };

        if (!isNavigable(currentIndex)) return;

        switch (e.key) {
            case 'ArrowDown':
                e.preventDefault();
                if (isNavigable(currentIndex + 1)) {
                    onNavigate(visibleNodes[currentIndex + 1].node.path);
                }
                break;
            case 'ArrowUp':
                e.preventDefault();
                if (isNavigable(currentIndex - 1)) {
                    onNavigate(visibleNodes[currentIndex - 1].node.path);
                }
                break;
            case 'ArrowRight':
                e.preventDefault();
                if (currentIndex !== -1) {
                    const node = visibleNodes[currentIndex].node;
                    if (node.hasSubdirs && !expandedPaths.has(node.path.toLowerCase())) {
                        toggleExpand(e as any, node);
                    } else if (expandedPaths.has(node.path.toLowerCase())) {
                        if (isNavigable(currentIndex + 1)) {
                            onNavigate(visibleNodes[currentIndex + 1].node.path);
                        }
                    }
                }
                break;
            case 'ArrowLeft':
                e.preventDefault();
                if (currentIndex !== -1) {
                    const node = visibleNodes[currentIndex].node;
                    if (expandedPaths.has(node.path.toLowerCase())) {
                        toggleExpand(e as any, node);
                    } else {
                        const parentPath = getParent(node.path);
                        if (parentPath) onNavigate(parentPath);
                    }
                }
                break;
            case 'Enter':
                e.preventDefault();
                if (currentIndex !== -1) {
                    onNavigate(visibleNodes[currentIndex].node.path);
                }
                break;
            case 'ContextMenu':
            case 'Apps':
                e.preventDefault();
                e.stopPropagation();
                if (currentIndex !== -1) {
                    const node = visibleNodes[currentIndex].node;
                    const element = Array.from((e.currentTarget as HTMLElement).querySelectorAll('.tree-node-content'))
                        .find(el => el.getAttribute('data-path') === node.path);
                    
                    if (element) {
                        const rect = element.getBoundingClientRect();
                        handleContextMenu({
                            clientX: rect.left + rect.width / 2,
                            clientY: rect.top + rect.height / 2,
                            preventDefault: () => { },
                            stopPropagation: () => { }
                        } as any, node);
                    } else {
                        // Fallback: try finding any active/selected item
                        const anyActive = (e.currentTarget as HTMLElement).querySelector('.tree-node-content.active');
                        if (anyActive) {
                            const rect = anyActive.getBoundingClientRect();
                            handleContextMenu({
                                clientX: rect.left + rect.width / 2,
                                clientY: rect.top + rect.height / 2,
                                preventDefault: () => { },
                                stopPropagation: () => { }
                            } as any, node);
                        } else {
                            const panelRect = (e.currentTarget as HTMLElement).getBoundingClientRect();
                            handleContextMenu({
                                clientX: panelRect.left + panelRect.width / 2,
                                clientY: panelRect.top + panelRect.height / 2,
                                preventDefault: () => { },
                                stopPropagation: () => { }
                            } as any, node);
                        }
                    }
                }
                break;
        }
    }, [visibleNodes, currentPath, expandedPaths, toggleExpand, onNavigate]);

    const handleContextMenu = (e: React.MouseEvent, node: TreeNode) => {
        e.preventDefault();
        e.stopPropagation();
        const isTrash = node.isTrash;
        const normPath = normalizePath(node.path);
        const lowerPath = normPath.toLowerCase();
        const isExpanded = expandedPaths.has(lowerPath);

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
            }),
            isNetworkComputer: node.isNetworkRoot || node.isNetwork || (node.path.startsWith('\\\\') && node.path.split('\\').filter(Boolean).length === 1),
            isTrash: isTrash
        });
    };

    const expandAll = async (path: string) => {
        setIsExpanding(true);
        try {
            const subtreeData = await invoke<Record<string, SidebarNode[]>>('get_subtree_nodes', { path });

            setTreeData(prev => {
                const next = new Map(prev);
                Object.entries(subtreeData).forEach(([parentPath, nodes]) => {
                    const normParent = normalizePath(parentPath);
                    const treeNodes: TreeNode[] = nodes.map(n => ({
                        path: n.path,
                        name: n.name,
                        isHidden: n.is_hidden,
                        isSystem: n.is_system,
                        isReadOnly: n.is_readonly,
                        isProtected: n.is_protected,
                        hasSubdirs: n.has_subdirs
                    }));
                    next.set(normParent, treeNodes);
                });
                treeDataRef.current = next;
                return next;
            });

            setExpandedPaths(prev => {
                const next = new Set(prev);
                Object.keys(subtreeData).forEach(p => next.add(normalizePath(p).toLowerCase()));
                return next;
            });

            Object.keys(subtreeData).forEach(p => loadedPathsRef.current.add(normalizePath(p).toLowerCase()));
        } catch (err) {
            console.error(`Expand all failed for ${path}`, err);
        } finally {
            setIsExpanding(false);
        }
    };

    const collapseAll = (path: string) => {
        const normPath = normalizePath(path).toLowerCase();
        setExpandedPaths(prev => {
            const next = new Set(prev);
            // Remove the path itself and all its children
            for (const p of next) {
                if (p === normPath || p.startsWith(normPath.endsWith('\\') ? normPath : normPath + '\\')) {
                    next.delete(p);
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
            if (node.driveType === 'remote') return <Network size="1rem" className={cx(driveClass)} />;
            return <HardDrive size="1rem" className={cx(driveClass)} />;
        }
        if (node.isTrash) return <Trash size="1rem" className={cx(driveClass)} />;
        if (node.isNetworkRoot) return <Globe size="1rem" className={cx(driveClass)} />;
        if (node.isNetwork) return <Network size="1rem" className={cx(driveClass)} />;
        if (node.isFavorite) return <Pin size="1rem" className="sidebar-favorite-icon" style={{ transform: 'rotate(45deg)' }} />;
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

        const normNodePath = normalizePath(node.path);
        const nodePathLower = normNodePath.toLowerCase();
        const isExpanded = expandedPaths.has(nodePathLower);
        const isActive = normalizePath(currentPath).toLowerCase() === nodePathLower;
        const isDragOver = dragOverNode === node.path;
        const isRootDrive = !!node.driveType || !!node.isTrash || !!node.isNetworkRoot;

        const children = treeData.get(normNodePath);
        const visibleChildren = children?.filter(c => {
            if (c.isSystem) { return showSystem; }
            if (c.isHidden) { return showHidden; }
            return true;
        });
        const hasEffectiveChildren = node.isFavorite ? false : (visibleChildren ? visibleChildren.length > 0 : node.hasSubdirs);

        if (node.isFavorite) {
            return (
                <div style={style} className="tree-node-row">
                    <div
                        className={cx('quick-access-node', {
                            active: isActive,
                            'context-active': contextMenu?.path.toLowerCase() === node.path.toLowerCase(),
                            'drag-over': isDragOver
                        })}
                        data-path={node.path}
                        data-tooltip={node.path}
                        onClick={(e) => {
                            if (e.button !== 0) return;
                            setSkipSyncInternal(true);
                            onNavigate(node.path);
                            setTimeout(() => setSkipSyncInternal(false), 500);
                        }}
                        onContextMenu={(e) => {
                            handleContextMenu(e, node);
                        }}
                        onMouseEnter={() => {
                            if (dragState) {
                                if (dragOverNode !== node.path) setDragOverNode(node.path);
                                if (onDragOver) onDragOver(node.path);
                            }
                        }}
                        onMouseLeave={() => {
                            setDragOverNode(null);
                            if (onDragOver) onDragOver(null);
                        }}
                        onMouseUp={(e) => {
                            if (onItemMiddleClick && e.button === 1) {
                                onItemMiddleClick({ path: node.path, name: node.name, is_dir: true } as any);
                            }
                            if (dragState) {
                                onDrop?.(e as any, node.path);
                                setDragOverNode(null);
                            }
                        }}
                    >
                        <div className="tree-icon">
                            <Pin size="0.875rem" style={{ transform: 'rotate(45deg)' }} />
                        </div>
                        <div className="tree-label">{node.name}</div>
                    </div>
                </div>
            );
        }

        return (
            <div style={style} className="tree-node-row">
                <div
                    className={cx('tree-node-content', {
                        active: isActive,
                        'context-active': contextMenu?.path.toLowerCase() === node.path.toLowerCase(),
                        dimmed: node.isHidden || node.isSystem,
                        protected: node.isProtected,
                        'drag-over': isDragOver,
                        'root-drive-item': isRootDrive
                    })}
                    data-path={node.path}
                    style={{ paddingLeft: `${level * 1 + 0.5}rem` }}
                    onClick={(e) => {
                        if (e.button !== 0) return;
                        onNavigate(node.path);
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
                            if (onDragOver) onDragOver(node.path);
                        }
                    }}
                    onMouseLeave={() => {
                        setDragOverNode(null);
                        if (onDragOver) onDragOver(null);
                    }}
                    onMouseUp={(e) => {
                        if (onItemMiddleClick && e.button === 1) {
                            onItemMiddleClick({ path: node.path, name: node.name, is_dir: true } as any);
                        }
                        if (dragState) {
                            onDrop?.(e as any, node.path);
                            setDragOverNode(null);
                        }
                    }}
                    draggable={!isRootDrive}
                    onDragStart={isRootDrive ? undefined : (e) => {
                        e.preventDefault();
                        if (onDragStart) {
                            onDragStart('left', [{ path: node.path, name: node.name, is_dir: true } as any]);
                        }
                    }}
                    data-tooltip={node.driveType ? getDriveTooltip({
                        path: node.path,
                        label: node.label,
                        drive_type: node.driveType,
                        remote_path: node.remotePath
                    } as any, t) : (node.isTrash || node.isNetworkRoot ? node.name : node.path)}
                    data-tooltip-total={node.driveType && shouldShowDriveCapacity({ drive_type: node.driveType } as any) ? node.totalBytes : undefined}
                    data-tooltip-free={node.driveType && shouldShowDriveCapacity({ drive_type: node.driveType } as any) ? node.freeBytes : undefined}
                >
                    <div
                        className={cx('tree-chevron', { invisible: !hasEffectiveChildren })}
                        onClick={hasEffectiveChildren ? (e) => {
                            e.stopPropagation();
                            toggleExpand(e, node);
                        } : undefined}
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
        <div 
            className={cx("directory-tree", { "is-loading": isExpanding })}
            onKeyDown={handleKeyDown}
            tabIndex={0}
            style={{ outline: 'none' }}
        >
            <AutoSizer renderProp={({ height, width }: any) => {
                if (height === undefined || width === undefined) return null;
                return (
                    <List
                        listRef={listRef}
                        rowCount={visibleNodes.length}
                        rowHeight={(index: number) => {
                            const vn = visibleNodes[index];
                            if (vn?.node.isSpacer) return 13;
                            if (vn?.node.driveType || vn?.node.isTrash || vn?.node.isNetworkRoot) return 34;
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
                    onDisconnectDrive={(path) => {
                        const cleanPath = path.replace(/[\\/]+$/, '');
                        onDisconnectDrive?.(cleanPath);
                        setContextMenu(null);
                    }}
                    onOpenNewTab={(path) => { onOpenNewTab?.(path); setContextMenu(null); }}
                    isDir={true}
                    isFavorite={contextMenu.isFavorite}
                    onAddToFavorites={() => { onAddToFavorites?.(contextMenu.path); setContextMenu(null); }}
                    onRemoveFromFavorites={() => { onRemoveFromFavorites?.(contextMenu.path); setContextMenu(null); }}
                    isNetworkComputer={contextMenu.isNetworkComputer}
                    isTrashContext={contextMenu.isTrash}
                    onOpenFile={(path) => { onNavigate(path); setContextMenu(null); }}
                    onEmptyTrash={() => { onEmptyTrash?.(); setContextMenu(null); }}
                    onRestoreAll={() => { onRestoreAll?.(); setContextMenu(null); }}
                    onTrashProperties={() => { onTrashProperties?.(); setContextMenu(null); }}
                />
            )}
        </div>
    );
});
