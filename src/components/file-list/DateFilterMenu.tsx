import React, { useRef, useEffect } from 'react';
import cx from 'classnames';
import { Check } from 'lucide-react';
import { TFunc } from '../../i18n';

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
            onChange(newSet.size === 0 ? null : newSet);
        } else {
            const newSet = new Set(selectedDates);
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
                    {t('filter_by_date' as any) || 'Filter by date'}
                </div>

                <div style={{ flex: 1, padding: '4px 0' }}>
                    {categoryKeys.map(cat => {
                        const key = DATE_CATEGORIES[cat];
                        const isChecked = selectedDates === null || selectedDates.has(cat);
                        const label = t(key as any) || cat;
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
