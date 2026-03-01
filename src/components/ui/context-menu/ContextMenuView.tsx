
import React, { useState, useRef, useEffect, useLayoutEffect } from 'react';
import { createPortal } from 'react-dom';
import { ChevronRight, Loader2 } from 'lucide-react';
import cx from 'classnames';
import { MenuItem } from './definitions';
import { invoke } from '@tauri-apps/api/core';
import { getNativeIcon } from '../../../utils/iconMapper';
import '../ContextMenu.css'; // We'll reuse existing CSS for now or update it

// --- SubComponents ---

interface ContextMenuItemProps {
    item: MenuItem;
    onClose: () => void;
    parentRef?: React.RefObject<HTMLDivElement>;
}

const ContextMenuItemRow: React.FC<ContextMenuItemProps> = ({ item, onClose }) => {
    const [isSubmenuOpen, setSubmenuOpen] = useState(false);
    const itemRef = useRef<HTMLDivElement>(null);
    const submenuRef = useRef<HTMLDivElement>(null);
    const [submenuPos, setSubmenuPos] = useState<{ top: number; flip: boolean; adjusted: boolean }>({ top: 0, flip: false, adjusted: false });

    // Calculate submenu position
    useLayoutEffect(() => {
        if (isSubmenuOpen && itemRef.current && submenuRef.current) {
            const rect = itemRef.current.getBoundingClientRect();
            const submenuRect = submenuRef.current.getBoundingClientRect();
            const screenH = window.innerHeight;
            const screenW = window.innerWidth;
            const margin = 10;

            // Horizontal flip
            const flip = rect.right + submenuRect.width > screenW - margin;

            // Vertical adjustment
            let topOffset = 0;
            const bottomPos = rect.top + submenuRect.height;

            if (bottomPos > screenH - margin) {
                // Shift up to stay on screen
                topOffset = (screenH - margin) - bottomPos;

                // If shifting up makes it go off the top, clamp to margin
                if (rect.top + topOffset < margin) {
                    topOffset = margin - rect.top;
                }
            }

            setSubmenuPos({ top: topOffset, flip, adjusted: true });
        } else if (!isSubmenuOpen) {
            setSubmenuPos(prev => ({ ...prev, adjusted: false }));
        }
    }, [isSubmenuOpen]);

    const isSubmenu = item.type === 'submenu';
    const isSplit = isSubmenu && !!item.action;

    const handleMouseEnter = () => {
        if ((isSubmenu && !isSplit) || item.type === 'native_menu') {
            setSubmenuOpen(true);
        }
    };

    const handleMouseLeave = () => {
        setSubmenuOpen(false);
    };

    const handleClick = (e: React.MouseEvent) => {
        e.stopPropagation();
        if (item.action) {
            item.action(); // Context already bound
            onClose();
        }
    };

    if (item.type === 'separator') {
        return <div className="menu-separator" />;
    }

    if (item.type === 'native_menu') {
        return (
            <NativeMenuItemRow
                item={item}
                onClose={onClose}
            />
        );
    }

    return (
        <div
            ref={itemRef}
            className={cx({ "menu-item": !isSubmenu, "menu-item-with-submenu": isSubmenu, "danger": item.danger })}
            onClick={isSplit ? undefined : handleClick}
            onMouseEnter={handleMouseEnter}
            onMouseLeave={handleMouseLeave}
            onMouseDown={(e) => e.preventDefault()}
        >
            {isSubmenu ? (
                <>
                    <div className="menu-item-main"
                        onClick={isSplit ? handleClick : undefined}
                        onMouseEnter={isSplit ? () => setSubmenuOpen(false) : undefined}>
                        {item.icon && <item.icon className="icon-md" />}
                        <span className="win-item-label">{item.label}</span>
                    </div>
                    {isSplit && <div className="submenu-divider" />}
                    <div className="menu-item-arrow"
                        onMouseEnter={isSplit ? () => setSubmenuOpen(true) : undefined}>
                        <ChevronRight className="icon-sm" />
                        {isSubmenuOpen && item.children && (
                            <div
                                ref={submenuRef}
                                className={cx("submenu", { "flip-left": submenuPos.flip })}
                                style={{
                                    top: submenuPos.top,
                                    visibility: submenuPos.adjusted ? 'visible' : 'hidden'
                                }}
                            >
                                {item.children.map((child: MenuItem) => (
                                    <ContextMenuItemRow key={child.id} item={child} onClose={onClose} />
                                ))}
                            </div>
                        )}
                    </div>
                </>
            ) : (
                <>
                    {item.icon && <item.icon className="icon-md" />}
                    <span className="win-item-label">{item.label}</span>
                </>
            )}
        </div>
    );
};


// --- Native Menu Specifics ---

interface WinMenuItem {
    id: number;
    label: string;
    verb?: string;
    has_submenu: boolean;
    children: WinMenuItem[];
}

const NativeMenuItemRow: React.FC<{ item: MenuItem; onClose: () => void }> = ({ item, onClose }) => {
    // We need the path to fetch items. It's not in MenuItem directly, but we can bind it?
    // Actually, simple hack: We attached the path to a custom property in definitions?
    // Or we expect `item.action` to NOT be the action but a provider?
    // Re-visiting definitions.ts -> onNativeMenu was empty.

    // Let's grab context from somewhere? No.
    // I should have passed the Path in the MenuItem definition for native_menu.
    // I will update definitions.ts next turn to include `data` field or similar.
    // FOR NOW, I will assume we can't implement it perfectly without updating definition.
    // But wait, I am the one writing this file right now.

    // I will assume `item` has a `data` prop containing `{ paths: string[] }` or similar.
    // I'll update definitions.ts afterwards to match.

    const [isOpen, setIsOpen] = useState(false);
    const [items, setItems] = useState<WinMenuItem[]>([]);
    const [loading, setLoading] = useState(false);
    const [pos, setPos] = useState({ flip: false, top: 0, heightAdjusted: false });
    const rowRef = useRef<HTMLDivElement>(null);
    const submenuRef = useRef<HTMLDivElement>(null);

    // Fetch on hover
    useEffect(() => {
        if (isOpen && items.length === 0 && !loading) {
            setLoading(true);
            const isBackground = (item as any).data?.isBackground ?? false;
            const path = (item as any).data?.target;
            if (path) {
                invoke<WinMenuItem[]>('get_native_context_menu_items', { path, isBackground })
                    .then(res => {
                        setItems(res);
                        setLoading(false);
                    })
                    .catch(() => setLoading(false));
            }
        }
    }, [isOpen, items.length, loading, item]);

    // Position logic - robust check against window edges
    useLayoutEffect(() => {
        if (isOpen && rowRef.current && submenuRef.current) {
            const rowRect = rowRef.current.getBoundingClientRect();
            const submenuRect = submenuRef.current.getBoundingClientRect();

            // Horizontal Flip
            const flip = rowRect.right + submenuRect.width > window.innerWidth;

            // Vertical Adjustment
            let topOffset = 0;
            const screenH = window.innerHeight;
            const margin = 10;

            // If dragging bottom goes off screen
            const bottomPos = rowRect.top + submenuRect.height;
            if (bottomPos > screenH - margin) {
                // Shift up
                topOffset = (screenH - margin) - bottomPos;

                // If shifting up makes top go off screen, clamp it
                if (rowRect.top + topOffset < margin) {
                    topOffset = margin - rowRect.top;
                }
            }

            setPos({
                flip,
                top: topOffset,
                heightAdjusted: true
            });
        }
    }, [isOpen, items, loading]);

    const handleExecute = async (id: number) => {
        const path = (item as any).data?.target;
        const isBackground = (item as any).data?.isBackground ?? false;
        if (path) {
            await invoke('execute_native_menu_item', { path, id, isBackground });
            onClose();
        }
    };

    return (
        <div
            ref={rowRef}
            className="menu-item-with-submenu"
            onMouseEnter={() => setIsOpen(true)}
            onMouseLeave={() => setIsOpen(false)}
            onMouseDown={(e) => e.preventDefault()}
        >
            <div className="menu-item-main">
                {item.icon && <item.icon className="icon-md" />}
                <span className="win-item-label">{item.label}</span>
            </div>
            <div className="menu-item-arrow">
                <ChevronRight className="icon-sm" />
                {isOpen && (
                    <div
                        ref={submenuRef}
                        className={cx("submenu", { "flip-left": pos.flip })}
                        style={{
                            width: '17.5rem',
                            minHeight: '3.125rem',
                            top: pos.top,
                            visibility: pos.heightAdjusted ? 'visible' : 'hidden' // Avoid flash before positioning
                        }}
                    >
                        {loading && (
                            <div className="menu-item disabled">
                                <Loader2 className="icon-md animate-spin" /> Loading...
                            </div>
                        )}
                        {!loading && items.map((wItem, idx) => (
                            <RecursiveWinItem key={`${wItem.id}-${idx}`} item={wItem} onExecute={handleExecute} />
                        ))}
                    </div>
                )}
            </div>
        </div>
    );
};

const RecursiveWinItem: React.FC<{ item: WinMenuItem, onExecute: (id: number) => void }> = ({ item, onExecute }) => {
    const [isOpen, setIsOpen] = useState(false);
    const [pos, setPos] = useState({ flip: false, top: 0, heightAdjusted: false });
    const rowRef = useRef<HTMLDivElement>(null);
    const submenuRef = useRef<HTMLDivElement>(null);

    useLayoutEffect(() => {
        if (isOpen && rowRef.current && submenuRef.current) {
            const rowRect = rowRef.current.getBoundingClientRect();
            const submenuRect = submenuRef.current.getBoundingClientRect();

            // Horizontal Flip
            const flip = rowRect.right + submenuRect.width > window.innerWidth;

            // Vertical Adjustment
            let topOffset = 0;
            const screenH = window.innerHeight;
            const margin = 10;

            // If dragging bottom goes off screen
            const bottomPos = rowRect.top + submenuRect.height;
            if (bottomPos > screenH - margin) {
                // Shift up
                topOffset = (screenH - margin) - bottomPos;

                // If shifting up makes top go off screen, clamp it
                if (rowRect.top + topOffset < margin) {
                    topOffset = margin - rowRect.top;
                }
            }

            setPos({
                flip,
                top: topOffset,
                heightAdjusted: true
            });
        }
    }, [isOpen]);

    if (item.has_submenu) {
        const Icon = getNativeIcon(item.verb, item.label);
        return (
            <div
                ref={rowRef}
                className="menu-item-with-submenu"
                onMouseEnter={() => setIsOpen(true)}
                onMouseLeave={() => setIsOpen(false)}
                onMouseDown={(e) => e.preventDefault()}
            >
                <div className="menu-item-main">
                    {Icon ? <Icon className="icon-md" /> : <div className="icon-md" />}
                    <span className="win-item-label">{item.label}</span>
                </div>
                <div className="menu-item-arrow">
                    <ChevronRight className="icon-sm" />
                    {isOpen && (
                        <div
                            ref={submenuRef}
                            className={cx("submenu", { "flip-left": pos.flip })}
                            style={{
                                top: pos.top,
                                visibility: pos.heightAdjusted ? 'visible' : 'hidden'
                            }}
                        >
                            {item.children.map((child, idx) => (
                                <RecursiveWinItem key={`${child.id}-${idx}`} item={child} onExecute={onExecute} />
                            ))}
                        </div>
                    )}
                </div>
            </div>
        );
    }

    const Icon = getNativeIcon(item.verb, item.label);

    return (
        <div className="menu-item" onClick={(e) => { e.stopPropagation(); onExecute(item.id); }} onMouseDown={(e) => e.preventDefault()}>
            {Icon ? <Icon className="icon-md" /> : <div className="icon-md" />}
            <span className="win-item-label">{item.label}</span>
        </div>
    );
}


// --- Main View ---

interface ContextMenuViewProps {
    items: MenuItem[];
    x: number;
    y: number;
    onClose: () => void;
}

export const ContextMenuView: React.FC<ContextMenuViewProps> = ({ items, x, y, onClose }) => {
    const menuRef = useRef<HTMLDivElement>(null);
    const [pos, setPos] = useState({ x, y });
    const [visible, setVisible] = useState(false);

    // Initial positioning to stay on screen
    useLayoutEffect(() => {
        if (menuRef.current) {
            const rect = menuRef.current.getBoundingClientRect();
            let newX = x;
            let newY = y;
            const pad = 10;

            if (x + rect.width > window.innerWidth - pad) {
                newX = window.innerWidth - rect.width - pad;
            }
            if (y + rect.height > window.innerHeight - pad) {
                newY = window.innerHeight - rect.height - pad;
            }

            setPos({ x: Math.max(pad, newX), y: Math.max(pad, newY) });
            setVisible(true);
        }
    }, [x, y]);

    // Close on outside click
    useEffect(() => {
        const handleDown = (e: MouseEvent) => {
            if (menuRef.current && !menuRef.current.contains(e.target as Node)) {
                onClose();
            }
        };
        document.addEventListener('mousedown', handleDown, true);
        return () => document.removeEventListener('mousedown', handleDown, true);
    }, [onClose]);

    return createPortal(
        <div
            ref={menuRef}
            className="context-menu"
            style={{
                top: pos.y,
                left: pos.x,
                opacity: visible ? 1 : 0,
                transition: 'opacity 0.1s'
            }}
            onClick={(e) => e.stopPropagation()}
            onContextMenu={(e) => e.preventDefault()}
            onMouseDown={(e) => e.preventDefault()}
        >
            {items.map((item, i) => (
                <ContextMenuItemRow key={item.id || i} item={item} onClose={onClose} />
            ))}
        </div>,
        document.body
    );
};

