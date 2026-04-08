import React, { useRef, useEffect } from 'react';
import { TFunc } from '../../i18n';
import { FilterMenuCheckbox } from './FilterMenuCheckbox';
import './FilterMenu.css';

interface ExtensionFilterMenuProps {
    x: number;
    y: number;
    availableExtensions: string[];
    selectedExtensions: Set<string> | null;
    onChange: (exts: Set<string> | null) => void;
    onClose: () => void;
    t: TFunc;
}

export const ExtensionFilterMenu: React.FC<ExtensionFilterMenuProps> = ({
    x, y, availableExtensions, selectedExtensions, onChange, onClose, t
}) => {
    const menuRef = useRef<HTMLDivElement>(null);

    useEffect(() => {
        const handleClickOutside = (e: MouseEvent) => {
            if (menuRef.current && !menuRef.current.contains(e.target as Node)) {
                onClose();
            }
        };

        const handleEscape = (e: KeyboardEvent) => {
            if (e.key === 'Escape') onClose();
        };

        document.addEventListener('mousedown', handleClickOutside);
        document.addEventListener('keydown', handleEscape);
        return () => {
            document.removeEventListener('mousedown', handleClickOutside);
            document.removeEventListener('keydown', handleEscape);
        };
    }, [onClose]);

    // Position adjustment logic (similar to context menus)
    const [position, setPosition] = React.useState({ left: x, top: y });
    useEffect(() => {
        if (menuRef.current) {
            const rect = menuRef.current.getBoundingClientRect();
            let newX = x;
            let newY = y;
            if (x + rect.width > window.innerWidth) newX = window.innerWidth - rect.width - 5;
            if (y + rect.height > window.innerHeight) newY = window.innerHeight - rect.height - 5;
            setPosition({ left: newX, top: newY });
        }
    }, [x, y]);

    const handleToggle = (ext: string) => {
        if (selectedExtensions === null) {
            // First time filtering, uncheck the clicked one, keep all others
            const newSet = new Set(availableExtensions);
            newSet.delete(ext);
            // If deleting makes it empty, just restore null? Actually, if unchecking the only checked one, we empty it?
            onChange(newSet.size === 0 ? new Set() : newSet);
        } else {
            const newSet = new Set(selectedExtensions);
            if (newSet.has(ext)) {
                newSet.delete(ext);
            } else {
                newSet.add(ext);
            }
            if (newSet.size === availableExtensions.length) {
                onChange(null); // All selected = no filter
            } else {
                onChange(newSet); // Includes empty Set = "Aucun"
            }
        }
    };

    const handleSelectAll = () => onChange(null);

    const handleClearAll = () => onChange(new Set()); // This will hide all files

    return (
        <div className="filter-menu-wrapper">
            <div
                ref={menuRef}
                className="context-menu filter-menu-inner"
                style={{ left: `${position.left}px`, top: `${position.top}px`, maxHeight: '300px', minWidth: '150px' }}
            >
                <div className="filter-menu-title">
                    {t('filter_by_ext' as any) || 'Filter by extension'}
                </div>

                <div className="filter-menu-list scrollable">
                    {availableExtensions.map(ext => {
                        const isChecked = selectedExtensions === null || selectedExtensions.has(ext);
                        let displayExt = ext.toUpperCase();
                        if (ext === '') displayExt = `(${t('none_fem' as any) || 'None'})`;
                        if (ext === '__DIR__') displayExt = t('folder' as any) || 'Folder';
                        
                        return (
                            <FilterMenuCheckbox
                                key={ext}
                                checked={isChecked}
                                label={displayExt}
                                onClick={() => handleToggle(ext)}
                            />
                        );
                    })}
                </div>
                <div className="filter-menu-footer">
                    {selectedExtensions === null ? (
                        <button className="btn ghost" onClick={handleClearAll}>{t('none' as any) || 'None'}</button>
                    ) : (
                        <button className="btn ghost" onClick={handleSelectAll}>{t('all' as any) || 'All'}</button>
                    )}
                </div>
            </div>
        </div>
    );
};
