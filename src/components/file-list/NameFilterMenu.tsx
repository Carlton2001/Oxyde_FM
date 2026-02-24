import React, { useRef, useEffect } from 'react';
import { X, Search } from 'lucide-react';
import { TFunc } from '../../i18n';

interface NameFilterMenuProps {
    x: number;
    y: number;
    value: string;
    onChange: (val: string | null) => void;
    onClose: () => void;
    t: TFunc;
}

export const NameFilterMenu: React.FC<NameFilterMenuProps> = ({
    x, y, value, onChange, onClose, t
}) => {
    const menuRef = useRef<HTMLDivElement>(null);
    const inputRef = useRef<HTMLInputElement>(null);

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
        if (inputRef.current) inputRef.current.focus();

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

    return (
        <div style={{ zIndex: 10001, pointerEvents: 'auto' }}>
            <div
                ref={menuRef}
                className="context-menu"
                style={{
                    position: 'fixed',
                    left: `${position.left}px`,
                    top: `${position.top}px`,
                    padding: '8px',
                    minWidth: '200px',
                    display: 'flex',
                    flexDirection: 'column',
                    gap: '8px'
                }}
            >
                <div style={{ fontSize: '0.75rem', fontWeight: 600, color: 'var(--text-muted)', marginBottom: '4px' }}>
                    {t('filter_by_name' as any) || 'Filter by name'}
                </div>
                <div style={{ position: 'relative', display: 'flex', alignItems: 'center' }}>
                    <Search size={14} style={{ position: 'absolute', left: '8px', color: 'var(--text-muted)' }} />
                    <input
                        ref={inputRef}
                        type="text"
                        className="input-field"
                        value={value}
                        onChange={(e) => onChange(e.target.value || null)}
                        style={{ paddingLeft: '28px', paddingRight: '28px', width: '100%', fontSize: '0.8rem' }}
                    />
                    {value && (
                        <button
                            className="btn-icon ghost"
                            onClick={() => onChange(null)}
                            style={{ position: 'absolute', right: '4px', width: '20px', height: '20px' }}
                        >
                            <X size={12} />
                        </button>
                    )}
                </div>
            </div>
        </div>
    );
};
