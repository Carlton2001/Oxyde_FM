import React, { useRef, useEffect } from 'react';
import { TFunc } from '../../i18n';
import { FilterMenuCheckbox } from './FilterMenuCheckbox';
import './FilterMenu.css';

// Date Categories
export const DATE_CATEGORIES = {
    today: 'date_today',
    yesterday: 'date_yesterday',
    this_week: 'date_this_week',
    last_week: 'date_last_week',
    this_month: 'date_this_month',
    older: 'date_older',
};

export type DateCategoryKey = keyof typeof DATE_CATEGORIES;

export const getDateCategoryForFile = (mtime: number): DateCategoryKey => {
    const now = new Date();
    const date = new Date(mtime);

    // Reset hours for comparison
    const today = new Date(now.getFullYear(), now.getMonth(), now.getDate());
    const yesterday = new Date(today);
    yesterday.setDate(yesterday.getDate() - 1);

    // Day of week (0 is Sunday)
    const currentDay = now.getDay();
    const diffToMonday = currentDay === 0 ? 6 : currentDay - 1;
    const startOfThisWeek = new Date(today);
    startOfThisWeek.setDate(startOfThisWeek.getDate() - diffToMonday);

    const startOfLastWeek = new Date(startOfThisWeek);
    startOfLastWeek.setDate(startOfLastWeek.getDate() - 7);

    const startOfThisMonth = new Date(now.getFullYear(), now.getMonth(), 1);

    if (date >= today) return 'today';
    if (date >= yesterday) return 'yesterday';
    if (date >= startOfThisWeek) return 'this_week';
    if (date >= startOfLastWeek) return 'last_week';
    if (date >= startOfThisMonth) return 'this_month';
    return 'older';
};

interface DateFilterMenuProps {
    x: number;
    y: number;
    selectedDates: Set<DateCategoryKey> | null;
    availableDateCategories: Set<DateCategoryKey>;
    onChange: (dates: Set<DateCategoryKey> | null) => void;
    onClose: () => void;
    t: TFunc;
}

export const DateFilterMenu: React.FC<DateFilterMenuProps> = ({
    x, y, selectedDates, availableDateCategories, onChange, onClose, t
}) => {
    const menuRef = useRef<HTMLDivElement>(null);
    const categoryKeys = (Object.keys(DATE_CATEGORIES) as DateCategoryKey[]).filter(cat => availableDateCategories.has(cat));

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

    const handleToggle = (cat: DateCategoryKey) => {
        if (selectedDates === null) {
            const newSet = new Set(categoryKeys);
            newSet.delete(cat);
            onChange(newSet.size === 0 ? new Set() : newSet);
        } else {
            const newSet = new Set(selectedDates);
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
                    {t('filter_by_date' as any) || 'Filter by date'}
                </div>

                <div className="filter-menu-list">
                    {categoryKeys.map(cat => {
                        const key = DATE_CATEGORIES[cat];
                        const isChecked = selectedDates === null || selectedDates.has(cat);
                        const label = t(key as any) || cat;
                        return (
                            <FilterMenuCheckbox
                                key={cat}
                                checked={isChecked}
                                label={label}
                                onClick={() => handleToggle(cat)}
                            />
                        );
                    })}
                </div>
                <div className="filter-menu-footer">
                    {selectedDates === null ? (
                        <button className="btn ghost" onClick={handleClearAll}>{t('none' as any) || 'None'}</button>
                    ) : (
                        <button className="btn ghost" onClick={handleSelectAll}>{t('all' as any) || 'All'}</button>
                    )}
                </div>
            </div>
        </div>
    );
};
