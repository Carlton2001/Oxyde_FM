import React, { useRef, useEffect } from 'react';
import cx from 'classnames';
import { Check } from 'lucide-react';
import { TFunc } from '../../i18n';

// Definition of our size brackets in bytes
export const SIZE_CATEGORIES = {
    empty: { min: 0, max: 0, key: 'size_empty' },
    tiny: { min: 1, max: 10 * 1024, key: 'size_tiny' },             // 1 B to 10 KB
    small: { min: 10 * 1024 + 1, max: 1024 * 1024, key: 'size_small' }, // 10 KB to 1 MB
    medium: { min: 1024 * 1024 + 1, max: 100 * 1024 * 1024, key: 'size_medium' }, // 1 MB to 100 MB
    large: { min: 100 * 1024 * 1024 + 1, max: 1024 * 1024 * 1024, key: 'size_large' }, // 100 MB to 1 GB
    huge: { min: 1024 * 1024 * 1024 + 1, max: Infinity, key: 'size_huge' }, // > 1 GB
};

export type SizeCategoryKey = keyof typeof SIZE_CATEGORIES;

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
    availableSizeCategories: Set<SizeCategoryKey>;
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
            onChange(newSet.size === 0 ? null : newSet);
        } else {
            const newSet = new Set(selectedSizes);
            if (newSet.has(cat)) {
                newSet.delete(cat);
            } else {
                newSet.add(cat);
            }
            if (newSet.size === categoryKeys.length || newSet.size === 0) {
                onChange(null);
            } else {
                onChange(newSet);
            }
        }
    };

    const handleSelectAll = () => onChange(null);
    const handleClearAll = () => onChange(new Set());

    return (
        <div style={{ zIndex: 10001, pointerEvents: 'auto', fontSize: '0.75rem' }}>
            <div
                ref={menuRef}
                className="context-menu"
                style={{
                    position: 'fixed',
                    left: `${position.left}px`,
                    top: `${position.top}px`,
                    display: 'flex',
                    flexDirection: 'column',
                    minWidth: '200px'
                }}
            >
                <div style={{ padding: '4px 8px', fontSize: '0.75rem', fontWeight: 600, color: 'var(--text-muted)', borderBottom: '1px solid var(--border-color)', marginBottom: '4px' }}>
                    {t('filter_by_size' as any) || 'Filter by size'}
                </div>

                <div style={{ flex: 1, padding: '4px 0' }}>
                    {categoryKeys.map(cat => {
                        const info = SIZE_CATEGORIES[cat];
                        const isChecked = selectedSizes === null || selectedSizes.has(cat);
                        const label = t(info.key as any) || cat;
                        return (
                            <div
                                key={cat}
                                className={cx("context-menu-item")}
                                onClick={() => handleToggle(cat)}
                                style={{ display: 'flex', alignItems: 'center', gap: '8px', padding: '4px 12px' }}
                            >
                                <div style={{ width: '14px', height: '14px', border: '1px solid var(--border-color)', borderRadius: '3px', display: 'flex', alignItems: 'center', justifyContent: 'center', background: isChecked ? 'var(--accent-color)' : 'transparent', borderColor: isChecked ? 'var(--accent-color)' : 'var(--border-color)' }}>
                                    {isChecked && <Check size={10} color="#fff" strokeWidth={3} />}
                                </div>
                                <span>{label}</span>
                            </div>
                        );
                    })}
                </div>
                <div style={{ borderTop: '1px solid var(--border-color)', marginTop: '4px', paddingTop: '4px', display: 'flex', gap: '4px', paddingBottom: '4px', paddingLeft: '8px', paddingRight: '8px' }}>
                    <button className="btn ghost" style={{ flex: 1, fontSize: '0.7rem', padding: '2px 4px' }} onClick={handleSelectAll}>{t('all' as any) || 'All'}</button>
                    <button className="btn ghost" style={{ flex: 1, fontSize: '0.7rem', padding: '2px 4px' }} onClick={handleClearAll}>{t('none' as any) || 'None'}</button>
                </div>
            </div>
        </div>
    );
};
