import React, { useState, useEffect, useLayoutEffect, useRef, useMemo, useCallback } from 'react';
import { ChevronRight, Folder, Trash, HardDrive, Usb, Disc, Copy, Network, Globe, Pin } from 'lucide-react';
import { invoke } from '@tauri-apps/api/core';
import cx from 'classnames';
import './PathBar.css';
import { DriveInfo, FileEntry, DirResponse, QuickAccessItem, PanelId } from '../../types';
import { TFunc } from '../../i18n';
import { useApp } from '../../context/AppContext';
import { AsyncFileIcon } from '../ui/AsyncFileIcon';
import { isVirtualPath } from '../../utils/path';
import { SearchBox } from '../ui/SearchBox';

interface PathBarProps {
    path: string;
    onNavigate: (path: string) => void;
    className?: string;
    isDragging?: boolean;
    onDrop?: (path: string, e: React.MouseEvent) => void;
    drives?: DriveInfo[];
    showHidden?: boolean;
    panelId: PanelId;
    t?: TFunc;
    favorites?: QuickAccessItem[];
    forwardPath?: string | null;

    // Search integration
    searchQuery?: string;
    isSearchActive?: boolean;
    onSearchChange?: (q: string) => void;
    onSearchSubmit?: () => void;
    onSearchClear?: () => void;
    onSearchCancel?: () => void;
}

export const PathBar: React.FC<PathBarProps> = ({ 
    path, onNavigate, className, isDragging, onDrop, drives, showHidden = false, panelId, t, favorites, forwardPath,
    searchQuery, isSearchActive, onSearchChange, onSearchSubmit, onSearchClear, onSearchCancel
}) => {
    const { useSystemIcons } = useApp();
    
    const isFavorite = useMemo(() => {
        if (!favorites || !path) return false;
        const normPath = path.toLowerCase().replace(/[\\/]+$/, '');
        return favorites.some(f => f.path.toLowerCase().replace(/[\\/]+$/, '') === normPath);
    }, [path, favorites]);

    // Special handling for trash path
    const isTrashPath = path?.startsWith('trash://') || path?.startsWith('trash:\\\\');
    const isSearchPath = path?.startsWith('search://') || path?.startsWith('search:\\\\');
    const [isEditing, setIsEditing] = useState(false);
    const [inputPath, setInputPath] = useState(path);
    const [dragTarget, setDragTarget] = useState<string | null>(null);
    const inputRef = useRef<HTMLInputElement>(null);

    // Dropdown state
    const [menuOpen, setMenuOpen] = useState<{ path: string; x: number; y: number } | null>(null);
    const [contextMenuOpen, setContextMenuOpen] = useState<{ path: string; x: number; y: number } | null>(null);
    const [subDirs, setSubDirs] = useState<FileEntry[]>([]);
    const [loadingSubDirs, setLoadingSubDirs] = useState(false);
    const menuRef = useRef<HTMLDivElement>(null);
    const contextMenuRef = useRef<HTMLDivElement>(null);

    useEffect(() => {
        if (!isEditing) {
            if (path === '__network_vincinity__') {
                setInputPath(t ? t('network_vincinity' as any) : 'Network');
            } else if (isTrashPath) {
                setInputPath(t ? t('recycle_bin' as any) : 'Recycle Bin');
            } else if (isVirtualPath(path)) {
                // Strip root param for display in edit mode to avoid clutter
                setInputPath(path.split('?')[0]);
            } else {
                setInputPath(path);
            }
        }
    }, [path, isEditing, t, isTrashPath]);

    // Focus input when entering edit mode
    useEffect(() => {
        if (isEditing && inputRef.current) {
            inputRef.current.focus();
        }
    }, [isEditing]);

    // Close menu on outside click
    useEffect(() => {
        const handleClickOutside = (e: MouseEvent) => {
            if (menuRef.current && !menuRef.current.contains(e.target as Node)) {
                setMenuOpen(null);
            }
            if (contextMenuRef.current && !contextMenuRef.current.contains(e.target as Node)) {
                setContextMenuOpen(null);
            }
        };
        if (menuOpen || contextMenuOpen) {
            document.addEventListener('mousedown', handleClickOutside, true);
            document.addEventListener('contextmenu', handleClickOutside, true);
        }
        return () => {
            document.removeEventListener('mousedown', handleClickOutside, true);
            document.removeEventListener('contextmenu', handleClickOutside, true);
        };
    }, [menuOpen, contextMenuOpen]);

    useLayoutEffect(() => {
        if (contextMenuOpen && contextMenuRef.current) {
            const { width, height } = contextMenuRef.current.getBoundingClientRect();
            const PAD = 8;
            let left = contextMenuOpen.x - width / 2;
            left = Math.max(PAD, Math.min(left, window.innerWidth - width - PAD));
            let top = contextMenuOpen.y;
            if (top + height > window.innerHeight - PAD) top = Math.max(PAD, window.innerHeight - height - PAD);
            contextMenuRef.current.style.left = `${left}px`;
            contextMenuRef.current.style.top = `${top}px`;
            contextMenuRef.current.style.visibility = 'visible';
        }
    }, [contextMenuOpen]);

    const fetchSubDirectories = useCallback(async (folderPath: string) => {
        setLoadingSubDirs(true);
        try {
            // Updated to match the new backend signature: panel_id, path, etc.
            const response = await invoke<DirResponse>('list_dir', {
                panelId,
                path: folderPath,
                showHidden,
                forceRefresh: false
            });
            // Only folders
            setSubDirs(response.entries.filter((ent: FileEntry) => ent.is_dir));
        } catch (err) {
            console.error("Failed to load subdirectories for breadcrumb:", err);
            setSubDirs([]);
        } finally {
            setLoadingSubDirs(false);
        }
    }, [showHidden, panelId]);

    // Refresh menu if showHidden changes
    useEffect(() => {
        if (menuOpen) {
            fetchSubDirectories(menuOpen.path);
        }
    }, [showHidden, menuOpen?.path, fetchSubDirectories]);

    const handleSeparatorClick = useCallback(async (e: React.MouseEvent, folderPath: string) => {
        e.stopPropagation();
        setContextMenuOpen(null);

        // Toggle if same path
        if (menuOpen?.path === folderPath) {
            setMenuOpen(null);
            return;
        }

        const rect = (e.currentTarget as HTMLElement).getBoundingClientRect();
        setMenuOpen({
            path: folderPath,
            x: rect.left,
            y: rect.bottom + 4
        });

        await fetchSubDirectories(folderPath);
    }, [menuOpen, fetchSubDirectories]);
    
    const handleEditClick = useCallback((e: React.MouseEvent) => {
        // Only trigger if clicking exactly the container or non-interactive areas
        // (Children like segments and searchbox stop propagation)
        setMenuOpen(null);
        setContextMenuOpen(null);
        if (!isEditing && !isDragging) {
            setIsEditing(true);
        }
    }, [isEditing, isDragging, setMenuOpen, setContextMenuOpen]);

    const [isNavigating, setIsNavigating] = useState(false);

    const handleSubmit = async () => {
        if (isNavigating) return;
        
        // Use the ref directly to be sure we have the absolute latest value from the DOM,
        // avoiding race conditions with rapid keystrokes after a paste.
        const currentInputValue = inputRef.current?.value || inputPath;
        
        // Strip all quotes and perform a clean trim
        let trimmed = currentInputValue.replace(/"/g, '').trim();

        // Handle friendly name mappings back to technical paths
        const networkTerm = t ? t('network_vincinity' as any) : 'Network';
        const trashTerm = t ? t('recycle_bin' as any) : 'Recycle Bin';

        if (trimmed.toLowerCase() === networkTerm.toLowerCase()) {
            trimmed = '__network_vincinity__';
        } else if (trimmed.toLowerCase() === trashTerm.toLowerCase()) {
            trimmed = 'trash://';
        }

        // Quick exit if path hasn't changed (normalized comparison)
        const normalizedInput = trimmed.replace(/[\\/]+$/, '').toLowerCase();
        const normalizedCurrent = path.replace(/[\\/]+$/, '').toLowerCase();
        
        if (normalizedInput === normalizedCurrent && trimmed !== '__network_vincinity__' && !trimmed.startsWith('search://')) {
            setIsEditing(false);
            return;
        }

        // Allow special virtual paths and network server roots without listing validation
        const isServerRoot = trimmed.startsWith('\\\\') && trimmed.split('\\').filter(Boolean).length === 1;
        if (trimmed === '__network_vincinity__' || trimmed === 'trash://' || isServerRoot) {
            onNavigate(trimmed);
            setIsEditing(false);
            return;
        }

        // Allow search:// paths without validation
        if (trimmed.startsWith('search://')) {
            // Preserve the original root if the user just edited the query
            let finalPath = trimmed;
            if (path.startsWith('search://') && !trimmed.includes('?root=')) {
                const searchPart = path.replace('search://', '');
                const querySepIndex = searchPart.indexOf('?');
                if (querySepIndex !== -1) {
                    finalPath = `${trimmed}${searchPart.substring(querySepIndex)}`;
                }
            }
            onNavigate(finalPath);
            setIsEditing(false);
            return;
        }

        setIsNavigating(true);
        try {
            // Validate existence by listing
            await invoke('list_dir', { panelId, path: trimmed });
            onNavigate(trimmed);
            setIsEditing(false);
        } catch (e) {
            console.error("Path validation failed", e);
            // Visual feedback on the CONTAINER, not the input (which has no border)
            if (inputRef.current && inputRef.current.parentElement) {
                const container = inputRef.current.parentElement;

                container.style.borderColor = '#ef4444'; // Red
                container.style.boxShadow = '0 0 0 1px #ef4444'; // Red ring

                container.animate([
                    { transform: 'translateX(0)' },
                    { transform: 'translateX(-5px)' },
                    { transform: 'translateX(5px)' },
                    { transform: 'translateX(0)' }
                ], { duration: 200, iterations: 2 });

                // Revert after short delay
                setTimeout(() => {
                    if (container) {
                        container.style.borderColor = '';
                        container.style.boxShadow = '';
                    }
                    setInputPath(path); // Revert to valid path
                    setIsEditing(false);
                }, 1000);
            }
        } finally {
            setIsNavigating(false);
        }
    };

    const handleKeyDown = (e: React.KeyboardEvent) => {
        if (e.key === 'Enter') {
            e.preventDefault();
            handleSubmit();
        } else if (e.key === 'Escape') {
            setInputPath(path);
            setIsEditing(false);
        }
    };

    const breadcrumbs = useMemo(() => {
        if (isTrashPath) {
            return [{
                name: t ? t('recycle_bin' as any) : 'Recycle Bin',
                fullPath: 'trash://',
                isLast: true,
                isGhost: false,
                isTrash: true
            }];
        }

        if (path === '__network_vincinity__') {
            return [{
                name: t ? t('network_vincinity' as any) : 'Network Neighborhood',
                fullPath: '__network_vincinity__',
                isLast: true,
                isGhost: false,
                isNetwork: true
            }];
        }

        if (isSearchPath) {
            const searchPart = path.startsWith('search://')
                ? path.replace('search://', '')
                : path.replace('search:\\\\', '');
            const querySepIndex = searchPart.indexOf('?');
            const query = decodeURIComponent(querySepIndex !== -1 ? searchPart.substring(0, querySepIndex) : searchPart);

            const params = new URLSearchParams(querySepIndex !== -1 ? searchPart.substring(querySepIndex + 1) : '');
            const root = params.get('root');
            const folderName = root ? (root.split('\\').filter(Boolean).pop() || root) : '';

            const displayName = root
                ? `${t ? t('search' as any) : 'Search'} "${query}" ${t ? t('in' as any) : 'in'} ${folderName}`
                : `${t ? t('search' as any) : 'Search'}: ${query}`;

            return [{
                name: displayName,
                fullPath: path,
                isLast: true,
                isGhost: false,
                isSearch: true
            }];
        }

        let pathToParse = path;
        let isForwardPathConfigured = false;
        let currentPartsLen = 0;

        if (forwardPath && forwardPath.toLowerCase().startsWith(path.toLowerCase() + '\\')) {
            pathToParse = forwardPath;
            currentPartsLen = path.split('\\').filter(p => p.length > 0).length;
            isForwardPathConfigured = true;
        }

        const parts = pathToParse.split('\\').filter(p => p.length > 0);
        let currentPath = "";
        const isUNC = pathToParse.startsWith('\\\\');

        return parts.map((part, index) => {
            if (isUNC) {
                if (index === 0) {
                    currentPath = `\\\\${part}`;
                } else {
                    currentPath = `${currentPath}\\${part}`;
                }
            } else {
                if (index === 0) {
                    currentPath = part + "\\";
                } else {
                    currentPath = currentPath.endsWith('\\') ? currentPath + part : currentPath + "\\" + part;
                }
            }

            let name = part.replace(/[:\\]+$/, '');
            if (!isUNC && index === 0 && drives) {
                const drivePath = part.endsWith(":") ? part + "\\" : part;
                const drive = drives.find(d => d.path.toUpperCase().startsWith(drivePath.toUpperCase()));
                if (drive && drive.label) {
                    let label = drive.label;
                    if (label === 'Local Disk' && t) {
                        label = t('local_disk');
                    } else if (label === 'Removable Disk' && t) {
                        label = t('removable_disk');
                    } else if (label === 'CD Drive' && t) {
                        label = t('cd_drive');
                    }
                    name = `${label} (${name})`;
                }
            }

            const isDrive = !isUNC && index === 0 && part.includes(':');
            const isNetworkServer = isUNC && index === 0;

            const currentIndex = isForwardPathConfigured ? currentPartsLen - 1 : parts.length - 1;

            return {
                name,
                fullPath: currentPath,
                isLast: index === parts.length - 1,
                isGhost: index !== currentIndex,
                isTrash: false,
                isDrive,
                isNetwork: isUNC,
                isNetworkServer
            };
        });
    }, [path, forwardPath, drives, isTrashPath, t]);

    const scrollRef = useRef<HTMLDivElement>(null);
    const [overflowState, setOverflowState] = useState<'none' | 'left' | 'right' | 'both'>('none');

    const updateOverflow = useCallback(() => {
        if (!scrollRef.current) return;
        const { scrollWidth, clientWidth, scrollLeft } = scrollRef.current;
        if (scrollWidth <= clientWidth + 2) {
            setOverflowState('none');
            return;
        }

        const isAtLeft = scrollLeft <= 2;
        const isAtRight = scrollLeft + clientWidth >= scrollWidth - 2;

        if (isAtLeft && !isAtRight) setOverflowState('right');
        else if (!isAtLeft && isAtRight) setOverflowState('left');
        else if (!isAtLeft && !isAtRight) setOverflowState('both');
        else setOverflowState('none');
    }, []);

    const scrollToActiveItem = useCallback(() => {
        if (scrollRef.current) {
            requestAnimationFrame(() => {
                if (!scrollRef.current) return;

                const container = scrollRef.current;
                const activeEl = container.querySelector('.path-segment.active-segment') as HTMLElement;
                
                if (activeEl) {
                    const maxScrollLeft = container.scrollWidth - container.clientWidth;
                    
                    const containerRect = container.getBoundingClientRect();
                    const activeRect = activeEl.getBoundingClientRect();
                    
                    // Predict the position if we scroll all the way to the right
                    const scrollDiff = maxScrollLeft - container.scrollLeft;
                    const futureLeft = activeRect.left - scrollDiff;
                    const futureRight = activeRect.right - scrollDiff;
                    
                    // Does it stay fully visible if we stick right?
                    if (futureLeft >= containerRect.left && futureRight <= containerRect.right) {
                        container.scrollLeft = maxScrollLeft;
                    } else {
                        // Center it
                        const activeCenter = activeRect.left + activeRect.width / 2;
                        const containerCenter = containerRect.left + containerRect.width / 2;
                        container.scrollLeft += (activeCenter - containerCenter);
                    }
                } else {
                    container.scrollLeft = container.scrollWidth;
                }
                
                updateOverflow();
            });
        }
    }, [updateOverflow]);

    // Check overflow and scroll on path change, mode change and window resize
    useEffect(() => {
        scrollToActiveItem();
        
        // Immediate and delayed triggers to handle both DOM changes and CSS transitions (0.15s)
        const timer1 = setTimeout(scrollToActiveItem, 10);
        const timer2 = setTimeout(scrollToActiveItem, 200);
        const timer3 = setTimeout(scrollToActiveItem, 500); // Final safety check

        window.addEventListener('resize', scrollToActiveItem);
        
        // Also observe the element size itself, because NavBar actions appearing
        // will change the pathbar width without a window resize event.
        let observer: ResizeObserver | null = null;
        if (scrollRef.current) {
            observer = new ResizeObserver(() => {
                scrollToActiveItem();
                // Double check after a small delay to handle parent transitions
                setTimeout(scrollToActiveItem, 160);
            });
            observer.observe(scrollRef.current);
        }

        return () => {
            window.removeEventListener('resize', scrollToActiveItem);
            clearTimeout(timer1);
            clearTimeout(timer2);
            clearTimeout(timer3);
            if (observer) observer.disconnect();
        };
    }, [path, isEditing, scrollToActiveItem]);

    // Horizontal scroll with mouse wheel
    useEffect(() => {
        const el = scrollRef.current;
        if (!el) return;
        const handleWheel = (e: WheelEvent) => {
            if (e.deltaY !== 0) {
                e.preventDefault();
                // Sensible horizontal scrolling
                el.scrollLeft += e.deltaY * 1.5; 
                updateOverflow();
            }
        };
        el.addEventListener('wheel', handleWheel, { passive: false });
        return () => el.removeEventListener('wheel', handleWheel);
    }, [updateOverflow]);

    return (
        <div
            className={cx("path-breadcrumbs", className, { editing: isEditing })}
            onClick={handleEditClick}
        >

            {isEditing ? (
                    <input
                        ref={inputRef}
                        className="path-input"
                        value={inputPath}
                        onChange={(e) => setInputPath(e.target.value)}
                        onBlur={handleSubmit}
                        onKeyDown={handleKeyDown}
                        autoFocus
                    />
            ) : (
                <>
                    {isTrashPath ? (
                        <Trash size="1rem" className="path-icon header-icon" />
                    ) : path === '__network_vincinity__' ? (
                        <Globe size="1rem" className="path-icon header-icon" />
                    ) : path?.startsWith('\\\\') ? (
                        <Network size="1rem" className="path-icon header-icon" />
                    ) : (
                        (() => {
                            const driveInfo = drives?.find(d => path?.toLowerCase().startsWith(d.path.toLowerCase()));
                            const Icon = driveInfo
                                ? (driveInfo.drive_type === 'removable' ? Usb : (driveInfo.drive_type === 'cdrom' ? Disc : (driveInfo.drive_type === 'remote' ? Network : HardDrive)))
                                : HardDrive;
                            return <Icon size="1rem" className="path-icon header-icon" />;
                        })()
                    )}
                    <div className={cx("breadcrumb-list", `overflow-${overflowState}`)} ref={scrollRef} onScroll={updateOverflow}>
                        {breadcrumbs.map((crumb, i) => (
                            <React.Fragment key={i}>
                                <div
                                    className={cx("path-segment", { 
                                        "drop-target": isDragging && dragTarget === crumb.fullPath,
                                        "ghost": crumb.isGhost,
                                        "active-segment": !crumb.isGhost
                                    })}
                                    data-path={crumb.fullPath}
                                    onClick={(e) => {
                                        e.stopPropagation();
                                        setMenuOpen(null);
                                        setContextMenuOpen(null);
                                        onNavigate(crumb.fullPath);
                                    }}
                                    onMouseDown={(e) => e.stopPropagation()}
                                    onMouseEnter={() => isDragging && setDragTarget(crumb.fullPath)}
                                    onMouseLeave={() => setDragTarget(null)}
                                    onMouseUp={(e) => {
                                        e.stopPropagation();
                                        if (e.button !== 2 && isDragging && onDrop) {
                                            onDrop(crumb.fullPath, e);
                                            setDragTarget(null);
                                        }
                                    }}
                                    onContextMenu={(e) => {
                                        e.preventDefault();
                                        e.stopPropagation();
                                        setMenuOpen(null);
                                        const rect = (e.currentTarget as HTMLElement).getBoundingClientRect();
                                        setContextMenuOpen({
                                            path: crumb.fullPath,
                                            x: rect.left + rect.width / 2,
                                            y: rect.bottom + 4
                                        });
                                    }}
                                >
                                    {crumb.name}
                                </div>
                                {!crumb.isLast && (
                                    <div
                                        className={cx("path-separator", { active: menuOpen?.path === crumb.fullPath, ghost: crumb.isGhost })}
                                        onClick={(e) => handleSeparatorClick(e, crumb.fullPath)}
                                        onMouseDown={(e) => e.stopPropagation()}
                                        onMouseUp={(e) => e.stopPropagation()}
                                        onContextMenu={(e) => {
                                            e.preventDefault();
                                            e.stopPropagation();
                                        }}
                                    >
                                        <ChevronRight size="0.75rem" />
                                    </div>
                                )}
                            </React.Fragment>
                        ))}
                    </div>
                    <div
                        className="breadcrumb-spacer"
                    />
                    {isFavorite && (
                        <div className="path-favorite-icon" onClick={(e) => e.stopPropagation()}>
                            <Pin size="0.875rem" fill="var(--accent-color)" stroke="var(--accent-color)" style={{ transform: 'rotate(45deg)' }} />
                        </div>
                    )}
                    <div className="path-bar-search-container" onClick={(e) => e.stopPropagation()}>
                        <SearchBox
                            query={searchQuery || ''}
                            placeholder={(t ? t('search') : 'Search') + "..."}
                            isSearching={isSearchActive}
                            onChange={onSearchChange || (() => { })}
                            onSubmit={onSearchSubmit || (() => { })}
                            onClear={onSearchClear || (() => { })}
                            onCancel={onSearchCancel}
                            autoExpand={true}
                            className="path-bar-search-box"
                        />
                    </div>
                </>
            )}

            {menuOpen && (
                <div
                    ref={menuRef}
                    className="breadcrumb-menu"
                    style={{
                        position: 'fixed',
                        top: menuOpen.y,
                        left: menuOpen.x
                    }}
                >
                    {loadingSubDirs ? (
                        <div className="menu-loading">{t ? t('loading' as any) : 'Loading...'}</div> // Minimal loader
                    ) : subDirs.length === 0 ? (
                        <div className="menu-empty">{t ? t('no_subfolders' as any) : 'No subfolders'}</div>
                    ) : (
                        subDirs.map(dir => (
                            <div
                                key={dir.path}
                                className="menu-item"
                                onClick={(e) => {
                                    e.stopPropagation();
                                    onNavigate(dir.path);
                                    setMenuOpen(null);
                                }}
                            >
                                {useSystemIcons ? (
                                    <AsyncFileIcon path={dir.path} isDir={true} name={dir.name} size={16} className="system-icon-img" />
                                ) : (
                                    <Folder size="0.875rem" className="file-icon folder" fill="currentColor" fillOpacity={0.2} />
                                )}
                                <span>{dir.name}</span>
                            </div>
                        ))
                    )}
                </div>
            )}

            {contextMenuOpen && (
                <div
                    ref={contextMenuRef}
                    className="breadcrumb-menu"
                    style={{
                        position: 'fixed',
                        top: contextMenuOpen.y,
                        left: contextMenuOpen.x,
                        visibility: 'hidden'
                    }}
                >
                    <div
                        className="menu-item"
                        onClick={(e) => {
                            e.stopPropagation();
                            navigator.clipboard.writeText(contextMenuOpen.path);
                            setContextMenuOpen(null);
                        }}
                    >
                        <Copy size="0.875rem" className="file-icon" />
                        <span>{t ? t('copy_path' as any) : 'Copy path'}</span>
                    </div>
                </div>
            )}
        </div>
    );
};

