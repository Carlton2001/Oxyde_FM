import React, { useRef, useEffect } from 'react';
import { TFunc } from '../../i18n';
import { FilterMenuCheckbox } from './FilterMenuCheckbox';
import './FilterMenu.css';

import { SIZE_CATEGORIES, SizeCategoryKey } from '../../types';

export const getSizeCategoryForFile = (size: number): SizeCategoryKey => {
    if (size === 0) return 'empty';
    if (size <= 10 * 1024) return 'tiny';
    if (size <= 1024 * 1024) return 'small';
    if (size <= 100 * 1024 * 1024) return 'medium';
    if (size <= 1024 * 1024 * 1024) return 'large';
    return 'huge';
};

interface SizeFilterMenuProps {
    x: number;
    y: number;
    selectedSizes: Set<SizeCategoryKey> | null;
    availableSizeCategories: Map<SizeCategoryKey, number>;
    onChange: (sizes: Set<SizeCategoryKey> | null) => void;
    onClose: () => void;
    t: TFunc;
}

export const SizeFilterMenu: React.FC<SizeFilterMenuProps> = ({
    x, y, selectedSizes, availableSizeCategories, onChange, onClose, t
}) => {
    const menuRef = useRef<HTMLDivElement>(null);
    const categoryKeys = (Object.keys(SIZE_CATEGORIES) as SizeCategoryKey[]).filter(cat => availableSizeCategories.has(cat));

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

    const handleToggle = (cat: SizeCategoryKey) => {
        if (selectedSizes === null) {
            const newSet = new Set(categoryKeys);
            newSet.delete(cat);
            onChange(newSet.size === 0 ? new Set() : newSet);
        } else {
            const newSet = new Set(selectedSizes);
            if (newSet.has(cat)) {
                newSet.delete(cat);
            } else {
                newSet.add(cat);
            }
            if (newSet.size === categoryKeys.length) {
                onChange(null); // All selected = no filter
            } else {
                onChange(newSet); // Includes empty Set = "Aucun"
            }
        }
    };

    const handleSelectAll = () => onChange(null);
    const handleClearAll = () => onChange(new Set());

    return (
        <div className="filter-menu-wrapper">
            <div
                ref={menuRef}
                className="context-menu filter-menu-inner"
                style={{ left: `${position.left}px`, top: `${position.top}px`, minWidth: '200px' }}
            >
                <div className="filter-menu-title">
                    {t('filter_by_size' as any) || 'Filter by size'}
                </div>

                <div className="filter-menu-list">
                    {categoryKeys.map(cat => {
                        const info = SIZE_CATEGORIES[cat];
                        const isChecked = selectedSizes === null || selectedSizes.has(cat);
                        const label = t(info.key as any) || cat;
                        const count = availableSizeCategories.get(cat);
                        return (
                            <FilterMenuCheckbox
                                key={cat}
                                checked={isChecked}
                                label={label}
                                count={count}
                                onClick={() => handleToggle(cat)}
                            />
                        );
                    })}
                </div>
                <div className="filter-menu-footer">
                    {selectedSizes === null ? (
                        <button className="btn ghost" onClick={handleClearAll}>{t('none' as any) || 'None'}</button>
                    ) : (
                        <button className="btn ghost" onClick={handleSelectAll}>{t('all' as any) || 'All'}</button>
                    )}
                </div>
            </div>
        </div>
    );
};
