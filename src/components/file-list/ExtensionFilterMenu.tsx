import React, { useRef, useEffect } from 'react';
import cx from 'classnames';
import { Check } from 'lucide-react';

import { TFunc } from '../../i18n';

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
            onChange(newSet.size === 0 ? null : newSet);
        } else {
            const newSet = new Set(selectedExtensions);
            if (newSet.has(ext)) {
                newSet.delete(ext);
            } else {
                newSet.add(ext);
            }
            // If all are selected, or none are selected, just reset to null
            if (newSet.size === availableExtensions.length || newSet.size === 0) {
                onChange(null);
            } else {
                onChange(newSet);
            }
        }
    };

    const handleSelectAll = () => onChange(null);

    const handleClearAll = () => onChange(new Set()); // This will hide all files

    return (
        <div style={{ zIndex: 10001, pointerEvents: 'auto', fontSize: '0.75rem' }}>
            <div
                ref={menuRef}
                className="context-menu"
                style={{
                    position: 'fixed',
                    left: `${position.left}px`,
                    top: `${position.top}px`,
                    maxHeight: '300px',
                    display: 'flex',
                    flexDirection: 'column',
                    minWidth: '150px'
                }}
            >
                <div style={{ padding: '4px 8px', fontSize: '0.75rem', fontWeight: 600, color: 'var(--text-muted)', borderBottom: '1px solid var(--border-color)', marginBottom: '4px' }}>
                    {t('filter_by_ext' as any) || 'Filter by extension'}
                </div>

                <div style={{ overflowY: 'auto', flex: 1, padding: '4px 0' }}>
                    {availableExtensions.map(ext => {
                        const isChecked = selectedExtensions === null || selectedExtensions.has(ext);
                        const displayExt = ext === '' ? `(${t('none_fem' as any) || 'None'})` : ext.toUpperCase();
                        return (
                            <div
                                key={ext}
                                className={cx("context-menu-item")}
                                onClick={() => handleToggle(ext)}
                                style={{ display: 'flex', alignItems: 'center', gap: '8px', padding: '4px 12px' }}
                            >
                                <div style={{ width: '14px', height: '14px', border: '1px solid var(--border-color)', borderRadius: '3px', display: 'flex', alignItems: 'center', justifyContent: 'center', background: isChecked ? 'var(--accent-color)' : 'transparent', borderColor: isChecked ? 'var(--accent-color)' : 'var(--border-color)' }}>
                                    {isChecked && <Check size={10} color="#fff" strokeWidth={3} />}
                                </div>
                                <span>{displayExt}</span>
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
