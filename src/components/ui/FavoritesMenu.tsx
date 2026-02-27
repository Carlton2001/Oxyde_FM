import React, { useRef, useEffect, useState } from 'react';
import { Star } from 'lucide-react';
import cx from 'classnames';
import { useFavorites } from '../../hooks/useFavorites';
import { useApp } from '../../context/AppContext';
import './FavoritesMenu.css';
import '../layout/PathBar.css';
import { createPortal } from 'react-dom';
import { Trash2 } from 'lucide-react';
import { ContextMenuView } from './context-menu/ContextMenuView';

interface FavoritesMenuProps {
    onNavigate: (path: string) => void;
    currentPath?: string;
    buttonClassName?: string;
}

export const FavoritesMenu: React.FC<FavoritesMenuProps> = ({ onNavigate, currentPath, buttonClassName = "drive-chip favorites-btn" }) => {
    const { t } = useApp();
    const [isOpen, setIsOpen] = useState(false);
    const { favorites, handleRemoveFavorite: removeFav } = useFavorites();
    const buttonRef = useRef<HTMLDivElement>(null);
    const menuRef = useRef<HTMLDivElement>(null);

    // Position state for portal
    const [position, setPosition] = useState({ top: 0, left: 0 });
    const [contextMenu, setContextMenu] = useState<{ x: number, y: number, path: string } | null>(null);

    useEffect(() => {
        const handleClickOutside = (event: MouseEvent) => {
            if (isOpen &&
                menuRef.current &&
                !menuRef.current.contains(event.target as Node) &&
                buttonRef.current &&
                !buttonRef.current.contains(event.target as Node) &&
                // Don't close if clicking inside context menu
                !(event.target as Element).closest('.context-menu')
            ) {
                setIsOpen(false);
                setContextMenu(null);
            }
        };

        document.addEventListener('mousedown', handleClickOutside);
        return () => document.removeEventListener('mousedown', handleClickOutside);
    }, [isOpen]);

    const handleToggle = (e: React.MouseEvent) => {
        e.stopPropagation();
        if (!isOpen && buttonRef.current) {
            const rect = buttonRef.current.getBoundingClientRect();
            setPosition({
                top: rect.bottom + 4,
                left: rect.left
            });
        }
        setIsOpen(!isOpen);
    };

    const handleSelect = (path: string, e: React.MouseEvent) => {
        e.stopPropagation();
        setIsOpen(false);
        setContextMenu(null);
        onNavigate(path);
    };

    const handleContextMenuRow = (path: string, e: React.MouseEvent) => {
        e.preventDefault();
        e.stopPropagation();
        setContextMenu({ x: e.clientX, y: e.clientY, path });
    };

    const contextMenuItems: any[] = contextMenu ? [
        {
            id: 'remove',
            type: 'action',
            label: t('remove_from_favorites' as any),
            icon: Trash2,
            action: async () => {
                await removeFav(contextMenu.path);
                setContextMenu(null);
            }
        }
    ] : [];

    return (
        <div className="favorites-menu-container">
            <div
                ref={buttonRef}
                className={cx(buttonClassName, { active: isOpen })}
                onClick={handleToggle}
                data-tooltip={t('favorites')}
                data-tooltip-pos="right"
            >
                <Star size="0.875rem" />
            </div>

            {isOpen && favorites.length > 0 && createPortal(
                <div
                    ref={menuRef}
                    className="breadcrumb-menu"
                    style={{ position: 'fixed', top: position.top, left: position.left }}
                    onContextMenu={(e) => {
                        e.preventDefault();
                        e.stopPropagation();
                    }}
                >
                    {favorites.map((fav) => (
                        <div
                            key={fav.path}
                            className={cx("menu-item", { active: currentPath === fav.path })}
                            onClick={(e) => handleSelect(fav.path, e)}
                            onContextMenu={(e) => handleContextMenuRow(fav.path, e)}
                        >
                            <Star size="0.875rem" className="file-icon folder" fill="currentColor" fillOpacity={0.2} />
                            <span className="fav-name">{fav.name}</span>
                        </div>
                    ))}
                </div>,
                document.body
            )}
            {contextMenu && (
                <ContextMenuView
                    items={contextMenuItems}
                    x={contextMenu.x}
                    y={contextMenu.y}
                    onClose={() => setContextMenu(null)}
                />
            )}
        </div>
    );
};
