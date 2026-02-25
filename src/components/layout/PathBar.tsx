import React, { useState, useEffect, useRef, useMemo, useCallback } from 'react';
import { ChevronRight, Folder, Trash, HardDrive, Usb, Disc, Copy } from 'lucide-react';
import { invoke } from '@tauri-apps/api/core';
import { getCurrentWindow } from '@tauri-apps/api/window';
import cx from 'classnames';
import './PathBar.css';
import { DriveInfo, FileEntry, DirResponse } from '../../types';
import { TFunc } from '../../i18n';
import { useApp } from '../../context/AppContext';
import { AsyncFileIcon } from '../ui/AsyncFileIcon';

interface PathBarProps {
    path: string;
    onNavigate: (path: string) => void;
    className?: string;
    isDragging?: boolean;
    onDrop?: (path: string, e: React.MouseEvent) => void;
    drives?: DriveInfo[];
    showHidden?: boolean;
    panelId: string;
    t?: TFunc;
}

export const PathBar: React.FC<PathBarProps> = ({ path, onNavigate, className, isDragging, onDrop, drives, showHidden = false, panelId, t }) => {
    const { useSystemIcons } = useApp();
    // Special handling for trash path
    const isTrashPath = path?.startsWith('trash://');
    const isSearchPath = path?.startsWith('search://');
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
            if (path?.startsWith('search://')) {
                // Strip root param for display in edit mode to avoid clutter
                setInputPath(path.split('?')[0]);
            } else {
                setInputPath(path);
            }
        }
    }, [path, isEditing]);

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
            document.addEventListener('mousedown', handleClickOutside);
        }
        return () => document.removeEventListener('mousedown', handleClickOutside);
    }, [menuOpen, contextMenuOpen]);

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

    const handleSubmit = async () => {
        const trimmed = inputPath.trim();
        if (trimmed === path) {
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

                // Save original styles if needed, or just let React reset them on re-render?
                // Direct DOM manipulation is fine here for transient effect.
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
                    container.style.borderColor = '';
                    container.style.boxShadow = '';
                    setInputPath(path); // Revert to valid path
                    setIsEditing(false);
                }, 1000);
            }
        }
    };

    const handleKeyDown = (e: React.KeyboardEvent) => {
        if (e.key === 'Enter') {
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
                isTrash: true
            }];
        }

        if (isSearchPath) {
            const searchPart = path.replace('search://', '');
            const querySepIndex = searchPart.indexOf('?');
            const query = decodeURIComponent(querySepIndex !== -1 ? searchPart.substring(0, querySepIndex) : searchPart);

            const params = new URLSearchParams(querySepIndex !== -1 ? searchPart.substring(querySepIndex + 1) : '');
            const root = params.get('root');
            const folderName = root ? (root.split('\\').filter(Boolean).pop() || root) : '';

            const inLabel = t && t('in' as any) === 'in' ? 'dans' : (t ? t('in' as any) : 'in');
            const displayName = root
                ? `${t ? t('search' as any) : 'Search'} "${query}" ${inLabel} ${folderName}`
                : `${t ? t('search' as any) : 'Search'}: ${query}`;

            return [{
                name: displayName,
                fullPath: path,
                isLast: true,
                isSearch: true
            }];
        }

        const parts = path.split('\\').filter(p => p.length > 0);
        let currentPath = "";
        return parts.map((part, index) => {
            if (index === 0) {
                currentPath = part + "\\";
            } else {
                currentPath = currentPath.endsWith('\\') ? currentPath + part : currentPath + "\\" + part;
            }

            let name = part.replace(/[:\\]+$/, '');
            if (index === 0 && drives) {
                const drivePath = part.endsWith(":") ? part + "\\" : part;
                const drive = drives.find(d => d.path.toUpperCase().startsWith(drivePath.toUpperCase()));
                if (drive && drive.label) {
                    name = `${drive.label} (${name}:)`;
                } else if (index === 0 && part.includes(':')) {
                    name = name + ":";
                }
            } else if (index === 0 && part.includes(':')) {
                name = name + ":";
            }

            const isDrive = index === 0 && part.includes(':');

            return {
                name,
                fullPath: currentPath,
                isLast: index === parts.length - 1,
                isTrash: false,
                isDrive
            };
        });
    }, [path, drives, isTrashPath, t]);

    const scrollRef = useRef<HTMLDivElement>(null);
    const [hasOverflow, setHasOverflow] = useState(false);

    // Check overflow on path change and resize
    useEffect(() => {
        const checkOverflow = () => {
            if (scrollRef.current) {
                const { scrollWidth, clientWidth } = scrollRef.current;
                setHasOverflow(scrollWidth > clientWidth);
            }
        };

        checkOverflow();
        // Auto-scroll to end
        if (scrollRef.current) {
            scrollRef.current.scrollLeft = scrollRef.current.scrollWidth;
        }

        window.addEventListener('resize', checkOverflow);
        return () => window.removeEventListener('resize', checkOverflow);
    }, [path]);

    // Manual Drag handling
    const isDragRef = useRef(false);
    const downPosRef = useRef<{ x: number, y: number } | null>(null);

    const handleMouseDown = (e: React.MouseEvent) => {
        if (e.button === 0 && !isEditing) {
            isDragRef.current = false;
            downPosRef.current = { x: e.clientX, y: e.clientY };
        }
    };

    const handleMouseMove = async (e: React.MouseEvent) => {
        if (downPosRef.current && !isDragRef.current && !isDragging) {
            const dx = Math.abs(e.clientX - downPosRef.current.x);
            const dy = Math.abs(e.clientY - downPosRef.current.y);
            if (dx > 5 || dy > 5) {
                isDragRef.current = true;
                downPosRef.current = null;
                getCurrentWindow().startDragging();
            }
        }
    };

    const handleMouseUp = () => {
        downPosRef.current = null;
        isDragRef.current = false;
    };

    return (
        <div
            className={cx("path-breadcrumbs", className, { editing: isEditing })}
            onMouseDown={handleMouseDown}
            onMouseMove={handleMouseMove}
            onMouseUp={handleMouseUp}
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
                    ) : (
                        (() => {
                            const driveInfo = drives?.find(d => path?.toLowerCase().startsWith(d.path.toLowerCase()));
                            const Icon = driveInfo
                                ? (driveInfo.drive_type === 'removable' ? Usb : (driveInfo.drive_type === 'cdrom' ? Disc : HardDrive))
                                : HardDrive;
                            return <Icon size="1rem" className="path-icon header-icon" />;
                        })()
                    )}
                    <div className={cx("breadcrumb-list", { overflowing: hasOverflow })} ref={scrollRef}>
                        {breadcrumbs.map((crumb, i) => (
                            <React.Fragment key={i}>
                                <div
                                    className={cx("path-segment", { "drop-target": isDragging && dragTarget === crumb.fullPath })}
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
                                        setContextMenuOpen({ path: crumb.fullPath, x: e.clientX, y: e.clientY });
                                    }}
                                >
                                    {crumb.name}
                                </div>
                                {!crumb.isLast && (
                                    <div
                                        className={cx("path-separator", { active: menuOpen?.path === crumb.fullPath })}
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
                        onClick={(e) => {
                            e.stopPropagation();
                            setMenuOpen(null);
                            setContextMenuOpen(null);
                            if (!isDragRef.current && !isEditing && !isDragging) {
                                setIsEditing(true);
                            }
                        }}
                    />
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
                        left: contextMenuOpen.x
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

