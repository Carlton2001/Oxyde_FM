
import React, { useRef } from 'react';
import { v4 as uuidv4 } from 'uuid';
import cx from 'classnames';
import { X, Plus, Folder, Copy, Split, XCircle, ChevronLeft, ChevronRight, HardDrive, Trash, Network, Globe } from 'lucide-react';
import { useTabs } from '../../context/TabsContext';
import { useApp } from '../../context/AppContext';
import { SearchBox } from './SearchBox';
import './Tabs.css';

import { FileEntry } from '../../types';

interface TabsProps {
    onSwitch: (tabId: string, path?: string) => void;
    onClose: (tabId: string) => void;
    isDraggingFiles?: boolean;
    dragState?: { sourcePanel: 'left' | 'right'; files: FileEntry[] } | null;
    onTabDrop?: (files: FileEntry[], index?: number) => void;
    searchQuery: string;
    onSearchChange: (query: string) => void;
    onSearchSubmit: () => void;
    onSearchClear: () => void;
    isSearching: boolean;
}

export const Tabs: React.FC<TabsProps> = ({
    onSwitch, onClose, isDraggingFiles, dragState, onTabDrop,
    searchQuery, onSearchChange, onSearchSubmit, onSearchClear, isSearching
}) => {
    const { tabs, activeTabId, addTab, duplicateTab, closeOtherTabs, reorderTabs } = useTabs();
    const { t } = useApp();
    const wrapperRef = useRef<HTMLDivElement>(null);
    const scrollRef = useRef<HTMLDivElement>(null);
    const isLockRef = useRef(false);

    const [showLeftScroll, setShowLeftScroll] = React.useState(false);
    const [showRightScroll, setShowRightScroll] = React.useState(false);

    // Filter to ensure we only react to folders
    const isDraggingFolders = isDraggingFiles && dragState?.files.some(f => f.is_dir);

    // File Drag Hover State
    const [fileHoverTabId, setFileHoverTabId] = React.useState<string | null>(null);
    const [fileDropIndex, setFileDropIndex] = React.useState<number | null>(null);
    const fileSwitchTimeoutRef = useRef<ReturnType<typeof setTimeout> | null>(null);

    React.useEffect(() => {
        if (!fileHoverTabId || !isDraggingFiles) {
            if (fileSwitchTimeoutRef.current) clearTimeout(fileSwitchTimeoutRef.current);
            return;
        }

        fileSwitchTimeoutRef.current = setTimeout(() => {
            onSwitch(fileHoverTabId);
        }, 700);

        return () => {
            if (fileSwitchTimeoutRef.current) clearTimeout(fileSwitchTimeoutRef.current);
        };
    }, [fileHoverTabId, isDraggingFiles, onSwitch]);

    React.useEffect(() => {
        if (!isDraggingFiles) {
            setFileHoverTabId(null);
        }
    }, [isDraggingFiles]);

    React.useEffect(() => {
        if (!isDraggingFolders) {
            setFileDropIndex(null);
        }
    }, [isDraggingFolders]);

    // Mouse Drag & Reorder State
    const [draggingId, setDraggingId] = React.useState<string | null>(null);
    const [dragPos, setDragPos] = React.useState({ x: 0, y: 0 });
    const [dropTargetId, setDropTargetId] = React.useState<string | null>(null);
    const [isOutOfBounds, setIsOutOfBounds] = React.useState(false);

    // Track initial click to enforce drag threshold
    const [mouseDownInfo, setMouseDownInfo] = React.useState<{ id: string, x: number, y: number } | null>(null);

    // Handle global mouse events for dragging
    React.useEffect(() => {
        const handleGlobalMouseMove = (e: MouseEvent) => {
            if (draggingId) {
                setDragPos({ x: e.clientX, y: e.clientY });

                if (wrapperRef.current) {
                    const rect = wrapperRef.current.getBoundingClientRect();
                    const threshold = 40;
                    const isOut =
                        e.clientY < rect.top - threshold ||
                        e.clientY > rect.bottom + threshold ||
                        e.clientX < rect.left - threshold ||
                        e.clientX > rect.right + threshold;

                    if (isOut !== isOutOfBounds) {
                        setIsOutOfBounds(isOut);
                        if (isOut) setDropTargetId(null);
                    }
                }
            } else if (mouseDownInfo) {
                const dx = e.clientX - mouseDownInfo.x;
                const dy = e.clientY - mouseDownInfo.y;
                const dist = Math.sqrt(dx * dx + dy * dy);

                if (dist > 10) {
                    setDraggingId(mouseDownInfo.id);
                    setDragPos({ x: e.clientX, y: e.clientY });
                    setDropTargetId(mouseDownInfo.id);
                    setMouseDownInfo(null);
                }
            }
        };

        const handleGlobalMouseUp = () => {
            if (draggingId && dropTargetId && draggingId !== dropTargetId && !isOutOfBounds) {
                const sourceIndex = tabs.findIndex(t => t.id === draggingId);
                const targetIndex = tabs.findIndex(t => t.id === dropTargetId);

                if (sourceIndex !== -1 && targetIndex !== -1) {
                    reorderTabs(sourceIndex, targetIndex);
                }
            }
            setDraggingId(null);
            setDropTargetId(null);
            setIsOutOfBounds(false);
            setMouseDownInfo(null);
        };

        if (draggingId || mouseDownInfo) {
            window.addEventListener('mousemove', handleGlobalMouseMove);
            window.addEventListener('mouseup', handleGlobalMouseUp);
        }
        return () => {
            window.removeEventListener('mousemove', handleGlobalMouseMove);
            window.removeEventListener('mouseup', handleGlobalMouseUp);
        };
    }, [draggingId, dropTargetId, tabs, reorderTabs, isOutOfBounds, mouseDownInfo]);

    const onTabMouseDown = (e: React.MouseEvent, id: string) => {
        if (e.button === 0) {
            setMouseDownInfo({ id, x: e.clientX, y: e.clientY });
        } else if (e.button === 1) {
            // Prevent autoscroll
            e.preventDefault();
        }
    };

    const onTabMouseEnter = (targetId: string) => {
        if (draggingId) {
            setDropTargetId(targetId);
        } else if (isDraggingFiles) {
            setFileHoverTabId(targetId);
        }
    };

    const onTabMouseLeave = (id: string) => {
        if (fileHoverTabId === id) {
            setFileHoverTabId(null);
        }
    };

    const handleFileDragMove = (e: React.MouseEvent) => {
        if (!isDraggingFolders) return;

        if (wrapperRef.current) {
            // Simple proximity calculation
            // We iterate over tabs to find which gap we are closest to
            const tabElements = wrapperRef.current.querySelectorAll('.tab');
            let foundIndex = tabs.length;

            for (let i = 0; i < tabElements.length; i++) {
                const rect = tabElements[i].getBoundingClientRect();
                const mid = rect.left + rect.width / 2;
                if (e.clientX < mid) {
                    foundIndex = i;
                    break;
                }
            }
            setFileDropIndex(foundIndex);
        }
    };

    // Scroll Buttons Logic
    const checkScroll = () => {
        if (scrollRef.current) {
            const { scrollLeft, scrollWidth, clientWidth } = scrollRef.current;
            setShowLeftScroll(scrollLeft > 0);
            setShowRightScroll(scrollLeft < scrollWidth - clientWidth - 1);
        }
    };

    React.useEffect(() => {
        checkScroll();
        window.addEventListener('resize', checkScroll);
        return () => window.removeEventListener('resize', checkScroll);
    }, [tabs]);

    const handleScroll = (direction: 'left' | 'right') => {
        if (scrollRef.current) {
            const scrollAmount = 200;
            scrollRef.current.scrollBy({
                left: direction === 'left' ? -scrollAmount : scrollAmount,
                behavior: 'smooth'
            });
            setTimeout(checkScroll, 300);
        }
    };

    const handleWheel = React.useCallback((e: WheelEvent) => {
        if (e.deltaY !== 0) {
            e.preventDefault();
            if (scrollRef.current) {
                scrollRef.current.scrollLeft += e.deltaY;
                checkScroll();
            }
        }
    }, []);

    React.useEffect(() => {
        const wrapper = wrapperRef.current;
        if (!wrapper) return;
        wrapper.addEventListener('wheel', handleWheel, { passive: false });
        // Clean up
        return () => wrapper.removeEventListener('wheel', handleWheel);
    }, [handleWheel]);

    React.useEffect(() => {
        const activeTab = document.getElementById(`tab-${activeTabId}`);
        if (activeTab && scrollRef.current) {
            activeTab.scrollIntoView({ behavior: 'smooth', block: 'nearest', inline: 'nearest' });
        }
    }, [activeTabId]);

    const handleCreateTab = (id: string) => {
        addTab('C:\\', { id });
        setTimeout(() => {
            if (scrollRef.current) {
                scrollRef.current.scrollTo({ left: scrollRef.current.scrollWidth, behavior: 'smooth' });
            }
        }, 50);
    };


    // We need a local context menu for tabs.
    const [menu, setMenu] = React.useState<{ x: number, y: number, tabId: string } | null>(null);

    const onContextMenu = (e: React.MouseEvent, tabId: string) => {
        e.preventDefault();
        e.stopPropagation();
        setMenu({ x: e.clientX, y: e.clientY, tabId });
    };

    React.useEffect(() => {
        const closeMenu = (e: MouseEvent) => {
            const menuEl = document.querySelector('.tab-context-menu');
            if (menuEl && menuEl.contains(e.target as Node)) return;
            setMenu(null);
        };
        window.addEventListener('mousedown', closeMenu, true);
        window.addEventListener('contextmenu', closeMenu, true);
        return () => {
            window.removeEventListener('mousedown', closeMenu, true);
            window.removeEventListener('contextmenu', closeMenu, true);
        };
    }, []);

    const getTabIcon = (path: string) => {
        if (path === 'trash://') return <Trash size={14} />;
        if (path === '__network_vincinity__') return <Globe size={14} />;
        if (path.startsWith('\\\\')) {
            const parts = path.split('\\').filter(Boolean);
            if (parts.length <= 2) {
                return <Network size={14} />;
            }
        }
        if (/^[a-zA-Z]:\\$/.test(path)) return <HardDrive size={14} />;
        // Default folder icon
        return <Folder size={14} />;
    };

    const getTabLabel = (tab: { label: string, path: string }) => {
        if (tab.path === 'trash://') return t('recycle_bin');
        if (tab.path === '__network_vincinity__') return t('network_vincinity');

        if (tab.path.startsWith('search://')) {
            const searchPart = tab.path.replace('search://', '');
            const querySepIndex = searchPart.indexOf('?');
            const query = decodeURIComponent(querySepIndex !== -1 ? searchPart.substring(0, querySepIndex) : searchPart);

            const params = new URLSearchParams(querySepIndex !== -1 ? searchPart.substring(querySepIndex + 1) : '');
            const root = params.get('root');
            const folderName = root ? (root.split('\\').filter(Boolean).pop() || root) : '';

            const inLabel = t('in' as any) === 'in' ? 'dans' : t('in' as any); // Simple detection or fallback

            return root
                ? `${t('search')} "${query}" ${inLabel} ${folderName}`
                : `${t('search')}: ${query}`;
        }

        return tab.label;
    };

    return (
        <div
            className="tabs-wrapper"
            ref={wrapperRef}
            onMouseMove={handleFileDragMove}
            onMouseLeave={() => {
                if (isDraggingFolders) setFileDropIndex(null);
            }}
            onMouseUp={(e) => {
                // Handle File Drop (Simulated DnD)
                // Use isDraggingFolders check
                if (isDraggingFolders && dragState && onTabDrop) {
                    e.preventDefault();
                    // Pass the calculated drop index
                    const folders = dragState.files.filter(f => f.is_dir);
                    if (folders.length > 0) {
                        onTabDrop(folders, fileDropIndex !== null ? fileDropIndex : undefined);
                    }
                    setFileDropIndex(null);
                    return;
                }
            }}
        >
            {showLeftScroll && (
                <button className="scroll-btn left" onClick={() => handleScroll('left')}>
                    <ChevronLeft size={16} />
                </button>
            )}

            <div
                className="tabs-container"
                ref={scrollRef}
                onScroll={checkScroll}
                onMouseDown={(e) => {
                    if (e.button === 1) {
                        e.preventDefault();
                    }
                }}
                onMouseUp={(e) => {
                    // Middle click on empty space creates new tab
                    if (e.button === 1 && e.target === e.currentTarget) {
                        e.preventDefault();
                        const id = uuidv4();
                        handleCreateTab(id);
                        onSwitch(id, 'C:\\');
                    }
                }}
            >
                {tabs.map((tab, index) => (
                    <div
                        id={`tab-${tab.id}`}
                        key={tab.id}
                        className={cx("tab", {
                            active: tab.id === activeTabId,
                            dragging: draggingId === tab.id
                        })}
                        onMouseDown={(e) => onTabMouseDown(e, tab.id)}
                        onMouseEnter={() => onTabMouseEnter(tab.id)}
                        onMouseLeave={() => onTabMouseLeave(tab.id)}
                        onClick={() => onSwitch(tab.id)}
                        onMouseUp={(e) => {
                            if (e.button === 1) { // Middle click
                                e.preventDefault();
                                e.stopPropagation();

                                // Allow closing any tab - application logic in onClose usually handles last tab
                                // but for now we keep the safety check or let onClose decide.
                                // The user said "supprimer un onglet", so let's make it work without restriction here.

                                if (isLockRef.current) return;
                                isLockRef.current = true;
                                setTimeout(() => { isLockRef.current = false; }, 200);

                                onClose(tab.id);
                            }
                        }}
                        onContextMenu={(e) => onContextMenu(e, tab.id)}
                        data-tooltip={draggingId ? undefined : (() => {
                            if (tab.path === '__network_vincinity__') return t('network_vincinity');
                            if (!tab.path.startsWith('search://')) return tab.path;
                            const searchPart = tab.path.replace('search://', '');
                            const querySepIndex = searchPart.indexOf('?');
                            const query = decodeURIComponent(querySepIndex !== -1 ? searchPart.substring(0, querySepIndex) : searchPart);
                            const params = new URLSearchParams(querySepIndex !== -1 ? searchPart.substring(querySepIndex + 1) : '');
                            const root = params.get('root');
                            const inLabel = t('in' as any) === 'in' ? 'dans' : t('in' as any);
                            const searchLabel = t('search') || 'Rechercher';
                            return root
                                ? `${searchLabel} "${query}" ${inLabel} ${root}`
                                : `${searchLabel}: ${query}`;
                        })()}
                        data-tooltip-pos="bottom"
                    >
                        {dropTargetId === tab.id && draggingId !== tab.id && (
                            <div className="insertion-marker" style={{
                                left: tabs.findIndex(t => t.id === dropTargetId) > tabs.findIndex(t => t.id === draggingId!) ? '100%' : '0'
                            }} />
                        )}

                        {/* Only show if folders are dragging */}
                        {isDraggingFolders && fileDropIndex === index && (
                            <div className="insertion-marker" style={{ left: 0 }} />
                        )}
                        {isDraggingFolders && index === tabs.length - 1 && fileDropIndex === tabs.length && (
                            <div className="insertion-marker" style={{ left: '100%' }} />
                        )}

                        <div className="tab-icon">
                            {getTabIcon(tab.path)}
                        </div>
                        <span className="tab-label">{getTabLabel(tab)}</span>
                        <div
                            className={cx("tab-close", { disabled: tabs.length <= 1 })}
                            onClick={(e) => {
                                e.stopPropagation();
                                if (tabs.length > 1) {
                                    onClose(tab.id);
                                }
                            }}
                        >
                            <X size={12} />
                        </div>
                    </div>
                ))}
                <div className="new-tab-btn" onClick={() => {
                    const id = uuidv4();
                    handleCreateTab(id);
                    onSwitch(id, 'C:\\');
                }} data-tooltip={t('new_tab') || "New Tab"}>
                    <Plus size={16} />
                </div>
            </div>

            {draggingId && !isOutOfBounds && (
                <div className="tab-ghost" style={{ left: dragPos.x, top: dragPos.y }}>
                    <div className="tab-icon">
                        {getTabIcon(tabs.find(t => t.id === draggingId)?.path || '')}
                    </div>
                    <span>{getTabLabel(tabs.find(t => t.id === draggingId) || { label: '', path: '' })}</span>
                </div>
            )}


            {showRightScroll && (
                <button className="scroll-btn right" onClick={() => handleScroll('right')}>
                    <ChevronRight size={16} />
                </button>
            )}

            <div className="tab-search-container">
                <SearchBox
                    query={searchQuery}
                    placeholder={t('search') + "..."}
                    isSearching={isSearching}
                    onChange={onSearchChange}
                    onSubmit={onSearchSubmit}
                    onClear={onSearchClear}
                    clearTitle={t('clear') || 'Clear'}
                    searchTitle={t('search')}
                />
            </div>

            {menu && (
                <div
                    className="tab-context-menu"
                    style={{
                        left: menu.x,
                        top: menu.y,
                    }}
                    onClick={(e) => e.stopPropagation()}
                >
                    <div className="menu-item" onClick={() => { duplicateTab(menu.tabId); setMenu(null); }}>
                        <Copy size={14} /> {t('duplicate_tab' as any) || "Duplicate Tab"}
                    </div>
                    {tabs.length > 1 && (
                        <>
                            <div className="menu-item" onClick={() => {
                                if (menu.tabId !== activeTabId) {
                                    onSwitch(menu.tabId);
                                }
                                closeOtherTabs(menu.tabId);
                                setMenu(null);
                            }}>
                                <Split size={14} /> {t('close_other_tabs' as any) || "Close Other Tabs"}
                            </div>
                            <div className="menu-separator" />
                            <div className="menu-item" onClick={() => { onClose(menu.tabId); setMenu(null); }}>
                                <XCircle size={14} /> {t('close_tab' as any) || "Close Tab"}
                            </div>
                        </>
                    )}
                </div>
            )}
        </div>
    );
};

