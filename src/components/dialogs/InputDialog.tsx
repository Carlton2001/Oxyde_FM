import React, { useState, useEffect, useRef } from 'react';
import { Edit2, FolderPlus, HelpCircle, X } from 'lucide-react';
import { TFunc } from '../../i18n';
import { useDraggable } from '../../hooks/useDraggable';
import '../../styles/components/Dialogs.css';

interface InputDialogProps {
    isOpen: boolean;
    title: string;
    label?: string;
    onClose: () => void;
    onSubmit: (value: string) => void;
    t: TFunc;
    initialValue?: string;
    placeholder?: string;
    confirmLabel?: string;
    icon?: 'rename' | 'new_folder' | 'default';
}

export const InputDialog: React.FC<InputDialogProps> = ({
    isOpen,
    title,
    label,
    onClose,
    onSubmit,
    t,
    initialValue = '',
    placeholder = '',
    confirmLabel,
    icon = 'default'
}) => {
    const dragRef = useRef<HTMLDivElement>(null);
    const { position, handleMouseDown } = useDraggable({ initialPosition: { x: 0, y: 0 }, dragRef });
    const [value, setValue] = useState(initialValue);
    const inputRef = useRef<HTMLInputElement>(null);

    useEffect(() => {
        if (isOpen) {
            setValue(initialValue);
            setTimeout(() => {
                if (inputRef.current) {
                    inputRef.current.focus();
                    const dotIndex = initialValue.lastIndexOf('.');
                    if (dotIndex > 0 && icon === 'rename') {
                        inputRef.current.setSelectionRange(0, dotIndex);
                    } else {
                        inputRef.current.select();
                    }
                }
            }, 50);
        }
    }, [isOpen, initialValue]);

    const handleSubmit = (e?: React.FormEvent) => {
        e?.preventDefault();
        if (value.trim()) {
            onSubmit(value.trim());
        }
    };

    const getIcon = () => {
        switch (icon) {
            case 'new_folder': return <FolderPlus size={16} />;
            case 'rename': return <Edit2 size={16} />;
            default: return <HelpCircle size={16} />;
        }
    };

    if (!isOpen) return null;

    return (
        <div className="modal-overlay" onClick={onClose} style={{ zIndex: 12000 }}>
            <div
                ref={dragRef}
                className="modal"
                onClick={(e) => e.stopPropagation()}
                style={{
                    width: '25rem',
                    transform: `translate(${position.x}px, ${position.y}px)`,
                    transition: 'none'
                }}
            >
                <div className="modal-header" onMouseDown={handleMouseDown}>
                    <div className="modal-title">
                        {getIcon()}
                        <span style={{ marginLeft: '0.5rem' }}>{title}</span>
                    </div>
                    <button className="btn-icon" onClick={onClose}>
                        <X size={16} />
                    </button>
                </div>

                <div className="modal-content">
                    <form onSubmit={handleSubmit}>
                        {label && (
                            <div style={{ marginBottom: '0.3125rem', fontSize: '0.85rem', color: 'var(--text-muted)' }}>
                                {label}
                            </div>
                        )}
                        <input
                            ref={inputRef}
                            type="text"
                            className="input-field"
                            value={value}
                            onChange={(e) => setValue(e.target.value)}
                            placeholder={placeholder}
                            style={{ width: '100%', boxSizing: 'border-box' }}
                        />
                    </form>
                </div>

                <div className="modal-footer">
                    <button type="button" className="btn secondary" onClick={onClose}>
                        {t('cancel' as any)}
                    </button>
                    <button
                        type="button"
                        className="btn primary"
                        disabled={!value.trim()}
                        onClick={() => handleSubmit()}
                    >
                        {confirmLabel || t('create' as any)}
                    </button>
                </div>
            </div>
        </div>
    );
};

